use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_hello_world_compilation() {
    // Create a simple hello world program
    let test_dir = Path::new("target/e2e_tests");
    fs::create_dir_all(test_dir).unwrap();

    let source_path = test_dir.join("hello.pd");
    let source = r#"
fn main() {
    print("Hello, World!");
}
"#;
    fs::write(&source_path, source).unwrap();

    // Compile with pdc
    let output = Command::new("./target/release/pdc")
        .arg("compile")
        .arg(&source_path)
        .output()
        .expect("Failed to execute pdc");

    // Check compilation succeeded
    assert!(
        output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check C file was generated in build_output
    let c_file = Path::new("build_output/hello.c");
    assert!(c_file.exists(), "C file not generated");

    // Check C file contains expected content
    let c_content = fs::read_to_string(&c_file).unwrap();
    assert!(
        c_content.contains("printf"),
        "C file doesn't contain printf"
    );
    assert!(
        c_content.contains("Hello, World!"),
        "C file doesn't contain Hello, World!"
    );
}

#[test]
fn test_fibonacci_compilation() {
    let test_dir = Path::new("target/e2e_tests");
    fs::create_dir_all(test_dir).unwrap();

    let source_path = test_dir.join("fibonacci.pd");
    let source = r#"
fn fibonacci(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    return fibonacci(n - 1) + fibonacci(n - 2);
}

fn main() {
    let result = fibonacci(10);
    print_int(result);
}
"#;
    fs::write(&source_path, source).unwrap();

    // Compile with pdc
    let output = Command::new("./target/release/pdc")
        .arg("compile")
        .arg(&source_path)
        .output()
        .expect("Failed to execute pdc");

    assert!(
        output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check C file was generated in build_output
    let c_file = Path::new("build_output/fibonacci.c");
    assert!(c_file.exists(), "C file not generated");

    // Check C file contains fibonacci function
    let c_content = fs::read_to_string(&c_file).unwrap();
    assert!(
        c_content.contains("fibonacci"),
        "C file doesn't contain fibonacci function"
    );
}

#[test]
fn test_array_compilation() {
    let test_dir = Path::new("target/e2e_tests");
    fs::create_dir_all(test_dir).unwrap();

    let source_path = test_dir.join("arrays.pd");
    let source = r#"
fn main() {
    let arr = [1, 2, 3, 4, 5];
    print_int(3); // Just print the value directly for now
}
"#;
    fs::write(&source_path, source).unwrap();

    // Compile with pdc
    let output = Command::new("./target/release/pdc")
        .arg("compile")
        .arg(&source_path)
        .output()
        .expect("Failed to execute pdc");

    assert!(
        output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let c_file = Path::new("build_output/arrays.c");
    assert!(c_file.exists(), "C file not generated");
}

#[test]
fn test_struct_compilation() {
    let test_dir = Path::new("target/e2e_tests");
    fs::create_dir_all(test_dir).unwrap();

    let source_path = test_dir.join("structs.pd");
    let source = r#"
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 10, y: 20 };
    print_int(p.x);
}
"#;
    fs::write(&source_path, source).unwrap();

    // Compile with pdc
    let output = Command::new("./target/release/pdc")
        .arg("compile")
        .arg(&source_path)
        .output()
        .expect("Failed to execute pdc");

    assert!(
        output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let c_file = Path::new("build_output/structs.c");
    assert!(c_file.exists(), "C file not generated");

    // Check struct definition
    let c_content = fs::read_to_string(&c_file).unwrap();
    assert!(
        c_content.contains("struct Point"),
        "C file doesn't contain struct Point"
    );
}

#[test]
fn test_error_reporting() {
    let test_dir = Path::new("target/e2e_tests");
    fs::create_dir_all(test_dir).unwrap();

    let source_path = test_dir.join("error.pd");
    let source = r#"
fn main() {
    let x = unknown_function();
}
"#;
    fs::write(&source_path, source).unwrap();

    // Compile with pdc (should fail)
    let output = Command::new("./target/release/pdc")
        .arg("compile")
        .arg(&source_path)
        .output()
        .expect("Failed to execute pdc");

    // Check compilation failed
    assert!(!output.status.success(), "Compilation should have failed");

    // Check error message
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown_function") || stderr.contains("Undefined"),
        "Error message should mention undefined function"
    );
}
