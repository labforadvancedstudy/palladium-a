// LLVM backend for Palladium
// "From Turing's proofs to von Neumann's performance"

use crate::ast::{Expr, Function, Item, Program, Stmt, Type};
use crate::errors::{CompileError, Result};
use std::path::PathBuf;

/// LLVM backend code generator
pub struct LLVMCodeGenerator {
    module_name: String,
    /// Whether LLVM is available
    llvm_available: bool,
}

impl LLVMCodeGenerator {
    pub fn new(module_name: &str) -> Result<Self> {
        Ok(Self {
            module_name: module_name.to_string(),
            llvm_available: Self::check_llvm_availability(),
        })
    }

    /// Check if LLVM is available on the system
    fn check_llvm_availability() -> bool {
        // For now, we'll check if LLVM is available via llvm-config
        std::process::Command::new("llvm-config")
            .arg("--version")
            .output()
            .is_ok()
    }

    /// Compile a program to LLVM IR
    pub fn compile(&mut self, program: &Program) -> Result<()> {
        if !self.llvm_available {
            println!("   Warning: LLVM tools not found. Generating LLVM IR text only.");
        }

        // For now, generate LLVM IR as text
        let ir = self.generate_ir(program)?;

        // Write to .ll file
        let output_path = PathBuf::from("build_output").join(format!("{}.ll", self.module_name));
        std::fs::write(&output_path, ir)?;

        println!("   Generated LLVM IR: {}", output_path.display());

        Ok(())
    }

    /// Generate LLVM IR for the program
    fn generate_ir(&self, program: &Program) -> Result<String> {
        let mut ir = String::new();

        // Module header
        ir.push_str(&format!("; ModuleID = '{}'\n", self.module_name));
        ir.push_str("source_filename = \"palladium\"\n");
        ir.push_str("target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128\"\n");
        ir.push_str("target triple = \"x86_64-pc-linux-gnu\"\n\n");

        // External function declarations
        ir.push_str("; External function declarations\n");
        ir.push_str("declare i32 @printf(i8*, ...)\n");
        ir.push_str("declare i8* @malloc(i64)\n");
        ir.push_str("declare void @free(i8*)\n");
        ir.push_str("declare i64 @strlen(i8*)\n");
        ir.push_str("declare i8* @strcpy(i8*, i8*)\n");
        ir.push_str("declare i8* @strcat(i8*, i8*)\n");
        ir.push_str("declare i32 @strcmp(i8*, i8*)\n\n");

        // String constants for print functions
        ir.push_str("; String constants\n");
        ir.push_str(
            "@.str_fmt = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1\n",
        );
        ir.push_str(
            "@.int_fmt = private unnamed_addr constant [6 x i8] c\"%lld\\0A\\00\", align 1\n\n",
        );

        // Generate functions
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    ir.push_str(&self.generate_function(func)?);
                    ir.push('\n');
                }
                _ => {
                    // Skip other items for now
                }
            }
        }

        Ok(ir)
    }

    /// Generate LLVM IR for a function
    fn generate_function(&self, func: &Function) -> Result<String> {
        let mut ir = String::new();

        // Function signature
        let ret_type = self.type_to_llvm(&func.return_type);
        ir.push_str(&format!("define {} @{}(", ret_type, func.name));

        // Parameters
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                ir.push_str(", ");
            }
            let param_type = self.type_to_llvm(&Some(param.ty.clone()));
            ir.push_str(&format!("{} %{}", param_type, param.name));
        }

        ir.push_str(") {\n");
        ir.push_str("entry:\n");

        // Function body
        let mut label_counter = 0;
        let mut var_counter = 0;

        for stmt in &func.body {
            ir.push_str(&self.generate_statement(stmt, &mut var_counter, &mut label_counter)?);
        }

        // Default return if needed
        if func.return_type.is_none() && !func.body.iter().any(|s| matches!(s, Stmt::Return(_))) {
            ir.push_str("  ret void\n");
        }

        ir.push_str("}\n");

        Ok(ir)
    }

    /// Convert Palladium type to LLVM type
    fn type_to_llvm(&self, ty: &Option<Type>) -> &'static str {
        match ty {
            None => "void",
            Some(Type::I32) => "i32",
            Some(Type::I64) => "i64",
            Some(Type::U32) => "i32",
            Some(Type::U64) => "i64",
            Some(Type::Bool) => "i1",
            Some(Type::String) => "i8*",
            Some(Type::Unit) => "void",
            _ => "i8*", // Default to pointer for complex types
        }
    }

    /// Generate LLVM IR for a statement
    fn generate_statement(
        &self,
        stmt: &Stmt,
        var_counter: &mut i32,
        label_counter: &mut i32,
    ) -> Result<String> {
        let mut ir = String::new();

        match stmt {
            Stmt::Expr(expr) => {
                let (expr_ir, _) = self.generate_expression(expr, var_counter)?;
                ir.push_str(&expr_ir);
            }

            Stmt::Let { name, value, .. } => {
                let (expr_ir, result_var) = self.generate_expression(value, var_counter)?;
                ir.push_str(&expr_ir);

                // For simplicity, we'll use the variable name directly
                // In a real implementation, we'd need proper SSA form
                ir.push_str(&format!("  %{} = alloca i64\n", name));
                ir.push_str(&format!("  store i64 {}, i64* %{}\n", result_var, name));
            }

            Stmt::Return(Some(expr)) => {
                let (expr_ir, result) = self.generate_expression(expr, var_counter)?;
                ir.push_str(&expr_ir);
                ir.push_str(&format!("  ret i64 {}\n", result));
            }

            Stmt::Return(None) => {
                ir.push_str("  ret void\n");
            }

            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let then_label = format!("then{}", label_counter);
                let else_label = format!("else{}", label_counter);
                let end_label = format!("endif{}", label_counter);
                *label_counter += 1;

                let (cond_ir, cond_result) = self.generate_expression(condition, var_counter)?;
                ir.push_str(&cond_ir);

                if else_branch.is_some() {
                    ir.push_str(&format!(
                        "  br i1 {}, label %{}, label %{}\n",
                        cond_result, then_label, else_label
                    ));
                } else {
                    ir.push_str(&format!(
                        "  br i1 {}, label %{}, label %{}\n",
                        cond_result, then_label, end_label
                    ));
                }

                // Then branch
                ir.push_str(&format!("{}:\n", then_label));
                for stmt in then_branch {
                    ir.push_str(&self.generate_statement(stmt, var_counter, label_counter)?);
                }
                ir.push_str(&format!("  br label %{}\n", end_label));

                // Else branch
                if let Some(else_stmts) = else_branch {
                    ir.push_str(&format!("{}:\n", else_label));
                    for stmt in else_stmts {
                        ir.push_str(&self.generate_statement(stmt, var_counter, label_counter)?);
                    }
                    ir.push_str(&format!("  br label %{}\n", end_label));
                }

                // End label
                ir.push_str(&format!("{}:\n", end_label));
            }

            _ => {
                // TODO: Implement other statements
                ir.push_str("  ; TODO: Implement this statement\n");
            }
        }

        Ok(ir)
    }

    /// Generate LLVM IR for an expression
    /// Returns (IR code, result variable/value)
    #[allow(clippy::only_used_in_recursion)]
    fn generate_expression(&self, expr: &Expr, var_counter: &mut i32) -> Result<(String, String)> {
        let mut ir = String::new();

        match expr {
            Expr::Integer(n) => Ok((String::new(), n.to_string())),

            Expr::Bool(b) => Ok((String::new(), if *b { "1" } else { "0" }.to_string())),

            Expr::String(s) => {
                // Create a string constant
                let const_name = format!("@.str.{}", var_counter);
                *var_counter += 1;

                let escaped = s
                    .replace("\\", "\\\\")
                    .replace("\"", "\\\"")
                    .replace("\n", "\\n");
                ir.push_str(&format!(
                    "{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"\n",
                    const_name,
                    s.len() + 1,
                    escaped
                ));

                let ptr_var = format!("%str.{}", var_counter);
                *var_counter += 1;
                ir.push_str(&format!(
                    "  {} = getelementptr [{} x i8], [{} x i8]* {}, i32 0, i32 0\n",
                    ptr_var,
                    s.len() + 1,
                    s.len() + 1,
                    const_name
                ));

                Ok((ir, ptr_var))
            }

            Expr::Ident(name) => {
                let var = format!("%{}.load", var_counter);
                *var_counter += 1;
                ir.push_str(&format!("  {} = load i64, i64* %{}\n", var, name));
                Ok((ir, var))
            }

            Expr::Binary {
                left, op, right, ..
            } => {
                let (left_ir, left_var) = self.generate_expression(left, var_counter)?;
                let (right_ir, right_var) = self.generate_expression(right, var_counter)?;

                ir.push_str(&left_ir);
                ir.push_str(&right_ir);

                let result_var = format!("%{}", var_counter);
                *var_counter += 1;

                let op_str = match op {
                    crate::ast::BinOp::Add => "add",
                    crate::ast::BinOp::Sub => "sub",
                    crate::ast::BinOp::Mul => "mul",
                    crate::ast::BinOp::Div => "sdiv",
                    crate::ast::BinOp::Mod => "srem",
                    crate::ast::BinOp::Lt => "icmp slt",
                    crate::ast::BinOp::Le => "icmp sle",
                    crate::ast::BinOp::Gt => "icmp sgt",
                    crate::ast::BinOp::Ge => "icmp sge",
                    crate::ast::BinOp::Eq => "icmp eq",
                    crate::ast::BinOp::Ne => "icmp ne",
                    _ => {
                        return Err(CompileError::Generic(
                            "Unsupported binary operator".to_string(),
                        ))
                    }
                };

                ir.push_str(&format!(
                    "  {} = {} i64 {}, {}\n",
                    result_var, op_str, left_var, right_var
                ));

                Ok((ir, result_var))
            }

            Expr::Call { func, args, .. } => {
                if let Expr::Ident(func_name) = func.as_ref() {
                    match func_name.as_str() {
                        "print" => {
                            if args.len() == 1 {
                                let (arg_ir, arg_var) =
                                    self.generate_expression(&args[0], var_counter)?;
                                ir.push_str(&arg_ir);
                                ir.push_str(&format!("  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.str_fmt, i32 0, i32 0), i8* {})\n", arg_var));
                            }
                            Ok((ir, "%0".to_string())) // Dummy return
                        }
                        "print_int" => {
                            if args.len() == 1 {
                                let (arg_ir, arg_var) =
                                    self.generate_expression(&args[0], var_counter)?;
                                ir.push_str(&arg_ir);
                                ir.push_str(&format!("  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.int_fmt, i32 0, i32 0), i64 {})\n", arg_var));
                            }
                            Ok((ir, "%0".to_string())) // Dummy return
                        }
                        _ => {
                            // User-defined function call
                            let mut arg_vars = Vec::new();
                            for arg in args {
                                let (arg_ir, arg_var) =
                                    self.generate_expression(arg, var_counter)?;
                                ir.push_str(&arg_ir);
                                arg_vars.push(arg_var);
                            }

                            let result_var = format!("%{}", var_counter);
                            *var_counter += 1;

                            ir.push_str(&format!("  {} = call i64 @{}(", result_var, func_name));
                            for (i, arg_var) in arg_vars.iter().enumerate() {
                                if i > 0 {
                                    ir.push_str(", ");
                                }
                                ir.push_str(&format!("i64 {}", arg_var));
                            }
                            ir.push_str(")\n");

                            Ok((ir, result_var))
                        }
                    }
                } else {
                    Err(CompileError::Generic(
                        "Complex function calls not yet supported".to_string(),
                    ))
                }
            }

            _ => {
                // TODO: Implement other expressions
                Ok((String::new(), "%0".to_string()))
            }
        }
    }

    /// Write the generated LLVM IR to a file
    pub fn write_output(&self) -> Result<PathBuf> {
        let build_dir = PathBuf::from("build_output");
        if !build_dir.exists() {
            std::fs::create_dir_all(&build_dir)?;
        }

        let output_path = build_dir.join(format!("{}.ll", self.module_name));

        Ok(output_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llvm_availability() {
        let available = LLVMCodeGenerator::check_llvm_availability();
        println!("LLVM available: {}", available);
    }
}
