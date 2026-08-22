// Ownership system for Palladium
// "Every value has a single owner"

pub mod borrow_checker;

pub use borrow_checker::BorrowChecker;

use crate::ast::Expr;
use crate::errors::{CompileError, Result, Span};
use std::collections::HashMap;

/// Ownership state of a value
#[derive(Debug, Clone, PartialEq)]
pub enum Ownership {
    /// Value is owned by this binding
    Owned,
    /// Value is borrowed immutably
    Borrowed { lifetime: Lifetime },
    /// Value is borrowed mutably
    BorrowedMut { lifetime: Lifetime },
    /// Value has been moved
    Moved,
}

/// Lifetime representation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Lifetime {
    /// Static lifetime ('static)
    Static,
    /// Named lifetime ('a, 'b, etc.)
    Named(String),
    /// Anonymous lifetime
    Anonymous(u32),
    /// Scope lifetime (for local scopes)
    Scope(u32),
}

/// Reference type
#[derive(Debug, Clone, PartialEq)]
pub enum RefKind {
    /// Immutable reference (&T)
    Shared,
    /// Mutable reference (&mut T)
    Mutable,
}

/// Borrow information
#[derive(Debug, Clone)]
pub struct Borrow {
    /// What is being borrowed
    pub place: Place,
    /// Kind of borrow
    pub kind: RefKind,
    /// Lifetime of the borrow
    pub lifetime: Lifetime,
    /// Where the borrow occurs
    pub span: Span,
    /// Depth of the scope that created this borrow, so that `exit_scope` can
    /// end it.
    ///
    /// The lifetime above cannot answer that question. `Lifetime::Scope` is
    /// declared but **never constructed** — the only two mentions in the tree
    /// are the equality test in `exit_scope` and the `Display` arm — so the
    /// `retain` there matched nothing and `borrows` grew monotonically for the
    /// whole of `check_program`. Every other borrow gets `new_lifetime()`
    /// (`Anonymous`), which nothing ends outside a call. Measured on plain
    /// `main` before this field existed: two sibling functions that each do
    /// `let mut v = 1; let r = &mut v;` were refused with `ConflictingBorrows`,
    /// the second `v` colliding with the first function's borrow of a
    /// completely different `v`.
    pub scope: u32,
}

/// Place in memory (what can be borrowed)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Place {
    /// Local variable
    Local(String),
    /// Field of a struct
    Field { base: Box<Place>, field: String },
    /// Array element
    Index { base: Box<Place>, index: String },
    /// Temporary value
    Temp(u32),
}

/// Ownership context for tracking ownership state
#[derive(Default)]
pub struct OwnershipContext {
    /// Current ownership state of each place
    ownership: HashMap<Place, Ownership>,
    /// Active borrows
    borrows: Vec<Borrow>,
    /// Current scope ID
    current_scope: u32,
    /// Next anonymous lifetime ID
    next_lifetime: u32,
    /// Next temporary ID
    next_temp: u32,
    /// Lifetime constraints (outlives relationships)
    constraints: Vec<LifetimeConstraint>,
    /// Places *declared* by each open scope, innermost frame last, each paired
    /// with whatever entry it shadowed.
    ///
    /// `ownership` above is a flat map with no notion of which scope put an
    /// entry there, and `exit_scope` used to drop borrows only. So an entry
    /// survived the binding that created it — for the whole rest of the
    /// compilation, across block *and* function boundaries. That is only
    /// invisible while every reader keys off "is this place moved"; the moment
    /// a reader keys off "does this name exist here" (which
    /// `BorrowChecker::check_expr` now does, to stop a local being laundered by
    /// a same-named function) a dead entry becomes a live lexical binding and
    /// refuses a valid program. Measured: `fn a()` moving a local named
    /// `helper` made `main`'s call to the real `fn helper()` fail with
    /// "Use of moved value: helper".
    ///
    /// Existence in this map is therefore made to mean *in scope*, by recording
    /// each binder and undoing it at scope exit.
    scope_decls: Vec<Vec<(Place, Option<Ownership>)>>,
}

/// Lifetime constraint (e.g., 'a: 'b means 'a outlives 'b)
#[derive(Debug, Clone)]
pub struct LifetimeConstraint {
    pub longer: Lifetime,
    pub shorter: Lifetime,
}

impl OwnershipContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter a new scope
    pub fn enter_scope(&mut self) {
        self.current_scope += 1;
        self.scope_decls.push(Vec::new());
    }

    /// Exit a scope, invalidating borrows and retiring the scope's bindings.
    pub fn exit_scope(&mut self) {
        // End every borrow this scope (or anything nested in it that failed to
        // clean up) created, and restore the ownership state of the places they
        // borrowed. Selecting on `scope` rather than on `Lifetime::Scope` is the
        // point: that variant is never constructed, so the retain this replaces
        // matched nothing at all. See `Borrow::scope`.
        let closing = self.current_scope;
        self.release_borrows(|borrow| borrow.scope >= closing);

        // Retire the bindings this scope introduced, innermost declaration
        // first, so that a name declared twice in one scope unwinds to the
        // entry that was there before the scope opened.
        //
        // Only *declarations* are undone. A move performed inside this scope on
        // a place that belongs to an ENCLOSING scope must survive — restoring a
        // whole snapshot of `ownership` would resurrect the moved-out value and
        // turn a genuine use-after-move into an accept:
        //
        //     let s: S = S { v: 1 };
        //     if c { let t: S = s; }   // moves the OUTER s
        //     print_int(s.v);          // must stay refused
        //
        // so the undo is per-binder, not per-map.
        if let Some(declared) = self.scope_decls.pop() {
            for (place, shadowed) in declared.into_iter().rev() {
                match shadowed {
                    Some(previous) => {
                        self.ownership.insert(place, previous);
                    }
                    None => {
                        self.ownership.remove(&place);
                    }
                }
            }
        }

        self.current_scope -= 1;
    }

    /// Record that `place` is bound by the innermost open scope, so that
    /// `exit_scope` can retire it.
    ///
    /// Deliberately does NOT touch the current ownership state: the binder's
    /// own state is set by `init_owned` or `move_value` right after, and
    /// pre-setting `Owned` here would make `let s: S = s;` accept a use of an
    /// already-moved `s`.
    pub fn declare(&mut self, place: &Place) {
        let shadowed = self.ownership.get(place).cloned();
        if let Some(frame) = self.scope_decls.last_mut() {
            frame.push((place.clone(), shadowed));
        }
    }

    /// End every borrow taken with `lifetime`, restoring the ownership state of
    /// each place that no longer has a live borrow.
    ///
    /// Dropping the borrow from `borrows` is not enough on its own: `borrow()`
    /// also stamps the place with `Borrowed`/`BorrowedMut` (see the ownership
    /// update at the end of `borrow`), and a place left in `BorrowedMut` is
    /// rejected by the *next* `borrow()` even when nothing borrows it any more.
    /// So every affected place is recomputed from the borrows that remain, and
    /// falls back to `Owned` when there are none.
    ///
    /// A place that is `Moved` stays moved — ending a borrow never revives it.
    pub fn end_borrows(&mut self, lifetime: &Lifetime) {
        self.release_borrows(|borrow| &borrow.lifetime == lifetime);
    }

    /// Drop every borrow matching `doomed` and recompute the ownership state of
    /// each place that lost one.
    ///
    /// The body is what `end_borrows` used to be; it is shared because
    /// `exit_scope` has to end borrows too and must not re-derive this
    /// recomputation. The only difference between the two callers is which
    /// borrows they select.
    fn release_borrows(&mut self, doomed: impl Fn(&Borrow) -> bool) {
        let mut affected: Vec<Place> = Vec::new();
        self.borrows.retain(|borrow| {
            if doomed(borrow) {
                affected.push(borrow.place.clone());
                false
            } else {
                true
            }
        });

        for place in affected {
            if !matches!(
                self.ownership.get(&place),
                Some(Ownership::Borrowed { .. }) | Some(Ownership::BorrowedMut { .. })
            ) {
                continue;
            }

            let state = {
                let mut mutable = None;
                let mut shared = None;
                for remaining in self.borrows.iter().filter(|b| b.place == place) {
                    match remaining.kind {
                        RefKind::Mutable => mutable = Some(remaining.lifetime.clone()),
                        RefKind::Shared => shared = Some(remaining.lifetime.clone()),
                    }
                }
                match (mutable, shared) {
                    (Some(lifetime), _) => Ownership::BorrowedMut { lifetime },
                    (None, Some(lifetime)) => Ownership::Borrowed { lifetime },
                    (None, None) => Ownership::Owned,
                }
            };

            self.ownership.insert(place, state);
        }
    }

    /// Create a new anonymous lifetime
    pub fn new_lifetime(&mut self) -> Lifetime {
        let lifetime = Lifetime::Anonymous(self.next_lifetime);
        self.next_lifetime += 1;
        lifetime
    }

    /// Create a new temporary place
    pub fn new_temp(&mut self) -> Place {
        let temp = Place::Temp(self.next_temp);
        self.next_temp += 1;
        temp
    }

    /// Initialize a new owned value
    pub fn init_owned(&mut self, place: Place) {
        self.ownership.insert(place, Ownership::Owned);
    }

    /// The place whose recorded ownership governs `place`.
    ///
    /// Only *locals* are ever registered — every `init_owned` call site is a
    /// parameter, a `let`, a `for` variable or a pattern binding. A projection
    /// such as `s.a` or `xs[0]` therefore has no entry of its own, and looking
    /// it up directly yields `None`, which the callers below report as "use of
    /// uninitialized value". That is wrong: you may use `s.a` exactly when you
    /// may use `s`, so a projection inherits the state of the nearest ancestor
    /// that *is* registered.
    ///
    /// Returns `None` only when nothing on the projection chain is known, which
    /// is the genuine uninitialized case.
    fn resolve_place<'p>(&self, place: &'p Place) -> Option<&'p Place> {
        let mut current = place;
        loop {
            if self.ownership.contains_key(current) {
                return Some(current);
            }
            match current {
                Place::Field { base, .. } | Place::Index { base, .. } => current = base.as_ref(),
                _ => return None,
            }
        }
    }

    /// Ownership state governing `place`, following projections to their base.
    fn effective_ownership(&self, place: &Place) -> Option<Ownership> {
        self.resolve_place(place)
            .and_then(|key| self.ownership.get(key))
            .cloned()
    }

    /// Move a value from one place to another
    pub fn move_value(&mut self, from: Place, to: Place, span: Span) -> Result<()> {
        // Check if the source can be moved
        match self.effective_ownership(&from) {
            Some(Ownership::Owned) => {
                // Move is allowed
                self.ownership.insert(from.clone(), Ownership::Moved);
                self.ownership.insert(to, Ownership::Owned);
                Ok(())
            }
            Some(Ownership::Borrowed { .. }) => {
                Err(CompileError::CannotMoveOutOfBorrowedContent { span: Some(span) })
            }
            Some(Ownership::BorrowedMut { .. }) => {
                Err(CompileError::CannotMoveOutOfBorrowedContent { span: Some(span) })
            }
            Some(Ownership::Moved) => Err(CompileError::UseOfMovedValue {
                name: from.to_string(),
                span: Some(span),
            }),
            None => Err(CompileError::UseOfUninitializedValue {
                name: from.to_string(),
                span: Some(span),
            }),
        }
    }

    /// Borrow a value
    pub fn borrow(
        &mut self,
        place: Place,
        kind: RefKind,
        lifetime: Lifetime,
        span: Span,
    ) -> Result<()> {
        // Check if the place can be borrowed
        match self.effective_ownership(&place) {
            Some(Ownership::Owned) | Some(Ownership::Borrowed { .. }) => {
                // Check for conflicting borrows
                for existing_borrow in &self.borrows {
                    if existing_borrow.place == place {
                        match (&existing_borrow.kind, &kind) {
                            (RefKind::Mutable, _) | (_, RefKind::Mutable) => {
                                return Err(CompileError::ConflictingBorrows {
                                    message: format!("cannot borrow `{}` as {} because it is also borrowed as {}", 
                                        place,
                                        if kind == RefKind::Mutable { "mutable" } else { "immutable" },
                                        if existing_borrow.kind == RefKind::Mutable { "mutable" } else { "immutable" }
                                    ),
                                    span: Some(span),
                                });
                            }
                            _ => {} // Multiple immutable borrows are allowed
                        }
                    }
                }

                // Add the new borrow
                self.borrows.push(Borrow {
                    place: place.clone(),
                    kind: kind.clone(),
                    lifetime: lifetime.clone(),
                    span,
                    scope: self.current_scope,
                });

                // Update ownership state
                match kind {
                    RefKind::Shared => {
                        if !matches!(
                            self.ownership.get(&place),
                            Some(Ownership::BorrowedMut { .. })
                        ) {
                            self.ownership
                                .insert(place, Ownership::Borrowed { lifetime });
                        }
                    }
                    RefKind::Mutable => {
                        self.ownership
                            .insert(place, Ownership::BorrowedMut { lifetime });
                    }
                }

                Ok(())
            }
            Some(Ownership::BorrowedMut { .. }) => Err(CompileError::ConflictingBorrows {
                message: format!(
                    "cannot borrow `{}` because it is already mutably borrowed",
                    place
                ),
                span: Some(span),
            }),
            Some(Ownership::Moved) => Err(CompileError::UseOfMovedValue {
                name: place.to_string(),
                span: Some(span),
            }),
            None => Err(CompileError::UseOfUninitializedValue {
                name: place.to_string(),
                span: Some(span),
            }),
        }
    }

    /// Check if a place is currently borrowed
    pub fn is_borrowed(&self, place: &Place) -> bool {
        self.borrows.iter().any(|b| &b.place == place)
    }

    /// Add a lifetime constraint
    pub fn add_constraint(&mut self, longer: Lifetime, shorter: Lifetime) {
        self.constraints
            .push(LifetimeConstraint { longer, shorter });
    }

    /// Get the ownership state of a place
    pub fn get_ownership(&self, place: &Place) -> Option<&Ownership> {
        self.ownership.get(place)
    }
}

/// Convert expression to a place (if possible)
pub fn expr_to_place(expr: &Expr) -> Option<Place> {
    match expr {
        Expr::Ident(name) => Some(Place::Local(name.clone())),
        Expr::FieldAccess { object, field, .. } => expr_to_place(object).map(|base| Place::Field {
            base: Box::new(base),
            field: field.clone(),
        }),
        Expr::Index { array, index, .. } => {
            // For simplicity, we convert index to string
            // In a real implementation, we'd need more sophisticated handling
            if let (Some(base), Expr::Integer(i)) = (expr_to_place(array), index.as_ref()) {
                Some(Place::Index {
                    base: Box::new(base),
                    index: i.to_string(),
                })
            } else {
                None
            }
        }
        Expr::Deref { expr, .. } => {
            // Dereferencing a reference gives us the place it points to
            expr_to_place(expr)
        }
        _ => None,
    }
}

impl std::fmt::Display for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Place::Local(name) => write!(f, "{}", name),
            Place::Field { base, field } => write!(f, "{}.{}", base, field),
            Place::Index { base, index } => write!(f, "{}[{}]", base, index),
            Place::Temp(id) => write!(f, "_temp{}", id),
        }
    }
}

impl std::fmt::Display for Lifetime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lifetime::Static => write!(f, "'static"),
            Lifetime::Named(name) => write!(f, "'{}", name),
            Lifetime::Anonymous(id) => write!(f, "'_{}", id),
            Lifetime::Scope(id) => write!(f, "'scope{}", id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_ownership() {
        let mut ctx = OwnershipContext::new();
        let x = Place::Local("x".to_string());

        // Initialize owned value
        ctx.init_owned(x.clone());
        assert_eq!(ctx.get_ownership(&x), Some(&Ownership::Owned));

        // Move value
        let y = Place::Local("y".to_string());
        ctx.move_value(x.clone(), y.clone(), Span::dummy()).unwrap();
        assert_eq!(ctx.get_ownership(&x), Some(&Ownership::Moved));
        assert_eq!(ctx.get_ownership(&y), Some(&Ownership::Owned));
    }

    #[test]
    fn test_borrow_checking() {
        let mut ctx = OwnershipContext::new();
        let x = Place::Local("x".to_string());

        ctx.init_owned(x.clone());

        // Immutable borrow
        let lifetime = ctx.new_lifetime();
        ctx.borrow(x.clone(), RefKind::Shared, lifetime.clone(), Span::dummy())
            .unwrap();

        // Second immutable borrow should succeed
        ctx.borrow(x.clone(), RefKind::Shared, lifetime.clone(), Span::dummy())
            .unwrap();

        // Mutable borrow should fail
        let result = ctx.borrow(x.clone(), RefKind::Mutable, lifetime, Span::dummy());
        assert!(result.is_err());
    }

    /// Ending the last borrow must put the place back to `Owned`. Dropping the
    /// borrow while leaving the place stamped `BorrowedMut` is what made a value
    /// permanently unusable after being passed to a function.
    #[test]
    fn test_end_borrows_restores_owned_and_allows_reborrow() {
        let mut ctx = OwnershipContext::new();
        let x = Place::Local("x".to_string());
        ctx.init_owned(x.clone());

        let first = ctx.new_lifetime();
        ctx.borrow(x.clone(), RefKind::Mutable, first.clone(), Span::dummy())
            .unwrap();
        assert!(ctx.is_borrowed(&x));

        ctx.end_borrows(&first);

        assert!(!ctx.is_borrowed(&x));
        assert_eq!(ctx.get_ownership(&x), Some(&Ownership::Owned));

        // A second mutable borrow must now succeed.
        let second = ctx.new_lifetime();
        ctx.borrow(x.clone(), RefKind::Mutable, second, Span::dummy())
            .unwrap();
    }

    /// Ending one lifetime must not release a borrow taken under another.
    #[test]
    fn test_end_borrows_keeps_borrows_of_other_lifetimes() {
        let mut ctx = OwnershipContext::new();
        let x = Place::Local("x".to_string());
        ctx.init_owned(x.clone());

        let outer = ctx.new_lifetime();
        let inner = ctx.new_lifetime();
        ctx.borrow(x.clone(), RefKind::Shared, outer.clone(), Span::dummy())
            .unwrap();
        ctx.borrow(x.clone(), RefKind::Shared, inner.clone(), Span::dummy())
            .unwrap();

        ctx.end_borrows(&inner);

        // The outer borrow survives, so the place stays borrowed...
        assert!(ctx.is_borrowed(&x));
        assert_eq!(
            ctx.get_ownership(&x),
            Some(&Ownership::Borrowed { lifetime: outer })
        );
        // ...and a mutable borrow is still a conflict.
        let extra = ctx.new_lifetime();
        assert!(ctx
            .borrow(x.clone(), RefKind::Mutable, extra, Span::dummy())
            .is_err());
    }

    /// A projection inherits the ownership state of its base, so borrowing
    /// `x.a` is legal whenever `x` is. Only locals are ever registered, so
    /// without this the field looked uninitialized.
    #[test]
    fn test_projection_inherits_base_ownership() {
        let mut ctx = OwnershipContext::new();
        let x = Place::Local("x".to_string());
        let field = Place::Field {
            base: Box::new(x.clone()),
            field: "a".to_string(),
        };
        ctx.init_owned(x);

        let lifetime = ctx.new_lifetime();
        ctx.borrow(field, RefKind::Shared, lifetime, Span::dummy())
            .expect("borrowing a field of an owned local must be allowed");
    }

    /// A projection of an unknown base is still genuinely uninitialized.
    #[test]
    fn test_projection_of_unknown_base_is_uninitialized() {
        let mut ctx = OwnershipContext::new();
        let field = Place::Field {
            base: Box::new(Place::Local("nope".to_string())),
            field: "a".to_string(),
        };

        let lifetime = ctx.new_lifetime();
        let result = ctx.borrow(field, RefKind::Shared, lifetime, Span::dummy());
        assert!(matches!(
            result,
            Err(CompileError::UseOfUninitializedValue { .. })
        ));
    }

    /// Ending a borrow must never revive a moved value.
    #[test]
    fn test_end_borrows_does_not_revive_moved_value() {
        let mut ctx = OwnershipContext::new();
        let x = Place::Local("x".to_string());
        let y = Place::Local("y".to_string());
        ctx.init_owned(x.clone());

        let lifetime = ctx.new_lifetime();
        ctx.borrow(x.clone(), RefKind::Shared, lifetime.clone(), Span::dummy())
            .unwrap();
        ctx.end_borrows(&lifetime);

        ctx.move_value(x.clone(), y, Span::dummy()).unwrap();
        assert_eq!(ctx.get_ownership(&x), Some(&Ownership::Moved));

        // A stale lifetime must not resurrect it.
        ctx.end_borrows(&lifetime);
        assert_eq!(ctx.get_ownership(&x), Some(&Ownership::Moved));
    }
}
