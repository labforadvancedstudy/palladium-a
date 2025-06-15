// Code generation for Palladium
// "Forging legends into machine code"

use crate::ast::*;
use crate::errors::{CompileError, Result};
use std::fs::File;
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
        
        // Generate functions
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.generate_function(func)?;
                }
            }
        }
        
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
            Some(Type::Custom(_)) => "void",  // TODO: Handle custom types
        };
        
        // Special case: main always returns int in C
        let actual_return_type = if func.name == "main" && return_type == "void" {
            "int"
        } else {
            return_type
        };
        
        self.output.push_str(&format!("{} {}() {{\n", actual_return_type, func.name));
        
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
            Stmt::Let { .. } => {
                return Err(CompileError::Generic(
                    "Let bindings not yet implemented in codegen".to_string()
                ));
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
                self.output.push_str(&n.to_string());
            }
            Expr::Ident(name) => {
                self.output.push_str(name);
            }
            Expr::Call { func, args, .. } => {
                // Generate function name
                match func.as_ref() {
                    Expr::Ident(name) => {
                        // Map built-in functions
                        match name.as_str() {
                            "print" => self.output.push_str("__pd_print"),
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
            Expr::Binary { .. } => {
                return Err(CompileError::Generic(
                    "Binary operations not yet implemented in codegen".to_string()
                ));
            }
        }
        Ok(())
    }
    
    /// Write the generated code to a file
    pub fn write_output(&self) -> Result<PathBuf> {
        // Clean module name (remove .pd extension if present)
        let base_name = Path::new(&self.module_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&self.module_name);
            
        let output_path = PathBuf::from(format!("{}.c", base_name));
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
}