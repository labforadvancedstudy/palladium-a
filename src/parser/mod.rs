// Parser for Palladium
// "Constructing legends from tokens"

use crate::ast::*;
use crate::errors::{CompileError, Result, Span};
use crate::lexer::Token;

pub struct Parser {
    tokens: Vec<(Token, Span)>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, Span)>) -> Self {
        Self { tokens, current: 0 }
    }
    
    /// Parse a complete program
    pub fn parse(&mut self) -> Result<Program> {
        let mut items = Vec::new();
        
        while !self.is_at_end() {
            items.push(self.parse_item()?);
        }
        
        Ok(Program { items })
    }
    
    /// Parse a top-level item
    fn parse_item(&mut self) -> Result<Item> {
        match self.peek()? {
            Token::Fn => Ok(Item::Function(self.parse_function()?)),
            _ => Err(CompileError::SyntaxError {
                message: "Expected function declaration".to_string(),
            }),
        }
    }
    
    /// Parse a function declaration
    fn parse_function(&mut self) -> Result<Function> {
        let start_span = self.consume(Token::Fn, "Expected 'fn'")?;
        
        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "function name".to_string(),
                    found: token.to_string(),
                });
            }
        };
        
        self.consume(Token::LeftParen, "Expected '('")?;
        
        // For v0.1, we don't support parameters
        let params = Vec::new();
        
        self.consume(Token::RightParen, "Expected ')'")?;
        
        // Parse return type if present
        let return_type = if self.check(&Token::Arrow) {
            self.advance()?; // consume '->'
            Some(self.parse_type()?)
        } else {
            None
        };
        
        self.consume(Token::LeftBrace, "Expected '{'")?;
        
        let mut body = Vec::new();
        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            body.push(self.parse_statement()?);
        }
        
        let end_span = self.consume(Token::RightBrace, "Expected '}'")?;
        
        Ok(Function {
            name,
            params,
            return_type,
            body,
            span: Span::new(start_span.start, end_span.end, start_span.line, start_span.column),
        })
    }
    
    /// Parse a statement
    fn parse_statement(&mut self) -> Result<Stmt> {
        match self.peek()? {
            Token::Let => self.parse_let(),
            Token::Return => self.parse_return(),
            Token::If => self.parse_if(),
            _ => {
                // Expression statement
                let expr = self.parse_expression()?;
                self.consume(Token::Semicolon, "Expected ';' after expression")?;
                Ok(Stmt::Expr(expr))
            }
        }
    }
    
    /// Parse a return statement
    fn parse_return(&mut self) -> Result<Stmt> {
        self.consume(Token::Return, "Expected 'return'")?;
        
        if self.check(&Token::Semicolon) {
            self.advance()?;
            Ok(Stmt::Return(None))
        } else {
            let expr = self.parse_expression()?;
            self.consume(Token::Semicolon, "Expected ';' after return value")?;
            Ok(Stmt::Return(Some(expr)))
        }
    }
    
    /// Parse a let statement
    fn parse_let(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::Let, "Expected 'let'")?;
        
        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "variable name".to_string(),
                    found: token.to_string(),
                });
            }
        };
        
        // Optional type annotation
        let ty = if self.check(&Token::Colon) {
            self.advance()?; // consume ':'
            Some(self.parse_type()?)
        } else {
            None
        };
        
        self.consume(Token::Eq, "Expected '=' after variable name")?;
        let value = self.parse_expression()?;
        let end_span = self.consume(Token::Semicolon, "Expected ';' after let statement")?;
        
        Ok(Stmt::Let { 
            name, 
            ty,
            value, 
            span: Span::new(start_span.start, end_span.end, start_span.line, start_span.column) 
        })
    }
    
    /// Parse an if statement
    fn parse_if(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::If, "Expected 'if'")?;
        
        let condition = self.parse_expression()?;
        
        self.consume(Token::LeftBrace, "Expected '{' after if condition")?;
        
        let mut then_branch = Vec::new();
        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            then_branch.push(self.parse_statement()?);
        }
        
        self.consume(Token::RightBrace, "Expected '}' after if body")?;
        
        let else_branch = if self.check(&Token::Else) {
            self.advance()?; // consume 'else'
            self.consume(Token::LeftBrace, "Expected '{' after else")?;
            
            let mut else_stmts = Vec::new();
            while !self.check(&Token::RightBrace) && !self.is_at_end() {
                else_stmts.push(self.parse_statement()?);
            }
            
            let end_span = self.consume(Token::RightBrace, "Expected '}' after else body")?;
            Some(else_stmts)
        } else {
            None
        };
        
        let end_span = if else_branch.is_some() {
            self.tokens[self.current - 1].1
        } else {
            self.tokens[self.current - 1].1
        };
        
        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
            span: Span::new(start_span.start, end_span.end, start_span.line, start_span.column),
        })
    }
    
    /// Parse an expression
    fn parse_expression(&mut self) -> Result<Expr> {
        self.parse_equality()
    }
    
    /// Parse equality operators (==, !=)
    fn parse_equality(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;
        
        while let Ok(token) = self.peek() {
            match token {
                Token::EqEq | Token::Ne => {
                    let op = match self.advance()?.0 {
                        Token::EqEq => BinOp::Eq,
                        Token::Ne => BinOp::Ne,
                        _ => unreachable!(),
                    };
                    let right = self.parse_comparison()?;
                    let span = Span::dummy(); // TODO: proper span tracking
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }
        
        Ok(left)
    }
    
    /// Parse comparison operators (<, >, <=, >=)
    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_addition()?;
        
        while let Ok(token) = self.peek() {
            match token {
                Token::Lt | Token::Gt | Token::Le | Token::Ge => {
                    let op = match self.advance()?.0 {
                        Token::Lt => BinOp::Lt,
                        Token::Gt => BinOp::Gt,
                        Token::Le => BinOp::Le,
                        Token::Ge => BinOp::Ge,
                        _ => unreachable!(),
                    };
                    let right = self.parse_addition()?;
                    let span = Span::dummy(); // TODO: proper span tracking
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }
        
        Ok(left)
    }
    
    /// Parse addition and subtraction
    fn parse_addition(&mut self) -> Result<Expr> {
        let mut left = self.parse_multiplication()?;
        
        while let Ok(token) = self.peek() {
            match token {
                Token::Plus | Token::Minus => {
                    let op = match self.advance()?.0 {
                        Token::Plus => BinOp::Add,
                        Token::Minus => BinOp::Sub,
                        _ => unreachable!(),
                    };
                    let right = self.parse_multiplication()?;
                    let span = Span::dummy(); // TODO: proper span tracking
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }
        
        Ok(left)
    }
    
    /// Parse multiplication and division
    fn parse_multiplication(&mut self) -> Result<Expr> {
        let mut left = self.parse_primary()?;
        
        while let Ok(token) = self.peek() {
            match token {
                Token::Star | Token::Slash | Token::Percent => {
                    let op = match self.advance()?.0 {
                        Token::Star => BinOp::Mul,
                        Token::Slash => BinOp::Div,
                        Token::Percent => BinOp::Mod,
                        _ => unreachable!(),
                    };
                    let right = self.parse_primary()?;
                    let span = Span::dummy(); // TODO: proper span tracking
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }
        
        Ok(left)
    }
    
    /// Parse a type
    fn parse_type(&mut self) -> Result<Type> {
        match self.advance()? {
            (Token::Identifier(name), _) => {
                match name.as_str() {
                    "i32" => Ok(Type::I32),
                    "i64" => Ok(Type::I64),
                    "u32" => Ok(Type::U32),
                    "u64" => Ok(Type::U64),
                    "bool" => Ok(Type::Bool),
                    "String" => Ok(Type::String),
                    _ => Ok(Type::Custom(name)),
                }
            }
            (Token::LeftParen, _) => {
                self.consume(Token::RightParen, "Expected ')' for unit type")?;
                Ok(Type::Unit)
            }
            (token, _) => Err(CompileError::UnexpectedToken {
                expected: "type".to_string(),
                found: token.to_string(),
            }),
        }
    }
    
    /// Parse a primary expression
    fn parse_primary(&mut self) -> Result<Expr> {
        match self.advance()? {
            (Token::String(s), _) => Ok(Expr::String(s)),
            (Token::Integer(n), _) => Ok(Expr::Integer(n)),
            (Token::True, _) => Ok(Expr::Bool(true)),
            (Token::False, _) => Ok(Expr::Bool(false)),
            (Token::Identifier(name), span) => {
                // Check if this is a function call
                if self.check(&Token::LeftParen) {
                    self.advance()?; // consume '('
                    
                    let mut args = Vec::new();
                    
                    if !self.check(&Token::RightParen) {
                        loop {
                            args.push(self.parse_expression()?);
                            
                            if !self.check(&Token::Comma) {
                                break;
                            }
                            self.advance()?; // consume ','
                        }
                    }
                    
                    let end_span = self.consume(Token::RightParen, "Expected ')'")?;
                    
                    Ok(Expr::Call {
                        func: Box::new(Expr::Ident(name)),
                        args,
                        span: Span::new(span.start, end_span.end, span.line, span.column),
                    })
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            (Token::LeftParen, _) => {
                // Parse parenthesized expression
                let expr = self.parse_expression()?;
                self.consume(Token::RightParen, "Expected ')' after expression")?;
                Ok(expr)
            }
            (token, _) => Err(CompileError::UnexpectedToken {
                expected: "expression".to_string(),
                found: token.to_string(),
            }),
        }
    }
    
    // Helper methods
    
    /// Check if we're at the end of tokens
    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len()
    }
    
    /// Peek at the current token without consuming it
    fn peek(&self) -> Result<&Token> {
        if self.is_at_end() {
            Err(CompileError::SyntaxError {
                message: "Unexpected end of file".to_string(),
            })
        } else {
            Ok(&self.tokens[self.current].0)
        }
    }
    
    /// Check if the current token matches the given token
    fn check(&self, token: &Token) -> bool {
        if self.is_at_end() {
            false
        } else {
            std::mem::discriminant(&self.tokens[self.current].0) == std::mem::discriminant(token)
        }
    }
    
    /// Advance to the next token
    fn advance(&mut self) -> Result<(Token, Span)> {
        if self.is_at_end() {
            Err(CompileError::SyntaxError {
                message: "Unexpected end of file".to_string(),
            })
        } else {
            let token = self.tokens[self.current].clone();
            self.current += 1;
            Ok(token)
        }
    }
    
    /// Consume a specific token or error
    fn consume(&mut self, expected: Token, _message: &str) -> Result<Span> {
        let (token, span) = self.advance()?;
        
        if std::mem::discriminant(&token) == std::mem::discriminant(&expected) {
            Ok(span)
        } else {
            Err(CompileError::UnexpectedToken {
                expected: expected.to_string(),
                found: token.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    
    #[test]
    fn test_parse_hello_world() {
        let source = r#"
        fn main() {
            print("Hello, World!");
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        assert_eq!(ast.items.len(), 1);
        
        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.params.len(), 0);
            assert_eq!(func.return_type, None);
            assert_eq!(func.body.len(), 1);
            
            if let Stmt::Expr(Expr::Call { func: _, args, .. }) = &func.body[0] {
                assert_eq!(args.len(), 1);
                if let Expr::String(s) = &args[0] {
                    assert_eq!(s, "Hello, World!");
                }
            }
        }
    }
    
    #[test]
    fn test_parse_function_with_return_type() {
        let source = r#"
        fn main() -> i32 {
            return 0;
        }
        "#;
        
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        
        assert_eq!(ast.items.len(), 1);
        
        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.params.len(), 0);
            assert_eq!(func.return_type, Some(Type::I32));
            assert_eq!(func.body.len(), 1);
            
            if let Stmt::Return(Some(Expr::Integer(n))) = &func.body[0] {
                assert_eq!(*n, 0);
            } else {
                panic!("Expected return statement with integer");
            }
        }
    }
}