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
