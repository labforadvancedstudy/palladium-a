// Lexer module for Palladium
// "Breaking down legends into their essence"

pub mod token;
pub mod scanner;

pub use token::Token;
pub use scanner::{Lexer, LexerError};

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