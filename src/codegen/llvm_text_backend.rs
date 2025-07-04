// Text-based LLVM IR backend for Palladium
// "Native code generation without dependencies"

use crate::ast::{ArraySize, AssignTarget, BinOp, Expr, Function, Item, Pattern, Program, Stmt, Type, UnaryOp};
use crate::errors::{CompileError, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// LLVM IR text generator - works without llvm-sys
pub struct LLVMTextBackend {
    module_name: String,
    /// String constants defined at module level
    string_constants: Vec<(String, String)>, // (name, value)
    /// Current string constant counter
    string_counter: i32,
    /// Variable mapping for SSA
    var_map: HashMap<String, VarInfo>,
    /// Current SSA counter
    ssa_counter: i32,
    /// Current label counter
    label_counter: i32,
}

#[derive(Clone, Debug)]
struct VarInfo {
    ptr: String,    // SSA register holding the pointer
    ty: String,     // LLVM type string
    #[allow(dead_code)]
    is_param: bool, // Whether this is a function parameter
}

impl LLVMTextBackend {
    pub fn new(module_name: &str) -> Result<Self> {
        Ok(Self {
            module_name: module_name.to_string(),
            string_constants: Vec::new(),
            string_counter: 0,
            var_map: HashMap::new(),
            ssa_counter: 0,
            label_counter: 0,
        })
    }

    /// Get a fresh SSA register
    fn fresh_ssa(&mut self) -> String {
        let reg = format!("%{}", self.ssa_counter);
        self.ssa_counter += 1;
        reg
    }

    /// Get a fresh label
    fn fresh_label(&mut self, prefix: &str) -> String {
        let label = format!("{}{}", prefix, self.label_counter);
        self.label_counter += 1;
        label
    }

    /// Compile a program to LLVM IR
    pub fn compile(&mut self, program: &Program) -> Result<String> {
        // First pass: collect string constants
        self.collect_string_constants(program)?;

        // Generate IR
        let mut ir = String::new();

        // Module header
        ir.push_str(&format!("; ModuleID = '{}'\n", self.module_name));
        ir.push_str("source_filename = \"palladium\"\n");

        // Target information for x86_64
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
        ir.push_str(
            "@.str_fmt = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1\n",
        );
        ir.push_str(
            "@.int_fmt = private unnamed_addr constant [6 x i8] c\"%lld\\0A\\00\", align 1\n",
        );

        // User-defined string constants
        for (name, value) in &self.string_constants {
            let escaped = value
                .replace("\\", "\\\\")
                .replace("\"", "\\\"")
                .replace("\n", "\\n");
            ir.push_str(&format!(
                "{} = private unnamed_addr constant [{} x i8] c\"{}\\00\", align 1\n",
                name,
                value.len() + 1,
                escaped
            ));
        }
        ir.push('\n');

        // Generate functions
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.ssa_counter = 0; // Reset for each function
                    self.var_map.clear();
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
                Stmt::If {
                    condition,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.collect_strings_from_expr(condition);
                    self.collect_strings_from_stmts(then_branch);
                    if let Some(else_stmts) = else_branch {
                        self.collect_strings_from_stmts(else_stmts);
                    }
                }
                Stmt::While {
                    condition, body, ..
                } => {
                    self.collect_strings_from_expr(condition);
                    self.collect_strings_from_stmts(body);
                }
                Stmt::For { iter, body, .. } => {
                    self.collect_strings_from_expr(iter);
                    self.collect_strings_from_stmts(body);
                }
                Stmt::Assign { value, .. } => {
                    self.collect_strings_from_expr(value);
                }
                _ => {}
            }
        }
    }

    fn collect_strings_from_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::String(s) => {
                // Check if we already have this string
                let exists = self.string_constants.iter().any(|(_, v)| v == s);
                if !exists {
                    let name = format!("@.str.{}", self.string_counter);
                    self.string_counter += 1;
                    self.string_constants.push((name, s.clone()));
                }
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
            Expr::ArrayLiteral { elements, .. } => {
                for elem in elements {
                    self.collect_strings_from_expr(elem);
                }
            }
            Expr::Index { array, index, .. } => {
                self.collect_strings_from_expr(array);
                self.collect_strings_from_expr(index);
            }
            Expr::Range { start, end, .. } => {
                self.collect_strings_from_expr(start);
                self.collect_strings_from_expr(end);
            }
            _ => {}
        }
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
            let param_reg = format!("%{}", param.name);
            ir.push_str(&format!("{} {}", param_type, param_reg));
        }

        ir.push_str(") {\n");
        ir.push_str("entry:\n");

        // Store parameters to local variables
        for param in &func.params {
            let param_type = self.type_to_llvm(&Some(param.ty.clone()));
            let param_reg = format!("%{}", param.name);
            let alloca = self.fresh_ssa();

            ir.push_str(&format!("  {} = alloca {}\n", alloca, param_type));
            ir.push_str(&format!(
                "  store {} {}, {}* {}\n",
                param_type, param_reg, param_type, alloca
            ));

            self.var_map.insert(
                param.name.clone(),
                VarInfo {
                    ptr: alloca,
                    ty: param_type,
                    is_param: true,
                },
            );
        }

        // Function body
        let mut has_terminator = false;
        for stmt in &func.body {
            if has_terminator {
                break; // Don't generate unreachable code
            }
            ir.push_str(&self.generate_statement(stmt)?);
            if Self::is_terminator(stmt) {
                has_terminator = true;
            }
        }

        // Default return if needed
        if func.return_type.is_none() && !func.body.iter().any(|s| matches!(s, Stmt::Return(_))) {
            ir.push_str("  ret void\n");
        }

        ir.push_str("}\n");

        Ok(ir)
    }

    /// Convert Palladium type to LLVM type
    #[allow(clippy::only_used_in_recursion)]
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
                    ArraySize::Literal(n) => {
                        format!(
                            "[{} x {}]",
                            n,
                            self.type_to_llvm(&Some(elem_ty.as_ref().clone()))
                        )
                    }
                    _ => {
                        // For dynamic or const param arrays, use pointer
                        format!("{}*", self.type_to_llvm(&Some(elem_ty.as_ref().clone())))
                    }
                }
            }
            _ => "i8*".to_string(), // Default to pointer for complex types
        }
    }

    /// Generate LLVM IR for a statement
    fn generate_statement(&mut self, stmt: &Stmt) -> Result<String> {
        let mut ir = String::new();

        match stmt {
            Stmt::Expr(expr) => {
                let (expr_ir, _) = self.generate_expression(expr)?;
                ir.push_str(&expr_ir);
            }

            Stmt::Let {
                name, value, ty, ..
            } => {
                // Generate the expression first
                let (expr_ir, result_var, result_type) = self.generate_expression_typed(value)?;
                ir.push_str(&expr_ir);

                // Determine the type to allocate
                let alloca_type = if let Some(t) = ty {
                    self.type_to_llvm(&Some(t.clone()))
                } else {
                    // Infer type from expression for better array handling
                    self.infer_expr_type(value)
                };

                // Allocate space for the variable
                let ptr = self.fresh_ssa();
                ir.push_str(&format!("  {} = alloca {}\n", ptr, alloca_type));

                // Store the value - handle arrays specially
                if alloca_type.starts_with('[') && alloca_type.ends_with(']') {
                    // For arrays, we need to copy element by element or use memcpy
                    // For now, just store the pointer if it's from an array literal
                    if matches!(value, Expr::ArrayLiteral { .. }) || matches!(value, Expr::ArrayRepeat { .. }) {
                        // The result_var is already a pointer to the array
                        // We need to copy the array contents
                        let array_size = if let Some(start) = alloca_type.find('[') {
                            if let Some(end) = alloca_type[start+1..].find(' ') {
                                alloca_type[start+1..start+1+end].parse::<usize>().unwrap_or(1)
                            } else {
                                1
                            }
                        } else {
                            1
                        };
                        
                        // Copy each element
                        for i in 0..array_size {
                            let src_ptr = self.fresh_ssa();
                            let dst_ptr = self.fresh_ssa();
                            let val = self.fresh_ssa();
                            
                            ir.push_str(&format!(
                                "  {} = getelementptr {}, {}* {}, i64 0, i64 {}\n",
                                src_ptr, alloca_type, alloca_type, result_var, i
                            ));
                            ir.push_str(&format!(
                                "  {} = getelementptr {}, {}* {}, i64 0, i64 {}\n",
                                dst_ptr, alloca_type, alloca_type, ptr, i
                            ));
                            ir.push_str(&format!(
                                "  {} = load i64, i64* {}\n",
                                val, src_ptr
                            ));
                            ir.push_str(&format!(
                                "  store i64 {}, i64* {}\n",
                                val, dst_ptr
                            ));
                        }
                    } else {
                        // For other array expressions, assume result_var is a pointer
                        // This is a simplification - proper array assignment would need memcpy
                        ir.push_str("  ; TODO: Proper array copy for non-literal arrays\n");
                    }
                } else {
                    ir.push_str(&format!(
                        "  store {} {}, {}* {}\n",
                        result_type, result_var, alloca_type, ptr
                    ));
                }

                // Save variable info
                self.var_map.insert(
                    name.clone(),
                    VarInfo {
                        ptr,
                        ty: alloca_type,
                        is_param: false,
                    },
                );
            }

            Stmt::Return(Some(expr)) => {
                let (expr_ir, result) = self.generate_expression(expr)?;
                ir.push_str(&expr_ir);

                // Infer return type from expression
                let ret_type = self.infer_expr_type(expr);
                ir.push_str(&format!("  ret {} {}\n", ret_type, result));
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
                let then_label = self.fresh_label("then");
                let else_label = self.fresh_label("else");
                let end_label = self.fresh_label("endif");

                let (cond_ir, cond_result) = self.generate_expression(condition)?;
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
                let mut has_terminator = false;
                for stmt in then_branch {
                    if has_terminator {
                        break; // Don't generate unreachable code
                    }
                    ir.push_str(&self.generate_statement(stmt)?);
                    if Self::is_terminator(stmt) {
                        has_terminator = true;
                    }
                }
                // Only generate branch to end if the then branch doesn't have a terminator
                if !has_terminator {
                    ir.push_str(&format!("  br label %{}\n", end_label));
                }

                // Else branch
                let mut else_has_terminator = false;
                if let Some(else_stmts) = else_branch {
                    ir.push_str(&format!("{}:\n", else_label));
                    for stmt in else_stmts {
                        if else_has_terminator {
                            break; // Don't generate unreachable code
                        }
                        ir.push_str(&self.generate_statement(stmt)?);
                        if Self::is_terminator(stmt) {
                            else_has_terminator = true;
                        }
                    }
                    // Only generate branch to end if the else branch doesn't have a terminator
                    if !else_has_terminator {
                        ir.push_str(&format!("  br label %{}\n", end_label));
                    }
                }

                // End label - always generate if we reference it
                // We reference it when:
                // 1. No else branch (we branch to it on false condition)
                // 2. Any branch that doesn't have a terminator branches to it
                let need_end_label = else_branch.is_none() || !has_terminator || !else_has_terminator;
                if need_end_label {
                    ir.push_str(&format!("{}:\n", end_label));
                }
            }

            Stmt::While {
                condition, body, ..
            } => {
                let cond_label = self.fresh_label("while_cond");
                let body_label = self.fresh_label("while_body");
                let end_label = self.fresh_label("while_end");

                // Jump to condition check
                ir.push_str(&format!("  br label %{}\n", cond_label));

                // Condition label
                ir.push_str(&format!("{}:\n", cond_label));
                let (cond_ir, cond_result) = self.generate_expression(condition)?;
                ir.push_str(&cond_ir);
                ir.push_str(&format!(
                    "  br i1 {}, label %{}, label %{}\n",
                    cond_result, body_label, end_label
                ));

                // Body label
                ir.push_str(&format!("{}:\n", body_label));
                let mut body_has_terminator = false;
                for stmt in body {
                    if body_has_terminator {
                        break; // Don't generate unreachable code
                    }
                    ir.push_str(&self.generate_statement(stmt)?);
                    if Self::is_terminator(stmt) {
                        body_has_terminator = true;
                    }
                }
                // Only generate branch back to condition if body doesn't have a terminator
                if !body_has_terminator {
                    ir.push_str(&format!("  br label %{}\n", cond_label));
                }

                // End label
                ir.push_str(&format!("{}:\n", end_label));
            }

            Stmt::For {
                var, iter, body, ..
            } => {
                match iter {
                    // Handle range iteration
                    Expr::Range { start, end, .. } => {
                        let (start_ir, start_val) = self.generate_expression(start)?;
                        let (end_ir, end_val) = self.generate_expression(end)?;
                        ir.push_str(&start_ir);
                        ir.push_str(&end_ir);

                        // Allocate loop variable
                        let loop_var_ptr = self.fresh_ssa();
                        ir.push_str(&format!("  {} = alloca i64\n", loop_var_ptr));
                        ir.push_str(&format!(
                            "  store i64 {}, i64* {}\n",
                            start_val, loop_var_ptr
                        ));

                        self.var_map.insert(
                            var.clone(),
                            VarInfo {
                                ptr: loop_var_ptr.clone(),
                                ty: "i64".to_string(),
                                is_param: false,
                            },
                        );

                        let cond_label = self.fresh_label("for_cond");
                        let body_label = self.fresh_label("for_body");
                        let inc_label = self.fresh_label("for_inc");
                        let end_label = self.fresh_label("for_end");

                        // Jump to condition
                        ir.push_str(&format!("  br label %{}\n", cond_label));

                        // Condition: check if i < end
                        ir.push_str(&format!("{}:\n", cond_label));
                        let i_val = self.fresh_ssa();
                        ir.push_str(&format!("  {} = load i64, i64* {}\n", i_val, loop_var_ptr));
                        let cmp = self.fresh_ssa();
                        ir.push_str(&format!(
                            "  {} = icmp slt i64 {}, {}\n",
                            cmp, i_val, end_val
                        ));
                        ir.push_str(&format!(
                            "  br i1 {}, label %{}, label %{}\n",
                            cmp, body_label, end_label
                        ));

                        // Body
                        ir.push_str(&format!("{}:\n", body_label));
                        let mut body_has_terminator = false;
                        for stmt in body {
                            if body_has_terminator {
                                break; // Don't generate unreachable code
                            }
                            ir.push_str(&self.generate_statement(stmt)?);
                            if Self::is_terminator(stmt) {
                                body_has_terminator = true;
                            }
                        }
                        // Only branch to increment if body doesn't have a terminator
                        if !body_has_terminator {
                            ir.push_str(&format!("  br label %{}\n", inc_label));
                        }

                        // Increment
                        ir.push_str(&format!("{}:\n", inc_label));
                        let curr_val = self.fresh_ssa();
                        let next_val = self.fresh_ssa();
                        ir.push_str(&format!(
                            "  {} = load i64, i64* {}\n",
                            curr_val, loop_var_ptr
                        ));
                        ir.push_str(&format!("  {} = add i64 {}, 1\n", next_val, curr_val));
                        ir.push_str(&format!(
                            "  store i64 {}, i64* {}\n",
                            next_val, loop_var_ptr
                        ));
                        ir.push_str(&format!("  br label %{}\n", cond_label));

                        // End
                        ir.push_str(&format!("{}:\n", end_label));
                    }
                    
                    // Handle array iteration
                    Expr::Ident(array_name) => {
                        if let Some(var_info) = self.var_map.get(array_name).cloned() {
                            // Check if it's an array type
                            if var_info.ty.starts_with('[') && var_info.ty.contains(" x ") {
                                // Extract array size and element type
                                let array_type = &var_info.ty;
                                let size_end = array_type.find(" x ").unwrap();
                                let size: usize = array_type[1..size_end].parse().unwrap_or(0);
                                let elem_type = array_type[size_end + 3..array_type.len() - 1].to_string();
                                
                                // Allocate index variable
                                let idx_ptr = self.fresh_ssa();
                                ir.push_str(&format!("  {} = alloca i64\n", idx_ptr));
                                ir.push_str(&format!("  store i64 0, i64* {}\n", idx_ptr));
                                
                                // Allocate loop variable
                                let loop_var_ptr = self.fresh_ssa();
                                ir.push_str(&format!("  {} = alloca {}\n", loop_var_ptr, elem_type));
                                
                                self.var_map.insert(
                                    var.clone(),
                                    VarInfo {
                                        ptr: loop_var_ptr.clone(),
                                        ty: elem_type.clone(),
                                        is_param: false,
                                    },
                                );
                                
                                let cond_label = self.fresh_label("for_cond");
                                let body_label = self.fresh_label("for_body");
                                let inc_label = self.fresh_label("for_inc");
                                let end_label = self.fresh_label("for_end");
                                
                                // Jump to condition
                                ir.push_str(&format!("  br label %{}\n", cond_label));
                                
                                // Condition: check if idx < size
                                ir.push_str(&format!("{}:\n", cond_label));
                                let idx_val = self.fresh_ssa();
                                ir.push_str(&format!("  {} = load i64, i64* {}\n", idx_val, idx_ptr));
                                let cmp = self.fresh_ssa();
                                ir.push_str(&format!("  {} = icmp slt i64 {}, {}\n", cmp, idx_val, size));
                                ir.push_str(&format!(
                                    "  br i1 {}, label %{}, label %{}\n",
                                    cmp, body_label, end_label
                                ));
                                
                                // Body: load array element into loop variable
                                ir.push_str(&format!("{}:\n", body_label));
                                let elem_ptr = self.fresh_ssa();
                                let elem_val = self.fresh_ssa();
                                ir.push_str(&format!(
                                    "  {} = getelementptr {}, {}* {}, i64 0, i64 {}\n",
                                    elem_ptr, array_type, array_type, var_info.ptr, idx_val
                                ));
                                ir.push_str(&format!(
                                    "  {} = load {}, {}* {}\n",
                                    elem_val, elem_type, elem_type, elem_ptr
                                ));
                                ir.push_str(&format!(
                                    "  store {} {}, {}* {}\n",
                                    elem_type, elem_val, elem_type, loop_var_ptr
                                ));
                                
                                // Execute loop body
                                let mut body_has_terminator = false;
                                for stmt in body {
                                    if body_has_terminator {
                                        break; // Don't generate unreachable code
                                    }
                                    ir.push_str(&self.generate_statement(stmt)?);
                                    if Self::is_terminator(stmt) {
                                        body_has_terminator = true;
                                    }
                                }
                                // Only branch to increment if body doesn't have a terminator
                                if !body_has_terminator {
                                    ir.push_str(&format!("  br label %{}\n", inc_label));
                                }
                                
                                // Increment index
                                ir.push_str(&format!("{}:\n", inc_label));
                                let curr_idx = self.fresh_ssa();
                                let next_idx = self.fresh_ssa();
                                ir.push_str(&format!("  {} = load i64, i64* {}\n", curr_idx, idx_ptr));
                                ir.push_str(&format!("  {} = add i64 {}, 1\n", next_idx, curr_idx));
                                ir.push_str(&format!("  store i64 {}, i64* {}\n", next_idx, idx_ptr));
                                ir.push_str(&format!("  br label %{}\n", cond_label));
                                
                                // End
                                ir.push_str(&format!("{}:\n", end_label));
                            } else {
                                return Err(CompileError::Generic(format!(
                                    "Cannot iterate over non-array type: {}",
                                    var_info.ty
                                )));
                            }
                        } else {
                            return Err(CompileError::Generic(format!(
                                "Undefined variable: {}",
                                array_name
                            )));
                        }
                    }
                    
                    // Handle array literal iteration
                    Expr::ArrayLiteral { elements, .. } => {
                        // First, generate a temporary array
                        let elem_type = "i64"; // For now, assume i64 arrays
                        let array_type = format!("[{} x {}]", elements.len(), elem_type);
                        let temp_array = self.fresh_ssa();
                        
                        ir.push_str(&format!("  {} = alloca {}\n", temp_array, array_type));
                        
                        // Initialize array elements
                        for (i, elem) in elements.iter().enumerate() {
                            let (elem_ir, elem_val) = self.generate_expression(elem)?;
                            ir.push_str(&elem_ir);
                            let ptr = self.fresh_ssa();
                            ir.push_str(&format!(
                                "  {} = getelementptr {}, {}* {}, i64 0, i64 {}\n",
                                ptr, array_type, array_type, temp_array, i
                            ));
                            ir.push_str(&format!(
                                "  store {} {}, {}* {}\n",
                                elem_type, elem_val, elem_type, ptr
                            ));
                        }
                        
                        // Now iterate over the temporary array
                        // Similar code to array iteration above...
                        let idx_ptr = self.fresh_ssa();
                        ir.push_str(&format!("  {} = alloca i64\n", idx_ptr));
                        ir.push_str(&format!("  store i64 0, i64* {}\n", idx_ptr));
                        
                        let loop_var_ptr = self.fresh_ssa();
                        ir.push_str(&format!("  {} = alloca {}\n", loop_var_ptr, elem_type));
                        
                        self.var_map.insert(
                            var.clone(),
                            VarInfo {
                                ptr: loop_var_ptr.clone(),
                                ty: elem_type.to_string(),
                                is_param: false,
                            },
                        );
                        
                        let cond_label = self.fresh_label("for_cond");
                        let body_label = self.fresh_label("for_body");
                        let inc_label = self.fresh_label("for_inc");
                        let end_label = self.fresh_label("for_end");
                        
                        ir.push_str(&format!("  br label %{}\n", cond_label));
                        
                        ir.push_str(&format!("{}:\n", cond_label));
                        let idx_val = self.fresh_ssa();
                        ir.push_str(&format!("  {} = load i64, i64* {}\n", idx_val, idx_ptr));
                        let cmp = self.fresh_ssa();
                        ir.push_str(&format!("  {} = icmp slt i64 {}, {}\n", cmp, idx_val, elements.len()));
                        ir.push_str(&format!(
                            "  br i1 {}, label %{}, label %{}\n",
                            cmp, body_label, end_label
                        ));
                        
                        ir.push_str(&format!("{}:\n", body_label));
                        let elem_ptr = self.fresh_ssa();
                        let elem_val = self.fresh_ssa();
                        ir.push_str(&format!(
                            "  {} = getelementptr {}, {}* {}, i64 0, i64 {}\n",
                            elem_ptr, array_type, array_type, temp_array, idx_val
                        ));
                        ir.push_str(&format!(
                            "  {} = load {}, {}* {}\n",
                            elem_val, elem_type, elem_type, elem_ptr
                        ));
                        ir.push_str(&format!(
                            "  store {} {}, {}* {}\n",
                            elem_type, elem_val, elem_type, loop_var_ptr
                        ));
                        
                        let mut body_has_terminator = false;
                        for stmt in body {
                            if body_has_terminator {
                                break; // Don't generate unreachable code
                            }
                            ir.push_str(&self.generate_statement(stmt)?);
                            if Self::is_terminator(stmt) {
                                body_has_terminator = true;
                            }
                        }
                        // Only branch to increment if body doesn't have a terminator
                        if !body_has_terminator {
                            ir.push_str(&format!("  br label %{}\n", inc_label));
                        }
                        
                        ir.push_str(&format!("{}:\n", inc_label));
                        let curr_idx = self.fresh_ssa();
                        let next_idx = self.fresh_ssa();
                        ir.push_str(&format!("  {} = load i64, i64* {}\n", curr_idx, idx_ptr));
                        ir.push_str(&format!("  {} = add i64 {}, 1\n", next_idx, curr_idx));
                        ir.push_str(&format!("  store i64 {}, i64* {}\n", next_idx, idx_ptr));
                        ir.push_str(&format!("  br label %{}\n", cond_label));
                        
                        ir.push_str(&format!("{}:\n", end_label));
                    }
                    
                    _ => {
                        return Err(CompileError::Generic(
                            "Unsupported iterator type in for loop".to_string(),
                        ));
                    }
                }
            }

            Stmt::Assign { target, value, .. } => {
                let (value_ir, value_var, value_type) = self.generate_expression_typed(value)?;
                ir.push_str(&value_ir);

                match target {
                    AssignTarget::Ident(name) => {
                        if let Some(var_info) = self.var_map.get(name).cloned() {
                            ir.push_str(&format!(
                                "  store {} {}, {}* {}\n",
                                value_type, value_var, var_info.ty, var_info.ptr
                            ));
                        } else {
                            return Err(CompileError::Generic(format!(
                                "Undefined variable: {}",
                                name
                            )));
                        }
                    }
                    AssignTarget::Index { array, index } => {
                        let (index_ir, index_var) = self.generate_expression(index)?;
                        ir.push_str(&index_ir);

                        if let Expr::Ident(array_name) = array.as_ref() {
                            if let Some(var_info) = self.var_map.get(array_name).cloned() {
                                let ptr = self.fresh_ssa();
                                // Extract array size from type string
                                if let Some(array_type) = var_info
                                    .ty
                                    .strip_prefix('[')
                                    .and_then(|s| s.find(" x "))
                                    .map(|i| &var_info.ty[1..i])
                                {
                                    let _size: usize = array_type.parse().unwrap_or(5);
                                    ir.push_str(&format!(
                                        "  {} = getelementptr {}, {}* {}, i64 0, i64 {}\n",
                                        ptr, var_info.ty, var_info.ty, var_info.ptr, index_var
                                    ));
                                    ir.push_str(&format!(
                                        "  store {} {}, {}* {}\n",
                                        value_type, value_var, value_type, ptr
                                    ));
                                }
                            }
                        }
                    }
                    AssignTarget::FieldAccess { object, field: _ } => {
                        // Generate code to get the field pointer
                        let (obj_ir, obj_var) = self.generate_expression(object)?;
                        ir.push_str(&obj_ir);
                        
                        // TODO: Look up struct definition to find field index
                        let field_idx = 0; // Placeholder
                        let field_ptr = self.fresh_ssa();
                        
                        // Infer struct type
                        let struct_type = if let Expr::Ident(name) = object.as_ref() {
                            if let Some(var_info) = self.var_map.get(name) {
                                var_info.ty.clone()
                            } else {
                                "%struct.Unknown".to_string()
                            }
                        } else {
                            "%struct.Unknown".to_string()
                        };
                        
                        ir.push_str(&format!(
                            "  {} = getelementptr {}, {}* {}, i32 0, i32 {}\n",
                            field_ptr, struct_type, struct_type, obj_var, field_idx
                        ));
                        ir.push_str(&format!(
                            "  store {} {}, {}* {}\n",
                            value_type, value_var, value_type, field_ptr
                        ));
                    }
                    AssignTarget::Deref { expr } => {
                        let (ptr_ir, ptr_var) = self.generate_expression(expr)?;
                        ir.push_str(&ptr_ir);
                        ir.push_str(&format!(
                            "  store {} {}, {}* {}\n",
                            value_type, value_var, value_type, ptr_var
                        ));
                    }
                }
            }

            Stmt::Break { .. } => {
                // Generate jump to the end of the current loop
                // For now, we'll use a placeholder since we need loop context
                ir.push_str("  ; TODO: Implement break with proper loop context\n");
                ir.push_str("  br label %loop_end_placeholder\n");
            }
            
            Stmt::Continue { .. } => {
                // Generate jump to the increment part of the current loop
                // For now, we'll use a placeholder since we need loop context
                ir.push_str("  ; TODO: Implement continue with proper loop context\n");
                ir.push_str("  br label %loop_inc_placeholder\n");
            }
            
            Stmt::Match { expr, arms, .. } => {
                // Generate switch-like control flow for match
                let (expr_ir, _expr_var) = self.generate_expression(expr)?;
                ir.push_str(&expr_ir);
                
                let end_label = self.fresh_label("match_end");
                
                // For now, generate a simple if-else chain
                // TODO: Implement proper pattern matching
                for (i, arm) in arms.iter().enumerate() {
                    let arm_label = self.fresh_label(&format!("match_arm{}", i));
                    let next_label = if i + 1 < arms.len() {
                        self.fresh_label(&format!("match_arm{}", i + 1))
                    } else {
                        end_label.clone()
                    };
                    
                    // Simple pattern matching for now
                    match &arm.pattern {
                        Pattern::Wildcard => {
                            // Always matches
                            ir.push_str(&format!("  br label %{}\n", arm_label));
                        }
                        Pattern::Ident(_) => {
                            // Bind the value and match
                            ir.push_str(&format!("  br label %{}\n", arm_label));
                        }
                        _ => {
                            // TODO: Implement enum pattern matching
                            ir.push_str("  ; TODO: Complex pattern matching\n");
                            ir.push_str(&format!("  br label %{}\n", next_label));
                        }
                    }
                    
                    ir.push_str(&format!("{}:\n", arm_label));
                    let mut arm_has_terminator = false;
                    for stmt in &arm.body {
                        if arm_has_terminator {
                            break; // Don't generate unreachable code
                        }
                        ir.push_str(&self.generate_statement(stmt)?);
                        if Self::is_terminator(stmt) {
                            arm_has_terminator = true;
                        }
                    }
                    // Only branch to end if arm doesn't have a terminator
                    if !arm_has_terminator {
                        ir.push_str(&format!("  br label %{}\n", end_label));
                    }
                }
                
                // Only generate end label if at least one arm can reach it
                let any_arm_can_reach_end = arms.iter().any(|arm| !Self::has_terminator(&arm.body));
                if any_arm_can_reach_end {
                    ir.push_str(&format!("{}:\n", end_label));
                }
            }
            
            Stmt::Unsafe { body, .. } => {
                // In LLVM IR, there's no explicit unsafe block
                // Just generate the body statements
                ir.push_str("  ; Unsafe block\n");
                for stmt in body {
                    ir.push_str(&self.generate_statement(stmt)?);
                }
            }
        }

        Ok(ir)
    }

    /// Generate LLVM IR for an expression, returning (IR code, result value, result type)
    fn generate_expression_typed(&mut self, expr: &Expr) -> Result<(String, String, String)> {
        let (ir, val) = self.generate_expression(expr)?;
        let ty = self.infer_expr_type(expr);
        Ok((ir, val, ty))
    }

    /// Generate LLVM IR for an expression
    /// Returns (IR code, result variable/value)
    fn generate_expression(&mut self, expr: &Expr) -> Result<(String, String)> {
        let mut ir = String::new();

        match expr {
            Expr::Integer(n) => Ok((String::new(), n.to_string())),

            Expr::Bool(b) => Ok((String::new(), if *b { "1" } else { "0" }.to_string())),

            Expr::String(s) => {
                // Find the pre-collected string constant
                let const_name = self
                    .string_constants
                    .iter()
                    .find(|(_, v)| v == s)
                    .map(|(n, _)| n.clone())
                    .unwrap_or_else(|| "@.str.unknown".to_string());

                let ptr_var = self.fresh_ssa();
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
                if let Some(var_info) = self.var_map.get(name).cloned() {
                    // Check if this is an array type
                    if var_info.ty.starts_with('[') && var_info.ty.ends_with(']') {
                        // For arrays, return the pointer directly - don't load
                        Ok((ir, var_info.ptr))
                    } else {
                        let load_var = self.fresh_ssa();
                        ir.push_str(&format!(
                            "  {} = load {}, {}* {}\n",
                            load_var, var_info.ty, var_info.ty, var_info.ptr
                        ));
                        Ok((ir, load_var))
                    }
                } else {
                    Err(CompileError::UndefinedVariable {
                        name: name.clone(),
                        span: None,
                    })
                }
            }

            Expr::Binary {
                left, op, right, ..
            } => {
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
                    _ => {
                        return Err(CompileError::Generic(
                            "Unsupported binary operator".to_string(),
                        ))
                    }
                };

                // Determine operand type (for now, assume i64 for arithmetic)
                // All operations work on i64 for now
                let op_type = "i64";

                ir.push_str(&format!(
                    "  {} = {} {} {}, {}\n",
                    result_var, op_str, op_type, left_var, right_var
                ));

                Ok((ir, result_var))
            }

            Expr::ArrayLiteral { elements, .. } => {
                // Generate array literal
                let elem_type = "i64"; // For now, assume i64 arrays
                let array_type = format!("[{} x {}]", elements.len(), elem_type);

                // Allocate array on stack
                let array_var = self.fresh_ssa();
                ir.push_str(&format!("  {} = alloca {}\n", array_var, array_type));

                // Initialize elements
                for (i, elem) in elements.iter().enumerate() {
                    let (elem_ir, elem_val) = self.generate_expression(elem)?;
                    ir.push_str(&elem_ir);

                    let ptr = self.fresh_ssa();
                    ir.push_str(&format!(
                        "  {} = getelementptr {}, {}* {}, i64 0, i64 {}\n",
                        ptr, array_type, array_type, array_var, i
                    ));
                    ir.push_str(&format!(
                        "  store {} {}, {}* {}\n",
                        elem_type, elem_val, elem_type, ptr
                    ));
                }

                Ok((ir, array_var))
            }

            Expr::Index { array, index, .. } => {
                let (idx_ir, idx_var) = self.generate_expression(index)?;
                ir.push_str(&idx_ir);

                // Handle array expression - could be an identifier or other expression
                if let Expr::Ident(name) = array.as_ref() {
                    if let Some(var_info) = self.var_map.get(name).cloned() {
                        let ptr = self.fresh_ssa();
                        let val = self.fresh_ssa();

                        // Use the actual array type from var_info
                        ir.push_str(&format!(
                            "  {} = getelementptr {}, {}* {}, i64 0, i64 {}\n",
                            ptr, var_info.ty, var_info.ty, var_info.ptr, idx_var
                        ));
                        ir.push_str(&format!("  {} = load i64, i64* {}\n", val, ptr));

                        Ok((ir, val))
                    } else {
                        Err(CompileError::Generic(format!("Undefined array: {}", name)))
                    }
                } else {
                    // For other array expressions, we need to evaluate them
                    let (array_ir, array_var) = self.generate_expression(array)?;
                    ir.push_str(&array_ir);

                    let ptr = self.fresh_ssa();
                    let val = self.fresh_ssa();

                    // TODO: Properly infer array type
                    ir.push_str(&format!(
                        "  {} = getelementptr [5 x i64], [5 x i64]* {}, i64 0, i64 {}\n",
                        ptr, array_var, idx_var
                    ));
                    ir.push_str(&format!("  {} = load i64, i64* {}\n", val, ptr));

                    Ok((ir, val))
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
                            let mut arg_types = Vec::new();

                            for arg in args {
                                let (arg_ir, arg_var, arg_type) =
                                    self.generate_expression_typed(arg)?;
                                ir.push_str(&arg_ir);
                                arg_vars.push(arg_var);
                                arg_types.push(arg_type);
                            }

                            let result_var = self.fresh_ssa();

                            // TODO: Look up actual function return type
                            let ret_type = "i64"; // Default to i64

                            ir.push_str(&format!(
                                "  {} = call {} @{}(",
                                result_var, ret_type, func_name
                            ));
                            for (i, (arg_var, arg_type)) in
                                arg_vars.iter().zip(arg_types.iter()).enumerate()
                            {
                                if i > 0 {
                                    ir.push_str(", ");
                                }
                                ir.push_str(&format!("{} {}", arg_type, arg_var));
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

            Expr::Range { start, end, .. } => {
                // Ranges are handled specially in for loops
                let (start_ir, start_val) = self.generate_expression(start)?;
                let (end_ir, end_val) = self.generate_expression(end)?;
                ir.push_str(&start_ir);
                ir.push_str(&end_ir);
                Ok((ir, format!("range({}, {})", start_val, end_val)))
            }

            Expr::StructLiteral { name, fields, .. } => {
                // Allocate struct on stack
                let struct_type = format!("%struct.{}", name);
                let struct_var = self.fresh_ssa();
                ir.push_str(&format!("  {} = alloca {}\n", struct_var, struct_type));
                
                // Initialize fields
                for (i, (_field_name, field_expr)) in fields.iter().enumerate() {
                    let (field_ir, field_val) = self.generate_expression(field_expr)?;
                    ir.push_str(&field_ir);
                    
                    let field_ptr = self.fresh_ssa();
                    ir.push_str(&format!(
                        "  {} = getelementptr {}, {}* {}, i32 0, i32 {}\n",
                        field_ptr, struct_type, struct_type, struct_var, i
                    ));
                    
                    let field_type = self.infer_expr_type(field_expr);
                    ir.push_str(&format!(
                        "  store {} {}, {}* {}\n",
                        field_type, field_val, field_type, field_ptr
                    ));
                }
                
                Ok((ir, struct_var))
            }
            
            Expr::FieldAccess { object, field: _, .. } => {
                let (obj_ir, obj_var) = self.generate_expression(object)?;
                ir.push_str(&obj_ir);
                
                // TODO: Look up struct definition to find field index
                let field_idx = 0; // Placeholder
                let field_ptr = self.fresh_ssa();
                let field_val = self.fresh_ssa();
                
                // Infer struct type from object expression
                let struct_type = if let Expr::Ident(name) = object.as_ref() {
                    if let Some(var_info) = self.var_map.get(name) {
                        var_info.ty.clone()
                    } else {
                        "%struct.Unknown".to_string()
                    }
                } else {
                    "%struct.Unknown".to_string()
                };
                
                ir.push_str(&format!(
                    "  {} = getelementptr {}, {}* {}, i32 0, i32 {}\n",
                    field_ptr, struct_type, struct_type, obj_var, field_idx
                ));
                ir.push_str(&format!("  {} = load i64, i64* {}\n", field_val, field_ptr));
                
                Ok((ir, field_val))
            }
            
            Expr::ArrayRepeat { value, count, .. } => {
                let (count_ir, _count_val) = self.generate_expression(count)?;
                ir.push_str(&count_ir);
                
                // For compile-time constant arrays
                if let Expr::Integer(n) = count.as_ref() {
                    let elem_type = "i64";
                    let array_type = format!("[{} x {}]", n, elem_type);
                    let array_var = self.fresh_ssa();
                    
                    ir.push_str(&format!("  {} = alloca {}\n", array_var, array_type));
                    
                    // Generate value once
                    let (val_ir, val_var) = self.generate_expression(value)?;
                    ir.push_str(&val_ir);
                    
                    // Initialize all elements
                    for i in 0..*n {
                        let ptr = self.fresh_ssa();
                        ir.push_str(&format!(
                            "  {} = getelementptr {}, {}* {}, i64 0, i64 {}\n",
                            ptr, array_type, array_type, array_var, i
                        ));
                        ir.push_str(&format!(
                            "  store {} {}, {}* {}\n",
                            elem_type, val_var, elem_type, ptr
                        ));
                    }
                    
                    Ok((ir, array_var))
                } else {
                    Err(CompileError::Generic(
                        "Dynamic array repeat not yet supported".to_string()
                    ))
                }
            }
            
            Expr::Unary { op, operand, .. } => {
                let (op_ir, op_var) = self.generate_expression(operand)?;
                ir.push_str(&op_ir);
                
                let result_var = self.fresh_ssa();
                
                match op {
                    UnaryOp::Neg => {
                        ir.push_str(&format!(
                            "  {} = sub i64 0, {}\n",
                            result_var, op_var
                        ));
                    }
                    UnaryOp::Not => {
                        ir.push_str(&format!(
                            "  {} = xor i1 {}, true\n",
                            result_var, op_var
                        ));
                    }
                }
                
                Ok((ir, result_var))
            }
            
            Expr::Reference { mutable: _, expr, .. } => {
                // For now, just return the address of the expression
                if let Expr::Ident(name) = expr.as_ref() {
                    if let Some(var_info) = self.var_map.get(name) {
                        Ok((String::new(), var_info.ptr.clone()))
                    } else {
                        Err(CompileError::Generic(format!("Undefined variable: {}", name)))
                    }
                } else {
                    Err(CompileError::Generic(
                        "Complex reference expressions not yet supported".to_string()
                    ))
                }
            }
            
            Expr::Deref { expr, .. } => {
                let (expr_ir, expr_var) = self.generate_expression(expr)?;
                ir.push_str(&expr_ir);
                
                let result_var = self.fresh_ssa();
                ir.push_str(&format!("  {} = load i64, i64* {}\n", result_var, expr_var));
                
                Ok((ir, result_var))
            }
            
            _ => {
                // TODO: Implement EnumConstructor, Question, MacroInvocation, Await
                Ok((String::new(), "0".to_string()))
            }
        }
    }

    /// Infer the LLVM type of an expression
    fn infer_expr_type(&self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(_) => "i64".to_string(),
            Expr::Bool(_) => "i1".to_string(),
            Expr::String(_) => "i8*".to_string(),
            Expr::ArrayLiteral { elements, .. } => {
                format!("[{} x i64]", elements.len())
            }
            Expr::ArrayRepeat { count, .. } => {
                if let Expr::Integer(n) = count.as_ref() {
                    format!("[{} x i64]", n)
                } else {
                    "i64".to_string() // Default for dynamic arrays
                }
            }
            Expr::Binary { op, .. } => {
                if matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne) {
                    "i1".to_string()
                } else {
                    "i64".to_string()
                }
            }
            Expr::Call { .. } => "i64".to_string(), // Default
            Expr::Ident(name) => {
                if let Some(var_info) = self.var_map.get(name) {
                    var_info.ty.clone()
                } else {
                    "i64".to_string() // Default
                }
            }
            _ => "i64".to_string(), // Default
        }
    }

    /// Check if a statement is a terminator (return, break, etc.)
    fn is_terminator(stmt: &Stmt) -> bool {
        matches!(stmt, Stmt::Return(_) | Stmt::Break { .. } | Stmt::Continue { .. })
    }

    /// Check if a list of statements ends with a terminator
    fn has_terminator(stmts: &[Stmt]) -> bool {
        stmts.last().is_some_and(Self::is_terminator)
    }

    /// Write the generated LLVM IR to a file
    pub fn write_output(&self, ir: &str) -> Result<PathBuf> {
        let build_dir = PathBuf::from("build_output");
        if !build_dir.exists() {
            std::fs::create_dir_all(&build_dir)?;
        }

        let output_path = build_dir.join(format!("{}.ll", self.module_name));
        std::fs::write(&output_path, ir)?;

        Ok(output_path)
    }
}
