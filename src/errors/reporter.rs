// Error reporter for Palladium
// "Making errors helpful, not scary"

use super::{Diagnostic, DiagnosticLevel, Span};
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
        // Print the main error message
        match diagnostic.level {
            DiagnosticLevel::Error => {
                eprintln!("\x1b[1;31merror\x1b[0m: {}", diagnostic.message);
            }
            DiagnosticLevel::Warning => {
                eprintln!("\x1b[1;33mwarning\x1b[0m: {}", diagnostic.message);
            }
            DiagnosticLevel::Info => {
                eprintln!("\x1b[1;36minfo\x1b[0m: {}", diagnostic.message);
            }
            DiagnosticLevel::Help => {
                eprintln!("\x1b[1;32mhelp\x1b[0m: {}", diagnostic.message);
            }
        }
        
        // Show source location if available
        if let Some(span) = diagnostic.span {
            self.show_source_snippet(span);
        }
        
        // Print notes
        for note in &diagnostic.notes {
            eprintln!("  \x1b[1;36mnote\x1b[0m: {}", note);
        }
        
        eprintln!(); // Empty line after error
    }
    
    fn show_source_snippet(&self, span: Span) {
        // Show file location
        eprintln!("  \x1b[1;34m-->\x1b[0m {}:{}:{}", 
            self.source_file, span.line, span.column);
        
        // Get the line containing the error
        let lines: Vec<&str> = self.source_content.lines().collect();
        if span.line == 0 || span.line > lines.len() {
            return;
        }
        
        let line_num = span.line;
        let line_text = lines[line_num - 1];
        
        // Show line number and content
        eprintln!("   \x1b[1;34m|\x1b[0m");
        eprintln!("{:3}\x1b[1;34m|\x1b[0m {}", line_num, line_text);
        
        // Show error indicator
        let mut indicator = String::new();
        indicator.push_str("   \x1b[1;34m|\x1b[0m ");
        
        // Add spaces to align with error position
        for _ in 0..span.column.saturating_sub(1) {
            indicator.push(' ');
        }
        
        // Add error markers
        indicator.push_str("\x1b[1;31m^");
        let error_len = (span.end - span.start).max(1);
        for _ in 1..error_len {
            indicator.push('~');
        }
        indicator.push_str("\x1b[0m");
        
        eprintln!("{}", indicator);
    }
}

/// Helper to create common diagnostics
pub struct DiagnosticBuilder;

impl DiagnosticBuilder {
    pub fn type_mismatch(expected: &str, found: &str, span: Span) -> Diagnostic {
        Diagnostic::error(format!("type mismatch: expected {}, found {}", expected, found))
            .with_span(span)
            .with_note("types must match exactly")
    }
    
    pub fn undefined_variable(name: &str, span: Span) -> Diagnostic {
        Diagnostic::error(format!("undefined variable: {}", name))
            .with_span(span)
            .with_note(format!("did you mean to declare it with 'let {} = ...'?", name))
    }
    
    pub fn missing_semicolon(span: Span) -> Diagnostic {
        Diagnostic::error("expected ';' after statement")
            .with_span(span)
            .with_note("each statement must end with a semicolon")
    }
    
    pub fn wrong_arg_count(func: &str, expected: usize, found: usize, span: Span) -> Diagnostic {
        let msg = if expected == 1 {
            format!("function '{}' expects {} argument, but {} were provided", func, expected, found)
        } else {
            format!("function '{}' expects {} arguments, but {} were provided", func, expected, found)
        };
        
        Diagnostic::error(msg)
            .with_span(span)
            .with_note(format!("function signature: {}(...)", func))
    }
}