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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Function, StructDef, EnumDef, TraitDef, Item, Visibility};
    use crate::errors::Span;
    
    

    fn create_test_server() -> LanguageServer {
        let mut server = LanguageServer::new();
        server.initialize(None).unwrap();
        server
    }

    #[test]
    fn test_diagnostic_creation() {
        let diagnostic = Diagnostic {
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 5 },
            },
            severity: DiagnosticSeverity::Error,
            code: Some("E0001".to_string()),
            source: Some("palladium".to_string()),
            message: "Test error".to_string(),
            related_information: Vec::new(),
        };

        assert_eq!(diagnostic.message, "Test error");
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.code, Some("E0001".to_string()));
    }

    #[test]
    fn test_run_diagnostics_no_document() {
        let mut server = create_test_server();
        let diagnostics = server.run_diagnostics("file:///nonexistent.pd");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_run_diagnostics_syntax_error() {
        let mut server = create_test_server();
        
        // Add document with syntax error
        server.open_document(
            "file:///test.pd".to_string(),
            1,
            "fn main() {".to_string() // Missing closing brace
        ).unwrap();

        let diagnostics = server.run_diagnostics("file:///test.pd");
        assert!(!diagnostics.is_empty());
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
        assert!(diagnostics[0].message.contains("expected"));
    }

    #[test]
    fn test_compile_error_to_diagnostic_syntax_error() {
        let server = create_test_server();
        let error = CompileError::SyntaxError {
            message: "unexpected token".to_string(),
            span: Some(Span::new(10, 15, 0, 0)),
        };

        let diagnostic = server.compile_error_to_diagnostic(error);
        assert_eq!(diagnostic.message, "unexpected token");
        assert_eq!(diagnostic.code, Some("E0001".to_string()));
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    }

    #[test]
    fn test_compile_error_to_diagnostic_type_mismatch() {
        let server = create_test_server();
        let error = CompileError::TypeMismatch {
            expected: "i32".to_string(),
            found: "String".to_string(),
            span: Some(Span::new(20, 25, 0, 0)),
        };

        let diagnostic = server.compile_error_to_diagnostic(error);
        assert!(diagnostic.message.contains("Type mismatch"));
        assert!(diagnostic.message.contains("expected i32"));
        assert!(diagnostic.message.contains("found String"));
        assert_eq!(diagnostic.code, Some("E0002".to_string()));
    }

    #[test]
    fn test_compile_error_to_diagnostic_undefined_variable() {
        let server = create_test_server();
        let error = CompileError::UndefinedVariable {
            name: "foo".to_string(),
            span: Some(Span::new(5, 8, 0, 0)),
        };

        let diagnostic = server.compile_error_to_diagnostic(error);
        assert!(diagnostic.message.contains("Undefined variable"));
        assert!(diagnostic.message.contains("foo"));
        assert_eq!(diagnostic.code, Some("E0003".to_string()));
    }

    #[test]
    fn test_compile_error_to_diagnostic_undefined_function() {
        let server = create_test_server();
        let error = CompileError::UndefinedFunction {
            name: "bar".to_string(),
            span: Some(Span::new(5, 8, 0, 0)),
        };

        let diagnostic = server.compile_error_to_diagnostic(error);
        assert_eq!(diagnostic.code, Some("E0004".to_string()));
    }

    #[test]
    fn test_compile_error_to_diagnostic_generic() {
        let server = create_test_server();
        let error = CompileError::Generic("Something went wrong".to_string());

        let diagnostic = server.compile_error_to_diagnostic(error);
        assert_eq!(diagnostic.message, "Something went wrong");
        assert_eq!(diagnostic.code, Some("E0000".to_string()));
    }

    #[test]
    fn test_compile_error_to_diagnostic_no_span() {
        let server = create_test_server();
        let error = CompileError::SyntaxError {
            message: "error without location".to_string(),
            span: None,
        };

        let diagnostic = server.compile_error_to_diagnostic(error);
        assert_eq!(diagnostic.range.start.line, 0);
        assert_eq!(diagnostic.range.start.character, 0);
        assert_eq!(diagnostic.range.end.line, 0);
        assert_eq!(diagnostic.range.end.character, 0);
    }

    #[test]
    fn test_error_code_mapping() {
        let server = create_test_server();
        
        let syntax_err = CompileError::SyntaxError {
            message: "test".to_string(),
            span: None,
        };
        assert_eq!(server.error_code(&syntax_err), "E0001");

        let type_err = CompileError::TypeMismatch {
            expected: "a".to_string(),
            found: "b".to_string(),
            span: None,
        };
        assert_eq!(server.error_code(&type_err), "E0002");

        let undef_var = CompileError::UndefinedVariable {
            name: "x".to_string(),
            span: None,
        };
        assert_eq!(server.error_code(&undef_var), "E0003");

        let undef_func = CompileError::UndefinedFunction {
            name: "f".to_string(),
            span: None,
        };
        assert_eq!(server.error_code(&undef_func), "E0004");

        let generic = CompileError::Generic("test".to_string());
        assert_eq!(server.error_code(&generic), "E0000");
    }

    #[test]
    fn test_check_naming_conventions_function() {
        let server = create_test_server();
        
        // Test function with bad naming
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Function(Function {
                    name: "BadName".to_string(), // Should be snake_case
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
                }),
            ],
        };

        let diagnostics = server.check_naming_conventions(&ast);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostics[0].code, Some("W0001".to_string()));
        assert!(diagnostics[0].message.contains("snake_case"));
    }

    #[test]
    fn test_check_naming_conventions_struct() {
        let server = create_test_server();
        
        // Test struct with bad naming
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Struct(StructDef {
                    name: "bad_struct".to_string(), // Should be PascalCase
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    fields: vec![],
                    visibility: Visibility::Private,
                    span: Span::new(0, 10, 0, 0),
                }),
            ],
        };

        let diagnostics = server.check_naming_conventions(&ast);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostics[0].code, Some("W0002".to_string()));
        assert!(diagnostics[0].message.contains("PascalCase"));
    }

    #[test]
    fn test_check_naming_conventions_enum() {
        let server = create_test_server();
        
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Enum(EnumDef {
                    name: "bad_enum".to_string(), // Should be PascalCase
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    variants: vec![],
                    span: Span::new(0, 10, 0, 0),
                }),
            ],
        };

        let diagnostics = server.check_naming_conventions(&ast);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostics[0].code, Some("W0003".to_string()));
        assert!(diagnostics[0].message.contains("PascalCase"));
    }

    #[test]
    fn test_check_naming_conventions_trait() {
        let server = create_test_server();
        
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Trait(TraitDef {
                    name: "bad_trait".to_string(), // Should be PascalCase
                    lifetime_params: vec![],
                    type_params: vec![],
                    methods: vec![],
                    visibility: Visibility::Private,
                    span: Span::new(0, 10, 0, 0),
                }),
            ],
        };

        let diagnostics = server.check_naming_conventions(&ast);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostics[0].code, Some("W0004".to_string()));
        assert!(diagnostics[0].message.contains("PascalCase"));
    }

    #[test]
    fn test_check_naming_conventions_good_names() {
        let server = create_test_server();
        
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Function(Function {
                    name: "good_function_name".to_string(),
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
                }),
                Item::Struct(StructDef {
                    name: "GoodStructName".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    fields: vec![],
                    visibility: Visibility::Private,
                    span: Span::new(0, 10, 0, 0),
                }),
                Item::Enum(EnumDef {
                    name: "GoodEnumName".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    variants: vec![],
                    span: Span::new(0, 10, 0, 0),
                }),
                Item::Trait(TraitDef {
                    name: "GoodTraitName".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    methods: vec![],
                    visibility: Visibility::Private,
                    span: Span::new(0, 10, 0, 0),
                }),
            ],
        };

        let diagnostics = server.check_naming_conventions(&ast);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_is_snake_case() {
        assert!(is_snake_case("hello_world"));
        assert!(is_snake_case("test_123"));
        assert!(is_snake_case("a"));
        assert!(is_snake_case("_private"));
        
        assert!(!is_snake_case("HelloWorld"));
        assert!(!is_snake_case("Test"));
        assert!(!is_snake_case(""));
        assert!(!is_snake_case("SCREAMING"));
    }

    #[test]
    fn test_is_pascal_case() {
        assert!(is_pascal_case("HelloWorld"));
        assert!(is_pascal_case("Test"));
        assert!(is_pascal_case("MyStruct123"));
        assert!(is_pascal_case("A"));
        
        assert!(!is_pascal_case("hello_world"));
        assert!(!is_pascal_case("test"));
        assert!(!is_pascal_case(""));
        assert!(!is_pascal_case("123Test"));
    }

    #[test]
    fn test_is_screaming_snake_case() {
        assert!(is_screaming_snake_case("HELLO_WORLD"));
        assert!(is_screaming_snake_case("TEST_123"));
        assert!(is_screaming_snake_case("A"));
        assert!(is_screaming_snake_case("_PRIVATE"));
        
        assert!(!is_screaming_snake_case("hello_world"));
        assert!(!is_screaming_snake_case("Test"));
        assert!(!is_screaming_snake_case(""));
        assert!(!is_screaming_snake_case("HelloWorld"));
    }

    #[test]
    fn test_check_unused_variables() {
        let server = create_test_server();
        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        
        // Currently returns empty (TODO)
        let diagnostics = server.check_unused_variables(&ast);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_check_unreachable_code() {
        let server = create_test_server();
        let ast = Program {
            imports: vec![],
            items: vec![],
        };
        
        // Currently returns empty (TODO)
        let diagnostics = server.check_unreachable_code(&ast);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_diagnostic_source() {
        let diagnostic = Diagnostic {
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 5 },
            },
            severity: DiagnosticSeverity::Error,
            code: Some("E0001".to_string()),
            source: Some("palladium".to_string()),
            message: "Test".to_string(),
            related_information: Vec::new(),
        };

        assert_eq!(diagnostic.source, Some("palladium".to_string()));
    }

    #[test]
    fn test_multiple_diagnostics() {
        let server = create_test_server();
        
        // Create AST with multiple naming issues
        let ast = Program {
            imports: vec![],
            items: vec![
                Item::Function(Function {
                    name: "BadFunction".to_string(),
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
                }),
                Item::Struct(StructDef {
                    name: "bad_struct".to_string(),
                    lifetime_params: vec![],
                    type_params: vec![],
                    const_params: vec![],
                    fields: vec![],
                    visibility: Visibility::Private,
                    span: Span::new(20, 30, 0, 0),
                }),
            ],
        };

        let diagnostics = server.check_naming_conventions(&ast);
        assert_eq!(diagnostics.len(), 2);
        
        // Check that we got both warnings
        let codes: Vec<_> = diagnostics.iter()
            .filter_map(|d| d.code.as_ref())
            .collect();
        assert!(codes.contains(&&"W0001".to_string()));
        assert!(codes.contains(&&"W0002".to_string()));
    }
}
