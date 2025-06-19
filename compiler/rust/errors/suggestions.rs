// Common error patterns and suggestions for Palladium
// "Learning from mistakes, one suggestion at a time"

// Removed unused imports

/// Provides intelligent suggestions based on error patterns
pub struct SuggestionEngine;

impl SuggestionEngine {
    /// Suggest similar identifier names (for typos)
    pub fn suggest_similar_name(name: &str, available: &[String]) -> Option<String> {
        let name_lower = name.to_lowercase();

        // Find exact case-insensitive match first
        for candidate in available {
            if candidate.to_lowercase() == name_lower {
                return Some(candidate.clone());
            }
        }

        // Find similar names using edit distance
        let mut best_match = None;
        let mut best_distance = usize::MAX;

        for candidate in available {
            let distance = Self::edit_distance(name, candidate);

            // Only suggest if the distance is reasonable (less than 1/3 of the length)
            if distance < best_distance && distance <= name.len() / 3 + 1 {
                best_distance = distance;
                best_match = Some(candidate.clone());
            }
        }

        best_match
    }

    /// Calculate Levenshtein edit distance between two strings
    fn edit_distance(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let a_len = a_chars.len();
        let b_len = b_chars.len();

        if a_len == 0 {
            return b_len;
        }
        if b_len == 0 {
            return a_len;
        }

        let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

        // Initialize first column and row
        for (i, row) in matrix.iter_mut().enumerate().take(a_len + 1) {
            row[0] = i;
        }
        for j in 0..=b_len {
            matrix[0][j] = j;
        }

        // Fill the matrix
        for i in 1..=a_len {
            for j in 1..=b_len {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                matrix[i][j] = (matrix[i - 1][j] + 1) // deletion
                    .min(matrix[i][j - 1] + 1) // insertion
                    .min(matrix[i - 1][j - 1] + cost); // substitution
            }
        }

        matrix[a_len][b_len]
    }

    /// Check if a character looks like a quote that should be ASCII
    pub fn is_fancy_quote(ch: char) -> bool {
        matches!(
            ch,
            '\u{201C}' | '\u{201D}' | '\u{2018}' | '\u{2019}' | '`' | '\u{00B4}'
        )
    }

    /// Get the ASCII equivalent of a fancy quote
    pub fn suggest_ascii_quote(ch: char) -> Option<char> {
        match ch {
            '\u{201C}' | '\u{201D}' => Some('"'),
            '\u{2018}' | '\u{2019}' | '`' | '\u{00B4}' => Some('\''),
            _ => None,
        }
    }

    /// Common beginner mistakes with C-style syntax
    pub fn suggest_for_c_style_mistake(code: &str) -> Option<String> {
        if code.contains("++") {
            Some(
                "Palladium doesn't have ++ operator. Use 'x = x + 1' or 'x += 1' instead"
                    .to_string(),
            )
        } else if code.contains("--") {
            Some(
                "Palladium doesn't have -- operator. Use 'x = x - 1' or 'x -= 1' instead"
                    .to_string(),
            )
        } else if code.contains("==") && code.contains("=") {
            Some("Make sure you're using '=' for assignment and '==' for comparison".to_string())
        } else if code.starts_with("#include") {
            Some(
                "Palladium uses 'import' instead of '#include'. Example: import std.io;"
                    .to_string(),
            )
        } else if code.contains("malloc") || code.contains("free") {
            Some(
                "Palladium has automatic memory management. You don't need malloc/free".to_string(),
            )
        } else {
            None
        }
    }

    /// Suggest fixes for common type errors
    pub fn suggest_type_conversion(from_type: &str, to_type: &str) -> Option<String> {
        match (
            from_type.to_lowercase().as_str(),
            to_type.to_lowercase().as_str(),
        ) {
            ("int" | "i64", "string") => {
                Some("Use int_to_string() to convert int to string".to_string())
            }
            ("string", "int" | "i64") => {
                Some("Use parse_int() to convert string to int".to_string())
            }
            ("float", "int" | "i64") => Some("Use to_int() to convert float to int".to_string()),
            ("int" | "i64", "float") => Some("Use to_float() to convert int to float".to_string()),
            ("bool", "string") => Some("Use to_string() to convert bool to string".to_string()),
            ("string", "bool") => Some("Use parse_bool() to convert string to bool".to_string()),
            _ => None,
        }
    }

    /// Suggest fixes for missing imports
    pub fn suggest_import_for_function(func_name: &str) -> Option<String> {
        match func_name {
            "println" | "print" | "readln" => Some("import std.io;".to_string()),
            "sqrt" | "pow" | "abs" | "sin" | "cos" => Some("import std.math;".to_string()),
            "len" | "substr" | "concat" => Some("import std.string;".to_string()),
            "Vec" | "HashMap" | "Set" => Some("import std.collections;".to_string()),
            _ => None,
        }
    }

    /// Check if parentheses are balanced
    pub fn check_balanced_delimiters(code: &str) -> Option<String> {
        let mut stack = Vec::new();

        for (i, ch) in code.chars().enumerate() {
            match ch {
                '(' | '[' | '{' => stack.push((ch, i)),
                ')' => {
                    if let Some((open, _)) = stack.pop() {
                        if open != '(' {
                            return Some(format!(
                                "Mismatched parentheses: expected '{}' to match '{}'",
                                Self::matching_delimiter(open),
                                open
                            ));
                        }
                    } else {
                        return Some("Unmatched closing parenthesis ')'".to_string());
                    }
                }
                ']' => {
                    if let Some((open, _)) = stack.pop() {
                        if open != '[' {
                            return Some(format!(
                                "Mismatched brackets: expected '{}' to match '{}'",
                                Self::matching_delimiter(open),
                                open
                            ));
                        }
                    } else {
                        return Some("Unmatched closing bracket ']'".to_string());
                    }
                }
                '}' => {
                    if let Some((open, _)) = stack.pop() {
                        if open != '{' {
                            return Some(format!(
                                "Mismatched braces: expected '{}' to match '{}'",
                                Self::matching_delimiter(open),
                                open
                            ));
                        }
                    } else {
                        return Some("Unmatched closing brace '}'".to_string());
                    }
                }
                _ => {}
            }
        }

        if let Some((open, _)) = stack.pop() {
            Some(format!(
                "Unclosed delimiter '{}', expected '{}'",
                open,
                Self::matching_delimiter(open)
            ))
        } else {
            None
        }
    }

    fn matching_delimiter(open: char) -> char {
        match open {
            '(' => ')',
            '[' => ']',
            '{' => '}',
            _ => open,
        }
    }
}

/// Common patterns that beginners might use incorrectly
pub struct BeginnerPatterns;

impl BeginnerPatterns {
    pub fn check_pattern(code: &str) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Check for printf-style formatting
        if code.contains("%d") || code.contains("%s") || code.contains("%f") {
            suggestions.push(
                "Palladium uses string interpolation instead of printf-style formatting. \
                 Example: println(\"x = {}\", x);"
                    .to_string(),
            );
        }

        // Check for null/nil/None
        if code.contains("null") || code.contains("nil") || code.contains("NULL") {
            suggestions.push(
                "Palladium uses Option types instead of null. \
                 Use 'Option<T>' and 'Some(value)' or 'None'"
                    .to_string(),
            );
        }

        // Check for var keyword
        if code.contains("var ") {
            suggestions.push(
                "Palladium uses 'let' for variables and 'let mut' for mutable variables"
                    .to_string(),
            );
        }

        // Check for const keyword at wrong position
        if code.contains("const ") && !code.trim_start().starts_with("const") {
            suggestions.push(
                "In Palladium, 'const' should be used at the top level for constants".to_string(),
            );
        }

        // Check for class keyword
        if code.contains("class ") {
            suggestions.push(
                "Palladium uses 'struct' for data types. Classes are not supported yet".to_string(),
            );
        }

        // Check for switch statement
        if code.contains("switch") {
            suggestions.push(
                "Palladium uses 'match' expressions instead of switch statements".to_string(),
            );
        }

        // Check for do-while
        if code.contains("do {") || code.contains("do{") {
            suggestions.push(
                "Palladium doesn't have do-while loops. Use 'loop { ... if condition { break; } }'"
                    .to_string(),
            );
        }

        suggestions
    }
}
