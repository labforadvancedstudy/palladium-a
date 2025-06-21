//! Expression simplification pass
//!
//! This pass simplifies expressions by removing redundant operations
//! and unnecessary parentheses in the generated code

use crate::ast::*;
use crate::errors::{CompileError, Span};
use crate::optimizer::OptimizationPass;

pub struct SimplificationPass {
    changes_made: usize,
}

impl Default for SimplificationPass {
    fn default() -> Self {
        Self::new()
    }
}

impl SimplificationPass {
    pub fn new() -> Self {
        Self { changes_made: 0 }
    }
}

impl OptimizationPass for SimplificationPass {
    fn name(&self) -> &str {
        "Expression Simplification"
    }

    fn optimize_program(&mut self, program: &mut Program) -> Result<bool, CompileError> {
        self.changes_made = 0;

        for item in &mut program.items {
            if let Item::Function(func) = item {
                self.optimize_function(func)?;
            }
        }

        Ok(self.changes_made > 0)
    }

    fn optimize_statement(&mut self, stmt: &mut Stmt) -> Result<bool, CompileError> {
        match stmt {
            Stmt::Let { value, .. } => {
                return self.optimize_expression(value);
            }
            Stmt::Expr(expr) => {
                return self.optimize_expression(expr);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let mut changed = self.optimize_expression(condition)?;

                for stmt in then_branch {
                    changed |= self.optimize_statement(stmt)?;
                }

                if let Some(else_branch) = else_branch {
                    for stmt in else_branch {
                        changed |= self.optimize_statement(stmt)?;
                    }
                }
                
                if changed {
                    return Ok(true);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                let mut changed = self.optimize_expression(condition)?;

                for stmt in body {
                    changed |= self.optimize_statement(stmt)?;
                }
                
                if changed {
                    return Ok(true);
                }
            }
            Stmt::Return(Some(expr)) => {
                return self.optimize_expression(expr);
            }
            Stmt::Return(None) => {}
            Stmt::Assign { value, .. } => {
                return self.optimize_expression(value);
            }
            _ => {}
        }

        Ok(false)
    }

    fn optimize_expression(&mut self, expr: &mut Expr) -> Result<bool, CompileError> {
        match expr {
            Expr::Binary {
                left, op, right, ..
            } => {
                // Optimize sub-expressions first
                let left_changed = self.optimize_expression(left)?;
                let right_changed = self.optimize_expression(right)?;

                // Simplify boolean comparisons
                match (left.as_ref(), *op, right.as_ref()) {
                    // x == true => x
                    (_, BinOp::Eq, Expr::Bool(true)) => {
                        *expr = left.as_ref().clone();
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // true == x => x
                    (Expr::Bool(true), BinOp::Eq, _) => {
                        *expr = right.as_ref().clone();
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // x == false => !x
                    (_, BinOp::Eq, Expr::Bool(false)) => {
                        *expr = Expr::Unary {
                            op: UnaryOp::Not,
                            operand: left.clone(),
                            span: Span::dummy(),
                        };
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // false == x => !x
                    (Expr::Bool(false), BinOp::Eq, _) => {
                        *expr = Expr::Unary {
                            op: UnaryOp::Not,
                            operand: right.clone(),
                            span: Span::dummy(),
                        };
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // x != false => x
                    (_, BinOp::Ne, Expr::Bool(false)) => {
                        *expr = left.as_ref().clone();
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // false != x => x
                    (Expr::Bool(false), BinOp::Ne, _) => {
                        *expr = right.as_ref().clone();
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // x != true => !x
                    (_, BinOp::Ne, Expr::Bool(true)) => {
                        *expr = Expr::Unary {
                            op: UnaryOp::Not,
                            operand: left.clone(),
                            span: Span::dummy(),
                        };
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    // true != x => !x
                    (Expr::Bool(true), BinOp::Ne, _) => {
                        *expr = Expr::Unary {
                            op: UnaryOp::Not,
                            operand: right.clone(),
                            span: Span::dummy(),
                        };
                        self.changes_made += 1;
                        return Ok(true);
                    }
                    _ => {}
                }

                // Simplify double negation patterns
                match (left.as_ref(), *op, right.as_ref()) {
                    // !(x == y) => x != y
                    (
                        Expr::Unary {
                            op: UnaryOp::Not,
                            operand,
                            ..
                        },
                        BinOp::Eq,
                        _,
                    ) => {
                        if let Expr::Binary {
                            left: l,
                            op: BinOp::Eq,
                            right: r,
                            ..
                        } = operand.as_ref()
                        {
                            *expr = Expr::Binary {
                                left: l.clone(),
                                op: BinOp::Ne,
                                right: r.clone(),
                                span: Span::dummy(),
                            };
                            self.changes_made += 1;
                            return Ok(true);
                        }
                    }
                    _ => {}
                }
                
                if left_changed || right_changed {
                    return Ok(true);
                }
            }
            Expr::Unary { op, operand, .. } => {
                let child_changed = self.optimize_expression(operand)?;

                // Simplify double negation: !!x => x
                if let UnaryOp::Not = op {
                    if let Expr::Unary {
                        op: UnaryOp::Not,
                        operand: inner,
                        ..
                    } = operand.as_ref()
                    {
                        *expr = inner.as_ref().clone();
                        self.changes_made += 1;
                        return Ok(true);
                    }
                }
                
                if child_changed {
                    return Ok(true);
                }
            }
            Expr::Call { args, .. } => {
                let mut changed = false;
                for arg in args {
                    changed |= self.optimize_expression(arg)?;
                }
                if changed {
                    return Ok(true);
                }
            }
            Expr::Index { array, index, .. } => {
                let array_changed = self.optimize_expression(array)?;
                let index_changed = self.optimize_expression(index)?;
                if array_changed || index_changed {
                    return Ok(true);
                }
            }
            Expr::FieldAccess { object, .. } => {
                if self.optimize_expression(object)? {
                    return Ok(true);
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                let mut changed = false;
                for elem in elements {
                    changed |= self.optimize_expression(elem)?;
                }
                if changed {
                    return Ok(true);
                }
            }
            _ => {}
        }

        Ok(false)
    }
}
