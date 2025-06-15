// Code generation for Palladium
// "Forging legends into machine code"

use crate::ast::{*, AssignTarget};
use crate::errors::{CompileError, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct CodeGenerator {
    module_name: String,
    output: String,
}

impl CodeGenerator {
    pub fn new(module_name: &str) -> Result<Self> {
        Ok(Self {
            module_name: module_name.to_string(),
            output: String::new(),
        })
    }
    
    /// Compile an AST to machine code
    pub fn compile(&mut self, program: &Program) -> Result<()> {
        // For v0.1, we'll generate a simple C file that we can compile with gcc
        // This is a temporary solution until LLVM integration is complete
        
        self.output.push_str("#include <stdio.h>\n\n");
        
        // Generate print function wrapper
        self.output.push_str("void __pd_print(const char* str) {\n");
        self.output.push_str("    printf(\"%s\\n\", str);\n");
        self.output.push_str("}\n\n");
        
        // Generate print_int function wrapper
        self.output.push_str("void __pd_print_int(long long value) {\n");
        self.output.push_str("    printf(\"%lld\\n\", value);\n");
        self.output.push_str("}\n\n");
        
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
                return Err(CompileError::Generic(
                    format!("Returning structs from functions is not yet supported: {}", name)
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
        
        for (i, (param_name, param_type)) in func.params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            
            let c_type = match param_type {
                Type::I32 => "int",
                Type::I64 => "long long",
                Type::U32 => "unsigned int",
                Type::U64 => "unsigned long long",
                Type::Bool => "int",
                Type::String => "const char*",
                Type::Array(elem_type, size) => {
                    // For arrays, we need to generate proper C array parameter syntax
                    let elem_c_type = match elem_type.as_ref() {
                        Type::I32 => "int",
                        Type::I64 => "long long",
                        Type::U32 => "unsigned int",
                        Type::U64 => "unsigned long long",
                        Type::Bool => "int",
                        _ => return Err(CompileError::Generic(
                            "Unsupported array element type in function parameter".to_string()
                        )),
                    };
                    // In C, array parameters are passed as pointers
                    // We'll generate: type name[size] for clarity, though it decays to pointer
                    self.output.push_str(&format!("{} {}[{}]", elem_c_type, param_name, size));
                    continue;
                }
                Type::Unit => "void",
                Type::Custom(name) => {
                    // For custom types (structs), pass by value
                    self.output.push_str(&format!("{} {}", name, param_name));
                    continue;
                }
            };
            
            self.output.push_str(&format!("{} {}", c_type, param_name));
        }
        
        self.output.push_str(") {\n");
        
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
                        // Infer type from value for now
                        match value {
                            Expr::Integer(_) => ("long long".to_string(), false, None),
                            Expr::String(_) => ("const char*".to_string(), false, None),
                            Expr::Bool(_) => ("int".to_string(), false, None),
                            Expr::ArrayLiteral { elements, .. } => {
                                // Infer array element type from first element
                                let elem_type = if !elements.is_empty() {
                                    match &elements[0] {
                                        Expr::Integer(_) => "long long",
                                        Expr::String(_) => "const char*",
                                        Expr::Bool(_) => "int",
                                        _ => "long long",
                                    }
                                } else {
                                    "long long"
                                };
                                (elem_type.to_string(), true, Some(elements.len()))
                            }
                            Expr::StructLiteral { name, .. } => (name.to_string(), false, None),
                            _ => ("long long".to_string(), false, None),  // Default to int for now
                        }
                    }
                };
                
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
                        self.output.push_str(&format!("{} = ", name));
                    }
                    AssignTarget::Index { array, index } => {
                        self.generate_expression(array)?;
                        self.output.push_str("[");
                        self.generate_expression(index)?;
                        self.output.push_str("] = ");
                    }
                    AssignTarget::FieldAccess { object, field } => {
                        self.generate_expression(object)?;
                        self.output.push_str(&format!(".{} = ", field));
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
                // For now, we only support iterating over arrays
                // Generate a C-style for loop with index
                self.output.push_str("    {\n");  // Create a new scope
                
                // Determine array size and element type
                // For simplicity, we'll generate code that iterates with an index
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
                self.output.push_str("    }\n");
            }
            Stmt::Break { .. } => {
                self.output.push_str("    break;\n");
            }
            Stmt::Continue { .. } => {
                self.output.push_str("    continue;\n");
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
                // Direct variable reference
                self.output.push_str(name);
            }
            Expr::Call { func, args, .. } => {
                // Generate function name
                match func.as_ref() {
                    Expr::Ident(name) => {
                        // Map built-in functions
                        match name.as_str() {
                            "print" => self.output.push_str("__pd_print"),
                            "print_int" => self.output.push_str("__pd_print_int"),
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
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.generate_expression(arg)?;
                }
                self.output.push_str(")");
            }
            Expr::Binary { left, op, right, .. } => {
                // Generate parentheses for proper precedence
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
                };
                self.output.push_str(op_str);
                
                // Generate right operand
                self.generate_expression(right)?;
                
                self.output.push_str(")");
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
                // Generate field access: obj.field
                self.generate_expression(object)?;
                self.output.push_str(&format!(".{}", field));
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