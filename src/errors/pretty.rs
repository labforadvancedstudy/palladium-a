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
