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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggest_similar_name_exact_case_insensitive() {
        let available = vec![
            "println".to_string(),
            "print_line".to_string(),
            "Printf".to_string(),
        ];
        
        // Exact case-insensitive match
        assert_eq!(
            SuggestionEngine::suggest_similar_name("PRINTLN", &available),
            Some("println".to_string())
        );
        assert_eq!(
            SuggestionEngine::suggest_similar_name("printf", &available),
            Some("Printf".to_string())
        );
    }

    #[test]
    fn test_suggest_similar_name_edit_distance() {
        let available = vec![
            "println".to_string(),
            "sprintf".to_string(),
            "random_func".to_string(),
        ];
        
        // Small typos
        assert_eq!(
            SuggestionEngine::suggest_similar_name("printl", &available),
            Some("println".to_string())
        );
        assert_eq!(
            SuggestionEngine::suggest_similar_name("print1n", &available),
            Some("println".to_string())
        );
        assert_eq!(
            SuggestionEngine::suggest_similar_name("sprintff", &available),
            Some("sprintf".to_string())
        );
    }

    #[test]
    fn test_suggest_similar_name_no_match() {
        let available = vec!["foo".to_string(), "bar".to_string()];
        
        // Too different
        assert_eq!(
            SuggestionEngine::suggest_similar_name("completely_different", &available),
            None
        );
        
        // Empty available list
        assert_eq!(
            SuggestionEngine::suggest_similar_name("anything", &[]),
            None
        );
    }

    #[test]
    fn test_edit_distance() {
        // Same strings
        assert_eq!(SuggestionEngine::edit_distance("hello", "hello"), 0);
        
        // One character difference
        assert_eq!(SuggestionEngine::edit_distance("hello", "hallo"), 1);
        assert_eq!(SuggestionEngine::edit_distance("hello", "hell"), 1);
        assert_eq!(SuggestionEngine::edit_distance("hello", "ello"), 1);
        
        // Multiple differences
        assert_eq!(SuggestionEngine::edit_distance("kitten", "sitting"), 3);
        
        // Empty strings
        assert_eq!(SuggestionEngine::edit_distance("", "hello"), 5);
        assert_eq!(SuggestionEngine::edit_distance("hello", ""), 5);
        assert_eq!(SuggestionEngine::edit_distance("", ""), 0);
    }

    #[test]
    fn test_is_fancy_quote() {
        // Fancy quotes
        assert!(SuggestionEngine::is_fancy_quote('\u{201C}')); // Left double quote
        assert!(SuggestionEngine::is_fancy_quote('\u{201D}')); // Right double quote
        assert!(SuggestionEngine::is_fancy_quote('\u{2018}')); // Left single quote
        assert!(SuggestionEngine::is_fancy_quote('\u{2019}')); // Right single quote
        assert!(SuggestionEngine::is_fancy_quote('`')); // Backtick
        assert!(SuggestionEngine::is_fancy_quote('\u{00B4}')); // Acute accent
        
        // Regular quotes
        assert!(!SuggestionEngine::is_fancy_quote('"'));
        assert!(!SuggestionEngine::is_fancy_quote('\''));
        assert!(!SuggestionEngine::is_fancy_quote('a'));
    }

    #[test]
    fn test_suggest_ascii_quote() {
        // Double quotes
        assert_eq!(SuggestionEngine::suggest_ascii_quote('\u{201C}'), Some('"'));
        assert_eq!(SuggestionEngine::suggest_ascii_quote('\u{201D}'), Some('"'));
        
        // Single quotes
        assert_eq!(SuggestionEngine::suggest_ascii_quote('\u{2018}'), Some('\''));
        assert_eq!(SuggestionEngine::suggest_ascii_quote('\u{2019}'), Some('\''));
        assert_eq!(SuggestionEngine::suggest_ascii_quote('`'), Some('\''));
        assert_eq!(SuggestionEngine::suggest_ascii_quote('\u{00B4}'), Some('\''));
        
        // Non-quotes
        assert_eq!(SuggestionEngine::suggest_ascii_quote('a'), None);
        assert_eq!(SuggestionEngine::suggest_ascii_quote('"'), None);
    }

    #[test]
    fn test_suggest_for_c_style_mistake() {
        // Increment/decrement
        assert!(SuggestionEngine::suggest_for_c_style_mistake("x++").unwrap().contains("x = x + 1"));
        assert!(SuggestionEngine::suggest_for_c_style_mistake("i--").unwrap().contains("x = x - 1"));
        
        // Assignment vs comparison
        assert!(SuggestionEngine::suggest_for_c_style_mistake("if (x = 5 && y == 3)")
            .unwrap().contains("'=' for assignment and '==' for comparison"));
        
        // Include statements
        assert!(SuggestionEngine::suggest_for_c_style_mistake("#include <stdio.h>")
            .unwrap().contains("import"));
        
        // Memory management
        assert!(SuggestionEngine::suggest_for_c_style_mistake("ptr = malloc(100)")
            .unwrap().contains("automatic memory management"));
        assert!(SuggestionEngine::suggest_for_c_style_mistake("free(ptr)")
            .unwrap().contains("automatic memory management"));
        
        // No mistakes
        assert_eq!(SuggestionEngine::suggest_for_c_style_mistake("let x = 5;"), None);
    }

    #[test]
    fn test_suggest_type_conversion() {
        // int to string
        assert!(SuggestionEngine::suggest_type_conversion("int", "string")
            .unwrap().contains("int_to_string"));
        assert!(SuggestionEngine::suggest_type_conversion("i64", "string")
            .unwrap().contains("int_to_string"));
        
        // string to int
        assert!(SuggestionEngine::suggest_type_conversion("string", "int")
            .unwrap().contains("parse_int"));
        assert!(SuggestionEngine::suggest_type_conversion("string", "i64")
            .unwrap().contains("parse_int"));
        
        // float conversions
        assert!(SuggestionEngine::suggest_type_conversion("float", "int")
            .unwrap().contains("to_int"));
        assert!(SuggestionEngine::suggest_type_conversion("int", "float")
            .unwrap().contains("to_float"));
        
        // bool conversions
        assert!(SuggestionEngine::suggest_type_conversion("bool", "string")
            .unwrap().contains("to_string"));
        assert!(SuggestionEngine::suggest_type_conversion("string", "bool")
            .unwrap().contains("parse_bool"));
        
        // Case insensitive
        assert!(SuggestionEngine::suggest_type_conversion("INT", "STRING").is_some());
        
        // No conversion available
        assert_eq!(SuggestionEngine::suggest_type_conversion("custom", "other"), None);
    }

    #[test]
    fn test_suggest_import_for_function() {
        // I/O functions
        assert_eq!(SuggestionEngine::suggest_import_for_function("println"), Some("import std.io;".to_string()));
        assert_eq!(SuggestionEngine::suggest_import_for_function("print"), Some("import std.io;".to_string()));
        assert_eq!(SuggestionEngine::suggest_import_for_function("readln"), Some("import std.io;".to_string()));
        
        // Math functions
        assert_eq!(SuggestionEngine::suggest_import_for_function("sqrt"), Some("import std.math;".to_string()));
        assert_eq!(SuggestionEngine::suggest_import_for_function("pow"), Some("import std.math;".to_string()));
        assert_eq!(SuggestionEngine::suggest_import_for_function("abs"), Some("import std.math;".to_string()));
        assert_eq!(SuggestionEngine::suggest_import_for_function("sin"), Some("import std.math;".to_string()));
        assert_eq!(SuggestionEngine::suggest_import_for_function("cos"), Some("import std.math;".to_string()));
        
        // String functions
        assert_eq!(SuggestionEngine::suggest_import_for_function("len"), Some("import std.string;".to_string()));
        assert_eq!(SuggestionEngine::suggest_import_for_function("substr"), Some("import std.string;".to_string()));
        assert_eq!(SuggestionEngine::suggest_import_for_function("concat"), Some("import std.string;".to_string()));
        
        // Collections
        assert_eq!(SuggestionEngine::suggest_import_for_function("Vec"), Some("import std.collections;".to_string()));
        assert_eq!(SuggestionEngine::suggest_import_for_function("HashMap"), Some("import std.collections;".to_string()));
        assert_eq!(SuggestionEngine::suggest_import_for_function("Set"), Some("import std.collections;".to_string()));
        
        // Unknown function
        assert_eq!(SuggestionEngine::suggest_import_for_function("unknown_func"), None);
    }

    #[test]
    fn test_check_balanced_delimiters() {
        // Balanced
        assert_eq!(SuggestionEngine::check_balanced_delimiters("(a + b)"), None);
        assert_eq!(SuggestionEngine::check_balanced_delimiters("[1, 2, 3]"), None);
        assert_eq!(SuggestionEngine::check_balanced_delimiters("{x: 1}"), None);
        assert_eq!(SuggestionEngine::check_balanced_delimiters("((a + b) * [c])"), None);
        assert_eq!(SuggestionEngine::check_balanced_delimiters(""), None);
        
        // Unmatched closing
        assert!(SuggestionEngine::check_balanced_delimiters("a + b)")
            .unwrap().contains("Unmatched closing parenthesis"));
        assert!(SuggestionEngine::check_balanced_delimiters("arr]")
            .unwrap().contains("Unmatched closing bracket"));
        assert!(SuggestionEngine::check_balanced_delimiters("obj}")
            .unwrap().contains("Unmatched closing brace"));
        
        // Unclosed opening
        assert!(SuggestionEngine::check_balanced_delimiters("(a + b")
            .unwrap().contains("Unclosed delimiter '('"));
        assert!(SuggestionEngine::check_balanced_delimiters("[1, 2")
            .unwrap().contains("Unclosed delimiter '['"));
        assert!(SuggestionEngine::check_balanced_delimiters("{x: 1")
            .unwrap().contains("Unclosed delimiter '{'"));
        
        // Mismatched
        assert!(SuggestionEngine::check_balanced_delimiters("(a + b]")
            .unwrap().contains("Mismatched"));
        assert!(SuggestionEngine::check_balanced_delimiters("[a + b)")
            .unwrap().contains("Mismatched"));
        assert!(SuggestionEngine::check_balanced_delimiters("{a + b]")
            .unwrap().contains("Mismatched"));
    }

    #[test]
    fn test_matching_delimiter() {
        assert_eq!(SuggestionEngine::matching_delimiter('('), ')');
        assert_eq!(SuggestionEngine::matching_delimiter('['), ']');
        assert_eq!(SuggestionEngine::matching_delimiter('{'), '}');
        assert_eq!(SuggestionEngine::matching_delimiter('x'), 'x'); // fallback
    }

    #[test]
    fn test_beginner_patterns_printf() {
        let code = "printf(\"%d %s %f\", x, name, pi);";
        let suggestions = BeginnerPatterns::check_pattern(code);
        assert!(!suggestions.is_empty());
        assert!(suggestions[0].contains("string interpolation"));
        assert!(suggestions[0].contains("println"));
    }

    #[test]
    fn test_beginner_patterns_null() {
        let suggestions = BeginnerPatterns::check_pattern("if (ptr == null) {");
        assert!(suggestions.iter().any(|s| s.contains("Option types")));
        
        let suggestions = BeginnerPatterns::check_pattern("value = nil;");
        assert!(suggestions.iter().any(|s| s.contains("Option<T>")));
        
        let suggestions = BeginnerPatterns::check_pattern("return NULL;");
        assert!(suggestions.iter().any(|s| s.contains("Some(value)") && s.contains("None")));
    }

    #[test]
    fn test_beginner_patterns_var() {
        let suggestions = BeginnerPatterns::check_pattern("var x = 5;");
        assert!(suggestions.iter().any(|s| s.contains("'let' for variables")));
        assert!(suggestions.iter().any(|s| s.contains("'let mut' for mutable")));
    }

    #[test]
    fn test_beginner_patterns_const() {
        // Wrong position
        let suggestions = BeginnerPatterns::check_pattern("fn main() { const X = 5; }");
        assert!(suggestions.iter().any(|s| s.contains("top level for constants")));
        
        // Correct position (should not trigger)
        let suggestions = BeginnerPatterns::check_pattern("const PI = 3.14;");
        assert!(!suggestions.iter().any(|s| s.contains("top level for constants")));
    }

    #[test]
    fn test_beginner_patterns_class() {
        let suggestions = BeginnerPatterns::check_pattern("class MyClass {");
        assert!(suggestions.iter().any(|s| s.contains("'struct' for data types")));
        assert!(suggestions.iter().any(|s| s.contains("Classes are not supported")));
    }

    #[test]
    fn test_beginner_patterns_switch() {
        let suggestions = BeginnerPatterns::check_pattern("switch (value) {");
        assert!(suggestions.iter().any(|s| s.contains("'match' expressions")));
    }

    #[test]
    fn test_beginner_patterns_do_while() {
        let suggestions = BeginnerPatterns::check_pattern("do { x++; } while (x < 10);");
        assert!(suggestions.iter().any(|s| s.contains("loop { ... if condition { break; } }")));
        
        // Test without space
        let suggestions = BeginnerPatterns::check_pattern("do{ x++; } while (x < 10);");
        assert!(suggestions.iter().any(|s| s.contains("doesn't have do-while loops")));
    }

    #[test]
    fn test_beginner_patterns_multiple() {
        let code = r#"
            var x = null;
            class Foo {
                switch (x) {
                    case 1: printf("%d", x);
                }
            }
        "#;
        
        let suggestions = BeginnerPatterns::check_pattern(code);
        
        // Should have suggestions for all patterns
        assert!(suggestions.iter().any(|s| s.contains("'let' for variables")));
        assert!(suggestions.iter().any(|s| s.contains("Option types")));
        assert!(suggestions.iter().any(|s| s.contains("'struct' for data types")));
        assert!(suggestions.iter().any(|s| s.contains("'match' expressions")));
        assert!(suggestions.iter().any(|s| s.contains("string interpolation")));
    }

    #[test]
    fn test_beginner_patterns_clean_code() {
        // Clean Palladium code should not trigger any suggestions
        let code = r#"
            let x = Some(42);
            struct Point { x: int, y: int }
            match value {
                Some(n) => println("Value: {}", n),
                None => println("No value"),
            }
        "#;
        
        let suggestions = BeginnerPatterns::check_pattern(code);
        assert_eq!(suggestions.len(), 0);
    }
}
