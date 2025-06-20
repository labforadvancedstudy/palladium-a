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
    }

    /// Exit a scope, invalidating borrows
    pub fn exit_scope(&mut self) {
        let scope_lifetime = Lifetime::Scope(self.current_scope);

        // Remove borrows that end with this scope
        self.borrows
            .retain(|borrow| borrow.lifetime != scope_lifetime);

        // Clean up moved values in this scope
        // TODO: Implement proper drop semantics

        self.current_scope -= 1;
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

    /// Move a value from one place to another
    pub fn move_value(&mut self, from: Place, to: Place, span: Span) -> Result<()> {
        // Check if the source can be moved
        match self.ownership.get(&from) {
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
        match self.ownership.get(&place) {
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
}
