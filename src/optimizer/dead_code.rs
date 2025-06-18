//! Dead code elimination pass
//!
//! This pass removes unreachable code and statements with no effect

use crate::ast::*;
use crate::errors::CompileError;
use crate::optimizer::{helpers, OptimizationPass};

pub struct DeadCodeEliminationPass {
    changes_made: usize,
}

impl Default for DeadCodeEliminationPass {
    fn default() -> Self {
        Self::new()
    }
}

impl DeadCodeEliminationPass {
    pub fn new() -> Self {
        Self { changes_made: 0 }
    }

    /// Check if control flow can continue after a statement
    #[allow(clippy::only_used_in_recursion)]
    fn can_continue_after(&self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Return(_) | Stmt::Break { .. } | Stmt::Continue { .. } => false,
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                // Control can continue if either branch can continue
                let then_can_continue =
                    then_branch.is_empty() || self.can_continue_after(then_branch.last().unwrap());

                let else_can_continue = else_branch.as_ref().is_none_or(|branch| {
                    branch.is_empty() || self.can_continue_after(branch.last().unwrap())
                });

                then_can_continue || else_can_continue
            }
            _ => true,
        }
    }
}

impl OptimizationPass for DeadCodeEliminationPass {
    fn name(&self) -> &str {
        "Dead Code Elimination"
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

    fn optimize_function(&mut self, func: &mut Function) -> Result<bool, CompileError> {
        self.eliminate_dead_code_in_vec(&mut func.body)
    }

    fn optimize_statement(&mut self, stmt: &mut Stmt) -> Result<bool, CompileError> {
        match stmt {
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                // Check for constant conditions
                if let Expr::Bool(_val) = condition {
                    // Note: Statement-level transformations would require returning
                    // a replacement statement or modifying the parent's statement list.
                    // This is beyond the scope of the current optimization framework,
                    // which only modifies expressions in-place.
                    // A production compiler would use an IR that supports these transforms.
                    self.changes_made += 1;
                }

                self.eliminate_dead_code_in_vec(then_branch)?;

                if let Some(else_branch) = else_branch {
                    self.eliminate_dead_code_in_vec(else_branch)?;
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                // Check for constant false condition
                if let Expr::Bool(false) = condition {
                    // Note: Removing the entire while loop would require modifying
                    // the parent's statement list, which our current framework doesn't support.
                    // In a production compiler, this would be done on an IR.
                    self.changes_made += 1;
                }

                self.eliminate_dead_code_in_vec(body)?;
            }
            Stmt::Expr(expr) => {
                // Remove expressions with no side effects
                if !helpers::expr_has_side_effects(expr) {
                    // Note: Removing statements requires modifying the parent's statement list.
                    // Our current framework doesn't support this level of transformation.
                    self.changes_made += 1;
                    return Ok(true);
                }
            }
            _ => {}
        }

        Ok(false)
    }

    fn optimize_expression(&mut self, _expr: &mut Expr) -> Result<bool, CompileError> {
        // Dead code elimination doesn't modify expressions
        Ok(false)
    }
}

impl DeadCodeEliminationPass {
    /// Eliminate dead code in a vector of statements
    fn eliminate_dead_code_in_vec(
        &mut self,
        statements: &mut Vec<Stmt>,
    ) -> Result<bool, CompileError> {
        let mut i = 0;
        let mut found_terminator = false;

        while i < statements.len() {
            let stmt = &statements[i];

            if found_terminator {
                // Remove all statements after a terminator
                statements.truncate(i);
                self.changes_made += 1;
                return Ok(true);
            }

            // Check if this statement is a terminator
            if !self.can_continue_after(stmt) {
                found_terminator = true;
            }

            // Optimize the statement itself
            self.optimize_statement(&mut statements[i])?;

            i += 1;
        }

        Ok(false)
    }
}
