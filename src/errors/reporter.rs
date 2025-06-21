// Error reporter for Palladium
// "Making errors helpful, not scary"

use super::{Diagnostic, DiagnosticLevel, Span, Suggestion};
use std::cmp::{max, min};
use std::fs;

pub struct ErrorReporter {
    source_file: String,
    source_content: String,
}

impl ErrorReporter {
    pub fn new(source_file: String) -> std::io::Result<Self> {
        let source_content = fs::read_to_string(&source_file)?;
        Ok(Self {
            source_file,
            source_content,
        })
    }

    pub fn report(&self, diagnostic: &Diagnostic) {
        // Print header with error level and message
        let (level_color, level_text) = match diagnostic.level {
            DiagnosticLevel::Error => ("31", "error"),
            DiagnosticLevel::Warning => ("33", "warning"),
            DiagnosticLevel::Info => ("36", "info"),
            DiagnosticLevel::Help => ("32", "help"),
        };

        eprintln!(
            "\x1b[1;{}m{}\x1b[0m\x1b[1m: {}\x1b[0m",
            level_color, level_text, diagnostic.message
        );

        // Show source location if available
        if let Some(span) = diagnostic.span {
            self.show_source_snippet_with_context(span, diagnostic.context_lines);
        }

        // Print notes
        for note in &diagnostic.notes {
            eprintln!("\x1b[1;36m  = note:\x1b[0m {}", note);
        }

        // Print suggestions
        for suggestion in &diagnostic.suggestions {
            self.show_suggestion(suggestion);
        }

        eprintln!(); // Empty line after error
    }

    #[allow(dead_code)]
    fn show_source_snippet(&self, span: Span) {
        self.show_source_snippet_with_context(span, 0);
    }

    fn show_source_snippet_with_context(&self, span: Span, context_lines: usize) {
        // Show file location with better formatting
        eprintln!(
            "\x1b[1;34m  --> \x1b[0m{}:{}:{}",
            self.source_file, span.line, span.column
        );

        let lines: Vec<&str> = self.source_content.lines().collect();
        if span.line == 0 || span.line > lines.len() {
            return;
        }

        // Calculate range of lines to show
        let start_line = max(1, span.line.saturating_sub(context_lines));
        let end_line = min(lines.len(), span.line + context_lines);

        // Find the width needed for line numbers
        let line_num_width = end_line.to_string().len();

        eprintln!("{}\x1b[1;34m |\x1b[0m", " ".repeat(line_num_width));

        // Show lines with context
        for line_num in start_line..=end_line {
            let line_text = lines[line_num - 1];

            if line_num == span.line {
                // Highlight the error line
                eprintln!(
                    "\x1b[1;34m{:>width$} |\x1b[0m {}",
                    line_num,
                    line_text,
                    width = line_num_width
                );

                // Show error indicator
                let mut indicator = String::new();
                indicator.push_str(&" ".repeat(line_num_width));
                indicator.push_str(" \x1b[1;34m|\x1b[0m ");

                // Add spaces to align with error position
                for (i, ch) in line_text.chars().enumerate() {
                    if i < span.column.saturating_sub(1) {
                        if ch == '\t' {
                            indicator.push('\t');
                        } else {
                            indicator.push(' ');
                        }
                    } else {
                        break;
                    }
                }

                // Add error markers with better visibility
                indicator.push_str("\x1b[1;31m^");
                let error_len = if span.end > span.start {
                    span.end - span.start
                } else {
                    // Try to highlight the whole token if we can
                    self.estimate_token_length(&line_text[span.column.saturating_sub(1)..])
                        .max(1)
                };

                for _ in 1..error_len {
                    indicator.push('~');
                }
                indicator.push_str("\x1b[0m");

                eprintln!("{}", indicator);
            } else {
                // Context lines in dimmed color
                eprintln!(
                    "\x1b[2;34m{:>width$} |\x1b[0m\x1b[2m {}\x1b[0m",
                    line_num,
                    line_text,
                    width = line_num_width
                );
            }
        }

        eprintln!("{}\x1b[1;34m |\x1b[0m", " ".repeat(line_num_width));
    }

    fn show_suggestion(&self, suggestion: &Suggestion) {
        eprintln!("\x1b[1;32m  = help:\x1b[0m {}", suggestion.message);

        if let Some(ref replacement) = suggestion.replacement {
            if let Some(span) = suggestion.span {
                // Show the suggested fix inline
                let lines: Vec<&str> = self.source_content.lines().collect();
                if span.line > 0 && span.line <= lines.len() {
                    let line = lines[span.line - 1];
                    let mut fixed_line = String::new();

                    // Build the fixed line
                    if span.column > 1 {
                        fixed_line.push_str(&line[..span.column - 1]);
                    }
                    fixed_line.push_str(replacement);
                    if span.end < line.len() {
                        fixed_line.push_str(&line[span.end..]);
                    }

                    eprintln!("\x1b[1;32m         Suggested fix:\x1b[0m");
                    eprintln!("\x1b[32m         {}\x1b[0m", fixed_line);
                }
            }
        }
    }

    fn estimate_token_length(&self, text: &str) -> usize {
        // Estimate the length of the token at the beginning of the text
        let mut len = 0;
        for ch in text.chars() {
            if ch.is_alphanumeric() || ch == '_' {
                len += 1;
            } else if len > 0 {
                break;
            } else if !ch.is_whitespace() {
                // Single character token
                return 1;
            }
        }
        len
    }
}

/// Helper to create common diagnostics
pub struct DiagnosticBuilder;

impl DiagnosticBuilder {
    pub fn type_mismatch(expected: &str, found: &str, span: Span) -> Diagnostic {
        Diagnostic::error(format!(
            "type mismatch: expected {}, found {}",
            expected, found
        ))
        .with_span(span)
        .with_note("types must match exactly")
        .with_context_lines(2)
    }

    pub fn undefined_variable(name: &str, span: Span) -> Diagnostic {
        Diagnostic::error(format!("undefined variable: {}", name))
            .with_span(span)
            .with_note("variables must be declared before use")
            .with_suggestion(
                format!("did you mean to declare it? Try: let {} = ...;", name),
                None,
            )
            .with_context_lines(3)
    }

    pub fn missing_semicolon(span: Span) -> Diagnostic {
        Diagnostic::error("expected ';' after statement")
            .with_span(span)
            .with_note("each statement must end with a semicolon")
            .with_suggestion(
                "add a semicolon at the end of this line",
                Some(";".to_string()),
            )
    }

    pub fn wrong_arg_count(func: &str, expected: usize, found: usize, span: Span) -> Diagnostic {
        let msg = if expected == 1 {
            format!(
                "function '{}' expects {} argument, but {} were provided",
                func, expected, found
            )
        } else {
            format!(
                "function '{}' expects {} arguments, but {} were provided",
                func, expected, found
            )
        };

        let mut diag = Diagnostic::error(msg)
            .with_span(span)
            .with_note(format!("function signature: {}(...)", func))
            .with_context_lines(2);

        if found < expected {
            diag = diag.with_suggestion(
                format!(
                    "add {} more argument{}",
                    expected - found,
                    if expected - found == 1 { "" } else { "s" }
                ),
                None,
            );
        } else {
            diag = diag.with_suggestion(
                format!(
                    "remove {} argument{}",
                    found - expected,
                    if found - expected == 1 { "" } else { "s" }
                ),
                None,
            );
        }

        diag
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        file
    }

    #[test]
    fn test_error_reporter_new() {
        let file = create_temp_file("test content\nline 2\nline 3");
        let reporter = ErrorReporter::new(file.path().to_str().unwrap().to_string()).unwrap();
        
        assert_eq!(reporter.source_file, file.path().to_str().unwrap());
        assert_eq!(reporter.source_content, "test content\nline 2\nline 3");
    }

    #[test]
    fn test_error_reporter_new_file_not_found() {
        let result = ErrorReporter::new("nonexistent_file.pd".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_estimate_token_length() {
        let file = create_temp_file("");
        let reporter = ErrorReporter::new(file.path().to_str().unwrap().to_string()).unwrap();
        
        // Alphanumeric tokens
        assert_eq!(reporter.estimate_token_length("variable123 + other"), 11);
        assert_eq!(reporter.estimate_token_length("fn_name()"), 7);
        assert_eq!(reporter.estimate_token_length("_underscore"), 11);
        
        // Single character tokens
        assert_eq!(reporter.estimate_token_length("+ other"), 1);
        assert_eq!(reporter.estimate_token_length("( expr"), 1);
        assert_eq!(reporter.estimate_token_length("; next"), 1);
        
        // Empty or whitespace
        assert_eq!(reporter.estimate_token_length(""), 0);
        assert_eq!(reporter.estimate_token_length("   "), 0);
    }

    #[test]
    fn test_show_source_snippet_with_context() {
        // Note: Since report() prints to stderr, we can't easily capture its output
        // in tests. Instead, we'll test the helper methods and trust the integration.
        let file = create_temp_file("line 1\nline 2\nline 3\nline 4\nline 5");
        let reporter = ErrorReporter::new(file.path().to_str().unwrap().to_string()).unwrap();
        
        // Test valid span
        let span = Span::new(0, 5, 3, 1);
        // This would normally print to stderr
        reporter.show_source_snippet_with_context(span, 1);
        
        // Test edge cases
        let span = Span::new(0, 5, 0, 1); // line 0 (invalid)
        reporter.show_source_snippet_with_context(span, 0);
        
        let span = Span::new(0, 5, 100, 1); // line too large
        reporter.show_source_snippet_with_context(span, 0);
    }

    #[test]
    fn test_diagnostic_builder_type_mismatch() {
        let span = Span::new(10, 20, 5, 5);
        let diag = DiagnosticBuilder::type_mismatch("int", "string", span);
        
        assert_eq!(diag.message, "type mismatch: expected int, found string");
        assert_eq!(diag.notes.len(), 1);
        assert_eq!(diag.notes[0], "types must match exactly");
        assert_eq!(diag.context_lines, 2);
        assert!(diag.span.is_some());
    }

    #[test]
    fn test_diagnostic_builder_undefined_variable() {
        let span = Span::new(15, 25, 10, 8);
        let diag = DiagnosticBuilder::undefined_variable("myVar", span);
        
        assert_eq!(diag.message, "undefined variable: myVar");
        assert_eq!(diag.notes.len(), 1);
        assert_eq!(diag.notes[0], "variables must be declared before use");
        assert_eq!(diag.suggestions.len(), 1);
        assert!(diag.suggestions[0].message.contains("let myVar = ..."));
        assert_eq!(diag.context_lines, 3);
    }

    #[test]
    fn test_diagnostic_builder_missing_semicolon() {
        let span = Span::new(30, 31, 15, 20);
        let diag = DiagnosticBuilder::missing_semicolon(span);
        
        assert_eq!(diag.message, "expected ';' after statement");
        assert_eq!(diag.notes.len(), 1);
        assert_eq!(diag.notes[0], "each statement must end with a semicolon");
        assert_eq!(diag.suggestions.len(), 1);
        assert_eq!(diag.suggestions[0].message, "add a semicolon at the end of this line");
        assert_eq!(diag.suggestions[0].replacement, Some(";".to_string()));
    }

    #[test]
    fn test_diagnostic_builder_wrong_arg_count_single() {
        let span = Span::new(40, 50, 20, 10);
        
        // Expected 1, found 0
        let diag = DiagnosticBuilder::wrong_arg_count("print", 1, 0, span);
        assert!(diag.message.contains("expects 1 argument, but 0 were provided"));
        assert!(diag.suggestions[0].message.contains("add 1 more argument"));
        
        // Expected 1, found 2
        let diag = DiagnosticBuilder::wrong_arg_count("print", 1, 2, span);
        assert!(diag.message.contains("expects 1 argument, but 2 were provided"));
        assert!(diag.suggestions[0].message.contains("remove 1 argument"));
    }

    #[test]
    fn test_diagnostic_builder_wrong_arg_count_multiple() {
        let span = Span::new(40, 50, 20, 10);
        
        // Expected 3, found 1
        let diag = DiagnosticBuilder::wrong_arg_count("add3", 3, 1, span);
        // Message says "were" even with 1 because expected is plural
        assert!(diag.message.contains("expects 3 arguments, but 1 were provided"));
        assert!(diag.suggestions[0].message.contains("add 2 more arguments"));
        
        // Expected 2, found 5
        let diag = DiagnosticBuilder::wrong_arg_count("add", 2, 5, span);
        assert!(diag.message.contains("expects 2 arguments, but 5 were provided"));
        assert!(diag.suggestions[0].message.contains("remove 3 arguments"));
    }

    #[test]
    fn test_show_suggestion_with_replacement() {
        let file = create_temp_file("let x = 42\nprint(x)\nlet y = x + 1");
        let reporter = ErrorReporter::new(file.path().to_str().unwrap().to_string()).unwrap();
        
        let suggestion = Suggestion {
            message: "Use println instead".to_string(),
            replacement: Some("println".to_string()),
            span: Some(Span::new(11, 16, 2, 1)), // "print" on line 2
        };
        
        // This would print to stderr
        reporter.show_suggestion(&suggestion);
    }

    #[test]
    fn test_show_suggestion_without_replacement() {
        let file = create_temp_file("test");
        let reporter = ErrorReporter::new(file.path().to_str().unwrap().to_string()).unwrap();
        
        let suggestion = Suggestion {
            message: "Consider adding type annotations".to_string(),
            replacement: None,
            span: None,
        };
        
        // This would print to stderr
        reporter.show_suggestion(&suggestion);
    }

    #[test]
    fn test_report_full_diagnostic() {
        let file = create_temp_file("fn main() {\n    let x = 42\n    println(x);\n}");
        let reporter = ErrorReporter::new(file.path().to_str().unwrap().to_string()).unwrap();
        
        let diagnostic = Diagnostic {
            level: DiagnosticLevel::Error,
            message: "Missing semicolon".to_string(),
            span: Some(Span::new(26, 27, 2, 14)),
            notes: vec!["Each statement must end with a semicolon".to_string()],
            suggestions: vec![Suggestion {
                message: "Add a semicolon here".to_string(),
                replacement: Some(";".to_string()),
                span: Some(Span::new(26, 27, 2, 14)),
            }],
            context_lines: 1,
        };
        
        // This would print a full diagnostic to stderr
        reporter.report(&diagnostic);
    }

    #[test]
    fn test_report_diagnostic_without_span() {
        let file = create_temp_file("test");
        let reporter = ErrorReporter::new(file.path().to_str().unwrap().to_string()).unwrap();
        
        let diagnostic = Diagnostic {
            level: DiagnosticLevel::Warning,
            message: "Generic warning".to_string(),
            span: None,
            notes: vec!["This is a note".to_string()],
            suggestions: vec![],
            context_lines: 0,
        };
        
        // Should handle missing span gracefully
        reporter.report(&diagnostic);
    }

    #[test]
    fn test_show_source_snippet_edge_cases() {
        let file = create_temp_file("single line");
        let reporter = ErrorReporter::new(file.path().to_str().unwrap().to_string()).unwrap();
        
        // Test with context lines greater than file size
        let span = Span::new(0, 6, 1, 1);
        reporter.show_source_snippet_with_context(span, 10);
        
        // Test at boundaries
        let file = create_temp_file("line 1\nline 2\nline 3");
        let reporter = ErrorReporter::new(file.path().to_str().unwrap().to_string()).unwrap();
        
        // First line with context
        let span = Span::new(0, 5, 1, 1);
        reporter.show_source_snippet_with_context(span, 2);
        
        // Last line with context
        let span = Span::new(14, 19, 3, 1);
        reporter.show_source_snippet_with_context(span, 2);
    }

    #[test]
    fn test_diagnostic_levels() {
        let file = create_temp_file("test");
        let reporter = ErrorReporter::new(file.path().to_str().unwrap().to_string()).unwrap();
        
        // Test each diagnostic level
        for level in [DiagnosticLevel::Error, DiagnosticLevel::Warning, 
                      DiagnosticLevel::Info, DiagnosticLevel::Help] {
            let diagnostic = Diagnostic {
                level,
                message: format!("Test {:?} message", level),
                span: None,
                notes: vec![],
                suggestions: vec![],
                context_lines: 0,
            };
            
            reporter.report(&diagnostic);
        }
    }

    #[test]
    fn test_tab_handling_in_source() {
        let file = create_temp_file("fn main() {\n\tlet x = 42;\n\t\tprintln(x);\n}");
        let reporter = ErrorReporter::new(file.path().to_str().unwrap().to_string()).unwrap();
        
        // Span pointing to "println" on line 3 with tabs
        let span = Span::new(24, 31, 3, 3);
        reporter.show_source_snippet_with_context(span, 0);
    }
}
