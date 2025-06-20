#[cfg(test)]
mod tests {
    use crate::lexer::{Lexer, Token};
    use crate::ownership::borrow_checker::BorrowChecker;
    use crate::parser::Parser;
    use crate::typeck::TypeChecker;

    #[test]
    fn test_unsafe_lexing() {
        let source = "unsafe { }";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();

        // Check that 'unsafe' is lexed correctly
        assert!(matches!(tokens[0].0, Token::Unsafe));
        assert!(matches!(tokens[1].0, Token::LeftBrace));
        assert!(matches!(tokens[2].0, Token::RightBrace));
    }

    #[test]
    fn test_unsafe_parsing() {
        let source = r#"
        fn main() {
            unsafe {
                print("Hello from unsafe");
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        // Verify the AST contains an unsafe block
        assert_eq!(ast.items.len(), 1);
        match &ast.items[0] {
            crate::ast::Item::Function(func) => {
                assert_eq!(func.body.len(), 1);
                assert!(matches!(&func.body[0], crate::ast::Stmt::Unsafe { .. }));
            }
            _ => panic!("Expected function"),
        }
    }

    #[test]
    fn test_unsafe_type_checking() {
        let source = r#"
        fn main() {
            unsafe {
                let x = 42;
                print_int(x);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_nested_unsafe_blocks() {
        let source = r#"
        fn main() {
            unsafe {
                unsafe {
                    print("Nested unsafe");
                }
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_unsafe_with_ownership() {
        let source = r#"
        fn main() {
            let s = "Hello";
            unsafe {
                print(s);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut borrow_checker = BorrowChecker::new();
        assert!(borrow_checker.check_program(&ast).is_ok());
    }
}
