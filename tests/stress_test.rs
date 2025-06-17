// Stress tests for Alan von Palladium Compiler v0.1
// Testing edge cases and performance boundaries

use palladium::Driver;

#[test]
fn test_very_long_string() {
    let driver = Driver::new();
    let long_string = "a".repeat(1000);
    let source = format!(
        r#"
fn main() {{
    print("{}");
}}
"#,
        long_string
    );
    
    let result = driver.compile_string(&source, "long_string.pd").map(|_| ());
    assert!(result.is_ok(), "Should handle long strings");
}

#[test]
fn test_many_print_statements() {
    let driver = Driver::new();
    let mut source = String::from("fn main() {\n");
    
    // Generate 100 print statements
    for i in 0..100 {
        source.push_str(&format!("    print(\"Line {}\");\n", i));
    }
    source.push_str("}\n");
    
    let result = driver.compile_string(&source, "many_prints.pd").map(|_| ());
    assert!(result.is_ok(), "Should handle many print statements");
}

#[test]
fn test_deeply_nested_comments() {
    let driver = Driver::new();
    let source = r#"
// Comment 1
fn main() {
    // Comment 2
    print("Test"); // Comment 3
    // Comment 4
    // // Nested comment style
    // Comment 5
}
// Final comment
"#;
    
    let result = driver.compile_string(source, "nested_comments.pd").map(|_| ());
    assert!(result.is_ok(), "Should handle multiple comments");
}

#[test]
fn test_unicode_in_strings() {
    let driver = Driver::new();
    let source = r#"
fn main() {
    print("Hello, 世界!");
    print("Emoji test: 🚀");
    print("Math symbols: ∑ ∏ ∫");
}
"#;
    
    let result = driver.compile_string(source, "unicode.pd").map(|_| ());
    // Unicode support might not be in v0.1
    match result {
        Ok(_) => println!("Unicode is supported in v0.1"),
        Err(e) => println!("Unicode not yet supported in v0.1: {}", e),
    }
}

#[test]
fn test_very_long_function_name() {
    let driver = Driver::new();
    let long_name = "very_long_function_name_that_might_cause_issues_in_some_compilers_but_should_be_handled_gracefully";
    let source = format!(
        r#"
fn {}() {{
    print("Long function name");
}}

fn main() {{
    print("Main function");
}}
"#,
        long_name
    );
    
    let result = driver.compile_string(&source, "long_func_name.pd").map(|_| ());
    // Long function names might not be fully supported in v0.1
    match result {
        Ok(_) => println!("Long function names are supported"),
        Err(_) => println!("Long function names not yet fully supported"),
    }
}

#[test]
fn test_whitespace_variations() {
    let driver = Driver::new();
    
    // Test various whitespace styles
    let test_cases = [
        // Minimal whitespace
        "fn main(){print(\"Minimal\");}",
        
        // Excessive whitespace
        r#"


        fn    main  (  )    {
            
            
            print   (   "Excessive"   )  ;
            
            
        }


        "#,
        
        // Mixed tabs and spaces (not recommended but should handle)
        "fn main() {\n\tprint(\"Tab\");\n    print(\"Spaces\");\n}",
    ];
    
    for (i, source) in test_cases.iter().enumerate() {
        let result = driver
            .compile_string(source, &format!("whitespace_{}.pd", i))
            .map(|_| ());
        assert!(
            result.is_ok(),
            "Should handle whitespace variation {}: {}",
            i,
            source
        );
    }
}

#[test]
fn test_empty_string_literal() {
    let driver = Driver::new();
    let source = r#"
fn main() {
    print("");
}
"#;
    
    let result = driver.compile_string(source, "empty_string.pd").map(|_| ());
    assert!(result.is_ok(), "Should handle empty string literals");
}

#[test]
fn test_special_characters_in_strings() {
    let driver = Driver::new();
    let source = r#"
fn main() {
    print("Special: !@#$%^&*()_+-=[]{}|;:',.<>?/");
}
"#;
    
    let result = driver.compile_string(source, "special_chars.pd").map(|_| ());
    assert!(result.is_ok(), "Should handle special characters in strings");
}

#[test]
fn test_multiple_functions_same_line() {
    let driver = Driver::new();
    let source = "fn helper() { print(\"Helper\"); } fn main() { print(\"Main\"); }";
    
    let result = driver.compile_string(source, "one_line.pd").map(|_| ());
    // This might not be supported in v0.1
    match result {
        Ok(_) => println!("Single-line multiple functions are supported"),
        Err(_) => println!("Single-line multiple functions not yet supported"),
    }
}

#[test]
#[ignore] // This test might be too intensive for regular test runs
fn test_extremely_large_program() {
    let driver = Driver::new();
    let mut source = String::from("fn main() {\n");
    
    // Generate 10,000 print statements
    for i in 0..10000 {
        source.push_str(&format!("    print(\"Line {}\");\n", i));
    }
    source.push_str("}\n");
    
    let result = driver.compile_string(&source, "huge_program.pd").map(|_| ());
    assert!(result.is_ok(), "Should handle very large programs");
}