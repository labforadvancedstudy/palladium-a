// Symbol information provider
// "Navigate your code with legendary precision"

use super::{LanguageServer, Location, Range, SymbolKind};
use serde::{Deserialize, Serialize};

/// Document symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbol {
    /// The name of this symbol
    pub name: String,
    /// More detail for this symbol
    pub detail: Option<String>,
    /// The kind of this symbol
    pub kind: SymbolKind,
    /// The range enclosing this symbol
    pub range: Range,
    /// The range that should be selected and revealed
    pub selection_range: Range,
    /// Children of this symbol
    pub children: Vec<DocumentSymbol>,
}

/// Symbol information for workspace symbol search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInformation {
    /// The name of this symbol
    pub name: String,
    /// The kind of this symbol
    pub kind: SymbolKind,
    /// The location of this symbol
    pub location: Location,
    /// The name of the symbol containing this symbol
    pub container_name: Option<String>,
}

impl LanguageServer {
    /// Get document symbols
    pub fn get_document_symbols(&self, uri: &str) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();

        let doc = match self.documents.get(uri) {
            Some(doc) => doc,
            None => return symbols,
        };

        let ast = match &doc.ast {
            Some(ast) => ast,
            None => return symbols,
        };

        for item in &ast.items {
            match item {
                crate::ast::Item::Function(func) => {
                    let children = self.get_function_symbols(func);

                    symbols.push(DocumentSymbol {
                        name: func.name.clone(),
                        detail: Some(self.function_signature(func)),
                        kind: SymbolKind::Function,
                        range: self.span_to_range(func.span),
                        selection_range: self.span_to_range(func.span),
                        children,
                    });
                }
                crate::ast::Item::Struct(struct_def) => {
                    let mut children = Vec::new();

                    for (field_name, field_ty) in &struct_def.fields {
                        children.push(DocumentSymbol {
                            name: field_name.clone(),
                            detail: Some(self.type_to_string(field_ty)),
                            kind: SymbolKind::Field,
                            range: self.span_to_range(struct_def.span),
                            selection_range: self.span_to_range(struct_def.span),
                            children: Vec::new(),
                        });
                    }

                    symbols.push(DocumentSymbol {
                        name: struct_def.name.clone(),
                        detail: Some(format!("struct {}", struct_def.name)),
                        kind: SymbolKind::Struct,
                        range: self.span_to_range(struct_def.span),
                        selection_range: self.span_to_range(struct_def.span),
                        children,
                    });
                }
                crate::ast::Item::Enum(enum_def) => {
                    let mut children = Vec::new();

                    for variant in &enum_def.variants {
                        let detail = match &variant.data {
                            crate::ast::EnumVariantData::Unit => None,
                            crate::ast::EnumVariantData::Tuple(types) => {
                                let type_strs: Vec<String> =
                                    types.iter().map(|t| self.type_to_string(t)).collect();
                                Some(format!("({})", type_strs.join(", ")))
                            }
                            crate::ast::EnumVariantData::Struct(fields) => {
                                let field_strs: Vec<String> = fields
                                    .iter()
                                    .map(|(name, ty)| {
                                        format!("{}: {}", name, self.type_to_string(ty))
                                    })
                                    .collect();
                                Some(format!("{{ {} }}", field_strs.join(", ")))
                            }
                        };

                        children.push(DocumentSymbol {
                            name: variant.name.clone(),
                            detail,
                            kind: SymbolKind::EnumVariant,
                            range: self.span_to_range(enum_def.span),
                            selection_range: self.span_to_range(enum_def.span),
                            children: Vec::new(),
                        });
                    }

                    symbols.push(DocumentSymbol {
                        name: enum_def.name.clone(),
                        detail: Some(format!("enum {}", enum_def.name)),
                        kind: SymbolKind::Enum,
                        range: self.span_to_range(enum_def.span),
                        selection_range: self.span_to_range(enum_def.span),
                        children,
                    });
                }
                crate::ast::Item::Trait(trait_def) => {
                    let mut children = Vec::new();

                    for method in &trait_def.methods {
                        children.push(DocumentSymbol {
                            name: method.name.clone(),
                            detail: Some(self.method_signature(method)),
                            kind: SymbolKind::Method,
                            range: self.span_to_range(method.span),
                            selection_range: self.span_to_range(method.span),
                            children: Vec::new(),
                        });
                    }

                    symbols.push(DocumentSymbol {
                        name: trait_def.name.clone(),
                        detail: Some(format!("trait {}", trait_def.name)),
                        kind: SymbolKind::Trait,
                        range: self.span_to_range(trait_def.span),
                        selection_range: self.span_to_range(trait_def.span),
                        children,
                    });
                }
                crate::ast::Item::TypeAlias(type_alias) => {
                    symbols.push(DocumentSymbol {
                        name: type_alias.name.clone(),
                        detail: Some(format!(
                            "type {} = {}",
                            type_alias.name,
                            self.type_to_string(&type_alias.ty)
                        )),
                        kind: SymbolKind::TypeAlias,
                        range: self.span_to_range(type_alias.span),
                        selection_range: self.span_to_range(type_alias.span),
                        children: Vec::new(),
                    });
                }
                _ => {}
            }
        }

        symbols
    }

    /// Get workspace symbols matching query
    pub fn get_workspace_symbols(&self, query: &str) -> Vec<SymbolInformation> {
        let mut symbols = Vec::new();
        let query_lower = query.to_lowercase();

        for symbol_list in self.symbol_index.symbols.values() {
            for symbol in symbol_list {
                if symbol.name.to_lowercase().contains(&query_lower) {
                    symbols.push(SymbolInformation {
                        name: symbol.name.clone(),
                        kind: symbol.kind,
                        location: symbol.location.clone(),
                        container_name: symbol.container_name.clone(),
                    });
                }
            }
        }

        symbols
    }

    /// Get symbols from a function
    fn get_function_symbols(&self, func: &crate::ast::Function) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();

        // Add parameters as symbols
        for param in &func.params {
            symbols.push(DocumentSymbol {
                name: param.name.clone(),
                detail: Some(self.type_to_string(&param.ty)),
                kind: SymbolKind::Variable,
                range: self.span_to_range(func.span),
                selection_range: self.span_to_range(func.span),
                children: Vec::new(),
            });
        }

        // TODO: Parse function body for local variables

        symbols
    }

    /// Get function signature
    fn function_signature(&self, func: &crate::ast::Function) -> String {
        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, self.type_to_string(&p.ty)))
            .collect();

        if let Some(ret) = &func.return_type {
            format!(
                "fn {}({}) -> {}",
                func.name,
                params.join(", "),
                self.type_to_string(ret)
            )
        } else {
            format!("fn {}({})", func.name, params.join(", "))
        }
    }

    /// Get method signature
    fn method_signature(&self, method: &crate::ast::TraitMethod) -> String {
        let params: Vec<String> = method
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, self.type_to_string(&p.ty)))
            .collect();

        if let Some(ret) = &method.return_type {
            format!(
                "fn {}({}) -> {}",
                method.name,
                params.join(", "),
                self.type_to_string(ret)
            )
        } else {
            format!("fn {}({})", method.name, params.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Program, Item, Function, StructDef, EnumDef, TraitDef, TypeAlias, Type, Param, 
                      EnumVariant, EnumVariantData, TraitMethod, Visibility};
    use crate::errors::Span;
    use crate::lsp::{Position, Location, Symbol};
    

    fn create_test_server() -> LanguageServer {
        let mut server = LanguageServer::new();
        server.initialize(None).unwrap();
        server
    }

    #[test]
    fn test_document_symbol_creation() {
        let symbol = DocumentSymbol {
            name: "test_function".to_string(),
            detail: Some("fn test_function()".to_string()),
            kind: SymbolKind::Function,
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 5, character: 1 },
            },
            selection_range: Range {
                start: Position { line: 0, character: 3 },
                end: Position { line: 0, character: 16 },
            },
            children: vec![],
        };

        assert_eq!(symbol.name, "test_function");
        assert_eq!(symbol.detail, Some("fn test_function()".to_string()));
        assert_eq!(symbol.kind, SymbolKind::Function);
        assert!(symbol.children.is_empty());
    }

    #[test]
    fn test_symbol_information_creation() {
        let symbol = SymbolInformation {
            name: "MyStruct".to_string(),
            kind: SymbolKind::Struct,
            location: Location {
                uri: "file:///test.pd".to_string(),
                range: Range {
                    start: Position { line: 10, character: 0 },
                    end: Position { line: 15, character: 1 },
                },
            },
            container_name: Some("module".to_string()),
        };

        assert_eq!(symbol.name, "MyStruct");
        assert_eq!(symbol.kind, SymbolKind::Struct);
        assert_eq!(symbol.location.uri, "file:///test.pd");
        assert_eq!(symbol.container_name, Some("module".to_string()));
    }

    #[test]
    fn test_get_document_symbols_empty() {
        let server = create_test_server();
        let symbols = server.get_document_symbols("file:///test.pd");
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_get_document_symbols_no_ast() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "fn main() {}".to_string()
        ).unwrap();

        // Manually clear AST to test behavior without AST
        if let Some(doc) = server.documents.get_mut("file:///test.pd") {
            doc.ast = None;
        }
        
        let symbols = server.get_document_symbols("file:///test.pd");
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_get_document_symbols_function() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "fn add(a: i32, b: i32) -> i32 { a + b }".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Function(Function {
                    name: "add".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    params: vec![
                        Param {
                            name: "a".to_string(),
                            ty: Type::I32,
                            mutable: false,
                        },
                        Param {
                            name: "b".to_string(),
                            ty: Type::I32,
                            mutable: false,
                        },
                    ],
                    return_type: Some(Type::I32),
                    body: vec![],
                    visibility: Visibility::Private,
                    is_async: false,
                    effects: None,
                    span: Span::new(0, 40, 0, 0),
                }),
            ],
        };

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);

        let symbols = server.get_document_symbols("file:///test.pd");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "add");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert!(symbols[0].detail.as_ref().unwrap().contains("fn add(a: i32, b: i32) -> i32"));
        
        // Function should have parameter children
        assert_eq!(symbols[0].children.len(), 2);
        assert_eq!(symbols[0].children[0].name, "a");
        assert_eq!(symbols[0].children[0].kind, SymbolKind::Variable);
        assert_eq!(symbols[0].children[1].name, "b");
        assert_eq!(symbols[0].children[1].kind, SymbolKind::Variable);
    }

    #[test]
    fn test_get_document_symbols_struct() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "struct Point { x: i32, y: i32 }".to_string()
        ).unwrap();

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
                    span: Span::new(0, 31, 0, 0),
                }),
            ],
        };

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);

        let symbols = server.get_document_symbols("file:///test.pd");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Point");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
        assert_eq!(symbols[0].detail, Some("struct Point".to_string()));
        
        // Struct should have field children
        assert_eq!(symbols[0].children.len(), 2);
        assert_eq!(symbols[0].children[0].name, "x");
        assert_eq!(symbols[0].children[0].kind, SymbolKind::Field);
        assert_eq!(symbols[0].children[0].detail, Some("i32".to_string()));
        assert_eq!(symbols[0].children[1].name, "y");
        assert_eq!(symbols[0].children[1].kind, SymbolKind::Field);
    }

    #[test]
    fn test_get_document_symbols_enum() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "enum Option { Some(T), None }".to_string()
        ).unwrap();

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
                    span: Span::new(0, 29, 0, 0),
                }),
            ],
        };

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);

        let symbols = server.get_document_symbols("file:///test.pd");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Option");
        assert_eq!(symbols[0].kind, SymbolKind::Enum);
        
        // Enum should have variant children
        assert_eq!(symbols[0].children.len(), 2);
        assert_eq!(symbols[0].children[0].name, "Some");
        assert_eq!(symbols[0].children[0].kind, SymbolKind::EnumVariant);
        assert_eq!(symbols[0].children[0].detail, Some("(T)".to_string()));
        assert_eq!(symbols[0].children[1].name, "None");
        assert_eq!(symbols[0].children[1].detail, None);
    }

    #[test]
    fn test_get_document_symbols_enum_struct_variant() {
        let mut server = create_test_server();
        
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Enum(EnumDef {
                    name: "Message".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    variants: vec![
                        EnumVariant {
                            name: "Move".to_string(),
                            data: EnumVariantData::Struct(vec![
                                ("x".to_string(), Type::I32),
                                ("y".to_string(), Type::I32),
                            ]),
                        },
                    ],
                    span: Span::new(0, 30, 0, 0),
                }),
            ],
        };

        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "enum Message { Move { x: i32, y: i32 } }".to_string()
        ).unwrap();

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);

        let symbols = server.get_document_symbols("file:///test.pd");
        assert_eq!(symbols[0].children[0].name, "Move");
        assert_eq!(symbols[0].children[0].detail, Some("{ x: i32, y: i32 }".to_string()));
    }

    #[test]
    fn test_get_document_symbols_trait() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "trait Display { fn fmt(&self) -> String; }".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Trait(TraitDef {
                    name: "Display".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    methods: vec![
                        TraitMethod {
                            name: "fmt".to_string(),
                            lifetime_params: vec![],
                            type_params: vec![],
                            params: vec![
                                Param {
                                    name: "self".to_string(),
                                    ty: Type::Custom("Self".to_string()),
                                    mutable: false,
                                },
                            ],
                            return_type: Some(Type::String),
                            has_body: false,
                            body: None,                            span: Span::new(16, 40, 0, 0),
                        },
                    ],
                    visibility: Visibility::Private,
                    span: Span::new(0, 42, 0, 0),
                }),
            ],
        };

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);

        let symbols = server.get_document_symbols("file:///test.pd");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Display");
        assert_eq!(symbols[0].kind, SymbolKind::Trait);
        
        // Trait should have method children
        assert_eq!(symbols[0].children.len(), 1);
        assert_eq!(symbols[0].children[0].name, "fmt");
        assert_eq!(symbols[0].children[0].kind, SymbolKind::Method);
        assert!(symbols[0].children[0].detail.as_ref().unwrap().contains("fn fmt(self: Self) -> String"));
    }

    #[test]
    fn test_get_document_symbols_type_alias() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "type Size = i64;".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![
                Item::TypeAlias(TypeAlias {
                    name: "Size".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    ty: Type::I64,
                    visibility: Visibility::Private,
                    span: Span::new(0, 16, 0, 0),
                }),
            ],
        };

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);

        let symbols = server.get_document_symbols("file:///test.pd");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Size");
        assert_eq!(symbols[0].kind, SymbolKind::TypeAlias);
        assert_eq!(symbols[0].detail, Some("type Size = i64".to_string()));
        assert!(symbols[0].children.is_empty());
    }

    #[test]
    fn test_get_document_symbols_multiple() {
        let mut server = create_test_server();
        
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Function(Function {
                    name: "main".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    params: vec![],
                    return_type: None,
                    body: vec![],
                    visibility: Visibility::Private,
                    is_async: false,
                    effects: None,
                    span: Span::new(0, 20, 0, 0),
                }),
                Item::Struct(StructDef {
                    name: "Data".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    fields: vec![("value".to_string(), Type::I32)],
                    visibility: Visibility::Private,
                    span: Span::new(22, 40, 0, 0),
                }),
                Item::Enum(EnumDef {
                    name: "Status".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    variants: vec![
                        EnumVariant {
                            name: "Ok".to_string(),
                            data: EnumVariantData::Unit,
                        },
                        EnumVariant {
                            name: "Error".to_string(),
                            data: EnumVariantData::Unit,
                        },
                    ],
                    span: Span::new(42, 60, 0, 0),
                }),
            ],
        };

        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "content".to_string()
        ).unwrap();

        let doc = server.documents.get_mut("file:///test.pd").unwrap();
        doc.ast = Some(ast);

        let symbols = server.get_document_symbols("file:///test.pd");
        assert_eq!(symbols.len(), 3);
        assert_eq!(symbols[0].name, "main");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[1].name, "Data");
        assert_eq!(symbols[1].kind, SymbolKind::Struct);
        assert_eq!(symbols[2].name, "Status");
        assert_eq!(symbols[2].kind, SymbolKind::Enum);
    }

    #[test]
    fn test_get_workspace_symbols_empty() {
        let server = create_test_server();
        let symbols = server.get_workspace_symbols("test");
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_get_workspace_symbols_with_matches() {
        let mut server = create_test_server();
        
        // Add some symbols to the index
        server.symbol_index.symbols.insert(
            "file:///test1.pd".to_string(),
            vec![
                Symbol {
                    name: "test_function".to_string(),
                    kind: SymbolKind::Function,
                    location: Location {
                        uri: "file:///test1.pd".to_string(),
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 0, character: 20 },
                        },
                    },
                    container_name: None,
                },
                Symbol {
                    name: "TestStruct".to_string(),
                    kind: SymbolKind::Struct,
                    location: Location {
                        uri: "file:///test1.pd".to_string(),
                        range: Range {
                            start: Position { line: 5, character: 0 },
                            end: Position { line: 10, character: 1 },
                        },
                    },
                    container_name: None,
                },
            ],
        );

        server.symbol_index.symbols.insert(
            "file:///test2.pd".to_string(),
            vec![
                Symbol {
                    name: "another_test".to_string(),
                    kind: SymbolKind::Function,
                    location: Location {
                        uri: "file:///test2.pd".to_string(),
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 0, character: 20 },
                        },
                    },
                    container_name: Some("module".to_string()),
                },
            ],
        );

        // Search for "test"
        let symbols = server.get_workspace_symbols("test");
        assert_eq!(symbols.len(), 3);
        
        // Verify all matches contain "test" (case-insensitive)
        for symbol in &symbols {
            assert!(symbol.name.to_lowercase().contains("test"));
        }
    }

    #[test]
    fn test_get_workspace_symbols_case_insensitive() {
        let mut server = create_test_server();
        
        server.symbol_index.symbols.insert(
            "file:///test.pd".to_string(),
            vec![
                Symbol {
                    name: "TEST_CONSTANT".to_string(),
                    kind: SymbolKind::Constant,
                    location: Location {
                        uri: "file:///test.pd".to_string(),
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 0, character: 13 },
                        },
                    },
                    container_name: None,
                },
            ],
        );

        // Search with lowercase should find uppercase
        let symbols = server.get_workspace_symbols("test");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "TEST_CONSTANT");
    }

    #[test]
    fn test_function_signature_simple() {
        let server = create_test_server();
        
        let func = Function {
            name: "main".to_string(),
            lifetime_params: vec![],
            type_params: vec![],
            const_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
            visibility: Visibility::Private,
            is_async: false,
            effects: None,
            span: Span::new(0, 10, 0, 0),
        };

        let sig = server.function_signature(&func);
        assert_eq!(sig, "fn main()");
    }

    #[test]
    fn test_function_signature_with_params_and_return() {
        let server = create_test_server();
        
        let func = Function {
            name: "add".to_string(),
            lifetime_params: vec![],
            type_params: vec![],
            const_params: vec![],
            params: vec![
                Param {
                    name: "a".to_string(),
                    ty: Type::I32,
                    mutable: false,
                },
                Param {
                    name: "b".to_string(),
                    ty: Type::I32,
                    mutable: false,
                },
            ],
            return_type: Some(Type::I32),
            body: vec![],
            visibility: Visibility::Private,
            is_async: false,
            effects: None,
            span: Span::new(0, 20, 0, 0),
        };

        let sig = server.function_signature(&func);
        assert_eq!(sig, "fn add(a: i32, b: i32) -> i32");
    }

    #[test]
    fn test_method_signature() {
        let server = create_test_server();
        
        let method = TraitMethod {
            name: "display".to_string(),
            lifetime_params: vec![],
            type_params: vec![],
            params: vec![
                Param {
                    name: "self".to_string(),
                    ty: Type::Custom("Self".to_string()),
                    mutable: false,
                },
            ],
            return_type: Some(Type::String),
            has_body: false,
                            body: None,            span: Span::new(0, 20, 0, 0),
        };

        let sig = server.method_signature(&method);
        assert_eq!(sig, "fn display(self: Self) -> String");
    }
}
