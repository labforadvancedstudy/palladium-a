// Ownership system for Palladium
// "Every value has a single owner"

pub mod borrow_checker;

pub use borrow_checker::BorrowChecker;

use crate::ast::Expr;
use crate::errors::{CompileError, Result, Span};
use std::collections::{HashMap, HashSet};

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
    /// How many loop bodies enclose the statement being checked. See
    /// `write_new_value` for the one decision that reads it.
    loop_depth: u32,
    /// Places *declared* by each open scope, innermost frame last, each paired
    /// with whatever it shadowed (`Shadowed`).
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
    ///
    /// A binder owns its whole SUBTREE, not just its own key: `d.inner` and
    /// `d.inner.w` get entries of their own when they are moved, assigned or
    /// borrowed, and they are retired with `d`.
    scope_decls: Vec<Vec<Shadowed>>,
}

/// Lifetime constraint (e.g., 'a: 'b means 'a outlives 'b)
#[derive(Debug, Clone)]
pub struct LifetimeConstraint {
    pub longer: Lifetime,
    pub shorter: Lifetime,
}

/// What `declare` took out of view when a binder came into scope, so that
/// `exit_scope` can put it back.
///
/// IT IS A SUBTREE, because state is keyed by a `Place` and a `Place` is a
/// NAME. Recording only the binder's own key retired `d` and left `d.inner`
/// behind: measured, `fn a(d: D) { let x: In = d.inner; … }` followed by
/// `fn b(d: D) { let y: In = d.inner; … }` refused `b` with "Use of moved
/// value: d.inner" — `a`'s field move outlived `a` and was found by
/// `resolve_place` before `b`'s freshly owned `d`. The borrows are part of the
/// same subtree for the same reason: an inner `let d` matched an outer `&d` by
/// name and was refused as a conflicting borrow of a variable nothing borrows.
struct Shadowed {
    /// The binder.
    place: Place,
    /// Every entry at or below `place` when the binder was declared.
    states: Vec<(Place, Ownership)>,
    /// Every borrow at or below `place` when the binder was declared.
    borrows: Vec<Borrow>,
}

/// The flow-dependent half of an `OwnershipContext` — what each place holds and
/// which borrows are live — taken at one program point so that the paths of a
/// branch can each start from it and be joined after (`snapshot`, `restore`,
/// `join`). Scopes and counters are not in it: every arm opens and closes its
/// own scope, and lifetimes and temporaries must stay distinct across arms.
pub struct FlowState {
    ownership: HashMap<Place, Ownership>,
    borrows: Vec<Borrow>,
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
        // so the undo is per-binder, not per-map. It is per-SUBTREE within the
        // binder: everything at or below a retiring binder goes, including the
        // projection keys its moves and borrows created, and what that binder
        // shadowed comes back. Every borrow this scope took has already been
        // released above, so nothing live is lost with the subtree.
        if let Some(declared) = self.scope_decls.pop() {
            for shadowed in declared.into_iter().rev() {
                let Shadowed {
                    place,
                    states,
                    borrows,
                } = shadowed;
                self.ownership.retain(|key, _| !key.is_within(&place));
                self.ownership.extend(states);
                self.borrows.extend(borrows);
            }
        }

        // ...and what came back is subject to the same expiry. A borrow THIS
        // scope took of an outer binding that a later shadow then hid was not
        // in `borrows` when the release above ran, so it would otherwise come
        // back alive after its own `}`. Measured on 6a3d028: `if c { let r: &In
        // = &d; let d: In = …; } bump(&mut d);` was refused as a conflicting
        // borrow; 2563001 compiled it (review round 1, fable c36 / gpt-6-astra
        // shadow_restores_dead_borrow).
        self.release_borrows(|borrow| borrow.scope >= closing);

        self.current_scope -= 1;
    }

    /// Record that `place` is bound by the innermost open scope, so that
    /// `exit_scope` can retire it, and take what the name already carried out
    /// of view until then.
    ///
    /// The new binding is a different variable: no field has been moved out of
    /// it and nothing borrows it. So the entries strictly below `place` and
    /// every borrow at or below it are set aside — not dropped — and
    /// `exit_scope` gives them back to the binding they belong to.
    ///
    /// Deliberately does NOT touch the binder's OWN state: it is set by
    /// `init_owned` right after, and pre-setting `Owned` here would make
    /// `let s: S = s;` accept a use of an already-moved `s`.
    pub fn declare(&mut self, place: &Place) {
        if self.scope_decls.is_empty() {
            return;
        }
        let states: Vec<(Place, Ownership)> = self
            .ownership
            .iter()
            .filter(|(key, _)| key.is_within(place))
            .map(|(key, state)| (key.clone(), state.clone()))
            .collect();
        self.ownership
            .retain(|key, _| key == place || !key.is_within(place));
        let mut borrows = Vec::new();
        self.borrows.retain(|borrow| {
            if borrow.place.is_within(place) {
                borrows.push(borrow.clone());
                false
            } else {
                true
            }
        });
        if let Some(frame) = self.scope_decls.last_mut() {
            frame.push(Shadowed {
                place: place.clone(),
                states,
                borrows,
            });
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
    ///
    /// A projection DOES get an entry of its own when it is moved out of,
    /// assigned to or borrowed, and that entry is found here BEFORE its
    /// ancestors. So an entry below a place must never outlive what happens to
    /// the place: it is retired with its binder (`declare` / `exit_scope`),
    /// erased when an ancestor is moved (`mark_moved`), and erased when an
    /// ancestor is given a new value (`reinitialize`).
    fn resolve_place<'p>(&self, place: &'p Place) -> Option<&'p Place> {
        resolve_in(&self.ownership, place)
    }

    /// Ownership state governing `place`, following projections to their base.
    fn effective_ownership(&self, place: &Place) -> Option<Ownership> {
        self.resolve_place(place)
            .and_then(|key| self.ownership.get(key))
            .cloned()
    }

    /// Move out of `from`, naming no destination.
    ///
    /// SPLIT OUT OF `move_value` BECAUSE THE DESTINATION WRITE CAN CANCEL THE
    /// SOURCE WRITE. `move_value` marks the source `Moved` and then marks the
    /// destination `Owned`, and a `Place` is just a name: in `let s: S = s;` the
    /// two are the SAME key, so the second insert overwrote the first and the
    /// move vanished. Nested in a block that shadows an outer `s`, that made the
    /// outer binding survive its own move — accepted, while the identical program
    /// using a different inner name was correctly refused.
    ///
    /// A binder that wants both halves does them in order: move out of the source
    /// first, record what the binding shadows second (so it snapshots the
    /// POST-move state), initialize the new binding third.
    pub fn move_out_of(&mut self, from: Place, span: Span) -> Result<()> {
        match self.effective_ownership(&from) {
            Some(Ownership::Owned) => {
                self.refuse_move_under_a_borrow(&from, span)?;
                self.mark_moved(from);
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

    /// Move a value from one place to another
    pub fn move_value(&mut self, from: Place, to: Place, span: Span) -> Result<()> {
        // Check if the source can be moved
        match self.effective_ownership(&from) {
            Some(Ownership::Owned) => {
                // Move is allowed — out of a place nothing borrows, into one
                // nothing borrows.
                self.refuse_move_under_a_borrow(&from, span)?;
                self.refuse_write_under_a_borrow(&to, span)?;
                self.mark_moved(from);
                self.write_new_value(to, true);
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

    /// A move out of `place` while a live borrow reaches into it — at it, above
    /// it or BELOW it — is a move out of borrowed content.
    ///
    /// The state lookup above only sees the place and its ancestors, so a
    /// borrow of a CHILD was invisible to it, and `mark_moved` then erased the
    /// child's `Borrowed` entry while the borrow stayed live. Measured on
    /// 6a3d028 (review round 1): `let r = &d.inner.w; let a = d.inner;
    /// d.inner = …; let z = d.inner.w;` compiled and printed 3. 2563001 had the
    /// same blind spot for a whole move (`let r = &d.inner; let e = d;`, #52).
    fn refuse_move_under_a_borrow(&self, place: &Place, span: Span) -> Result<()> {
        if self
            .borrows
            .iter()
            .any(|borrow| borrow.place.may_alias(place))
        {
            return Err(CompileError::CannotMoveOutOfBorrowedContent { span: Some(span) });
        }
        Ok(())
    }

    /// An assignment to `place` while a live borrow overlaps it is refused AT
    /// THE ASSIGNMENT (#53). It is reported as a `ConflictingBorrows` because
    /// that is what it is — a write conflicting with a borrow — and the message
    /// names the assignment. Measured on 2563001: `let r = &d; d = e;
    /// print_int(r.v);` passed the borrow checker.
    fn refuse_write_under_a_borrow(&self, place: &Place, span: Span) -> Result<()> {
        if let Some(borrow) = self.borrows.iter().find(|b| b.place.may_alias(place)) {
            return Err(CompileError::ConflictingBorrows {
                message: format!(
                    "cannot assign to `{}` because `{}` is borrowed",
                    place, borrow.place
                ),
                span: Some(span),
            });
        }
        Ok(())
    }

    /// Record that the value at `place` has been moved out — ALL of it.
    ///
    /// Nothing below a moved place keeps a state of its own: an entry left
    /// there is found by `resolve_place` before the `Moved` above it. Measured
    /// before: `peek(&d.inner.w)` left `d.inner.w: Owned` once its borrow
    /// ended, `let a: In = d.inner;` then moved the parent, and
    /// `let z: W = d.inner.w;` COMPILED. No entry below is borrowed: a move
    /// under a live borrow is refused first (`refuse_move_under_a_borrow`).
    fn mark_moved(&mut self, place: Place) {
        self.ownership
            .retain(|key, _| key == &place || !key.is_within(&place));
        self.ownership.insert(place, Ownership::Moved);
    }

    /// Record that `place` has been given a new value by an assignment whose
    /// right-hand side is not a place to move out of (a literal, a call, a Copy
    /// value). Refused while a live borrow overlaps `place`.
    pub fn reinitialize(&mut self, place: Place, span: Span) -> Result<()> {
        self.refuse_write_under_a_borrow(&place, span)?;
        self.write_new_value(place, false);
        Ok(())
    }

    /// The state an assignment leaves behind, once it is known to be allowed.
    ///
    /// The new value is whole, so a move out of the OLD one says nothing about
    /// it: every entry below `place` goes and `place` is `Owned`. Measured
    /// before: `let a: In = d.inner; d = e; let b: In = d.inner;` was refused,
    /// the stale `d.inner: Moved` found before the re-initialised `d`.
    ///
    /// Two exceptions leave the state as it was:
    ///   * a STRICT ancestor is MOVED — writing `d.inner.w` after `d.inner` was
    ///     moved out does not make `d.inner` whole again. Measured before: with
    ///     a place on the right it COMPILED and read `w` through the moved parent.
    ///   * the assignment is inside a LOOP BODY. The body is walked once (#51)
    ///     and nothing records the state at a `break` or a `continue`, or the
    ///     state when the body runs zero times, so a revival here would count on
    ///     paths that never ran it. Measured on 6a3d028: `let a = d.inner;
    ///     while i < 0 { d = mk(2); } let b = d.inner;` COMPILED, as did the
    ///     `loop`/`break` and `continue` spellings. Inside a loop an assignment
    ///     therefore does exactly what it did on 2563001: a place on the right
    ///     marks the target `Owned` — except a constant-index element, whose
    ///     target there was the `[dynamic]` key no read finds (measured on
    ///     33b519f: `while c { xs[0] = e; }` revived a moved `xs[0]`) — and
    ///     anything else records nothing.
    fn write_new_value(&mut self, place: Place, from_place: bool) {
        if let Some(governing) = self.resolve_place(&place) {
            if governing != &place && self.ownership.get(governing) == Some(&Ownership::Moved) {
                return;
            }
        }
        if self.loop_depth > 0 {
            if from_place && !matches!(&place, Place::Index { index, .. } if index != "dynamic") {
                self.ownership.insert(place, Ownership::Owned);
            }
            return;
        }
        self.ownership
            .retain(|key, _| key == &place || !key.is_within(&place));
        self.ownership.insert(place, Ownership::Owned);
    }

    /// Enter a loop body (`while`, `loop`, `for`). See `write_new_value`.
    pub fn enter_loop(&mut self) {
        self.loop_depth += 1;
    }

    /// Leave a loop body.
    pub fn exit_loop(&mut self) {
        self.loop_depth -= 1;
    }

    /// The flow state at this point, for a branch to start each path from.
    pub fn snapshot(&self) -> FlowState {
        FlowState {
            ownership: self.ownership.clone(),
            borrows: self.borrows.clone(),
        }
    }

    /// Put the flow state back to `state` — the start of the next path.
    pub fn restore(&mut self, state: &FlowState) {
        self.ownership = state.ownership.clone();
        self.borrows = state.borrows.clone();
    }

    /// Merge the end states of the paths that reach the same point.
    ///
    /// A place is `Moved` if it is moved on ANY path, and only otherwise what
    /// the paths recorded for it — `Owned` only when it is owned on ALL of them.
    /// Borrows are the union of what survives each path. Each path's state is
    /// read through `resolve_in`, so a path that moved `d.inner` and a path that
    /// re-assigned the whole `d` (which erased the `d.inner` entry) disagree
    /// about `d.inner` and the join keeps it moved.
    ///
    /// The checker used to walk the paths of a branch one after the other, so
    /// the LAST write won: measured on 6a3d028 (review round 1), `let a =
    /// d.inner; if c { d = D { … }; } let b = d.inner;` COMPILED; and the second
    /// arm saw the first arm's moves, so `if c { let a = d.inner; } else { let
    /// b = d.inner; }` was refused on 2563001 and 6a3d028 alike.
    pub fn join(&mut self, paths: Vec<FlowState>) {
        if paths.is_empty() {
            return;
        }
        let keys: HashSet<&Place> = paths.iter().flat_map(|p| p.ownership.keys()).collect();
        let mut ownership = HashMap::new();
        for key in keys {
            let moved = paths.iter().any(|path| {
                resolve_in(&path.ownership, key).and_then(|k| path.ownership.get(k))
                    == Some(&Ownership::Moved)
            });
            let state = if moved {
                Ownership::Moved
            } else {
                paths
                    .iter()
                    .filter_map(|path| path.ownership.get(key))
                    .find(|state| !matches!(state, Ownership::Owned))
                    .cloned()
                    .unwrap_or(Ownership::Owned)
            };
            ownership.insert(key.clone(), state);
        }
        let mut borrows: Vec<Borrow> = Vec::new();
        for path in &paths {
            for borrow in &path.borrows {
                let seen = borrows.iter().any(|b| {
                    b.place == borrow.place
                        && b.kind == borrow.kind
                        && b.lifetime == borrow.lifetime
                        && b.scope == borrow.scope
                });
                if !seen {
                    borrows.push(borrow.clone());
                }
            }
        }
        self.ownership = ownership;
        self.borrows = borrows;
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

/// `resolve_place` over any map — a `FlowState`'s as well as the live one.
fn resolve_in<'p>(ownership: &HashMap<Place, Ownership>, place: &'p Place) -> Option<&'p Place> {
    let mut current = place;
    loop {
        if ownership.contains_key(current) {
            return Some(current);
        }
        match current {
            Place::Field { base, .. } | Place::Index { base, .. } => current = base.as_ref(),
            _ => return None,
        }
    }
}

impl Place {
    /// Whether `self` is `root` or a projection reached from it (`root.a`,
    /// `root[0].b`, …) — the subtree a binding owns.
    pub fn is_within(&self, root: &Place) -> bool {
        let mut current = self;
        loop {
            if current == root {
                return true;
            }
            match current {
                Place::Field { base, .. } | Place::Index { base, .. } => current = base.as_ref(),
                _ => return false,
            }
        }
    }

    /// Whether `self` and `other` can name overlapping memory: one is the other
    /// or a projection of it. An index written as an expression is recorded as
    /// `[dynamic]` and may be ANY element, so it overlaps every index at the
    /// same position.
    pub fn may_alias(&self, other: &Place) -> bool {
        fn chain(place: &Place) -> Vec<&Place> {
            let mut nodes = vec![place];
            let mut current = place;
            while let Place::Field { base, .. } | Place::Index { base, .. } = current {
                current = base.as_ref();
                nodes.push(current);
            }
            nodes.reverse();
            nodes
        }
        let (a, b) = (chain(self), chain(other));
        a[0] == b[0]
            && a.iter().zip(b.iter()).skip(1).all(|pair| match pair {
                (Place::Field { field: x, .. }, Place::Field { field: y, .. }) => x == y,
                (Place::Index { index: x, .. }, Place::Index { index: y, .. }) => {
                    x == y || x == "dynamic" || y == "dynamic"
                }
                _ => false,
            })
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

    /// `reinitialize` driven directly, below the checker: re-assigning the
    /// whole value makes a moved field usable again, and is refused while a
    /// field is borrowed — the borrow ends (here, by its lifetime) before the
    /// write is allowed. The source-level shapes are in
    /// tests/m2_projection_move_scope.rs.
    #[test]
    fn test_reinitialize_revives_moved_fields_and_waits_for_borrows() {
        let mut ctx = OwnershipContext::new();
        ctx.enter_scope();
        let d = Place::Local("d".to_string());
        let field = |name: &str| Place::Field {
            base: Box::new(d.clone()),
            field: name.to_string(),
        };
        ctx.declare(&d);
        ctx.init_owned(d.clone());

        ctx.move_out_of(field("inner"), Span::dummy()).unwrap();
        let lifetime = ctx.new_lifetime();
        ctx.borrow(
            field("other"),
            RefKind::Shared,
            lifetime.clone(),
            Span::dummy(),
        )
        .unwrap();

        assert!(matches!(
            ctx.reinitialize(d.clone(), Span::dummy()),
            Err(CompileError::ConflictingBorrows { .. })
        ));
        ctx.end_borrows(&lifetime);
        ctx.reinitialize(d.clone(), Span::dummy())
            .expect("nothing borrows `d` any more");

        ctx.move_out_of(field("inner"), Span::dummy())
            .expect("a field of a re-initialised value must be usable again");
        ctx.exit_scope();
        assert_eq!(ctx.get_ownership(&field("other")), None);
    }
}
