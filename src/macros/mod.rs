// Macro expansion for Palladium
// "Expanding the code's possibilities"

pub mod parser;
pub mod expander;

use crate::ast::{MacroDef, Program, Item, Expr, Stmt};
use crate::errors::{CompileError, Result};
use crate::lexer::{Token, Lexer};
use std::collections::HashMap;
use parser::{PatternParser, PatternElement};
use expander::{match_pattern, substitute_template, expand_to_expr, expand_to_stmts};

/// Parsed macro definition
#[derive(Clone)]
struct ParsedMacro {
    name: String,
    pattern: Vec<PatternElement>,
    template: Vec<Token>,
}

pub struct MacroExpander {
    /// Registered macros (name -> definition)
    macros: HashMap<String, ParsedMacro>,
}

impl MacroExpander {
    pub fn new() -> Self {
        let mut expander = Self {
            macros: HashMap::new(),
        };
        
        // Register built-in macros
        expander.register_builtin_macros();
        
        expander
    }
    
    /// Register built-in macros
    fn register_builtin_macros(&mut self) {
        // println! macro
        self.register_builtin_println();
        
        // assert! macro
        self.register_builtin_assert();
        
        // vec! macro
        self.register_builtin_vec();
        
        // dbg! macro
        self.register_builtin_dbg();
    }
    
    /// Register println! macro
    fn register_builtin_println(&mut self) {
        // println!() -> print("\n")
        // println!($expr:expr) -> print($expr); print("\n")
        // println!($expr:expr, $($args:expr),*) -> print($expr); $(print(" "); print($args);)* print("\n")
        
        // For now, simplified version
        let pattern = vec![
            PatternElement::Variable {
                name: "msg".to_string(),
                kind: parser::CaptureKind::Expr,
            },
        ];
        
        let template = vec![
            Token::Identifier("print".to_string()),
            Token::LeftParen,
            Token::Dollar,
            Token::Identifier("msg".to_string()),
            Token::RightParen,
            Token::Semicolon,
            Token::Identifier("print".to_string()),
            Token::LeftParen,
            Token::String("\\n".to_string()),
            Token::RightParen,
        ];
        
        self.macros.insert("println".to_string(), ParsedMacro {
            name: "println".to_string(),
            pattern,
            template,
        });
    }
    
    /// Register assert! macro
    fn register_builtin_assert(&mut self) {
        // assert!($cond:expr) -> if (!$cond) { panic("Assertion failed"); }
        let pattern = vec![
            PatternElement::Variable {
                name: "cond".to_string(),
                kind: parser::CaptureKind::Expr,
            },
        ];
        
        let template = vec![
            Token::If,
            Token::LeftParen,
            Token::Not,
            Token::LeftParen,
            Token::Dollar,
            Token::Identifier("cond".to_string()),
            Token::RightParen,
            Token::RightParen,
            Token::LeftBrace,
            Token::Identifier("panic".to_string()),
            Token::LeftParen,
            Token::String("Assertion failed".to_string()),
            Token::RightParen,
            Token::Semicolon,
            Token::RightBrace,
        ];
        
        self.macros.insert("assert".to_string(), ParsedMacro {
            name: "assert".to_string(),
            pattern,
            template,
        });
    }
    
    /// Register vec! macro
    fn register_builtin_vec(&mut self) {
        // vec![$($elem:expr),*] -> { let mut v = Vec::new(); $(v.push($elem);)* v }
        // Simplified: vec![$elem:expr] -> [$elem]
        let pattern = vec![
            PatternElement::Variable {
                name: "elem".to_string(),
                kind: parser::CaptureKind::Expr,
            },
        ];
        
        let template = vec![
            Token::LeftBracket,
            Token::Dollar,
            Token::Identifier("elem".to_string()),
            Token::RightBracket,
        ];
        
        self.macros.insert("vec".to_string(), ParsedMacro {
            name: "vec".to_string(),
            pattern,
            template,
        });
    }
    
    /// Register dbg! macro
    fn register_builtin_dbg(&mut self) {
        // dbg!($expr:expr) -> { print("DEBUG: "); print($expr); print("\n"); $expr }
        let pattern = vec![
            PatternElement::Variable {
                name: "expr".to_string(),
                kind: parser::CaptureKind::Expr,
            },
        ];
        
        let template = vec![
            Token::LeftBrace,
            Token::Identifier("print".to_string()),
            Token::LeftParen,
            Token::String("DEBUG: ".to_string()),
            Token::RightParen,
            Token::Semicolon,
            Token::Identifier("print_debug".to_string()),
            Token::LeftParen,
            Token::Dollar,
            Token::Identifier("expr".to_string()),
            Token::RightParen,
            Token::Semicolon,
            Token::Identifier("print".to_string()),
            Token::LeftParen,
            Token::String("\\n".to_string()),
            Token::RightParen,
            Token::Semicolon,
            Token::Dollar,
            Token::Identifier("expr".to_string()),
            Token::RightBrace,
        ];
        
        self.macros.insert("dbg".to_string(), ParsedMacro {
            name: "dbg".to_string(),
            pattern,
            template,
        });
    }

    /// Expand macros in a program
    pub fn expand_program(&mut self, program: &mut Program) -> Result<()> {
        // First pass: collect macro definitions
        let mut new_items = Vec::new();
        
        for item in &program.items {
            match item {
                Item::Macro(macro_def) => {
                    // Parse and register the macro
                    self.register_macro(macro_def)?;
                }
                _ => {
                    // Keep non-macro items
                    new_items.push(item.clone());
                }
            }
        }
        
        // Update program items (removing macro definitions)
        program.items = new_items;
        
        // Second pass: expand macro invocations in the remaining items
        for item in &mut program.items {
            self.expand_item(item)?;
        }
        
        Ok(())
    }
    
    /// Register a macro definition
    fn register_macro(&mut self, macro_def: &MacroDef) -> Result<()> {
        if self.macros.contains_key(&macro_def.name) {
            return Err(CompileError::Generic(format!(
                "Macro '{}' is already defined",
                macro_def.name
            )));
        }
        
        // For now, convert params to a simple pattern
        // macro! name($param1:expr, $param2:expr) -> pattern with variables
        let mut pattern = Vec::new();
        for (i, param) in macro_def.params.iter().enumerate() {
            if i > 0 {
                pattern.push(PatternElement::Literal(crate::lexer::Token::Comma));
            }
            pattern.push(PatternElement::Variable {
                name: param.clone(),
                kind: parser::CaptureKind::Expr, // Default to expr for now
            });
        }
        
        // Convert AST tokens to lexer tokens
        let template_tokens = self.convert_ast_tokens_to_lexer_tokens(&macro_def.body)?;
        
        self.macros.insert(macro_def.name.clone(), ParsedMacro {
            name: macro_def.name.clone(),
            pattern,
            template: template_tokens,
        });
        
        Ok(())
    }
    
    /// Tokenize a pattern string
    fn tokenize_pattern(&self, pattern: &str) -> Result<Vec<Token>> {
        let mut lexer = Lexer::new(pattern);
        let tokens_with_spans = lexer.collect_tokens()?;
        Ok(tokens_with_spans.into_iter().map(|(token, _)| token).collect())
    }
    
    /// Tokenize a template string
    fn tokenize_template(&self, template: &str) -> Result<Vec<Token>> {
        let mut lexer = Lexer::new(template);
        let tokens_with_spans = lexer.collect_tokens()?;
        Ok(tokens_with_spans.into_iter().map(|(token, _)| token).collect())
    }
    
    /// Convert AST tokens to lexer tokens
    fn convert_ast_tokens_to_lexer_tokens(&self, ast_tokens: &[crate::ast::Token]) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        
        for ast_token in ast_tokens {
            match ast_token {
                crate::ast::Token::Ident(s) => tokens.push(Token::Identifier(s.clone())),
                crate::ast::Token::Literal(s) => {
                    // Try to parse as integer or string
                    if let Ok(n) = s.parse::<i64>() {
                        tokens.push(Token::Integer(n));
                    } else {
                        tokens.push(Token::String(s.clone()));
                    }
                }
                crate::ast::Token::Punct(ch) => {
                    match ch {
                        '+' => tokens.push(Token::Plus),
                        '-' => tokens.push(Token::Minus),
                        '*' => tokens.push(Token::Star),
                        '/' => tokens.push(Token::Slash),
                        '%' => tokens.push(Token::Percent),
                        '=' => tokens.push(Token::Eq),
                        '<' => tokens.push(Token::Lt),
                        '>' => tokens.push(Token::Gt),
                        '!' => tokens.push(Token::Not),
                        '&' => tokens.push(Token::Ampersand),
                        '|' => tokens.push(Token::Pipe),
                        '(' => tokens.push(Token::LeftParen),
                        ')' => tokens.push(Token::RightParen),
                        '{' => tokens.push(Token::LeftBrace),
                        '}' => tokens.push(Token::RightBrace),
                        '[' => tokens.push(Token::LeftBracket),
                        ']' => tokens.push(Token::RightBracket),
                        ';' => tokens.push(Token::Semicolon),
                        ',' => tokens.push(Token::Comma),
                        ':' => tokens.push(Token::Colon),
                        '.' => tokens.push(Token::Dot),
                        '?' => tokens.push(Token::Question),
                        '$' => tokens.push(Token::Dollar),
                        _ => return Err(CompileError::Generic(format!("Unknown punct: {}", ch))),
                    }
                }
                crate::ast::Token::Group(delim, inner_tokens) => {
                    // Add opening delimiter
                    match delim {
                        crate::ast::Delimiter::Paren => tokens.push(Token::LeftParen),
                        crate::ast::Delimiter::Brace => tokens.push(Token::LeftBrace),
                        crate::ast::Delimiter::Bracket => tokens.push(Token::LeftBracket),
                    }
                    
                    // Convert inner tokens
                    tokens.extend(self.convert_ast_tokens_to_lexer_tokens(inner_tokens)?);
                    
                    // Add closing delimiter
                    match delim {
                        crate::ast::Delimiter::Paren => tokens.push(Token::RightParen),
                        crate::ast::Delimiter::Brace => tokens.push(Token::RightBrace),
                        crate::ast::Delimiter::Bracket => tokens.push(Token::RightBracket),
                    }
                }
            }
        }
        
        Ok(tokens)
    }
    
    /// Expand macros in an item
    fn expand_item(&mut self, item: &mut Item) -> Result<()> {
        match item {
            Item::Function(func) => {
                // Expand macros in function body
                let mut new_body = Vec::new();
                for stmt in &func.body {
                    new_body.extend(self.expand_stmt(stmt)?);
                }
                func.body = new_body;
            }
            // TODO: Handle other item types that might contain expressions
            _ => {}
        }
        Ok(())
    }
    
    /// Expand macros in a statement (returns possibly multiple statements)
    fn expand_stmt(&mut self, stmt: &Stmt) -> Result<Vec<Stmt>> {
        let mut result = vec![stmt.clone()];
        
        match &mut result[0] {
            Stmt::Expr(expr) => {
                if let Expr::MacroInvocation { .. } = expr {
                    // Expand macro to statements
                    return self.expand_macro_to_stmts(expr);
                } else {
                    self.expand_expr(expr)?;
                }
            }
            Stmt::Let { value, .. } => self.expand_expr(value)?,
            Stmt::Assign { value, .. } => self.expand_expr(value)?,
            Stmt::Return(Some(expr)) => self.expand_expr(expr)?,
            Stmt::If { condition, then_branch, else_branch, .. } => {
                self.expand_expr(condition)?;
                *then_branch = self.expand_stmts(then_branch)?;
                if let Some(else_stmts) = else_branch {
                    *else_stmts = self.expand_stmts(else_stmts)?;
                }
            }
            Stmt::While { condition, body, .. } => {
                self.expand_expr(condition)?;
                *body = self.expand_stmts(body)?;
            }
            Stmt::For { iter, body, .. } => {
                self.expand_expr(iter)?;
                *body = self.expand_stmts(body)?;
            }
            Stmt::Match { expr, arms, .. } => {
                self.expand_expr(expr)?;
                for arm in arms {
                    arm.body = self.expand_stmts(&arm.body)?;
                }
            }
            Stmt::Unsafe { body, .. } => {
                *body = self.expand_stmts(body)?;
            }
            _ => {}
        }
        
        Ok(result)
    }
    
    /// Expand statements
    fn expand_stmts(&mut self, stmts: &[Stmt]) -> Result<Vec<Stmt>> {
        let mut result = Vec::new();
        for stmt in stmts {
            result.extend(self.expand_stmt(stmt)?);
        }
        Ok(result)
    }
    
    /// Expand macro invocation to statements
    fn expand_macro_to_stmts(&mut self, expr: &Expr) -> Result<Vec<Stmt>> {
        if let Expr::MacroInvocation { name, args, .. } = expr {
            // Look up the macro
            let parsed_macro = self.macros.get(name).ok_or_else(|| {
                CompileError::Generic(format!("Unknown macro '{}'", name))
            })?.clone();
            
            // Convert AST tokens to lexer tokens
            let arg_tokens = self.convert_ast_tokens_to_lexer_tokens(args)?;
            
            // Match pattern
            if let Some(context) = match_pattern(&parsed_macro.pattern, &arg_tokens)? {
                // Substitute template
                let expanded_tokens = substitute_template(&parsed_macro.template, &context)?;
                
                // Parse as statements
                return expand_to_stmts(expanded_tokens);
            } else {
                return Err(CompileError::Generic(format!(
                    "Macro '{}' arguments don't match pattern",
                    name
                )));
            }
        }
        
        Ok(vec![Stmt::Expr(expr.clone())])
    }
    
    /// Expand macros in an expression
    fn expand_expr(&mut self, expr: &mut Expr) -> Result<()> {
        match expr {
            Expr::MacroInvocation { name, args, .. } => {
                // Look up the macro
                let parsed_macro = self.macros.get(name).ok_or_else(|| {
                    CompileError::Generic(format!("Unknown macro '{}'", name))
                })?.clone();
                
                // Convert AST tokens to lexer tokens
                let arg_tokens = self.convert_ast_tokens_to_lexer_tokens(args)?;
                
                // Match pattern
                if let Some(context) = match_pattern(&parsed_macro.pattern, &arg_tokens)? {
                    // Substitute template
                    let expanded_tokens = substitute_template(&parsed_macro.template, &context)?;
                    
                    // Parse as expression
                    *expr = expand_to_expr(expanded_tokens)?;
                } else {
                    return Err(CompileError::Generic(format!(
                        "Macro '{}' arguments don't match pattern",
                        name
                    )));
                }
            }
            
            // Recursively expand in other expression types
            Expr::Binary { left, right, .. } => {
                self.expand_expr(left)?;
                self.expand_expr(right)?;
            }
            Expr::Unary { operand, .. } => {
                self.expand_expr(operand)?;
            }
            Expr::Call { func, args, .. } => {
                self.expand_expr(func)?;
                for arg in args {
                    self.expand_expr(arg)?;
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                for elem in elements {
                    self.expand_expr(elem)?;
                }
            }
            Expr::ArrayRepeat { value, count, .. } => {
                self.expand_expr(value)?;
                self.expand_expr(count)?;
            }
            Expr::Index { array, index, .. } => {
                self.expand_expr(array)?;
                self.expand_expr(index)?;
            }
            Expr::StructLiteral { fields, .. } => {
                for (_, field_expr) in fields {
                    self.expand_expr(field_expr)?;
                }
            }
            Expr::FieldAccess { object, .. } => {
                self.expand_expr(object)?;
            }
            Expr::EnumConstructor { data, .. } => {
                if let Some(data) = data {
                    match data {
                        crate::ast::EnumConstructorData::Tuple(exprs) => {
                            for e in exprs {
                                self.expand_expr(e)?;
                            }
                        }
                        crate::ast::EnumConstructorData::Struct(fields) => {
                            for (_, e) in fields {
                                self.expand_expr(e)?;
                            }
                        }
                    }
                }
            }
            Expr::Range { start, end, .. } => {
                self.expand_expr(start)?;
                self.expand_expr(end)?;
            }
            Expr::Reference { expr, .. } => {
                self.expand_expr(expr)?;
            }
            Expr::Deref { expr, .. } => {
                self.expand_expr(expr)?;
            }
            Expr::Question { expr, .. } => {
                self.expand_expr(expr)?;
            }
            // Literals don't need expansion
            _ => {}
        }
        Ok(())
    }
}

impl Default for MacroExpander {
    fn default() -> Self {
        Self::new()
    }
}