// Error handling for Palladium compiler
// "Even legends make mistakes, but they handle them gracefully"

use thiserror::Error;

pub mod pretty;
pub mod reporter;
pub mod suggestions;

pub type Result<T> = std::result::Result<T, CompileError>;

#[derive(Error, Debug)]
pub enum CompileError {
    // Lexer errors
    #[error("Unexpected character '{ch}' at line {line}, column {col}")]
    UnexpectedChar {
        ch: char,
        line: usize,
        col: usize,
        span: Option<Span>,
    },

    #[error("Unterminated string literal at line {line}")]
    UnterminatedString { line: usize, span: Option<Span> },

    // Parser errors
    #[error("Unexpected token: expected {expected}, found {found}")]
    UnexpectedToken {
        expected: String,
        found: String,
        span: Option<Span>,
    },

    #[error("Syntax error: {message}")]
    SyntaxError { message: String, span: Option<Span> },

    // Type errors
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch {
        expected: String,
        found: String,
        span: Option<Span>,
    },

    #[error("Undefined variable: {name}")]
    UndefinedVariable { name: String, span: Option<Span> },

    #[error("Undefined function: {name}")]
    UndefinedFunction { name: String, span: Option<Span> },

    #[error("Function {name} expects {expected} arguments, but {found} were provided")]
    ArgumentCountMismatch {
        name: String,
        expected: usize,
        found: usize,
        span: Option<Span>,
    },

    // Codegen errors
    #[error("Code generation failed: {message}")]
    CodegenError { message: String },

    // IO errors
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    // Generic error
    #[error("{0}")]
    Generic(String),

    // Missing semicolon
    #[error("Missing semicolon after statement")]
    MissingSemicolon { span: Option<Span> },

    // Invalid function signature
    #[error("Invalid function signature")]
    InvalidFunctionSignature { message: String, span: Option<Span> },

    // Borrow checker errors
    #[error("Borrow checker error: {message}")]
    BorrowChecker { message: String, span: Option<Span> },

    #[error("Use of moved value: {name}")]
    UseOfMovedValue { name: String, span: Option<Span> },

    #[error("Use of uninitialized value: {name}")]
    UseOfUninitializedValue { name: String, span: Option<Span> },

    #[error("Cannot move out of borrowed content")]
    CannotMoveOutOfBorrowedContent { span: Option<Span> },

    // Unsafe operation errors
    #[error("Unsafe operation '{operation}' requires unsafe block")]
    UnsafeOperation { operation: String, span: Span },

    #[error("Conflicting borrows: {message}")]
    ConflictingBorrows { message: String, span: Option<Span> },

    #[error("Lifetime error: {message}")]
    LifetimeError { message: String, span: Option<Span> },

    // Pattern matching errors
    #[error("Non-exhaustive match: missing patterns {}", missing_patterns.join(", "))]
    NonExhaustiveMatch {
        missing_patterns: Vec<String>,
        span: Option<Span>,
    },

    #[error("Unreachable pattern: {}", patterns.join(", "))]
    UnreachablePattern {
        patterns: Vec<String>,
        span: Option<Span>,
    },
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
        Self {
            start,
            end,
            line,
            column,
        }
    }

    pub fn dummy() -> Self {
        Self {
            start: 0,
            end: 0,
            line: 0,
            column: 0,
        }
    }

    pub fn extend_to(&self, other: &Span) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line.min(other.line),
            column: if self.line < other.line {
                self.column
            } else {
                self.column.min(other.column)
            },
        }
    }
}

impl CompileError {
    /// Convert this error into a diagnostic with helpful suggestions
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            CompileError::UnexpectedChar {
                ch,
                line,
                col,
                span,
            } => Diagnostic::error(format!(
                "Unexpected character '{}' at line {}, column {}",
                ch, line, col
            ))
            .with_span(span.unwrap_or(Span::new(0, 1, *line, *col)))
            .with_note("Palladium only allows ASCII letters, numbers, and common symbols")
            .with_suggestion("Remove or replace this character with a valid one", None),

            CompileError::UnterminatedString { line, span } => {
                Diagnostic::error(format!("Unterminated string literal at line {}", line))
                    .with_span(span.unwrap_or(Span::new(0, 0, *line, 1)))
                    .with_note("Strings must be closed with a matching quote")
                    .with_suggestion(
                        "Add a closing quote (\") to end the string",
                        Some("\"".to_string()),
                    )
            }

            CompileError::UnexpectedToken {
                expected,
                found,
                span,
            } => Diagnostic::error(format!("Expected {}, but found {}", expected, found))
                .with_span(span.unwrap_or(Span::dummy()))
                .with_note("The syntax requires a specific token here")
                .with_suggestion(
                    format!("Replace '{}' with '{}'", found, expected),
                    Some(expected.clone()),
                ),

            CompileError::SyntaxError { message, span } => Diagnostic::error(message.clone())
                .with_span(span.unwrap_or(Span::dummy()))
                .with_note("Check the language syntax rules"),

            CompileError::TypeMismatch {
                expected,
                found,
                span,
            } => {
                let mut diag = Diagnostic::error(format!(
                    "Type mismatch: expected {}, found {}",
                    expected, found
                ))
                .with_span(span.unwrap_or(Span::dummy()))
                .with_note("Types must match exactly in Palladium");

                // Add specific suggestions based on common type mismatches
                match (expected.as_str(), found.as_str()) {
                    ("int", "string") => {
                        diag = diag.with_suggestion(
                            "Convert the string to an integer using parse_int()",
                            None,
                        );
                    }
                    ("string", "int") => {
                        diag = diag.with_suggestion(
                            "Convert the integer to a string using to_string()",
                            None,
                        );
                    }
                    ("bool", _) => {
                        diag =
                            diag.with_suggestion("Use 'true' or 'false' for boolean values", None);
                    }
                    _ => {}
                }

                diag
            }

            CompileError::UndefinedVariable { name, span } => {
                Diagnostic::error(format!("Undefined variable: {}", name))
                    .with_span(span.unwrap_or(Span::dummy()))
                    .with_note("Variables must be declared before use")
                    .with_suggestion(
                        format!("Did you mean to declare it? Try: let {} = ...;", name),
                        None,
                    )
                    .with_context_lines(3)
            }

            CompileError::UndefinedFunction { name, span } => {
                let mut diag = Diagnostic::error(format!("Undefined function: {}", name))
                    .with_span(span.unwrap_or(Span::dummy()))
                    .with_note("Functions must be defined before they are called");

                // Suggest common function names if they're close
                match name.as_str() {
                    "print" => {
                        diag = diag.with_suggestion(
                            "Did you mean 'println'? The print function is called 'println' in Palladium",
                            Some("println".to_string())
                        );
                    }
                    "printf" => {
                        diag = diag.with_suggestion(
                            "Palladium uses 'println' instead of 'printf'",
                            Some("println".to_string()),
                        );
                    }
                    _ => {
                        diag = diag.with_suggestion(
                            format!("Define the function first: fn {}() {{ ... }}", name),
                            None,
                        );
                    }
                }

                diag
            }

            CompileError::ArgumentCountMismatch {
                name,
                expected,
                found,
                span,
            } => {
                let mut diag = Diagnostic::error(format!(
                    "Function '{}' expects {} argument{}, but {} {} provided",
                    name,
                    expected,
                    if *expected == 1 { "" } else { "s" },
                    found,
                    if *found == 1 { "was" } else { "were" }
                ))
                .with_span(span.unwrap_or(Span::dummy()));

                if *found < *expected {
                    diag = diag.with_suggestion(
                        format!(
                            "Add {} more argument{}",
                            expected - found,
                            if expected - found == 1 { "" } else { "s" }
                        ),
                        None,
                    );
                } else {
                    diag = diag.with_suggestion(
                        format!(
                            "Remove {} argument{}",
                            found - expected,
                            if found - expected == 1 { "" } else { "s" }
                        ),
                        None,
                    );
                }

                diag
            }

            CompileError::MissingSemicolon { span } => {
                Diagnostic::error("Missing semicolon after statement")
                    .with_span(span.unwrap_or(Span::dummy()))
                    .with_note("Every statement in Palladium must end with a semicolon")
                    .with_suggestion(
                        "Add a semicolon (;) at the end of this line",
                        Some(";".to_string()),
                    )
            }

            CompileError::InvalidFunctionSignature { message, span } => Diagnostic::error(format!(
                "Invalid function signature: {}",
                message
            ))
            .with_span(span.unwrap_or(Span::dummy()))
            .with_note(
                "Function signatures must follow the pattern: fn name(param: Type) -> ReturnType",
            )
            .with_suggestion(
                "Example: fn add(x: int, y: int) -> int { return x + y; }",
                None,
            ),

            CompileError::NonExhaustiveMatch {
                missing_patterns,
                span,
            } => {
                let mut diag = Diagnostic::error("Non-exhaustive match expression")
                    .with_span(span.unwrap_or(Span::dummy()))
                    .with_note("All possible patterns must be covered in a match expression");

                if missing_patterns.len() == 1 {
                    diag = diag.with_suggestion(
                        format!("Add a pattern for: {}", missing_patterns[0]),
                        None,
                    );
                } else if missing_patterns.len() <= 3 {
                    diag = diag.with_suggestion(
                        format!("Add patterns for: {}", missing_patterns.join(", ")),
                        None,
                    );
                } else {
                    diag = diag.with_suggestion(
                        "Add remaining patterns or use a wildcard pattern (_) to match all other cases",
                        None
                    );
                }

                diag
            }

            CompileError::UnreachablePattern { patterns: _, span } => Diagnostic::error(
                "Unreachable pattern detected",
            )
            .with_span(span.unwrap_or(Span::dummy()))
            .with_note(
                "This pattern can never be matched because previous patterns cover all cases",
            )
            .with_suggestion("Remove this pattern or reorder the patterns", None),

            _ => {
                // Default diagnostic for other errors
                Diagnostic::error(self.to_string())
            }
        }
    }
}

/// A diagnostic message with source location
#[derive(Debug)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub span: Option<Span>,
    pub notes: Vec<String>,
    pub suggestions: Vec<Suggestion>,
    pub context_lines: usize, // Number of lines to show before/after the error
}

/// A suggestion for fixing an error
#[derive(Debug)]
pub struct Suggestion {
    pub message: String,
    pub replacement: Option<String>,
    pub span: Option<Span>,
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
            suggestions: Vec::new(),
            context_lines: 2,
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

    pub fn with_suggestion(
        mut self,
        message: impl Into<String>,
        replacement: Option<String>,
    ) -> Self {
        self.suggestions.push(Suggestion {
            message: message.into(),
            replacement,
            span: self.span,
        });
        self
    }

    pub fn with_context_lines(mut self, lines: usize) -> Self {
        self.context_lines = lines;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_new() {
        let span = Span::new(10, 20, 5, 3);
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 20);
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 3);
    }

    #[test]
    fn test_span_dummy() {
        let span = Span::dummy();
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 0);
        assert_eq!(span.line, 0);
        assert_eq!(span.column, 0);
    }

    #[test]
    fn test_span_extend_to() {
        let span1 = Span::new(10, 20, 5, 3);
        let span2 = Span::new(15, 25, 6, 5);
        let extended = span1.extend_to(&span2);
        
        assert_eq!(extended.start, 10);
        assert_eq!(extended.end, 25);
        assert_eq!(extended.line, 5);
        assert_eq!(extended.column, 3);
    }

    #[test]
    fn test_span_extend_to_same_line() {
        let span1 = Span::new(10, 20, 5, 10);
        let span2 = Span::new(5, 15, 5, 5);
        let extended = span1.extend_to(&span2);
        
        assert_eq!(extended.start, 5);
        assert_eq!(extended.end, 20);
        assert_eq!(extended.line, 5);
        assert_eq!(extended.column, 5);
    }

    #[test]
    fn test_compile_error_display() {
        let err = CompileError::UnexpectedChar {
            ch: '$',
            line: 10,
            col: 5,
            span: None,
        };
        assert_eq!(err.to_string(), "Unexpected character '$' at line 10, column 5");

        let err = CompileError::UnterminatedString {
            line: 42,
            span: None,
        };
        assert_eq!(err.to_string(), "Unterminated string literal at line 42");

        let err = CompileError::TypeMismatch {
            expected: "int".to_string(),
            found: "string".to_string(),
            span: None,
        };
        assert_eq!(err.to_string(), "Type mismatch: expected int, found string");
    }

    #[test]
    fn test_io_error_conversion() {
        use std::io;
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let compile_err: CompileError = io_err.into();
        assert!(matches!(compile_err, CompileError::IoError(_)));
    }

    #[test]
    fn test_diagnostic_builder() {
        let diag = Diagnostic::error("test error")
            .with_span(Span::new(0, 10, 1, 1))
            .with_note("this is a note")
            .with_suggestion("fix it like this", Some("fixed".to_string()))
            .with_context_lines(5);

        assert!(matches!(diag.level, DiagnosticLevel::Error));
        assert_eq!(diag.message, "test error");
        assert_eq!(diag.notes.len(), 1);
        assert_eq!(diag.notes[0], "this is a note");
        assert_eq!(diag.suggestions.len(), 1);
        assert_eq!(diag.suggestions[0].message, "fix it like this");
        assert_eq!(diag.suggestions[0].replacement, Some("fixed".to_string()));
        assert_eq!(diag.context_lines, 5);
    }

    #[test]
    fn test_unexpected_char_diagnostic() {
        let err = CompileError::UnexpectedChar {
            ch: '€',
            line: 10,
            col: 5,
            span: Some(Span::new(100, 101, 10, 5)),
        };
        
        let diag = err.to_diagnostic();
        assert_eq!(diag.message, "Unexpected character '€' at line 10, column 5");
        assert_eq!(diag.notes.len(), 1);
        assert_eq!(diag.suggestions.len(), 1);
        assert!(diag.span.is_some());
    }

    #[test]
    fn test_type_mismatch_suggestions() {
        // int to string
        let err = CompileError::TypeMismatch {
            expected: "string".to_string(),
            found: "int".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("to_string()")));

        // string to int
        let err = CompileError::TypeMismatch {
            expected: "int".to_string(),
            found: "string".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("parse_int()")));

        // bool suggestion
        let err = CompileError::TypeMismatch {
            expected: "bool".to_string(),
            found: "int".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("true") && s.message.contains("false")));
    }

    #[test]
    fn test_undefined_function_suggestions() {
        // print -> println
        let err = CompileError::UndefinedFunction {
            name: "print".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.replacement == Some("println".to_string())));

        // printf -> println
        let err = CompileError::UndefinedFunction {
            name: "printf".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.replacement == Some("println".to_string())));

        // generic function
        let err = CompileError::UndefinedFunction {
            name: "myFunc".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("fn myFunc()")));
    }

    #[test]
    fn test_argument_count_mismatch() {
        // Too few arguments
        let err = CompileError::ArgumentCountMismatch {
            name: "add".to_string(),
            expected: 2,
            found: 1,
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.message.contains("expects 2 arguments, but 1 was provided"));
        assert!(diag.suggestions.iter().any(|s| s.message.contains("Add 1 more argument")));

        // Too many arguments
        let err = CompileError::ArgumentCountMismatch {
            name: "print".to_string(),
            expected: 1,
            found: 3,
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.message.contains("expects 1 argument, but 3 were provided"));
        assert!(diag.suggestions.iter().any(|s| s.message.contains("Remove 2 arguments")));
    }

    #[test]
    fn test_missing_semicolon_diagnostic() {
        let err = CompileError::MissingSemicolon {
            span: Some(Span::new(50, 51, 10, 20)),
        };
        let diag = err.to_diagnostic();
        assert_eq!(diag.message, "Missing semicolon after statement");
        assert!(diag.suggestions.iter().any(|s| s.replacement == Some(";".to_string())));
    }

    #[test]
    fn test_non_exhaustive_match_suggestions() {
        // Single missing pattern
        let err = CompileError::NonExhaustiveMatch {
            missing_patterns: vec!["None".to_string()],
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("Add a pattern for: None")));

        // Multiple missing patterns (<=3)
        let err = CompileError::NonExhaustiveMatch {
            missing_patterns: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("Red, Green, Blue")));

        // Many missing patterns (>3)
        let err = CompileError::NonExhaustiveMatch {
            missing_patterns: vec!["A".to_string(), "B".to_string(), "C".to_string(), "D".to_string(), "E".to_string()],
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("wildcard pattern (_)")));
    }

    #[test]
    fn test_borrow_checker_errors() {
        let err = CompileError::UseOfMovedValue {
            name: "x".to_string(),
            span: Some(Span::new(10, 11, 5, 5)),
        };
        assert_eq!(err.to_string(), "Use of moved value: x");

        let err = CompileError::UseOfUninitializedValue {
            name: "y".to_string(),
            span: None,
        };
        assert_eq!(err.to_string(), "Use of uninitialized value: y");

        let err = CompileError::CannotMoveOutOfBorrowedContent {
            span: None,
        };
        assert_eq!(err.to_string(), "Cannot move out of borrowed content");
    }

    #[test]
    fn test_unsafe_operation_error() {
        let err = CompileError::UnsafeOperation {
            operation: "raw pointer dereference".to_string(),
            span: Span::new(0, 10, 1, 1),
        };
        assert_eq!(err.to_string(), "Unsafe operation 'raw pointer dereference' requires unsafe block");
    }

    #[test]
    fn test_generic_error() {
        let err = CompileError::Generic("Something went wrong".to_string());
        assert_eq!(err.to_string(), "Something went wrong");
        
        let diag = err.to_diagnostic();
        assert_eq!(diag.message, "Something went wrong");
    }
}
