// References and definitions provider
// "Find all the places your code lives"

use super::{LanguageServer, Location, Position, Range};
use crate::ast::{Expr, Item, Program, Stmt};

impl LanguageServer {
    /// Find definition of symbol at position
    pub fn find_definition(&self, uri: &str, position: Position) -> Option<Location> {
        let doc = self.documents.get(uri)?;
        let symbol = self.find_symbol_at_position(&doc.content, position)?;

        // Look in symbol index
        if let Some(symbols) = self.symbol_index.symbols.get(&symbol) {
            // Return the first definition found
            return symbols.first().map(|s| s.location.clone());
        }

        None
    }

    /// Find all references to symbol at position
    pub fn find_references(
        &self,
        uri: &str,
        position: Position,
        include_declaration: bool,
    ) -> Vec<Location> {
        let mut references = Vec::new();

        let symbol = match self.find_symbol_at_position(
            &self
                .documents
                .get(uri)
                .map(|d| d.content.clone())
                .unwrap_or_default(),
            position,
        ) {
            Some(s) => s,
            None => return references,
        };

        // Search all documents
        for (doc_uri, doc) in &self.documents {
            if let Some(ast) = &doc.ast {
                let mut finder = ReferenceFinder {
                    symbol: symbol.clone(),
                    references: Vec::new(),
                    uri: doc_uri.clone(),
                    include_declaration,
                };

                finder.find_in_program(ast);
                references.extend(finder.references);
            }
        }

        references
    }

    /// Rename symbol at position
    pub fn prepare_rename(&self, uri: &str, position: Position) -> Option<(String, Range)> {
        let doc = self.documents.get(uri)?;
        let symbol = self.find_symbol_at_position(&doc.content, position)?;

        // Find the exact range of the symbol
        let lines: Vec<&str> = doc.content.lines().collect();
        let line = lines.get(position.line as usize)?;
        let chars: Vec<char> = line.chars().collect();

        let pos = position.character as usize;

        // Find word boundaries
        let mut start = pos;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }

        let mut end = pos;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }

        let range = Range {
            start: Position {
                line: position.line,
                character: start as u32,
            },
            end: Position {
                line: position.line,
                character: end as u32,
            },
        };

        Some((symbol, range))
    }

    /// Compute rename edits
    pub fn compute_rename_edits(
        &self,
        uri: &str,
        position: Position,
        new_name: &str,
    ) -> HashMap<String, Vec<TextEdit>> {
        use std::collections::HashMap;

        let mut edits: HashMap<String, Vec<TextEdit>> = HashMap::new();

        // Find all references
        let references = self.find_references(uri, position, true);

        // Create edits for each reference
        for location in references {
            let doc_edits = edits.entry(location.uri).or_default();
            doc_edits.push(TextEdit {
                range: location.range,
                new_text: new_name.to_string(),
            });
        }

        edits
    }
}

/// Text edit
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

/// Helper to find references in AST
struct ReferenceFinder {
    symbol: String,
    references: Vec<Location>,
    uri: String,
    include_declaration: bool,
}

impl ReferenceFinder {
    fn find_in_program(&mut self, program: &Program) {
        for item in &program.items {
            self.find_in_item(item);
        }
    }

    fn find_in_item(&mut self, item: &Item) {
        match item {
            Item::Function(func) => {
                if self.include_declaration && func.name == self.symbol {
                    self.add_reference(func.span);
                }

                self.find_in_statements(&func.body);
            }
            Item::Struct(struct_def) => {
                if self.include_declaration && struct_def.name == self.symbol {
                    self.add_reference(struct_def.span);
                }
            }
            Item::Enum(enum_def) => {
                if self.include_declaration && enum_def.name == self.symbol {
                    self.add_reference(enum_def.span);
                }
            }
            Item::Trait(trait_def) => {
                if self.include_declaration && trait_def.name == self.symbol {
                    self.add_reference(trait_def.span);
                }

                for method in &trait_def.methods {
                    if let Some(body) = &method.body {
                        self.find_in_statements(body);
                    }
                }
            }
            Item::Impl(impl_block) => {
                for func in &impl_block.methods {
                    self.find_in_statements(&func.body);
                }
            }
            _ => {}
        }
    }

    fn find_in_statements(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.find_in_statement(stmt);
        }
    }

    fn find_in_statement(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { value, .. } => {
                self.find_in_expression(value);
            }
            Stmt::Expr(expr) => {
                self.find_in_expression(expr);
            }
            Stmt::Return(Some(expr)) => {
                self.find_in_expression(expr);
            }
            Stmt::Return(None) => {}
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.find_in_expression(condition);
                self.find_in_statements(then_branch);
                if let Some(else_branch) = else_branch {
                    self.find_in_statements(else_branch);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.find_in_expression(condition);
                self.find_in_statements(body);
            }
            Stmt::For { iter, body, .. } => {
                self.find_in_expression(iter);
                self.find_in_statements(body);
            }
            Stmt::Match { expr, arms, .. } => {
                self.find_in_expression(expr);
                for arm in arms {
                    self.find_in_statements(&arm.body);
                }
            }
            _ => {}
        }
    }

    fn find_in_expression(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(name) => {
                if name == &self.symbol {
                    self.add_reference(crate::errors::Span {
                        start: 0,
                        end: 0,
                        line: 0,
                        column: 0,
                    });
                }
            }
            Expr::Call { func, args, .. } => {
                self.find_in_expression(func);
                for arg in args {
                    self.find_in_expression(arg);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.find_in_expression(left);
                self.find_in_expression(right);
            }
            Expr::Unary { operand, .. } => {
                self.find_in_expression(operand);
            }
            Expr::FieldAccess { object, .. } => {
                self.find_in_expression(object);
            }
            Expr::Index { array, index, .. } => {
                self.find_in_expression(array);
                self.find_in_expression(index);
            }
            Expr::StructLiteral { name, fields, .. } => {
                if name == &self.symbol {
                    // TODO: Add reference to struct name
                }
                for (_, expr) in fields {
                    self.find_in_expression(expr);
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                for elem in elements {
                    self.find_in_expression(elem);
                }
            }
            Expr::ArrayRepeat { value, count, .. } => {
                self.find_in_expression(value);
                self.find_in_expression(count);
            }
            _ => {}
        }
    }

    fn add_reference(&mut self, span: crate::errors::Span) {
        self.references.push(Location {
            uri: self.uri.clone(),
            range: Range {
                start: Position {
                    line: span.start as u32,
                    character: 0, // TODO: Calculate character offset
                },
                end: Position {
                    line: span.end as u32,
                    character: 0, // TODO: Calculate character offset
                },
            },
        });
    }
}

use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Program, Item, Function, StructDef, EnumDef, TraitDef, Type, Stmt, Expr,
                      BinOp, UnaryOp, ImplBlock, Visibility};
    use crate::errors::Span;
    use crate::lsp::{Symbol, SymbolKind};

    fn create_test_server() -> LanguageServer {
        let mut server = LanguageServer::new();
        server.initialize(None).unwrap();
        server
    }

    fn create_position(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn test_text_edit_creation() {
        let edit = TextEdit {
            range: Range {
                start: Position { line: 0, character: 5 },
                end: Position { line: 0, character: 10 },
            },
            new_text: "hello".to_string(),
        };

        assert_eq!(edit.new_text, "hello");
        assert_eq!(edit.range.start.character, 5);
        assert_eq!(edit.range.end.character, 10);
    }

    #[test]
    fn test_find_definition_no_document() {
        let server = create_test_server();
        let def = server.find_definition("file:///test.pd", create_position(0, 0));
        assert!(def.is_none());
    }

    #[test]
    fn test_find_definition_no_symbol() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "   ".to_string() // Just spaces
        ).unwrap();

        let def = server.find_definition("file:///test.pd", create_position(0, 1));
        assert!(def.is_none());
    }

    #[test]
    fn test_find_definition_in_index() {
        let mut server = create_test_server();
        
        // Add document
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "main()".to_string()
        ).unwrap();

        // Add symbol to index
        server.symbol_index.symbols.insert(
            "main".to_string(),
            vec![Symbol {
                name: "main".to_string(),
                kind: SymbolKind::Function,
                location: Location {
                    uri: "file:///lib.pd".to_string(),
                    range: Range {
                        start: Position { line: 10, character: 0 },
                        end: Position { line: 10, character: 20 },
                    },
                },
                container_name: None,
            }],
        );

        let def = server.find_definition("file:///test.pd", create_position(0, 0));
        assert!(def.is_some());
        
        let location = def.unwrap();
        assert_eq!(location.uri, "file:///lib.pd");
        assert_eq!(location.range.start.line, 10);
    }

    #[test]
    fn test_find_references_no_symbol() {
        let server = create_test_server();
        let refs = server.find_references("file:///test.pd", create_position(0, 0), true);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_find_references_simple() {
        let mut server = create_test_server();
        
        // Add two documents with the same symbol
        server.open_document(
            "file:///test1.pd".to_string(),
            1,
            "fn foo() {}\nfoo();".to_string()
        ).unwrap();

        let ast1 = Program {
            imports: vec![],
            items: vec![
                Item::Function(Function {
                    name: "foo".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    params: vec![],
                    return_type: None,
                    body: vec![
                        Stmt::Expr(Expr::Call {
                            func: Box::new(Expr::Ident("foo".to_string())),
                            args: vec![],
                            span: Span::new(12, 17, 0, 0),
                        }),
                    ],
                    visibility: Visibility::Private,
                    is_async: false,
                    effects: None,
                    span: Span::new(0, 11, 0, 0),
                }),
            ],
        };

        server.documents.get_mut("file:///test1.pd").unwrap().ast = Some(ast1);

        let refs = server.find_references("file:///test1.pd", create_position(0, 3), true);
        assert_eq!(refs.len(), 2); // Declaration + usage
    }

    #[test]
    fn test_find_references_exclude_declaration() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "fn foo() {}\nfoo();".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Function(Function {
                    name: "foo".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    params: vec![],
                    return_type: None,
                    body: vec![
                        Stmt::Expr(Expr::Call {
                            func: Box::new(Expr::Ident("foo".to_string())),
                            args: vec![],
                            span: Span::new(12, 17, 0, 0),
                        }),
                    ],
                    visibility: Visibility::Private,
                    is_async: false,
                    effects: None,
                    span: Span::new(0, 11, 0, 0),
                }),
            ],
        };

        server.documents.get_mut("file:///test.pd").unwrap().ast = Some(ast);

        let refs = server.find_references("file:///test.pd", create_position(0, 3), false);
        assert_eq!(refs.len(), 1); // Only usage, not declaration
    }

    #[test]
    fn test_prepare_rename_simple() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "let hello_world = 42;".to_string()
        ).unwrap();

        let result = server.prepare_rename("file:///test.pd", create_position(0, 6));
        assert!(result.is_some());
        
        let (symbol, range) = result.unwrap();
        assert_eq!(symbol, "hello_world");
        assert_eq!(range.start.character, 4);
        assert_eq!(range.end.character, 15);
    }

    #[test]
    fn test_prepare_rename_at_boundaries() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "let test = 42;".to_string()
        ).unwrap();

        // Test at start of identifier
        let result = server.prepare_rename("file:///test.pd", create_position(0, 4));
        assert!(result.is_some());
        let (symbol, _) = result.unwrap();
        assert_eq!(symbol, "test");

        // Test at end of identifier
        let result = server.prepare_rename("file:///test.pd", create_position(0, 7));
        assert!(result.is_some());
        let (symbol, _) = result.unwrap();
        assert_eq!(symbol, "test");
    }

    #[test]
    fn test_compute_rename_edits() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "fn foo() {}\nfoo();".to_string()
        ).unwrap();

        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Function(Function {
                    name: "foo".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    params: vec![],
                    return_type: None,
                    body: vec![],
                    visibility: Visibility::Private,
                    is_async: false,
                    effects: None,
                    span: Span::new(0, 11, 0, 0),
                }),
            ],
        };

        server.documents.get_mut("file:///test.pd").unwrap().ast = Some(ast);

        // Mock find_references to return some locations
        let edits = server.compute_rename_edits("file:///test.pd", create_position(0, 3), "bar");
        
        // Should have edits for at least one file
        assert!(!edits.is_empty());
        
        // All edits should replace with "bar"
        for (_, file_edits) in edits {
            for edit in file_edits {
                assert_eq!(edit.new_text, "bar");
            }
        }
    }

    #[test]
    fn test_reference_finder_struct() {
        let mut finder = ReferenceFinder {
            symbol: "Point".to_string(),
            references: Vec::new(),
            uri: "file:///test.pd".to_string(),
            include_declaration: true,
        };

        let program = Program {
            imports: vec![],
            items: vec![
                Item::Struct(StructDef {
                    name: "Point".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    fields: vec![],
                    visibility: Visibility::Private,
                    span: Span::new(0, 20, 0, 0),
                }),
            ],
        };

        finder.find_in_program(&program);
        assert_eq!(finder.references.len(), 1);
    }

    #[test]
    fn test_reference_finder_enum() {
        let mut finder = ReferenceFinder {
            symbol: "Status".to_string(),
            references: Vec::new(),
            uri: "file:///test.pd".to_string(),
            include_declaration: true,
        };

        let program = Program {
            imports: vec![],
            items: vec![
                Item::Enum(EnumDef {
                    name: "Status".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    variants: vec![],
                    span: Span::new(0, 20, 0, 0),
                }),
            ],
        };

        finder.find_in_program(&program);
        assert_eq!(finder.references.len(), 1);
    }

    #[test]
    fn test_reference_finder_trait() {
        let mut finder = ReferenceFinder {
            symbol: "Display".to_string(),
            references: Vec::new(),
            uri: "file:///test.pd".to_string(),
            include_declaration: true,
        };

        let program = Program {
            imports: vec![],
            items: vec![
                Item::Trait(TraitDef {
                    name: "Display".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    methods: vec![],
                    visibility: Visibility::Private,
                    span: Span::new(0, 20, 0, 0),
                }),
            ],
        };

        finder.find_in_program(&program);
        assert_eq!(finder.references.len(), 1);
    }

    #[test]
    fn test_reference_finder_in_function_body() {
        let mut finder = ReferenceFinder {
            symbol: "x".to_string(),
            references: Vec::new(),
            uri: "file:///test.pd".to_string(),
            include_declaration: false,
        };

        let program = Program {
            imports: vec![],
            items: vec![
                Item::Function(Function {
                    name: "test".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    params: vec![],
                    return_type: None,
                    body: vec![
                        Stmt::Let {
                            name: "x".to_string(),
                            mutable: false,
                            ty: Some(Type::I32),
                            value: Expr::Integer(42),
                            span: Span::new(0, 10, 0, 0),
                        },
                        Stmt::Expr(Expr::Ident("x".to_string())),
                    ],
                    visibility: Visibility::Private,
                    is_async: false,
                    effects: None,
                    span: Span::new(0, 30, 0, 0),
                }),
            ],
        };

        finder.find_in_program(&program);
        assert_eq!(finder.references.len(), 1); // Only the usage, not the declaration
    }

    #[test]
    fn test_reference_finder_in_expressions() {
        let mut finder = ReferenceFinder {
            symbol: "foo".to_string(),
            references: Vec::new(),
            uri: "file:///test.pd".to_string(),
            include_declaration: false,
        };

        // Test various expression types
        let exprs = vec![
            // Binary expression
            Expr::Binary {
                left: Box::new(Expr::Ident("foo".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::Integer(1)),
                span: Span::new(0, 10, 0, 0),
            },
            // Unary expression
            Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(Expr::Ident("foo".to_string())),
                span: Span::new(0, 5, 0, 0),
            },
            // Call expression
            Expr::Call {
                func: Box::new(Expr::Ident("bar".to_string())),
                args: vec![Expr::Ident("foo".to_string())],
                span: Span::new(0, 10, 0, 0),
            },
            // Field access
            Expr::FieldAccess {
                object: Box::new(Expr::Ident("foo".to_string())),
                field: "x".to_string(),
                span: Span::new(0, 5, 0, 0),
            },
            // Array index
            Expr::Index {
                array: Box::new(Expr::Ident("foo".to_string())),
                index: Box::new(Expr::Integer(0)),
                span: Span::new(0, 6, 0, 0),
            },
        ];

        for expr in exprs {
            finder.references.clear();
            finder.find_in_expression(&expr);
            assert!(!finder.references.is_empty(), "Should find reference in {:?}", expr);
        }
    }

    #[test]
    fn test_reference_finder_in_control_flow() {
        let mut finder = ReferenceFinder {
            symbol: "cond".to_string(),
            references: Vec::new(),
            uri: "file:///test.pd".to_string(),
            include_declaration: false,
        };

        // Test if statement
        let if_stmt = Stmt::If {
            condition: Expr::Ident("cond".to_string()),
            then_branch: vec![],
            else_branch: Some(vec![
                Stmt::Expr(Expr::Ident("cond".to_string())),
            ]),
            span: Span::new(0, 20, 0, 0),
        };

        finder.find_in_statement(&if_stmt);
        assert_eq!(finder.references.len(), 2); // In condition and else branch

        // Test while loop
        finder.references.clear();
        let while_stmt = Stmt::While {
            condition: Expr::Ident("cond".to_string()),
            body: vec![
                Stmt::Expr(Expr::Ident("cond".to_string())),
            ],
            span: Span::new(0, 20, 0, 0),
        };

        finder.find_in_statement(&while_stmt);
        assert_eq!(finder.references.len(), 2); // In condition and body
    }

    #[test]
    fn test_reference_finder_struct_literal() {
        let mut finder = ReferenceFinder {
            symbol: "Point".to_string(),
            references: Vec::new(),
            uri: "file:///test.pd".to_string(),
            include_declaration: false,
        };
        let expr = Expr::StructLiteral {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Expr::Integer(10)),
                ("y".to_string(), Expr::Integer(20)),
            ],
            span: Span::new(0, 30, 0, 0),
        };
        finder.find_in_expression(&expr);
        // TODO: Currently doesn't add reference for struct name in literals
        // This is noted in the code with a TODO comment
    }

    #[test]
    fn test_reference_finder_array_expressions() {
        let mut finder = ReferenceFinder {
            symbol: "x".to_string(),
            references: Vec::new(),
            uri: "file:///test.pd".to_string(),
            include_declaration: false,
        };

        // Array literal
        let array_lit = Expr::ArrayLiteral {
            elements: vec![
                Expr::Ident("x".to_string()),
                Expr::Integer(2),
                Expr::Ident("x".to_string()),
            ],
            span: Span::new(0, 20, 0, 0),
        };

        finder.find_in_expression(&array_lit);
        assert_eq!(finder.references.len(), 2);

        // Array repeat
        finder.references.clear();
        let array_repeat = Expr::ArrayRepeat {
            value: Box::new(Expr::Ident("x".to_string())),
            count: Box::new(Expr::Ident("x".to_string())),
            span: Span::new(0, 10, 0, 0),
        };

        finder.find_in_expression(&array_repeat);
        assert_eq!(finder.references.len(), 2);
    }

    #[test]
    fn test_reference_finder_impl_block() {
        let mut finder = ReferenceFinder {
            symbol: "self".to_string(),
            references: Vec::new(),
            uri: "file:///test.pd".to_string(),
            include_declaration: false,
        };

        let program = Program {
            imports: vec![],
            items: vec![
                Item::Impl(ImplBlock {
                    for_type: Type::Custom("Point".to_string()),
                    lifetime_params: vec![],
                    type_params: vec![],
                    trait_type: None,
                    methods: vec![
                        Function {
                            name: "new".to_string(),
                            lifetime_params: vec![],
                            type_params: vec![],
                            const_params: vec![],
                            params: vec![],
                            return_type: Some(Type::Custom("Self".to_string())),
                            body: vec![
                                Stmt::Expr(Expr::Ident("self".to_string())),
                            ],
                            visibility: Visibility::Private,
                            is_async: false,
                            effects: None,
                            span: Span::new(0, 20, 0, 0),
                        },
                    ],
                    span: Span::new(0, 50, 0, 0),
                }),
            ],
        };

        finder.find_in_program(&program);
        assert_eq!(finder.references.len(), 1);
    }

    #[test]
    fn test_prepare_rename_multiline() {
        let mut server = create_test_server();
        
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "fn main() {\n    let variable_name = 42;\n}".to_string()
        ).unwrap();

        let result = server.prepare_rename("file:///test.pd", create_position(1, 10));
        assert!(result.is_some());
        
        let (symbol, range) = result.unwrap();
        assert_eq!(symbol, "variable_name");
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 8);
        assert_eq!(range.end.character, 21);
    }
}
