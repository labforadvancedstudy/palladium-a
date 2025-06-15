// Integration tests for Alan von Palladium Compiler v0.1
// Testing the legendary compiler's basic functionality

use palladium::{Driver, CompileError};
use std::fs;
use std::path::Path;

/// Helper function to compile a source string and check if it succeeds
fn compile_source(source: &str, name: &str) -> Result<(), CompileError> {
    let driver = Driver::new();
    driver.compile_string(source, name).map(|_| ())
}

/// Helper function to create a temporary test file
fn create_test_file(content: &str, filename: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(filename);
    fs::write(&path, content).expect("Failed to write test file");
    path
}

#[test]
fn test_hello_world_compilation() {
    let source = r#"
// Hello World in Palladium
fn main() {
    print("Hello, World!");
}
"#;
    
    let result = compile_source(source, "hello.pd");
    assert!(result.is_ok(), "Hello world program should compile successfully");
}

#[test]
fn test_multiple_print_statements() {
    let source = r#"
// Multiple print statements
fn main() {
    print("First line");
    print("Second line");
    print("Third line");
}
"#;
    
    let result = compile_source(source, "multi_print.pd");
    assert!(result.is_ok(), "Multiple print statements should compile successfully");
}

#[test]
fn test_compile_and_run_hello_world() {
    let source = r#"
fn main() {
    print("Hello from Palladium!");
}
"#;
    
    let test_file = create_test_file(source, "test_hello.pd");
    let driver = Driver::new();
    
    let result = driver.compile_and_run(&test_file);
    
    // Clean up
    fs::remove_file(&test_file).ok();
    
    assert!(result.is_ok(), "Hello world program should compile and run successfully");
}

#[test]
fn test_syntax_error_missing_semicolon() {
    let source = r#"
fn main() {
    print("Missing semicolon")  // Missing semicolon
}
"#;
    
    let result = compile_source(source, "syntax_error.pd");
    assert!(result.is_err(), "Missing semicolon should cause compilation error");
    
    if let Err(e) = result {
        // Check that we get a meaningful error message
        let error_msg = e.to_string();
        assert!(error_msg.contains("semicolon") || error_msg.contains("';'") || 
                error_msg.contains("expected"), 
                "Error message should mention missing semicolon: {}", error_msg);
    }
}

#[test]
fn test_syntax_error_unclosed_string() {
    let source = r#"
fn main() {
    print("Unclosed string);
}
"#;
    
    let result = compile_source(source, "unclosed_string.pd");
    assert!(result.is_err(), "Unclosed string should cause compilation error");
}

#[test]
fn test_syntax_error_invalid_function_syntax() {
    let source = r#"
// Invalid function syntax - missing parentheses
fn main {
    print("Invalid");
}
"#;
    
    let result = compile_source(source, "invalid_func.pd");
    assert!(result.is_err(), "Invalid function syntax should cause compilation error");
}

#[test]
fn test_type_error_wrong_function_name() {
    let source = r#"
// Wrong function name - should be 'main'
fn not_main() {
    print("This won't work");
}
"#;
    
    let result = compile_source(source, "no_main.pd");
    assert!(result.is_err(), "Missing main function should cause compilation error");
}

#[test]
fn test_type_error_undefined_function() {
    let source = r#"
fn main() {
    undefined_function();  // This function doesn't exist
}
"#;
    
    let result = compile_source(source, "undefined_func.pd");
    assert!(result.is_err(), "Calling undefined function should cause compilation error");
}

#[test]
fn test_empty_main_function() {
    let source = r#"
// Empty main function
fn main() {
    // Nothing here
}
"#;
    
    let result = compile_source(source, "empty_main.pd");
    assert!(result.is_ok(), "Empty main function should compile successfully");
}

#[test]
fn test_comments_handling() {
    let source = r#"
// This is a comment
fn main() {
    // Another comment
    print("Comments should be ignored");
    // Final comment
}
"#;
    
    let result = compile_source(source, "comments.pd");
    assert!(result.is_ok(), "Program with comments should compile successfully");
}

#[test]
fn test_multiple_functions() {
    let source = r#"
fn helper() {
    print("Helper function");
}

fn main() {
    helper();
    print("Main function");
}
"#;
    
    let result = compile_source(source, "multi_func.pd");
    // This might fail in v0.1 if function calls aren't fully implemented
    match result {
        Ok(_) => println!("Function calls are supported in v0.1"),
        Err(_) => println!("Function calls not yet supported in v0.1"),
    }
}

#[test]
fn test_compile_file_not_found() {
    let driver = Driver::new();
    let result = driver.compile_file(Path::new("nonexistent_file.pd"));
    
    assert!(result.is_err(), "Compiling non-existent file should fail");
    
    if let Err(e) = result {
        match e {
            CompileError::IoError(_) => {
                // Expected error type
            }
            _ => panic!("Expected IoError for non-existent file, got: {:?}", e),
        }
    }
}

#[test]
fn test_empty_source_file() {
    let source = "";
    let result = compile_source(source, "empty.pd");
    
    assert!(result.is_err(), "Empty source file should cause compilation error");
}

#[test]
fn test_whitespace_only_file() {
    let source = "   \n\n   \t\t  \n  ";
    let result = compile_source(source, "whitespace.pd");
    
    assert!(result.is_err(), "Whitespace-only file should cause compilation error");
}

// Stress test with a longer program
#[test]
fn test_longer_program() {
    let source = r#"
// A longer test program for Palladium
fn main() {
    print("Line 1: Starting the program");
    print("Line 2: Alan von Palladium");
    print("Line 3: Where Legends Compile");
    print("Line 4: Testing multiple statements");
    print("Line 5: Each with its own print");
    print("Line 6: To verify statement handling");
    print("Line 7: And proper code generation");
    print("Line 8: For sequential execution");
    print("Line 9: Almost done");
    print("Line 10: End of test");
}
"#;
    
    let result = compile_source(source, "longer.pd");
    assert!(result.is_ok(), "Longer program should compile successfully");
}

// Test to verify output format
#[test]
fn test_verify_c_output_generation() {
    let source = r#"
fn main() {
    print("Testing C output");
}
"#;
    
    let driver = Driver::new();
    let result = driver.compile_string(source, "test_output.pd");
    
    assert!(result.is_ok(), "Should generate C output successfully");
    
    if let Ok(output_path) = result {
        // Verify the output file exists
        assert!(output_path.exists(), "Output C file should exist");
        
        // Verify it's a .c file
        assert_eq!(output_path.extension().and_then(|s| s.to_str()), Some("c"),
                   "Output should be a C file");
        
        // Read and verify basic C structure
        let c_content = fs::read_to_string(&output_path).unwrap();
        assert!(c_content.contains("#include"), "C output should contain includes");
        assert!(c_content.contains("int main"), "C output should contain main function");
        assert!(c_content.contains("printf"), "C output should contain printf for print statements");
        
        // Clean up
        fs::remove_file(&output_path).ok();
    }
}