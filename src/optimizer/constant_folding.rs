//! Constant folding optimization pass
//!
//! This pass evaluates constant expressions at compile time

use crate::ast::*;
use crate::errors::CompileError;
use crate::optimizer::{helpers, OptimizationPass};

pub struct ConstantFoldingPass {
    changes_made: usize,
}

impl Default for ConstantFoldingPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstantFoldingPass {
    pub fn new() -> Self {
        Self { changes_made: 0 }
    }
}

impl OptimizationPass for ConstantFoldingPass {
    fn name(&self) -> &str {
        "Constant Folding"
    }

    fn optimize_program(&mut self, program: &mut Program) -> Result<bool, CompileError> {
        self.changes_made = 0;

        for item in &mut program.items {
            match item {
                Item::Function(func) => {
                    self.optimize_function(func)?;
                }
                _ => {} // Skip other items for now
            }
        }

        Ok(self.changes_made > 0)
    }

    fn optimize_statement(&mut self, stmt: &mut Stmt) -> Result<bool, CompileError> {
        match stmt {
            Stmt::Let { value, .. } => {
                self.optimize_expression(value)?;
            }
            Stmt::Expr(expr) => {
                self.optimize_expression(expr)?;
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.optimize_expression(condition)?;

                // Optimize constant conditions
                if let Expr::Bool(_val) = condition {
                    // Note: To replace the if statement with just one branch,
                    // we would need to return a different statement type.
                    // This would require a more sophisticated optimization framework
                    // that can transform statements, not just expressions.
                    // For now, we optimize the condition and both branches.
                    self.changes_made += 1;

                    // The dead code elimination pass will handle removing
                    // unreachable branches based on the constant condition.
                }

                for stmt in then_branch {
                    self.optimize_statement(stmt)?;
                }

                if let Some(else_branch) = else_branch {
                    for stmt in else_branch {
                        self.optimize_statement(stmt)?;
                    }
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.optimize_expression(condition)?;

                for stmt in body {
                    self.optimize_statement(stmt)?;
                }
            }
            Stmt::Return(Some(expr)) => {
                self.optimize_expression(expr)?;
            }
            Stmt::Return(None) => {}
            Stmt::Assign { value, .. } => {
                self.optimize_expression(value)?;
            }
            Stmt::For { iter, body, .. } => {
                self.optimize_expression(iter)?;
                for stmt in body {
                    self.optimize_statement(stmt)?;
                }
            }
            _ => {}
        }

        Ok(false) // Statement structure not changed
    }

    fn optimize_expression(&mut self, expr: &mut Expr) -> Result<bool, CompileError> {
        match expr {
            Expr::Binary {
                left,
                op,
                right,
                span: _,
            } => {
                // First optimize sub-expressions
                self.optimize_expression(left)?;
                self.optimize_expression(right)?;

                // Then try to fold constants
                match (left.as_ref(), right.as_ref()) {
                    (Expr::Integer(l), Expr::Integer(r)) => {
                        // Fold integer arithmetic
                        if let Some(result) = helpers::eval_binary_int(*l, *op, *r) {
                            *expr = Expr::Integer(result);
                            self.changes_made += 1;
                            return Ok(true);
                        }

                        // Fold integer comparisons
                        if let Some(result) = helpers::eval_comparison(*l, *op, *r) {
                            *expr = Expr::Bool(result);
                            self.changes_made += 1;
                            return Ok(true);
                        }
                    }
                    (Expr::Bool(l), Expr::Bool(r)) => {
                        // Fold boolean operations
                        match op {
                            BinOp::And => {
                                *expr = Expr::Bool(*l && *r);
                                self.changes_made += 1;
                                return Ok(true);
                            }
                            BinOp::Or => {
                                *expr = Expr::Bool(*l || *r);
                                self.changes_made += 1;
                                return Ok(true);
                            }
                            BinOp::Eq => {
                                *expr = Expr::Bool(*l == *r);
                                self.changes_made += 1;
                                return Ok(true);
                            }
                            BinOp::Ne => {
                                *expr = Expr::Bool(*l != *r);
                                self.changes_made += 1;
                                return Ok(true);
                            }
                            _ => {}
                        }
                    }
                    (Expr::String(l), Expr::String(r)) => {
                        // Fold string concatenation
                        if matches!(op, BinOp::Add) {
                            let concatenated = format!("{}{}", l, r);
                            *expr = Expr::String(concatenated);
                            self.changes_made += 1;
                            return Ok(true);
                        }
                    }
                    _ => {}
                }

                // Short-circuit evaluation
                match (left.as_ref(), *op) {
                    // false && _ => false
                    (Expr::Bool(false), BinOp::And) => {
                        *expr = Expr::Bool(false);
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // true || _ => true
                    (Expr::Bool(true), BinOp::Or) => {
                        *expr = Expr::Bool(true);
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    _ => {}
                }
                
                match (*op, right.as_ref()) {
                    // _ && false => false
                    (BinOp::And, Expr::Bool(false)) => {
                        *expr = Expr::Bool(false);
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // _ || true => true
                    (BinOp::Or, Expr::Bool(true)) => {
                        *expr = Expr::Bool(true);
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    _ => {}
                }

                // Algebraic simplifications
                match (left.as_ref(), op, right.as_ref()) {
                    // x + 0 => x
                    (_, BinOp::Add, Expr::Integer(0)) => {
                        *expr = left.as_ref().clone();
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // 0 + x => x
                    (Expr::Integer(0), BinOp::Add, _) => {
                        *expr = right.as_ref().clone();
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // x - 0 => x
                    (_, BinOp::Sub, Expr::Integer(0)) => {
                        *expr = left.as_ref().clone();
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // x * 0 => 0
                    (_, BinOp::Mul, Expr::Integer(0)) | (Expr::Integer(0), BinOp::Mul, _) => {
                        *expr = Expr::Integer(0);
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // x * 1 => x
                    (_, BinOp::Mul, Expr::Integer(1)) => {
                        *expr = left.as_ref().clone();
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // 1 * x => x
                    (Expr::Integer(1), BinOp::Mul, _) => {
                        *expr = right.as_ref().clone();
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // x / 1 => x
                    (_, BinOp::Div, Expr::Integer(1)) => {
                        *expr = left.as_ref().clone();
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    _ => {}
                }
            }
            Expr::Unary { op, operand, .. } => {
                self.optimize_expression(operand)?;

                // Fold unary operations on constants
                match (op, operand.as_ref()) {
                    (UnaryOp::Neg, Expr::Integer(n)) => {
                        *expr = Expr::Integer(-n);
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    (UnaryOp::Not, Expr::Bool(b)) => {
                        *expr = Expr::Bool(!b);
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    _ => {}
                }
            }
            Expr::Call { args, .. } => {
                // Optimize function arguments
                for arg in args {
                    self.optimize_expression(arg)?;
                }
            }
            Expr::Index { array, index, .. } => {
                self.optimize_expression(array)?;
                self.optimize_expression(index)?;
            }
            Expr::FieldAccess { object, .. } => {
                self.optimize_expression(object)?;
            }
            Expr::ArrayLiteral { elements, .. } => {
                for elem in elements {
                    self.optimize_expression(elem)?;
                }
            }
            _ => {} // Literals and identifiers don't need optimization
        }

        Ok(false)
    }
}
