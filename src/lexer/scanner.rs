// Lexical scanner for Palladium
// "Reading the runes of modern sorcery"

use super::token::{escape_spellings, string_escape_spellings, LexError, Token};
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
            Some(Err(e)) => {
                let span = self.inner.span();
                let start_pos = span.start;
                let (line, col) = self.position_at(start_pos);
                let here = Span::new(start_pos, span.end.max(start_pos + 1), line, col);
                Err(match e {
                    LexError::UnexpectedChar => {
                        let ch = self.source[start_pos..].chars().next().unwrap_or('?');
                        CompileError::UnexpectedChar {
                            ch,
                            line,
                            col,
                            // `len_utf8()`, not 1. The span is a BYTE range, and
                            // a one-byte span over a multi-byte character ends
                            // INSIDE a UTF-8 code point — every consumer that
                            // slices the source with it then panics or renders
                            // a broken caret. `'한'` is three bytes and this is
                            // reachable from ordinary source now that char
                            // literals and non-ASCII strings lex.
                            span: Some(Span::new(
                                start_pos,
                                start_pos + ch.len_utf8(),
                                line,
                                col,
                            )),
                        }
                    }
                    LexError::UnterminatedBlockComment => {
                        CompileError::unterminated_block_comment(here)
                    }
                    LexError::UnknownEscape(c) => {
                        CompileError::unknown_escape(c, &escape_spellings(), here)
                    }
                    LexError::NulInStringLiteral => {
                        CompileError::nul_in_string_literal(&string_escape_spellings(), here)
                    }
                })
            }
            None => Ok(None),
        }
    }

    /// Line and column (both 1-based) for a BYTE position.
    ///
    /// `char_indices()`, not `chars().enumerate()`. `pos` is a byte offset —
    /// logos spans are byte ranges — and the old loop compared it against a
    /// CHARACTER ORDINAL. The two agree only while the source is pure ASCII, so
    /// one `한` or `é` or `—` earlier in the file made every subsequent
    /// diagnostic point at the wrong place, by one column per extra byte, and
    /// eventually at the wrong LINE.
    ///
    /// This was unreachable-by-accident before: nothing in the language could
    /// produce a non-ASCII token, so non-ASCII only ever appeared in comments,
    /// which are skipped but still counted here. Char literals and non-ASCII
    /// string content are now ordinary source, which is why it is fixed with
    /// them rather than after them.
    ///
    /// The column is counted in CHARACTERS, which is what a reader counts.
    fn position_at(&self, pos: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;

        for (byte, ch) in self.source.char_indices() {
            if byte >= pos {
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
