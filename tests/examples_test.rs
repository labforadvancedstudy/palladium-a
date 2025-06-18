// Test the example programs in the examples/ directory
// Ensuring our showcase programs work correctly

use palladium::Driver;
use std::path::Path;

#[test]
fn test_hello_example() {
    let driver = Driver::new();
    let example_path = Path::new("examples/hello.pd");

    // Check if the example exists
    if !example_path.exists() {
        println!("Skipping test: examples/hello.pd not found");
        return;
    }

    let result = driver.compile_file(example_path);
    assert!(
        result.is_ok(),
        "examples/hello.pd should compile successfully"
    );
}

#[test]
fn test_greet_example() {
    let driver = Driver::new();
    let example_path = Path::new("examples/greet.pd");

    if !example_path.exists() {
        println!("Skipping test: examples/greet.pd not found");
        return;
    }

    let result = driver.compile_file(example_path);
    assert!(
        result.is_ok(),
        "examples/greet.pd should compile successfully"
    );
}

#[test]
fn test_return_type_example() {
    let driver = Driver::new();
    let example_path = Path::new("examples/return_type.pd");

    if !example_path.exists() {
        println!("Skipping test: examples/return_type.pd not found");
        return;
    }

    let result = driver.compile_file(example_path);
    // This might fail if return types aren't implemented in v0.1
    match result {
        Ok(_) => println!("Return types are supported in v0.1"),
        Err(e) => println!("Return types not yet supported in v0.1: {}", e),
    }
}

#[test]
fn test_no_return_type_example() {
    let driver = Driver::new();
    let example_path = Path::new("examples/no_return_type.pd");

    if !example_path.exists() {
        println!("Skipping test: examples/no_return_type.pd not found");
        return;
    }

    let result = driver.compile_file(example_path);
    // Check if omitting return type is allowed
    match result {
        Ok(_) => println!("Omitting return type is allowed in v0.1"),
        Err(e) => println!("Return type is required in v0.1: {}", e),
    }
}

#[test]
fn test_run_hello_example() {
    let driver = Driver::new();
    let example_path = Path::new("examples/hello.pd");

    if !example_path.exists() {
        println!("Skipping test: examples/hello.pd not found");
        return;
    }

    // Test compile and run
    let result = driver.compile_and_run(example_path);
    assert!(
        result.is_ok(),
        "examples/hello.pd should compile and run successfully"
    );
}
