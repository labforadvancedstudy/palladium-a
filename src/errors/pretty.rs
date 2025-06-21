// Pretty error formatting for Palladium
// "Making errors beautiful and informative"

use super::{Diagnostic, DiagnosticLevel};
use std::fmt::Write;

/// ANSI color codes for terminal output
pub mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const UNDERLINE: &str = "\x1b[4m";

    // Foreground colors
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";

    // Bright colors
    pub const BRIGHT_RED: &str = "\x1b[91m";
    pub const BRIGHT_GREEN: &str = "\x1b[92m";
    pub const BRIGHT_YELLOW: &str = "\x1b[93m";
    pub const BRIGHT_BLUE: &str = "\x1b[94m";
    pub const BRIGHT_MAGENTA: &str = "\x1b[95m";
    pub const BRIGHT_CYAN: &str = "\x1b[96m";
}

/// Style configuration for error output
pub struct ErrorStyle {
    pub use_color: bool,
    pub use_unicode: bool,
    pub compact: bool,
}

impl Default for ErrorStyle {
    fn default() -> Self {
        Self {
            use_color: true,
            use_unicode: true,
            compact: false,
        }
    }
}

impl ErrorStyle {
    /// Get the appropriate error level styling
    pub fn level_style(&self, level: DiagnosticLevel) -> (&'static str, String, &'static str) {
        if !self.use_color {
            return match level {
                DiagnosticLevel::Error => ("error", String::new(), ""),
                DiagnosticLevel::Warning => ("warning", String::new(), ""),
                DiagnosticLevel::Info => ("info", String::new(), ""),
                DiagnosticLevel::Help => ("help", String::new(), ""),
            };
        }

        match level {
            DiagnosticLevel::Error => (
                "error",
                format!("{}{}", colors::BOLD, colors::BRIGHT_RED),
                colors::RESET,
            ),
            DiagnosticLevel::Warning => (
                "warning",
                format!("{}{}", colors::BOLD, colors::BRIGHT_YELLOW),
                colors::RESET,
            ),
            DiagnosticLevel::Info => (
                "info",
                format!("{}{}", colors::BOLD, colors::BRIGHT_CYAN),
                colors::RESET,
            ),
            DiagnosticLevel::Help => (
                "help",
                format!("{}{}", colors::BOLD, colors::BRIGHT_GREEN),
                colors::RESET,
            ),
        }
    }

    /// Get the style for file paths
    pub fn path_style(&self) -> (String, &'static str) {
        if self.use_color {
            (format!("{}{}", colors::BOLD, colors::BLUE), colors::RESET)
        } else {
            (String::new(), "")
        }
    }

    /// Get the style for line numbers
    pub fn line_number_style(&self) -> (&'static str, &'static str) {
        if self.use_color {
            (colors::BLUE, colors::RESET)
        } else {
            ("", "")
        }
    }

    /// Get the style for error underlining
    pub fn error_style(&self) -> (String, &'static str) {
        if self.use_color {
            (
                format!("{}{}", colors::BOLD, colors::BRIGHT_RED),
                colors::RESET,
            )
        } else {
            (String::new(), "")
        }
    }

    /// Get the style for notes
    pub fn note_style(&self) -> (String, &'static str) {
        if self.use_color {
            (
                format!("{}{}", colors::BOLD, colors::BRIGHT_CYAN),
                colors::RESET,
            )
        } else {
            (String::new(), "")
        }
    }

    /// Get the style for suggestions
    pub fn suggestion_style(&self) -> (String, &'static str) {
        if self.use_color {
            (
                format!("{}{}", colors::BOLD, colors::BRIGHT_GREEN),
                colors::RESET,
            )
        } else {
            (String::new(), "")
        }
    }

    /// Get the style for dimmed context
    pub fn dim_style(&self) -> (&'static str, &'static str) {
        if self.use_color {
            (colors::DIM, colors::RESET)
        } else {
            ("", "")
        }
    }

    /// Get unicode or ASCII characters for drawing
    pub fn get_chars(&self) -> DrawingChars {
        if self.use_unicode {
            DrawingChars::unicode()
        } else {
            DrawingChars::ascii()
        }
    }
}

/// Characters used for drawing error indicators
pub struct DrawingChars {
    pub vertical: &'static str,
    pub horizontal: &'static str,
    pub top_left: &'static str,
    pub arrow: &'static str,
    pub pointer_start: &'static str,
    pub pointer_line: &'static str,
}

impl DrawingChars {
    pub fn unicode() -> Self {
        Self {
            vertical: "│",
            horizontal: "─",
            top_left: "┌",
            arrow: "→",
            pointer_start: "^",
            pointer_line: "─",
        }
    }

    pub fn ascii() -> Self {
        Self {
            vertical: "|",
            horizontal: "-",
            top_left: "+",
            arrow: "->",
            pointer_start: "^",
            pointer_line: "~",
        }
    }
}

/// Format a diagnostic message with pretty colors and formatting
pub fn format_diagnostic(diagnostic: &Diagnostic, style: &ErrorStyle) -> String {
    let mut output = String::new();

    // Format the main error message
    let (level_text, level_start, level_end) = style.level_style(diagnostic.level);
    
    if style.use_color {
        write!(
            &mut output,
            "{}{}{}: {}{}{}",
            level_start,
            level_text,
            level_end,
            colors::BOLD,
            diagnostic.message,
            colors::RESET
        )
        .unwrap();
    } else {
        write!(
            &mut output,
            "{}: {}",
            level_text,
            diagnostic.message
        )
        .unwrap();
    }

    output
}

/// Create a fancy box around important messages
pub fn boxed_message(title: &str, content: &str, style: &ErrorStyle) -> String {
    let chars = style.get_chars();
    let width = content
        .lines()
        .map(|line| line.len())
        .max()
        .unwrap_or(0)
        .max(title.len() + 4);

    let mut output = String::new();

    // Top border
    writeln!(
        &mut output,
        "{}{}",
        chars.top_left,
        chars.horizontal.repeat(width + 2)
    )
    .unwrap();

    // Title
    if !title.is_empty() {
        let (suggestion_start, suggestion_end) = style.suggestion_style();
        writeln!(
            &mut output,
            "{} {} {}{}",
            chars.vertical, suggestion_start, title, suggestion_end
        )
        .unwrap();

        // Separator
        writeln!(
            &mut output,
            "{}{}",
            chars.vertical,
            chars.horizontal.repeat(width + 2)
        )
        .unwrap();
    }

    // Content
    for line in content.lines() {
        writeln!(
            &mut output,
            "{} {:<width$} {}",
            chars.vertical,
            line,
            chars.vertical,
            width = width
        )
        .unwrap();
    }

    // Bottom border
    writeln!(
        &mut output,
        "{}{}",
        chars.top_left,
        chars.horizontal.repeat(width + 2)
    )
    .unwrap();

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_style_default() {
        let style = ErrorStyle::default();
        assert!(style.use_color);
        assert!(style.use_unicode);
        assert!(!style.compact);
    }

    #[test]
    fn test_level_style_with_color() {
        let style = ErrorStyle {
            use_color: true,
            use_unicode: true,
            compact: false,
        };

        let (text, start, end) = style.level_style(DiagnosticLevel::Error);
        assert_eq!(text, "error");
        assert!(start.contains(colors::BRIGHT_RED));
        assert_eq!(end, colors::RESET);

        let (text, start, end) = style.level_style(DiagnosticLevel::Warning);
        assert_eq!(text, "warning");
        assert!(start.contains(colors::BRIGHT_YELLOW));
        assert_eq!(end, colors::RESET);

        let (text, start, end) = style.level_style(DiagnosticLevel::Info);
        assert_eq!(text, "info");
        assert!(start.contains(colors::BRIGHT_CYAN));
        assert_eq!(end, colors::RESET);

        let (text, start, end) = style.level_style(DiagnosticLevel::Help);
        assert_eq!(text, "help");
        assert!(start.contains(colors::BRIGHT_GREEN));
        assert_eq!(end, colors::RESET);
    }

    #[test]
    fn test_level_style_without_color() {
        let style = ErrorStyle {
            use_color: false,
            use_unicode: true,
            compact: false,
        };

        let (text, start, end) = style.level_style(DiagnosticLevel::Error);
        assert_eq!(text, "error");
        assert_eq!(start, "");
        assert_eq!(end, "");

        let (text, start, end) = style.level_style(DiagnosticLevel::Warning);
        assert_eq!(text, "warning");
        assert_eq!(start, "");
        assert_eq!(end, "");
    }

    #[test]
    fn test_path_style() {
        let style_with_color = ErrorStyle {
            use_color: true,
            use_unicode: true,
            compact: false,
        };
        let (start, end) = style_with_color.path_style();
        assert!(start.contains(colors::BLUE));
        assert_eq!(end, colors::RESET);

        let style_without_color = ErrorStyle {
            use_color: false,
            use_unicode: true,
            compact: false,
        };
        let (start, end) = style_without_color.path_style();
        assert_eq!(start, "");
        assert_eq!(end, "");
    }

    #[test]
    fn test_line_number_style() {
        let style_with_color = ErrorStyle {
            use_color: true,
            use_unicode: true,
            compact: false,
        };
        let (start, end) = style_with_color.line_number_style();
        assert_eq!(start, colors::BLUE);
        assert_eq!(end, colors::RESET);

        let style_without_color = ErrorStyle {
            use_color: false,
            use_unicode: true,
            compact: false,
        };
        let (start, end) = style_without_color.line_number_style();
        assert_eq!(start, "");
        assert_eq!(end, "");
    }

    #[test]
    fn test_error_style() {
        let style_with_color = ErrorStyle {
            use_color: true,
            use_unicode: true,
            compact: false,
        };
        let (start, end) = style_with_color.error_style();
        assert!(start.contains(colors::BRIGHT_RED));
        assert_eq!(end, colors::RESET);

        let style_without_color = ErrorStyle {
            use_color: false,
            use_unicode: true,
            compact: false,
        };
        let (start, end) = style_without_color.error_style();
        assert_eq!(start, "");
        assert_eq!(end, "");
    }

    #[test]
    fn test_note_style() {
        let style_with_color = ErrorStyle {
            use_color: true,
            use_unicode: true,
            compact: false,
        };
        let (start, end) = style_with_color.note_style();
        assert!(start.contains(colors::BRIGHT_CYAN));
        assert_eq!(end, colors::RESET);
    }

    #[test]
    fn test_suggestion_style() {
        let style_with_color = ErrorStyle {
            use_color: true,
            use_unicode: true,
            compact: false,
        };
        let (start, end) = style_with_color.suggestion_style();
        assert!(start.contains(colors::BRIGHT_GREEN));
        assert_eq!(end, colors::RESET);
    }

    #[test]
    fn test_dim_style() {
        let style_with_color = ErrorStyle {
            use_color: true,
            use_unicode: true,
            compact: false,
        };
        let (start, end) = style_with_color.dim_style();
        assert_eq!(start, colors::DIM);
        assert_eq!(end, colors::RESET);

        let style_without_color = ErrorStyle {
            use_color: false,
            use_unicode: true,
            compact: false,
        };
        let (start, end) = style_without_color.dim_style();
        assert_eq!(start, "");
        assert_eq!(end, "");
    }

    #[test]
    fn test_get_chars_unicode() {
        let style = ErrorStyle {
            use_color: true,
            use_unicode: true,
            compact: false,
        };
        let chars = style.get_chars();
        assert_eq!(chars.vertical, "│");
        assert_eq!(chars.horizontal, "─");
        assert_eq!(chars.top_left, "┌");
        assert_eq!(chars.arrow, "→");
        assert_eq!(chars.pointer_start, "^");
        assert_eq!(chars.pointer_line, "─");
    }

    #[test]
    fn test_get_chars_ascii() {
        let style = ErrorStyle {
            use_color: true,
            use_unicode: false,
            compact: false,
        };
        let chars = style.get_chars();
        assert_eq!(chars.vertical, "|");
        assert_eq!(chars.horizontal, "-");
        assert_eq!(chars.top_left, "+");
        assert_eq!(chars.arrow, "->");
        assert_eq!(chars.pointer_start, "^");
        assert_eq!(chars.pointer_line, "~");
    }

    #[test]
    fn test_drawing_chars_unicode() {
        let chars = DrawingChars::unicode();
        assert_eq!(chars.vertical, "│");
        assert_eq!(chars.horizontal, "─");
        assert_eq!(chars.top_left, "┌");
        assert_eq!(chars.arrow, "→");
        assert_eq!(chars.pointer_start, "^");
        assert_eq!(chars.pointer_line, "─");
    }

    #[test]
    fn test_drawing_chars_ascii() {
        let chars = DrawingChars::ascii();
        assert_eq!(chars.vertical, "|");
        assert_eq!(chars.horizontal, "-");
        assert_eq!(chars.top_left, "+");
        assert_eq!(chars.arrow, "->");
        assert_eq!(chars.pointer_start, "^");
        assert_eq!(chars.pointer_line, "~");
    }

    #[test]
    fn test_format_diagnostic() {
        let diagnostic = Diagnostic {
            level: DiagnosticLevel::Error,
            message: "Test error message".to_string(),
            span: None,
            notes: vec![],
            suggestions: vec![],
            context_lines: 2,
        };

        let style = ErrorStyle {
            use_color: false,
            use_unicode: true,
            compact: false,
        };

        let formatted = format_diagnostic(&diagnostic, &style);
        assert!(formatted.contains("error: Test error message"));
    }

    #[test]
    fn test_format_diagnostic_with_color() {
        let diagnostic = Diagnostic {
            level: DiagnosticLevel::Warning,
            message: "Test warning".to_string(),
            span: None,
            notes: vec![],
            suggestions: vec![],
            context_lines: 2,
        };

        let style = ErrorStyle {
            use_color: true,
            use_unicode: true,
            compact: false,
        };

        let formatted = format_diagnostic(&diagnostic, &style);
        assert!(formatted.contains("warning"));
        assert!(formatted.contains("Test warning"));
        assert!(formatted.contains(colors::RESET));
    }

    #[test]
    fn test_boxed_message_simple() {
        let style = ErrorStyle {
            use_color: false,
            use_unicode: false,
            compact: false,
        };

        let boxed = boxed_message("Title", "Content line 1\nContent line 2", &style);
        
        // Check structure
        assert!(boxed.contains("+--"));
        assert!(boxed.contains(" Title"));
        assert!(boxed.contains("| Content line 1"));
        assert!(boxed.contains("| Content line 2"));
    }

    #[test]
    fn test_boxed_message_unicode() {
        let style = ErrorStyle {
            use_color: false,
            use_unicode: true,
            compact: false,
        };

        let boxed = boxed_message("Unicode Box", "Test content", &style);
        
        // Check unicode characters
        assert!(boxed.contains("┌"));
        assert!(boxed.contains("─"));
        assert!(boxed.contains("│"));
    }

    #[test]
    fn test_boxed_message_empty_title() {
        let style = ErrorStyle {
            use_color: false,
            use_unicode: false,
            compact: false,
        };

        let boxed = boxed_message("", "Just content", &style);
        
        // Should not have title separator when title is empty
        // Count the horizontal lines - should only have top and bottom borders
        let horizontal_count = boxed.matches("+--").count();
        assert_eq!(horizontal_count, 2); // Only top and bottom borders
        assert!(boxed.contains("| Just content"));
    }

    #[test]
    fn test_boxed_message_multiline() {
        let style = ErrorStyle {
            use_color: false,
            use_unicode: false,
            compact: false,
        };

        let content = "First line\nSecond line\nThird line with more text";
        let boxed = boxed_message("Multi", content, &style);
        
        // Check all lines are present
        assert!(boxed.contains("| First line"));
        assert!(boxed.contains("| Second line"));
        assert!(boxed.contains("| Third line with more text"));
        
        // Check padding is correct
        let lines: Vec<&str> = boxed.lines().collect();
        let content_lines: Vec<&str> = lines.iter()
            .filter(|l| l.contains("| ") && !l.contains("Multi"))
            .copied()
            .collect();
        
        // All content lines should have the same length
        if !content_lines.is_empty() {
            let first_len = content_lines[0].len();
            assert!(content_lines.iter().all(|l| l.len() == first_len));
        }
    }

    #[test]
    fn test_colors_constants() {
        // Just verify the color constants are defined correctly
        assert_eq!(colors::RESET, "\x1b[0m");
        assert_eq!(colors::BOLD, "\x1b[1m");
        assert_eq!(colors::DIM, "\x1b[2m");
        assert_eq!(colors::UNDERLINE, "\x1b[4m");
        
        assert_eq!(colors::RED, "\x1b[31m");
        assert_eq!(colors::GREEN, "\x1b[32m");
        assert_eq!(colors::YELLOW, "\x1b[33m");
        assert_eq!(colors::BLUE, "\x1b[34m");
        assert_eq!(colors::MAGENTA, "\x1b[35m");
        assert_eq!(colors::CYAN, "\x1b[36m");
        assert_eq!(colors::WHITE, "\x1b[37m");
        
        assert_eq!(colors::BRIGHT_RED, "\x1b[91m");
        assert_eq!(colors::BRIGHT_GREEN, "\x1b[92m");
        assert_eq!(colors::BRIGHT_YELLOW, "\x1b[93m");
        assert_eq!(colors::BRIGHT_BLUE, "\x1b[94m");
        assert_eq!(colors::BRIGHT_MAGENTA, "\x1b[95m");
        assert_eq!(colors::BRIGHT_CYAN, "\x1b[96m");
    }
}
