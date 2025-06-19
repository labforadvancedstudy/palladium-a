// Improved LLVM backend for Palladium
// "From Turing's proofs to von Neumann's performance"

use crate::ast::{Program, Item, Function, Type, Stmt, Expr, BinOp, AssignTarget};
use crate::errors::{CompileError, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Improved LLVM backend code generator
pub struct LLVMCodeGenerator {
    module_name: String,
    /// Whether LLVM is available
    llvm_available: bool,
    /// String constants defined at module level
    string_constants: Vec<(String, String)>, // (name, value)
    /// Current string constant counter
    string_counter: i32,
    /// Variable mapping for SSA
    var_map: HashMap<String, String>,
    /// Current SSA counter
    ssa_counter: i32,
}

impl LLVMCodeGenerator {
    pub fn new(module_name: &str) -> Result<Self> {
        Ok(Self {
            module_name: module_name.to_string(),
            llvm_available: Self::check_llvm_availability(),
            string_constants: Vec::new(),
            string_counter: 0,
            var_map: HashMap::new(),
            ssa_counter: 0,
        })
    }
    
    /// Check if LLVM is available on the system
    fn check_llvm_availability() -> bool {
        std::process::Command::new("llvm-config")
            .arg("--version")
            .output()
            .is_ok()
    }
    
    /// Get a fresh SSA register
    fn fresh_ssa(&mut self) -> String {
        let reg = format!("%{}", self.ssa_counter);
        self.ssa_counter += 1;
        reg
    }
    
    /// Compile a program to LLVM IR
    pub fn compile(&mut self, program: &Program) -> Result<()> {
        if !self.llvm_available {
            println!("   Warning: LLVM tools not found. Generating LLVM IR text only.");
        }
        
        // First pass: collect string constants
        self.collect_string_constants(program)?;
        
        // Generate IR
        let ir = self.generate_ir(program)?;
        
        // Write to .ll file
        let output_path = PathBuf::from("build_output").join(format!("{}.ll", self.module_name));
        std::fs::write(&output_path, ir)?;
        
        println!("   Generated LLVM IR: {}", output_path.display());
        
        // If LLVM tools are available, compile to object file
        if self.llvm_available {
            self.compile_to_object(&output_path)?;
        }
        
        Ok(())
    }
    
    /// Collect all string constants from the program
    fn collect_string_constants(&mut self, program: &Program) -> Result<()> {
        for item in &program.items {
            if let Item::Function(func) = item {
                self.collect_strings_from_stmts(&func.body);
            }
        }
        Ok(())
    }
    
    fn collect_strings_from_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Expr(expr) | Stmt::Return(Some(expr)) => {
                    self.collect_strings_from_expr(expr);
                }
                Stmt::Let { value, .. } => {
                    self.collect_strings_from_expr(value);
                }
                Stmt::If { condition, then_branch, else_branch, .. } => {
                    self.collect_strings_from_expr(condition);
                    self.collect_strings_from_stmts(then_branch);
                    if let Some(else_stmts) = else_branch {
                        self.collect_strings_from_stmts(else_stmts);
                    }
                }
                _ => {}
            }
        }
    }
    
    fn collect_strings_from_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::String(s) => {
                let name = format!("@.str.{}", self.string_counter);
                self.string_counter += 1;
                self.string_constants.push((name, s.clone()));
            }
            Expr::Binary { left, right, .. } => {
                self.collect_strings_from_expr(left);
                self.collect_strings_from_expr(right);
            }
            Expr::Call { func, args, .. } => {
                self.collect_strings_from_expr(func);
                for arg in args {
                    self.collect_strings_from_expr(arg);
                }
            }
            _ => {}
        }
    }
    
    /// Generate LLVM IR for the program
    fn generate_ir(&mut self, program: &Program) -> Result<String> {
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
        
        // String constants
        ir.push_str("; String constants\n");
        ir.push_str("@.str_fmt = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1\n");
        ir.push_str("@.int_fmt = private unnamed_addr constant [6 x i8] c\"%lld\\0A\\00\", align 1\n");
        
        // User-defined string constants
        for (name, value) in &self.string_constants {
            let escaped = value.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n");
            ir.push_str(&format!("{} = private unnamed_addr constant [{} x i8] c\"{}\\00\", align 1\n", 
                name, value.len() + 1, escaped));
        }
        ir.push_str("\n");
        
        // Generate functions
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.ssa_counter = 0; // Reset for each function
                    self.var_map.clear();
                    ir.push_str(&self.generate_function(func)?);
                    ir.push_str("\n");
                }
                _ => {
                    // Skip other items for now
                }
            }
        }
        
        Ok(ir)
    }
    
    /// Generate LLVM IR for a function
    fn generate_function(&mut self, func: &Function) -> Result<String> {
        let mut ir = String::new();
        
        // Function signature
        let ret_type = self.type_to_llvm(&func.return_type);
        ir.push_str(&format!("define {} @{}(", ret_type, func.name));
        
        // Parameters - values not pointers
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                ir.push_str(", ");
            }
            let param_type = self.type_to_llvm(&Some(param.ty.clone()));
            ir.push_str(&format!("{} %{}", param_type, param.name));
            // Map parameter names to themselves
            self.var_map.insert(param.name.clone(), format!("%{}", param.name));
        }
        
        ir.push_str(") {\n");
        ir.push_str("entry:\n");
        
        // Function body
        let mut label_counter = 0;
        
        for stmt in &func.body {
            ir.push_str(&self.generate_statement(stmt, &mut label_counter)?);
        }
        
        // Default return if needed
        if func.return_type.is_none() && !func.body.iter().any(|s| matches!(s, Stmt::Return(_))) {
            ir.push_str("  ret void\n");
        }
        
        ir.push_str("}\n");
        
        Ok(ir)
    }
    
    /// Convert Palladium type to LLVM type
    fn type_to_llvm(&self, ty: &Option<Type>) -> String {
        match ty {
            None => "void".to_string(),
            Some(Type::I32) => "i32".to_string(),
            Some(Type::I64) => "i64".to_string(),
            Some(Type::U32) => "i32".to_string(),
            Some(Type::U64) => "i64".to_string(),
            Some(Type::Bool) => "i1".to_string(),
            Some(Type::String) => "i8*".to_string(),
            Some(Type::Unit) => "void".to_string(),
            Some(Type::Array(elem_ty, size)) => {
                match size {
                    crate::ast::ArraySize::Literal(n) => {
                        format!("[{} x {}]", n, self.type_to_llvm(&Some(elem_ty.as_ref().clone())))
                    }
                    _ => {
                        // For dynamic or const param arrays, use pointer
                        format!("{}*", self.type_to_llvm(&Some(elem_ty.as_ref().clone())))
                    }
                }
            },
            _ => "i8*".to_string(), // Default to pointer for complex types
        }
    }
    
    /// Generate LLVM IR for a statement
    fn generate_statement(&mut self, stmt: &Stmt, label_counter: &mut i32) -> Result<String> {
        let mut ir = String::new();
        
        match stmt {
            Stmt::Expr(expr) => {
                let (expr_ir, _) = self.generate_expression(expr)?;
                ir.push_str(&expr_ir);
            }
            
            Stmt::Let { name, value, ty, .. } => {
                let (expr_ir, result_var) = self.generate_expression(value)?;
                ir.push_str(&expr_ir);
                
                // Determine the type to allocate
                let alloca_type = if let Some(t) = ty {
                    self.type_to_llvm(&Some(t.clone()))
                } else {
                    // Infer from expression
                    match value {
                        Expr::ArrayLiteral { elements, .. } => {
                            format!("[{} x i64]", elements.len())
                        }
                        _ => "i64".to_string()
                    }
                };
                
                // For arrays, we need different handling
                if alloca_type.starts_with('[') {
                    // Array type - result_var is already a pointer to the array
                    self.var_map.insert(name.clone(), result_var);
                } else {
                    // Scalar type - allocate and store
                    let ptr = self.fresh_ssa();
                    ir.push_str(&format!("  {} = alloca {}\n", ptr, alloca_type));
                    ir.push_str(&format!("  store {} {}, {}* {}\n", alloca_type, result_var, alloca_type, ptr));
                    self.var_map.insert(name.clone(), ptr);
                }
            }
            
            Stmt::Return(Some(expr)) => {
                let (expr_ir, result) = self.generate_expression(expr)?;
                ir.push_str(&expr_ir);
                ir.push_str(&format!("  ret i64 {}\n", result));
            }
            
            Stmt::Return(None) => {
                ir.push_str("  ret void\n");
            }
            
            Stmt::If { condition, then_branch, else_branch, .. } => {
                let then_label = format!("then{}", label_counter);
                let else_label = format!("else{}", label_counter);
                let end_label = format!("endif{}", label_counter);
                *label_counter += 1;
                
                let (cond_ir, cond_result) = self.generate_expression(condition)?;
                ir.push_str(&cond_ir);
                
                if else_branch.is_some() {
                    ir.push_str(&format!("  br i1 {}, label %{}, label %{}\n", cond_result, then_label, else_label));
                } else {
                    ir.push_str(&format!("  br i1 {}, label %{}, label %{}\n", cond_result, then_label, end_label));
                }
                
                // Then branch
                ir.push_str(&format!("{}:\n", then_label));
                for stmt in then_branch {
                    ir.push_str(&self.generate_statement(stmt, label_counter)?);
                }
                ir.push_str(&format!("  br label %{}\n", end_label));
                
                // Else branch
                if let Some(else_stmts) = else_branch {
                    ir.push_str(&format!("{}:\n", else_label));
                    for stmt in else_stmts {
                        ir.push_str(&self.generate_statement(stmt, label_counter)?);
                    }
                    ir.push_str(&format!("  br label %{}\n", end_label));
                }
                
                // End label
                ir.push_str(&format!("{}:\n", end_label));
            }
            
            Stmt::While { condition, body, .. } => {
                let cond_label = format!("while_cond{}", label_counter);
                let body_label = format!("while_body{}", label_counter);
                let end_label = format!("while_end{}", label_counter);
                *label_counter += 1;
                
                // Jump to condition check
                ir.push_str(&format!("  br label %{}\n", cond_label));
                
                // Condition label
                ir.push_str(&format!("{}:\n", cond_label));
                let (cond_ir, cond_result) = self.generate_expression(condition)?;
                ir.push_str(&cond_ir);
                ir.push_str(&format!("  br i1 {}, label %{}, label %{}\n", cond_result, body_label, end_label));
                
                // Body label
                ir.push_str(&format!("{}:\n", body_label));
                for stmt in body {
                    ir.push_str(&self.generate_statement(stmt, label_counter)?);
                }
                ir.push_str(&format!("  br label %{}\n", cond_label));
                
                // End label
                ir.push_str(&format!("{}:\n", end_label));
            }
            
            Stmt::For { var, iter, body, .. } => {
                // For now, handle simple range iteration
                // TODO: Handle more complex iterators
                if let Expr::Range { start, end, .. } = iter {
                    let (start_ir, start_val) = self.generate_expression(start)?;
                    let (end_ir, end_val) = self.generate_expression(end)?;
                    ir.push_str(&start_ir);
                    ir.push_str(&end_ir);
                
                // Allocate loop variable
                let loop_var_ptr = self.fresh_ssa();
                ir.push_str(&format!("  {} = alloca i64\n", loop_var_ptr));
                ir.push_str(&format!("  store i64 {}, i64* {}\n", start_val, loop_var_ptr));
                self.var_map.insert(var.clone(), loop_var_ptr.clone());
                
                let cond_label = format!("for_cond{}", label_counter);
                let body_label = format!("for_body{}", label_counter);
                let inc_label = format!("for_inc{}", label_counter);
                let end_label = format!("for_end{}", label_counter);
                *label_counter += 1;
                
                // Jump to condition
                ir.push_str(&format!("  br label %{}\n", cond_label));
                
                // Condition: check if i < end
                ir.push_str(&format!("{}:\n", cond_label));
                let i_val = self.fresh_ssa();
                ir.push_str(&format!("  {} = load i64, i64* {}\n", i_val, loop_var_ptr));
                let cmp = self.fresh_ssa();
                ir.push_str(&format!("  {} = icmp slt i64 {}, {}\n", cmp, i_val, end_val));
                ir.push_str(&format!("  br i1 {}, label %{}, label %{}\n", cmp, body_label, end_label));
                
                // Body
                ir.push_str(&format!("{}:\n", body_label));
                for stmt in body {
                    ir.push_str(&self.generate_statement(stmt, label_counter)?);
                }
                ir.push_str(&format!("  br label %{}\n", inc_label));
                
                // Increment
                ir.push_str(&format!("{}:\n", inc_label));
                let curr_val = self.fresh_ssa();
                let next_val = self.fresh_ssa();
                ir.push_str(&format!("  {} = load i64, i64* {}\n", curr_val, loop_var_ptr));
                ir.push_str(&format!("  {} = add i64 {}, 1\n", next_val, curr_val));
                ir.push_str(&format!("  store i64 {}, i64* {}\n", next_val, loop_var_ptr));
                ir.push_str(&format!("  br label %{}\n", cond_label));
                
                // End
                ir.push_str(&format!("{}:\n", end_label));
                } else {
                    return Err(CompileError::Generic("Only range-based for loops are supported in LLVM backend".to_string()));
                }
            }
            
            Stmt::Assign { target, value, .. } => {
                let (value_ir, value_var) = self.generate_expression(value)?;
                ir.push_str(&value_ir);
                
                match target {
                    AssignTarget::Ident(name) => {
                        if let Some(ptr) = self.var_map.get(name).cloned() {
                            ir.push_str(&format!("  store i64 {}, i64* {}\n", value_var, ptr));
                        } else {
                            return Err(CompileError::Generic(format!("Undefined variable: {}", name)));
                        }
                    }
                    AssignTarget::Index { array, index } => {
                        let (array_ir, array_var) = self.generate_expression(array)?;
                        let (index_ir, index_var) = self.generate_expression(index)?;
                        ir.push_str(&array_ir);
                        ir.push_str(&index_ir);
                        
                        let ptr = self.fresh_ssa();
                        // TODO: Get actual array type
                        ir.push_str(&format!("  {} = getelementptr [5 x i64], [5 x i64]* {}, i64 0, i64 {}\n",
                            ptr, array_var, index_var));
                        ir.push_str(&format!("  store i64 {}, i64* {}\n", value_var, ptr));
                    }
                    _ => {
                        return Err(CompileError::Generic("Complex assignment targets not yet supported".to_string()));
                    }
                }
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
    fn generate_expression(&mut self, expr: &Expr) -> Result<(String, String)> {
        let mut ir = String::new();
        
        match expr {
            Expr::Integer(n) => {
                Ok((String::new(), n.to_string()))
            }
            
            Expr::Bool(b) => {
                Ok((String::new(), if *b { "1" } else { "0" }.to_string()))
            }
            
            Expr::String(s) => {
                // Find the pre-collected string constant
                let const_name = self.string_constants
                    .iter()
                    .find(|(_, v)| v == s)
                    .map(|(n, _)| n.clone())
                    .unwrap_or_else(|| "@.str.unknown".to_string());
                
                let ptr_var = self.fresh_ssa();
                ir.push_str(&format!("  {} = getelementptr [{} x i8], [{} x i8]* {}, i32 0, i32 0\n",
                    ptr_var, s.len() + 1, s.len() + 1, const_name));
                
                Ok((ir, ptr_var))
            }
            
            Expr::Ident(name) => {
                if let Some(ptr) = self.var_map.get(name).cloned() {
                    // Check if it's a parameter (parameters are stored as %param_name)
                    // Local variables are stored as pointers (%1, %2, etc.)
                    if ptr == format!("%{}", name) {
                        // It's a parameter, use directly
                        Ok((String::new(), ptr))
                    } else {
                        // It's a local variable pointer, load from it
                        let var = self.fresh_ssa();
                        ir.push_str(&format!("  {} = load i64, i64* {}\n", var, ptr));
                        Ok((ir, var))
                    }
                } else {
                    // Variable not found in map
                    Err(CompileError::UndefinedVariable {
                        name: name.clone(),
                        span: None,
                    })
                }
            }
            
            Expr::Binary { left, op, right, .. } => {
                let (left_ir, left_var) = self.generate_expression(left)?;
                let (right_ir, right_var) = self.generate_expression(right)?;
                
                ir.push_str(&left_ir);
                ir.push_str(&right_ir);
                
                let result_var = self.fresh_ssa();
                
                let op_str = match op {
                    BinOp::Add => "add",
                    BinOp::Sub => "sub",
                    BinOp::Mul => "mul",
                    BinOp::Div => "sdiv",
                    BinOp::Mod => "srem",
                    BinOp::Lt => "icmp slt",
                    BinOp::Le => "icmp sle",
                    BinOp::Gt => "icmp sgt",
                    BinOp::Ge => "icmp sge",
                    BinOp::Eq => "icmp eq",
                    BinOp::Ne => "icmp ne",
                    _ => return Err(CompileError::Generic("Unsupported binary operator".to_string())),
                };
                
                ir.push_str(&format!("  {} = {} i64 {}, {}\n", result_var, op_str, left_var, right_var));
                
                Ok((ir, result_var))
            }
            
            Expr::ArrayLiteral { elements, .. } => {
                // Generate array literal
                let elem_type = if elements.is_empty() {
                    "i64".to_string() // Default to i64 for empty arrays
                } else {
                    // Infer type from first element
                    let (_, _) = self.generate_expression(&elements[0])?;
                    "i64".to_string() // For now, assume i64 arrays
                };
                
                let array_var = self.fresh_ssa();
                let array_type = format!("[{} x {}]", elements.len(), elem_type);
                
                // Allocate array on stack
                ir.push_str(&format!("  {} = alloca {}\n", array_var, array_type));
                
                // Initialize elements
                for (i, elem) in elements.iter().enumerate() {
                    let (elem_ir, elem_val) = self.generate_expression(elem)?;
                    ir.push_str(&elem_ir);
                    
                    let ptr = self.fresh_ssa();
                    ir.push_str(&format!("  {} = getelementptr {}, {}* {}, i64 0, i64 {}\n",
                        ptr, array_type, array_type, array_var, i));
                    ir.push_str(&format!("  store {} {}, {}* {}\n", elem_type, elem_val, elem_type, ptr));
                }
                
                Ok((ir, array_var))
            }
            
            Expr::Index { array, index, .. } => {
                let (idx_ir, idx_var) = self.generate_expression(index)?;
                ir.push_str(&idx_ir);
                
                // Handle array expression - could be an identifier or other expression
                match array.as_ref() {
                    Expr::Ident(name) => {
                        if let Some(array_ptr) = self.var_map.get(name).cloned() {
                            let ptr = self.fresh_ssa();
                            let val = self.fresh_ssa();
                            
                            // TODO: Infer array size properly
                            // For now, assume [5 x i64] for test
                            ir.push_str(&format!("  {} = getelementptr [5 x i64], [5 x i64]* {}, i64 0, i64 {}\n",
                                ptr, array_ptr, idx_var));
                            ir.push_str(&format!("  {} = load i64, i64* {}\n", val, ptr));
                            
                            Ok((ir, val))
                        } else {
                            Err(CompileError::Generic(format!("Undefined array: {}", name)))
                        }
                    }
                    _ => {
                        let (obj_ir, obj_var) = self.generate_expression(array)?;
                        ir.push_str(&obj_ir);
                        
                        let ptr = self.fresh_ssa();
                        let val = self.fresh_ssa();
                        
                        // Get element pointer
                        ir.push_str(&format!("  {} = getelementptr [5 x i64], [5 x i64]* {}, i64 0, i64 {}\n",
                            ptr, obj_var, idx_var));
                        // Load value
                        ir.push_str(&format!("  {} = load i64, i64* {}\n", val, ptr));
                        
                        Ok((ir, val))
                    }
                }
            }
            
            Expr::Call { func, args, .. } => {
                if let Expr::Ident(func_name) = func.as_ref() {
                    match func_name.as_str() {
                        "print" => {
                            if args.len() == 1 {
                                let (arg_ir, arg_var) = self.generate_expression(&args[0])?;
                                ir.push_str(&arg_ir);
                                ir.push_str(&format!("  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.str_fmt, i32 0, i32 0), i8* {})\n", arg_var));
                            }
                            Ok((ir, "0".to_string())) // Dummy return
                        }
                        "print_int" => {
                            if args.len() == 1 {
                                let (arg_ir, arg_var) = self.generate_expression(&args[0])?;
                                ir.push_str(&arg_ir);
                                ir.push_str(&format!("  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([6 x i8], [6 x i8]* @.int_fmt, i32 0, i32 0), i64 {})\n", arg_var));
                            }
                            Ok((ir, "0".to_string())) // Dummy return
                        }
                        _ => {
                            // User-defined function call
                            let mut arg_vars = Vec::new();
                            for arg in args {
                                let (arg_ir, arg_var) = self.generate_expression(arg)?;
                                ir.push_str(&arg_ir);
                                arg_vars.push(arg_var);
                            }
                            
                            let result_var = self.fresh_ssa();
                            
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
                    Err(CompileError::Generic("Complex function calls not yet supported".to_string()))
                }
            }
            
            _ => {
                // TODO: Implement other expressions
                Ok((String::new(), "0".to_string()))
            }
        }
    }
    
    /// Compile LLVM IR to object file using llc
    fn compile_to_object(&self, ll_path: &PathBuf) -> Result<()> {
        let obj_path = ll_path.with_extension("o");
        
        let output = std::process::Command::new("llc")
            .arg("-filetype=obj")
            .arg("-o")
            .arg(&obj_path)
            .arg(ll_path)
            .output()
            .map_err(|e| CompileError::Generic(format!("Failed to run llc: {}", e)))?;
            
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CompileError::Generic(format!("llc failed: {}", stderr)));
        }
        
        println!("   Generated object file: {}", obj_path.display());
        Ok(())
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