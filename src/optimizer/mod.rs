//! Optimization passes for the Palladium compiler
//!
//! This module implements various optimization passes that improve
//! the generated code quality without changing program semantics.

use crate::ast::*;
use crate::errors::CompileError;

mod constant_folding;
mod dead_code;
mod simplify;

#[cfg(test)]
mod constant_folding_test;
#[cfg(test)]
mod dead_code_test;
#[cfg(test)]
mod simplify_test;

pub use constant_folding::ConstantFoldingPass;
pub use dead_code::DeadCodeEliminationPass;
pub use simplify::SimplificationPass;

/// Trait for optimization passes
pub trait OptimizationPass {
    /// Name of the optimization pass for debugging
    fn name(&self) -> &str;

    /// Optimize a complete program
    fn optimize_program(&mut self, program: &mut Program) -> Result<bool, CompileError>;

    /// Optimize a single function (default implementation)
    fn optimize_function(&mut self, func: &mut Function) -> Result<bool, CompileError> {
        let mut changed = false;
        for stmt in &mut func.body {
            changed |= self.optimize_statement(stmt)?;
        }
        Ok(changed)
    }

    /// Optimize a statement
    fn optimize_statement(&mut self, stmt: &mut Stmt) -> Result<bool, CompileError>;

    /// Optimize an expression
    fn optimize_expression(&mut self, expr: &mut Expr) -> Result<bool, CompileError>;
}

/// Optimizer that runs multiple optimization passes
pub struct Optimizer {
    passes: Vec<Box<dyn OptimizationPass>>,
    enable_logging: bool,
}

impl Default for Optimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Optimizer {
    /// Create a new optimizer with default passes
    pub fn new() -> Self {
        Self {
            passes: vec![
                Box::new(ConstantFoldingPass::new()),
                Box::new(DeadCodeEliminationPass::new()),
                Box::new(SimplificationPass::new()),
            ],
            enable_logging: false,
        }
    }

    /// Enable optimization logging
    pub fn with_logging(mut self) -> Self {
        self.enable_logging = true;
        self
    }

    /// Add a custom optimization pass
    pub fn add_pass(&mut self, pass: Box<dyn OptimizationPass>) {
        self.passes.push(pass);
    }

    /// Run all optimization passes on the program
    pub fn optimize(&mut self, program: &mut Program) -> Result<(), CompileError> {
        let mut total_changes = 0;
        let mut iteration = 0;

        // Keep running passes until no more changes occur (fixed point)
        loop {
            let mut changed_in_iteration = false;

            for pass in &mut self.passes {
                if self.enable_logging {
                    println!("   Running {}", pass.name());
                }

                let changed = pass.optimize_program(program)?;
                if changed {
                    changed_in_iteration = true;
                    total_changes += 1;
                }
            }

            iteration += 1;

            // Stop if no changes or max iterations reached
            if !changed_in_iteration || iteration >= 10 {
                break;
            }
        }

        if self.enable_logging && total_changes > 0 {
            println!(
                "   Made {} optimization(s) in {} iteration(s)",
                total_changes, iteration
            );
        }

        Ok(())
    }
}

/// Helper functions for optimization passes
pub mod helpers {
    use crate::ast::*;

    /// Check if an expression is a compile-time constant
    pub fn is_constant(expr: &Expr) -> bool {
        match expr {
            Expr::Integer(_) | Expr::Bool(_) | Expr::String(_) => true,
            Expr::Binary { left, right, .. } => is_constant(left) && is_constant(right),
            Expr::Unary { operand, .. } => is_constant(operand),
            _ => false,
        }
    }

    /// Evaluate a binary operation on integers at compile time
    pub fn eval_binary_int(left: i64, op: BinOp, right: i64) -> Option<i64> {
        match op {
            BinOp::Add => Some(left.wrapping_add(right)),
            BinOp::Sub => Some(left.wrapping_sub(right)),
            BinOp::Mul => Some(left.wrapping_mul(right)),
            BinOp::Div => {
                if right != 0 {
                    Some(left / right)
                } else {
                    None // Division by zero
                }
            }
            BinOp::Mod => {
                if right != 0 {
                    Some(left % right)
                } else {
                    None
                }
            }
            _ => None, // Comparison operators return bool, not int
        }
    }

    /// Evaluate a comparison operation at compile time
    pub fn eval_comparison(left: i64, op: BinOp, right: i64) -> Option<bool> {
        match op {
            BinOp::Eq => Some(left == right),
            BinOp::Ne => Some(left != right),
            BinOp::Lt => Some(left < right),
            BinOp::Gt => Some(left > right),
            BinOp::Le => Some(left <= right),
            BinOp::Ge => Some(left >= right),
            _ => None,
        }
    }

    /// Check if a statement has side effects
    pub fn has_side_effects(stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Let { .. } => false,
            Stmt::Expr(expr) => expr_has_side_effects(expr),
            Stmt::If { .. } | Stmt::While { .. } => true,
            Stmt::Return(_) => true,
            Stmt::Break { .. } | Stmt::Continue { .. } => true,
            Stmt::Assign { .. } => true,
            _ => false,
        }
    }

    /// Check if an expression has side effects
    pub fn expr_has_side_effects(expr: &Expr) -> bool {
        match expr {
            Expr::Call { .. } => true, // Function calls might have side effects
            Expr::Binary { left, right, .. } => {
                expr_has_side_effects(left) || expr_has_side_effects(right)
            }
            Expr::Unary { operand, .. } => expr_has_side_effects(operand),
            Expr::Index { array, index, .. } => {
                expr_has_side_effects(array) || expr_has_side_effects(index)
            }
            Expr::FieldAccess { object, .. } => expr_has_side_effects(object),
            _ => false,
        }
    }
}
