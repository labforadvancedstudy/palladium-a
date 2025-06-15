// Type checker for Palladium
// "Ensuring legends are logically sound"

use crate::ast::{*, AssignTarget};
use crate::errors::{CompileError, Result};
use std::collections::HashMap;

/// Type representation for type checker (wraps AST Type)
#[derive(Debug, Clone, PartialEq)]
pub enum CheckerType {
    Unit,
    String,
    Int,
    Bool,
    Array(Box<CheckerType>, usize),
    Function(Vec<CheckerType>, Box<CheckerType>),
}

impl From<&crate::ast::Type> for CheckerType {
    fn from(ast_type: &crate::ast::Type) -> Self {
        match ast_type {
            crate::ast::Type::Unit => CheckerType::Unit,
            crate::ast::Type::String => CheckerType::String,
            crate::ast::Type::I32 | crate::ast::Type::I64 => CheckerType::Int,
            crate::ast::Type::Bool => CheckerType::Bool,
            crate::ast::Type::U32 | crate::ast::Type::U64 => CheckerType::Int,
            crate::ast::Type::Array(elem_type, size) => {
                CheckerType::Array(Box::new(CheckerType::from(elem_type.as_ref())), *size)
            }
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
            CheckerType::Bool => write!(f, "Bool"),
            CheckerType::Array(elem_type, size) => write!(f, "[{}; {}]", elem_type, size),
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

/// Variable information including type and mutability
#[derive(Debug, Clone)]
struct VarInfo {
    ty: CheckerType,
    mutable: bool,
}

/// Symbol table for storing variable types with scope support
#[derive(Debug, Clone)]
struct SymbolTable {
    scopes: Vec<HashMap<String, VarInfo>>,
}

impl SymbolTable {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()], // Start with global scope
        }
    }
    
    /// Enter a new scope
    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    
    /// Exit the current scope
    fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }
    
    /// Define a variable in the current scope
    fn define(&mut self, name: String, ty: CheckerType, mutable: bool) -> Result<()> {
        if let Some(scope) = self.scopes.last_mut() {
            if scope.contains_key(&name) {
                return Err(CompileError::Generic(
                    format!("Variable '{}' already defined in this scope", name)
                ));
            }
            scope.insert(name, VarInfo { ty, mutable });
            Ok(())
        } else {
            Err(CompileError::Generic("No active scope".to_string()))
        }
    }
    
    /// Look up a variable (searches all scopes from innermost to outermost)
    fn lookup(&self, name: &str) -> Option<&VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }
}

pub struct TypeChecker {
    /// Function signatures
    functions: HashMap<String, CheckerType>,
    /// Current function return type (for checking return statements)
    current_function_return: Option<CheckerType>,
    /// Symbol table for variables
    symbols: SymbolTable,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut functions = HashMap::new();
        
        // Built-in functions
        functions.insert(
            "print".to_string(),
            CheckerType::Function(vec![CheckerType::String], Box::new(CheckerType::Unit)),
        );
        
        // print_int built-in function
        functions.insert(
            "print_int".to_string(),
            CheckerType::Function(vec![CheckerType::Int], Box::new(CheckerType::Unit)),
        );
        
        Self {
            functions,
            current_function_return: None,
            symbols: SymbolTable::new(),
        }
    }
    
    /// Type check a program
    pub fn check(&mut self, program: &Program) -> Result<()> {
        // First pass: collect all function signatures
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    // Extract parameter types
                    let param_types: Vec<CheckerType> = func.params.iter()
                        .map(|(_, ty)| CheckerType::from(ty))
                        .collect();
                    
                    // Extract return type from function
                    let return_type = func.return_type.as_ref()
                        .map(|t| CheckerType::from(t))
                        .unwrap_or(CheckerType::Unit);
                    
                    let func_type = CheckerType::Function(param_types, Box::new(return_type));
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
        // Enter function scope
        self.symbols.enter_scope();
        
        // Add function parameters to symbol table
        for (param_name, param_type) in &func.params {
            let checker_type = CheckerType::from(param_type);
            self.symbols.define(param_name.clone(), checker_type, false)?;
        }
        
        // Set current function return type
        let return_type = func.return_type.as_ref()
            .map(|t| CheckerType::from(t))
            .unwrap_or(CheckerType::Unit);
        self.current_function_return = Some(return_type);
        
        // Type check each statement in the body
        for stmt in &func.body {
            self.check_statement(stmt)?;
        }
        
        // Exit function scope
        self.symbols.exit_scope();
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
            Stmt::Let { name, ty, value, mutable, .. } => {
                // Type check the value expression
                let value_type = self.check_expression(value)?;
                
                // If type annotation is provided, check that it matches
                if let Some(annotated_type) = ty {
                    let expected_type = CheckerType::from(annotated_type);
                    if value_type != expected_type {
                        return Err(CompileError::TypeMismatch {
                            expected: expected_type.to_string(),
                            found: value_type.to_string(),
                        });
                    }
                    // Define variable with annotated type
                    self.symbols.define(name.clone(), expected_type, *mutable)?;
                } else {
                    // Define variable with inferred type
                    self.symbols.define(name.clone(), value_type, *mutable)?;
                }
                
                Ok(())
            }
            Stmt::Assign { target, value, .. } => {
                match target {
                    AssignTarget::Ident(name) => {
                        // Look up the variable and clone necessary info
                        let (var_type, var_mutable) = {
                            let var_info = self.symbols.lookup(name)
                                .ok_or_else(|| CompileError::Generic(
                                    format!("Undefined variable: '{}'", name)
                                ))?;
                            (var_info.ty.clone(), var_info.mutable)
                        };
                        
                        // Check if variable is mutable
                        if !var_mutable {
                            return Err(CompileError::Generic(
                                format!("Cannot assign to immutable variable '{}'", name)
                            ));
                        }
                        
                        // Type check the value expression
                        let value_type = self.check_expression(value)?;
                        
                        // Check that types match
                        if value_type != var_type {
                            return Err(CompileError::TypeMismatch {
                                expected: var_type.to_string(),
                                found: value_type.to_string(),
                            });
                        }
                        
                        Ok(())
                    }
                    AssignTarget::Index { array, index } => {
                        // Type check the array expression
                        let array_type = self.check_expression(array)?;
                        
                        // Type check the index expression (must be Int)
                        let index_type = self.check_expression(index)?;
                        if index_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "Int".to_string(),
                                found: index_type.to_string(),
                            });
                        }
                        
                        // Extract element type from array type
                        let elem_type = match array_type {
                            CheckerType::Array(elem_type, _size) => elem_type.as_ref().clone(),
                            _ => {
                                return Err(CompileError::Generic(
                                    format!("Cannot index into non-array type: {}", array_type)
                                ));
                            }
                        };
                        
                        // Type check the value expression
                        let value_type = self.check_expression(value)?;
                        
                        // Check that types match
                        if value_type != elem_type {
                            return Err(CompileError::TypeMismatch {
                                expected: elem_type.to_string(),
                                found: value_type.to_string(),
                            });
                        }
                        
                        Ok(())
                    }
                }
            }
            Stmt::If { condition, then_branch, else_branch, .. } => {
                // Type check the condition - must be Bool
                let cond_type = self.check_expression(condition)?;
                if cond_type != CheckerType::Bool {
                    return Err(CompileError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: cond_type.to_string(),
                    });
                }
                
                // Type check then branch in new scope
                self.symbols.enter_scope();
                for stmt in then_branch {
                    self.check_statement(stmt)?;
                }
                self.symbols.exit_scope();
                
                // Type check else branch in new scope if it exists
                if let Some(else_stmts) = else_branch {
                    self.symbols.enter_scope();
                    for stmt in else_stmts {
                        self.check_statement(stmt)?;
                    }
                    self.symbols.exit_scope();
                }
                
                Ok(())
            }
            Stmt::While { condition, body, .. } => {
                // Type check the condition - must be Bool
                let cond_type = self.check_expression(condition)?;
                if cond_type != CheckerType::Bool {
                    return Err(CompileError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: cond_type.to_string(),
                    });
                }
                
                // Type check body in new scope
                self.symbols.enter_scope();
                for stmt in body {
                    self.check_statement(stmt)?;
                }
                self.symbols.exit_scope();
                
                Ok(())
            }
            Stmt::For { var, iter, body, .. } => {
                // Type check the iterator expression
                let iter_type = self.check_expression(iter)?;
                
                // Extract element type from array
                let elem_type = match iter_type {
                    CheckerType::Array(elem_type, _size) => elem_type.as_ref().clone(),
                    _ => {
                        return Err(CompileError::Generic(
                            format!("For loop requires an array, found {}", iter_type)
                        ));
                    }
                };
                
                // Enter new scope for loop body
                self.symbols.enter_scope();
                
                // Define loop variable with element type
                self.symbols.define(var.clone(), elem_type, false)?;
                
                // Type check body
                for stmt in body {
                    self.check_statement(stmt)?;
                }
                
                self.symbols.exit_scope();
                
                Ok(())
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {
                // TODO: Check that we're inside a loop
                // For now, just allow them
                Ok(())
            }
        }
    }
    
    /// Type check an expression and return its type
    fn check_expression(&mut self, expr: &Expr) -> Result<CheckerType> {
        match expr {
            Expr::String(_) => Ok(CheckerType::String),
            Expr::Integer(_) => Ok(CheckerType::Int),
            Expr::Bool(_) => Ok(CheckerType::Bool),
            Expr::Ident(name) => {
                // First check if it's a variable
                if let Some(var_info) = self.symbols.lookup(name) {
                    return Ok(var_info.ty.clone());
                }
                
                // Then check if it's a function
                self.functions.get(name)
                    .cloned()
                    .ok_or_else(|| CompileError::Generic(
                        format!("Undefined variable or function: '{}'", name)
                    ))
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
            Expr::Binary { op, left, right, .. } => {
                let left_type = self.check_expression(left)?;
                let right_type = self.check_expression(right)?;
                
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                        // Arithmetic operations require both operands to be Int
                        if left_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "Int".to_string(),
                                found: left_type.to_string(),
                            });
                        }
                        if right_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "Int".to_string(),
                                found: right_type.to_string(),
                            });
                        }
                        Ok(CheckerType::Int)
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                        // Comparison operations require same types
                        if left_type != right_type {
                            return Err(CompileError::TypeMismatch {
                                expected: left_type.to_string(),
                                found: right_type.to_string(),
                            });
                        }
                        // Comparison operations return Bool
                        Ok(CheckerType::Bool)
                    }
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                if elements.is_empty() {
                    return Err(CompileError::Generic(
                        "Empty array literals are not supported (cannot infer type)".to_string()
                    ));
                }
                
                // Type check first element
                let elem_type = self.check_expression(&elements[0])?;
                
                // Check that all elements have the same type
                for elem in &elements[1..] {
                    let elem_expr_type = self.check_expression(elem)?;
                    if elem_expr_type != elem_type {
                        return Err(CompileError::TypeMismatch {
                            expected: elem_type.to_string(),
                            found: elem_expr_type.to_string(),
                        });
                    }
                }
                
                Ok(CheckerType::Array(Box::new(elem_type), elements.len()))
            }
            Expr::Index { array, index, .. } => {
                // Type check the array expression
                let array_type = self.check_expression(array)?;
                
                // Type check the index expression (must be Int)
                let index_type = self.check_expression(index)?;
                if index_type != CheckerType::Int {
                    return Err(CompileError::TypeMismatch {
                        expected: "Int".to_string(),
                        found: index_type.to_string(),
                    });
                }
                
                // Extract element type from array type
                match array_type {
                    CheckerType::Array(elem_type, _size) => Ok(elem_type.as_ref().clone()),
                    _ => Err(CompileError::Generic(
                        format!("Cannot index into non-array type: {}", array_type)
                    )),
                }
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
    }
    
    #[test]
    fn test_let_binding() {
        let source = r#"
        fn main() {
            let x = 42;
            let y: i32 = 10;
            let message = "Hello";
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
    fn test_variable_usage() {
        let source = r#"
        fn main() {
            let x = 42;
            let y = x;
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
    fn test_undefined_variable() {
        let source = r#"
        fn main() {
            let x = y;
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_binary_operations() {
        let source = r#"
        fn main() {
            let x = 10 + 20;
            let y = x - 5;
            let z = y * 2;
            let w = z / 3;
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
    fn test_type_mismatch_in_binary() {
        let source = r#"
        fn main() {
            let x = "hello" + 42;
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
        
        if let Err(CompileError::TypeMismatch { expected, found }) = result {
            assert_eq!(expected, "Int");
            assert_eq!(found, "String");
        }
    }
    
    #[test]
    fn test_type_annotation_mismatch() {
        let source = r#"
        fn main() {
            let x: i32 = "not an int";
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
        
        if let Err(CompileError::TypeMismatch { expected, found }) = result {
            assert_eq!(expected, "Int");
            assert_eq!(found, "String");
        }
    }
    
    #[test]
    fn test_variable_redefinition() {
        let source = r#"
        fn main() {
            let x = 42;
            let x = "redefined";
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_for_loop_type_checking() {
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
        
        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }
    
    #[test]
    fn test_for_loop_wrong_type() {
        let source = r#"
        fn main() {
            let x = 42;
            for i in x {
                print_int(i);
            }
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
        
        if let Err(CompileError::Generic(msg)) = result {
            assert!(msg.contains("For loop requires an array"));
        }
    }
    
    #[test]
    fn test_break_continue_in_loops() {
        let source = r#"
        fn main() {
            let arr = [1, 2, 3, 4, 5];
            
            // Test break and continue in while loop
            let mut i = 0;
            while i < 10 {
                if i == 5 {
                    break;
                }
                if i == 3 {
                    i = i + 1;
                    continue;
                }
                i = i + 1;
            }
            
            // Test break and continue in for loop
            for n in arr {
                if n == 3 {
                    continue;
                }
                if n > 4 {
                    break;
                }
                print_int(n);
            }
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }
}