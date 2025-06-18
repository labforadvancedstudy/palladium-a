// Lexer module for Palladium
// "Breaking down legends into their essence"

pub mod scanner;
pub mod token;

pub use scanner::{Lexer, LexerError};
pub use token::Token;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_world() {
        let source = r#"
        fn main() {
            print("Hello, World!");
        }
        "#;

        let lexer = Lexer::new(source);
        let tokens: Vec<Token> = lexer.collect();

        assert!(!tokens.is_empty());
        assert_eq!(tokens[0], Token::Fn);
        assert_eq!(tokens[1], Token::Identifier("main".to_string()));
    }
}
