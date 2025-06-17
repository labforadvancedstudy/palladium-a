use std::fs;
use std::process::Command;

/// Helper function to compile Palladium code and check output
fn compile_and_check(name: &str, pd_code: &str, expected_output: &str) {
    let pd_file = format!("test_{}.pd", name);
    let out_file = format!("test_{}", name);
    let c_file = format!("test_{}.c", name);

    // Write Palladium code to file
    fs::write(&pd_file, pd_code).expect("Failed to write test file");

    // Use the 'run' command which compiles and executes
    let run_output = Command::new("cargo")
        .args(["run", "--", "run", &pd_file])
        .output()
        .expect("Failed to run palladiumc");

    // Extract the actual program output (between the dashed lines)
    let full_output = String::from_utf8_lossy(&run_output.stdout);
    let lines: Vec<&str> = full_output.lines().collect();
    
    // Find the output between the dashed lines
    let mut start_idx = None;
    let mut end_idx = None;
    for (i, line) in lines.iter().enumerate() {
        if line.contains("─────────────────────────────────────") {
            if start_idx.is_none() {
                start_idx = Some(i + 1);
            } else {
                end_idx = Some(i);
                break;
            }
        }
    }
    
    let actual_output = if let (Some(start), Some(end)) = (start_idx, end_idx) {
        lines[start..end].join("\n")
    } else {
        // Fallback if pattern not found
        full_output.to_string()
    };

    assert_eq!(
        actual_output.trim(),
        expected_output.trim(),
        "Output mismatch for {}.\nFull output:\n{}",
        name,
        full_output
    );

    // Cleanup
    fs::remove_file(&pd_file).ok();
    fs::remove_file(&out_file).ok();
    fs::remove_file(&c_file).ok();
}

#[test]
fn test_variable_declaration_and_usage() {
    let code = r#"
fn main() {
    let x = 42;
    let y = x;
    print_int(y);
}
"#;
    compile_and_check("var_decl", code, "42");
}

#[test]
fn test_integer_literals() {
    let code = r#"
fn main() {
    print_int(0);
    print_int(123);
    print_int(999999);
}
"#;
    compile_and_check("int_literals", code, "0\n123\n999999");
}

#[test]
fn test_arithmetic_operations() {
    let code = r#"
fn main() {
    let a = 10;
    let b = 3;
    
    print_int(a + b);  // 13
    print_int(a - b);  // 7
    print_int(a * b);  // 30
    print_int(a / b);  // 3
    print_int(a % b);  // 1
}
"#;
    compile_and_check("arithmetic_ops", code, "13\n7\n30\n3\n1");
}

#[test]
fn test_complex_arithmetic_expressions() {
    let code = r#"
fn main() {
    let result = (10 + 5) * 2 - 8 / 4;
    print_int(result);  // (15 * 2) - 2 = 30 - 2 = 28
}
"#;
    compile_and_check("complex_arithmetic", code, "28");
}

#[test]
fn test_comparison_operators() {
    let code = r#"
fn main() {
    let a = 10;
    let b = 5;
    
    if a > b {
        print_int(1);  // Should print
    }
    
    if a < b {
        print_int(0);  // Should not print
    }
    
    if a >= 10 {
        print_int(2);  // Should print
    }
    
    if b <= 5 {
        print_int(3);  // Should print
    }
    
    if a == 10 {
        print_int(4);  // Should print
    }
    
    if a != b {
        print_int(5);  // Should print
    }
}
"#;
    compile_and_check("comparison_ops", code, "1\n2\n3\n4\n5");
}

#[test]
fn test_bool_type_and_literals() {
    let code = r#"
fn main() {
    let t = true;
    let f = false;
    
    if t {
        print_int(1);  // Should print
    }
    
    if f {
        print_int(0);  // Should not print
    } else {
        print_int(2);  // Should print
    }
}
"#;
    compile_and_check("bool_literals", code, "1\n2");
}

#[test]
fn test_if_else_statements() {
    let code = r#"
fn main() {
    let x = 10;
    
    if x > 5 {
        print_int(1);
    } else {
        print_int(0);
    }
    
    if x < 5 {
        print_int(0);
    } else {
        print_int(2);
    }
}
"#;
    compile_and_check("if_else", code, "1\n2");
}

#[test]
fn test_nested_if_statements() {
    let code = r#"
fn main() {
    let x = 10;
    let y = 20;
    
    if x > 5 {
        if y > 15 {
            print_int(1);  // Should print
        } else {
            print_int(0);
        }
    } else {
        print_int(0);
    }
    
    if x < 20 {
        if y < 10 {
            print_int(0);
        } else {
            if x == 10 {
                print_int(2);  // Should print
            }
        }
    }
}
"#;
    compile_and_check("nested_if", code, "1\n2");
}

#[test]
fn test_type_inference() {
    let code = r#"
fn main() {
    let i = 42;        // Should infer Int
    let b = true;      // Should infer Bool
    let sum = i + 8;   // Should infer Int
    let cmp = i > 30;  // Should infer Bool
    
    print_int(i);
    print_int(sum);
    
    if b {
        print_int(100);
    }
    
    if cmp {
        print_int(200);
    }
}
"#;
    compile_and_check("type_inference", code, "42\n50\n100\n200");
}

#[test]
fn test_variable_scoping() {
    let code = r#"
fn main() {
    let x = 10;
    print_int(x);  // 10
    
    if true {
        let x = 20;  // Shadows outer x
        print_int(x);    // 20
    }
    
    print_int(x);  // 10 (outer x is still 10)
}
"#;
    compile_and_check("scoping", code, "10\n20\n10");
}

#[test]
fn test_complex_scoping_with_nested_blocks() {
    let code = r#"
fn main() {
    let a = 1;
    let b = 2;
    
    if a < b {
        let a = 10;  // Shadows outer a
        print_int(a);    // 10
        
        if b > 1 {
            let b = 20;  // Shadows outer b
            print_int(a);    // 10 (from parent block)
            print_int(b);    // 20
        }
        
        print_int(b);    // 2 (original b)
    }
    
    print_int(a);  // 1 (original a)
    print_int(b);  // 2 (original b)
}
"#;
    compile_and_check("complex_scoping", code, "10\n10\n20\n2\n1\n2");
}

#[test]
fn test_chained_arithmetic_and_comparisons() {
    let code = r#"
fn main() {
    let x = 5;
    let y = 10;
    let z = 15;
    
    let sum = x + y + z;
    print_int(sum);  // 30
    
    if x < y {
        if y < z {
            print_int(100);  // Should print
        }
    }
    
    let result = (x * 2) + (y / 2) - (z % 10);
    print_int(result);  // 10 + 5 - 5 = 10
}
"#;
    compile_and_check("chained_ops", code, "30\n100\n10");
}

#[test]
fn test_multiple_variable_declarations() {
    let code = r#"
fn main() {
    let a = 1;
    let b = 2;
    let c = 3;
    let d = a + b + c;
    
    print_int(a);
    print_int(b);
    print_int(c);
    print_int(d);
}
"#;
    compile_and_check("multi_vars", code, "1\n2\n3\n6");
}

#[test]
fn test_expression_in_if_condition() {
    let code = r#"
fn main() {
    let x = 10;
    let y = 5;
    
    if (x + y) > 10 {
        print_int(1);  // Should print
    }
    
    if (x - y) * 2 == 10 {
        print_int(2);  // Should print
    }
    
    if x / y + 3 < 5 {
        print_int(3);  // Should not print
    } else {
        print_int(4);  // Should print
    }
}
"#;
    compile_and_check("expr_in_condition", code, "1\n2\n4");
}