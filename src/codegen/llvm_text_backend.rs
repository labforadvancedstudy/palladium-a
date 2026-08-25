// Text-based LLVM IR backend for Palladium
// "Native code generation without dependencies"

use crate::ast::{ArraySize, AssignTarget, BinOp, Expr, Function, Item, Pattern, Program, Stmt, Type, UnaryOp};
use crate::errors::{CompileError, Result, Span};
use std::collections::HashMap;
use std::path::PathBuf;

/// Whether `LLVMTextBackend::compile` refuses before doing any work.
///
/// A constant rather than a `cfg` or an option, because there is no
/// configuration in which this backend is correct today and offering one would
/// be the same lie in a switch. Flipping it to `false` restores the lowering
/// wholesale, and the granular refusals at the bottom of this file become the
/// live behaviour again — which is the point of keeping them.
const BACKEND_REFUSES: bool = true;

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

    /// Compile a program to LLVM IR.
    ///
    /// Refuses unconditionally. See [`unimplemented_backend`] for why the gate
    /// is here and not spread across the individual gaps.
    pub fn compile(&mut self, program: &Program) -> Result<String> {
        if BACKEND_REFUSES {
            return Err(unimplemented_backend());
        }
        self.compile_unchecked(program)
    }

    /// The lowering itself, with the backend gate lifted.
    ///
    /// Private, and reached only by this file's own tests. It exists so that
    /// the granular refusals below stay executable — they are the record of
    /// *what* is unimplemented, and they become the live behaviour again the
    /// moment `compile` stops refusing.
    fn compile_unchecked(&mut self, program: &Program) -> Result<String> {
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
                Stmt::Loop { body, .. } => self.collect_strings_from_stmts(body),
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

            // `loop` is refused rather than lowered as `while true`: the only
            // way out of it is a `break`, and `break` is refused one arm down,
            // so the lowering would be an endless loop with no exit edge.
            Stmt::Loop { span, .. } => {
                return Err(unimplemented_loop_jump("loop", "loop_end", *span));
            }

            Stmt::Break { span, .. } => {
                return Err(unimplemented_loop_jump("break", "loop_end", *span));
            }

            Stmt::Continue { span } => {
                return Err(unimplemented_loop_jump("continue", "loop_inc", *span));
            }

            Stmt::Match {
                expr,
                arms,
                span: match_span,
            } => {
                // Generate switch-like control flow for match
                let (expr_ir, _expr_var) = self.generate_expression(expr)?;
                ir.push_str(&expr_ir);
                
                let end_label = self.fresh_label("match_end");
                
                // For now, generate a simple if-else chain
                // TODO: Implement proper pattern matching
                for (i, arm) in arms.iter().enumerate() {
                    let arm_label = self.fresh_label(&format!("match_arm{}", i));
                    // Only the enum-pattern arm ever branched here, and that
                    // arm now refuses, so nothing reads this. The label is
                    // still allocated because `fresh_label` bumps the shared
                    // counter, and dropping the call would renumber every
                    // label in every other `match` this backend emits.
                    let _next_label = if i + 1 < arms.len() {
                        self.fresh_label(&format!("match_arm{}", i + 1))
                    } else {
                        end_label.clone()
                    };

                    // Simple pattern matching for now. Exhaustive on purpose:
                    // a new `Pattern` variant must be decided on here, not
                    // swallowed by a wildcard.
                    match &arm.pattern {
                        Pattern::Wildcard => {
                            // Always matches
                            ir.push_str(&format!("  br label %{}\n", arm_label));
                        }
                        Pattern::Ident(_) => {
                            // Bind the value and match
                            ir.push_str(&format!("  br label %{}\n", arm_label));
                        }
                        Pattern::EnumPattern {
                            enum_name, variant, ..
                        } => {
                            return Err(unimplemented_enum_pattern(
                                enum_name,
                                variant,
                                *match_span,
                            ));
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

            // The LLVM backends are refused wholesale at the driver (`--llvm`),
            // so these arms exist to keep the match exhaustive and are not a
            // claim that float or char lowering works here.
            Expr::Float(x) => Ok((String::new(), format!("{:?}", x))),

            Expr::Char(c) => Ok((String::new(), (*c as u32).to_string())),

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
                    UnaryOp::BitNot => {
                        // `~x` is `x ^ -1` in two's complement, which is what
                        // LLVM's canonical form for it is too.
                        ir.push_str(&format!("  {} = xor i64 {}, -1\n", result_var, op_var));
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
            
            // The four expression kinds this backend cannot lower. They are
            // listed one by one rather than swept into a `_` arm: with no
            // wildcard here, adding a variant to `Expr` stops compiling until
            // somebody decides what this backend does with it. A wildcard is
            // what let these four silently become `0` in the first place.
            Expr::EnumConstructor {
                enum_name,
                variant,
                span,
                ..
            } => Err(unimplemented_enum_constructor(enum_name, variant, *span)),

            Expr::Question { span, .. } => Err(unimplemented_question(*span)),

            Expr::MacroInvocation { name, span, .. } => {
                Err(unimplemented_macro_invocation(name, *span))
            }

            Expr::Await { span, .. } => Err(unimplemented_await(*span)),

            // The LLVM text backend is refused wholesale at the driver, so
            // these arms exist to keep the match exhaustive. They are NOT a
            // lowering: an honest one needs basic blocks and a phi, and
            // claiming one here would be a second backend making promises the
            // first has to keep.
            Expr::If { span, .. }
            | Expr::Block { span, .. }
            | Expr::Loop { span, .. }
            | Expr::Match { span, .. } => Err(unimplemented_value_block(*span)),

            // No numeric conversion lowering here (`sext`/`trunc`/`fptosi`/…),
            // and inventing one would be this backend making a promise the
            // supported one has to keep.
            Expr::Cast { span, .. } => Err(CompileError::Unimplemented {
                construct: "an `as` cast".to_string(),
                consequence: "this backend has no numeric conversion lowering, so the operand \
                     would be used at its original width and sign"
                    .to_string(),
                workaround: "compile without `--llvm`, which is the supported backend and emits \
                     the C cast"
                    .to_string(),
                span: Some(*span),
            }),
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

// ---------------------------------------------------------------------------
// Refusals
//
// These live in this file rather than next to the shared constructors in
// `src/errors/mod.rs` because each one describes what *this* backend would
// otherwise emit — a claim that can only be checked against the code above —
// and because they are used nowhere else. The error variant itself is the
// shared `CompileError::Unimplemented`, so there is still only one way for the
// compiler to say "not implemented".
//
// Two rules hold for every message below:
//   * the `consequence` describes IR that was actually observed, not IR that
//     was imagined;
//   * the `workaround` is compiled and run by the `*_help_*` tests in this
//     file's `mod tests`, which read the suggestion the diagnostic actually
//     carries rather than a copy of it in a comment, and only then execute the
//     rewrite. A suggestion nobody has executed is a claim, and this milestone
//     is about the compiler not making claims it cannot back.
// ---------------------------------------------------------------------------

/// The whole backend refuses, before it looks at the program.
///
/// The granular refusals below cover seven constructs that fail *loudly*. They
/// are not the whole problem. Auditing the rest of this file turned up seven
/// more sites that fabricate rather than refuse, and say nothing about it:
///
///   * `compile` skips every non-function `Item`, so struct and enum
///     definitions are dropped while expressions still refer to them.
///   * `type_to_llvm` maps every type it does not enumerate — custom, generic,
///     tuple, reference, `Future` — to `i8*`.
///   * every user-function call is emitted as returning `i64`, whatever the
///     declared signature says, and `infer_expr_type` agrees with it.
///   * field access hard-codes index 0, for reads and for assignment alike.
///     Measured: `struct Point { x, y }` with `print_int(p.y)` lowers to a
///     `getelementptr` on index 0 and reads `x`. The module is valid. The
///     answer is wrong.
///   * `match` on a wildcard or identifier pattern discards the scrutinee and
///     never binds the identifier.
///   * string collection does not walk `Stmt::Match` or `Stmt::Unsafe`, so a
///     literal inside either becomes `@.str.unknown`, which is never defined.
///   * a plain `main` emits `ret void`, putting the process exit status
///     outside the program's semantics — measured as a correct-looking run
///     that exits 2.
///
/// They do not all fail the same way. Dropped definitions and the undefined
/// `@.str.unknown` produce *invalid* IR, which an assembler rejects. But some
/// of the rest — field-zero access demonstrably — produce IR that is **valid**
/// and mean something other than the source, so verifying the assembly is not a
/// defence against the list. Half a
/// gate reads as protection while providing none, so the gate is whole: the
/// flag refuses, and the seven granular refusals stay underneath as the record
/// of what is missing.
///
/// No span: this is a property of the backend, not of any one line of source.
fn unimplemented_backend() -> CompileError {
    CompileError::Unimplemented {
        construct: "the LLVM backend (`--llvm`)".to_string(),
        consequence: "it is a skeleton kept for development, not a working backend: it drops \
             type, struct and enum definitions, reads every struct field as if it were the \
             first, gives every function call the type i64, and leaves a program's exit status \
             undefined — some of which produces assembly that is valid and silently means \
             something other than what was written"
            .to_string(),
        workaround: "build with the default C backend by dropping `--llvm`; it is the backend \
             this language is defined against, as described in \
             docs/specification/language-spec.md §1"
            .to_string(),
        span: None,
    }
}

/// Enum construction, e.g. `Color::Green`.
///
/// This is the sharpest of the four expression refusals, because it is the
/// only one whose fabricated value used to *link and run*. The old catch-all
/// returned `("", "0")`, so `Color::Red` and `Color::Green` compiled to
/// byte-identical IR (`store i64 0`) and the arguments of a data-carrying
/// variant — `Wrapper::Val(loud(99))` — vanished with the call inside them.
///
/// The workaround says *non-generic* because that is exactly as far as the
/// receipts go, and no further. Measured on the C backend: unit and tuple
/// variants run (`enum_help_recommends_the_c_backend_and_the_c_backend_delivers`)
/// and struct variants run (`enum_help_covers_struct_variants_because_it_says_so`)
/// — but a generic enum does not. `enum Opt<T> { None, Some(T) }` with
/// `Opt::Some(41)` emits
/// `struct Opt o = Opt_Some__new(41);` against a type it only forward-declares,
/// and gcc answers `variable has incomplete type 'struct Opt'`. Advising the C
/// backend without that qualifier would send a `Result<T, E>` user to a second
/// broken backend, which is the disease, not the cure. Pinned by
/// `the_generic_enum_caveat_in_the_help_is_real`.
fn unimplemented_enum_constructor(enum_name: &str, variant: &str, span: Span) -> CompileError {
    CompileError::Unimplemented {
        construct: format!("enum construction (`{}::{}`)", enum_name, variant),
        consequence: "the LLVM backend has no lowering for enum constructors, and would emit \
             the constant 0 for every variant of every enum — so distinct variants become the \
             same value — while dropping the constructor's arguments and any effects they have"
            .to_string(),
        workaround: "build with the default C backend by dropping `--llvm`; it lowers \
             construction of non-generic enums — unit, tuple and struct variants — and `match` \
             on them"
            .to_string(),
        span: Some(span),
    }
}

/// `expr?`.
///
/// Measured before the fix: `let v: i64 = might_fail(x)?;` produced
/// `store i64 0` with no call to `might_fail` anywhere in the function, so the
/// operand was not merely mis-propagated, it was never evaluated.
///
/// The workaround cannot say "use the C backend" on its own — the C backend
/// emits C for a `struct Result` layout it never defines, and gcc rejects it.
/// The rewrite has to remove the `?` as well, which is what
/// `question_help_requires_both_a_match_and_the_c_backend` executes.
fn unimplemented_question(span: Span) -> CompileError {
    CompileError::Unimplemented {
        construct: "the `?` operator".to_string(),
        consequence: "the LLVM backend has no lowering for `?`, and would emit the constant 0 \
             in place of the whole expression without evaluating the operand at all"
            .to_string(),
        workaround: "match on the enum the operand returns and handle each variant explicitly, \
             and build with the default C backend by dropping `--llvm`; neither backend lowers \
             `?` itself"
            .to_string(),
        span: Some(span),
    }
}

/// A macro invocation that survived macro expansion.
///
/// Reaching this is a phase-ordering fault rather than a missing feature:
/// expansion runs in `src/macros/mod.rs` before code generation, and the type
/// checker already refuses a stray invocation at `src/typeck/mod.rs:4141-4143`, so
/// no source program measured here gets this far. It is spelled out anyway
/// because the wildcard that used to cover it is gone, and because "currently
/// unreachable" is not "safe to fabricate".
fn unimplemented_macro_invocation(name: &str, span: Span) -> CompileError {
    CompileError::Unimplemented {
        construct: format!("the macro invocation `{}!`", name),
        consequence: "macro expansion runs before code generation, so an invocation that \
             reaches the LLVM backend was never expanded; the backend would emit the constant 0 \
             in its place"
            .to_string(),
        workaround: "declare the macro before the code that invokes it, or write out the code \
             the macro expands to"
            .to_string(),
        span: Some(span),
    }
}

/// `expr.await`.
///
/// Measured before the fix: `let v: i64 = work(3).await;` produced
/// `store i64 0` and printed `0`, with no call to `work`.
///
/// The workaround has to change the *signature*, not just delete the `.await`.
/// The only shape that reaches this arm is a plain function declared
/// `-> Future<T>` (a call to an `async fn` is typed as its bare return type, so
/// awaiting one never type checked), and dropping `.await` there leaves a
/// `Future<T>` where a `T` is required. Receipted by
/// `await_help_changes_the_signature_and_that_program_runs` and, for the
/// suggestion this message must never make,
/// `deleting_the_await_alone_does_not_compile`.
fn unimplemented_await(span: Span) -> CompileError {
    CompileError::Unimplemented {
        construct: "`.await`".to_string(),
        consequence: "there is no async runtime, and the LLVM backend would emit the constant 0 \
             in place of the awaited value without evaluating the operand at all"
            .to_string(),
        workaround: "declare the function to return its value directly (`-> T`, not \
             `-> Future<T>`) and call it; deleting `.await` on its own leaves a Future where a \
             value is required"
            .to_string(),
        span: Some(span),
    }
}

/// An `if` or a block used as a VALUE (N5-03, N5-05).
///
/// The C backend lowers these by hoisting: a temporary, a statement-form `if`
/// that assigns it, and a use of the name. The same shape here needs basic
/// blocks and a phi node, and this backend's statement lowering has neither —
/// so the arm refuses instead of emitting something that links.
fn unimplemented_value_block(span: Span) -> CompileError {
    CompileError::Unimplemented {
        construct: "an `if` or a block used as a value".to_string(),
        consequence: "this backend has no branch-and-merge lowering for an expression, so the \
             expression would be replaced by a constant and neither branch would run"
            .to_string(),
        workaround: "compile without `--llvm`, which is the supported backend and implements \
             both; or bind the value with a `let mut` and assign it from a statement `if`"
            .to_string(),
        span: Some(span),
    }
}

/// `break` and `continue`.
///
/// Statement lowering carries no loop context, so these used to emit
/// `br label %loop_end_placeholder` / `%loop_inc_placeholder` — branches to
/// labels this backend never defines. The module was invalid, but `pdc compile
/// --llvm` still exited 0 and still printed "Compilation successful", which is
/// the lie M1 is about.
fn unimplemented_loop_jump(keyword: &str, placeholder: &str, span: Span) -> CompileError {
    CompileError::Unimplemented {
        construct: format!("`{}`", keyword),
        consequence: format!(
            "the LLVM backend does not track loop context, and would emit a branch to \
             `%{}_placeholder`, a label it never defines; the module would be rejected by the \
             assembler",
            placeholder
        ),
        workaround: format!(
            "build with the default C backend by dropping `--llvm`; it lowers `{}`",
            keyword
        ),
        span: Some(span),
    }
}

/// An enum pattern in a `match` arm, e.g. `Color::Green => …`.
///
/// The scrutinee was never compared against anything: the arm emitted
/// `br label %match_arm12` where `%match_arm12` is a label allocated for a
/// later arm and never defined, so the assembler answered `use of undefined
/// value '%match_arm12'`. `Pattern` has no span of its own, so the diagnostic
/// points at the `match` statement.
fn unimplemented_enum_pattern(enum_name: &str, variant: &str, span: Span) -> CompileError {
    CompileError::Unimplemented {
        construct: format!("matching the enum pattern `{}::{}`", enum_name, variant),
        consequence: "the LLVM backend never compares the scrutinee against an enum pattern, \
             and would emit a branch to a label it does not define; the module would be \
             rejected by the assembler"
            .to_string(),
        workaround: "build with the default C backend by dropping `--llvm`; it lowers `match` \
             on non-generic enums"
            .to_string(),
        span: Some(span),
    }
}

#[cfg(test)]
mod tests {
    //! The granular refusals, kept executable behind the backend gate.
    //!
    //! `compile` now refuses before it looks at the program, so no integration
    //! test can reach the arms below any more. They are still the record of
    //! *what* is unimplemented, and they become the live behaviour the moment
    //! `BACKEND_REFUSES` flips — so they are driven here through
    //! `compile_unchecked`, which is the same code path minus the gate.
    //!
    //! Assertions here read `to_diagnostic()`, not just `to_string()`. The
    //! headline is the least interesting part of these diagnostics: the `note:`
    //! and the `help:` are the claims that can quietly become false.

    use super::*;
    use crate::ast::{EnumConstructorData, MatchArm, Visibility};
    use crate::errors::Diagnostic;

    fn main_fn(body: Vec<Stmt>) -> Program {
        Program {
            imports: vec![],
            items: vec![Item::Function(Function {
                visibility: Visibility::Private,
                is_async: false,
                name: "main".to_string(),
                lifetime_params: vec![],
                type_params: vec![],
                const_params: vec![],
                params: vec![],
                return_type: None,
                body,
                span: Span::dummy(),
                effects: None,
            })],
        }
    }

    /// Drive the lowering with the backend gate lifted, and return the
    /// diagnostic it refuses with.
    fn refusal_for(body: Vec<Stmt>) -> Diagnostic {
        let program = main_fn(body);
        let err = LLVMTextBackend::new("granular")
            .unwrap()
            .compile_unchecked(&program)
            .expect_err("the backend must refuse this, not lower it");
        err.to_diagnostic()
    }

    fn assert_refusal(diag: &Diagnostic, construct: &str, note_fragment: &str) {
        assert!(
            diag.message.contains(construct) && diag.message.contains("is not implemented"),
            "headline was {:?}",
            diag.message
        );
        assert!(
            diag.notes.iter().any(|n| n.contains(note_fragment)),
            "no note mentioning {:?}; notes = {:?}",
            note_fragment,
            diag.notes
        );
        assert!(
            !diag.suggestions.is_empty(),
            "a refusal with no `help:` is half a diagnostic"
        );
    }

    /// Every refusal must offer a way forward, and none may promise one.
    ///
    /// The second half is the rule that is easy to lose: a message saying
    /// "coming in M4" is a schedule, not a workaround, and it ages into a lie
    /// without anyone editing it.
    fn assert_help_is_actionable_and_makes_no_promise(diag: &Diagnostic) {
        for s in &diag.suggestions {
            let lower = s.message.to_lowercase();
            for banned in [
                "coming in", "will be", "planned", "roadmap", "future release", "milestone",
                "soon", "not yet supported but", "next version",
            ] {
                assert!(
                    !lower.contains(banned),
                    "help text promises a schedule ({:?}): {:?}",
                    banned,
                    s.message
                );
            }
        }
    }

    fn enum_ctor(variant: &str, data: Option<EnumConstructorData>) -> Expr {
        Expr::EnumConstructor {
            enum_name: "Color".to_string(),
            variant: variant.to_string(),
            data,
            span: Span::dummy(),
        }
    }

    #[test]
    fn enum_construction_is_refused() {
        let diag = refusal_for(vec![Stmt::Expr(enum_ctor("Green", None))]);
        assert_refusal(
            &diag,
            "enum construction (`Color::Green`)",
            "would emit the constant 0 for every variant",
        );
        assert_help_is_actionable_and_makes_no_promise(&diag);
        // The caveat the reviewers were right to demand: the help must not
        // recommend the C backend for generic enums, which it cannot build.
        assert!(
            diag.suggestions[0].message.contains("non-generic"),
            "help must not advertise the C backend for generic enums: {:?}",
            diag.suggestions[0].message
        );
    }

    #[test]
    fn enum_constructor_arguments_are_refused_not_dropped() {
        let diag = refusal_for(vec![Stmt::Expr(enum_ctor(
            "Val",
            Some(EnumConstructorData::Tuple(vec![Expr::Integer(99)])),
        ))]);
        assert_refusal(
            &diag,
            "enum construction (`Color::Val`)",
            "dropping the constructor's arguments",
        );
    }

    #[test]
    fn question_is_refused() {
        let diag = refusal_for(vec![Stmt::Expr(Expr::Question {
            expr: Box::new(Expr::Integer(1)),
            span: Span::dummy(),
        })]);
        assert_refusal(
            &diag,
            "the `?` operator",
            "without evaluating the operand at all",
        );
        assert_help_is_actionable_and_makes_no_promise(&diag);
    }

    #[test]
    fn await_is_refused() {
        let diag = refusal_for(vec![Stmt::Expr(Expr::Await {
            expr: Box::new(Expr::Integer(1)),
            span: Span::dummy(),
        })]);
        assert_refusal(&diag, "`.await`", "there is no async runtime");
        assert_help_is_actionable_and_makes_no_promise(&diag);
        // The suggestion this diagnostic must never make: deleting `.await`
        // alone leaves a Future where a value is required.
        assert!(
            diag.suggestions[0].message.contains("-> T"),
            "help must change the signature, not just delete `.await`: {:?}",
            diag.suggestions[0].message
        );
    }

    #[test]
    fn a_macro_invocation_reaching_the_backend_is_refused() {
        let diag = refusal_for(vec![Stmt::Expr(Expr::MacroInvocation {
            name: "println".to_string(),
            args: vec![],
            span: Span::dummy(),
        })]);
        assert_refusal(
            &diag,
            "the macro invocation `println!`",
            "macro expansion runs before code generation",
        );
        assert_help_is_actionable_and_makes_no_promise(&diag);
    }

    #[test]
    fn break_is_refused() {
        let diag = refusal_for(vec![Stmt::Break {
            value: None,
            span: Span::dummy(),
        }]);
        assert_refusal(&diag, "`break`", "%loop_end_placeholder");
        assert_help_is_actionable_and_makes_no_promise(&diag);
    }

    #[test]
    fn continue_is_refused() {
        let diag = refusal_for(vec![Stmt::Continue {
            span: Span::dummy(),
        }]);
        assert_refusal(&diag, "`continue`", "%loop_inc_placeholder");
        assert_help_is_actionable_and_makes_no_promise(&diag);
    }

    #[test]
    fn enum_patterns_are_refused() {
        let diag = refusal_for(vec![Stmt::Match {
            expr: Expr::Integer(0),
            arms: vec![MatchArm {
                pattern: Pattern::EnumPattern {
                    enum_name: "Color".to_string(),
                    variant: "Red".to_string(),
                    data: None,
                },
                body: vec![],
            }],
            span: Span::dummy(),
        }]);
        assert_refusal(
            &diag,
            "matching the enum pattern `Color::Red`",
            "never compares the scrutinee against an enum pattern",
        );
        assert_help_is_actionable_and_makes_no_promise(&diag);
    }

    /// Nothing that the granular arms refuse ever yields IR.
    ///
    /// `compile_unchecked` returning `Err` is the property; a future arm that
    /// "refuses" by pushing a comment into `ir` and carrying on would pass the
    /// message assertions above and fail here.
    #[test]
    fn no_granular_refusal_yields_ir() {
        let cases: Vec<Vec<Stmt>> = vec![
            vec![Stmt::Expr(enum_ctor("Green", None))],
            vec![Stmt::Expr(Expr::Question {
                expr: Box::new(Expr::Integer(1)),
                span: Span::dummy(),
            })],
            vec![Stmt::Expr(Expr::Await {
                expr: Box::new(Expr::Integer(1)),
                span: Span::dummy(),
            })],
            vec![Stmt::Expr(Expr::MacroInvocation {
                name: "m".to_string(),
                args: vec![],
                span: Span::dummy(),
            })],
            vec![Stmt::Break {
                value: None,
                span: Span::dummy(),
            }],
            vec![Stmt::Continue {
                span: Span::dummy(),
            }],
        ];
        for body in cases {
            let program = main_fn(body);
            let result = LLVMTextBackend::new("no_ir").unwrap().compile_unchecked(&program);
            assert!(result.is_err(), "lowered to IR: {:?}", result.ok());
        }
    }

    /// The gate itself: `compile` refuses without consulting the program.
    ///
    /// The body here lowers perfectly well — `compile_unchecked` produces a
    /// module for it — which is the point. The refusal is a property of the
    /// backend, not of anything the user wrote.
    #[test]
    fn the_backend_gate_refuses_a_program_it_could_otherwise_lower() {
        let body = vec![Stmt::Expr(Expr::Integer(1))];

        let ir = LLVMTextBackend::new("gate")
            .unwrap()
            .compile_unchecked(&main_fn(body.clone()))
            .expect("this program is within what the skeleton can lower");
        assert!(ir.contains("define void @main()"));

        let diag = LLVMTextBackend::new("gate")
            .unwrap()
            .compile(&main_fn(body))
            .expect_err("`--llvm` must refuse regardless of the program")
            .to_diagnostic();

        assert!(
            diag.message.contains("the LLVM backend (`--llvm`)")
                && diag.message.contains("is not implemented"),
            "headline was {:?}",
            diag.message
        );
        assert_help_is_actionable_and_makes_no_promise(&diag);
        assert!(
            diag.span.is_none() || diag.span == Some(Span::dummy()),
            "a whole-backend refusal must not point at a line of source"
        );
    }

    /// The wholesale refusal has to say three things, and it is the only
    /// message a `--llvm` user will now see.
    #[test]
    fn the_backend_refusal_states_what_it_is_and_what_to_use_instead() {
        let diag = LLVMTextBackend::new("msg")
            .unwrap()
            .compile(&main_fn(vec![]))
            .unwrap_err()
            .to_diagnostic();

        let note = diag.notes.join(" ");
        assert!(
            note.contains("kept for development"),
            "the note must say why the backend still exists: {:?}",
            note
        );
        assert!(
            note.contains("valid and silently means something other than what was written"),
            "the note must say the failure is silent, not loud: {:?}",
            note
        );

        let help = diag.suggestions[0].message.clone();
        assert!(
            help.contains("default C backend") && help.contains("--llvm"),
            "the help must name the working backend: {:?}",
            help
        );
        assert!(
            help.contains("docs/specification/language-spec.md"),
            "the help must point at the specification, not a milestone: {:?}",
            help
        );
        assert_help_is_actionable_and_makes_no_promise(&diag);
    }

    // -----------------------------------------------------------------------
    // The workarounds, executed — and bound to the message that suggests them
    // -----------------------------------------------------------------------
    //
    // These sit next to the constructors rather than in `tests/`, because the
    // coupling is the point. Each one reads the real
    // `to_diagnostic().suggestions[0].message`, asserts the property it is
    // about to rely on, and only then compiles and runs the rewrite. If the
    // help text drifts, the assertion fires before anything is executed; if the
    // rewrite stops working, the run fails. Asserting a quoted copy of the help
    // in a doc comment — which is what this change did at first — catches
    // neither.

    use crate::linker::{link_command, OptLevel};
    use crate::Driver;
    use std::process::Command;
    use tempfile::TempDir;

    /// Compile with the default C backend, link against the real runtime, run,
    /// and return stdout.
    fn run_on_the_c_backend(source: &str, name: &str) -> String {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join(format!("{}.pd", name));
        let exe = dir.path().join(name);
        std::fs::write(&src, source).unwrap();

        let c_file = Driver::new()
            .compile_file(&src)
            .unwrap_or_else(|e| panic!("the suggested workaround did not compile: {}", e));

        let out = link_command(&c_file, &exe, OptLevel::Default)
            .expect("link_command")
            .output()
            .expect("gcc");
        assert!(
            out.status.success(),
            "gcc rejected the suggested workaround: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        let run = Command::new(&exe).output().expect("run");
        assert!(
            run.status.success(),
            "the suggested workaround failed at runtime: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        String::from_utf8_lossy(&run.stdout).to_string()
    }

    fn help_of(diag: &Diagnostic) -> String {
        diag.suggestions
            .first()
            .expect("a refusal with no `help:` is half a diagnostic")
            .message
            .clone()
    }

    fn words(s: &str) -> Vec<&str> {
        s.split_whitespace().collect()
    }

    /// `help:` for enum construction — unit and tuple variants.
    #[test]
    fn enum_help_recommends_the_c_backend_and_the_c_backend_delivers() {
        let diag = refusal_for(vec![Stmt::Expr(enum_ctor("Green", None))]);
        let help = help_of(&diag);
        assert!(
            help.contains("dropping `--llvm`") && help.contains("unit, tuple and struct"),
            "help changed shape: {:?}",
            help
        );

        // Unit variants: `Color::Green` is the *second* variant, so a backend
        // that fabricated 0 would print 1 here.
        let out = run_on_the_c_backend(
            r#"
enum Color { Red, Green }

fn main() {
    let c = Color::Green;
    match c {
        Color::Red => print_int(1),
        Color::Green => print_int(2),
    }
}
"#,
            "wa_enum_unit",
        );
        assert_eq!(out.trim(), "2");

        // Tuple variants, carrying a side effect the old catch-all deleted.
        let out = run_on_the_c_backend(
            r#"
enum Wrapper { Val(i64) }

fn loud(x: i64) -> i64 {
    print_int(x);
    return x;
}

fn main() {
    let w = Wrapper::Val(loud(99));
    print_int(1);
}
"#,
            "wa_enum_tuple",
        );
        assert_eq!(words(&out), vec!["99", "1"]);
    }

    /// The third variant shape the help now advertises. Field bindings are
    /// written out (`w: w`) because the parser has no shorthand for them.
    #[test]
    fn enum_help_covers_struct_variants_because_it_says_so() {
        let diag = refusal_for(vec![Stmt::Expr(enum_ctor("Green", None))]);
        assert!(help_of(&diag).contains("struct"));

        let out = run_on_the_c_backend(
            r#"
enum Shape {
    Circle { r: i64 },
    Rect { w: i64, h: i64 },
}

fn main() {
    let s = Shape::Rect { w: 3, h: 4 };
    match s {
        Shape::Circle { r: r } => print_int(r),
        Shape::Rect { w: w, h: h } => print_int(w * h),
    }
}
"#,
            "wa_enum_struct",
        );
        assert_eq!(out.trim(), "12");
    }

    /// The word "non-generic" in the help is load-bearing, not hedging.
    ///
    /// Without it the message would send a `Result<T, E>` user to a backend
    /// that cannot build their program either. This pins the reason the
    /// qualifier exists, so nobody "tidies" it away.
    #[test]
    fn the_generic_enum_caveat_in_the_help_is_real() {
        let diag = refusal_for(vec![Stmt::Expr(enum_ctor("Green", None))]);
        assert!(
            help_of(&diag).contains("non-generic"),
            "help must qualify the recommendation: {:?}",
            help_of(&diag)
        );

        let dir = TempDir::new().unwrap();
        let src = dir.path().join("wa_enum_generic.pd");
        std::fs::write(
            &src,
            r#"
enum Opt<T> {
    None,
    Some(T),
}

fn main() {
    let o = Opt::Some(41);
    print_int(7);
}
"#,
        )
        .unwrap();

        // The C backend accepts this and then emits C that gcc rejects with
        // `variable has incomplete type 'struct Opt'`. Front-end success is
        // exactly why the caveat has to be in the help text.
        let c_file = Driver::new()
            .compile_file(&src)
            .expect("the front end accepts generic enums");
        let exe = dir.path().join("wa_enum_generic");
        let out = link_command(&c_file, &exe, OptLevel::Default)
            .expect("link_command")
            .output()
            .expect("gcc");
        assert!(
            !out.status.success(),
            "generic enums now build on the C backend — widen the help text"
        );
    }

    /// `help:` for `?` — both halves are load-bearing.
    ///
    /// Dropping `--llvm` alone is not enough: the C backend emits C for a
    /// `struct Result` layout it never defines. The `?` has to go too.
    #[test]
    fn question_help_requires_both_a_match_and_the_c_backend() {
        let diag = refusal_for(vec![Stmt::Expr(Expr::Question {
            expr: Box::new(Expr::Integer(1)),
            span: Span::dummy(),
        })]);
        let help = help_of(&diag);
        assert!(
            help.contains("match on the enum") && help.contains("dropping `--llvm`"),
            "help must ask for both: {:?}",
            help
        );

        let out = run_on_the_c_backend(
            r#"
enum Result {
    Ok(i64),
    Err(i64),
}

fn might_fail(x: i64) -> Result {
    if x < 0 {
        return Result::Err(0 - x);
    }
    return Result::Ok(x * 2);
}

fn main() {
    match might_fail(5) {
        Result::Ok(v) => print_int(v),
        Result::Err(e) => print_int(e),
    }
    match might_fail(0 - 7) {
        Result::Ok(v) => print_int(v),
        Result::Err(e) => print_int(e),
    }
}
"#,
            "wa_question",
        );
        assert_eq!(words(&out), vec!["10", "7"]);
    }

    /// `help:` for `.await` — the signature changes, the call stays.
    #[test]
    fn await_help_changes_the_signature_and_that_program_runs() {
        let diag = refusal_for(vec![Stmt::Expr(Expr::Await {
            expr: Box::new(Expr::Integer(1)),
            span: Span::dummy(),
        })]);
        let help = help_of(&diag);
        assert!(
            help.contains("-> T") && help.contains("not `-> Future<T>`"),
            "help must change the signature: {:?}",
            help
        );

        let out = run_on_the_c_backend(
            r#"
fn work(x: i64) -> i64 {
    return x * 2;
}

fn main() {
    let v: i64 = work(3);
    print_int(v);
}
"#,
            "wa_await",
        );
        assert_eq!(out.trim(), "6");
    }

    /// The suggestion the `.await` help must never make.
    ///
    /// Deleting `.await` and changing nothing else leaves a `Future<i64>` bound
    /// to an `i64`. If anyone "simplifies" the help back to "just remove the
    /// .await", the receipt for why that is wrong is already here.
    #[test]
    fn deleting_the_await_alone_does_not_compile() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("wa_await_naive.pd");
        std::fs::write(
            &src,
            r#"
fn work(x: i64) -> Future<i64> {
    return work(x);
}

fn main() {
    let v: i64 = work(3);
    print_int(v);
}
"#,
        )
        .unwrap();
        let err = Driver::new()
            .compile_file(&src)
            .expect_err("a Future bound to an i64 must not type check")
            .to_string();
        assert!(err.contains("Type mismatch"), "{}", err);
        assert!(err.contains("Future"), "{}", err);
    }

    /// `help:` for the macro invocation — the clause a user can always take.
    #[test]
    fn macro_help_offers_writing_the_expansion_out() {
        let diag = refusal_for(vec![Stmt::Expr(Expr::MacroInvocation {
            name: "println".to_string(),
            args: vec![],
            span: Span::dummy(),
        })]);
        assert!(
            help_of(&diag).contains("write out the code"),
            "help must offer the inline expansion: {:?}",
            help_of(&diag)
        );

        let out = run_on_the_c_backend("fn main() { print_int(6); }", "wa_macro");
        assert_eq!(out.trim(), "6");
    }

    /// `help:` for `break` and `continue`.
    #[test]
    fn loop_jump_help_recommends_the_c_backend_and_it_lowers_them() {
        let brk = refusal_for(vec![Stmt::Break {
            value: None,
            span: Span::dummy(),
        }]);
        assert!(help_of(&brk).contains("dropping `--llvm`") && help_of(&brk).contains("`break`"));

        let out = run_on_the_c_backend(
            r#"
fn main() {
    let mut i: i64 = 0;
    while i < 10 {
        if i > 3 {
            break;
        }
        print_int(i);
        i = i + 1;
    }
}
"#,
            "wa_break",
        );
        assert_eq!(words(&out), vec!["0", "1", "2", "3"]);

        let cont = refusal_for(vec![Stmt::Continue {
            span: Span::dummy(),
        }]);
        assert!(
            help_of(&cont).contains("dropping `--llvm`") && help_of(&cont).contains("`continue`")
        );

        let out = run_on_the_c_backend(
            r#"
fn main() {
    let mut i: i64 = 0;
    while i < 5 {
        i = i + 1;
        if i == 3 {
            continue;
        }
        print_int(i);
    }
}
"#,
            "wa_continue",
        );
        assert_eq!(words(&out), vec!["1", "2", "4", "5"]);
    }

    /// `help:` for the enum pattern.
    #[test]
    fn enum_pattern_help_recommends_the_c_backend_and_it_matches() {
        let diag = refusal_for(vec![Stmt::Match {
            expr: Expr::Integer(0),
            arms: vec![MatchArm {
                pattern: Pattern::EnumPattern {
                    enum_name: "Color".to_string(),
                    variant: "Red".to_string(),
                    data: None,
                },
                body: vec![],
            }],
            span: Span::dummy(),
        }]);
        let help = help_of(&diag);
        assert!(
            help.contains("dropping `--llvm`") && help.contains("non-generic"),
            "help changed shape: {:?}",
            help
        );

        let out = run_on_the_c_backend(
            r#"
enum Color { Red, Green }

fn describe(c: Color) {
    match c {
        Color::Red => print_int(1),
        Color::Green => print_int(2),
    }
}

fn main() {
    describe(Color::Red);
    describe(Color::Green);
}
"#,
            "wa_enum_pattern",
        );
        assert_eq!(words(&out), vec!["1", "2"]);
    }

    /// `help:` for the wholesale refusal — the only message a `--llvm` user now
    /// sees, so its advice had better work on a real program.
    #[test]
    fn backend_help_recommends_the_default_and_the_default_works() {
        let diag = LLVMTextBackend::new("wa_backend")
            .unwrap()
            .compile(&main_fn(vec![]))
            .unwrap_err()
            .to_diagnostic();
        assert!(help_of(&diag).contains("dropping `--llvm`"));

        let out = run_on_the_c_backend(
            r#"
enum Color { Red, Green }

fn main() {
    let mut i: i64 = 0;
    while i < 3 {
        if i == 1 {
            i = i + 1;
            continue;
        }
        print_int(i);
        i = i + 1;
    }
    let c = Color::Green;
    match c {
        Color::Red => print_int(100),
        Color::Green => print_int(200),
    }
}
"#,
            "wa_backend",
        );
        assert_eq!(words(&out), vec!["0", "2", "200"]);
    }
}
