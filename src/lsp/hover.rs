// Hover information provider
// "Type information at your fingertips"

use super::{LanguageServer, Position, Range};
use serde::{Deserialize, Serialize};

/// Hover response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hover {
    /// The hover contents
    pub contents: MarkupContent,
    /// An optional range
    pub range: Option<Range>,
}

/// Markup content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkupContent {
    /// The type of the markup
    pub kind: MarkupKind,
    /// The content
    pub value: String,
}

/// Markup kind
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkupKind {
    #[serde(rename = "plaintext")]
    PlainText,
    #[serde(rename = "markdown")]
    Markdown,
}

impl LanguageServer {
    /// Get hover information at a position
    pub fn get_hover(&self, uri: &str, position: Position) -> Option<Hover> {
        let doc = self.documents.get(uri)?;
        let _ast = doc.ast.as_ref()?;

        // Find what's at the position
        let symbol = self.find_symbol_at_position(&doc.content, position)?;

        // Look up type information
        if let Some(type_info) = &doc.type_info {
            // Check if it's a variable
            if let Some(var_type) = type_info.variables.get(&symbol) {
                return Some(Hover {
                    contents: MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!(
                            "```palladium\nlet {}: {}\n```",
                            symbol,
                            self.type_to_string(var_type)
                        ),
                    },
                    range: None,
                });
            }

            // Check if it's a function
            if let Some(func_sig) = type_info.functions.get(&symbol) {
                let params: Vec<String> = func_sig
                    .params
                    .iter()
                    .map(|(name, ty)| format!("{}: {}", name, self.type_to_string(ty)))
                    .collect();

                let signature = if let Some(ret) = &func_sig.return_type {
                    format!(
                        "fn {}({}) -> {}",
                        symbol,
                        params.join(", "),
                        self.type_to_string(ret)
                    )
                } else {
                    format!("fn {}({})", symbol, params.join(", "))
                };

                let mut hover_text = format!("```palladium\n{}\n```", signature);

                if func_sig.is_async {
                    hover_text.push_str("\n\n*Async function*");
                }

                if !func_sig.effects.is_empty() {
                    hover_text
                        .push_str(&format!("\n\n**Effects**: {}", func_sig.effects.join(", ")));
                }

                return Some(Hover {
                    contents: MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: hover_text,
                    },
                    range: None,
                });
            }

            // Check if it's a type alias
            if let Some(alias_type) = type_info.type_aliases.get(&symbol) {
                return Some(Hover {
                    contents: MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!(
                            "```palladium\ntype {} = {}\n```",
                            symbol,
                            self.type_to_string(alias_type)
                        ),
                    },
                    range: None,
                });
            }
        }

        // Check built-in functions
        let builtins = vec![
            ("print", "fn print(s: String)", "Print a string to stdout"),
            (
                "print_int",
                "fn print_int(n: i64)",
                "Print an integer to stdout",
            ),
            (
                "string_len",
                "fn string_len(s: String) -> i64",
                "Get the length of a string",
            ),
            (
                "string_concat",
                "fn string_concat(a: String, b: String) -> String",
                "Concatenate two strings",
            ),
            (
                "int_to_string",
                "fn int_to_string(n: i64) -> String",
                "Convert an integer to a string",
            ),
            (
                "string_to_int",
                "fn string_to_int(s: String) -> Option<i64>",
                "Parse an integer from a string",
            ),
        ];

        for (name, sig, doc) in builtins {
            if name == symbol {
                return Some(Hover {
                    contents: MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("```palladium\n{}\n```\n\n{}", sig, doc),
                    },
                    range: None,
                });
            }
        }

        // Check built-in types
        let types = vec![
            ("i32", "32-bit signed integer"),
            ("i64", "64-bit signed integer"),
            ("u32", "32-bit unsigned integer"),
            ("u64", "64-bit unsigned integer"),
            ("bool", "Boolean type"),
            ("String", "UTF-8 string type"),
            ("Vec", "Dynamic array type"),
            ("HashMap", "Hash map type"),
            ("Option", "Optional type"),
            ("Result", "Result type"),
        ];

        for (type_name, description) in types {
            if type_name == symbol {
                return Some(Hover {
                    contents: MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("```palladium\ntype {}\n```\n\n{}", type_name, description),
                    },
                    range: None,
                });
            }
        }

        None
    }
    
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Type, Program, ArraySize};
    
    use crate::lsp::{TypeInfo, FunctionSignature};
    use std::collections::HashMap;

    fn create_test_server() -> LanguageServer {
        let mut server = LanguageServer::new();
        server.initialize(None).unwrap();
        server
    }

    fn create_position(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn test_hover_creation() {
        let hover = Hover {
            contents: MarkupContent {
                kind: MarkupKind::Markdown,
                value: "Test content".to_string(),
            },
            range: None,
        };

        assert_eq!(hover.contents.value, "Test content");
        assert_eq!(hover.contents.kind as u32, MarkupKind::Markdown as u32);
        assert!(hover.range.is_none());
    }

    #[test]
    fn test_markup_content_creation() {
        let content = MarkupContent {
            kind: MarkupKind::PlainText,
            value: "Plain text".to_string(),
        };

        assert_eq!(content.value, "Plain text");
        assert_eq!(content.kind as u32, MarkupKind::PlainText as u32);
    }

    #[test]
    fn test_hover_empty_document() {
        let server = create_test_server();
        let hover = server.get_hover("file:///test.pd", create_position(0, 0));
        assert!(hover.is_none());
    }

    #[test]
    fn test_hover_variable() {
        let mut server = create_test_server();
        
        // Open document with variable
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "let x = 42;".to_string()
        ).unwrap();

        // Create AST
        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        
        // Create type info
        let mut type_info = TypeInfo {
            variables: HashMap::new(),
            functions: HashMap::new(),
            type_aliases: HashMap::new(),
        };
        type_info.variables.insert("x".to_string(), Type::I32);

        // Set AST and type info
        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);
        doc.type_info = Some(type_info);

        // Get hover on 'x'
        let hover = server.get_hover("file:///test.pd", create_position(0, 4));
        assert!(hover.is_some());

        let hover = hover.unwrap();
        assert_eq!(hover.contents.kind as u32, MarkupKind::Markdown as u32);
        assert!(hover.contents.value.contains("let x: i32"));
        assert!(hover.contents.value.contains("```palladium"));
    }

    #[test]
    fn test_hover_function() {
        let mut server = create_test_server();
        
        // Open document
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "fn add(a: i32, b: i32) -> i32 { a + b }".to_string()
        ).unwrap();

        // Create AST
        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        
        // Create type info with function
        let mut type_info = TypeInfo {
            variables: HashMap::new(),
            functions: HashMap::new(),
            type_aliases: HashMap::new(),
        };
        
        let func_sig = FunctionSignature {
            params: vec![
                ("a".to_string(), Type::I32),
                ("b".to_string(), Type::I32),
            ],
            return_type: Some(Type::I32),
            is_async: false,
            effects: vec![],
        };
        type_info.functions.insert("add".to_string(), func_sig);

        // Set AST and type info
        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);
        doc.type_info = Some(type_info);

        // Get hover on 'add'
        let hover = server.get_hover("file:///test.pd", create_position(0, 3));
        assert!(hover.is_some());

        let hover = hover.unwrap();
        assert!(hover.contents.value.contains("fn add(a: i32, b: i32) -> i32"));
    }

    #[test]
    fn test_hover_async_function() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "async fn fetch() -> String { }".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        
        let mut type_info = TypeInfo {
            variables: HashMap::new(),
            functions: HashMap::new(),
            type_aliases: HashMap::new(),
        };
        
        let func_sig = FunctionSignature {
            params: vec![],
            return_type: Some(Type::String),
            is_async: true,
            effects: vec![],
        };
        type_info.functions.insert("fetch".to_string(), func_sig);

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);
        doc.type_info = Some(type_info);

        let hover = server.get_hover("file:///test.pd", create_position(0, 10));
        assert!(hover.is_some());

        let hover = hover.unwrap();
        assert!(hover.contents.value.contains("fn fetch() -> String"));
        assert!(hover.contents.value.contains("*Async function*"));
    }

    #[test]
    fn test_hover_function_with_effects() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "fn write_file() effects [io] { }".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        
        let mut type_info = TypeInfo {
            variables: HashMap::new(),
            functions: HashMap::new(),
            type_aliases: HashMap::new(),
        };
        
        let func_sig = FunctionSignature {
            params: vec![],
            return_type: None,
            is_async: false,
            effects: vec!["io".to_string()],
        };
        type_info.functions.insert("write_file".to_string(), func_sig);

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);
        doc.type_info = Some(type_info);

        let hover = server.get_hover("file:///test.pd", create_position(0, 3));
        assert!(hover.is_some());

        let hover = hover.unwrap();
        assert!(hover.contents.value.contains("fn write_file()"));
        assert!(hover.contents.value.contains("**Effects**: io"));
    }

    #[test]
    fn test_hover_type_alias() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "type Size = i64;".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        
        let mut type_info = TypeInfo {
            variables: HashMap::new(),
            functions: HashMap::new(),
            type_aliases: HashMap::new(),
        };
        type_info.type_aliases.insert("Size".to_string(), Type::I64);

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);
        doc.type_info = Some(type_info);

        let hover = server.get_hover("file:///test.pd", create_position(0, 5));
        assert!(hover.is_some());

        let hover = hover.unwrap();
        assert!(hover.contents.value.contains("type Size = i64"));
    }

    #[test]
    fn test_hover_builtin_function() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "print(\"hello\");".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);

        let hover = server.get_hover("file:///test.pd", create_position(0, 0));
        assert!(hover.is_some());

        let hover = hover.unwrap();
        assert!(hover.contents.value.contains("fn print(s: String)"));
        assert!(hover.contents.value.contains("Print a string to stdout"));
    }

    #[test]
    fn test_hover_builtin_type() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "let x: i32 = 42;".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);

        let hover = server.get_hover("file:///test.pd", create_position(0, 7));
        assert!(hover.is_some());

        let hover = hover.unwrap();
        assert!(hover.contents.value.contains("type i32"));
        assert!(hover.contents.value.contains("32-bit signed integer"));
    }

    #[test]
    fn test_hover_all_builtin_functions() {
        let mut server = create_test_server();
        
        // Test each builtin function
        let builtins = vec![
                ("print_int", "fn print_int(n: i64)"),
                ("string_len", "fn string_len(s: String) -> i64"),
                ("string_concat", "fn string_concat(a: String, b: String) -> String"),
                ("int_to_string", "fn int_to_string(n: i64) -> String"),
            ("string_to_int", "fn string_to_int(s: String) -> Option<i64>"),
        ];

        for (name, expected_sig) in builtins {
            server.open_document(
                "file:///test.pd".to_string(),
                1,
                format!("{}();", name)
            ).unwrap();

            let ast = Program {
            imports: vec![],
            items: vec![],
        };
            let doc = server.documents.get_mut("file:///test.pd").unwrap();
            doc.ast = Some(ast);

            let hover = server.get_hover("file:///test.pd", create_position(0, 0));
            assert!(hover.is_some(), "Should have hover for {}", name);

            let hover = hover.unwrap();
            assert!(hover.contents.value.contains(expected_sig), 
                "Hover for {} should contain signature", name);
        }
    }

    #[test]
    fn test_hover_all_builtin_types() {
        let mut server = create_test_server();
        
        // Test each builtin type
        let types = vec![
            ("i64", "64-bit signed integer"),
            ("u32", "32-bit unsigned integer"),
                ("u64", "64-bit unsigned integer"),
                ("bool", "Boolean type"),
                ("String", "UTF-8 string type"),
                ("Vec", "Dynamic array type"),
            ("HashMap", "Hash map type"),
            ("Option", "Optional type"),
            ("Result", "Result type"),
        ];

        for (type_name, expected_desc) in types {
            server.open_document(
                "file:///test.pd".to_string(),
                1,
                format!("let x: {};", type_name)
            ).unwrap();

            let ast = Program {
            imports: vec![],
            items: vec![],
        };
            let doc = server.documents.get_mut("file:///test.pd").unwrap();
            doc.ast = Some(ast);

            let hover = server.get_hover("file:///test.pd", create_position(0, 7));
            assert!(hover.is_some(), "Should have hover for {}", type_name);

            let hover = hover.unwrap();
            assert!(hover.contents.value.contains(expected_desc), 
                "Hover for {} should contain description", type_name);
        }
    }

    #[test]
    fn test_hover_complex_types() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "let arr: [i32; 10];".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        
        let mut type_info = TypeInfo {
            variables: HashMap::new(),
            functions: HashMap::new(),
            type_aliases: HashMap::new(),
        };
        type_info.variables.insert("arr".to_string(), Type::Array(Box::new(Type::I32), ArraySize::Literal(10)));

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);
        doc.type_info = Some(type_info);

        let hover = server.get_hover("file:///test.pd", create_position(0, 4));
        assert!(hover.is_some());

        let hover = hover.unwrap();
        assert!(hover.contents.value.contains("let arr: [i32; 10]"));
    }

    #[test]
    fn test_hover_reference_types() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "let r: &mut String;".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        
        let mut type_info = TypeInfo {
            variables: HashMap::new(),
            functions: HashMap::new(),
            type_aliases: HashMap::new(),
        };
        type_info.variables.insert("r".to_string(), Type::Reference {
            mutable: true,
            inner: Box::new(Type::String),
            lifetime: None,
        });

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);
        doc.type_info = Some(type_info);

        let hover = server.get_hover("file:///test.pd", create_position(0, 4));
        assert!(hover.is_some());

        let hover = hover.unwrap();
        assert!(hover.contents.value.contains("let r: &mut String"));
    }

    #[test]
    fn test_hover_no_symbol() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "   ".to_string() // Just spaces
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);

        let hover = server.get_hover("file:///test.pd", create_position(0, 1));
        assert!(hover.is_none());
    }

    #[test]
    fn test_hover_unknown_symbol() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "unknown_symbol".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);
        doc.type_info = Some(TypeInfo {
            variables: HashMap::new(),
            functions: HashMap::new(),
            type_aliases: HashMap::new(),
        });

        let hover = server.get_hover("file:///test.pd", create_position(0, 5));
        assert!(hover.is_none());
    }

    #[test]
    fn test_hover_with_range() {
        let hover = Hover {
            contents: MarkupContent {
                kind: MarkupKind::Markdown,
                value: "Test".to_string(),
            },
            range: Some(Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 4 },
            }),
        };

        assert!(hover.range.is_some());
        let range = hover.range.unwrap();
        assert_eq!(range.start.line, 0);
        assert_eq!(range.end.character, 4);
    }
}
