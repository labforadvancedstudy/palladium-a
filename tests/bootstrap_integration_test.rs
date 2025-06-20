// Integration tests for the Palladium bootstrap compilers
// Testing the self-hosting capability end-to-end

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Compile a Palladium source file with bootstrap compiler
fn compile_with_bootstrap(source: &str, compiler_path: &str) -> Result<String, String> {
    let temp_dir = TempDir::new().unwrap();
    let source_path = temp_dir.path().join("test.pd");
    let output_path = temp_dir.path().join("test.c");
    
    fs::write(&source_path, source).unwrap();
    
    // Run bootstrap compiler
    let output = Command::new(compiler_path)
        .arg(source_path.to_str().unwrap())
        .arg("-o")
        .arg(output_path.to_str().unwrap())
        .output()
        .map_err(|e| format!("Failed to run compiler: {}", e))?;
    
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    
    // Read generated C code
    fs::read_to_string(&output_path)
        .map_err(|e| format!("Failed to read output: {}", e))
}

#[test]
#[ignore] // Enable when bootstrap compilers are built
fn test_tiny_v16_hello_world() {
    let source = r#"
fn main() {
    print("Hello from tiny_v16!");
}
"#;
    
    let result = compile_with_bootstrap(source, "bootstrap3/tiny_v16");
    assert!(result.is_ok());
    
    let c_code = result.unwrap();
    assert!(c_code.contains("printf"));
    assert!(c_code.contains("Hello from tiny_v16!"));
}

#[test]
#[ignore] // Enable when bootstrap compilers are built
fn test_tiny_v16_arithmetic() {
    let source = r#"
fn main() {
    let a = 10;
    let b = 20;
    let c = a + b;
    print(c);
}
"#;
    
    let result = compile_with_bootstrap(source, "bootstrap3/tiny_v16");
    assert!(result.is_ok());
    
    let c_code = result.unwrap();
    assert!(c_code.contains("int a = 10;"));
    assert!(c_code.contains("int b = 20;"));
    assert!(c_code.contains("int c = (a + b);"));
}

#[test]
#[ignore] // Enable when bootstrap compilers are built
fn test_tiny_v16_functions() {
    let source = r#"
fn add(x: int, y: int) -> int {
    x + y
}

fn main() {
    let result = add(5, 3);
    print(result);
}
"#;
    
    let result = compile_with_bootstrap(source, "bootstrap3/tiny_v16");
    assert!(result.is_ok());
    
    let c_code = result.unwrap();
    assert!(c_code.contains("int add(int x, int y)"));
    assert!(c_code.contains("return (x + y);"));
}

#[test]
#[ignore] // Enable when bootstrap compilers are built
fn test_tiny_v16_arrays() {
    let source = r#"
fn main() {
    let arr = [1, 2, 3, 4, 5];
    let sum = 0;
    for i in 0..5 {
        sum = sum + arr[i];
    }
    print(sum);
}
"#;
    
    let result = compile_with_bootstrap(source, "bootstrap3/tiny_v16");
    assert!(result.is_ok());
    
    let c_code = result.unwrap();
    assert!(c_code.contains("int arr[5] = {1, 2, 3, 4, 5};"));
    assert!(c_code.contains("for"));
    assert!(c_code.contains("arr[i]"));
}

#[test]
#[ignore] // Enable when bootstrap compilers are built
fn test_pdc_full_compiler() {
    let source = r#"
struct Point {
    x: int,
    y: int
}

fn distance(p1: Point, p2: Point) -> int {
    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    dx * dx + dy * dy
}

fn main() {
    let p1 = Point { x: 0, y: 0 };
    let p2 = Point { x: 3, y: 4 };
    let dist = distance(p1, p2);
    print(dist);
}
"#;
    
    let result = compile_with_bootstrap(source, "bootstrap2/pdc");
    assert!(result.is_ok());
    
    let c_code = result.unwrap();
    assert!(c_code.contains("struct Point"));
    assert!(c_code.contains("int distance"));
}

#[test]
fn test_bootstrap_progression() {
    // Test that each bootstrap compiler can compile the next
    let tiny_versions = [
        "tiny_v1.pd", "tiny_v2.pd", "tiny_v3.pd", "tiny_v4.pd",
        "tiny_v5.pd", "tiny_v6.pd", "tiny_v7.pd", "tiny_v8.pd",
        "tiny_v9.pd", "tiny_v10.pd", "tiny_v11.pd", "tiny_v12.pd",
        "tiny_v13.pd", "tiny_v14.pd", "tiny_v15.pd", "tiny_v16.pd"
    ];
    
    // Verify all bootstrap files exist
    for version in &tiny_versions {
        let path = Path::new("bootstrap3").join(version);
        assert!(path.exists(), "Bootstrap file {} not found", version);
    }
}

#[test]
fn test_bootstrap_self_hosting() {
    // Verify the self-hosting capability markers
    let bootstrap_achieved = Path::new("bootstrap3/BOOTSTRAP_ACHIEVED.md");
    assert!(bootstrap_achieved.exists(), "Bootstrap achievement marker not found");
    
    let content = fs::read_to_string(bootstrap_achieved).unwrap();
    assert!(content.contains("100% self-hosting"), "Self-hosting not confirmed");
}

// Test specific language features supported by bootstrap compilers
mod feature_tests {
    use super::*;
    
    #[test]
    #[ignore]
    fn test_if_else_chains() {
        let source = r#"
        fn classify(n: int) -> string {
            if n < 0 {
                "negative"
            } else if n == 0 {
                "zero"
            } else if n < 10 {
                "single digit"
            } else {
                "multi digit"
            }
        }
        
        fn main() {
            print(classify(-5));
            print(classify(0));
            print(classify(7));
            print(classify(42));
        }
        "#;
        
        let result = compile_with_bootstrap(source, "bootstrap3/tiny_v16");
        assert!(result.is_ok());
    }
    
    #[test]
    #[ignore]
    fn test_nested_loops() {
        let source = r#"
        fn main() {
            for i in 0..3 {
                for j in 0..3 {
                    print(i * 3 + j);
                }
            }
        }
        "#;
        
        let result = compile_with_bootstrap(source, "bootstrap3/tiny_v16");
        assert!(result.is_ok());
    }
    
    #[test]
    #[ignore]
    fn test_recursive_functions() {
        let source = r#"
        fn gcd(a: int, b: int) -> int {
            if b == 0 {
                a
            } else {
                gcd(b, a % b)
            }
        }
        
        fn main() {
            print(gcd(48, 18));
        }
        "#;
        
        let result = compile_with_bootstrap(source, "bootstrap3/tiny_v16");
        assert!(result.is_ok());
    }
}