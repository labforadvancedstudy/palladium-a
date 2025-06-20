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

    #[token("for")]
    For,

    #[token("in")]
    In,

    #[token("break")]
    Break,

    #[token("continue")]
    Continue,

    #[token("struct")]
    Struct,

    #[token("enum")]
    Enum,

    #[token("trait")]
    Trait,

    #[token("impl")]
    Impl,

    #[token("match")]
    Match,

    #[token("import")]
    Import,

    #[token("pub")]
    Pub,

    #[token("as")]
    As,

    #[token("Self")]
    SelfType,

    #[token("self")]
    SelfParam,

    #[token("type")]
    Type,

    #[token("const")]
    Const,

    #[token("unsafe")]
    Unsafe,

    #[token("async")]
    Async,

    #[token("await")]
    Await,

    #[token("macro")]
    Macro,

    // Operators
    #[token("+")]
    Plus,

    #[token("-")]
    Minus,

    #[token("*")]
    Star,

    #[token("&")]
    Ampersand,

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

    #[token("!")]
    Not,

    #[token("<")]
    Lt,

    #[token(">")]
    Gt,

    #[token("<=")]
    Le,

    #[token(">=")]
    Ge,

    #[token("&&")]
    AndAnd,

    #[token("||")]
    OrOr,

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

    #[token(".")]
    Dot,

    #[token("..")]
    DotDot,

    #[token("->")]
    Arrow,

    #[token("::")]
    DoubleColon,

    #[token("_", priority = 10)]
    Underscore,

    #[token("=>")]
    FatArrow,

    #[token("'")]
    SingleQuote,

    #[token("?")]
    Question,

    #[token("$")]
    Dollar,

    #[token("|")]
    Pipe,

    // End of file marker (not produced by logos)
    Eof,
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
                | Token::Not
        )
    }

    /// Returns true if this token can start a statement
    pub fn can_start_stmt(&self) -> bool {
        matches!(
            self,
            Token::Let
                | Token::Return
                | Token::If
                | Token::While
                | Token::For
                | Token::Break
                | Token::Continue
                | Token::Match
                | Token::Unsafe
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
            Token::For => write!(f, "'for'"),
            Token::In => write!(f, "'in'"),
            Token::Break => write!(f, "'break'"),
            Token::Continue => write!(f, "'continue'"),
            Token::Struct => write!(f, "'struct'"),
            Token::Enum => write!(f, "'enum'"),
            Token::Trait => write!(f, "'trait'"),
            Token::Impl => write!(f, "'impl'"),
            Token::Match => write!(f, "'match'"),
            Token::Import => write!(f, "'import'"),
            Token::Pub => write!(f, "'pub'"),
            Token::As => write!(f, "'as'"),
            Token::SelfType => write!(f, "'Self'"),
            Token::Type => write!(f, "'type'"),
            Token::Unsafe => write!(f, "'unsafe'"),
            Token::Macro => write!(f, "'macro'"),
            Token::Plus => write!(f, "'+'"),
            Token::Minus => write!(f, "'-'"),
            Token::Star => write!(f, "'*'"),
            Token::Slash => write!(f, "'/'"),
            Token::Percent => write!(f, "'%'"),
            Token::Eq => write!(f, "'='"),
            Token::EqEq => write!(f, "'=='"),
            Token::Ne => write!(f, "'!='"),
            Token::Not => write!(f, "'!'"),
            Token::Lt => write!(f, "'<'"),
            Token::Gt => write!(f, "'>'"),
            Token::Le => write!(f, "'<='"),
            Token::Ge => write!(f, "'>='"),
            Token::AndAnd => write!(f, "'&&'"),
            Token::OrOr => write!(f, "'||'"),
            Token::LeftParen => write!(f, "'('"),
            Token::RightParen => write!(f, "')'"),
            Token::LeftBrace => write!(f, "'{{'"),
            Token::RightBrace => write!(f, "'}}'"),
            Token::LeftBracket => write!(f, "'['"),
            Token::RightBracket => write!(f, "']'"),
            Token::Semicolon => write!(f, "';'"),
            Token::Comma => write!(f, "','"),
            Token::Colon => write!(f, "':'"),
            Token::Dot => write!(f, "'.'"),
            Token::DotDot => write!(f, "'..'"),
            Token::Arrow => write!(f, "'->'"),
            Token::DoubleColon => write!(f, "'::'"),
            Token::Underscore => write!(f, "'_'"),
            Token::FatArrow => write!(f, "'=>'"),
            Token::SingleQuote => write!(f, "'"),
            Token::Ampersand => write!(f, "'&'"),
            Token::Question => write!(f, "'?'"),
            Token::Dollar => write!(f, "'$'"),
            Token::Pipe => write!(f, "'|'"),
            Token::Eof => write!(f, "EOF"),
            Token::Const => write!(f, "'const'"),
            Token::Async => write!(f, "'async'"),
            Token::Await => write!(f, "'await'"),
            Token::SelfParam => write!(f, "'self'"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_lexing() {
        let mut lex = Token::lexer(r#""Hello, World!""#);
        assert_eq!(
            lex.next(),
            Some(Ok(Token::String("Hello, World!".to_string())))
        );
    }

    #[test]
    fn test_escaped_string() {
        let mut lex = Token::lexer(r#""Hello\nWorld\t!""#);
        assert_eq!(
            lex.next(),
            Some(Ok(Token::String("Hello\nWorld\t!".to_string())))
        );
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

    #[test]
    fn test_loop_keywords() {
        let mut lex = Token::lexer("for in while break continue");
        assert_eq!(lex.next(), Some(Ok(Token::For)));
        assert_eq!(lex.next(), Some(Ok(Token::In)));
        assert_eq!(lex.next(), Some(Ok(Token::While)));
        assert_eq!(lex.next(), Some(Ok(Token::Break)));
        assert_eq!(lex.next(), Some(Ok(Token::Continue)));
    }

    #[test]
    fn test_struct_and_dot() {
        let mut lex = Token::lexer("struct Point { x: i32 } p.x");
        assert_eq!(lex.next(), Some(Ok(Token::Struct)));
        assert_eq!(lex.next(), Some(Ok(Token::Identifier("Point".to_string()))));
        assert_eq!(lex.next(), Some(Ok(Token::LeftBrace)));
        assert_eq!(lex.next(), Some(Ok(Token::Identifier("x".to_string()))));
        assert_eq!(lex.next(), Some(Ok(Token::Colon)));
        assert_eq!(lex.next(), Some(Ok(Token::Identifier("i32".to_string()))));
        assert_eq!(lex.next(), Some(Ok(Token::RightBrace)));
        assert_eq!(lex.next(), Some(Ok(Token::Identifier("p".to_string()))));
        assert_eq!(lex.next(), Some(Ok(Token::Dot)));
        assert_eq!(lex.next(), Some(Ok(Token::Identifier("x".to_string()))));
    }

    #[test]
    fn test_enum_keywords() {
        let mut lex = Token::lexer("enum match Color::Red");
        assert_eq!(lex.next(), Some(Ok(Token::Enum)));
        assert_eq!(lex.next(), Some(Ok(Token::Match)));
        assert_eq!(lex.next(), Some(Ok(Token::Identifier("Color".to_string()))));
        assert_eq!(lex.next(), Some(Ok(Token::DoubleColon)));
        assert_eq!(lex.next(), Some(Ok(Token::Identifier("Red".to_string()))));
    }

    #[test]
    fn test_unsafe_keyword() {
        let mut lex = Token::lexer("unsafe { }");
        assert_eq!(lex.next(), Some(Ok(Token::Unsafe)));
        assert_eq!(lex.next(), Some(Ok(Token::LeftBrace)));
        assert_eq!(lex.next(), Some(Ok(Token::RightBrace)));
    }
}
