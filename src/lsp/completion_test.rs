// Tests for code completion functionality
// "Testing intelligent suggestions for legendary code"

#[cfg(test)]
mod tests {
    use super::super::{LanguageServer, Position, completion::*};
    use crate::ast::{Type, Program, Item, Function, StructDef, EnumDef, TraitDef, Param, EnumVariant, EnumVariantData, Visibility};
    use crate::errors::Span;

    fn create_test_server() -> LanguageServer {
        let mut server = LanguageServer::new();
        server.initialize(None).unwrap();
        server
    }

    fn create_position(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn test_completion_item_creation() {
        let item = CompletionItem {
            label: "test".to_string(),
            kind: Some(CompletionItemKind::Function),
            detail: Some("fn test()".to_string()),
            documentation: Some("Test function".to_string()),
            insert_text: Some("test()".to_string()),
            insert_text_format: Some(InsertTextFormat::PlainText),
            additional_text_edits: None,
        };

        assert_eq!(item.label, "test");
        assert_eq!(item.kind, Some(CompletionItemKind::Function));
        assert_eq!(item.detail, Some("fn test()".to_string()));
    }

    #[test]
    fn test_get_completions_empty_document() {
        let server = create_test_server();
        let completions = server.get_completions("file:///test.pd", create_position(0, 0));
        assert!(completions.is_empty());
    }

    #[test]
    fn test_get_completions_with_document() {
        let mut server = create_test_server();
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "fn main() { pri".to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(0, 15));
        
        // Should have completions starting with "pri"
        assert!(!completions.is_empty());
        assert!(completions.iter().any(|c| c.label == "print"));
    }

    #[test]
    fn test_get_completion_context() {
        let server = create_test_server();
        
        // Test basic word completion
        let context = server.get_completion_context("let x = pri", create_position(0, 11));
        assert!(context.is_some());
        let ctx = context.unwrap();
        assert_eq!(ctx.word, "pri");
        assert!(!ctx.is_dot_access);
        assert!(!ctx.is_module_access);

        // Test dot access
        let context = server.get_completion_context("value.len", create_position(0, 9));
        assert!(context.is_some());
        let ctx = context.unwrap();
        assert_eq!(ctx.word, "len");
        assert!(ctx.is_dot_access);
        assert!(!ctx.is_module_access);

        // Test module access
        let context = server.get_completion_context("std::io", create_position(0, 7));
        assert!(context.is_some());
        let ctx = context.unwrap();
        assert_eq!(ctx.word, "io");
        assert!(!ctx.is_dot_access);
        assert!(ctx.is_module_access);
    }

    #[test]
    fn test_keyword_completions() {
        let mut server = create_test_server();
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "f".to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(0, 1));
        
        // Should have keyword completions starting with "f"
        let keyword_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Keyword))
            .filter(|c| c.label.starts_with("f"))
            .collect();
        
        assert!(keyword_completions.iter().any(|c| c.label == "fn"));
        assert!(keyword_completions.iter().any(|c| c.label == "for"));
        assert!(keyword_completions.iter().any(|c| c.label == "false"));
    }

    #[test]
    fn test_type_completions() {
        let mut server = create_test_server();
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "let x: i".to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(0, 8));
        
        // Should have type completions starting with "i"
        let type_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Class))
            .filter(|c| c.label.starts_with("i"))
            .collect();
        
        assert!(type_completions.iter().any(|c| c.label == "i32"));
        assert!(type_completions.iter().any(|c| c.label == "i64"));
    }

    #[test]
    fn test_builtin_function_completions() {
        let mut server = create_test_server();
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "print_".to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(0, 6));
        
        // Should have print_int completion
        let func_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Function))
            .filter(|c| c.label.starts_with("print_"))
            .collect();
        
        assert!(func_completions.iter().any(|c| c.label == "print_int"));
        assert!(func_completions.iter().any(|c| c.documentation.is_some()));
    }

    #[test]
    fn test_function_completions_from_ast() {
        let mut server = create_test_server();
        
        // Create a simple AST with a function
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Function(Function {
                    name: "test_func".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    params: vec![
                        Param {
                            name: "x".to_string(),
                            ty: Type::I32,
                            mutable: false,
                        }
                    ],
                    return_type: Some(Type::I32),
                    body: vec![],
                    visibility: Visibility::Private,
                    is_async: false,
                    effects: None,
                    span: Span::new(0, 0, 0, 0),
                }),
            ],
        };

        // Open document and set AST
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "test_".to_string()
        ).unwrap();
        
        server.documents.get_mut("file:///test.pd").unwrap().ast = Some(ast);

        let completions = server.get_completions("file:///test.pd", create_position(0, 5));
        
        // Should have test_func completion
        let func_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Function))
            .filter(|c| c.label == "test_func")
            .collect();
        
        assert_eq!(func_completions.len(), 1);
        assert!(func_completions[0].detail.as_ref().unwrap().contains("fn test_func(x: i32) -> i32"));
    }

    #[test]
    fn test_struct_completions_from_ast() {
        let mut server = create_test_server();
        
        // Create AST with a struct
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Struct(StructDef {
                    name: "Point".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    fields: vec![
                        ("x".to_string(), Type::I32),
                        ("y".to_string(), Type::I32),
                    ],
                    visibility: Visibility::Private,
                    span: Span::new(0, 0, 0, 0),
                }),
            ],
        };

        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "Po".to_string()
        ).unwrap();
        
        server.documents.get_mut("file:///test.pd").unwrap().ast = Some(ast);

        let completions = server.get_completions("file:///test.pd", create_position(0, 2));
        
        let struct_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Struct))
            .filter(|c| c.label == "Point")
            .collect();
        
        assert_eq!(struct_completions.len(), 1);
    }

    #[test]
    fn test_enum_completions_from_ast() {
        let mut server = create_test_server();
        
        // Create AST with an enum
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Enum(EnumDef {
                    name: "Option".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    variants: vec![
                        EnumVariant {
                            name: "Some".to_string(),
                            data: EnumVariantData::Tuple(vec![Type::TypeParam("T".to_string())]),
                        },
                        EnumVariant {
                            name: "None".to_string(),
                            data: EnumVariantData::Unit,
                        },
                    ],
                    span: Span::new(0, 0, 0, 0),
                }),
            ],
        };

        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "Op".to_string()
        ).unwrap();
        
        server.documents.get_mut("file:///test.pd").unwrap().ast = Some(ast);

        let completions = server.get_completions("file:///test.pd", create_position(0, 2));
        
        let enum_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Enum))
            .filter(|c| c.label == "Option")
            .collect();
        
        assert_eq!(enum_completions.len(), 1);
    }

    #[test]
    fn test_dot_completions_string_methods() {
        let mut server = create_test_server();
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "let str = \"hello\"; str.".to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(0, 23));
        
        // Should have string method completions
        let method_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Method))
            .collect();
        
        assert!(!method_completions.is_empty());
        assert!(method_completions.iter().any(|c| c.label == "len"));
        assert!(method_completions.iter().any(|c| c.label == "is_empty"));
        assert!(method_completions.iter().any(|c| c.label == "to_uppercase"));
    }

    #[test]
    fn test_dot_completions_array_methods() {
        let mut server = create_test_server();
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "let array = [1, 2, 3]; array.".to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(0, 29));
        
        // Should have array method completions
        let method_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Method))
            .collect();
        
        assert!(!method_completions.is_empty());
        assert!(method_completions.iter().any(|c| c.label == "len"));
        assert!(method_completions.iter().any(|c| c.label == "push"));
        assert!(method_completions.iter().any(|c| c.label == "pop"));
    }

    #[test]
    fn test_dot_completions_struct_fields() {
        let mut server = create_test_server();
        
        // Create AST with a struct
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Struct(StructDef {
                    name: "Point".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    fields: vec![
                        ("x".to_string(), Type::I32),
                        ("y".to_string(), Type::I32),
                    ],
                    visibility: Visibility::Private,
                    span: Span::new(0, 0, 0, 0),
                }),
            ],
        };

        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "let p = Point { x: 1, y: 2 }; p.".to_string()
        ).unwrap();
        
        server.documents.get_mut("file:///test.pd").unwrap().ast = Some(ast);

        let completions = server.get_completions("file:///test.pd", create_position(0, 32));
        
        // Should have field completions
        let field_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Field))
            .collect();
        
        assert_eq!(field_completions.len(), 2);
        assert!(field_completions.iter().any(|c| c.label == "x"));
        assert!(field_completions.iter().any(|c| c.label == "y"));
    }

    #[test]
    fn test_module_completions_std() {
        let mut server = create_test_server();
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "use std::".to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(0, 9));
        
        // Should have std module completions
        let module_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Module))
            .collect();
        
        assert!(!module_completions.is_empty());
        assert!(module_completions.iter().any(|c| c.label == "io"));
        assert!(module_completions.iter().any(|c| c.label == "collections"));
        assert!(module_completions.iter().any(|c| c.label == "math"));
    }

    #[test]
    fn test_module_completions_result() {
        let mut server = create_test_server();
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "Result::".to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(0, 8));
        
        // Should have Result constructors
        let constructor_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Constructor))
            .collect();
        
        assert_eq!(constructor_completions.len(), 2);
        assert!(constructor_completions.iter().any(|c| c.label == "Ok"));
        assert!(constructor_completions.iter().any(|c| c.label == "Err"));
    }

    #[test]
    fn test_module_completions_option() {
        let mut server = create_test_server();
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "Option::S".to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(0, 9));
        
        // Should have Option::Some
        let constructor_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Constructor))
            .filter(|c| c.label == "Some")
            .collect();
        
        assert_eq!(constructor_completions.len(), 1);
        assert_eq!(constructor_completions[0].insert_text, Some("Some(".to_string()));
    }

    #[test]
    fn test_type_to_string() {
        let server = create_test_server();
        
        assert_eq!(server.type_to_string(&Type::I32), "i32");
        assert_eq!(server.type_to_string(&Type::I64), "i64");
        assert_eq!(server.type_to_string(&Type::Bool), "bool");
        assert_eq!(server.type_to_string(&Type::String), "String");
        assert_eq!(server.type_to_string(&Type::Unit), "()");
        assert_eq!(server.type_to_string(&Type::Custom("MyType".to_string())), "MyType");
        
        // Array type
        assert_eq!(
            server.type_to_string(&Type::Array(Box::new(Type::I32), Some(10))),
            "[i32; 10]"
        );
        
        // Reference types
        assert_eq!(
            server.type_to_string(&Type::Reference {
                mutable: false,
                inner: Box::new(Type::I32),
                lifetime: None,
            }),
            "&i32"
        );
        assert_eq!(
            server.type_to_string(&Type::Reference {
                mutable: true,
                inner: Box::new(Type::String),
                lifetime: None,
            }),
            "&mut String"
        );
        
        // Future type
        assert_eq!(
            server.type_to_string(&Type::Future {
                output: Box::new(Type::I32),
            }),
            "Future<i32>"
        );
        
        // Generic type
        assert_eq!(
            server.type_to_string(&Type::Generic {
                name: "Vec".to_string(),
                args: vec![],
            }),
            "Vec"
        );
        
        // Type parameter
        assert_eq!(
            server.type_to_string(&Type::TypeParam("T".to_string())),
            "T"
        );
        
        // Tuple type
        assert_eq!(
            server.type_to_string(&Type::Tuple(vec![Type::I32, Type::String])),
            "(i32, String)"
        );
    }

    #[test]
    fn test_completion_with_partial_match() {
        let mut server = create_test_server();
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "let x = string_c".to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(0, 16));
        
        // Should find string_concat
        let func_completions: Vec<_> = completions.iter()
            .filter(|c| c.kind == Some(CompletionItemKind::Function))
            .filter(|c| c.label == "string_concat")
            .collect();
        
        assert_eq!(func_completions.len(), 1);
        assert!(func_completions[0].documentation.is_some());
    }

    #[test]
    fn test_insert_text_format() {
        let mut server = create_test_server();
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "pr".to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(0, 2));
        
        // Check print function has proper insert text
        let print_completion = completions.iter()
            .find(|c| c.label == "print")
            .expect("Should have print completion");
        
        assert_eq!(print_completion.insert_text, Some("print(".to_string()));
        assert_eq!(print_completion.insert_text_format, Some(InsertTextFormat::PlainText));
    }

    #[test]
    fn test_completion_empty_context() {
        let server = create_test_server();
        
        // Test with empty line
        let context = server.get_completion_context("", create_position(0, 0));
        assert!(context.is_some());
        let ctx = context.unwrap();
        assert_eq!(ctx.word, "");
        assert!(!ctx.is_dot_access);
        assert!(!ctx.is_module_access);
    }

    #[test]
    fn test_completion_at_line_end() {
        let server = create_test_server();
        
        // Test completion at end of line
        let context = server.get_completion_context("let x = ", create_position(0, 8));
        assert!(context.is_some());
        let ctx = context.unwrap();
        assert_eq!(ctx.word, "");
    }

    #[test]
    fn test_completion_multiline() {
        let mut server = create_test_server();
        let content = "fn main() {\n    pri\n}";
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            content.to_string()
        ).unwrap();

        let completions = server.get_completions("file:///test.pd", create_position(1, 7));
        
        // Should have print completion
        assert!(completions.iter().any(|c| c.label == "print"));
    }
}