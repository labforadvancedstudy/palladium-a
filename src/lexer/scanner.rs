// Lexical scanner for Palladium
// "Reading the runes of modern sorcery"

use super::token::Token;
use crate::errors::{CompileError, Result, Span};
use logos::{Lexer as LogosLexer, Logos};

pub struct Lexer<'a> {
    inner: LogosLexer<'a, Token>,
    source: &'a str,
}

pub type LexerError = CompileError;

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            inner: Token::lexer(source),
            source,
        }
    }

    /// Get the next token
    pub fn next_token(&mut self) -> Result<Option<(Token, Span)>> {
        match self.inner.next() {
            Some(Ok(token)) => {
                let span = self.inner.span();
                let start_pos = span.start;
                let end_pos = span.end;

                // Calculate line and column
                let (line, col) = self.position_at(start_pos);

                let span = Span::new(start_pos, end_pos, line, col);
                Ok(Some((token, span)))
            }
            Some(Err(_)) => {
                let span = self.inner.span();
                let start_pos = span.start;
                let (line, col) = self.position_at(start_pos);
                let ch = self.source.chars().nth(start_pos).unwrap_or('?');
                Err(CompileError::UnexpectedChar {
                    ch,
                    line,
                    col,
                    span: Some(crate::errors::Span::new(
                        start_pos,
                        start_pos + 1,
                        line,
                        col,
                    )),
                })
            }
            None => Ok(None),
        }
    }

    /// Calculate line and column for a byte position
    fn position_at(&self, pos: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;

        for (i, ch) in self.source.chars().enumerate() {
            if i >= pos {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }

        (line, col)
    }

    /// Peek at the next token without consuming it
    pub fn peek(&mut self) -> Result<Option<Token>> {
        let saved = self.inner.clone();
        let result = self.next_token().map(|opt| opt.map(|(token, _)| token));
        self.inner = saved;
        result
    }

    /// Consume all remaining tokens
    pub fn collect_tokens(&mut self) -> Result<Vec<(Token, Span)>> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token()? {
            tokens.push(token);
        }
        Ok(tokens)
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token().ok()?.map(|(token, _)| token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let source = "fn main() { }";
        let mut lexer = Lexer::new(source);

        assert_eq!(lexer.next(), Some(Token::Fn));
        assert_eq!(lexer.next(), Some(Token::Identifier("main".to_string())));
        assert_eq!(lexer.next(), Some(Token::LeftParen));
        assert_eq!(lexer.next(), Some(Token::RightParen));
        assert_eq!(lexer.next(), Some(Token::LeftBrace));
        assert_eq!(lexer.next(), Some(Token::RightBrace));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_string_literal() {
        let source = r#"print("Hello, World!");"#;
        let mut lexer = Lexer::new(source);

        assert_eq!(lexer.next(), Some(Token::Identifier("print".to_string())));
        assert_eq!(lexer.next(), Some(Token::LeftParen));
        assert_eq!(
            lexer.next(),
            Some(Token::String("Hello, World!".to_string()))
        );
        assert_eq!(lexer.next(), Some(Token::RightParen));
        assert_eq!(lexer.next(), Some(Token::Semicolon));
    }

    #[test]
    fn test_comments() {
        let source = r#"
        // This is a comment
        fn main() {
            /* Multi-line
               comment */
            print("Hi");
        }
        "#;
        let mut lexer = Lexer::new(source);

        assert_eq!(lexer.next(), Some(Token::Fn));
        assert_eq!(lexer.next(), Some(Token::Identifier("main".to_string())));
        // Comments should be skipped
    }

    #[test]
    fn test_position_tracking() {
        let source = "fn\nmain";
        let mut lexer = Lexer::new(source);

        let (token1, span1) = lexer.next_token().unwrap().unwrap();
        assert_eq!(token1, Token::Fn);
        assert_eq!(span1.line, 1);

        let (token2, span2) = lexer.next_token().unwrap().unwrap();
        assert_eq!(token2, Token::Identifier("main".to_string()));
        assert_eq!(span2.line, 2);
    }
}
