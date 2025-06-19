// Language Server Protocol implementation for Palladium
// "Bringing IDE intelligence to legendary code"

pub mod server;
pub mod handlers;
pub mod analysis;
pub mod diagnostics;
pub mod completion;
pub mod hover;
pub mod symbols;
pub mod references;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use crate::errors::{CompileError, Result};

/// Language server state
pub struct LanguageServer {
    /// Workspace root
    workspace_root: Option<PathBuf>,
    /// Open documents
    documents: HashMap<String, Document>,
    /// Diagnostics for each document
    diagnostics: HashMap<String, Vec<Diagnostic>>,
    /// Symbol index
    symbol_index: SymbolIndex,
    /// Server capabilities
    capabilities: ServerCapabilities,
}

/// Document in the workspace
#[derive(Debug, Clone)]
pub struct Document {
    /// Document URI
    pub uri: String,
    /// Document version
    pub version: i32,
    /// Document content
    pub content: String,
    /// Parsed AST (if available)
    pub ast: Option<crate::ast::Program>,
    /// Type information
    pub type_info: Option<TypeInfo>,
}

/// Type information for a document
#[derive(Debug, Clone)]
pub struct TypeInfo {
    /// Variable types
    pub variables: HashMap<String, crate::ast::Type>,
    /// Function signatures
    pub functions: HashMap<String, FunctionSignature>,
    /// Type aliases
    pub type_aliases: HashMap<String, crate::ast::Type>,
}

/// Function signature information
#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub params: Vec<(String, crate::ast::Type)>,
    pub return_type: Option<crate::ast::Type>,
    pub is_async: bool,
    pub effects: Vec<String>,
}

/// Symbol index for quick lookup
#[derive(Debug, Default)]
pub struct SymbolIndex {
    /// Symbols by name
    pub symbols: HashMap<String, Vec<Symbol>>,
    /// Symbols by file
    pub file_symbols: HashMap<String, Vec<Symbol>>,
}

/// Symbol information
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Location,
    pub container_name: Option<String>,
}

/// Symbol kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Variable,
    Constant,
    TypeAlias,
    Module,
    Method,
    Field,
    EnumVariant,
}

/// Location in a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

/// Range in a document
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// Position in a document
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// Diagnostic (error, warning, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub code: Option<String>,
    pub source: Option<String>,
    pub message: String,
    pub related_information: Vec<DiagnosticRelatedInformation>,
}

/// Diagnostic severity
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum DiagnosticSeverity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

/// Related diagnostic information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticRelatedInformation {
    pub location: Location,
    pub message: String,
}

/// Server capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub text_document_sync: TextDocumentSyncKind,
    pub hover_provider: bool,
    pub completion_provider: Option<CompletionOptions>,
    pub definition_provider: bool,
    pub references_provider: bool,
    pub document_highlight_provider: bool,
    pub document_symbol_provider: bool,
    pub workspace_symbol_provider: bool,
    pub code_action_provider: bool,
    pub code_lens_provider: bool,
    pub document_formatting_provider: bool,
    pub document_range_formatting_provider: bool,
    pub rename_provider: bool,
    pub diagnostic_provider: bool,
}

/// Text document sync kind
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum TextDocumentSyncKind {
    None = 0,
    Full = 1,
    Incremental = 2,
}

/// Completion options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionOptions {
    pub resolve_provider: bool,
    pub trigger_characters: Vec<String>,
}

impl LanguageServer {
    /// Create a new language server
    pub fn new() -> Self {
        Self {
            workspace_root: None,
            documents: HashMap::new(),
            diagnostics: HashMap::new(),
            symbol_index: SymbolIndex::default(),
            capabilities: ServerCapabilities {
                text_document_sync: TextDocumentSyncKind::Full,
                hover_provider: true,
                completion_provider: Some(CompletionOptions {
                    resolve_provider: true,
                    trigger_characters: vec![".".to_string(), "::".to_string()],
                }),
                definition_provider: true,
                references_provider: true,
                document_highlight_provider: true,
                document_symbol_provider: true,
                workspace_symbol_provider: true,
                code_action_provider: true,
                code_lens_provider: false,
                document_formatting_provider: true,
                document_range_formatting_provider: true,
                rename_provider: true,
                diagnostic_provider: true,
            },
        }
    }
    
    /// Initialize the language server
    pub fn initialize(&mut self, root_uri: Option<String>) -> Result<()> {
        if let Some(uri) = root_uri {
            self.workspace_root = Some(self.uri_to_path(&uri)?);
        }
        
        // Index workspace if available
        if let Some(root) = self.workspace_root.clone() {
            self.index_workspace(&root)?;
        }
        
        Ok(())
    }
    
    /// Open a document
    pub fn open_document(&mut self, uri: String, version: i32, content: String) -> Result<()> {
        let document = Document {
            uri: uri.clone(),
            version,
            content: content.clone(),
            ast: None,
            type_info: None,
        };
        
        self.documents.insert(uri.clone(), document);
        
        // Analyze the document
        self.analyze_document(&uri)?;
        
        Ok(())
    }
    
    /// Update a document
    pub fn update_document(&mut self, uri: String, version: i32, content: String) -> Result<()> {
        if let Some(doc) = self.documents.get_mut(&uri) {
            doc.version = version;
            doc.content = content;
            doc.ast = None;
            doc.type_info = None;
        }
        
        // Re-analyze the document
        self.analyze_document(&uri)?;
        
        Ok(())
    }
    
    /// Close a document
    pub fn close_document(&mut self, uri: String) -> Result<()> {
        self.documents.remove(&uri);
        self.diagnostics.remove(&uri);
        Ok(())
    }
    
    /// Analyze a document
    fn analyze_document(&mut self, uri: &str) -> Result<()> {
        let doc = self.documents.get(uri)
            .ok_or_else(|| CompileError::Generic(format!("Document not found: {}", uri)))?
            .clone();
        
        // Parse the document
        let mut diagnostics = Vec::new();
        
        match self.parse_document(&doc.content) {
            Ok(ast) => {
                // Update AST
                if let Some(doc) = self.documents.get_mut(uri) {
                    doc.ast = Some(ast.clone());
                }
                
                // Type check
                match self.typecheck_document(&ast) {
                    Ok(type_info) => {
                        if let Some(doc) = self.documents.get_mut(uri) {
                            doc.type_info = Some(type_info);
                        }
                    }
                    Err(e) => {
                        diagnostics.push(self.error_to_diagnostic(e));
                    }
                }
                
                // Index symbols
                self.index_document_symbols(uri, &ast)?;
            }
            Err(e) => {
                diagnostics.push(self.error_to_diagnostic(e));
            }
        }
        
        // Store diagnostics
        self.diagnostics.insert(uri.to_string(), diagnostics);
        
        Ok(())
    }
    
    /// Parse a document
    fn parse_document(&self, content: &str) -> Result<crate::ast::Program> {
        let mut lexer = crate::lexer::Lexer::new(content);
        let tokens = lexer.collect_tokens()?;
        let mut parser = crate::parser::Parser::new(tokens);
        parser.parse()
    }
    
    /// Type check a document
    fn typecheck_document(&self, ast: &crate::ast::Program) -> Result<TypeInfo> {
        let mut type_checker = crate::typeck::TypeChecker::new();
        type_checker.check(ast)?;
        
        // Extract type information
        // TODO: Implement type info extraction
        Ok(TypeInfo {
            variables: HashMap::new(),
            functions: HashMap::new(),
            type_aliases: HashMap::new(),
        })
    }
    
    /// Index document symbols
    fn index_document_symbols(&mut self, uri: &str, ast: &crate::ast::Program) -> Result<()> {
        let mut symbols = Vec::new();
        
        for item in &ast.items {
            match item {
                crate::ast::Item::Function(func) => {
                    symbols.push(Symbol {
                        name: func.name.clone(),
                        kind: SymbolKind::Function,
                        location: Location {
                            uri: uri.to_string(),
                            range: self.span_to_range(func.span),
                        },
                        container_name: None,
                    });
                }
                crate::ast::Item::Struct(struct_def) => {
                    symbols.push(Symbol {
                        name: struct_def.name.clone(),
                        kind: SymbolKind::Struct,
                        location: Location {
                            uri: uri.to_string(),
                            range: self.span_to_range(struct_def.span),
                        },
                        container_name: None,
                    });
                }
                crate::ast::Item::Enum(enum_def) => {
                    symbols.push(Symbol {
                        name: enum_def.name.clone(),
                        kind: SymbolKind::Enum,
                        location: Location {
                            uri: uri.to_string(),
                            range: self.span_to_range(enum_def.span),
                        },
                        container_name: None,
                    });
                }
                crate::ast::Item::Trait(trait_def) => {
                    symbols.push(Symbol {
                        name: trait_def.name.clone(),
                        kind: SymbolKind::Trait,
                        location: Location {
                            uri: uri.to_string(),
                            range: self.span_to_range(trait_def.span),
                        },
                        container_name: None,
                    });
                }
                _ => {}
            }
        }
        
        // Update symbol index
        self.symbol_index.file_symbols.insert(uri.to_string(), symbols.clone());
        
        for symbol in symbols {
            self.symbol_index.symbols
                .entry(symbol.name.clone())
                .or_insert_with(Vec::new)
                .push(symbol);
        }
        
        Ok(())
    }
    
    /// Index the workspace
    fn index_workspace(&mut self, root: &std::path::Path) -> Result<()> {
        // TODO: Walk workspace and index all .pd files
        Ok(())
    }
    
    /// Convert URI to file path
    fn uri_to_path(&self, uri: &str) -> Result<PathBuf> {
        if uri.starts_with("file://") {
            Ok(PathBuf::from(&uri[7..]))
        } else {
            Err(CompileError::Generic(format!("Invalid URI: {}", uri)))
        }
    }
    
    /// Convert span to range
    fn span_to_range(&self, span: crate::errors::Span) -> Range {
        Range {
            start: Position {
                line: span.start as u32,
                character: 0, // TODO: Calculate character offset
            },
            end: Position {
                line: span.end as u32,
                character: 0, // TODO: Calculate character offset
            },
        }
    }
    
    /// Convert error to diagnostic
    fn error_to_diagnostic(&self, error: CompileError) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 0 },
            },
            severity: DiagnosticSeverity::Error,
            code: None,
            source: Some("palladium".to_string()),
            message: error.to_string(),
            related_information: Vec::new(),
        }
    }
}