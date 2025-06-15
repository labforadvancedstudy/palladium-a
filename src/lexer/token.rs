// Token definitions for Palladium
// "The atoms of legendary code"

use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\f]+")]
#[logos(skip r"//[^\n]*")]
#[logos(skip r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/")]
pub enum Token {
    // Literals
    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        // Remove quotes and handle escape sequences
        let content = &s[1..s.len()-1];
        let unescaped = content
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\r", "\r")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\");
        Some(unescaped)
    })]
    String(String),
    
    #[regex(r"-?[0-9]+", |lex| lex.slice().parse().ok())]
    Integer(i64),
    
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| Some(lex.slice().to_owned()))]
    Identifier(String),
    
    // Keywords
    #[token("fn")]
    Fn,
    
    #[token("let")]
    Let,
    
    #[token("mut")]
    Mut,
    
    #[token("if")]
    If,
    
    #[token("else")]
    Else,
    
    #[token("while")]
    While,
    
    #[token("return")]
    Return,
    
    #[token("true")]
    True,
    
    #[token("false")]
    False,
    
    // Operators
    #[token("+")]
    Plus,
    
    #[token("-")]
    Minus,
    
    #[token("*")]
    Star,
    
    #[token("/")]
    Slash,
    
    #[token("%")]
    Percent,
    
    #[token("=")]
    Eq,
    
    #[token("==")]
    EqEq,
    
    #[token("!=")]
    Ne,
    
    #[token("<")]
    Lt,
    
    #[token(">")]
    Gt,
    
    #[token("<=")]
    Le,
    
    #[token(">=")]
    Ge,
    
    // Delimiters
    #[token("(")]
    LeftParen,
    
    #[token(")")]
    RightParen,
    
    #[token("{")]
    LeftBrace,
    
    #[token("}")]
    RightBrace,
    
    #[token("[")]
    LeftBracket,
    
    #[token("]")]
    RightBracket,
    
    #[token(";")]
    Semicolon,
    
    #[token(",")]
    Comma,
    
    #[token(":")]
    Colon,
    
    #[token("->")]
    Arrow,
}

impl Token {
    /// Returns true if this token can start an expression
    pub fn can_start_expr(&self) -> bool {
        matches!(
            self,
            Token::String(_)
                | Token::Integer(_)
                | Token::Identifier(_)
                | Token::True
                | Token::False
                | Token::LeftParen
                | Token::Minus
        )
    }
    
    /// Returns true if this token can start a statement
    pub fn can_start_stmt(&self) -> bool {
        matches!(
            self,
            Token::Let | Token::Return | Token::If | Token::While
        ) || self.can_start_expr()
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::String(s) => write!(f, "string \"{}\"", s),
            Token::Integer(n) => write!(f, "integer {}", n),
            Token::Identifier(name) => write!(f, "identifier '{}'", name),
            Token::Fn => write!(f, "'fn'"),
            Token::Let => write!(f, "'let'"),
            Token::Mut => write!(f, "'mut'"),
            Token::If => write!(f, "'if'"),
            Token::Else => write!(f, "'else'"),
            Token::While => write!(f, "'while'"),
            Token::Return => write!(f, "'return'"),
            Token::True => write!(f, "'true'"),
            Token::False => write!(f, "'false'"),
            Token::Plus => write!(f, "'+'"),
            Token::Minus => write!(f, "'-'"),
            Token::Star => write!(f, "'*'"),
            Token::Slash => write!(f, "'/'"),
            Token::Percent => write!(f, "'%'"),
            Token::Eq => write!(f, "'='"),
            Token::EqEq => write!(f, "'=='"),
            Token::Ne => write!(f, "'!='"),
            Token::Lt => write!(f, "'<'"),
            Token::Gt => write!(f, "'>'"),
            Token::Le => write!(f, "'<='"),
            Token::Ge => write!(f, "'>='"),
            Token::LeftParen => write!(f, "'('"),
            Token::RightParen => write!(f, "')'"),
            Token::LeftBrace => write!(f, "'{{'"),
            Token::RightBrace => write!(f, "'}}'"),
            Token::LeftBracket => write!(f, "'['"),
            Token::RightBracket => write!(f, "']'"),
            Token::Semicolon => write!(f, "';'"),
            Token::Comma => write!(f, "','"),
            Token::Colon => write!(f, "':'"),
            Token::Arrow => write!(f, "'->'"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_string_lexing() {
        let mut lex = Token::lexer(r#""Hello, World!""#);
        assert_eq!(lex.next(), Some(Ok(Token::String("Hello, World!".to_string()))));
    }
    
    #[test]
    fn test_escaped_string() {
        let mut lex = Token::lexer(r#""Hello\nWorld\t!""#);
        assert_eq!(lex.next(), Some(Ok(Token::String("Hello\nWorld\t!".to_string()))));
    }
    
    #[test]
    fn test_integer() {
        let mut lex = Token::lexer("42 -17");
        assert_eq!(lex.next(), Some(Ok(Token::Integer(42))));
        assert_eq!(lex.next(), Some(Ok(Token::Integer(-17))));
    }
    
    #[test]
    fn test_identifiers_and_keywords() {
        let mut lex = Token::lexer("fn main print");
        assert_eq!(lex.next(), Some(Ok(Token::Fn)));
        assert_eq!(lex.next(), Some(Ok(Token::Identifier("main".to_string()))));
        assert_eq!(lex.next(), Some(Ok(Token::Identifier("print".to_string()))));
    }
}