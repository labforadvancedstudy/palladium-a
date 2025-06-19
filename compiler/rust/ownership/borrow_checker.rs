// Borrow checker for Palladium
// "Ensuring memory safety through static analysis"

use crate::ast::{AssignTarget, Expr, Function, Item, Pattern, Program, Stmt, Type};
use crate::errors::{CompileError, Result};
use crate::ownership::{expr_to_place, Lifetime, OwnershipContext, Place, RefKind};
use std::collections::HashMap;

/// The borrow checker analyzes the program to ensure memory safety
pub struct BorrowChecker {
    /// Current ownership context
    context: OwnershipContext,
    /// Function signatures for ownership analysis
    functions: HashMap<String, FunctionSig>,
    /// Current function being analyzed
    current_function: Option<String>,
    /// Local variable types for Copy checking
    local_types: HashMap<String, Type>,
    /// Track if we're in an unsafe context
    unsafe_depth: usize,
}

/// Function signature for ownership analysis
#[derive(Debug, Clone)]
struct FunctionSig {
    /// Parameter ownership requirements
    params: Vec<ParamOwnership>,
    /// Return value ownership
    returns: ReturnOwnership,
}

#[derive(Debug, Clone)]
enum ParamOwnership {
    /// Parameter takes ownership (moves the value)
    Move,
    /// Parameter borrows immutably
    Borrow(Lifetime),
    /// Parameter borrows mutably
    BorrowMut(Lifetime),
    /// Parameter is Copy (no ownership transfer)
    Copy,
}

#[derive(Debug, Clone)]
enum ReturnOwnership {
    /// Returns owned value
    Owned,
    /// Returns borrowed value with lifetime
    Borrowed(Lifetime),
    /// No return value
    Unit,
    /// Returns copy value (primitives)
    Copy,
}

impl BorrowChecker {
    pub fn new() -> Self {
        let mut functions = HashMap::new();
        
        // Register built-in functions
        // These functions take their arguments by value (for simplicity)
        functions.insert(
            "print".to_string(),
            FunctionSig {
                params: vec![ParamOwnership::Copy], // String is treated as Copy for built-ins
                returns: ReturnOwnership::Unit,
            },
        );
        
        functions.insert(
            "print_int".to_string(),
            FunctionSig {
                params: vec![ParamOwnership::Copy],
                returns: ReturnOwnership::Unit,
            },
        );
        
        // String manipulation functions
        functions.insert(
            "string_concat".to_string(),
            FunctionSig {
                params: vec![ParamOwnership::Borrow(Lifetime::Named("fn".to_string())), 
                           ParamOwnership::Borrow(Lifetime::Named("fn".to_string()))],
                returns: ReturnOwnership::Owned,
            },
        );
        
        functions.insert(
            "string_substring".to_string(),
            FunctionSig {
                params: vec![ParamOwnership::Borrow(Lifetime::Named("fn".to_string())), 
                           ParamOwnership::Copy, 
                           ParamOwnership::Copy],
                returns: ReturnOwnership::Owned,
            },
        );
        
        functions.insert(
            "int_to_string".to_string(),
            FunctionSig {
                params: vec![ParamOwnership::Copy],
                returns: ReturnOwnership::Owned,
            },
        );
        
        functions.insert(
            "string_to_int".to_string(),
            FunctionSig {
                params: vec![ParamOwnership::Borrow(Lifetime::Named("fn".to_string()))],
                returns: ReturnOwnership::Copy,
            },
        );
        
        Self {
            context: OwnershipContext::new(),
            functions,
            current_function: None,
            local_types: HashMap::new(),
            unsafe_depth: 0,
        }
    }

    /// Check if we're currently in an unsafe context
    fn in_unsafe_context(&self) -> bool {
        self.unsafe_depth > 0
    }

    /// Check a program for ownership violations
    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        // First pass: collect function signatures
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.collect_function_sig(func);
                }
                Item::Impl(impl_block) => {
                    // Collect method signatures from impl blocks
                    for method in &impl_block.methods {
                        // Create qualified method name
                        let qualified_name = format!("{}::{}", impl_block.for_type, method.name);
                        self.collect_function_sig_with_name(method, &qualified_name);
                    }
                }
                _ => {}
            }
        }

        // Second pass: check function bodies
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.check_function(func)?;
                }
                Item::Impl(impl_block) => {
                    // Check method bodies from impl blocks
                    for method in &impl_block.methods {
                        self.check_function(method)?;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Collect function signature for ownership analysis
    fn collect_function_sig(&mut self, func: &Function) {
        self.collect_function_sig_with_name(func, &func.name);
    }
    
    /// Collect function signature with a custom name
    fn collect_function_sig_with_name(&mut self, func: &Function, name: &str) {
        let mut params = Vec::new();
        
        for param in &func.params {
            let ownership = match &param.ty {
                Type::String | Type::Array(_, _) | Type::Custom(_) => {
                    // Non-copy types
                    if param.mutable {
                        ParamOwnership::BorrowMut(Lifetime::Named("fn".to_string()))
                    } else {
                        ParamOwnership::Move
                    }
                }
                Type::Reference { mutable, .. } => {
                    if *mutable {
                        ParamOwnership::BorrowMut(Lifetime::Named("fn".to_string()))
                    } else {
                        ParamOwnership::Borrow(Lifetime::Named("fn".to_string()))
                    }
                }
                _ => ParamOwnership::Copy, // Primitives are Copy
            };
            params.push(ownership);
        }

        let returns = match &func.return_type {
            Some(Type::Reference { .. }) => ReturnOwnership::Borrowed(Lifetime::Named("fn".to_string())),
            Some(_) => ReturnOwnership::Owned,
            None => ReturnOwnership::Unit,
        };

        self.functions.insert(
            name.to_string(),
            FunctionSig { params, returns },
        );
    }

    /// Check a function for ownership violations
    fn check_function(&mut self, func: &Function) -> Result<()> {
        self.current_function = Some(func.name.clone());
        self.context.enter_scope();
        self.local_types.clear();

        // Initialize parameters and their types
        for param in &func.params {
            let place = Place::Local(param.name.clone());
            self.context.init_owned(place);
            self.local_types.insert(param.name.clone(), param.ty.clone());
        }

        // Check function body
        for stmt in &func.body {
            self.check_stmt(stmt)?;
        }

        self.context.exit_scope();
        self.current_function = None;
        Ok(())
    }

    /// Check a statement for ownership violations
    fn check_stmt(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Let { name, value, ty, .. } => {
                // Check the value expression
                self.check_expr(value)?;
                
                // Store the type if provided
                if let Some(ty) = ty {
                    self.local_types.insert(name.clone(), ty.clone());
                } else {
                    // Infer type from expression
                    let inferred_type = self.expr_type(value);
                    self.local_types.insert(name.clone(), inferred_type);
                }
                
                // Initialize the new variable
                let place = Place::Local(name.clone());
                
                // Check if value is moved or copied
                if let Some(from_place) = expr_to_place(value) {
                    if self.is_expr_copy(value) {
                        // Copy types don't move
                        self.context.init_owned(place);
                    } else {
                        // Move ownership
                        self.context.move_value(from_place, place, value.span())?;
                    }
                } else {
                    // Temporary value (like string literal), take ownership
                    self.context.init_owned(place);
                }
            }
            
            Stmt::Assign { target, value, span } => {
                // Check the value expression
                self.check_expr(value)?;
                
                // Get target place
                let target_place = match target {
                    AssignTarget::Ident(name) => Place::Local(name.clone()),
                    AssignTarget::Index { array, index } => {
                        self.check_expr(array)?;
                        self.check_expr(index)?;
                        if let Some(base) = expr_to_place(array) {
                            Place::Index {
                                base: Box::new(base),
                                index: "dynamic".to_string(),
                            }
                        } else {
                            return Err(CompileError::BorrowChecker {
                                message: "Cannot assign to temporary value".to_string(),
                                span: Some(*span),
                            });
                        }
                    }
                    AssignTarget::FieldAccess { object, field } => {
                        self.check_expr(object)?;
                        if let Some(base) = expr_to_place(object) {
                            Place::Field {
                                base: Box::new(base),
                                field: field.clone(),
                            }
                        } else {
                            return Err(CompileError::BorrowChecker {
                                message: "Cannot assign to temporary value".to_string(),
                                span: Some(*span),
                            });
                        }
                    }
                    AssignTarget::Deref { expr } => {
                        self.check_expr(expr)?;
                        // For dereference assignment, we need the place that the reference points to
                        if let Some(place) = expr_to_place(expr) {
                            // The dereferenced place is what we're assigning to
                            place
                        } else {
                            return Err(CompileError::BorrowChecker {
                                message: "Cannot dereference temporary value".to_string(),
                                span: Some(*span),
                            });
                        }
                    }
                };
                
                // Check if assignment is allowed
                if let Some(from_place) = expr_to_place(value) {
                    if !self.is_expr_copy(value) {
                        // Move ownership
                        self.context.move_value(from_place, target_place, *span)?;
                    }
                }
            }
            
            Stmt::Expr(expr) => {
                self.check_expr(expr)?;
            }
            
            Stmt::Return(Some(expr)) => {
                self.check_expr(expr)?;
                // TODO: Check return value ownership matches function signature
            }
            
            Stmt::Return(None) => {}
            
            Stmt::If { condition, then_branch, else_branch, .. } => {
                self.check_expr(condition)?;
                
                self.context.enter_scope();
                for stmt in then_branch {
                    self.check_stmt(stmt)?;
                }
                self.context.exit_scope();
                
                if let Some(else_stmts) = else_branch {
                    self.context.enter_scope();
                    for stmt in else_stmts {
                        self.check_stmt(stmt)?;
                    }
                    self.context.exit_scope();
                }
            }
            
            Stmt::While { condition, body, .. } => {
                self.check_expr(condition)?;
                
                self.context.enter_scope();
                for stmt in body {
                    self.check_stmt(stmt)?;
                }
                self.context.exit_scope();
            }
            
            Stmt::For { var, iter, body, .. } => {
                self.check_expr(iter)?;
                
                self.context.enter_scope();
                // Initialize loop variable
                let place = Place::Local(var.clone());
                self.context.init_owned(place);
                
                for stmt in body {
                    self.check_stmt(stmt)?;
                }
                self.context.exit_scope();
            }
            
            Stmt::Match { expr, arms, .. } => {
                self.check_expr(expr)?;
                
                for arm in arms {
                    self.context.enter_scope();
                    
                    // Bind pattern variables
                    self.bind_pattern(&arm.pattern)?;
                    
                    for stmt in &arm.body {
                        self.check_stmt(stmt)?;
                    }
                    
                    self.context.exit_scope();
                }
            }
            
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
            
            Stmt::Unsafe { body, .. } => {
                // In unsafe blocks, we still perform ownership checks
                // but allow certain operations that would normally be forbidden
                self.unsafe_depth += 1;
                self.context.enter_scope();
                for stmt in body {
                    self.check_stmt(stmt)?;
                }
                self.context.exit_scope();
                self.unsafe_depth -= 1;
            }
        }
        
        Ok(())
    }

    /// Check an expression for ownership violations
    fn check_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Ident(name) => {
                // Check if this is a function name or a variable
                if self.functions.contains_key(name) {
                    // It's a function - no ownership check needed
                    return Ok(());
                }
                
                let place = Place::Local(name.clone());
                
                // Check if the value is initialized and not moved
                match self.context.get_ownership(&place) {
                    Some(crate::ownership::Ownership::Owned) => {
                        // Value is accessible
                    }
                    Some(crate::ownership::Ownership::Borrowed { .. }) => {
                        // Value is borrowed but still accessible
                    }
                    Some(crate::ownership::Ownership::BorrowedMut { .. }) => {
                        // Value is mutably borrowed but still accessible
                    }
                    Some(crate::ownership::Ownership::Moved) => {
                        return Err(CompileError::UseOfMovedValue {
                            name: name.clone(),
                            span: Some(expr.span()),
                        });
                    }
                    None => {
                        return Err(CompileError::UseOfUninitializedValue {
                            name: name.clone(),
                            span: Some(expr.span()),
                        });
                    }
                }
            }
            
            Expr::Call { func, args, span } => {
                // Check function expression
                self.check_expr(func)?;
                
                // Check arguments and handle ownership transfer
                if let Expr::Ident(func_name) = func.as_ref() {
                    // Clone the function signature to avoid borrow conflicts
                    let sig_opt = self.functions.get(func_name).cloned();
                    
                    if let Some(sig) = sig_opt {
                        for (i, arg) in args.iter().enumerate() {
                            self.check_expr(arg)?;
                            
                            // Handle ownership based on parameter type
                            if let Some(param_ownership) = sig.params.get(i) {
                                if let Some(place) = expr_to_place(arg) {
                                    match param_ownership {
                                        ParamOwnership::Move => {
                                            // Move the argument
                                            let temp = self.context.new_temp();
                                            self.context.move_value(place, temp, *span)?;
                                        }
                                        ParamOwnership::Borrow(lifetime) => {
                                            // Borrow immutably
                                            self.context.borrow(place, RefKind::Shared, lifetime.clone(), *span)?;
                                        }
                                        ParamOwnership::BorrowMut(lifetime) => {
                                            // Borrow mutably
                                            self.context.borrow(place, RefKind::Mutable, lifetime.clone(), *span)?;
                                        }
                                        ParamOwnership::Copy => {
                                            // No ownership transfer
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        // Function not found, just check arguments
                        for arg in args {
                            self.check_expr(arg)?;
                        }
                    }
                } else {
                    // Check all arguments without special ownership handling
                    for arg in args {
                        self.check_expr(arg)?;
                    }
                }
            }
            
            Expr::Binary { left, right, .. } => {
                self.check_expr(left)?;
                self.check_expr(right)?;
            }
            
            Expr::Unary { operand, .. } => {
                self.check_expr(operand)?;
            }
            
            Expr::ArrayLiteral { elements, .. } => {
                for elem in elements {
                    self.check_expr(elem)?;
                }
            }
            
            Expr::ArrayRepeat { value, count, .. } => {
                self.check_expr(value)?;
                self.check_expr(count)?;
            }
            
            Expr::Index { array, index, .. } => {
                self.check_expr(array)?;
                self.check_expr(index)?;
            }
            
            Expr::StructLiteral { fields, .. } => {
                for (_, expr) in fields {
                    self.check_expr(expr)?;
                }
            }
            
            Expr::FieldAccess { object, .. } => {
                self.check_expr(object)?;
            }
            
            Expr::EnumConstructor { data, .. } => {
                match data {
                    Some(crate::ast::EnumConstructorData::Tuple(exprs)) => {
                        for expr in exprs {
                            self.check_expr(expr)?;
                        }
                    }
                    Some(crate::ast::EnumConstructorData::Struct(fields)) => {
                        for (_, expr) in fields {
                            self.check_expr(expr)?;
                        }
                    }
                    None => {}
                }
            }
            
            Expr::Range { start, end, .. } => {
                self.check_expr(start)?;
                self.check_expr(end)?;
            }
            
            Expr::Reference { mutable, expr, span } => {
                // Taking a reference to an expression
                self.check_expr(expr)?;
                
                // If we can get a place for the expression, create a borrow
                if let Some(place) = expr_to_place(expr) {
                    let lifetime = self.context.new_lifetime();
                    let kind = if *mutable {
                        RefKind::Mutable
                    } else {
                        RefKind::Shared
                    };
                    self.context.borrow(place, kind, lifetime, *span)?;
                } else {
                    // Can't take reference to temporary
                    return Err(CompileError::BorrowChecker {
                        message: "Cannot take reference to temporary value".to_string(),
                        span: Some(*span),
                    });
                }
            }
            
            Expr::Deref { expr, .. } => {
                // Dereferencing an expression
                self.check_expr(expr)?;
                // TODO: Check that the expression is actually a reference type
            }
            
            Expr::Question { expr, .. } => {
                // Question operator checks the inner expression
                self.check_expr(expr)?;
                // TODO: Handle ownership implications of early return
            }
            
            // Literals don't need ownership checking
            Expr::String(_) | Expr::Integer(_) | Expr::Bool(_) => {}
            Expr::MacroInvocation { .. } => {
                // Macros should have been expanded before borrow checking
                return Err(CompileError::Generic(
                    "Unexpected macro invocation in borrow checking - macros should be expanded before this phase".to_string()
                ));
            }
            Expr::Await { expr, .. } => {
                self.check_expr(expr)?;
            }
        }
        
        Ok(())
    }

    /// Bind variables in a pattern
    fn bind_pattern(&mut self, pattern: &Pattern) -> Result<()> {
        match pattern {
            Pattern::Ident(name) => {
                let place = Place::Local(name.clone());
                self.context.init_owned(place);
            }
            Pattern::EnumPattern { data, .. } => {
                if let Some(pattern_data) = data {
                    match pattern_data {
                        crate::ast::PatternData::Tuple(patterns) => {
                            for pattern in patterns {
                                self.bind_pattern(pattern)?;
                            }
                        }
                        crate::ast::PatternData::Struct(fields) => {
                            for (_, pattern) in fields {
                                self.bind_pattern(pattern)?;
                            }
                        }
                    }
                }
            }
            Pattern::Wildcard => {}
        }
        Ok(())
    }

    /// Check if a type is Copy (doesn't move on assignment)
    fn is_copy_type(&self, ty: &Type) -> bool {
        match ty {
            Type::I32 | Type::I64 | Type::U32 | Type::U64 | Type::Bool => true,
            Type::String | Type::Array(_, _) | Type::Custom(_) => false,
            Type::Reference { .. } => true, // References are Copy
            Type::Unit => true,
            Type::TypeParam(_) => false, // Conservative: assume not Copy
            Type::Generic { .. } => false, // Conservative: assume not Copy
            Type::Future { .. } => false, // Futures are not Copy
        }
    }
    
    /// Check if an expression type is Copy
    fn is_expr_copy(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Integer(_) | Expr::Bool(_) => true,
            Expr::String(_) => false, // Strings are not Copy
            Expr::Ident(name) => {
                // Look up the type of the identifier from local_types
                if let Some(ty) = self.local_types.get(name) {
                    self.is_copy_type(ty)
                } else {
                    // If we can't find the type, conservatively assume non-Copy
                    false
                }
            }
            _ => false, // Conservative default
        }
    }

    /// Get the type of an expression (simplified version)
    fn expr_type(&self, expr: &Expr) -> Type {
        match expr {
            Expr::Integer(_) => Type::I64,
            Expr::String(_) => Type::String,
            Expr::Bool(_) => Type::Bool,
            Expr::Ident(_) => Type::I64, // TODO: Proper type lookup
            _ => Type::I64, // Default for now
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn dummy_span() -> crate::errors::Span {
        crate::errors::Span::new(0, 0, 0, 0)
    }

    #[test]
    fn test_basic_move() {
        let mut checker = BorrowChecker::new();
        
        let func = Function {
            visibility: Visibility::Private,
            name: "test".to_string(),
            lifetime_params: vec![],
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![
                Stmt::Let {
                    name: "x".to_string(),
                    ty: Some(Type::String),
                    value: Expr::String("hello".to_string()),
                    mutable: false,
                    span: dummy_span(),
                },
                Stmt::Let {
                    name: "y".to_string(),
                    ty: Some(Type::String),
                    value: Expr::Ident("x".to_string()),
                    mutable: false,
                    span: dummy_span(),
                },
                // This should fail - x was moved
                Stmt::Expr(Expr::Ident("x".to_string())),
            ],
            span: dummy_span(),
        };
        
        let result = checker.check_function(&func);
        assert!(result.is_err());
    }

    #[test]
    fn test_copy_type_no_move() {
        let mut checker = BorrowChecker::new();
        
        let func = Function {
            visibility: Visibility::Private,
            name: "test".to_string(),
            lifetime_params: vec![],
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![
                Stmt::Let {
                    name: "x".to_string(),
                    ty: Some(Type::I32),
                    value: Expr::Integer(42),
                    mutable: false,
                    span: dummy_span(),
                },
                Stmt::Let {
                    name: "y".to_string(),
                    ty: Some(Type::I32),
                    value: Expr::Ident("x".to_string()),
                    mutable: false,
                    span: dummy_span(),
                },
                // This should succeed - i32 is Copy
                Stmt::Expr(Expr::Ident("x".to_string())),
            ],
            span: dummy_span(),
        };
        
        let result = checker.check_function(&func);
        if let Err(ref e) = result {
            eprintln!("Error in test_copy_type_no_move: {:?}", e);
        }
        assert!(result.is_ok());
    }
}