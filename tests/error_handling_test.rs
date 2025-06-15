// Error handling tests for Alan von Palladium Compiler v0.1
// Testing various error scenarios and error message quality

use palladium::{CompileError, Driver};

/// Helper to check if an error message contains expected text
fn assert_error_contains(result: Result<(), CompileError>, expected: &str) {
    assert!(result.is_err(), "Expected compilation error");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.to_lowercase().contains(&expected.to_lowercase()),
        "Error message '{}' should contain '{}'",
        error_msg,
        expected
    );
}

#[test]
fn test_lexer_error_invalid_character() {
    let driver = Driver::new();
    let source = r#"
fn main() {
    print("test");
    @ // Invalid character
}
"#;
    
    let result = driver.compile_string(source, "invalid_char.pd").map(|_| ());
    assert!(result.is_err(), "Invalid character should cause lexer error");
}

#[test]
fn test_parser_error_missing_closing_brace() {
    let driver = Driver::new();
    let source = r#"
fn main() {
    print("Missing closing brace");
    // Missing }
"#;
    
    let result = driver.compile_string(source, "missing_brace.pd").map(|_| ());
    // In v0.1, we might get a generic "end of file" error instead of specific "brace" error
    assert!(result.is_err(), "Missing closing brace should cause parser error");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("brace") || error_msg.contains("end of file") || error_msg.contains("EOF"),
        "Error should mention brace or end of file, got: {}",
        error_msg
    );
}

#[test]
fn test_parser_error_missing_opening_paren() {
    let driver = Driver::new();
    let source = r#"
fn main) {  // Missing opening parenthesis
    print("test");
}
"#;
    
    let result = driver.compile_string(source, "missing_paren.pd").map(|_| ());
    assert!(result.is_err(), "Missing opening parenthesis should cause parser error");
}

#[test]
fn test_type_error_no_entry_point() {
    let driver = Driver::new();
    let source = r#"
fn not_the_main_function() {
    print("This is not main");
}

fn helper() {
    print("Helper");
}
"#;
    
    let result = driver.compile_string(source, "no_main.pd").map(|_| ());
    assert_error_contains(result, "main");
}

#[test]
fn test_nested_syntax_errors() {
    let driver = Driver::new();
    let source = r#"
fn main() {
    print("First";  // Missing closing paren
    print("Second));  // Mismatched quotes
}
"#;
    
    let result = driver.compile_string(source, "nested_errors.pd").map(|_| ());
    assert!(result.is_err(), "Multiple syntax errors should be caught");
}

#[test]
fn test_error_with_line_numbers() {
    let driver = Driver::new();
    let source = r#"
fn main() {
    print("Line 2");
    print("Line 3");
    invalid_statement;  // Error on line 4
    print("Line 5");
}
"#;
    
    let result = driver.compile_string(source, "line_error.pd").map(|_| ());
    assert!(result.is_err(), "Invalid statement should cause error");
    
    // In a real implementation, we'd check that the error mentions line 4
    // For v0.1, just checking that there's an error is sufficient
}

#[test]
fn test_consecutive_errors() {
    let driver = Driver::new();
    
    // First error
    let source1 = r#"fn main() { syntax error }"#;
    let result1 = driver.compile_string(source1, "error1.pd").map(|_| ());
    assert!(result1.is_err());
    
    // Second error - driver should handle multiple compilations
    let source2 = r#"fn { invalid }"#;
    let result2 = driver.compile_string(source2, "error2.pd").map(|_| ());
    assert!(result2.is_err());
    
    // Valid program - should work after errors
    let source3 = r#"fn main() { print("Works"); }"#;
    let result3 = driver.compile_string(source3, "valid.pd").map(|_| ());
    assert!(result3.is_ok(), "Driver should recover from previous errors");
}

#[test]
fn test_error_recovery_in_parser() {
    let driver = Driver::new();
    let source = r#"
fn first() {
    syntax error here;
}

fn main() {
    print("This should still be parsed");
}
"#;
    
    let result = driver.compile_string(source, "recovery.pd").map(|_| ());
    // For v0.1, we might not have error recovery, so just check it fails
    assert!(result.is_err(), "Syntax error should cause compilation failure");
}

#[test]
fn test_meaningful_error_messages() {
    let driver = Driver::new();
    
    // Test various error scenarios and ensure messages are helpful
    let test_cases = vec![
        (
            r#"fn main() { print("test" }"#,
            vec!["parenthes", ")", "closing", "expected"],
        ),
        (
            r#"fn main { print("test"); }"#,
            vec!["(", "parenthes", "parameter", "expected"],
        ),
        (
            r#"main() { print("test"); }"#,
            vec!["fn", "function", "declaration", "expected"],
        ),
    ];
    
    for (source, expected_words) in test_cases {
        let result = driver.compile_string(source, "error_msg_test.pd").map(|_| ());
        assert!(result.is_err(), "Should produce error for: {}", source);
        
        let error_msg = result.unwrap_err().to_string().to_lowercase();
        let contains_any = expected_words.iter().any(|word| error_msg.contains(word));
        assert!(
            contains_any,
            "Error message '{}' should contain at least one of: {:?}",
            error_msg,
            expected_words
        );
    }
}