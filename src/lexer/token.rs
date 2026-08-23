// Token definitions for Palladium
// "The atoms of legendary code"

use logos::{FilterResult, Logos};

/// Why the lexer stopped, in the lexer's own vocabulary.
///
/// Logos' default error type is `()`, which is why every lexical failure used
/// to surface as `Unexpected character '<first byte of the span>'`. That is the
/// right message for a stray `$` and the wrong one for a string that ends in
/// `\q`: the character is not unexpected, the escape is unknown, and the reader
/// is sent looking at the wrong thing. Each variant here is a distinct question
/// a reader would ask, and `src/lexer/scanner.rs` turns each into a distinct
/// `CompileError` with its own span.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    /// Nothing in the token set starts here. The historic behaviour, kept as
    /// the `Default` because logos constructs the error type itself when a
    /// callback returns `None`.
    #[default]
    UnexpectedChar,
    /// A `/*` that no `*/` closes at depth 0. Comments nest (N2-08), so the
    /// scanner counts depth and a missing close is reported at the `/*` that
    /// opened the outermost unbalanced comment rather than at end of file.
    UnterminatedBlockComment,
    /// `\` followed by something outside the escape set. Carries the offending
    /// character so the diagnostic can name it.
    UnknownEscape(char),
    /// `''` — a char literal with nothing in it.
    EmptyCharLiteral,
}

/// The escape sequences a string or char literal may contain.
///
/// `grammar.ebnf` records five (`\n \t \r \" \\`); `\0` and `\'` are the two
/// this table adds, and both are forced rather than chosen. `\0` because
/// `tests/01_lexical_literals.pd` has written `"null\0terminator"` since before
/// this repository had a gate, and the old lexer passed it through as the two
/// characters `\` and `0` — a literal that says NUL and a binary that contains
/// a backslash. `\'` because `char_literal = "'" ( char | escape ) "'"` has no
/// other way to spell a quote.
///
/// Anything not in this table is `LexError::UnknownEscape`, NOT a pass-through.
/// Pass-through is how `"\q"` became the two characters `\q` with no diagnostic,
/// which is the same defect as an attribute that lexes and is then ignored
/// (N2-11), one layer down: the source says one thing and the bytes say another.
const ESCAPES: &[(char, char)] = &[
    ('n', '\n'),
    ('t', '\t'),
    ('r', '\r'),
    ('"', '"'),
    ('\\', '\\'),
    ('0', '\0'),
    ('\'', '\''),
];

/// Resolve one escape character (the one *after* the backslash).
pub fn escape_char(c: char) -> Option<char> {
    ESCAPES.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}

/// The escape set, spelled as source, for a diagnostic to list.
pub fn escape_spellings() -> Vec<String> {
    ESCAPES.iter().map(|(k, _)| format!("\\{}", k)).collect()
}

/// Unescape the body of a string literal.
fn unescape(body: &str) -> Result<String, LexError> {
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        // The regex that produced this slice guarantees a character follows a
        // backslash, so `None` here would be a logos bug rather than a program
        // error; treat it as an unknown escape of the backslash itself instead
        // of panicking.
        let e = chars.next().unwrap_or('\\');
        match escape_char(e) {
            Some(v) => out.push(v),
            None => return Err(LexError::UnknownEscape(e)),
        }
    }
    Ok(out)
}

/// Consume a comment, or emit the division operator.
///
/// `/` opens three different things and only one of them is a token. Handling
/// all three here rather than as two `#[logos(skip)]` regexes is what makes
/// N2-08 possible at all: **a regular expression cannot count**, so the old
/// `/\*[^*]*\*+(?:[^/*][^*]*\*+)*/` stopped at the FIRST `*/` and
/// `/* a /* b */ c */` left ` c */` as live source — reported to the user as
/// `Expected expression, but found '/'`, five characters away from the cause.
/// Depth counting needs a loop, so it needs a callback.
///
/// Unterminated is an error rather than "comment to end of file": a file whose
/// last `*/` was deleted would otherwise compile as a shorter program.
fn slash_or_comment(lex: &mut logos::Lexer<Token>) -> FilterResult<(), LexError> {
    let rest = lex.remainder();
    let bytes = rest.as_bytes();
    match bytes.first() {
        Some(b'/') => {
            // Line comment: to end of line, newline left for the whitespace rule.
            let n = rest.find('\n').unwrap_or(rest.len());
            lex.bump(n);
            FilterResult::Skip
        }
        Some(b'*') => {
            let mut depth = 1usize;
            let mut i = 1usize; // past the `*` of the opening `/*`
            while i < bytes.len() {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        lex.bump(i);
                        return FilterResult::Skip;
                    }
                } else {
                    i += 1;
                }
            }
            // Leave the span at the opening `/*` so the caret points at the
            // comment that was never closed, not at end of file.
            lex.bump(1);
            FilterResult::Error(LexError::UnterminatedBlockComment)
        }
        _ => FilterResult::Emit(()),
    }
}

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(error = LexError)]
#[logos(skip r"[ \t\n\f]+")]
pub enum Token {
    // Literals
    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        unescape(&s[1..s.len()-1])
    })]
    String(String),

    /// A char literal's VALUE is its Unicode scalar.
    ///
    /// Kept as a `char` rather than folded to its code point here so that the
    /// spelling survives into the AST: `'a'` and `97` are the same value and
    /// not the same program text, and a tool that reads the AST (the LSP, a
    /// future formatter) has no way back from the integer.
    ///
    /// The regex needs both quotes, which is what keeps `Token::SingleQuote`
    /// working for lifetimes: in `<'a>` the tick has no partner two characters
    /// on, no longer match exists, and logos falls back to the one-character
    /// token. `'a'` is three bytes and wins on length wherever both could match.
    #[regex(r"'([^'\\]|\\.)'", |lex| {
        let s = lex.slice();
        let body = &s[1..s.len()-1];
        let mut chars = body.chars();
        let c = chars.next().ok_or(LexError::EmptyCharLiteral)?;
        if c != '\\' {
            return Ok::<char, LexError>(c);
        }
        let e = chars.next().unwrap_or('\\');
        escape_char(e).ok_or(LexError::UnknownEscape(e))
    })]
    Char(char),

    /// A float literal. The sign is part of the token, exactly as it is for
    /// `Integer` — so `x-1.5` lexes as `x` then `-1.5` and must be written
    /// `x - 1.5`. Splitting the two conventions would give the language two
    /// different answers to the same question.
    ///
    /// Declared BEFORE `Integer` and matching `[0-9]+\.[0-9]+`, so `1..5` still
    /// lexes as `1 .. 5`: the float regex needs a digit after the dot and the
    /// second `.` is not one.
    #[regex(r"-?[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

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

    #[token("/", slash_or_comment)]
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

    /// `#`, which exists only to open an attribute (N2-10).
    ///
    /// `#!` is its own token rather than `Hash` followed by `Not`, because the
    /// two are only an inner attribute when they are adjacent: `#! [total]`
    /// with a space is not one, and a parser reading two separate tokens cannot
    /// tell the difference without re-reading the source.
    #[token("#")]
    Hash,

    #[token("#!")]
    HashBang,

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
                | Token::Float(_)
                | Token::Char(_)
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
            Token::Float(x) => write!(f, "float {}", x),
            Token::Char(c) => write!(f, "char '{}'", c),
            Token::Hash => write!(f, "'#'"),
            Token::HashBang => write!(f, "'#!'"),
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
