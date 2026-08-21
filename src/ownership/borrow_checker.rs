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
    /// Field types of every struct in the program, so that the type of a
    /// projection like `v.data[0]` can be resolved when deciding Copy vs move.
    struct_fields: HashMap<String, Vec<(String, Type)>>,
    /// Lifetime of the call expression whose arguments are being checked, if any.
    /// Every borrow created while evaluating those arguments — including the
    /// temporary `&x` / `&mut x` references written in argument position — gets
    /// this lifetime and is released when the call completes.
    call_lifetime: Option<Lifetime>,
}

/// Function signature for ownership analysis
#[derive(Debug, Clone)]
struct FunctionSig {
    /// Parameter ownership requirements
    params: Vec<ParamOwnership>,
    /// Return value ownership
    #[allow(dead_code)]
    returns: ReturnOwnership,
}

/// How a callee takes each parameter.
///
/// The borrowing modes carry no lifetime: the caller-side borrow always lasts
/// exactly for the call expression, so `check_call_args` supplies that lifetime.
#[derive(Debug, Clone)]
enum ParamOwnership {
    /// Parameter takes ownership (moves the value)
    Move,
    /// Parameter borrows immutably
    Borrow,
    /// Parameter borrows mutably
    BorrowMut,
    /// Parameter is Copy (no ownership transfer)
    Copy,
}

#[derive(Debug, Clone)]
enum ReturnOwnership {
    /// Returns owned value
    Owned,
    /// Returns borrowed value with lifetime
    #[allow(dead_code)]
    Borrowed(Lifetime),
    /// No return value
    Unit,
    /// Returns copy value (primitives)
    Copy,
}

impl FunctionSig {
    /// Build the ownership signature of a built-in from the canonical table.
    fn from_builtin(b: &crate::builtins::Builtin) -> Self {
        use crate::builtins::{ParamMode, ReturnMode};

        let params = b
            .params
            .iter()
            .map(|param| match param.mode {
                ParamMode::Copy => ParamOwnership::Copy,
                ParamMode::Borrow => ParamOwnership::Borrow,
                ParamMode::Move => ParamOwnership::Move,
            })
            .collect();

        let returns = match b.ret_mode {
            ReturnMode::Owned => ReturnOwnership::Owned,
            // Storage the built-in never allocated and that outlives the program —
            // `arg_at` handing back a pointer into `argv`. Borrowed for 'static is
            // the honest signature; Owned would tell this pass that the caller is
            // free to release memory belonging to the process.
            ReturnMode::BorrowedStatic => ReturnOwnership::Borrowed(Lifetime::Static),
            ReturnMode::Copy => ReturnOwnership::Copy,
            ReturnMode::Unit => ReturnOwnership::Unit,
        };

        FunctionSig { params, returns }
    }
}

impl Default for BorrowChecker {
    fn default() -> Self {
        // Register built-in functions from the single source of truth
        // (src/builtins.rs). Deriving them here — instead of keeping a
        // hand-written copy — is what keeps this pass in sync with the type
        // checker; a built-in known to only one pass used to type-check and
        // then die here with "Use of uninitialized value".
        let functions = crate::builtins::BUILTINS
            .iter()
            .map(|b| (b.name.to_string(), FunctionSig::from_builtin(b)))
            .collect();

        Self {
            context: OwnershipContext::new(),
            functions,
            current_function: None,
            local_types: HashMap::new(),
            unsafe_depth: 0,
            struct_fields: HashMap::new(),
            call_lifetime: None,
        }
    }
}

impl BorrowChecker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Names of every function signature this pass knows.
    ///
    /// On a freshly constructed checker this is exactly the built-in set, which is
    /// what the drift test in `crate::builtins` asserts against. Returning *all*
    /// names (not just the ones that happen to be in the canonical table) is
    /// deliberate: a hand-added registration here must show up as a diff.
    #[allow(dead_code)] // used by the drift tests in crate::builtins
    pub(crate) fn registered_function_names(&self) -> std::collections::BTreeSet<String> {
        self.functions.keys().cloned().collect()
    }

    /// Check if we're currently in an unsafe context
    #[allow(dead_code)]
    fn in_unsafe_context(&self) -> bool {
        self.unsafe_depth > 0
    }

    /// Check a program for ownership violations
    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        // First pass: collect function signatures and struct layouts
        for item in &program.items {
            match item {
                Item::Struct(struct_def) => {
                    self.struct_fields
                        .insert(struct_def.name.clone(), struct_def.fields.clone());
                }
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
                Type::Array(_, _) => {
                    // An array parameter is a *reference*, not a move. The C
                    // backend emits `T name[N]`, which decays to a pointer, so
                    // the callee writes into the caller's storage and the caller
                    // keeps using the array afterwards — see the parameter
                    // emission in src/codegen/mod.rs ("In C, array parameters
                    // are passed as pointers"). Codegen is the ground truth
                    // here, so this pass models it as a borrow either way and
                    // only the mutability of the binding picks the kind.
                    if param.mutable {
                        ParamOwnership::BorrowMut
                    } else {
                        ParamOwnership::Borrow
                    }
                }
                // `String` is Copy (see `is_copy_type`): it lowers to
                // `const char*` and nothing frees it per value, so passing one
                // neither transfers nor invalidates anything.
                Type::String => ParamOwnership::Copy,
                Type::Custom(_) => {
                    // Non-copy types
                    if param.mutable {
                        ParamOwnership::BorrowMut
                    } else {
                        ParamOwnership::Move
                    }
                }
                Type::Reference { mutable, .. } => {
                    if *mutable {
                        ParamOwnership::BorrowMut
                    } else {
                        ParamOwnership::Borrow
                    }
                }
                _ => ParamOwnership::Copy, // Primitives are Copy
            };
            params.push(ownership);
        }

        let returns = match &func.return_type {
            Some(Type::Reference { .. }) => {
                ReturnOwnership::Borrowed(Lifetime::Named("fn".to_string()))
            }
            Some(_) => ReturnOwnership::Owned,
            None => ReturnOwnership::Unit,
        };

        self.functions
            .insert(name.to_string(), FunctionSig { params, returns });
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
            self.local_types
                .insert(param.name.clone(), param.ty.clone());
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
            Stmt::Let {
                name, value, ty, ..
            } => {
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

            Stmt::Assign {
                target,
                value,
                span,
            } => {
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

            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
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

            Stmt::While {
                condition, body, ..
            } => {
                self.check_expr(condition)?;

                self.context.enter_scope();
                for stmt in body {
                    self.check_stmt(stmt)?;
                }
                self.context.exit_scope();
            }

            Stmt::For {
                var, iter, body, ..
            } => {
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

                // A borrow taken to pass an argument lasts exactly as long as
                // the call expression. Give this call its own lifetime, tag
                // every borrow created while checking its arguments with it,
                // and end them all once the call is done — otherwise the value
                // stays borrowed forever and the next use of it is rejected.
                let call_lifetime = self.context.new_lifetime();
                let outer_lifetime = self.call_lifetime.replace(call_lifetime.clone());

                let result = self.check_call_args(func, args, &call_lifetime, *span);

                self.call_lifetime = outer_lifetime;
                self.context.end_borrows(&call_lifetime);
                result?;
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

            Expr::EnumConstructor { data, .. } => match data {
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
            },

            Expr::Range { start, end, .. } => {
                self.check_expr(start)?;
                self.check_expr(end)?;
            }

            Expr::Reference {
                mutable,
                expr,
                span,
            } => {
                // Taking a reference to an expression
                self.check_expr(expr)?;

                // If we can get a place for the expression, create a borrow
                if let Some(place) = expr_to_place(expr) {
                    // In argument position (`f(&mut v)`) the reference is a
                    // temporary that dies with the call, so it joins that
                    // call's lifetime and is released with it. Anywhere else
                    // (`let r = &mut v;`) the reference outlives the expression
                    // and keeps the conservative never-released lifetime.
                    let lifetime = match &self.call_lifetime {
                        Some(call_lifetime) => call_lifetime.clone(),
                        None => self.context.new_lifetime(),
                    };
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

    /// Check a call's arguments and apply the callee's ownership requirements.
    ///
    /// `call_lifetime` is the lifetime of the enclosing call expression; every
    /// borrow taken here uses it so that `end_borrows` can release the whole set
    /// when the call returns. The lifetime recorded in the signature is only the
    /// *mode* marker — the caller-side borrow lives for the call, not forever.
    fn check_call_args(
        &mut self,
        func: &Expr,
        args: &[Expr],
        call_lifetime: &Lifetime,
        span: crate::errors::Span,
    ) -> Result<()> {
        // Only a direct call by name has a signature to consult.
        let sig_opt = match func {
            Expr::Ident(func_name) => self.functions.get(func_name).cloned(),
            _ => None,
        };

        let Some(sig) = sig_opt else {
            // Unknown callee: still check the arguments themselves.
            for arg in args {
                self.check_expr(arg)?;
            }
            return Ok(());
        };

        for (i, arg) in args.iter().enumerate() {
            self.check_expr(arg)?;

            // Handle ownership based on parameter type
            let Some(param_ownership) = sig.params.get(i) else {
                continue;
            };
            let Some(place) = expr_to_place(arg) else {
                continue;
            };

            match param_ownership {
                ParamOwnership::Move => {
                    // Move the argument
                    let temp = self.context.new_temp();
                    self.context.move_value(place, temp, span)?;
                }
                ParamOwnership::Borrow => {
                    // Borrow immutably for the duration of the call
                    self.context
                        .borrow(place, RefKind::Shared, call_lifetime.clone(), span)?;
                }
                ParamOwnership::BorrowMut => {
                    // Borrow mutably for the duration of the call
                    self.context
                        .borrow(place, RefKind::Mutable, call_lifetime.clone(), span)?;
                }
                ParamOwnership::Copy => {
                    // No ownership transfer
                }
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
    #[allow(clippy::only_used_in_recursion)]
    fn is_copy_type(&self, ty: &Type) -> bool {
        match ty {
            Type::I32 | Type::I64 | Type::U32 | Type::U64 | Type::Bool => true,
            // `String` is Copy. It lowers to `const char*`, there is no drop
            // glue and no per-value free anywhere in the language (strings live
            // in a static arena released once by `__pd_cleanup_strings` via
            // `atexit`), so a copy duplicates nothing and invalidates nothing.
            //
            // Treating it as a move was also *unsatisfiable* rather than merely
            // strict: there is no `clone`, and `&T` is erased by the type
            // checker (no reference type exists there), so no syntax could read
            // a String twice out of an aggregate — `let x = s.t[0];` moved out
            // of the slot and the slot could never be read again.
            Type::String => true,
            Type::Array(_, _) | Type::Custom(_) => false,
            Type::Reference { .. } => true, // References are Copy
            Type::Unit => true,
            Type::TypeParam(_) => false, // Conservative: assume not Copy
            Type::Generic { .. } => false, // Conservative: assume not Copy
            Type::Future { .. } => false, // Futures are not Copy
            Type::Tuple(types) => {
                // Tuple is Copy if all its elements are Copy
                types.iter().all(|t| self.is_copy_type(t))
            }
        }
    }

    /// Type of a place expression, following field and index projections.
    ///
    /// This exists so `is_expr_copy` can tell `v.data[0]` (an `i64`, Copy) from
    /// `s.name` (a `String`, not Copy). Without it every projection fell into the
    /// conservative "not Copy" default and `let max = v.data[0];` *moved* the
    /// element, so the second read of the same element was rejected.
    fn place_type(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Ident(name) => self.local_types.get(name).cloned(),
            Expr::FieldAccess { object, field, .. } => {
                let base = self.place_type(object)?;
                let name = match Self::strip_reference(&base) {
                    Type::Custom(name) => name.clone(),
                    _ => return None,
                };
                self.struct_fields
                    .get(&name)?
                    .iter()
                    .find(|(field_name, _)| field_name == field)
                    .map(|(_, ty)| ty.clone())
            }
            Expr::Index { array, .. } => {
                let base = self.place_type(array)?;
                match Self::strip_reference(&base) {
                    Type::Array(elem, _) => Some(elem.as_ref().clone()),
                    _ => None,
                }
            }
            Expr::Deref { expr, .. } => self
                .place_type(expr)
                .map(|ty| Self::strip_reference(&ty).clone()),
            _ => None,
        }
    }

    /// A `&T` / `&mut T` projects to `T` for the purposes above.
    fn strip_reference(ty: &Type) -> &Type {
        match ty {
            Type::Reference { inner, .. } => Self::strip_reference(inner),
            other => other,
        }
    }

    /// Check if an expression type is Copy
    fn is_expr_copy(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Integer(_) | Expr::Bool(_) | Expr::String(_) => true,
            // Idents and projections are Copy exactly when their type is; an
            // unresolvable type stays conservatively non-Copy.
            Expr::Ident(_) | Expr::FieldAccess { .. } | Expr::Index { .. } | Expr::Deref { .. } => {
                self.place_type(expr)
                    .map(|ty| self.is_copy_type(&ty))
                    .unwrap_or(false)
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
            _ => Type::I64,              // Default for now
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

    /// Run the real front end up to and including borrow checking.
    ///
    /// Driving these from source instead of a hand-built AST is deliberate: the
    /// defects below are about how *call expressions* thread borrows, and a
    /// hand-built AST is exactly where a wrong assumption about the shape the
    /// parser produces would hide.
    fn borrow_check(source: &str) -> Result<()> {
        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.collect_tokens()?;
        let mut parser = crate::parser::Parser::new(tokens);
        let mut program = parser.parse()?;
        crate::macros::MacroExpander::new().expand_program(&mut program)?;
        BorrowChecker::new().check_program(&program)
    }

    /// REGRESSION: a borrow taken to pass an argument was never released, so the
    /// *second* function call on the same array was rejected with "Conflicting
    /// borrows". A self-hosting compiler passes its token/AST buffers to one
    /// function after another, so this made the bootstrap program unwritable.
    #[test]
    fn test_array_argument_is_usable_after_the_call() {
        let result = borrow_check(
            r#"
            fn fill(mut a: [i64; 4]) { a[0] = 99; }
            fn peek(mut a: [i64; 4]) -> i64 { return a[0]; }
            fn main() {
                let mut xs = [0; 4];
                fill(xs);
                print_int(peek(xs));
                print_int(xs[0]);
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "call-argument borrow outlived the call: {:?}",
            result.err()
        );
    }

    /// REGRESSION: a non-`mut` array parameter was classified as a Move, so the
    /// caller could never touch the array again. The C backend emits `T a[N]`,
    /// which decays to a pointer — reference semantics — so a move is wrong on
    /// codegen's own terms.
    #[test]
    fn test_non_mut_array_parameter_is_a_borrow_not_a_move() {
        let result = borrow_check(
            r#"
            fn fill(a: [i64; 4]) { a[0] = 7; }
            fn main() {
                let mut xs = [0; 4];
                fill(xs);
                print_int(xs[0]);
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "array parameter still treated as a move: {:?}",
            result.err()
        );
    }

    /// A `&mut x` written in argument position is a temporary that dies with the
    /// call, so repeated `f(&mut v)` is fine. This is the `tests/misc/test_vec_i64.pd`
    /// shape, which failed with "already mutably borrowed" on the second push.
    #[test]
    fn test_reference_arguments_are_released_after_the_call() {
        let result = borrow_check(
            r#"
            struct S { x: i64 }
            fn bump(s: &mut S) { s.x = s.x + 1; }
            fn get(s: &S) -> i64 { return s.x; }
            fn main() {
                let mut v = S { x: 0 };
                bump(&mut v);
                bump(&mut v);
                print_int(get(&v));
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "reference argument borrow outlived the call: {:?}",
            result.err()
        );
    }

    /// REGRESSION: passing a struct *field* to a borrowing built-in died with
    /// "Use of uninitialized value: s.a" even though the field was assigned on
    /// the line before. Only locals are ever registered in the ownership map, so
    /// the projection `s.a` had no entry and was read as uninitialized. This is
    /// the exact program from the bootstrap compiler that first hit it.
    #[test]
    fn test_struct_field_argument_is_not_uninitialized() {
        let result = borrow_check(
            r#"
            struct S { a: String }
            fn go(mut s: S) {
                s.a = "q";
                if string_len(s.a) == 0 { print("e"); }
                print(s.a);
            }
            fn main() { let mut x: S = S { a: "" }; go(x); }
            "#,
        );
        assert!(
            result.is_ok(),
            "struct field argument reported uninitialized: {:?}",
            result.err()
        );
    }

    /// The same defect, with no assignment and no `if` block anywhere: the
    /// trigger is the projection itself, not the assignment or the block scope.
    #[test]
    fn test_unassigned_struct_field_argument_is_not_uninitialized() {
        let result = borrow_check(
            r#"
            struct S { a: String }
            fn go(s: S) -> i64 { return string_len(s.a); }
            fn main() { let x: S = S { a: "q" }; print_int(go(x)); }
            "#,
        );
        assert!(
            result.is_ok(),
            "unassigned struct field argument reported uninitialized: {:?}",
            result.err()
        );
    }

    /// Array elements are projections too, and had the same defect.
    #[test]
    fn test_array_element_argument_is_not_uninitialized() {
        let result = borrow_check(
            r#"
            fn main() {
                let parts: [String; 2] = ["ab", "cd"];
                print_int(string_len(parts[0]));
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "array element argument reported uninitialized: {:?}",
            result.err()
        );
    }

    /// GENUINE ERROR, still rejected: resolving a projection to its base must not
    /// lose *partial* moves — a non-Copy field moved out of a struct stays moved.
    /// The field type is a struct, since `String` is Copy.
    #[test]
    fn test_moved_struct_field_is_still_rejected() {
        let result = borrow_check(
            r#"
            struct Inner { v: i64 }
            struct S { a: Inner }
            fn main() {
                let x: S = S { a: Inner { v: 1 } };
                let p = x.a;
                let q = x.a;
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::UseOfMovedValue { .. })),
            "reuse of a moved struct field was accepted: {:?}",
            result
        );
    }

    /// GENUINE ERROR, still rejected: a real move of a non-Copy value.
    ///
    /// This used to be spelled with a `String` (`let a = "x"; let b = a;`), but
    /// `String` is now Copy — it lowers to `const char*`, nothing frees it per
    /// value, and with no `clone` and no usable reference type the move rule was
    /// unsatisfiable. Struct values are still non-Copy, so the use-after-move
    /// path is exercised with one of those instead.
    #[test]
    fn test_use_after_move_is_still_rejected() {
        let result = borrow_check(
            r#"
            struct S { v: i64 }
            fn main() {
                let a: S = S { v: 1 };
                let b = a;
                let c = a;
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::UseOfMovedValue { .. })),
            "use after move was accepted: {:?}",
            result
        );
    }

    /// GENUINE ERROR, still rejected: two mutable borrows of the same array that
    /// really are live at the same time, because they are arguments to the *same*
    /// call. Releasing borrows per call must not release them per argument.
    #[test]
    fn test_aliased_mutable_arguments_in_one_call_are_rejected() {
        let result = borrow_check(
            r#"
            fn f(mut a: [i64; 4], mut b: [i64; 4]) { a[0] = 1; b[0] = 2; }
            fn main() {
                let mut xs = [0; 4];
                f(xs, xs);
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::ConflictingBorrows { .. })),
            "aliased mutable arguments were accepted: {:?}",
            result
        );
    }

    /// GENUINE ERROR, still rejected: a shared and a mutable borrow of the same
    /// place, live simultaneously as arguments to one call.
    #[test]
    fn test_shared_and_mutable_arguments_in_one_call_are_rejected() {
        let result = borrow_check(
            r#"
            struct S { x: i64 }
            fn g(a: &S, b: &mut S) -> i64 { return a.x; }
            fn main() {
                let mut v = S { x: 1 };
                print_int(g(&v, &mut v));
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::ConflictingBorrows { .. })),
            "shared+mutable arguments were accepted: {:?}",
            result
        );
    }

    /// A non-Copy value moves on `let`. Uses a struct type: `String` is Copy
    /// now (see `is_copy_type`), so it no longer exercises this path.
    #[test]
    fn test_basic_move() {
        let mut checker = BorrowChecker::new();

        let func = Function {
            visibility: Visibility::Private,
            is_async: false,
            name: "test".to_string(),
            lifetime_params: vec![],
            type_params: vec![],
            const_params: vec![],
            params: vec![],
            return_type: None,
            effects: None,
            body: vec![
                Stmt::Let {
                    name: "x".to_string(),
                    ty: Some(Type::Custom("S".to_string())),
                    value: Expr::StructLiteral {
                        name: "S".to_string(),
                        fields: vec![("v".to_string(), Expr::Integer(1))],
                        span: dummy_span(),
                    },
                    mutable: false,
                    span: dummy_span(),
                },
                Stmt::Let {
                    name: "y".to_string(),
                    ty: Some(Type::Custom("S".to_string())),
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
            is_async: false,
            name: "test".to_string(),
            lifetime_params: vec![],
            type_params: vec![],
            const_params: vec![],
            params: vec![],
            return_type: None,
            effects: None,
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
