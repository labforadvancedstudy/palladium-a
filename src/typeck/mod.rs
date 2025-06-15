// Type checker for Palladium
// "Ensuring legends are logically sound"

use crate::ast::*;
use crate::errors::{CompileError, Result};
use std::collections::HashMap;

/// Type representation for type checker (wraps AST Type)
#[derive(Debug, Clone, PartialEq)]
pub enum CheckerType {
    Unit,
    String,
    Int,
    Function(Vec<CheckerType>, Box<CheckerType>),
}

impl From<&crate::ast::Type> for CheckerType {
    fn from(ast_type: &crate::ast::Type) -> Self {
        match ast_type {
            crate::ast::Type::Unit => CheckerType::Unit,
            crate::ast::Type::String => CheckerType::String,
            crate::ast::Type::I32 | crate::ast::Type::I64 => CheckerType::Int,
            crate::ast::Type::Bool => CheckerType::Unit, // TODO: Add Bool type
            crate::ast::Type::U32 | crate::ast::Type::U64 => CheckerType::Int,
            crate::ast::Type::Custom(_) => CheckerType::Unit, // TODO: Handle custom types
        }
    }
}

impl std::fmt::Display for CheckerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckerType::Unit => write!(f, "()"),
            CheckerType::String => write!(f, "String"),
            CheckerType::Int => write!(f, "Int"),
            CheckerType::Function(params, ret) => {
                write!(f, "fn(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", param)?;
                }
                write!(f, ") -> {}", ret)
            }
        }
    }
}

pub struct TypeChecker {
    /// Function signatures
    functions: HashMap<String, CheckerType>,
    /// Current function return type (for checking return statements)
    current_function_return: Option<CheckerType>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut functions = HashMap::new();
        
        // Built-in functions
        functions.insert(
            "print".to_string(),
            CheckerType::Function(vec![CheckerType::String], Box::new(CheckerType::Unit)),
        );
        
        Self {
            functions,
            current_function_return: None,
        }
    }
    
    /// Type check a program
    pub fn check(&mut self, program: &Program) -> Result<()> {
        // First pass: collect all function signatures
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    // Extract return type from function
                    let return_type = func.return_type.as_ref()
                        .map(|t| CheckerType::from(t))
                        .unwrap_or(CheckerType::Unit);
                    
                    let func_type = CheckerType::Function(vec![], Box::new(return_type));
                    self.functions.insert(func.name.clone(), func_type);
                }
            }
        }
        
        // Check for main function
        if !self.functions.contains_key("main") {
            return Err(CompileError::Generic(
                "No 'main' function found".to_string()
            ));
        }
        
        // Second pass: type check function bodies
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.check_function(func)?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Type check a function
    fn check_function(&mut self, func: &Function) -> Result<()> {
        // Set current function return type
        let return_type = func.return_type.as_ref()
            .map(|t| CheckerType::from(t))
            .unwrap_or(CheckerType::Unit);
        self.current_function_return = Some(return_type);
        
        // Type check each statement in the body
        for stmt in &func.body {
            self.check_statement(stmt)?;
        }
        
        self.current_function_return = None;
        Ok(())
    }
    
    /// Type check a statement
    fn check_statement(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Expr(expr) => {
                self.check_expression(expr)?;
                Ok(())
            }
            Stmt::Return(None) => {
                // Returning nothing is Unit type
                if self.current_function_return != Some(CheckerType::Unit) {
                    return Err(CompileError::TypeMismatch {
                        expected: "()".to_string(),
                        found: "return value".to_string(),
                    });
                }
                Ok(())
            }
            Stmt::Return(Some(expr)) => {
                let expr_type = self.check_expression(expr)?;
                if let Some(expected) = &self.current_function_return {
                    if expr_type != *expected {
                        return Err(CompileError::TypeMismatch {
                            expected: expected.to_string(),
                            found: expr_type.to_string(),
                        });
                    }
                }
                Ok(())
            }
            Stmt::Let { .. } => {
                // Not implemented in v0.1
                Err(CompileError::Generic(
                    "Let bindings not yet implemented".to_string()
                ))
            }
        }
    }
    
    /// Type check an expression and return its type
    fn check_expression(&mut self, expr: &Expr) -> Result<CheckerType> {
        match expr {
            Expr::String(_) => Ok(CheckerType::String),
            Expr::Integer(_) => Ok(CheckerType::Int),
            Expr::Ident(name) => {
                // For now, identifiers can only be functions
                self.functions.get(name)
                    .cloned()
                    .ok_or_else(|| CompileError::UndefinedFunction {
                        name: name.clone(),
                    })
            }
            Expr::Call { func, args, .. } => {
                // Get function name (for v0.1, only direct calls)
                let func_name = match func.as_ref() {
                    Expr::Ident(name) => name,
                    _ => return Err(CompileError::Generic(
                        "Indirect function calls not yet supported".to_string()
                    )),
                };
                
                // Look up function type
                let func_type = self.functions.get(func_name)
                    .ok_or_else(|| CompileError::UndefinedFunction {
                        name: func_name.clone(),
                    })?
                    .clone();
                
                // Check function type
                match func_type {
                    CheckerType::Function(param_types, return_type) => {
                        // Check argument count
                        if args.len() != param_types.len() {
                            return Err(CompileError::ArgumentCountMismatch {
                                name: func_name.clone(),
                                expected: param_types.len(),
                                found: args.len(),
                            });
                        }
                        
                        // Check argument types
                        for (arg, expected_type) in args.iter().zip(param_types.iter()) {
                            let arg_type = self.check_expression(arg)?;
                            if arg_type != *expected_type {
                                return Err(CompileError::TypeMismatch {
                                    expected: expected_type.to_string(),
                                    found: arg_type.to_string(),
                                });
                            }
                        }
                        
                        Ok(return_type.as_ref().clone())
                    }
                    _ => Err(CompileError::Generic(
                        format!("{} is not a function", func_name)
                    )),
                }
            }
            Expr::Binary { .. } => {
                // Not implemented in v0.1
                Err(CompileError::Generic(
                    "Binary operations not yet implemented".to_string()
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    
    #[test]
    fn test_type_check_hello_world() {
        let source = r#"
        fn main() {
            print("Hello, World!");
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }
    
    #[test]
    fn test_undefined_function() {
        let source = r#"
        fn main() {
            unknown_function();
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
        
        if let Err(CompileError::UndefinedFunction { name }) = result {
            assert_eq!(name, "unknown_function");
        }
    }
}