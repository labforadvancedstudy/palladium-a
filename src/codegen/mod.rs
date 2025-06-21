// Code generation for Palladium
// "Forging legends into machine code"

pub mod llvm_backend;
pub mod llvm_backend_improved;
pub mod llvm_native;
pub mod llvm_text_backend;

use crate::ast::{AssignTarget, UnaryOp, *};
use crate::errors::{CompileError, Result, Span};
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
    /// Imported modules
    imported_modules: std::collections::HashMap<String, crate::resolver::ModuleInfo>,
    /// Generic function instantiations to generate
    generic_instantiations: Vec<(String, Vec<String>, crate::typeck::GenericFunction)>,
    /// Generic struct instantiations to generate
    generic_struct_instantiations: Vec<(String, Vec<String>, crate::typeck::GenericStruct)>,
    /// Type aliases for resolving custom types
    type_aliases: std::collections::HashMap<String, Type>,
    /// Counter for generating unique temporary variable names
    temp_counter: usize,
    /// Map of enum names to their definitions
    enums: std::collections::HashMap<String, EnumDef>,
    /// Map from original generic struct name to list of instantiations
    /// e.g., "Box" -> [("i64", "Box_i64"), ("bool", "Box_bool")]
    generic_struct_instantiation_map: std::collections::HashMap<String, Vec<(Vec<String>, String)>>,
    /// Set of async function names
    async_functions: std::collections::HashSet<String>,
}

impl CodeGenerator {
    pub fn new(module_name: &str) -> Result<Self> {
        // Pre-allocate string capacity for better performance
        let initial_capacity = 64 * 1024; // 64KB initial capacity
        Ok(Self {
            module_name: module_name.to_string(),
            output: String::with_capacity(initial_capacity),
            functions: std::collections::HashMap::new(),
            variables: std::collections::HashMap::new(),
            mutable_params: std::collections::HashMap::new(),
            imported_modules: std::collections::HashMap::new(),
            generic_instantiations: Vec::new(),
            generic_struct_instantiations: Vec::new(),
            type_aliases: std::collections::HashMap::new(),
            temp_counter: 0,
            enums: std::collections::HashMap::new(),
            generic_struct_instantiation_map: std::collections::HashMap::new(),
            async_functions: std::collections::HashSet::new(),
        })
    }

    /// Set imported modules for code generation
    pub fn set_imported_modules(
        &mut self,
        modules: std::collections::HashMap<String, crate::resolver::ModuleInfo>,
    ) {
        self.imported_modules = modules;
    }

    /// Set generic function instantiations for code generation
    pub fn set_generic_instantiations(
        &mut self,
        instantiations: Vec<(String, Vec<String>, crate::typeck::GenericFunction)>,
    ) {
        self.generic_instantiations = instantiations;
    }

    /// Set generic struct instantiations for code generation
    pub fn set_generic_struct_instantiations(
        &mut self,
        instantiations: Vec<(String, Vec<String>, crate::typeck::GenericStruct)>,
    ) {
        self.generic_struct_instantiations = instantiations;
    }

    /// Infer the C type of an expression
    fn infer_expr_type(&self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(_) => "long long".to_string(),
            Expr::String(_) => "const char*".to_string(),
            Expr::Bool(_) => "int".to_string(),
            Expr::StructLiteral { name, fields, .. } => {
                // Check if this is a generic struct instantiation
                if let Some(instantiations) = self.generic_struct_instantiation_map.get(name) {
                    // Need to determine which instantiation to use based on field types
                    // For now, we'll infer from the first field's type
                    if let Some((_, field_expr)) = fields.first() {
                        let field_type = match field_expr {
                            Expr::Integer(_) => "long long",
                            Expr::String(_) => "const char*",
                            Expr::Bool(_) => "int",
                            _ => "long long",
                        };

                        // Find the matching instantiation
                        for (type_args, mangled_name) in instantiations {
                            // Simple heuristic: check if any type arg matches the field type
                            for type_arg in type_args {
                                if (type_arg == "i64" && field_type.contains("long long"))
                                    || (type_arg == "bool" && field_type == "int")
                                    || (type_arg == "String" && field_type.contains("char*"))
                                {
                                    return format!("struct {}", mangled_name);
                                }
                            }
                        }
                    }
                    format!("struct {}", name)
                } else {
                    format!("struct {}", name)
                }
            }
            Expr::Ident(name) => {
                // Look up variable type
                self.variables
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| "long long".to_string())
            }
            Expr::Call { func, .. } => {
                // Look up function return type
                if let Expr::Ident(func_name) = func.as_ref() {
                    // Check built-in functions that return strings
                    match func_name.as_str() {
                        "string_concat" | "string_substring" | "string_from_char"
                        | "int_to_string" | "file_read_all" | "file_read_line" | "trim"
                        | "trim_start" | "trim_end" => return "const char*".to_string(),
                        _ => {}
                    }

                    // Look up user-defined function return type
                    if let Some((_params, ret_type)) = self.functions.get(func_name) {
                        // Check if this is an async function
                        if self.async_functions.contains(func_name) {
                            return format!("{}_Future", func_name);
                        }

                        match ret_type {
                            Some(Type::String) => return "const char*".to_string(),
                            Some(Type::Bool) => return "int".to_string(),
                            Some(Type::Custom(name)) => return name.to_string(),
                            Some(Type::Reference { inner: _, .. }) => {
                                return format!(
                                    "{}*",
                                    self.infer_expr_type(&Expr::Ident("dummy".to_string()))
                                );
                            }
                            _ => return "long long".to_string(),
                        }
                    }
                }
                "long long".to_string()
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                // String concatenation returns a string
                if matches!(op, BinOp::Add) {
                    let left_type = self.infer_expr_type(left);
                    let right_type = self.infer_expr_type(right);
                    if left_type == "const char*" && right_type == "const char*" {
                        return "const char*".to_string();
                    }
                }
                "long long".to_string()
            }
            Expr::EnumConstructor { enum_name, .. } => enum_name.to_string(),
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
        self.output.push_str("#include <ctype.h>\n");
        self.output.push_str("#include <stdint.h>\n\n");

        // Memory management for strings
        self.output
            .push_str("// String memory pool to prevent leaks\n");
        self.output.push_str("#define STRING_POOL_SIZE 65536\n");
        self.output.push_str("#define MAX_STRINGS 1024\n");
        self.output
            .push_str("static char __pd_string_pool[STRING_POOL_SIZE];\n");
        self.output
            .push_str("static size_t __pd_string_pool_offset = 0;\n");
        self.output
            .push_str("static char* __pd_allocated_strings[MAX_STRINGS];\n");
        self.output.push_str("static int __pd_num_strings = 0;\n\n");

        // String allocation function
        self.output
            .push_str("static char* __pd_alloc_string(size_t size) {\n");
        self.output
            .push_str("    if (__pd_string_pool_offset + size > STRING_POOL_SIZE) {\n");
        self.output
            .push_str("        // Pool exhausted, fall back to malloc\n");
        self.output
            .push_str("        char* ptr = (char*)malloc(size);\n");
        self.output
            .push_str("        if (__pd_num_strings < MAX_STRINGS) {\n");
        self.output
            .push_str("            __pd_allocated_strings[__pd_num_strings++] = ptr;\n");
        self.output.push_str("        }\n");
        self.output.push_str("        return ptr;\n");
        self.output.push_str("    }\n");
        self.output
            .push_str("    char* ptr = &__pd_string_pool[__pd_string_pool_offset];\n");
        self.output
            .push_str("    __pd_string_pool_offset += size;\n");
        self.output.push_str("    return ptr;\n");
        self.output.push_str("}\n\n");

        // Cleanup function
        self.output
            .push_str("static void __pd_cleanup_strings() {\n");
        self.output
            .push_str("    for (int i = 0; i < __pd_num_strings; i++) {\n");
        self.output
            .push_str("        free(__pd_allocated_strings[i]);\n");
        self.output.push_str("    }\n");
        self.output.push_str("    __pd_num_strings = 0;\n");
        self.output.push_str("    __pd_string_pool_offset = 0;\n");
        self.output.push_str("}\n\n");

        // Register cleanup with atexit
        self.output
            .push_str("static void __pd_init() __attribute__((constructor));\n");
        self.output.push_str("static void __pd_init() {\n");
        self.output.push_str("    atexit(__pd_cleanup_strings);\n");
        self.output.push_str("}\n\n");

        // Generate print function wrapper
        self.output.push_str("void __pd_print(const char* str) {\n");
        self.output.push_str("    printf(\"%s\\n\", str);\n");
        self.output.push_str("}\n\n");

        // Generate print_int function wrapper
        self.output
            .push_str("void __pd_print_int(long long value) {\n");
        self.output.push_str("    printf(\"%lld\\n\", value);\n");
        self.output.push_str("}\n\n");

        // Generate panic function wrapper
        self.output.push_str("void __pd_panic(const char* msg) {\n");
        self.output.push_str("    fprintf(stderr, \"panic: %s\\n\", msg);\n");
        self.output.push_str("    abort();\n");
        self.output.push_str("}\n\n");

        // Generate string manipulation functions

        // string_len
        self.output
            .push_str("long long __pd_string_len(const char* str) {\n");
        self.output.push_str("    return strlen(str);\n");
        self.output.push_str("}\n\n");

        // string_concat
        self.output
            .push_str("const char* __pd_string_concat(const char* s1, const char* s2) {\n");
        self.output.push_str("    size_t len1 = strlen(s1);\n");
        self.output.push_str("    size_t len2 = strlen(s2);\n");
        self.output
            .push_str("    char* result = __pd_alloc_string(len1 + len2 + 1);\n");
        self.output.push_str("    strcpy(result, s1);\n");
        self.output.push_str("    strcat(result, s2);\n");
        self.output.push_str("    return result;\n");
        self.output.push_str("}\n\n");

        // string_eq
        self.output
            .push_str("int __pd_string_eq(const char* s1, const char* s2) {\n");
        self.output.push_str("    return strcmp(s1, s2) == 0;\n");
        self.output.push_str("}\n\n");

        // string_char_at
        self.output
            .push_str("long long __pd_string_char_at(const char* str, long long index) {\n");
        self.output
            .push_str("    if (index < 0 || index >= (long long)strlen(str)) return -1;\n");
        self.output
            .push_str("    return (long long)(unsigned char)str[index];\n");
        self.output.push_str("}\n\n");

        // string_substring
        self.output.push_str("const char* __pd_string_substring(const char* str, long long start, long long end) {\n");
        self.output.push_str("    size_t len = strlen(str);\n");
        self.output.push_str("    if (start < 0) start = 0;\n");
        self.output
            .push_str("    if (end > (long long)len) end = len;\n");
        self.output.push_str("    if (start >= end) return \"\";\n");
        self.output.push_str("    size_t sub_len = end - start;\n");
        self.output
            .push_str("    char* result = __pd_alloc_string(sub_len + 1);\n");
        self.output
            .push_str("    strncpy(result, str + start, sub_len);\n");
        self.output.push_str("    result[sub_len] = '\\0';\n");
        self.output.push_str("    return result;\n");
        self.output.push_str("}\n\n");

        // string_from_char
        self.output
            .push_str("const char* __pd_string_from_char(long long c) {\n");
        self.output
            .push_str("    char* result = __pd_alloc_string(2);\n");
        self.output.push_str("    result[0] = (char)c;\n");
        self.output.push_str("    result[1] = '\\0';\n");
        self.output.push_str("    return result;\n");
        self.output.push_str("}\n\n");

        // char_is_digit
        self.output
            .push_str("int __pd_char_is_digit(long long c) {\n");
        self.output.push_str("    return isdigit((int)c);\n");
        self.output.push_str("}\n\n");

        // char_is_alpha
        self.output
            .push_str("int __pd_char_is_alpha(long long c) {\n");
        self.output.push_str("    return isalpha((int)c);\n");
        self.output.push_str("}\n\n");

        // char_is_whitespace
        self.output
            .push_str("int __pd_char_is_whitespace(long long c) {\n");
        self.output.push_str("    return isspace((int)c);\n");
        self.output.push_str("}\n\n");

        // string_to_int
        self.output
            .push_str("long long __pd_string_to_int(const char* str) {\n");
        self.output.push_str("    return atoll(str);\n");
        self.output.push_str("}\n\n");

        // int_to_string
        self.output
            .push_str("const char* __pd_int_to_string(long long n) {\n");
        self.output
            .push_str("    char* buffer = __pd_alloc_string(32);\n");
        self.output
            .push_str("    snprintf(buffer, 32, \"%lld\", n);\n");
        self.output.push_str("    return buffer;\n");
        self.output.push_str("}\n\n");

        // File I/O functions
        self.output.push_str("// File I/O support\n");
        self.output.push_str("#define MAX_FILES 256\n");
        self.output
            .push_str("static FILE* __pd_file_handles[MAX_FILES] = {0};\n");
        self.output.push_str("static int __pd_next_handle = 1;\n\n");

        // file_open
        self.output
            .push_str("long long __pd_file_open(const char* path) {\n");
        self.output
            .push_str("    if (__pd_next_handle >= MAX_FILES) return -1;\n");
        self.output.push_str("    FILE* f = fopen(path, \"r+\");\n");
        self.output
            .push_str("    if (!f) f = fopen(path, \"w+\");\n");
        self.output.push_str("    if (!f) return -1;\n");
        self.output
            .push_str("    int handle = __pd_next_handle++;\n");
        self.output.push_str("    __pd_file_handles[handle] = f;\n");
        self.output.push_str("    return handle;\n");
        self.output.push_str("}\n\n");

        // file_read_all
        self.output
            .push_str("const char* __pd_file_read_all(long long handle) {\n");
        self.output.push_str("    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return \"\";\n");
        self.output
            .push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output.push_str("    fseek(f, 0, SEEK_END);\n");
        self.output.push_str("    long size = ftell(f);\n");
        self.output.push_str("    fseek(f, 0, SEEK_SET);\n");
        self.output
            .push_str("    char* buffer = __pd_alloc_string(size + 1);\n");
        self.output.push_str("    fread(buffer, 1, size, f);\n");
        self.output.push_str("    buffer[size] = '\\0';\n");
        self.output.push_str("    return buffer;\n");
        self.output.push_str("}\n\n");

        // file_read_line
        self.output
            .push_str("const char* __pd_file_read_line(long long handle) {\n");
        self.output.push_str("    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return \"\";\n");
        self.output.push_str("    static char line_buffer[4096];\n");
        self.output
            .push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output
            .push_str("    if (fgets(line_buffer, sizeof(line_buffer), f)) {\n");
        self.output
            .push_str("        size_t len = strlen(line_buffer);\n");
        self.output.push_str(
            "        if (len > 0 && line_buffer[len-1] == '\\n') line_buffer[len-1] = '\\0';\n",
        );
        self.output
            .push_str("        char* result = __pd_alloc_string(len + 1);\n");
        self.output
            .push_str("        strcpy(result, line_buffer);\n");
        self.output.push_str("        return result;\n");
        self.output.push_str("    }\n");
        self.output.push_str("    return \"\";\n");
        self.output.push_str("}\n\n");

        // file_write
        self.output
            .push_str("int __pd_file_write(long long handle, const char* content) {\n");
        self.output.push_str(
            "    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return 0;\n",
        );
        self.output
            .push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output.push_str("    return fputs(content, f) >= 0;\n");
        self.output.push_str("}\n\n");

        // file_close
        self.output
            .push_str("int __pd_file_close(long long handle) {\n");
        self.output.push_str(
            "    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return 0;\n",
        );
        self.output
            .push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output
            .push_str("    __pd_file_handles[handle] = NULL;\n");
        self.output.push_str("    return fclose(f) == 0;\n");
        self.output.push_str("}\n\n");

        // file_exists
        self.output
            .push_str("int __pd_file_exists(const char* path) {\n");
        self.output.push_str("    FILE* f = fopen(path, \"r\");\n");
        self.output.push_str("    if (f) {\n");
        self.output.push_str("        fclose(f);\n");
        self.output.push_str("        return 1;\n");
        self.output.push_str("    }\n");
        self.output.push_str("    return 0;\n");
        self.output.push_str("}\n\n");

        // Enhanced I/O Runtime Function Declarations
        // External function declarations for runtime I/O
        self.output.push_str("// Enhanced I/O runtime functions\n");
        self.output.push_str("// File handle type (opaque pointer)\n");
        self.output.push_str("typedef void* FileHandle;\n\n");
        
        // File mode enum
        self.output.push_str("// File modes\n");
        self.output.push_str("enum FileMode {\n");
        self.output.push_str("    FileMode_Read = 0,\n");
        self.output.push_str("    FileMode_Write = 1,\n");
        self.output.push_str("    FileMode_Append = 2,\n");
        self.output.push_str("    FileMode_ReadWrite = 3\n");
        self.output.push_str("};\n\n");
        
        // External function declarations
        self.output.push_str("// External runtime I/O functions\n");
        self.output.push_str("extern FileHandle pd_file_open(const char* path, size_t path_len, int mode);\n");
        self.output.push_str("extern int pd_file_close(FileHandle handle);\n");
        self.output.push_str("extern int64_t pd_file_read(FileHandle handle, char* buffer, size_t len);\n");
        self.output.push_str("extern int64_t pd_file_write(FileHandle handle, const char* buffer, size_t len);\n");
        self.output.push_str("extern int64_t pd_file_seek(FileHandle handle, uint8_t whence, int64_t offset);\n");
        self.output.push_str("extern int pd_file_flush(FileHandle handle);\n");
        self.output.push_str("extern int pd_path_exists(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_path_is_file(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_path_is_dir(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_create_dir(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_create_dir_all(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_remove_file(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_remove_dir(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_remove_dir_all(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_read_file_to_string(const char* path, size_t path_len, char** out_str, size_t* out_len);\n");
        self.output.push_str("extern int pd_write_string_to_file(const char* path, size_t path_len, const char* data, size_t data_len);\n\n");
        
        // Wrapper functions that call the external pd_* functions
        // pd_file_open wrapper (enhanced version with mode)
        self.output.push_str("FileHandle __pd_file_open_ex(const char* path, int mode) {\n");
        self.output.push_str("    return pd_file_open(path, strlen(path), mode);\n");
        self.output.push_str("}\n\n");
        
        // pd_file_close wrapper (enhanced version)
        self.output.push_str("int __pd_file_close_ex(FileHandle handle) {\n");
        self.output.push_str("    return pd_file_close(handle);\n");
        self.output.push_str("}\n\n");
        
        // pd_file_read wrapper (enhanced version)
        self.output.push_str("int64_t __pd_file_read_ex(FileHandle handle, char* buffer, size_t len) {\n");
        self.output.push_str("    return pd_file_read(handle, buffer, len);\n");
        self.output.push_str("}\n\n");
        
        // pd_file_write wrapper (enhanced version)
        self.output.push_str("int64_t __pd_file_write_ex(FileHandle handle, const char* buffer, size_t len) {\n");
        self.output.push_str("    return pd_file_write(handle, buffer, len);\n");
        self.output.push_str("}\n\n");
        
        // pd_file_seek wrapper
        self.output.push_str("int64_t __pd_file_seek(FileHandle handle, uint8_t whence, int64_t offset) {\n");
        self.output.push_str("    return pd_file_seek(handle, whence, offset);\n");
        self.output.push_str("}\n\n");
        
        // pd_file_flush wrapper
        self.output.push_str("int __pd_file_flush(FileHandle handle) {\n");
        self.output.push_str("    return pd_file_flush(handle);\n");
        self.output.push_str("}\n\n");
        
        // Path manipulation functions
        self.output.push_str("int __pd_path_exists(const char* path) {\n");
        self.output.push_str("    return pd_path_exists(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_path_is_file(const char* path) {\n");
        self.output.push_str("    return pd_path_is_file(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_path_is_dir(const char* path) {\n");
        self.output.push_str("    return pd_path_is_dir(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        // Directory operations
        self.output.push_str("int __pd_create_dir(const char* path) {\n");
        self.output.push_str("    return pd_create_dir(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_create_dir_all(const char* path) {\n");
        self.output.push_str("    return pd_create_dir_all(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_remove_file(const char* path) {\n");
        self.output.push_str("    return pd_remove_file(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_remove_dir(const char* path) {\n");
        self.output.push_str("    return pd_remove_dir(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_remove_dir_all(const char* path) {\n");
        self.output.push_str("    return pd_remove_dir_all(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        // Enhanced file operations with string helpers
        self.output.push_str("char* __pd_read_file_to_string(const char* path) {\n");
        self.output.push_str("    char* out_str = NULL;\n");
        self.output.push_str("    size_t out_len = 0;\n");
        self.output.push_str("    if (pd_read_file_to_string(path, strlen(path), &out_str, &out_len) == 0) {\n");
        self.output.push_str("        return out_str;\n");
        self.output.push_str("    }\n");
        self.output.push_str("    return NULL;\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_write_string_to_file(const char* path, const char* data) {\n");
        self.output.push_str("    return pd_write_string_to_file(path, strlen(path), data, strlen(data));\n");
        self.output.push_str("}\n\n");

        // First pass: collect function signatures, type aliases, and enum definitions from imported modules
        for module_info in self.imported_modules.values() {
            for item in &module_info.ast.items {
                match item {
                    Item::Function(func) => {
                        if matches!(func.visibility, crate::ast::Visibility::Public) {
                            self.functions.insert(
                                func.name.clone(),
                                (func.params.clone(), func.return_type.clone()),
                            );
                            if func.is_async {
                                self.async_functions.insert(func.name.clone());
                            }
                        }
                    }
                    Item::TypeAlias(type_alias) => {
                        if matches!(type_alias.visibility, crate::ast::Visibility::Public) {
                            // Skip generic type aliases for now
                            if type_alias.type_params.is_empty()
                                && type_alias.lifetime_params.is_empty()
                            {
                                self.type_aliases
                                    .insert(type_alias.name.clone(), type_alias.ty.clone());
                            }
                        }
                    }
                    Item::Enum(enum_def) => {
                        // Skip generic enums for now
                        if enum_def.type_params.is_empty() && enum_def.lifetime_params.is_empty() {
                            self.enums.insert(enum_def.name.clone(), enum_def.clone());
                        }
                    }
                    Item::Macro(_) => {
                        // Macros are expanded before codegen, skip here
                    }
                    _ => {}
                }
            }
        }

        // Then collect function signatures, type aliases, and enum definitions from main program
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.functions.insert(
                        func.name.clone(),
                        (func.params.clone(), func.return_type.clone()),
                    );
                    if func.is_async {
                        self.async_functions.insert(func.name.clone());
                    }
                }
                Item::TypeAlias(type_alias) => {
                    // Skip generic type aliases for now
                    if type_alias.type_params.is_empty() && type_alias.lifetime_params.is_empty() {
                        self.type_aliases
                            .insert(type_alias.name.clone(), type_alias.ty.clone());
                    }
                }
                Item::Enum(enum_def) => {
                    // Skip generic enums for now
                    if enum_def.type_params.is_empty() && enum_def.lifetime_params.is_empty() {
                        self.enums.insert(enum_def.name.clone(), enum_def.clone());
                    }
                }
                Item::Macro(_) => {
                    // Macros are expanded before codegen, skip here
                }
                _ => {}
            }
        }

        // Generate struct definitions from imported modules first
        let imported_modules = self.imported_modules.clone();
        for module_info in imported_modules.values() {
            for item in &module_info.ast.items {
                match item {
                    Item::Struct(struct_def) => {
                        if matches!(struct_def.visibility, crate::ast::Visibility::Public) {
                            // Skip generic structs - they should only be generated when instantiated
                            if struct_def.type_params.is_empty()
                                && struct_def.lifetime_params.is_empty()
                            {
                                self.generate_struct(struct_def)?;
                            }
                        }
                    }
                    Item::Enum(enum_def) => {
                        // Skip generic enums - they should only be generated when instantiated
                        if enum_def.type_params.is_empty() && enum_def.lifetime_params.is_empty() {
                            self.generate_enum(enum_def)?;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Generate struct and enum definitions from main program
        for item in &program.items {
            match item {
                Item::Struct(struct_def) => {
                    // Skip generic structs - they should only be generated when instantiated
                    if struct_def.type_params.is_empty() && struct_def.lifetime_params.is_empty() {
                        self.generate_struct(struct_def)?;
                    }
                }
                Item::Enum(enum_def) => {
                    // Skip generic enums - they should only be generated when instantiated
                    if enum_def.type_params.is_empty() && enum_def.lifetime_params.is_empty() {
                        self.generate_enum(enum_def)?;
                    }
                }
                _ => {}
            }
        }

        // Generate monomorphized versions of generic structs FIRST
        if !self.generic_struct_instantiations.is_empty() {
            self.output.push_str("// Monomorphized generic structs\n");

            for (struct_name, type_args, generic_struct) in
                &self.generic_struct_instantiations.clone()
            {
                // Create a concrete struct from the generic template
                let concrete_struct =
                    self.monomorphize_struct(struct_name, type_args, generic_struct)?;

                // Track the instantiation mapping for struct literal generation
                let instantiations = self
                    .generic_struct_instantiation_map
                    .entry(struct_name.clone())
                    .or_default();
                instantiations.push((type_args.clone(), concrete_struct.name.clone()));

                self.generate_struct(&concrete_struct)?;
            }
            self.output.push('\n');
        }

        // Generate monomorphized versions of generic functions AFTER structs
        if !self.generic_instantiations.is_empty() {
            self.output.push_str("// Monomorphized generic functions\n");

            for (func_name, type_args, generic_func) in &self.generic_instantiations.clone() {
                // Create a concrete function from the generic template
                let concrete_func =
                    self.monomorphize_function(func_name, type_args, generic_func)?;
                self.generate_function(&concrete_func)?;
            }
            self.output.push('\n');
        }

        // Generate functions from imported modules
        for module_info in imported_modules.values() {
            for item in &module_info.ast.items {
                if let Item::Function(func) = item {
                    // Only generate public, non-generic functions
                    if matches!(func.visibility, crate::ast::Visibility::Public)
                        && func.type_params.is_empty()
                    {
                        self.generate_function(func)?;
                    }
                }
            }
        }

        // Generate functions from main program
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
                    // Enum definitions already generated above
                }
                Item::Trait(_) => {
                    // Traits don't generate C code directly
                    // They are used for type checking only
                }
                Item::TypeAlias(_) => {
                    // Type aliases don't generate C code
                    // They are resolved during type checking
                }
                Item::Impl(impl_block) => {
                    // Generate methods from impl blocks
                    for method in &impl_block.methods {
                        if !method.type_params.is_empty() {
                            continue;
                        }
                        // Create a mangled method name
                        let mangled_name = format!(
                            "__pd_{}_{}",
                            impl_block.for_type.to_string().replace("::", "_"),
                            method.name
                        );
                        self.generate_function_with_name(method, &mangled_name)?;
                    }
                }
                Item::Macro(_) => {
                    // Macros are expanded before codegen, skip here
                }
            }
        }

        Ok(())
    }

    /// Convert Type to C type string, resolving type aliases
    fn type_to_c(&self, ty: &Type) -> String {
        match ty {
            Type::I32 => "int".to_string(),
            Type::I64 => "long long".to_string(),
            Type::U32 => "unsigned int".to_string(),
            Type::U64 => "unsigned long long".to_string(),
            Type::Bool => "int".to_string(),
            Type::String => "const char*".to_string(),
            Type::Unit => "void".to_string(),
            Type::Array(elem_type, size) => {
                let size_str = match size {
                    ArraySize::Literal(n) => n.to_string(),
                    ArraySize::ConstParam(name) => name.clone(),
                    ArraySize::Expr(_) => "0".to_string(), // TODO: evaluate expression
                };
                format!("{}[{}]", self.type_to_c(elem_type), size_str)
            }
            Type::Custom(name) => {
                // First check if it's a type alias
                if let Some(aliased_type) = self.type_aliases.get(name) {
                    // Recursively resolve the aliased type
                    self.type_to_c(aliased_type)
                } else {
                    // Otherwise it's a struct or enum name
                    // In C, structs need the "struct" prefix
                    format!("struct {}", name)
                }
            }
            Type::TypeParam(_) | Type::Generic { .. } => {
                // TODO: Proper generic handling
                "void*".to_string() // Placeholder
            }
            Type::Reference { inner, .. } => {
                // References compile to pointers in C
                format!("{}*", self.type_to_c(inner))
            }
            Type::Future { output } => {
                // Futures compile to a struct with state and result
                format!("Future_{}", self.type_to_c(output))
            }
            Type::Tuple(_) => {
                // Tuples not yet supported in C codegen
                "void*".to_string() // TODO: Generate struct for tuple
            }
        }
    }

    /// Generate code for an enum definition
    fn generate_enum(&mut self, enum_def: &EnumDef) -> Result<()> {
        // Generate a tagged union for the enum
        self.output.push_str(&format!(
            "// Enum {}
",
            enum_def.name
        ));

        // First, generate the tag enum
        self.output.push_str("typedef enum {\n");
        for variant in &enum_def.variants {
            self.output.push_str(&format!(
                "    __{}__{},
",
                enum_def.name, variant.name
            ));
        }
        self.output.push_str(&format!(
            "}} {}Tag;
\n",
            enum_def.name
        ));

        // Generate data structs for variants with data
        for variant in &enum_def.variants {
            match &variant.data {
                EnumVariantData::Unit => {
                    // Unit variants don't need data structs
                }
                EnumVariantData::Tuple(types) => {
                    if !types.is_empty() {
                        self.output.push_str("typedef struct {\n");
                        for (i, ty) in types.iter().enumerate() {
                            let c_type = self.type_to_c(ty);
                            self.output.push_str(&format!(
                                "    {} field{};
",
                                c_type, i
                            ));
                        }
                        self.output.push_str(&format!(
                            "}} {}__{}_Data;
\n",
                            enum_def.name, variant.name
                        ));
                    }
                }
                EnumVariantData::Struct(fields) => {
                    self.output.push_str("typedef struct {\n");
                    for (field_name, field_type) in fields {
                        let c_type = self.type_to_c(field_type);
                        self.output.push_str(&format!(
                            "    {} {};
",
                            c_type, field_name
                        ));
                    }
                    self.output.push_str(&format!(
                        "}} {}__{}_Data;
\n",
                        enum_def.name, variant.name
                    ));
                }
            }
        }

        // Generate the enum struct with tag and union
        self.output.push_str(&format!(
            "typedef struct {} {{
",
            enum_def.name
        ));
        self.output.push_str(&format!(
            "    {}Tag tag;
",
            enum_def.name
        ));

        // Only generate union if there are variants with data
        let has_data_variants = enum_def
            .variants
            .iter()
            .any(|v| !matches!(v.data, EnumVariantData::Unit));
        if has_data_variants {
            self.output.push_str(
                "    union {
",
            );
            for variant in &enum_def.variants {
                match &variant.data {
                    EnumVariantData::Unit => {
                        // Unit variants don't have data in the union
                    }
                    EnumVariantData::Tuple(types) => {
                        if !types.is_empty() {
                            self.output.push_str(&format!(
                                "        {}__{}_Data {};
",
                                enum_def.name,
                                variant.name,
                                variant.name.to_lowercase()
                            ));
                        }
                    }
                    EnumVariantData::Struct(_) => {
                        self.output.push_str(&format!(
                            "        {}__{}_Data {};
",
                            enum_def.name,
                            variant.name,
                            variant.name.to_lowercase()
                        ));
                    }
                }
            }
            self.output.push_str(
                "    } data;
",
            );
        }

        self.output.push_str(&format!(
            "}} {};
\n",
            enum_def.name
        ));

        // Generate constructor functions for each variant
        for variant in &enum_def.variants {
            match &variant.data {
                EnumVariantData::Unit => {
                    // Unit variant constructor
                    self.output.push_str(&format!(
                        "static inline {} {}_{}() {{
",
                        enum_def.name, enum_def.name, variant.name
                    ));
                    self.output.push_str(&format!(
                        "    {} result = {{.tag = __{}__{}}};
",
                        enum_def.name, enum_def.name, variant.name
                    ));
                    self.output.push_str(
                        "    return result;
",
                    );
                    self.output.push_str("}\n\n");
                }
                EnumVariantData::Tuple(types) => {
                    // Tuple variant constructor
                    self.output.push_str(&format!(
                        "static inline {} {}_{}__new(",
                        enum_def.name, enum_def.name, variant.name
                    ));

                    for (i, ty) in types.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        let c_type = self.type_to_c(ty);
                        self.output.push_str(&format!("{} arg{}", c_type, i));
                    }

                    self.output.push_str(") {\n");
                    self.output.push_str(&format!(
                        "    {} result = {{.tag = __{}__{}}};
",
                        enum_def.name, enum_def.name, variant.name
                    ));

                    if !types.is_empty() {
                        for i in 0..types.len() {
                            self.output.push_str(&format!(
                                "    result.data.{}.field{} = arg{};
",
                                variant.name.to_lowercase(),
                                i,
                                i
                            ));
                        }
                    }

                    self.output.push_str(
                        "    return result;
",
                    );
                    self.output.push_str("}\n\n");
                }
                EnumVariantData::Struct(fields) => {
                    // Struct variant constructor
                    self.output.push_str(&format!(
                        "static inline {} {}_{}__new(",
                        enum_def.name, enum_def.name, variant.name
                    ));

                    for (i, (field_name, field_type)) in fields.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        let c_type = self.type_to_c(field_type);
                        self.output.push_str(&format!("{} {}", c_type, field_name));
                    }

                    self.output.push_str(") {\n");
                    self.output.push_str(&format!(
                        "    {} result = {{.tag = __{}__{}}};
",
                        enum_def.name, enum_def.name, variant.name
                    ));

                    for (field_name, _) in fields {
                        self.output.push_str(&format!(
                            "    result.data.{}.{} = {};
",
                            variant.name.to_lowercase(),
                            field_name,
                            field_name
                        ));
                    }

                    self.output.push_str(
                        "    return result;
",
                    );
                    self.output.push_str("}\n\n");
                }
            }
        }

        Ok(())
    }

    /// Generate code for a struct definition
    fn generate_struct(&mut self, struct_def: &StructDef) -> Result<()> {
        self.output
            .push_str(&format!("typedef struct {} {{\n", struct_def.name));

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
                    let elem_c_type = self.type_to_c(elem_type.as_ref());
                    let size_str = match size {
                        ArraySize::Literal(n) => n.to_string(),
                        ArraySize::ConstParam(name) => name.clone(),
                        ArraySize::Expr(_) => "0".to_string(), // TODO: evaluate expression
                    };
                    self.output
                        .push_str(&format!("{} {}[{}];\n", elem_c_type, field_name, size_str));
                    continue;
                }
                Type::Unit => "void",
                Type::Custom(_name) => {
                    // Use type_to_c to resolve type aliases
                    let resolved_type = self.type_to_c(field_type);
                    self.output
                        .push_str(&format!("{} {};\n", resolved_type, field_name));
                    continue;
                }
                Type::TypeParam(_) | Type::Generic { .. } => {
                    return Err(CompileError::Generic(
                        "Generic types in structs not yet supported".to_string(),
                    ));
                }
                Type::Reference { .. } => {
                    return Err(CompileError::Generic(
                        "Reference types in structs not yet supported".to_string(),
                    ));
                }
                Type::Future { .. } => {
                    return Err(CompileError::Generic(
                        "Future types in structs not yet supported".to_string(),
                    ));
                }
                Type::Tuple(_) => {
                    return Err(CompileError::Generic(
                        "Tuple types in structs not yet supported".to_string(),
                    ));
                }
            };

            self.output
                .push_str(&format!("{} {};\n", c_type, field_name));
        }

        self.output
            .push_str(&format!("}} {};\n\n", struct_def.name));
        Ok(())
    }

    /// Generate code for a function
    fn generate_function(&mut self, func: &Function) -> Result<()> {
        self.generate_function_with_name(func, &func.name)
    }

    fn generate_function_with_name(&mut self, func: &Function, name: &str) -> Result<()> {
        // For async functions, generate a Future-returning wrapper
        if func.is_async {
            self.generate_async_function_with_name(func, name)?;
            return Ok(());
        }

        // Function signature with return type
        let return_type_string = match &func.return_type {
            Some(Type::Array(_, _)) => {
                // Arrays cannot be returned by value in C, would need to return pointer
                return Err(CompileError::Generic(
                    "Returning arrays from functions is not yet supported".to_string(),
                ));
            }
            Some(t) => self.type_to_c(t),
            None => "void".to_string(),
        };

        let return_type = return_type_string.as_str();

        // Special case: main always returns int in C
        let actual_return_type = if name == "main" && return_type == "void" {
            "int"
        } else {
            return_type
        };

        // Generate function parameters
        self.output
            .push_str(&format!("{} {}(", actual_return_type, name));

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
                        Type::String => "char*", // String arrays are arrays of char pointers
                        Type::Custom(name) => name.as_str(), // Support struct arrays
                        _ => {
                            return Err(CompileError::Generic(format!(
                                "Unsupported array element type in function parameter: {:?}",
                                elem_type
                            )))
                        }
                    };
                    // In C, array parameters are passed as pointers
                    // We'll generate: type name[size] for clarity, though it decays to pointer
                    let size_str = match size {
                        ArraySize::Literal(n) => n.to_string(),
                        ArraySize::ConstParam(name) => name.clone(),
                        ArraySize::Expr(_) => "".to_string(), // Arrays as params don't need size
                    };
                    self.output
                        .push_str(&format!("{} {}[{}]", elem_c_type, param.name, size_str));
                }
                Type::Custom(_) => {
                    // Use type_to_c to resolve type aliases
                    let c_type = self.type_to_c(&param.ty);
                    if param.mutable {
                        // Pass by pointer for mutable parameters
                        self.output.push_str(&format!("{}* {}", c_type, param.name));
                    } else {
                        // Pass by value for immutable parameters
                        self.output.push_str(&format!("{} {}", c_type, param.name));
                    }
                }
                Type::Reference { inner, mutable, .. } => {
                    // Handle reference parameters
                    match inner.as_ref() {
                        Type::I32 => {
                            self.output
                                .push_str(if *mutable { "int* " } else { "const int* " });
                        }
                        Type::I64 => {
                            self.output.push_str(if *mutable {
                                "long long* "
                            } else {
                                "const long long* "
                            });
                        }
                        Type::U32 => {
                            self.output.push_str(if *mutable {
                                "unsigned int* "
                            } else {
                                "const unsigned int* "
                            });
                        }
                        Type::U64 => {
                            self.output.push_str(if *mutable {
                                "unsigned long long* "
                            } else {
                                "const unsigned long long* "
                            });
                        }
                        Type::Bool => {
                            self.output
                                .push_str(if *mutable { "int* " } else { "const int* " });
                        }
                        Type::String => {
                            self.output.push_str(if *mutable {
                                "char** "
                            } else {
                                "const char** "
                            });
                        }
                        Type::Custom(name) => {
                            if *mutable {
                                self.output.push_str(&format!("struct {}* ", name));
                            } else {
                                self.output.push_str(&format!("const struct {}* ", name));
                            }
                        }
                        _ => {
                            return Err(CompileError::Generic(
                                "Unsupported type in reference parameter".to_string(),
                            ));
                        }
                    }
                    self.output.push_str(&param.name);
                }
                _ => {
                    // For other types
                    let c_type = self.type_to_c(&param.ty);

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
            // Track if parameter is a pointer (either mutable or reference)
            let is_pointer = param.mutable || matches!(&param.ty, Type::Reference { .. });
            self.mutable_params.insert(param.name.clone(), is_pointer);

            // Also track parameter types for type inference
            let c_type = match &param.ty {
                Type::String => "const char*".to_string(),
                Type::I32 => "int".to_string(),
                Type::I64 => "long long".to_string(),
                Type::Bool => "int".to_string(),
                Type::Custom(name) => name.clone(),
                Type::Reference { inner, .. } => {
                    // For references, we track the base type
                    match inner.as_ref() {
                        Type::Custom(name) => name.clone(),
                        Type::I32 => "int".to_string(),
                        Type::I64 => "long long".to_string(),
                        _ => "long long".to_string(),
                    }
                }
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
            Stmt::Let {
                name, ty, value, ..
            } => {
                self.output.push_str("    ");

                // Determine C type
                let (c_type, is_array, array_size) = match ty {
                    Some(t) => match t {
                        Type::Array(elem_type, size) => {
                            let size_val = match size {
                                ArraySize::Literal(n) => *n,
                                ArraySize::ConstParam(_) => 0, // TODO: resolve const param
                                ArraySize::Expr(_) => 0,       // TODO: evaluate expression
                            };
                            (self.type_to_c(elem_type), true, Some(size_val))
                        }
                        _ => (self.type_to_c(t), false, None),
                    },
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
                                    0 // This should have been caught by type checker
                                };
                                (elem_type, true, Some(size))
                            }
                            Expr::StructLiteral { .. } => {
                                (self.infer_expr_type(value), false, None)
                            }
                            Expr::Call { .. } => {
                                // Use our unified type inference
                                (inferred_type, false, None)
                            }
                            _ => ("long long".to_string(), false, None), // Default to int for now
                        }
                    }
                };

                // Track variable type for future inference
                if is_array {
                    // For arrays, store the full array type including brackets
                    self.variables.insert(
                        name.clone(),
                        format!("{}[{}]", c_type, array_size.unwrap_or(0)),
                    );
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
                        self.output.push('[');
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
                    AssignTarget::Deref { expr } => {
                        // Generate dereference assignment: *expr = value
                        self.output.push_str("*(");
                        self.generate_expression(expr)?;
                        self.output.push_str(") = ");
                    }
                }
                self.generate_expression(value)?;
                self.output.push_str(";\n");
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
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

                self.output.push('\n');
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.output.push_str("    while (");
                self.generate_expression(condition)?;
                self.output.push_str(") {\n");

                // Generate body
                for stmt in body {
                    self.generate_statement(stmt)?;
                }

                self.output.push_str("    }\n");
            }
            Stmt::For {
                var, iter, body, ..
            } => {
                self.output.push_str("    {\n"); // Create a new scope

                // Check if iterating over a range
                match iter {
                    Expr::Range { start, end, .. } => {
                        // Generate C-style for loop for range
                        self.output.push_str("        // For loop with range\n");
                        self.output
                            .push_str(&format!("        for (long long {} = ", var));
                        self.generate_expression(start)?;
                        self.output.push_str(&format!("; {} < ", var));
                        self.generate_expression(end)?;
                        self.output.push_str(&format!("; {}++) {{\n", var));

                        // Generate body
                        for stmt in body {
                            self.output.push_str("        "); // Extra indentation
                            self.generate_statement(stmt)?;
                        }

                        self.output.push_str("        }\n");
                    }
                    _ => {
                        // For arrays and other iterables
                        self.output.push_str("        // For-in loop\n");
                        self.output
                            .push_str("        for (long long _i = 0; _i < sizeof(");
                        self.generate_expression(iter)?;
                        self.output.push_str(")/sizeof(");
                        self.generate_expression(iter)?;
                        self.output.push_str("[0]); _i++) {\n");

                        // Declare loop variable and assign current element
                        self.output
                            .push_str(&format!("            long long {} = ", var));
                        self.generate_expression(iter)?;
                        self.output.push_str("[_i];\n");

                        // Generate body
                        for stmt in body {
                            self.output.push_str("        "); // Extra indentation
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

                // Determine the type of the match expression
                let expr_type = self.infer_expr_type(expr);
                let is_enum =
                    expr_type != "long long" && expr_type != "const char*" && expr_type != "int";

                // Store the match expression in a temporary variable
                self.output
                    .push_str("        // Temporary for match expression\n");
                if is_enum {
                    self.output
                        .push_str(&format!("        {} _match_expr = ", expr_type));
                } else {
                    self.output.push_str("        long long _match_expr = ");
                }
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
                            self.output.push('1');
                        }
                        Pattern::Ident(name) => {
                            // Identifier pattern always matches and binds
                            self.output.push_str("1) {\n");
                            self.output.push_str(&format!(
                                "            long long {} = _match_expr;\n",
                                name
                            ));
                            // Continue with body generation below
                            for stmt in &arm.body {
                                self.output.push_str("        ");
                                self.generate_statement(stmt)?;
                            }
                            self.output.push_str("        }");
                            continue;
                        }
                        Pattern::EnumPattern {
                            enum_name,
                            variant,
                            data,
                        } => {
                            // Generate enum tag check
                            self.output.push_str(&format!(
                                "_match_expr.tag == __{}__{})",
                                enum_name, variant
                            ));
                            self.output.push_str(" {\n");

                            // Extract data if present
                            if let Some(pattern_data) = data {
                                // Look up the enum definition to get field types
                                if let Some(enum_def) = self.enums.get(enum_name) {
                                    // Find the variant
                                    if let Some(variant_def) =
                                        enum_def.variants.iter().find(|v| &v.name == variant)
                                    {
                                        match (&variant_def.data, pattern_data) {
                                            (
                                                EnumVariantData::Tuple(types),
                                                PatternData::Tuple(patterns),
                                            ) => {
                                                // Extract tuple fields with proper types
                                                for (i, (pattern, ty)) in
                                                    patterns.iter().zip(types.iter()).enumerate()
                                                {
                                                    if let Pattern::Ident(name) = pattern {
                                                        let c_type = self.type_to_c(ty);
                                                        self.output.push_str(&format!(
                                                            "            {} {} = _match_expr.data.{}.field{};\n",
                                                            c_type, name, variant.to_lowercase(), i
                                                        ));
                                                    }
                                                }
                                            }
                                            (
                                                EnumVariantData::Struct(fields),
                                                PatternData::Struct(field_patterns),
                                            ) => {
                                                // Extract struct fields with proper types
                                                for (field_name, pattern) in field_patterns {
                                                    if let Pattern::Ident(name) = pattern {
                                                        // Find the field type
                                                        if let Some((_, field_type)) = fields
                                                            .iter()
                                                            .find(|(fname, _)| fname == field_name)
                                                        {
                                                            let c_type = self.type_to_c(field_type);
                                                            self.output.push_str(&format!(
                                                                "            {} {} = _match_expr.data.{}.{};\n",
                                                                c_type, name, variant.to_lowercase(), field_name
                                                            ));
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {
                                                // Fallback for mismatched patterns (shouldn't happen with proper type checking)
                                                return Err(CompileError::Generic(
                                                    "Pattern type mismatch in enum variant"
                                                        .to_string(),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }

                            // Continue with body generation below
                            for stmt in &arm.body {
                                self.output.push_str("        ");
                                self.generate_statement(stmt)?;
                            }
                            self.output.push_str("        }");
                            continue;
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
            Stmt::Unsafe { body, .. } => {
                // Unsafe blocks in C are just regular blocks
                // The safety checks are done at compile time
                self.output.push_str("    // unsafe block\n");
                self.output.push_str("    {\n");

                // Generate body
                for stmt in body {
                    self.output.push_str("    "); // Extra indentation
                    self.generate_statement(stmt)?;
                }

                self.output.push_str("    }\n");
            }
        }
        Ok(())
    }

    /// Generate code for an expression
    fn generate_expression(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::String(s) => {
                // Escape the string properly
                let escaped = s
                    .replace("\\", "\\\\")
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
                        let is_array =
                            self.functions
                                .values()
                                .find_map(|(params, _)| {
                                    params.iter().find(|p| &p.name == name).and_then(|p| {
                                        match &p.ty {
                                            Type::Array(_, _) => Some(true),
                                            _ => None,
                                        }
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
                            "panic" => self.output.push_str("__pd_panic"),
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
                            "int_to_string" => self.output.push_str("__pd_int_to_string"),
                            "file_open" => self.output.push_str("__pd_file_open"),
                            "file_read_all" => self.output.push_str("__pd_file_read_all"),
                            "file_read_line" => self.output.push_str("__pd_file_read_line"),
                            "file_write" => self.output.push_str("__pd_file_write"),
                            "file_close" => self.output.push_str("__pd_file_close"),
                            "file_exists" => self.output.push_str("__pd_file_exists"),
                            // Enhanced I/O functions
                            "path_exists" => self.output.push_str("__pd_path_exists"),
                            "path_is_file" => self.output.push_str("__pd_path_is_file"),
                            "path_is_dir" => self.output.push_str("__pd_path_is_dir"),
                            "create_dir" => self.output.push_str("__pd_create_dir"),
                            "create_dir_all" => self.output.push_str("__pd_create_dir_all"),
                            "remove_file" => self.output.push_str("__pd_remove_file"),
                            "remove_dir" => self.output.push_str("__pd_remove_dir"),
                            "remove_dir_all" => self.output.push_str("__pd_remove_dir_all"),
                            "read_file_to_string" => self.output.push_str("__pd_read_file_to_string"),
                            "write_string_to_file" => self.output.push_str("__pd_write_string_to_file"),
                            "file_flush" => self.output.push_str("__pd_file_flush"),
                            "file_seek" => self.output.push_str("__pd_file_seek"),
                            // Enhanced file operations with mode support
                            "file_open_ex" => self.output.push_str("__pd_file_open_ex"),
                            "file_close_ex" => self.output.push_str("__pd_file_close_ex"),
                            "file_read_ex" => self.output.push_str("__pd_file_read_ex"),
                            "file_write_ex" => self.output.push_str("__pd_file_write_ex"),
                            _ => {
                                // Check if this is a method call (contains ::)
                                if name.contains("::") {
                                    // Convert Type::method to __pd_Type_method
                                    let mangled = format!("__pd_{}", name.replace("::", "_"));
                                    self.output.push_str(&mangled);
                                } else if let Some(mangled_name) =
                                    self.get_mangled_name_for_call(name, args)
                                {
                                    // Check if this is a generic function that needs name mangling
                                    self.output.push_str(&mangled_name);
                                } else {
                                    self.output.push_str(name);
                                }
                            }
                        }
                    }
                    _ => {
                        return Err(CompileError::Generic(
                            "Indirect calls not yet supported".to_string(),
                        ));
                    }
                }

                // Generate arguments
                self.output.push('(');

                // Get function signature to check parameter mutability
                let func_params = match func.as_ref() {
                    Expr::Ident(name) => self.functions.get(name).map(|(params, _)| params.clone()),
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
                                if var_type.is_some_and(|t| t.contains("[")) {
                                    // It's an array, don't take address
                                    self.generate_expression(arg)?;
                                } else {
                                    // Need to take address
                                    self.output.push('&');
                                    self.generate_expression(arg)?;
                                }
                            }
                        } else {
                            // Need to take address
                            self.output.push('&');
                            self.generate_expression(arg)?;
                        }
                    } else {
                        self.generate_expression(arg)?;
                    }
                }
                self.output.push(')');
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                // Check if this is string concatenation
                let left_type = self.infer_expr_type(left);
                let right_type = self.infer_expr_type(right);

                if matches!(op, BinOp::Add)
                    && left_type == "const char*"
                    && right_type == "const char*"
                {
                    // String concatenation - use helper function
                    self.output.push_str("__pd_string_concat(");
                    self.generate_expression(left)?;
                    self.output.push_str(", ");
                    self.generate_expression(right)?;
                    self.output.push(')');
                } else {
                    // Regular binary operation
                    self.output.push('(');

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

                    self.output.push(')');
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                // Generate array literal: {1, 2, 3}
                self.output.push('{');
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.generate_expression(elem)?;
                }
                self.output.push('}');
            }
            Expr::ArrayRepeat { value, count, .. } => {
                // Generate array repeat initialization
                // For [0; 10], generate: {0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
                self.output.push('{');
                if let Expr::Integer(n) = count.as_ref() {
                    for i in 0..*n {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.generate_expression(value)?;
                    }
                }
                self.output.push('}');
            }
            Expr::Index { array, index, .. } => {
                // Generate array indexing: arr[i]
                self.generate_expression(array)?;
                self.output.push('[');
                self.generate_expression(index)?;
                self.output.push(']');
            }
            Expr::StructLiteral { name, fields, .. } => {
                // Generate struct literal: (StructName){.field1 = value1, .field2 = value2}
                // Check if this is a generic struct instantiation
                let struct_name =
                    if let Some(instantiations) = self.generic_struct_instantiation_map.get(name) {
                        // Need to determine which instantiation to use based on field types
                        // For now, we'll infer from the first field's type
                        if let Some((_field_name, field_expr)) = fields.first() {
                            let field_type = self.infer_expr_type(field_expr);

                            // Find the matching instantiation
                            let mut found_name = None;
                            for (type_args, mangled_name) in instantiations {
                                // Simple heuristic: check if any type arg matches the field type
                                for type_arg in type_args {
                                    if (type_arg == "i64" && field_type.contains("long long"))
                                        || (type_arg == "bool" && field_type == "int")
                                        || (type_arg == "String" && field_type.contains("char*"))
                                    {
                                        found_name = Some(mangled_name.as_str());
                                        break;
                                    }
                                }
                                if found_name.is_some() {
                                    break;
                                }
                            }
                            found_name.unwrap_or(name.as_str())
                        } else {
                            name.as_str()
                        }
                    } else {
                        // Use the original name for non-generic structs
                        name.as_str()
                    };

                self.output.push_str(&format!("(struct {})", struct_name));
                self.output.push('{');
                for (i, (field_name, field_expr)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&format!(".{} = ", field_name));
                    self.generate_expression(field_expr)?;
                }
                self.output.push('}');
            }
            Expr::FieldAccess { object, field, .. } => {
                // Check if object is a mutable parameter (pointer)
                let use_arrow = match object.as_ref() {
                    Expr::Ident(name) => self.mutable_params.get(name).copied().unwrap_or(false),
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
            Expr::EnumConstructor {
                enum_name,
                variant,
                data,
                ..
            } => {
                // Generate enum constructor call
                match data {
                    None => {
                        // Unit variant
                        self.output.push_str(&format!("{}_{}", enum_name, variant));
                        self.output.push_str("()");
                    }
                    Some(EnumConstructorData::Tuple(exprs)) => {
                        // Tuple variant
                        self.output
                            .push_str(&format!("{}_{}__new", enum_name, variant));
                        self.output.push('(');
                        for (i, expr) in exprs.iter().enumerate() {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            self.generate_expression(expr)?;
                        }
                        self.output.push(')');
                    }
                    Some(EnumConstructorData::Struct(fields)) => {
                        // Struct variant
                        self.output
                            .push_str(&format!("{}_{}__new", enum_name, variant));
                        self.output.push('(');
                        for (i, (_, expr)) in fields.iter().enumerate() {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            self.generate_expression(expr)?;
                        }
                        self.output.push(')');
                    }
                }
            }
            Expr::Range { .. } => {
                // Range expressions are not directly translatable to C
                // They should only appear in for loops which handle them specially
                return Err(CompileError::Generic(
                    "Range expressions can only be used in for loops".to_string(),
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
            Expr::Reference { mutable, expr, .. } => {
                // Generate reference (address-of) expression
                if *mutable {
                    // For now, C doesn't distinguish between & and &mut
                    self.output.push_str("(&(");
                } else {
                    self.output.push_str("(&(");
                }
                self.generate_expression(expr)?;
                self.output.push_str("))");
            }
            Expr::Deref { expr, .. } => {
                // Generate dereference expression
                self.output.push_str("(*(");
                self.generate_expression(expr)?;
                self.output.push_str("))");
            }
            Expr::Question { expr, .. } => {
                // The ? operator is syntactic sugar for:
                // match expr {
                //     Ok(value) => value,
                //     Err(e) => return Err(e),
                // }

                // Generate a temporary variable name
                self.temp_counter += 1;
                let temp_var = format!("__question_result_{}", self.temp_counter);

                // For now, we'll generate code that assumes Result is a struct with:
                // - an 'is_ok' field (0 for Err, 1 for Ok)
                // - a union containing 'ok' and 'err' fields
                // This is a simplified representation until proper enum support is added

                self.output.push_str("({\n");
                self.output
                    .push_str(&format!("        struct Result {} = ", temp_var));
                self.generate_expression(expr)?;
                self.output.push_str(";\n");
                self.output
                    .push_str(&format!("        if (!{}.is_ok) {{\n", temp_var));
                self.output.push_str(&format!(
                    "            return (struct Result){{.is_ok = 0, .data.err = {}.data.err}};\n",
                    temp_var
                ));
                self.output.push_str("        }\n");
                self.output
                    .push_str(&format!("        {}.data.ok;\n", temp_var));
                self.output.push_str("    })");

                // Note: This implementation assumes:
                // 1. Result is represented as: struct Result { int is_ok; union { T ok; E err; } data; }
                // 2. The containing function returns a Result type
                // 3. Error types are compatible between the expression and function return

                // TODO: Once enum support is properly implemented:
                // - Generate proper enum variant checking
                // - Handle generic Result<T,E> types correctly
                // - Ensure type safety for error propagation
            }
            Expr::MacroInvocation { .. } => {
                // Macros should have been expanded before codegen
                return Err(CompileError::Generic(
                    "Unexpected macro invocation in code generation - macros should be expanded before this phase".to_string()
                ));
            }
            Expr::Await { expr, .. } => {
                // For now, generate a simple blocking wait
                // In a real implementation, this would integrate with an async runtime
                self.output.push_str("({\n");
                self.output
                    .push_str("        // Await expression - simplified blocking implementation\n");

                // Generate temporary for the future
                self.temp_counter += 1;
                let future_var = format!("__await_future_{}", self.temp_counter);

                // Get the future
                self.output.push_str(&format!(
                    "        {} {} = ",
                    self.infer_expr_type(expr),
                    future_var
                ));
                self.generate_expression(expr)?;
                self.output.push_str(";\n");

                // Poll until ready (simplified - assumes poll function exists)
                self.output.push_str(&format!(
                    "        while (!{}.poll(&{})) {{\n",
                    future_var, future_var
                ));
                self.output
                    .push_str("            // In real async runtime, would yield here\n");
                self.output.push_str("        }\n");

                // Return the result
                self.output
                    .push_str(&format!("        {}.result;\n", future_var));
                self.output.push_str("    })");
            }
        }
        Ok(())
    }

    /// Create a monomorphized version of a generic struct
    /// Generate code for an async function
    fn generate_async_function_with_name(&mut self, func: &Function, name: &str) -> Result<()> {
        // For now, we'll generate a simple Future struct
        let future_name = format!("{}_Future", name);
        let output_type = func
            .return_type
            .as_ref()
            .map(|t| self.type_to_c(t))
            .unwrap_or_else(|| "void".to_string());

        // Generate Future struct
        self.output
            .push_str(&format!("// Future struct for async function {}\n", name));
        self.output
            .push_str(&format!("typedef struct {} {{\n", future_name));
        self.output.push_str("    int state;\n");
        if output_type != "void" {
            self.output
                .push_str(&format!("    {} result;\n", output_type));
        }

        // Add fields for parameters
        for param in &func.params {
            let param_type = self.type_to_c(&param.ty);
            self.output
                .push_str(&format!("    {} {};\n", param_type, param.name));
        }

        self.output.push_str(&format!("}} {};\n\n", future_name));

        // Generate poll function
        self.output
            .push_str(&format!("// Poll function for {}\n", future_name));
        self.output
            .push_str(&format!("int {}_poll({} *future) {{\n", name, future_name));
        self.output
            .push_str("    // Simplified async - immediately ready\n");
        self.output.push_str("    if (future->state == 0) {\n");
        self.output.push_str("        future->state = 1;\n");

        // Generate the actual function body
        self.output
            .push_str("        // Execute async function body\n");
        for stmt in &func.body {
            self.output.push_str("        ");
            self.generate_statement(stmt)?;
        }

        self.output.push_str("        return 1; // Ready\n");
        self.output.push_str("    }\n");
        self.output.push_str("    return 1; // Already completed\n");
        self.output.push_str("}\n\n");

        // Generate the function that creates the Future
        self.output.push_str(&format!("{} {}(", future_name, name));

        // Parameters
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            let param_type = self.type_to_c(&param.ty);
            self.output
                .push_str(&format!("{} {}", param_type, param.name));
        }

        self.output.push_str(") {\n");
        self.output
            .push_str(&format!("    {} future;\n", future_name));
        self.output.push_str("    future.state = 0;\n");

        // Copy parameters to future
        for param in &func.params {
            self.output
                .push_str(&format!("    future.{} = {};\n", param.name, param.name));
        }

        self.output.push_str("    return future;\n");
        self.output.push_str("}\n\n");

        Ok(())
    }

    fn monomorphize_struct(
        &self,
        struct_name: &str,
        type_args: &[String],
        generic_struct: &crate::typeck::GenericStruct,
    ) -> Result<StructDef> {
        // Generate a mangled name for the concrete struct
        let mangled_name = format!("{}_{}", struct_name, type_args.join("_"));

        // Create a mapping from type parameters to concrete types
        let mut type_map = std::collections::HashMap::new();
        for (i, type_param) in generic_struct.type_params.iter().enumerate() {
            if i < type_args.len() {
                type_map.insert(type_param.clone(), type_args[i].clone());
            }
        }

        // Substitute types in fields
        let concrete_fields = generic_struct
            .fields
            .iter()
            .map(|(name, ty)| {
                let concrete_type = self.substitute_type(ty, &type_map);
                (name.clone(), concrete_type)
            })
            .collect();

        // Create the concrete struct
        Ok(StructDef {
            name: mangled_name,
            lifetime_params: vec![], // No longer generic
            type_params: vec![],     // No longer generic
            const_params: vec![],    // No longer generic
            fields: concrete_fields,
            visibility: crate::ast::Visibility::Private, // Monomorphized structs are internal
            span: Span {
                start: 0,
                end: 0,
                line: 0,
                column: 0,
            }, // Synthetic span for generated struct
        })
    }

    /// Create a monomorphized version of a generic function
    fn monomorphize_function(
        &self,
        func_name: &str,
        type_args: &[String],
        generic_func: &crate::typeck::GenericFunction,
    ) -> Result<Function> {
        // Generate a mangled name for the concrete function
        let mangled_name = format!("{}__{}", func_name, type_args.join("_"));

        // Create a mapping from type parameters to concrete types
        let mut type_map = std::collections::HashMap::new();
        for (i, type_param) in generic_func.type_params.iter().enumerate() {
            if i < type_args.len() {
                type_map.insert(type_param.clone(), type_args[i].clone());
            }
        }

        // Substitute types in parameters
        let concrete_params = generic_func
            .params
            .iter()
            .map(|(name, ty)| {
                let concrete_type = self.substitute_type(ty, &type_map);
                Param {
                    name: name.clone(),
                    ty: concrete_type,
                    mutable: false, // TODO: Preserve mutability from original
                }
            })
            .collect();

        // Substitute type in return type
        let concrete_return_type = generic_func
            .return_type
            .as_ref()
            .map(|ty| self.substitute_type(ty, &type_map));

        // Substitute types in the function body
        let concrete_body = self.substitute_types_in_body(&generic_func.body, &type_map);

        // Create the concrete function
        Ok(Function {
            name: mangled_name,
            is_async: false,         // Monomorphized functions are not async
            lifetime_params: vec![], // No longer generic
            type_params: vec![],     // No longer generic
            const_params: vec![],    // No longer generic
            params: concrete_params,
            return_type: concrete_return_type,
            body: concrete_body,
            visibility: crate::ast::Visibility::Private, // Monomorphized functions are internal
            span: Span {
                start: 0,
                end: 0,
                line: 0,
                column: 0,
            }, // Synthetic span for generated function
            effects: None, // Effects are not tracked for monomorphized functions yet
        })
    }

    /// Create a mangled name for a generic function
    fn mangle_generic_name(&self, func_name: &str, type_args: &[String]) -> String {
        format!("{}__{}", func_name, type_args.join("_"))
    }

    /// Get the mangled name for a generic function call
    fn get_mangled_name_for_call(&self, func_name: &str, args: &[Expr]) -> Option<String> {
        // Check if this function has generic instantiations
        let mut instantiations_for_func = Vec::new();
        for (name, type_args, _) in &self.generic_instantiations {
            if name == func_name {
                instantiations_for_func.push(type_args.clone());
            }
        }

        if instantiations_for_func.is_empty() {
            return None;
        }

        // If there's only one instantiation, use it
        if instantiations_for_func.len() == 1 {
            return Some(self.mangle_generic_name(func_name, &instantiations_for_func[0]));
        }

        // Try to infer which instantiation based on the first argument type
        if let Some(first_arg) = args.first() {
            let arg_type_str = match first_arg {
                Expr::ArrayLiteral { elements, .. } => {
                    if !elements.is_empty() {
                        // Infer from first element
                        self.infer_expr_type(&elements[0])
                    } else {
                        return None;
                    }
                }
                Expr::Ident(name) => {
                    // Look up variable type
                    self.variables
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| self.infer_expr_type(first_arg))
                }
                _ => self.infer_expr_type(first_arg),
            };

            // Find best matching instantiation
            for type_args in &instantiations_for_func {
                // Check if any type arg matches our inferred type
                for type_arg in type_args {
                    if type_arg == "i64" && arg_type_str.contains("long long") {
                        return Some(self.mangle_generic_name(func_name, type_args));
                    }
                    if type_arg == "bool"
                        && arg_type_str.contains("int")
                        && !arg_type_str.contains("long")
                    {
                        return Some(self.mangle_generic_name(func_name, type_args));
                    }
                    if type_arg == "String" && arg_type_str.contains("char*") {
                        return Some(self.mangle_generic_name(func_name, type_args));
                    }
                    if type_arg == &arg_type_str {
                        return Some(self.mangle_generic_name(func_name, type_args));
                    }
                }
            }
        }

        // Default to first instantiation if we can't determine
        Some(self.mangle_generic_name(func_name, &instantiations_for_func[0]))
    }

    /// Substitute type parameters with concrete types in a type
    #[allow(clippy::only_used_in_recursion)]
    fn substitute_type(
        &self,
        ty: &Type,
        type_map: &std::collections::HashMap<String, String>,
    ) -> Type {
        match ty {
            Type::TypeParam(name) => {
                // Replace type parameter with concrete type
                if let Some(concrete_name) = type_map.get(name) {
                    // Parse the concrete type name
                    match concrete_name.as_str() {
                        "i32" | "I32" => Type::I32,
                        "i64" | "I64" => Type::I64,
                        "u32" | "U32" => Type::U32,
                        "u64" | "U64" => Type::U64,
                        "bool" | "Bool" => Type::Bool,
                        "string" | "String" => Type::String,
                        _ => Type::Custom(concrete_name.clone()),
                    }
                } else {
                    ty.clone()
                }
            }
            Type::Array(elem_type, size) => Type::Array(
                Box::new(self.substitute_type(elem_type, type_map)),
                size.clone(),
            ),
            Type::Generic { name, args } => {
                // Substitute in generic type arguments
                let substituted_args = args
                    .iter()
                    .map(|arg| match arg {
                        GenericArg::Type(t) => GenericArg::Type(self.substitute_type(t, type_map)),
                        GenericArg::Const(c) => GenericArg::Const(c.clone()), // TODO: substitute const params
                    })
                    .collect();
                Type::Generic {
                    name: name.clone(),
                    args: substituted_args,
                }
            }
            Type::Custom(name) => {
                // Check if this custom type is actually a type parameter
                if let Some(concrete_name) = type_map.get(name) {
                    // Parse the concrete type name
                    match concrete_name.as_str() {
                        "i32" | "I32" => Type::I32,
                        "i64" | "I64" => Type::I64,
                        "u32" | "U32" => Type::U32,
                        "u64" | "U64" => Type::U64,
                        "bool" | "Bool" => Type::Bool,
                        "string" | "String" => Type::String,
                        _ => Type::Custom(concrete_name.clone()),
                    }
                } else {
                    ty.clone()
                }
            }
            _ => ty.clone(),
        }
    }

    /// Substitute types in a statement body
    fn substitute_types_in_body(
        &self,
        stmts: &[Stmt],
        _type_map: &std::collections::HashMap<String, String>,
    ) -> Vec<Stmt> {
        // For now, we'll just clone the body
        // In a full implementation, we'd need to walk the AST and substitute types
        // This is a simplified version that works for basic cases
        stmts.to_vec()
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
