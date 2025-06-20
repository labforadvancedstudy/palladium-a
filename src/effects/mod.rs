// Effect system for Palladium
// "Tracking the ripples of computation"

use crate::ast::{Expr, Function, Stmt};
use crate::errors::Result;
use std::collections::HashSet;

/// Represents different kinds of effects a function can have
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Effect {
    /// IO operations (file, network, console)
    IO,
    /// Memory allocation/deallocation
    Memory,
    /// Can panic or throw exceptions
    Panic,
    /// Asynchronous operations
    Async,
    /// Unsafe operations
    Unsafe,
    /// Pure - no side effects
    Pure,
}

/// Effect set for a function or expression
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffectSet {
    effects: HashSet<Effect>,
}

impl EffectSet {
    /// Create an empty effect set (pure)
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an effect set with a single effect
    pub fn singleton(effect: Effect) -> Self {
        let mut effects = HashSet::new();
        effects.insert(effect);
        Self { effects }
    }

    /// Add an effect to the set
    pub fn add(&mut self, effect: Effect) {
        // Pure effect is removed when any other effect is added
        if effect != Effect::Pure {
            self.effects.remove(&Effect::Pure);
        }
        self.effects.insert(effect);
    }

    /// Union two effect sets
    pub fn union(&mut self, other: &EffectSet) {
        for effect in &other.effects {
            self.add(effect.clone());
        }
    }

    /// Check if the effect set is pure
    pub fn is_pure(&self) -> bool {
        self.effects.is_empty() || self.effects.contains(&Effect::Pure)
    }

    /// Check if the effect set contains a specific effect
    pub fn contains(&self, effect: &Effect) -> bool {
        self.effects.contains(effect)
    }

    /// Get all effects
    pub fn effects(&self) -> &HashSet<Effect> {
        &self.effects
    }
}

/// Effect analyzer for tracking effects in code
pub struct EffectAnalyzer {
    /// Map of function names to their effect sets
    function_effects: std::collections::HashMap<String, EffectSet>,
    /// Built-in functions and their effects
    builtin_effects: std::collections::HashMap<String, EffectSet>,
}

impl Default for EffectAnalyzer {
    fn default() -> Self {
        let mut builtin_effects = std::collections::HashMap::new();

        // IO functions
        builtin_effects.insert("print".to_string(), EffectSet::singleton(Effect::IO));
        builtin_effects.insert("print_int".to_string(), EffectSet::singleton(Effect::IO));
        builtin_effects.insert("file_open".to_string(), EffectSet::singleton(Effect::IO));
        builtin_effects.insert(
            "file_read_all".to_string(),
            EffectSet::singleton(Effect::IO),
        );
        builtin_effects.insert(
            "file_read_line".to_string(),
            EffectSet::singleton(Effect::IO),
        );
        builtin_effects.insert("file_write".to_string(), EffectSet::singleton(Effect::IO));
        builtin_effects.insert("file_close".to_string(), EffectSet::singleton(Effect::IO));
        builtin_effects.insert("file_exists".to_string(), EffectSet::singleton(Effect::IO));

        // Memory functions
        // For now, we don't have explicit allocation functions

        // Pure functions
        builtin_effects.insert("string_len".to_string(), EffectSet::new());
        builtin_effects.insert("string_concat".to_string(), EffectSet::new());
        builtin_effects.insert("string_eq".to_string(), EffectSet::new());
        builtin_effects.insert("string_char_at".to_string(), EffectSet::new());
        builtin_effects.insert("string_substring".to_string(), EffectSet::new());
        builtin_effects.insert("string_from_char".to_string(), EffectSet::new());
        builtin_effects.insert("char_is_digit".to_string(), EffectSet::new());
        builtin_effects.insert("char_is_alpha".to_string(), EffectSet::new());
        builtin_effects.insert("char_is_whitespace".to_string(), EffectSet::new());
        builtin_effects.insert("string_to_int".to_string(), EffectSet::new());
        builtin_effects.insert("int_to_string".to_string(), EffectSet::new());

        Self {
            function_effects: std::collections::HashMap::new(),
            builtin_effects,
        }
    }
}

impl EffectAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze effects for a function
    pub fn analyze_function(&mut self, func: &Function) -> Result<EffectSet> {
        let mut effects = EffectSet::new();

        // Async functions have async effect
        if func.is_async {
            effects.add(Effect::Async);
        }

        // Analyze function body
        for stmt in &func.body {
            let stmt_effects = self.analyze_statement(stmt)?;
            effects.union(&stmt_effects);
        }

        // Store the function's effects
        self.function_effects
            .insert(func.name.clone(), effects.clone());

        Ok(effects)
    }

    /// Analyze effects for a statement
    fn analyze_statement(&mut self, stmt: &Stmt) -> Result<EffectSet> {
        match stmt {
            Stmt::Expr(expr) => self.analyze_expression(expr),
            Stmt::Let { value, .. } => self.analyze_expression(value),
            Stmt::Return(Some(expr)) => self.analyze_expression(expr),
            Stmt::Return(None) => Ok(EffectSet::new()),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let mut effects = self.analyze_expression(condition)?;

                for stmt in then_branch {
                    let stmt_effects = self.analyze_statement(stmt)?;
                    effects.union(&stmt_effects);
                }

                if let Some(else_stmts) = else_branch {
                    for stmt in else_stmts {
                        let stmt_effects = self.analyze_statement(stmt)?;
                        effects.union(&stmt_effects);
                    }
                }

                Ok(effects)
            }
            Stmt::While {
                condition, body, ..
            } => {
                let mut effects = self.analyze_expression(condition)?;

                for stmt in body {
                    let stmt_effects = self.analyze_statement(stmt)?;
                    effects.union(&stmt_effects);
                }

                Ok(effects)
            }
            Stmt::For {
                var: _, iter, body, ..
            } => {
                let mut effects = self.analyze_expression(iter)?;

                for stmt in body {
                    let stmt_effects = self.analyze_statement(stmt)?;
                    effects.union(&stmt_effects);
                }

                Ok(effects)
            }
            Stmt::Match { expr, arms, .. } => {
                let mut effects = self.analyze_expression(expr)?;

                for arm in arms {
                    // Pattern matching is pure

                    for stmt in &arm.body {
                        let stmt_effects = self.analyze_statement(stmt)?;
                        effects.union(&stmt_effects);
                    }
                }

                Ok(effects)
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => Ok(EffectSet::new()),
            Stmt::Unsafe { body, .. } => {
                let mut effects = EffectSet::singleton(Effect::Unsafe);

                for stmt in body {
                    let stmt_effects = self.analyze_statement(stmt)?;
                    effects.union(&stmt_effects);
                }

                Ok(effects)
            }
            Stmt::Assign { value, .. } => self.analyze_expression(value),
        }
    }

    /// Analyze effects for an expression
    fn analyze_expression(&mut self, expr: &Expr) -> Result<EffectSet> {
        match expr {
            // Literals are pure
            Expr::Integer(_) | Expr::String(_) | Expr::Bool(_) | Expr::Ident(_) => {
                Ok(EffectSet::new())
            }

            // Function calls may have effects
            Expr::Call { func, args, .. } => {
                let mut effects = EffectSet::new();

                // Analyze function expression (in case it's not just an identifier)
                let func_effects = self.analyze_expression(func)?;
                effects.union(&func_effects);

                // Analyze arguments
                for arg in args {
                    let arg_effects = self.analyze_expression(arg)?;
                    effects.union(&arg_effects);
                }

                // Add function's own effects
                if let Expr::Ident(func_name) = func.as_ref() {
                    if let Some(builtin_effects) = self.builtin_effects.get(func_name) {
                        effects.union(builtin_effects);
                    } else if let Some(func_effects) = self.function_effects.get(func_name) {
                        effects.union(func_effects);
                    }
                    // If function is unknown, we conservatively assume it's pure
                }

                Ok(effects)
            }

            // Binary operations are usually pure
            Expr::Binary { left, right, .. } => {
                let mut effects = self.analyze_expression(left)?;
                let right_effects = self.analyze_expression(right)?;
                effects.union(&right_effects);
                Ok(effects)
            }

            // Unary operations are pure
            Expr::Unary { operand, .. } => self.analyze_expression(operand),

            // Array operations
            Expr::ArrayLiteral { elements, .. } => {
                let mut effects = EffectSet::new();
                for elem in elements {
                    let elem_effects = self.analyze_expression(elem)?;
                    effects.union(&elem_effects);
                }
                Ok(effects)
            }

            Expr::ArrayRepeat { value, count, .. } => {
                let mut effects = self.analyze_expression(value)?;
                let count_effects = self.analyze_expression(count)?;
                effects.union(&count_effects);
                Ok(effects)
            }

            Expr::Index { array, index, .. } => {
                let mut effects = self.analyze_expression(array)?;
                let index_effects = self.analyze_expression(index)?;
                effects.union(&index_effects);
                Ok(effects)
            }

            // Struct operations are pure
            Expr::StructLiteral { fields, .. } => {
                let mut effects = EffectSet::new();
                for (_, field_expr) in fields {
                    let field_effects = self.analyze_expression(field_expr)?;
                    effects.union(&field_effects);
                }
                Ok(effects)
            }

            Expr::FieldAccess { object, .. } => self.analyze_expression(object),

            // Enum operations are pure
            Expr::EnumConstructor { data, .. } => {
                let mut effects = EffectSet::new();
                if let Some(constructor_data) = data {
                    match constructor_data {
                        crate::ast::EnumConstructorData::Tuple(exprs) => {
                            for expr in exprs {
                                let expr_effects = self.analyze_expression(expr)?;
                                effects.union(&expr_effects);
                            }
                        }
                        crate::ast::EnumConstructorData::Struct(fields) => {
                            for (_, expr) in fields {
                                let expr_effects = self.analyze_expression(expr)?;
                                effects.union(&expr_effects);
                            }
                        }
                    }
                }
                Ok(effects)
            }

            // Range is pure
            Expr::Range { start, end, .. } => {
                let mut effects = EffectSet::new();
                let start_effects = self.analyze_expression(start)?;
                effects.union(&start_effects);
                let end_effects = self.analyze_expression(end)?;
                effects.union(&end_effects);
                Ok(effects)
            }

            // References are pure
            Expr::Reference { expr, .. } => self.analyze_expression(expr),
            Expr::Deref { expr, .. } => self.analyze_expression(expr),

            // Question mark operator can panic
            Expr::Question { expr, .. } => {
                let mut effects = self.analyze_expression(expr)?;
                effects.add(Effect::Panic);
                Ok(effects)
            }

            // Await has async effect
            Expr::Await { expr, .. } => {
                let mut effects = self.analyze_expression(expr)?;
                effects.add(Effect::Async);
                Ok(effects)
            }

            // Macros are analyzed based on their expansion
            Expr::MacroInvocation { .. } => {
                // For now, assume macros are pure
                // In a real implementation, we'd analyze the expanded code
                Ok(EffectSet::new())
            }
        }
    }

    /// Get the effects for a function
    pub fn get_function_effects(&self, func_name: &str) -> Option<&EffectSet> {
        self.function_effects.get(func_name)
    }

    /// Check if a function is pure
    pub fn is_function_pure(&self, func_name: &str) -> bool {
        self.function_effects
            .get(func_name)
            .map(|effects| effects.is_pure())
            .unwrap_or(true) // Unknown functions assumed pure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_set() {
        let mut effects = EffectSet::new();
        assert!(effects.is_pure());

        effects.add(Effect::IO);
        assert!(!effects.is_pure());
        assert!(effects.contains(&Effect::IO));

        let other = EffectSet::singleton(Effect::Async);
        effects.union(&other);
        assert!(effects.contains(&Effect::IO));
        assert!(effects.contains(&Effect::Async));
    }
}
