// Unsafe operations for Palladium
// "With great power comes great responsibility"

use crate::ast::{Expr, Function, Stmt};
use crate::errors::{CompileError, Result};

/// Unsafe operation checker
pub struct UnsafeChecker {
    /// Whether we're currently in an unsafe context
    in_unsafe_context: bool,
    /// Stack of unsafe contexts for nested unsafe blocks
    unsafe_stack: Vec<bool>,
}

impl UnsafeChecker {
    pub fn new() -> Self {
        Self {
            in_unsafe_context: false,
            unsafe_stack: Vec::new(),
        }
    }

    /// Enter an unsafe context
    fn enter_unsafe(&mut self) {
        self.unsafe_stack.push(self.in_unsafe_context);
        self.in_unsafe_context = true;
    }

    /// Exit an unsafe context
    fn exit_unsafe(&mut self) {
        self.in_unsafe_context = self.unsafe_stack.pop().unwrap_or(false);
    }

    /// Check if we're in an unsafe context
    fn is_unsafe_context(&self) -> bool {
        self.in_unsafe_context
    }

    /// Check a function for unsafe operations
    pub fn check_function(&mut self, func: &Function) -> Result<()> {
        // If the function is marked unsafe, the entire body is unsafe
        if func.name.starts_with("unsafe_") {
            self.enter_unsafe();
        }

        for stmt in &func.body {
            self.check_statement(stmt)?;
        }

        if func.name.starts_with("unsafe_") {
            self.exit_unsafe();
        }

        Ok(())
    }

    /// Check a statement for unsafe operations
    fn check_statement(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Unsafe { body, .. } => {
                self.enter_unsafe();
                for stmt in body {
                    self.check_statement(stmt)?;
                }
                self.exit_unsafe();
                Ok(())
            }

            Stmt::Expr(expr) => self.check_expression(expr),

            Stmt::Let { value, .. } => self.check_expression(value),

            Stmt::Return(Some(expr)) => self.check_expression(expr),

            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.check_expression(condition)?;
                for stmt in then_branch {
                    self.check_statement(stmt)?;
                }
                if let Some(else_stmts) = else_branch {
                    for stmt in else_stmts {
                        self.check_statement(stmt)?;
                    }
                }
                Ok(())
            }

            Stmt::While {
                condition, body, ..
            } => {
                self.check_expression(condition)?;
                for stmt in body {
                    self.check_statement(stmt)?;
                }
                Ok(())
            }

            Stmt::For { iter, body, .. } => {
                self.check_expression(iter)?;
                for stmt in body {
                    self.check_statement(stmt)?;
                }
                Ok(())
            }

            Stmt::Match { expr, arms, .. } => {
                self.check_expression(expr)?;
                for arm in arms {
                    for stmt in &arm.body {
                        self.check_statement(stmt)?;
                    }
                }
                Ok(())
            }

            Stmt::Assign { value, .. } => self.check_expression(value),

            _ => Ok(()),
        }
    }

    /// Check an expression for unsafe operations
    fn check_expression(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            // Dereferencing a raw pointer is unsafe
            Expr::Deref {
                expr: inner, span, ..
            } => {
                // Check if the inner expression is a raw pointer
                // For now, we'll just check the inner expression
                self.check_expression(inner)?;

                // If this is a raw pointer dereference and we're not in unsafe context
                if !self.is_unsafe_context() && self.is_raw_pointer_expr(inner) {
                    return Err(CompileError::UnsafeOperation {
                        operation: "raw pointer dereference".to_string(),
                        span: span.clone(),
                    });
                }

                Ok(())
            }

            // Calling unsafe functions requires unsafe context
            Expr::Call {
                func, args, span, ..
            } => {
                if let Expr::Ident(func_name) = func.as_ref() {
                    if self.is_unsafe_function(func_name) && !self.is_unsafe_context() {
                        return Err(CompileError::UnsafeOperation {
                            operation: format!("call to unsafe function '{}'", func_name),
                            span: span.clone(),
                        });
                    }
                }

                // Check arguments
                for arg in args {
                    self.check_expression(arg)?;
                }

                Ok(())
            }

            // Other expressions - recursively check sub-expressions
            Expr::Binary { left, right, .. } => {
                self.check_expression(left)?;
                self.check_expression(right)?;
                Ok(())
            }

            Expr::Unary { operand, .. } => self.check_expression(operand),

            Expr::Index { array, index, .. } => {
                self.check_expression(array)?;
                self.check_expression(index)?;
                Ok(())
            }

            Expr::FieldAccess { object, .. } => self.check_expression(object),

            Expr::StructLiteral { fields, .. } => {
                for (_, field_expr) in fields {
                    self.check_expression(field_expr)?;
                }
                Ok(())
            }

            Expr::ArrayLiteral { elements, .. } => {
                for elem in elements {
                    self.check_expression(elem)?;
                }
                Ok(())
            }

            Expr::ArrayRepeat { value, count, .. } => {
                self.check_expression(value)?;
                self.check_expression(count)?;
                Ok(())
            }

            Expr::EnumConstructor { data, .. } => {
                if let Some(constructor_data) = data {
                    match constructor_data {
                        crate::ast::EnumConstructorData::Tuple(exprs) => {
                            for expr in exprs {
                                self.check_expression(expr)?;
                            }
                        }
                        crate::ast::EnumConstructorData::Struct(fields) => {
                            for (_, expr) in fields {
                                self.check_expression(expr)?;
                            }
                        }
                    }
                }
                Ok(())
            }

            Expr::Range { start, end, .. } => {
                self.check_expression(start)?;
                self.check_expression(end)?;
                Ok(())
            }

            Expr::Reference { expr, .. } => self.check_expression(expr),

            Expr::Question { expr, .. } => self.check_expression(expr),

            Expr::Await { expr, .. } => self.check_expression(expr),

            // Literals and identifiers are safe
            Expr::Integer(_) | Expr::String(_) | Expr::Bool(_) | Expr::Ident(_) => Ok(()),

            Expr::MacroInvocation { .. } => Ok(()), // Macros are expanded before this phase
        }
    }

    /// Check if an expression is a raw pointer
    fn is_raw_pointer_expr(&self, _expr: &Expr) -> bool {
        // TODO: Implement raw pointer detection
        // For now, we'll check if the expression contains "ptr" in its identifier
        match _expr {
            Expr::Ident(name) => name.contains("ptr") || name.contains("pointer"),
            _ => false,
        }
    }

    /// Check if a function is unsafe
    fn is_unsafe_function(&self, func_name: &str) -> bool {
        // Functions starting with "unsafe_" are considered unsafe
        func_name.starts_with("unsafe_") ||
        // Some built-in unsafe functions
        matches!(func_name, "raw_ptr_deref" | "transmute" | "mem_write" | "mem_read")
    }
}

/// Unsafe operation types
#[derive(Debug, Clone, PartialEq)]
pub enum UnsafeOp {
    /// Raw pointer dereference
    RawPointerDeref,
    /// Call to unsafe function
    UnsafeFunctionCall(String),
    /// Access to mutable static
    MutableStaticAccess(String),
    /// Union field access
    UnionFieldAccess,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    #[test]
    fn test_unsafe_checker() {
        let source = r#"
        fn main() {
            let x = 42;
            unsafe {
                let y = x + 1;
                print_int(y);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut checker = UnsafeChecker::new();
        for item in &ast.items {
            if let crate::ast::Item::Function(func) = item {
                assert!(checker.check_function(func).is_ok());
            }
        }
    }
}
