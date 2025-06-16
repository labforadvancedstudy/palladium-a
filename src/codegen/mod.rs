// Code generation for Palladium
// "Forging legends into machine code"

use crate::ast::{*, AssignTarget, UnaryOp};
use crate::errors::{CompileError, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct CodeGenerator {
    module_name: String,
    output: String,
    /// Map of function names to their signatures (params and return type)
    functions: std::collections::HashMap<String, (Vec<Param>, Option<Type>)>,
    /// Map of variable names to their C types (for type inference)
    variables: std::collections::HashMap<String, String>,
    /// Map of parameter names to their mutability (for current function)
    mutable_params: std::collections::HashMap<String, bool>,
}

impl CodeGenerator {
    pub fn new(module_name: &str) -> Result<Self> {
        Ok(Self {
            module_name: module_name.to_string(),
            output: String::new(),
            functions: std::collections::HashMap::new(),
            variables: std::collections::HashMap::new(),
            mutable_params: std::collections::HashMap::new(),
        })
    }
    
    /// Infer the C type of an expression
    fn infer_expr_type(&self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(_) => "long long".to_string(),
            Expr::String(_) => "const char*".to_string(),
            Expr::Bool(_) => "int".to_string(),
            Expr::StructLiteral { name, .. } => name.to_string(),
            Expr::Ident(name) => {
                // Look up variable type
                self.variables.get(name).cloned().unwrap_or_else(|| "long long".to_string())
            },
            Expr::Binary { left, op, right, .. } => {
                // String concatenation returns a string
                if matches!(op, BinOp::Add) {
                    let left_type = self.infer_expr_type(left);
                    let right_type = self.infer_expr_type(right);
                    if left_type == "const char*" && right_type == "const char*" {
                        return "const char*".to_string();
                    }
                }
                "long long".to_string()
            },
            _ => "long long".to_string(), // fallback
        }
    }
    
    /// Compile an AST to machine code
    pub fn compile(&mut self, program: &Program) -> Result<()> {
        // For v0.1, we'll generate a simple C file that we can compile with gcc
        // This is a temporary solution until LLVM integration is complete
        
        self.output.push_str("#include <stdio.h>\n");
        self.output.push_str("#include <string.h>\n");
        self.output.push_str("#include <stdlib.h>\n");
        self.output.push_str("#include <ctype.h>\n\n");
        
        // Generate print function wrapper
        self.output.push_str("void __pd_print(const char* str) {\n");
        self.output.push_str("    printf(\"%s\\n\", str);\n");
        self.output.push_str("}\n\n");
        
        // Generate print_int function wrapper
        self.output.push_str("void __pd_print_int(long long value) {\n");
        self.output.push_str("    printf(\"%lld\\n\", value);\n");
        self.output.push_str("}\n\n");
        
        // Generate string manipulation functions
        
        // string_len
        self.output.push_str("long long __pd_string_len(const char* str) {\n");
        self.output.push_str("    return strlen(str);\n");
        self.output.push_str("}\n\n");
        
        // string_concat
        self.output.push_str("const char* __pd_string_concat(const char* s1, const char* s2) {\n");
        self.output.push_str("    size_t len1 = strlen(s1);\n");
        self.output.push_str("    size_t len2 = strlen(s2);\n");
        self.output.push_str("    char* result = (char*)malloc(len1 + len2 + 1);\n");
        self.output.push_str("    strcpy(result, s1);\n");
        self.output.push_str("    strcat(result, s2);\n");
        self.output.push_str("    return result;\n");
        self.output.push_str("}\n\n");
        
        // string_eq
        self.output.push_str("int __pd_string_eq(const char* s1, const char* s2) {\n");
        self.output.push_str("    return strcmp(s1, s2) == 0;\n");
        self.output.push_str("}\n\n");
        
        // string_char_at
        self.output.push_str("long long __pd_string_char_at(const char* str, long long index) {\n");
        self.output.push_str("    if (index < 0 || index >= (long long)strlen(str)) return -1;\n");
        self.output.push_str("    return (long long)(unsigned char)str[index];\n");
        self.output.push_str("}\n\n");
        
        // string_substring
        self.output.push_str("const char* __pd_string_substring(const char* str, long long start, long long end) {\n");
        self.output.push_str("    size_t len = strlen(str);\n");
        self.output.push_str("    if (start < 0) start = 0;\n");
        self.output.push_str("    if (end > (long long)len) end = len;\n");
        self.output.push_str("    if (start >= end) return \"\";\n");
        self.output.push_str("    size_t sub_len = end - start;\n");
        self.output.push_str("    char* result = (char*)malloc(sub_len + 1);\n");
        self.output.push_str("    strncpy(result, str + start, sub_len);\n");
        self.output.push_str("    result[sub_len] = '\\0';\n");
        self.output.push_str("    return result;\n");
        self.output.push_str("}\n\n");
        
        // string_from_char
        self.output.push_str("const char* __pd_string_from_char(long long c) {\n");
        self.output.push_str("    char* result = (char*)malloc(2);\n");
        self.output.push_str("    result[0] = (char)c;\n");
        self.output.push_str("    result[1] = '\\0';\n");
        self.output.push_str("    return result;\n");
        self.output.push_str("}\n\n");
        
        // char_is_digit
        self.output.push_str("int __pd_char_is_digit(long long c) {\n");
        self.output.push_str("    return isdigit((int)c);\n");
        self.output.push_str("}\n\n");
        
        // char_is_alpha
        self.output.push_str("int __pd_char_is_alpha(long long c) {\n");
        self.output.push_str("    return isalpha((int)c);\n");
        self.output.push_str("}\n\n");
        
        // char_is_whitespace
        self.output.push_str("int __pd_char_is_whitespace(long long c) {\n");
        self.output.push_str("    return isspace((int)c);\n");
        self.output.push_str("}\n\n");
        
        // string_to_int
        self.output.push_str("long long __pd_string_to_int(const char* str) {\n");
        self.output.push_str("    return atoll(str);\n");
        self.output.push_str("}\n\n");
        
        // File I/O functions
        self.output.push_str("// File I/O support\n");
        self.output.push_str("#define MAX_FILES 256\n");
        self.output.push_str("static FILE* __pd_file_handles[MAX_FILES] = {0};\n");
        self.output.push_str("static int __pd_next_handle = 1;\n\n");
        
        // file_open
        self.output.push_str("long long __pd_file_open(const char* path) {\n");
        self.output.push_str("    if (__pd_next_handle >= MAX_FILES) return -1;\n");
        self.output.push_str("    FILE* f = fopen(path, \"r+\");\n");
        self.output.push_str("    if (!f) f = fopen(path, \"w+\");\n");
        self.output.push_str("    if (!f) return -1;\n");
        self.output.push_str("    int handle = __pd_next_handle++;\n");
        self.output.push_str("    __pd_file_handles[handle] = f;\n");
        self.output.push_str("    return handle;\n");
        self.output.push_str("}\n\n");
        
        // file_read_all
        self.output.push_str("const char* __pd_file_read_all(long long handle) {\n");
        self.output.push_str("    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return \"\";\n");
        self.output.push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output.push_str("    fseek(f, 0, SEEK_END);\n");
        self.output.push_str("    long size = ftell(f);\n");
        self.output.push_str("    fseek(f, 0, SEEK_SET);\n");
        self.output.push_str("    char* buffer = (char*)malloc(size + 1);\n");
        self.output.push_str("    fread(buffer, 1, size, f);\n");
        self.output.push_str("    buffer[size] = '\\0';\n");
        self.output.push_str("    return buffer;\n");
        self.output.push_str("}\n\n");
        
        // file_read_line
        self.output.push_str("const char* __pd_file_read_line(long long handle) {\n");
        self.output.push_str("    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return \"\";\n");
        self.output.push_str("    static char line_buffer[4096];\n");
        self.output.push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output.push_str("    if (fgets(line_buffer, sizeof(line_buffer), f)) {\n");
        self.output.push_str("        size_t len = strlen(line_buffer);\n");
        self.output.push_str("        if (len > 0 && line_buffer[len-1] == '\\n') line_buffer[len-1] = '\\0';\n");
        self.output.push_str("        return strdup(line_buffer);\n");
        self.output.push_str("    }\n");
        self.output.push_str("    return \"\";\n");
        self.output.push_str("}\n\n");
        
        // file_write
        self.output.push_str("int __pd_file_write(long long handle, const char* content) {\n");
        self.output.push_str("    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return 0;\n");
        self.output.push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output.push_str("    return fputs(content, f) >= 0;\n");
        self.output.push_str("}\n\n");
        
        // file_close
        self.output.push_str("int __pd_file_close(long long handle) {\n");
        self.output.push_str("    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return 0;\n");
        self.output.push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output.push_str("    __pd_file_handles[handle] = NULL;\n");
        self.output.push_str("    return fclose(f) == 0;\n");
        self.output.push_str("}\n\n");
        
        // file_exists
        self.output.push_str("int __pd_file_exists(const char* path) {\n");
        self.output.push_str("    FILE* f = fopen(path, \"r\");\n");
        self.output.push_str("    if (f) {\n");
        self.output.push_str("        fclose(f);\n");
        self.output.push_str("        return 1;\n");
        self.output.push_str("    }\n");
        self.output.push_str("    return 0;\n");
        self.output.push_str("}\n\n");
        
        // First pass: collect function signatures
        for item in &program.items {
            if let Item::Function(func) = item {
                self.functions.insert(func.name.clone(), (func.params.clone(), func.return_type.clone()));
            }
        }
        
        // Generate struct definitions first
        for item in &program.items {
            match item {
                Item::Struct(struct_def) => {
                    self.generate_struct(struct_def)?;
                }
                _ => {}
            }
        }
        
        // Generate functions
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    // Skip generic functions - they should only be generated when instantiated
                    if !func.type_params.is_empty() {
                        continue;
                    }
                    self.generate_function(func)?;
                }
                Item::Struct(_) => {
                    // Already generated above
                }
                Item::Enum(_) => {
                    // TODO: Generate enum definitions for C
                    // For now, skip enum generation
                }
            }
        }
        
        Ok(())
    }
    
    /// Generate code for a struct definition
    fn generate_struct(&mut self, struct_def: &StructDef) -> Result<()> {
        self.output.push_str(&format!("typedef struct {} {{\n", struct_def.name));
        
        for (field_name, field_type) in &struct_def.fields {
            self.output.push_str("    ");
            
            let c_type = match field_type {
                Type::I32 => "int",
                Type::I64 => "long long",
                Type::U32 => "unsigned int",
                Type::U64 => "unsigned long long",
                Type::Bool => "int",
                Type::String => "const char*",
                Type::Array(elem_type, size) => {
                    // For arrays in structs, we need to handle them specially
                    let elem_c_type = match elem_type.as_ref() {
                        Type::I32 => "int",
                        Type::I64 => "long long",
                        Type::U32 => "unsigned int",
                        Type::U64 => "unsigned long long",
                        Type::Bool => "int",
                        Type::Custom(name) => &format!("struct {}", name),  // Support struct arrays
                        _ => return Err(CompileError::Generic(
                            "Unsupported array element type in struct field".to_string()
                        )),
                    };
                    self.output.push_str(&format!("{} {}[{}];\n", elem_c_type, field_name, size));
                    continue;
                }
                Type::Unit => "void",
                Type::Custom(name) => {
                    // For custom types (other structs), we use the struct name directly
                    self.output.push_str(&format!("struct {} {};\n", name, field_name));
                    continue;
                }
                Type::TypeParam(_) | Type::Generic { .. } => {
                    return Err(CompileError::Generic(
                        "Generic types in structs not yet supported".to_string()
                    ));
                }
            };
            
            self.output.push_str(&format!("{} {};\n", c_type, field_name));
        }
        
        self.output.push_str(&format!("}} {};\n\n", struct_def.name));
        Ok(())
    }
    
    /// Generate code for a function
    fn generate_function(&mut self, func: &Function) -> Result<()> {
        // Function signature with return type
        let return_type = match &func.return_type {
            Some(Type::I32) => "int",
            Some(Type::I64) => "long long",
            Some(Type::U32) => "unsigned int",
            Some(Type::U64) => "unsigned long long",
            Some(Type::Bool) => "int",  // C doesn't have bool by default
            Some(Type::String) => "const char*",
            Some(Type::Unit) | None => "void",
            Some(Type::Array(_, _)) => {
                // Arrays cannot be returned by value in C, would need to return pointer
                return Err(CompileError::Generic(
                    "Returning arrays from functions is not yet supported".to_string()
                ));
            }
            Some(Type::Custom(name)) => {
                // For custom types (structs), we return by value
                // Note: In real C, returning large structs by value might not be ideal
                // but it works and is simple to implement
                name.as_str()
            }
            Some(Type::TypeParam(_)) | Some(Type::Generic { .. }) => {
                return Err(CompileError::Generic(
                    "Generic return types not yet supported".to_string()
                ));
            }
        };
        
        // Special case: main always returns int in C
        let actual_return_type = if func.name == "main" && return_type == "void" {
            "int"
        } else {
            return_type
        };
        
        // Generate function parameters
        self.output.push_str(&format!("{} {}(", actual_return_type, func.name));
        
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            
            match &param.ty {
                Type::Array(elem_type, size) => {
                    // For arrays, we need to generate proper C array parameter syntax
                    let elem_c_type = match elem_type.as_ref() {
                        Type::I32 => "int",
                        Type::I64 => "long long",
                        Type::U32 => "unsigned int",
                        Type::U64 => "unsigned long long",
                        Type::Bool => "int",
                        Type::Custom(name) => name.as_str(),  // Support struct arrays
                        _ => return Err(CompileError::Generic(
                            "Unsupported array element type in function parameter".to_string()
                        )),
                    };
                    // In C, array parameters are passed as pointers
                    // We'll generate: type name[size] for clarity, though it decays to pointer
                    self.output.push_str(&format!("{} {}[{}]", elem_c_type, param.name, size));
                }
                Type::Custom(name) => {
                    // For custom types (structs)
                    if param.mutable {
                        // Pass by pointer for mutable parameters
                        self.output.push_str(&format!("{}* {}", name, param.name));
                    } else {
                        // Pass by value for immutable parameters
                        self.output.push_str(&format!("{} {}", name, param.name));
                    }
                }
                _ => {
                    // For primitive types
                    let c_type = match &param.ty {
                        Type::I32 => "int",
                        Type::I64 => "long long",
                        Type::U32 => "unsigned int",
                        Type::U64 => "unsigned long long",
                        Type::Bool => "int",
                        Type::String => "const char*",
                        Type::Unit => "void",
                        _ => unreachable!(),
                    };
                    
                    if param.mutable {
                        // Pass by pointer for mutable parameters
                        self.output.push_str(&format!("{}* {}", c_type, param.name));
                    } else {
                        // Pass by value for immutable parameters
                        self.output.push_str(&format!("{} {}", c_type, param.name));
                    }
                }
            }
        }
        
        self.output.push_str(") {\n");
        
        // Clear mutable_params from previous function and populate with current function's params
        self.mutable_params.clear();
        self.variables.clear(); // Clear variables from previous function
        
        for param in &func.params {
            self.mutable_params.insert(param.name.clone(), param.mutable);
            
            // Also track parameter types for type inference
            let c_type = match &param.ty {
                Type::String => "const char*".to_string(),
                Type::I32 => "int".to_string(),
                Type::I64 => "long long".to_string(),
                Type::Bool => "int".to_string(),
                Type::Custom(name) => name.clone(),
                _ => "long long".to_string(),
            };
            self.variables.insert(param.name.clone(), c_type);
        }
        
        // Function body
        for stmt in &func.body {
            self.generate_statement(stmt)?;
        }
        
        // Close function
        // Only add default return for void main or if no explicit return
        if func.name == "main" && func.return_type.is_none() {
            self.output.push_str("    return 0;\n");
        }
        self.output.push_str("}\n\n");
        
        // Clear parameter tracking after function
        self.mutable_params.clear();
        
        Ok(())
    }
    
    /// Generate code for a statement
    fn generate_statement(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Expr(expr) => {
                self.output.push_str("    ");
                self.generate_expression(expr)?;
                self.output.push_str(";\n");
            }
            Stmt::Return(None) => {
                self.output.push_str("    return;\n");
            }
            Stmt::Return(Some(expr)) => {
                self.output.push_str("    return ");
                self.generate_expression(expr)?;
                self.output.push_str(";\n");
            }
            Stmt::Let { name, ty, value, .. } => {
                self.output.push_str("    ");
                
                // Helper function to convert Type to C type string
                fn type_to_c(ty: &Type) -> String {
                    match ty {
                        Type::I32 => "int".to_string(),
                        Type::I64 => "long long".to_string(),
                        Type::U32 => "unsigned int".to_string(),
                        Type::U64 => "unsigned long long".to_string(),
                        Type::Bool => "int".to_string(),
                        Type::String => "const char*".to_string(),
                        Type::Unit => "void".to_string(),
                        Type::Array(elem_type, size) => {
                            format!("{}[{}]", type_to_c(elem_type), size)
                        }
                        Type::Custom(name) => name.to_string(),
                        Type::TypeParam(_) | Type::Generic { .. } => {
                            // TODO: Proper generic handling
                            "void*".to_string() // Placeholder
                        }
                    }
                }
                
                // Determine C type
                let (c_type, is_array, array_size) = match ty {
                    Some(t) => {
                        match t {
                            Type::Array(elem_type, size) => {
                                (type_to_c(elem_type), true, Some(*size))
                            }
                            _ => (type_to_c(t), false, None)
                        }
                    }
                    None => {
                        // Infer type from value using our helper
                        let inferred_type = self.infer_expr_type(value);
                        match value {
                            Expr::Integer(_) => ("long long".to_string(), false, None),
                            Expr::String(_) => ("const char*".to_string(), false, None),
                            Expr::Bool(_) => ("int".to_string(), false, None),
                            Expr::Binary { .. } => (inferred_type, false, None),
                            Expr::ArrayLiteral { elements, .. } => {
                                // Infer array element type from first element
                                let elem_type = if !elements.is_empty() {
                                    self.infer_expr_type(&elements[0])
                                } else {
                                    "long long".to_string()
                                };
                                (elem_type, true, Some(elements.len()))
                            }
                            Expr::ArrayRepeat { value, count, .. } => {
                                // Infer array element type from value
                                let elem_type = self.infer_expr_type(value);
                                let size = if let Expr::Integer(n) = count.as_ref() {
                                    *n as usize
                                } else {
                                    0  // This should have been caught by type checker
                                };
                                (elem_type, true, Some(size))
                            }
                            Expr::StructLiteral { name, .. } => (name.to_string(), false, None),
                            Expr::Call { func, .. } => {
                                // Infer type from function name for built-ins
                                match func.as_ref() {
                                    Expr::Ident(fname) => {
                                        match fname.as_str() {
                                            "string_concat" | "string_substring" | "string_from_char" => {
                                                ("const char*".to_string(), false, None)
                                            }
                                            "string_len" | "string_char_at" | "string_to_int" | "file_open" => {
                                                ("long long".to_string(), false, None)
                                            }
                                            "string_eq" | "char_is_digit" | "char_is_alpha" | "char_is_whitespace" 
                                            | "file_write" | "file_close" | "file_exists" => {
                                                ("int".to_string(), false, None)
                                            }
                                            "file_read_all" | "file_read_line" => {
                                                ("const char*".to_string(), false, None)
                                            }
                                            _ => {
                                                // Look up user-defined function return type
                                                if let Some((_, ret_type)) = self.functions.get(fname) {
                                                    match ret_type {
                                                        Some(t) => (type_to_c(t), false, None),
                                                        None => ("void".to_string(), false, None),
                                                    }
                                                } else {
                                                    ("long long".to_string(), false, None)
                                                }
                                            }
                                        }
                                    }
                                    _ => ("long long".to_string(), false, None)
                                }
                            }
                            _ => ("long long".to_string(), false, None),  // Default to int for now
                        }
                    }
                };
                
                // Track variable type for future inference
                if is_array {
                    // For arrays, store the full array type including brackets
                    self.variables.insert(name.clone(), format!("{}[{}]", c_type, array_size.unwrap_or(0)));
                } else {
                    self.variables.insert(name.clone(), c_type.clone());
                }
                
                if is_array {
                    // Array declaration
                    self.output.push_str(&format!("{} {}", c_type, name));
                    if let Some(size) = array_size {
                        self.output.push_str(&format!("[{}]", size));
                    }
                    self.output.push_str(" = ");
                    self.generate_expression(value)?;
                    self.output.push_str(";\n");
                } else {
                    // Regular variable declaration
                    self.output.push_str(&format!("{} {} = ", c_type, name));
                    self.generate_expression(value)?;
                    self.output.push_str(";\n");
                }
            }
            Stmt::Assign { target, value, .. } => {
                self.output.push_str("    ");
                match target {
                    AssignTarget::Ident(name) => {
                        // Check if this is a mutable parameter
                        if let Some(&is_mutable) = self.mutable_params.get(name) {
                            if is_mutable {
                                // Dereference mutable parameters
                                self.output.push_str(&format!("(*{}) = ", name));
                            } else {
                                self.output.push_str(&format!("{} = ", name));
                            }
                        } else {
                            self.output.push_str(&format!("{} = ", name));
                        }
                    }
                    AssignTarget::Index { array, index } => {
                        self.generate_expression(array)?;
                        self.output.push_str("[");
                        self.generate_expression(index)?;
                        self.output.push_str("] = ");
                    }
                    AssignTarget::FieldAccess { object, field } => {
                        // Check if object is a mutable parameter (pointer)
                        let use_arrow = match object.as_ref() {
                            Expr::Ident(name) => {
                                self.mutable_params.get(name).copied().unwrap_or(false)
                            }
                            _ => false,
                        };
                        
                        if use_arrow {
                            // For mutable params, we need special handling
                            if let Expr::Ident(name) = object.as_ref() {
                                self.output.push_str(&format!("{}->{} = ", name, field));
                            } else {
                                self.generate_expression(object)?;
                                self.output.push_str(&format!("->{} = ", field));
                            }
                        } else {
                            self.generate_expression(object)?;
                            self.output.push_str(&format!(".{} = ", field));
                        }
                    }
                }
                self.generate_expression(value)?;
                self.output.push_str(";\n");
            }
            Stmt::If { condition, then_branch, else_branch, .. } => {
                self.output.push_str("    if (");
                self.generate_expression(condition)?;
                self.output.push_str(") {\n");
                
                // Generate then branch
                for stmt in then_branch {
                    self.generate_statement(stmt)?;
                }
                
                self.output.push_str("    }");
                
                // Generate else branch if present
                if let Some(else_stmts) = else_branch {
                    self.output.push_str(" else {\n");
                    for stmt in else_stmts {
                        self.generate_statement(stmt)?;
                    }
                    self.output.push_str("    }");
                }
                
                self.output.push_str("\n");
            }
            Stmt::While { condition, body, .. } => {
                self.output.push_str("    while (");
                self.generate_expression(condition)?;
                self.output.push_str(") {\n");
                
                // Generate body
                for stmt in body {
                    self.generate_statement(stmt)?;
                }
                
                self.output.push_str("    }\n");
            }
            Stmt::For { var, iter, body, .. } => {
                self.output.push_str("    {\n");  // Create a new scope
                
                // Check if iterating over a range
                match iter {
                    Expr::Range { start, end, .. } => {
                        // Generate C-style for loop for range
                        self.output.push_str("        // For loop with range\n");
                        self.output.push_str(&format!("        for (long long {} = ", var));
                        self.generate_expression(start)?;
                        self.output.push_str(&format!("; {} < ", var));
                        self.generate_expression(end)?;
                        self.output.push_str(&format!("; {}++) {{\n", var));
                        
                        // Generate body
                        for stmt in body {
                            self.output.push_str("        ");  // Extra indentation
                            self.generate_statement(stmt)?;
                        }
                        
                        self.output.push_str("        }\n");
                    }
                    _ => {
                        // For arrays and other iterables
                        self.output.push_str("        // For-in loop\n");
                        self.output.push_str("        for (long long _i = 0; _i < sizeof(");
                        self.generate_expression(iter)?;
                        self.output.push_str(")/sizeof(");
                        self.generate_expression(iter)?;
                        self.output.push_str("[0]); _i++) {\n");
                        
                        // Declare loop variable and assign current element
                        self.output.push_str(&format!("            long long {} = ", var));
                        self.generate_expression(iter)?;
                        self.output.push_str("[_i];\n");
                        
                        // Generate body
                        for stmt in body {
                            self.output.push_str("        ");  // Extra indentation
                            self.generate_statement(stmt)?;
                        }
                        
                        self.output.push_str("        }\n");
                    }
                }
                self.output.push_str("    }\n");
            }
            Stmt::Break { .. } => {
                self.output.push_str("    break;\n");
            }
            Stmt::Continue { .. } => {
                self.output.push_str("    continue;\n");
            }
            Stmt::Match { expr, arms, .. } => {
                // Generate a series of if-else statements for pattern matching
                self.output.push_str("    // Match statement\n");
                self.output.push_str("    {\n");
                
                // Store the match expression in a temporary variable
                self.output.push_str("        // Temporary for match expression\n");
                self.output.push_str("        long long _match_expr = ");
                self.generate_expression(expr)?;
                self.output.push_str(";\n");
                
                // Generate if-else chain for each arm
                for (i, arm) in arms.iter().enumerate() {
                    if i == 0 {
                        self.output.push_str("        if (");
                    } else {
                        self.output.push_str(" else if (");
                    }
                    
                    // Generate pattern matching condition
                    match &arm.pattern {
                        Pattern::Wildcard => {
                            // Wildcard always matches
                            self.output.push_str("1");
                        }
                        Pattern::Ident(name) => {
                            // Identifier pattern always matches and binds
                            self.output.push_str("1) {\n");
                            self.output.push_str(&format!("            long long {} = _match_expr;\n", name));
                            // Continue with body generation below
                            for stmt in &arm.body {
                                self.output.push_str("        ");
                                self.generate_statement(stmt)?;
                            }
                            self.output.push_str("        }");
                            continue;
                        }
                        Pattern::EnumPattern { enum_name, variant, data: _ } => {
                            // For now, simple enum matching based on variant index
                            // TODO: Implement proper enum variant matching
                            self.output.push_str(&format!("/* match {}::{} */ 1", enum_name, variant));
                        }
                    }
                    
                    self.output.push_str(") {\n");
                    
                    // Generate arm body
                    for stmt in &arm.body {
                        self.output.push_str("        ");
                        self.generate_statement(stmt)?;
                    }
                    
                    self.output.push_str("        }");
                }
                
                // If no wildcard pattern, we might need a default case
                // TODO: Add exhaustiveness checking
                
                self.output.push_str("\n    }\n");
            }
        }
        Ok(())
    }
    
    /// Generate code for an expression
    fn generate_expression(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::String(s) => {
                // Escape the string properly
                let escaped = s.replace("\\", "\\\\")
                    .replace("\"", "\\\"")
                    .replace("\n", "\\n")
                    .replace("\t", "\\t")
                    .replace("\r", "\\r");
                self.output.push_str(&format!("\"{}\"", escaped));
            }
            Expr::Integer(n) => {
                self.output.push_str(&format!("{}", n));
            }
            Expr::Bool(b) => {
                // C represents bool as 1 or 0
                self.output.push_str(if *b { "1" } else { "0" });
            }
            Expr::Ident(name) => {
                // Check if this is a mutable parameter
                if let Some(&is_mutable) = self.mutable_params.get(name) {
                    if is_mutable {
                        // For arrays, don't dereference as they're already pointers
                        // We need to check the parameter type
                        let is_array = self.functions.values()
                            .find_map(|(params, _)| {
                                params.iter().find(|p| &p.name == name)
                                    .and_then(|p| match &p.ty {
                                        Type::Array(_, _) => Some(true),
                                        _ => None,
                                    })
                            })
                            .unwrap_or(false);
                        
                        if is_array {
                            // Arrays are already pointers, don't dereference
                            self.output.push_str(name);
                        } else {
                            // Dereference mutable parameters
                            self.output.push_str(&format!("(*{})", name));
                        }
                    } else {
                        self.output.push_str(name);
                    }
                } else {
                    // Regular variable
                    self.output.push_str(name);
                }
            }
            Expr::Call { func, args, .. } => {
                // Generate function name
                match func.as_ref() {
                    Expr::Ident(name) => {
                        // Map built-in functions
                        match name.as_str() {
                            "print" => self.output.push_str("__pd_print"),
                            "print_int" => self.output.push_str("__pd_print_int"),
                            "string_len" => self.output.push_str("__pd_string_len"),
                            "string_concat" => self.output.push_str("__pd_string_concat"),
                            "string_eq" => self.output.push_str("__pd_string_eq"),
                            "string_char_at" => self.output.push_str("__pd_string_char_at"),
                            "string_substring" => self.output.push_str("__pd_string_substring"),
                            "string_from_char" => self.output.push_str("__pd_string_from_char"),
                            "char_is_digit" => self.output.push_str("__pd_char_is_digit"),
                            "char_is_alpha" => self.output.push_str("__pd_char_is_alpha"),
                            "char_is_whitespace" => self.output.push_str("__pd_char_is_whitespace"),
                            "string_to_int" => self.output.push_str("__pd_string_to_int"),
                            "file_open" => self.output.push_str("__pd_file_open"),
                            "file_read_all" => self.output.push_str("__pd_file_read_all"),
                            "file_read_line" => self.output.push_str("__pd_file_read_line"),
                            "file_write" => self.output.push_str("__pd_file_write"),
                            "file_close" => self.output.push_str("__pd_file_close"),
                            "file_exists" => self.output.push_str("__pd_file_exists"),
                            _ => self.output.push_str(name),
                        }
                    }
                    _ => {
                        return Err(CompileError::Generic(
                            "Indirect calls not yet supported".to_string()
                        ));
                    }
                }
                
                // Generate arguments
                self.output.push_str("(");
                
                // Get function signature to check parameter mutability
                let func_params = match func.as_ref() {
                    Expr::Ident(name) => {
                        self.functions.get(name).map(|(params, _)| params.clone())
                    }
                    _ => None,
                };
                
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    
                    // Check if this parameter is mutable
                    let needs_address = if let Some(params) = &func_params {
                        if i < params.len() && params[i].mutable {
                            // Need to pass address for mutable parameters
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    
                    if needs_address {
                        // Check if argument is already a pointer (mutable param) or array
                        if let Expr::Ident(name) = arg {
                            if self.mutable_params.get(name).copied().unwrap_or(false) {
                                // Already a pointer, just pass it
                                self.output.push_str(name);
                            } else {
                                // Check if it's an array variable - arrays are already pointers
                                let var_type = self.variables.get(name).map(|s| s.as_str());
                                if var_type.map_or(false, |t| t.contains("[")) {
                                    // It's an array, don't take address
                                    self.generate_expression(arg)?;
                                } else {
                                    // Need to take address
                                    self.output.push_str("&");
                                    self.generate_expression(arg)?;
                                }
                            }
                        } else {
                            // Need to take address
                            self.output.push_str("&");
                            self.generate_expression(arg)?;
                        }
                    } else {
                        self.generate_expression(arg)?;
                    }
                }
                self.output.push_str(")");
            }
            Expr::Binary { left, op, right, .. } => {
                // Check if this is string concatenation
                let left_type = self.infer_expr_type(left);
                let right_type = self.infer_expr_type(right);
                
                if matches!(op, BinOp::Add) && left_type == "const char*" && right_type == "const char*" {
                    // String concatenation - use helper function
                    self.output.push_str("__pd_string_concat(");
                    self.generate_expression(left)?;
                    self.output.push_str(", ");
                    self.generate_expression(right)?;
                    self.output.push_str(")");
                } else {
                    // Regular binary operation
                    self.output.push_str("(");
                    
                    // Generate left operand
                    self.generate_expression(left)?;
                    
                    // Generate operator
                    let op_str = match op {
                        BinOp::Add => " + ",
                        BinOp::Sub => " - ",
                        BinOp::Mul => " * ",
                        BinOp::Div => " / ",
                        BinOp::Mod => " % ",
                        BinOp::Eq => " == ",
                        BinOp::Ne => " != ",
                        BinOp::Lt => " < ",
                        BinOp::Gt => " > ",
                        BinOp::Le => " <= ",
                        BinOp::Ge => " >= ",
                        BinOp::And => " && ",
                        BinOp::Or => " || ",
                    };
                    self.output.push_str(op_str);
                    
                    // Generate right operand
                    self.generate_expression(right)?;
                    
                    self.output.push_str(")");
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                // Generate array literal: {1, 2, 3}
                self.output.push_str("{");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.generate_expression(elem)?;
                }
                self.output.push_str("}");
            }
            Expr::ArrayRepeat { value, count, .. } => {
                // Generate array repeat initialization
                // For [0; 10], generate: {0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
                self.output.push_str("{");
                if let Expr::Integer(n) = count.as_ref() {
                    for i in 0..*n {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.generate_expression(value)?;
                    }
                }
                self.output.push_str("}");
            }
            Expr::Index { array, index, .. } => {
                // Generate array indexing: arr[i]
                self.generate_expression(array)?;
                self.output.push_str("[");
                self.generate_expression(index)?;
                self.output.push_str("]");
            }
            Expr::StructLiteral { name, fields, .. } => {
                // Generate struct literal: (StructName){.field1 = value1, .field2 = value2}
                self.output.push_str(&format!("({})", name));
                self.output.push_str("{");
                for (i, (field_name, field_expr)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&format!(".{} = ", field_name));
                    self.generate_expression(field_expr)?;
                }
                self.output.push_str("}");
            }
            Expr::FieldAccess { object, field, .. } => {
                // Check if object is a mutable parameter (pointer)
                let use_arrow = match object.as_ref() {
                    Expr::Ident(name) => {
                        self.mutable_params.get(name).copied().unwrap_or(false)
                    }
                    _ => false,
                };
                
                // Generate field access: obj.field or obj->field
                // Note: Don't generate expression for object if it's a mutable param
                // because we already handle the dereference in Expr::Ident
                if use_arrow {
                    // For mutable params, we need special handling
                    if let Expr::Ident(name) = object.as_ref() {
                        self.output.push_str(&format!("{}->{}", name, field));
                    } else {
                        self.generate_expression(object)?;
                        self.output.push_str(&format!("->{}", field));
                    }
                } else {
                    self.generate_expression(object)?;
                    self.output.push_str(&format!(".{}", field));
                }
            }
            Expr::EnumConstructor { enum_name, variant, data, .. } => {
                // TODO: Properly generate enum constructors for C
                // For now, just generate a placeholder
                self.output.push_str(&format!("/* enum {}::{}", enum_name, variant));
                if data.is_some() {
                    self.output.push_str(" with data");
                }
                self.output.push_str(" */ 0");
            }
            Expr::Range { .. } => {
                // Range expressions are not directly translatable to C
                // They should only appear in for loops which handle them specially
                return Err(CompileError::Generic(
                    "Range expressions can only be used in for loops".to_string()
                ));
            }
            Expr::Unary { op, operand, .. } => {
                // Generate unary expression
                match op {
                    UnaryOp::Neg => {
                        self.output.push_str("(-(");
                        self.generate_expression(operand)?;
                        self.output.push_str("))");
                    }
                    UnaryOp::Not => {
                        self.output.push_str("(!(");
                        self.generate_expression(operand)?;
                        self.output.push_str("))");
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Write the generated code to a file
    pub fn write_output(&self) -> Result<PathBuf> {
        // Create build_output directory if it doesn't exist
        let build_dir = PathBuf::from("build_output");
        if !build_dir.exists() {
            fs::create_dir_all(&build_dir)?;
        }
        
        // Clean module name (remove .pd extension if present)
        let base_name = Path::new(&self.module_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&self.module_name);
            
        let output_path = build_dir.join(format!("{}.c", base_name));
        let mut file = File::create(&output_path)?;
        file.write_all(self.output.as_bytes())?;
        
        println!("   Generated C code: {}", output_path.display());
        
        Ok(output_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    
    #[test]
    fn test_codegen_hello_world() {
        let source = r#"
        fn main() {
            print("Hello, World!");
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());
        
        // Check generated code contains expected elements
        assert!(codegen.output.contains("int main()"));
        assert!(codegen.output.contains("__pd_print(\"Hello, World!\")"));
    }
    
    #[test]
    fn test_codegen_let_binding() {
        let source = r#"
        fn main() {
            let x: i32 = 42;
            let y = 100;
            print_int(x);
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());
        
        // Check generated code contains expected elements
        assert!(codegen.output.contains("int x = 42;"));
        assert!(codegen.output.contains("long long y = 100;"));
        assert!(codegen.output.contains("__pd_print_int(x)"));
    }
    
    #[test]
    fn test_codegen_binary_operations() {
        let source = r#"
        fn main() {
            let x = 10;
            let y = 20;
            let sum = x + y;
            let product = x * y;
            print_int(sum);
            print_int(product);
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());
        
        // Check generated code contains expected elements
        assert!(codegen.output.contains("long long sum = (x + y);"));
        assert!(codegen.output.contains("long long product = (x * y);"));
        assert!(codegen.output.contains("__pd_print_int(sum)"));
        assert!(codegen.output.contains("__pd_print_int(product)"));
    }
    
    #[test]
    fn test_codegen_comparison_operations() {
        let source = r#"
        fn main() -> i32 {
            let a = 5;
            let b = 10;
            let result = a < b;
            return result;
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());
        
        // Check generated code contains expected elements
        assert!(codegen.output.contains("int main()"));
        assert!(codegen.output.contains("long long result = (a < b);"));
        assert!(codegen.output.contains("return result;"));
    }
    
    #[test]
    fn test_codegen_for_loop() {
        let source = r#"
        fn main() {
            let arr = [1, 2, 3, 4, 5];
            for i in arr {
                print_int(i);
            }
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());
        
        // Check generated code contains for loop
        assert!(codegen.output.contains("for (long long _i = 0;"));
        assert!(codegen.output.contains("long long i = arr[_i];"));
        assert!(codegen.output.contains("__pd_print_int(i)"));
    }
    
    #[test]
    fn test_codegen_break_continue() {
        let source = r#"
        fn main() {
            while true {
                if x > 10 {
                    break;
                }
                if x == 5 {
                    continue;
                }
            }
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());
        
        // Check generated code contains break and continue
        assert!(codegen.output.contains("break;"));
        assert!(codegen.output.contains("continue;"));
    }
}