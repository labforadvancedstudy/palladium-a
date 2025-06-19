// References and definitions provider
// "Find all the places your code lives"

use super::{LanguageServer, Location, Position, Range};
use crate::ast::{Program, Item, Expr, Stmt};

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
    pub fn find_references(&self, uri: &str, position: Position, include_declaration: bool) -> Vec<Location> {
        let mut references = Vec::new();
        
        let symbol = match self.find_symbol_at_position(
            &self.documents.get(uri).map(|d| d.content.clone()).unwrap_or_default(), 
            position
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
    pub fn compute_rename_edits(&self, uri: &str, position: Position, new_name: &str) -> HashMap<String, Vec<TextEdit>> {
        use std::collections::HashMap;
        
        let mut edits: HashMap<String, Vec<TextEdit>> = HashMap::new();
        
        // Find all references
        let references = self.find_references(uri, position, true);
        
        // Create edits for each reference
        for location in references {
            let doc_edits = edits.entry(location.uri).or_insert_with(Vec::new);
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
            Stmt::Return(expr) => {
                if let Some(expr) = expr {
                    self.find_in_expression(expr);
                }
            }
            Stmt::If { condition, then_branch, else_branch, .. } => {
                self.find_in_expression(condition);
                self.find_in_statements(then_branch);
                if let Some(else_branch) = else_branch {
                    self.find_in_statements(else_branch);
                }
            }
            Stmt::While { condition, body, .. } => {
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
                    self.add_reference(crate::errors::Span { start: 0, end: 0, line: 0, column: 0 });
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