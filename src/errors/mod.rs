// Error handling for Palladium compiler
// "Even legends make mistakes, but they handle them gracefully"

use thiserror::Error;

pub mod reporter;

pub type Result<T> = std::result::Result<T, CompileError>;

#[derive(Error, Debug)]
pub enum CompileError {
    // Lexer errors
    #[error("Unexpected character '{ch}' at line {line}, column {col}")]
    UnexpectedChar { ch: char, line: usize, col: usize },
    
    #[error("Unterminated string literal at line {line}")]
    UnterminatedString { line: usize },
    
    // Parser errors
    #[error("Unexpected token: expected {expected}, found {found}")]
    UnexpectedToken { expected: String, found: String },
    
    #[error("Syntax error: {message}")]
    SyntaxError { message: String },
    
    // Type errors
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },
    
    #[error("Undefined function: {name}")]
    UndefinedFunction { name: String },
    
    #[error("Function {name} expects {expected} arguments, but {found} were provided")]
    ArgumentCountMismatch { name: String, expected: usize, found: usize },
    
    // Codegen errors
    #[error("Code generation failed: {message}")]
    CodegenError { message: String },
    
    // IO errors
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    // Generic error
    #[error("{0}")]
    Generic(String),
}

/// Source location information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self { start, end, line, column }
    }
    
    pub fn dummy() -> Self {
        Self { start: 0, end: 0, line: 0, column: 0 }
    }
}

/// A diagnostic message with source location
#[derive(Debug)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub span: Option<Span>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Info,
    Help,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            message: message.into(),
            span: None,
            notes: Vec::new(),
        }
    }
    
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
    
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}