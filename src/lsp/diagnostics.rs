// Diagnostics provider for Palladium LSP
// "Catching errors before they catch you"

use super::{Diagnostic, DiagnosticSeverity, LanguageServer, Position, Range};
use crate::ast::Program;
use crate::errors::CompileError;

impl LanguageServer {
    /// Run diagnostics on a document
    pub fn run_diagnostics(&mut self, uri: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        let doc = match self.documents.get(uri) {
            Some(doc) => doc.clone(),
            None => return diagnostics,
        };

        // Parse errors
        match self.parse_document(&doc.content) {
            Ok(ast) => {
                // Type checking errors
                match self.typecheck_document(&ast) {
                    Ok(_) => {
                        // Additional semantic checks
                        diagnostics.extend(self.check_unused_variables(&ast));
                        diagnostics.extend(self.check_unreachable_code(&ast));
                        diagnostics.extend(self.check_naming_conventions(&ast));
                    }
                    Err(e) => {
                        diagnostics.push(self.compile_error_to_diagnostic(e));
                    }
                }
            }
            Err(e) => {
                diagnostics.push(self.compile_error_to_diagnostic(e));
            }
        }

        diagnostics
    }

    /// Convert compile error to diagnostic
    fn compile_error_to_diagnostic(&self, error: CompileError) -> Diagnostic {
        let (message, span) = match &error {
            CompileError::SyntaxError { message, span } => (message.clone(), span.as_ref()),
            CompileError::TypeMismatch {
                expected,
                found,
                span,
            } => (
                format!("Type mismatch: expected {}, found {}", expected, found),
                span.as_ref(),
            ),
            CompileError::UndefinedVariable { name, span } => {
                (format!("Undefined variable: {}", name), span.as_ref())
            }
            CompileError::Generic(msg) => (msg.clone(), None),
            _ => (error.to_string(), None),
        };

        let range = if let Some(span) = span {
            self.span_to_range(*span)
        } else {
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            }
        };

        Diagnostic {
            range,
            severity: DiagnosticSeverity::Error,
            code: Some(self.error_code(&error)),
            source: Some("palladium".to_string()),
            message,
            related_information: Vec::new(),
        }
    }

    /// Get error code for compile error
    fn error_code(&self, error: &CompileError) -> String {
        match error {
            CompileError::SyntaxError { .. } => "E0001".to_string(),
            CompileError::TypeMismatch { .. } => "E0002".to_string(),
            CompileError::UndefinedVariable { .. } => "E0003".to_string(),
            CompileError::UndefinedFunction { .. } => "E0004".to_string(),
            CompileError::Generic(_) => "E0000".to_string(),
            _ => "E9999".to_string(),
        }
    }

    /// Check for unused variables
    fn check_unused_variables(&self, _ast: &Program) -> Vec<Diagnostic> {
        // TODO: Implement unused variable detection
        Vec::new()
    }

    /// Check for unreachable code
    fn check_unreachable_code(&self, _ast: &Program) -> Vec<Diagnostic> {
        // TODO: Implement unreachable code detection
        Vec::new()
    }

    /// Check naming conventions
    fn check_naming_conventions(&self, ast: &Program) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for item in &ast.items {
            match item {
                crate::ast::Item::Function(func) => {
                    if !is_snake_case(&func.name) {
                        diagnostics.push(Diagnostic {
                            range: self.span_to_range(func.span),
                            severity: DiagnosticSeverity::Warning,
                            code: Some("W0001".to_string()),
                            source: Some("palladium".to_string()),
                            message: format!(
                                "Function name '{}' should be in snake_case",
                                func.name
                            ),
                            related_information: Vec::new(),
                        });
                    }
                }
                crate::ast::Item::Struct(struct_def) => {
                    if !is_pascal_case(&struct_def.name) {
                        diagnostics.push(Diagnostic {
                            range: self.span_to_range(struct_def.span),
                            severity: DiagnosticSeverity::Warning,
                            code: Some("W0002".to_string()),
                            source: Some("palladium".to_string()),
                            message: format!(
                                "Struct name '{}' should be in PascalCase",
                                struct_def.name
                            ),
                            related_information: Vec::new(),
                        });
                    }
                }
                crate::ast::Item::Enum(enum_def) => {
                    if !is_pascal_case(&enum_def.name) {
                        diagnostics.push(Diagnostic {
                            range: self.span_to_range(enum_def.span),
                            severity: DiagnosticSeverity::Warning,
                            code: Some("W0003".to_string()),
                            source: Some("palladium".to_string()),
                            message: format!(
                                "Enum name '{}' should be in PascalCase",
                                enum_def.name
                            ),
                            related_information: Vec::new(),
                        });
                    }
                }
                crate::ast::Item::Trait(trait_def) => {
                    if !is_pascal_case(&trait_def.name) {
                        diagnostics.push(Diagnostic {
                            range: self.span_to_range(trait_def.span),
                            severity: DiagnosticSeverity::Warning,
                            code: Some("W0004".to_string()),
                            source: Some("palladium".to_string()),
                            message: format!(
                                "Trait name '{}' should be in PascalCase",
                                trait_def.name
                            ),
                            related_information: Vec::new(),
                        });
                    }
                }
                _ => {}
            }
        }

        diagnostics
    }
}

/// Check if string is snake_case
fn is_snake_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_lowercase() || c.is_numeric() || c == '_')
}

/// Check if string is PascalCase
fn is_pascal_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().unwrap().is_uppercase()
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Check if string is SCREAMING_SNAKE_CASE
#[allow(dead_code)]
fn is_screaming_snake_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_uppercase() || c.is_numeric() || c == '_')
}
