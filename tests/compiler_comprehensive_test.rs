// Comprehensive tests for the Palladium compiler core components
// Testing from a bird's eye view to achieve 80% coverage

use palladium::{Driver, CompileError};

/// Helper function to compile source and check success
fn compile_source(source: &str) -> Result<String, CompileError> {
    let driver = Driver::new();
    driver.compile_string(source, "test.pd").map(|path| {
        // Read the generated C file
        std::fs::read_to_string(path).unwrap_or_else(|_| String::new())
    })
}

/// Helper to compile and verify C output contains expected patterns
fn compile_and_verify(source: &str, expected_patterns: &[&str]) {
    let result = compile_source(source);
    assert!(result.is_ok(), "Failed to compile:\n{}", source);
    
    let output = result.unwrap();
    for pattern in expected_patterns {
        assert!(
            output.contains(pattern),
            "Output missing '{}' for source:\n{}\nGenerated:\n{}",
            pattern, source, output
        );
    }
}

#[test]
fn test_all_keywords() {
    let test_cases = vec![
        // Core keywords
        ("fn main() { }", vec!["int main("]),
        ("fn main() { let x = 5; }", vec!["int x = 5;"]),
        ("fn main() { if true { } }", vec!["if (1)"]),
        ("fn main() { if true { } else { } }", vec!["if (1)", "else"]),
        ("fn main() { while true { } }", vec!["while (1)"]),
        ("fn main() { for i in 0..10 { } }", vec!["for", "int i"]),
        ("fn foo() -> int { return 42; }", vec!["return 42;"]),
        ("struct Point { x: int, y: int }", vec!["struct Point", "int x;", "int y;"]),
        ("enum Option { Some, None }", vec!["typedef enum"]),
        
        // Advanced keywords
        ("fn main() { match 1 { 1 => {}, _ => {} } }", vec!["switch"]),
        ("trait Display { }", vec!["// Trait:"]),
        ("pub fn main() { }", vec!["int main("]),
        ("const X: int = 5;", vec!["const int X = 5;"]),
        ("static Y: int = 10;", vec!["static int Y = 10;"]),
        ("fn main() { let mut z = 0; }", vec!["int z = 0;"]),
        ("type Int = int;", vec!["typedef int Int;"]),
        ("fn main() { loop { break; } }", vec!["while (1)", "break;"]),
        ("fn main() { loop { continue; } }", vec!["while (1)", "continue;"]),
        ("fn main() { let x = 5 as int; }", vec!["int x = ((int)5);"]),
        ("fn main() { let t = true; let f = false; }", vec!["int t = 1;", "int f = 0;"]),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

#[test]
fn test_all_operators() {
    let test_cases = vec![
        // Arithmetic operators
        ("fn main() { let x = 1 + 2; }", vec!["(1 + 2)"]),
        ("fn main() { let x = 3 - 1; }", vec!["(3 - 1)"]),
        ("fn main() { let x = 2 * 3; }", vec!["(2 * 3)"]),
        ("fn main() { let x = 6 / 2; }", vec!["(6 / 2)"]),
        ("fn main() { let x = 5 % 2; }", vec!["(5 % 2)"]),
        
        // Comparison operators
        ("fn main() { let x = 1 == 1; }", vec!["(1 == 1)"]),
        ("fn main() { let x = 1 != 2; }", vec!["(1 != 2)"]),
        ("fn main() { let x = 1 < 2; }", vec!["(1 < 2)"]),
        ("fn main() { let x = 2 > 1; }", vec!["(2 > 1)"]),
        ("fn main() { let x = 1 <= 2; }", vec!["(1 <= 2)"]),
        ("fn main() { let x = 2 >= 1; }", vec!["(2 >= 1)"]),
        
        // Logical operators
        ("fn main() { let x = true && false; }", vec!["(1 && 0)"]),
        ("fn main() { let x = true || false; }", vec!["(1 || 0)"]),
        ("fn main() { let x = !true; }", vec!["(!1)"]),
        
        // Bitwise operators
        ("fn main() { let x = 1 & 2; }", vec!["(1 & 2)"]),
        ("fn main() { let x = 1 | 2; }", vec!["(1 | 2)"]),
        ("fn main() { let x = 1 ^ 2; }", vec!["(1 ^ 2)"]),
        ("fn main() { let x = 1 << 2; }", vec!["(1 << 2)"]),
        ("fn main() { let x = 4 >> 1; }", vec!["(4 >> 1)"]),
        
        // Assignment operators
        ("fn main() { let mut x = 0; x += 1; }", vec!["x = x + 1;"]),
        ("fn main() { let mut x = 2; x -= 1; }", vec!["x = x - 1;"]),
        ("fn main() { let mut x = 2; x *= 3; }", vec!["x = x * 3;"]),
        ("fn main() { let mut x = 6; x /= 2; }", vec!["x = x / 2;"]),
        ("fn main() { let mut x = 5; x %= 2; }", vec!["x = x % 2;"]),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

#[test]
fn test_literals() {
    let test_cases = vec![
        // Integer literals
        ("fn main() { let x = 123; }", vec!["123"]),
        ("fn main() { let x = 0; }", vec!["0"]),
        
        // String literals
        (r#"fn main() { print("hello"); }"#, vec![r#""hello""#]),
        (r#"fn main() { print("hello world"); }"#, vec![r#""hello world""#]),
        (r#"fn main() { print("hello\nworld"); }"#, vec![r#""hello\nworld""#]),
        (r#"fn main() { print("hello\tworld"); }"#, vec![r#""hello\tworld""#]),
        
        // Boolean literals
        ("fn main() { let x = true; }", vec!["1"]),
        ("fn main() { let x = false; }", vec!["0"]),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

#[test]
fn test_control_flow() {
    let test_cases = vec![
        // If statements
        (
            "fn main() { if 5 > 3 { print(\"yes\"); } }",
            vec!["if ((5 > 3))", "printf"]
        ),
        (
            "fn main() { if 1 < 2 { print(\"a\"); } else { print(\"b\"); } }",
            vec!["if ((1 < 2))", "else", "printf"]
        ),
        (
            "fn main() { 
                if 1 > 2 { 
                    print(\"a\"); 
                } else if 2 > 1 { 
                    print(\"b\"); 
                } else { 
                    print(\"c\"); 
                } 
            }",
            vec!["if ((1 > 2))", "else if ((2 > 1))", "else"]
        ),
        
        // While loops
        (
            "fn main() { 
                let mut i = 0; 
                while i < 10 { 
                    i = i + 1; 
                } 
            }",
            vec!["while ((i < 10))", "i = i + 1;"]
        ),
        
        // For loops
        (
            "fn main() { 
                for i in 0..10 { 
                    print(i); 
                } 
            }",
            vec!["for", "int i", "printf"]
        ),
        
        // Loop with break/continue
        (
            "fn main() { 
                loop { 
                    if true { break; } 
                    continue; 
                } 
            }",
            vec!["while (1)", "break;", "continue;"]
        ),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

#[test]
fn test_functions() {
    let test_cases = vec![
        // Basic functions
        (
            "fn add(x: int, y: int) -> int { x + y }",
            vec!["int add(int x, int y)", "return (x + y);"]
        ),
        (
            "fn greet(name: string) { print(name); }",
            vec!["void greet(char* name)", "printf"]
        ),
        (
            "fn get_value() -> int { 42 }",
            vec!["int get_value()", "return 42;"]
        ),
        
        // Function calls
        (
            "fn double(x: int) -> int { x * 2 }
             fn main() { let y = double(21); }",
            vec!["int double(int x)", "int y = double(21);"]
        ),
        
        // Recursive functions
        (
            "fn factorial(n: int) -> int {
                if n <= 1 { 1 } else { n * factorial(n - 1) }
             }",
            vec!["int factorial(int n)", "factorial((n - 1))"]
        ),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

#[test]
fn test_structs() {
    let test_cases = vec![
        // Basic struct
        (
            "struct Point { x: int, y: int }",
            vec!["struct Point {", "int x;", "int y;"]
        ),
        
        // Struct with methods
        (
            "struct Point { x: int, y: int }
             impl Point {
                 fn new(x: int, y: int) -> Point {
                     Point { x: x, y: y }
                 }
             }",
            vec!["struct Point", "Point new(int x, int y)"]
        ),
        
        // Using structs
        (
            "struct Point { x: int, y: int }
             fn main() {
                 let p = Point { x: 10, y: 20 };
                 print(p.x);
             }",
            vec!["Point p =", "p.x"]
        ),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

#[test]
fn test_arrays() {
    let test_cases = vec![
        // Array declaration
        (
            "fn main() { let arr = [1, 2, 3, 4, 5]; }",
            vec!["int arr[5] =", "{1, 2, 3, 4, 5}"]
        ),
        
        // Array access
        (
            "fn main() { 
                let arr = [10, 20, 30]; 
                let x = arr[1]; 
            }",
            vec!["arr[1]"]
        ),
        
        // Array in loops
        (
            "fn main() {
                let arr = [1, 2, 3, 4, 5];
                for i in 0..5 {
                    print(arr[i]);
                }
            }",
            vec!["for", "arr[i]"]
        ),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

#[test]
fn test_enums() {
    let test_cases = vec![
        // Basic enum
        (
            "enum Color { Red, Green, Blue }",
            vec!["typedef enum", "Red", "Green", "Blue"]
        ),
        
        // Enum with values
        (
            "enum Option { Some(int), None }",
            vec!["typedef enum"]
        ),
        
        // Match on enum
        (
            "enum Color { Red, Green, Blue }
             fn main() {
                 let c = Color::Red;
                 match c {
                     Color::Red => print(\"red\"),
                     Color::Green => print(\"green\"),
                     Color::Blue => print(\"blue\")
                 }
             }",
            vec!["switch", "case", "printf"]
        ),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

#[test]
fn test_error_cases() {
    let error_cases = vec![
        // Type errors
        "fn main() { let x: int = \"hello\"; }",
        "fn main() { let x: string = 42; }",
        "fn main() { let x = 1 + \"hello\"; }",
        
        // Undefined variables
        "fn main() { print(undefined_var); }",
        
        // Undefined functions
        "fn main() { undefined_func(); }",
        
        // Wrong number of arguments
        "fn add(x: int, y: int) -> int { x + y }
         fn main() { add(1); }",
        
        // Type mismatch in function
        "fn get_int() -> int { \"not an int\" }",
        
        // Missing return
        "fn get_value() -> int { }",
        
        // Invalid control flow
        "fn main() { if 42 { } }",
        "fn main() { while \"not bool\" { } }",
    ];

    for source in error_cases {
        let result = compile_source(source);
        assert!(result.is_err(), "Expected error for: {}", source);
    }
}

#[test]
fn test_operator_precedence() {
    let test_cases = vec![
        // Arithmetic precedence
        (
            "fn main() { let x = 1 + 2 * 3; }",
            vec!["(1 + (2 * 3))"]
        ),
        (
            "fn main() { let x = (1 + 2) * 3; }",
            vec!["((1 + 2) * 3)"]
        ),
        
        // Comparison and logical
        (
            "fn main() { let x = 1 < 2 && 3 < 4; }",
            vec!["((1 < 2) && (3 < 4))"]
        ),
        (
            "fn main() { let x = 1 + 2 < 3 * 4; }",
            vec!["((1 + 2) < (3 * 4))"]
        ),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

#[test]
fn test_complex_programs() {
    // Fibonacci
    compile_and_verify(
        r#"
        fn fib(n: int) -> int {
            if n <= 1 {
                n
            } else {
                fib(n - 1) + fib(n - 2)
            }
        }
        
        fn main() {
            let result = fib(10);
            print(result);
        }
        "#,
        &["int fib(int n)", "fib((n - 1))", "fib((n - 2))"]
    );
    
    // Bubble sort
    compile_and_verify(
        r#"
        fn bubble_sort(arr: [int; 10], n: int) {
            for i in 0..n {
                for j in 0..(n - i - 1) {
                    if arr[j] > arr[j + 1] {
                        let temp = arr[j];
                        arr[j] = arr[j + 1];
                        arr[j + 1] = temp;
                    }
                }
            }
        }
        "#,
        &["void bubble_sort", "for", "if", "temp"]
    );
    
    // Struct with multiple methods
    compile_and_verify(
        r#"
        struct Rectangle {
            width: int,
            height: int
        }
        
        impl Rectangle {
            fn new(w: int, h: int) -> Rectangle {
                Rectangle { width: w, height: h }
            }
            
            fn area(&self) -> int {
                self.width * self.height
            }
            
            fn perimeter(&self) -> int {
                2 * (self.width + self.height)
            }
        }
        
        fn main() {
            let rect = Rectangle::new(10, 20);
            print(rect.area());
            print(rect.perimeter());
        }
        "#,
        &["struct Rectangle", "int area", "int perimeter", "rect.area()", "rect.perimeter()"]
    );
}