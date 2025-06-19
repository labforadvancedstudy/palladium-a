// Macro pattern parser for Palladium
// "Parsing the patterns of power"

use crate::errors::{CompileError, Result};
use crate::lexer::Token;

/// Macro pattern element
#[derive(Debug, Clone, PartialEq)]
pub enum PatternElement {
    /// Literal token to match
    Literal(Token),
    /// Variable to capture (e.g., $expr:expr)
    Variable {
        name: String,
        kind: CaptureKind,
    },
    /// Repetition (e.g., $($x:expr),*)
    Repetition {
        pattern: Vec<PatternElement>,
        separator: Option<Token>,
        kind: RepetitionKind,
    },
}

/// Kind of capture variable
#[derive(Debug, Clone, PartialEq)]
pub enum CaptureKind {
    /// Expression
    Expr,
    /// Statement
    Stmt,
    /// Type
    Type,
    /// Pattern
    Pat,
    /// Identifier
    Ident,
    /// Literal
    Lit,
    /// Token tree (any tokens)
    Tt,
}

/// Kind of repetition
#[derive(Debug, Clone, PartialEq)]
pub enum RepetitionKind {
    /// Zero or more (*)
    ZeroOrMore,
    /// One or more (+)
    OneOrMore,
    /// Zero or one (?)
    ZeroOrOne,
}

/// Macro pattern parser
pub struct PatternParser {
    tokens: Vec<Token>,
    pos: usize,
}

impl PatternParser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }
    
    /// Parse a macro pattern
    pub fn parse_pattern(&mut self) -> Result<Vec<PatternElement>> {
        let mut elements = Vec::new();
        
        while !self.is_at_end() {
            elements.push(self.parse_element()?);
        }
        
        Ok(elements)
    }
    
    /// Parse a single pattern element
    fn parse_element(&mut self) -> Result<PatternElement> {
        if self.check_token(&Token::Dollar) {
            self.parse_capture_or_repetition()
        } else {
            // Literal token
            let token = self.advance()?;
            Ok(PatternElement::Literal(token))
        }
    }
    
    /// Parse a capture variable or repetition
    fn parse_capture_or_repetition(&mut self) -> Result<PatternElement> {
        self.consume(Token::Dollar)?;
        
        if self.check_token(&Token::LeftParen) {
            // Repetition: $(...)
            self.parse_repetition()
        } else {
            // Capture variable: $name:kind
            self.parse_capture()
        }
    }
    
    /// Parse a capture variable
    fn parse_capture(&mut self) -> Result<PatternElement> {
        let name = self.expect_ident()?;
        self.consume(Token::Colon)?;
        let kind = self.parse_capture_kind()?;
        
        Ok(PatternElement::Variable { name, kind })
    }
    
    /// Parse capture kind
    fn parse_capture_kind(&mut self) -> Result<CaptureKind> {
        let kind_name = self.expect_ident()?;
        
        match kind_name.as_str() {
            "expr" => Ok(CaptureKind::Expr),
            "stmt" => Ok(CaptureKind::Stmt),
            "type" => Ok(CaptureKind::Type),
            "pat" => Ok(CaptureKind::Pat),
            "ident" => Ok(CaptureKind::Ident),
            "lit" => Ok(CaptureKind::Lit),
            "tt" => Ok(CaptureKind::Tt),
            _ => Err(CompileError::Generic(format!(
                "Unknown capture kind: {}",
                kind_name
            ))),
        }
    }
    
    /// Parse a repetition
    fn parse_repetition(&mut self) -> Result<PatternElement> {
        self.consume(Token::LeftParen)?;
        
        // Parse inner pattern
        let mut pattern = Vec::new();
        while !self.check_token(&Token::RightParen) {
            pattern.push(self.parse_element()?);
        }
        
        self.consume(Token::RightParen)?;
        
        // Parse separator (optional)
        let separator = if self.check_token(&Token::Comma) 
            || self.check_token(&Token::Semicolon) {
            Some(self.advance()?)
        } else {
            None
        };
        
        // Parse repetition kind
        let kind = if self.check_token(&Token::Star) {
            self.advance()?;
            RepetitionKind::ZeroOrMore
        } else if self.check_token(&Token::Plus) {
            self.advance()?;
            RepetitionKind::OneOrMore
        } else if self.check_token(&Token::Question) {
            self.advance()?;
            RepetitionKind::ZeroOrOne
        } else {
            return Err(CompileError::Generic(
                "Expected repetition operator (*, +, or ?)".to_string()
            ));
        };
        
        Ok(PatternElement::Repetition {
            pattern,
            separator,
            kind,
        })
    }
    
    /// Check if we're at the end
    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }
    
    /// Check if current token matches
    fn check_token(&self, expected: &Token) -> bool {
        if self.is_at_end() {
            false
        } else {
            std::mem::discriminant(&self.tokens[self.pos]) == std::mem::discriminant(expected)
        }
    }
    
    /// Consume a specific token
    fn consume(&mut self, expected: Token) -> Result<()> {
        if self.check_token(&expected) {
            self.advance()?;
            Ok(())
        } else {
            Err(CompileError::Generic(format!(
                "Expected {:?}, found {:?}",
                expected,
                self.current()
            )))
        }
    }
    
    /// Advance to next token
    fn advance(&mut self) -> Result<Token> {
        if self.is_at_end() {
            Err(CompileError::Generic("Unexpected end of input".to_string()))
        } else {
            let token = self.tokens[self.pos].clone();
            self.pos += 1;
            Ok(token)
        }
    }
    
    /// Get current token
    fn current(&self) -> Option<&Token> {
        if self.is_at_end() {
            None
        } else {
            Some(&self.tokens[self.pos])
        }
    }
    
    /// Expect an identifier
    fn expect_ident(&mut self) -> Result<String> {
        match self.advance()? {
            Token::Identifier(name) => Ok(name),
            other => Err(CompileError::Generic(format!(
                "Expected identifier, found {:?}",
                other
            ))),
        }
    }
}