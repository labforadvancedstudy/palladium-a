// End-to-end tests: drive the real `pdc` binary, then inspect what it wrote.
//
// All four of these used to fail on "C file not generated" while the
// compilation they had just asserted succeeded. They were looking in
// `target/build/`, which only ever holds the linked *executable*
// (`src/driver/mod.rs:259`); the generated C goes to `build_output/`
// (`src/codegen/mod.rs:2963-2974`). The path was the whole bug.
//
// The file stem is unique per run because `build_output/<stem>.c` is a global
// name and other test binaries compile programs of their own — see
// `tests/common/mod.rs`.

mod common;

use common::unique_module_name;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the driver puts the generated C for a module of this name.
fn generated_c(module: &str) -> PathBuf {
    Path::new("build_output").join(format!("{}.c", module))
}

/// Write `source` to a scratch `.pd` file and compile it with the real binary.
fn compile_with_pdc(module: &str, source: &str) -> std::process::Output {
    let test_dir = Path::new("target/e2e_tests");
    fs::create_dir_all(test_dir).unwrap();

    let source_path = test_dir.join(format!("{}.pd", module));
    fs::write(&source_path, source).unwrap();

    Command::new("./target/release/pdc")
        .arg("compile")
        .arg(&source_path)
        .output()
        .expect("Failed to execute pdc")
}

#[test]
fn test_hello_world_compilation() {
    let module = unique_module_name("hello");
    let output = compile_with_pdc(
        &module,
        r#"
fn main() {
    print("Hello, World!");
}
"#,
    );

    assert!(
        output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let c_file = generated_c(&module);
    assert!(
        c_file.exists(),
        "C file not generated at {}",
        c_file.display()
    );

    let c_content = fs::read_to_string(&c_file).unwrap();
    assert!(
        c_content.contains("__pd_print"),
        "C file doesn't call the print runtime"
    );
    assert!(
        c_content.contains("Hello, World!"),
        "C file doesn't contain Hello, World!"
    );
}

#[test]
fn test_fibonacci_compilation() {
    let module = unique_module_name("fibonacci");
    let output = compile_with_pdc(
        &module,
        r#"
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
"#,
    );

    assert!(
        output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let c_file = generated_c(&module);
    assert!(
        c_file.exists(),
        "C file not generated at {}",
        c_file.display()
    );

    let c_content = fs::read_to_string(&c_file).unwrap();
    assert!(
        c_content.contains("long long fibonacci(long long n)"),
        "C file doesn't contain the fibonacci function"
    );
}

#[test]
fn test_array_compilation() {
    let module = unique_module_name("arrays");
    let output = compile_with_pdc(
        &module,
        r#"
fn main() {
    let arr = [1, 2, 3, 4, 5];
    print_int(arr[3]);
}
"#,
    );

    assert!(
        output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let c_file = generated_c(&module);
    assert!(
        c_file.exists(),
        "C file not generated at {}",
        c_file.display()
    );

    let c_content = fs::read_to_string(&c_file).unwrap();
    assert!(
        c_content.contains("long long arr[5] = {1, 2, 3, 4, 5}"),
        "C file doesn't contain the array"
    );
}

#[test]
fn test_struct_compilation() {
    let module = unique_module_name("structs");
    let output = compile_with_pdc(
        &module,
        r#"
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 10, y: 20 };
    print_int(p.x);
}
"#,
    );

    assert!(
        output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let c_file = generated_c(&module);
    assert!(
        c_file.exists(),
        "C file not generated at {}",
        c_file.display()
    );

    let c_content = fs::read_to_string(&c_file).unwrap();
    assert!(
        c_content.contains("struct Point"),
        "C file doesn't contain struct Point"
    );
}

#[test]
fn test_error_reporting() {
    let module = unique_module_name("error");
    let output = compile_with_pdc(
        &module,
        r#"
fn main() {
    let x = unknown_function();
}
"#,
    );

    assert!(!output.status.success(), "Compilation should have failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown_function") || stderr.contains("Undefined"),
        "Error message should mention undefined function"
    );
}
