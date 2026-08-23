// Comprehensive tests for the Palladium compiler core components.
//
// Every expectation here is the C the compiler actually emits, checked against
// `docs/specification/grammar.ebnf`. Three things had drifted and made all of
// these fail:
//
//   1. `int`/`string` in the expected C. The integer type lowers to
//      `long long` (`int` is only a source-level alias for `i64`,
//      `src/parser/mod.rs:2250`) and `print` lowers to `__pd_print`, not
//      `printf`.
//   2. Fragments with no `fn main`. The driver rejects a program without one
//      ("No main function found"), so a declaration-only snippet cannot be
//      compiled at all.
//   3. Constant folding. The optimizer runs by default
//      (`src/driver/mod.rs:198-201`), so `1 + 2` never reaches the C as
//      `(1 + 2)`. Operator tests therefore use variables, which is what they
//      meant to test anyway: that the operator survives, not that folding is off.
//
// Cases that need a language feature that does not exist are `#[ignore]`d
// individually with the missing feature named — never folded back into a
// passing test. See `make test-xfail`.

mod common;

use common::unique_source_name;
use palladium::{CompileError, Driver};

/// Compile source and return the generated C.
///
/// The virtual filename is unique per call: it decides the output path
/// (`build_output/<stem>.c`), and a shared one races under cargo's parallel
/// test threads. See `tests/common/mod.rs`.
fn compile_source(source: &str) -> Result<String, CompileError> {
    let driver = Driver::new();
    driver
        .compile_string(source, &unique_source_name("cct"))
        .map(|path| std::fs::read_to_string(path).unwrap_or_else(|_| String::new()))
}

/// Compile and verify the C output contains every expected pattern.
fn compile_and_verify(source: &str, expected_patterns: &[&str]) {
    let result = compile_source(source);
    assert!(
        result.is_ok(),
        "Failed to compile:\n{}\nError: {}",
        source,
        result
            .as_ref()
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
    );

    let output = result.unwrap();
    for pattern in expected_patterns {
        assert!(
            output.contains(pattern),
            "Output missing '{}' for source:\n{}\nGenerated:\n{}",
            pattern,
            source,
            output
        );
    }
}

/// Assert that a source fails to compile, and that the message says why.
fn compile_error_contains(source: &str, needle: &str) {
    match compile_source(source) {
        Ok(c) => panic!(
            "Expected an error for:\n{}\nbut it compiled to:\n{}",
            source, c
        ),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains(needle),
                "Error for:\n{}\nshould mention '{}', got: {}",
                source,
                needle,
                msg
            );
        }
    }
}

#[test]
fn test_all_keywords() {
    let test_cases = vec![
        // Core keywords
        ("fn main() { }", vec!["int main("]),
        ("fn main() { let x = 5; }", vec!["long long x = 5;"]),
        ("fn main() { if true { } }", vec!["if (1)"]),
        ("fn main() { if true { } else { } }", vec!["if (1)", "else"]),
        ("fn main() { while true { } }", vec!["while (1)"]),
        (
            "fn main() { for i in 0..10 { } }",
            vec!["for (long long i = 0; i < 10; i++)"],
        ),
        (
            "fn foo() -> int { return 42; }\nfn main() { }",
            vec!["long long foo()", "return 42;"],
        ),
        (
            "struct Point { x: int, y: int }\nfn main() { }",
            vec!["struct Point", "long long x;", "long long y;"],
        ),
        (
            "enum Option { Some, None }\nfn main() { }",
            vec!["typedef enum"],
        ),
        // Advanced keywords
        ("pub fn main() { }", vec!["int main("]),
        ("fn main() { let mut z = 0; }", vec!["long long z = 0;"]),
        (
            "fn main() { let t = true; let f = false; }",
            vec!["int t = 1;", "int f = 0;"],
        ),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

#[test]
#[ignore = "XFAIL: literal patterns in `match` — grammar.ebnf:269 'No literal patterns'; the parser reports \"Expected pattern, but found integer 1\" (owned by M2, item 4)"]
fn test_match_on_integer_literal() {
    compile_and_verify("fn main() { match 1 { 1 => {}, _ => {} } }", &["switch"]);
}

#[test]
#[ignore = "XFAIL: `trait` emits no C at all — grammar.ebnf:134 'Traits also emit no code'; there is no vtable mechanism anywhere in the compiler (owned by M4, 'Traits with real dispatch')"]
fn test_trait_declaration_emits_code() {
    compile_and_verify("trait Display { }\nfn main() { }", &["// Trait:"]);
}

#[test]
#[ignore = "XFAIL: top-level `const` items — grammar.ebnf:97-98 lists no const item, so the parser reports \"Expected function, struct, enum, trait, type, impl, or macro declaration\" (owned by M2, surface syntax)"]
fn test_top_level_const() {
    compile_and_verify(
        "const X: int = 5;\nfn main() { }",
        &["const long long X = 5;"],
    );
}

#[test]
#[ignore = "XFAIL: top-level `static` items — grammar.ebnf:97-98 lists no static item, same parse error as `const` (owned by M2, surface syntax)"]
fn test_top_level_static() {
    compile_and_verify(
        "static Y: int = 10;\nfn main() { }",
        &["static long long Y = 10;"],
    );
}

#[test]
#[ignore = "XFAIL: `type` aliases parse and then emit nothing — no typedef reaches the C (owned by M4, part of making the type system real)"]
fn test_type_alias_emits_typedef() {
    compile_and_verify(
        "type Int = int;\nfn main() { }",
        &["typedef long long Int;"],
    );
}

#[test]
#[ignore = "XFAIL: `loop` — grammar.ebnf:206 'and no `loop`'; the token does not exist, so the body's '{' is a parse error (owned by M2, item 3)"]
fn test_loop_keyword() {
    compile_and_verify("fn main() { loop { break; } }", &["while (1)", "break;"]);
    compile_and_verify(
        "fn main() { loop { continue; } }",
        &["while (1)", "continue;"],
    );
}

#[test]
#[ignore = "XFAIL: `as` casts — grammar.ebnf:251 'no `as` casts'; the parser stops at 'as' (owned by M2, surface syntax)"]
fn test_as_cast() {
    compile_and_verify("fn main() { let x = 5 as int; }", &["(long long)5"]);
}

#[test]
fn test_arithmetic_operators() {
    // Variables, not literals: the optimizer folds constant arithmetic away
    // before codegen, so `1 + 2` would never appear as `(1 + 2)` in the C.
    compile_and_verify(
        "fn main() { let a = 1; let b = 2; let x = a + b; }",
        &["(a + b)"],
    );
    compile_and_verify(
        "fn main() { let a = 3; let b = 1; let x = a - b; }",
        &["(a - b)"],
    );
    compile_and_verify(
        "fn main() { let a = 2; let b = 3; let x = a * b; }",
        &["(a * b)"],
    );
    compile_and_verify(
        "fn main() { let a = 6; let b = 2; let x = a / b; }",
        &["(a / b)"],
    );
    compile_and_verify(
        "fn main() { let a = 5; let b = 2; let x = a % b; }",
        &["(a % b)"],
    );
}

#[test]
fn test_comparison_operators() {
    for (op, expected) in [
        ("==", "(a == b)"),
        ("!=", "(a != b)"),
        ("<", "(a < b)"),
        (">", "(a > b)"),
        ("<=", "(a <= b)"),
        (">=", "(a >= b)"),
    ] {
        compile_and_verify(
            &format!("fn main() {{ let a = 1; let b = 2; let x = a {} b; }}", op),
            &[expected],
        );
    }
}

#[test]
fn test_logical_operators() {
    compile_and_verify(
        "fn main() { let t = true; let f = false; let x = t && f; }",
        &["(t && f)"],
    );
    compile_and_verify(
        "fn main() { let t = true; let f = false; let x = t || f; }",
        &["(t || f)"],
    );
    compile_and_verify("fn main() { let t = true; let x = !t; }", &["(!(t))"]);
}

#[test]
#[ignore = "XFAIL: bitwise operators — the expression grammar (grammar.ebnf:215-223) has no `&`/`|`/`^`/`<<`/`>>` level and the lexer has no '^' token at all (owned by M2, surface syntax)"]
fn test_bitwise_operators() {
    for (op, expected) in [
        ("&", "(a & b)"),
        ("|", "(a | b)"),
        ("^", "(a ^ b)"),
        ("<<", "(a << b)"),
        (">>", "(a >> b)"),
    ] {
        compile_and_verify(
            &format!("fn main() {{ let a = 1; let b = 2; let x = a {} b; }}", op),
            &[expected],
        );
    }
}

#[test]
#[ignore = "XFAIL: compound assignment (`+=`, `-=`, `*=`, `/=`, `%=`) — missing lexer tokens, so `x += 1` parses as `x +` then fails on '=' (owned by M2, item 3)"]
fn test_compound_assignment_operators() {
    for (op, expected) in [
        ("+=", "x = x + 1;"),
        ("-=", "x = x - 1;"),
        ("*=", "x = x * 1;"),
        ("/=", "x = x / 1;"),
        ("%=", "x = x % 1;"),
    ] {
        compile_and_verify(
            &format!("fn main() {{ let mut x = 6; x {} 1; }}", op),
            &[expected],
        );
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
        (
            r#"fn main() { print("hello world"); }"#,
            vec![r#""hello world""#],
        ),
        (
            r#"fn main() { print("hello\nworld"); }"#,
            vec![r#""hello\nworld""#],
        ),
        (
            r#"fn main() { print("hello\tworld"); }"#,
            vec![r#""hello\tworld""#],
        ),
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
            "fn main() { let a = 5; let b = 3; if a > b { print(\"yes\"); } }",
            vec!["if ((a > b))", "__pd_print"],
        ),
        (
            "fn main() { let a = 1; let b = 2; if a < b { print(\"a\"); } else { print(\"b\"); } }",
            vec!["if ((a < b))", "else", "__pd_print"],
        ),
        // While loops
        (
            "fn main() {
                let mut i = 0;
                while i < 10 {
                    i = i + 1;
                }
            }",
            vec!["while ((i < 10))", "i = (i + 1);"],
        ),
        // For loops
        (
            "fn main() {
                for i in 0..10 {
                    print_int(i);
                }
            }",
            vec!["for (long long i = 0; i < 10; i++)", "__pd_print_int"],
        ),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

#[test]
#[ignore = "XFAIL: `else if` — grammar.ebnf:198 'There is NO `else if`: after `else` the parser requires \\'{\\'' (owned by M2, item 2)"]
fn test_else_if_chain() {
    compile_and_verify(
        "fn main() {
            let a = 1;
            let b = 2;
            if a > b { print(\"a\"); } else if b > a { print(\"b\"); } else { print(\"c\"); }
        }",
        &["if ((a > b))", "else if ((b > a))", "else"],
    );
}

#[test]
fn test_functions() {
    let test_cases = vec![
        // Basic functions
        (
            "fn add(x: int, y: int) -> int { x + y }\nfn main() { }",
            vec!["long long add(long long x, long long y)", "return (x + y);"],
        ),
        (
            "fn greet(name: String) { print(name); }\nfn main() { }",
            vec!["void greet(const char* name)", "__pd_print"],
        ),
        (
            "fn get_value() -> int { 42 }\nfn main() { }",
            vec!["long long get_value()", "return 42;"],
        ),
        // Function calls. Upstream this case used `double`, which collides with
        // the C keyword and emits `long long double(long long x)` — not valid C.
        // Renaming it here is not a fix: that defect is pinned separately by
        // e2e_test::test_c_keyword_identifier_still_links, which *links* the
        // output instead of grepping it. This case is about call codegen, so it
        // uses a name that is not simultaneously a bug report.
        (
            "fn triple(x: int) -> int { x * 3 }
             fn main() { let y = triple(21); }",
            vec!["long long triple(long long x)", "long long y = triple(21);"],
        ),
        // Recursive functions
        (
            "fn factorial(n: int) -> int {
                if n <= 1 { return 1; }
                return n * factorial(n - 1);
             }
             fn main() { }",
            vec![
                "long long factorial(long long n)",
                "factorial((n - 1))",
                "return (n * factorial((n - 1)));",
            ],
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
            "struct Point { x: int, y: int }\nfn main() { }",
            vec!["struct Point {", "long long x;", "long long y;"],
        ),
        // Associated function in an impl block
        (
            "struct Point { x: int, y: int }
             impl Point {
                 fn new(x: int, y: int) -> Point {
                     Point { x: x, y: y }
                 }
             }
             fn main() { }",
            vec![
                "struct Point",
                "struct Point __pd_Point_new(long long x, long long y)",
            ],
        ),
        // Using structs
        (
            "struct Point { x: int, y: int }
             fn main() {
                 let p = Point { x: 10, y: 20 };
                 print_int(p.x);
             }",
            vec!["struct Point p =", "p.x"],
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
            vec!["long long arr[5] =", "{1, 2, 3, 4, 5}"],
        ),
        // Array access
        (
            "fn main() {
                let arr = [10, 20, 30];
                let x = arr[1];
            }",
            vec!["arr[1]"],
        ),
        // Array in loops
        (
            "fn main() {
                let arr = [1, 2, 3, 4, 5];
                for i in 0..5 {
                    print_int(arr[i]);
                }
            }",
            vec!["for (long long i = 0; i < 5; i++)", "arr[i]"],
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
            "enum Color { Red, Green, Blue }\nfn main() { }",
            vec![
                "typedef enum",
                "__Color__Red",
                "__Color__Green",
                "__Color__Blue",
            ],
        ),
        // Enum with a payload
        (
            "enum Option { Some(int), None }\nfn main() { }",
            vec!["typedef enum", "Option__Some_Data"],
        ),
        // Match on enum. The C backend lowers a match to a tag comparison
        // chain, not to a `switch`.
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
            vec![
                "_match_expr.tag == __Color__Red",
                "_match_expr.tag == __Color__Green",
                "__pd_print",
            ],
        ),
    ];

    for (source, patterns) in test_cases {
        compile_and_verify(source, &patterns);
    }
}

/// Each case asserts *why* it fails, not just that it fails. Half of these
/// used to be declaration-only fragments, so they were all rejected with
/// "No main function found" and proved nothing about the type checker.
#[test]
fn test_error_cases() {
    let error_cases = vec![
        // Type errors
        (
            "fn main() { let x: int = \"hello\"; }",
            "expected Int, found String",
        ),
        (
            "fn main() { let x: String = 42; }",
            "expected String, found Int",
        ),
        (
            "fn main() { let x = 1 + \"hello\"; }",
            "expected Int, found String",
        ),
        // Undefined variables
        (
            "fn main() { print(undefined_var); }",
            "Undefined variable or function: 'undefined_var'",
        ),
        // Undefined functions
        (
            "fn main() { undefined_func(); }",
            "Undefined function: undefined_func",
        ),
        // Wrong number of arguments
        (
            "fn add(x: int, y: int) -> int { x + y }
             fn main() { add(1); }",
            "expects 2 arguments, but 1 were provided",
        ),
        // Type mismatch in a function's tail expression
        (
            "fn get_int() -> int { \"not an int\" }
             fn main() { }",
            "expected Int, found String",
        ),
        // Invalid control flow
        ("fn main() { if 42 { } }", "expected Bool, found Int"),
        (
            "fn main() { while \"not bool\" { } }",
            "expected Bool, found String",
        ),
    ];

    for (source, reason) in error_cases {
        compile_error_contains(source, reason);
    }
}

// PAID. The `#[ignore = "XFAIL: …"]` this carried is gone and the row in
// tests/rust-debt-manifest.txt moved `owed M1 …` -> `paid - -`; leaving the
// attribute on a passing test is an XPASS and `make test-xfail` reports it.
//
// The reason it carried said the rest "needs a real flow analysis over the
// whole body". That turned out to be an overestimate of the work and an
// underestimate of what was already there: `returns_on_every_path` in
// src/parser/mod.rs had been deciding exactly this question since D3b, and the
// call site simply did not act on a `false` when no value had been written in
// tail position. The declared return type is the evidence that was said to be
// missing. See `CompileError::missing_return`.
//
// This test is the END-TO-END statement of the refusal and nothing more — one
// program, one word in the message. The path shapes (empty body, `if` with no
// `else` last, a loop that may not run) and, more importantly, the ACCEPT side
// the refusal must not touch are receipted in tests/m1_missing_return.rs.
#[test]
fn test_missing_return_is_an_error() {
    compile_error_contains(
        "fn get_value() -> int { }
         fn main() { }",
        "return",
    );
}

/// A program with no `main` is not a program. This is the reason the
/// declaration-only fragments above all carry a `fn main() { }`.
#[test]
fn test_missing_main_is_an_error() {
    compile_error_contains("struct Point { x: int, y: int }", "No main function found");
}

#[test]
fn test_operator_precedence() {
    let test_cases = vec![
        // Arithmetic precedence
        (
            "fn main() { let a = 1; let b = 2; let c = 3; let x = a + b * c; }",
            vec!["(a + (b * c))"],
        ),
        (
            "fn main() { let a = 1; let b = 2; let c = 3; let x = (a + b) * c; }",
            vec!["((a + b) * c)"],
        ),
        // Comparison and logical
        (
            "fn main() { let a = 1; let b = 2; let c = 3; let d = 4; let x = a < b && c < d; }",
            vec!["((a < b) && (c < d))"],
        ),
        (
            "fn main() { let a = 1; let b = 2; let c = 3; let d = 4; let x = a + b < c * d; }",
            vec!["((a + b) < (c * d))"],
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
                return n;
            }
            return fib(n - 1) + fib(n - 2);
        }

        fn main() {
            let result = fib(10);
            print_int(result);
        }
        "#,
        &["long long fib(long long n)", "fib((n - 1))", "fib((n - 2))"],
    );

    // Bubble sort. `let temp: i64` is required, not decoration: indexing an
    // array-typed *parameter* has no inference rule, and the compiler says so.
    // `mut` is required to write through it: a bare `[T; N]` parameter decays to a
    // pointer into the caller's array, and whether that is a copy or an alias is an
    // open question in the specification (§A9.2), so codegen refuses rather than guess.
    // rather than guessing.
    compile_and_verify(
        r#"
        fn bubble_sort(mut arr: [int; 10], n: int) {
            for i in 0..n {
                for j in 0..(n - i - 1) {
                    if arr[j] > arr[j + 1] {
                        let temp: i64 = arr[j];
                        arr[j] = arr[j + 1];
                        arr[j + 1] = temp;
                    }
                }
            }
        }
        fn main() { }
        "#,
        &[
            "void bubble_sort",
            "for (long long i = 0;",
            "long long temp = arr[j];",
        ],
    );
}

#[test]
#[ignore = "XFAIL: an `impl` block's `Self` is not a known type — this fixture is refused with 'Unknown struct type: Self' before it reaches the `&self` receiver and the method-call syntax (`x.f()`, grammar.ebnf:248, rejected as \"Indirect function calls not yet supported\") it was written for. All three are owed; this is the one that fires (owned by M4, 'Traits with real dispatch' / a real reference type)"]
fn test_struct_with_methods() {
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
        }

        fn main() {
            let rect = Rectangle::new(10, 20);
            print_int(rect.area());
        }
        "#,
        &["struct Rectangle", "area", "rect.area()"],
    );
}
