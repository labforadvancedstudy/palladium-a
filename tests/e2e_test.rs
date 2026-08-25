// End-to-end tests: drive the real `pdc` binary, then inspect what it wrote.
//
// All four of these used to fail on "C file not generated" while the
// compilation they had just asserted succeeded. They were looking in
// `target/build/`, which only ever holds the linked *executable*
// (`src/driver/mod.rs:274`); the generated C goes to `build_output/`
// (`src/codegen/mod.rs:6190-6201`). The path was the whole bug.
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

/// The same, but with `-o`, which makes the driver hand the generated C to gcc.
///
/// Two other helpers do compile the emitted C and reject a failure —
/// `advanced_e2e_test.rs`'s and `advanced_features_test.rs`'s `compile_and_run`
/// both shell out to `cc`. But every test in both files is XFAIL and dies at
/// parse or type-check, so none of them ever reaches `cc`: today they provide
/// no positive evidence that ordinary emitted C compiles. Everything else in
/// the suite greps the C for substrings, which proves a fragment was emitted,
/// not that the file is valid C. So this path is currently the suite's only
/// *executed* check of that, and it is why the C-keyword defect below could sit
/// under a "passing" test.
fn compile_and_link_with_pdc(module: &str, source: &str) -> std::process::Output {
    let test_dir = Path::new("target/e2e_tests");
    fs::create_dir_all(test_dir).unwrap();

    let source_path = test_dir.join(format!("{}.pd", module));
    fs::write(&source_path, source).unwrap();

    Command::new("./target/release/pdc")
        .arg("compile")
        .arg(&source_path)
        .arg("-o")
        .arg(module)
        .output()
        .expect("Failed to execute pdc")
}

/// Extract the body of a generated C function, so an assertion can be made
/// about that function rather than about the whole file (which always contains
/// the runtime prelude, and therefore always contains the word `return`).
fn c_function_body<'a>(c_source: &'a str, signature: &str) -> &'a str {
    let start = c_source
        .find(&format!("{} {{", signature))
        .unwrap_or_else(|| panic!("no definition of `{}` in the generated C", signature));
    let after_brace = start + c_source[start..].find('{').unwrap() + 1;
    let mut depth = 1usize;
    for (i, ch) in c_source[after_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &c_source[after_brace..after_brace + i];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated body for `{}`", signature);
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

// --- M1 defects, PAID -------------------------------------------------------
// This was a silent miscompile found while repairing this suite. It is no
// longer #[ignore]d and its row in tests/rust-debt-manifest.txt moved
// `owed M1 …` -> `paid - -`; leaving the attribute on a passing test is an
// XPASS and `make test-xfail` reports it.

/// `fn double(…)` used to emit `long long double(long long x)`, which is not
/// valid C — `double` is a C type specifier. Nothing else in the suite could
/// catch it: every other test greps the C text, and the text was exactly what
/// was asked for. Only handing it to gcc failed, and only `-o` does that.
///
/// Fixed by `src/codegen/c_ident.rs`, which renames reserved words on the way
/// into code generation (`double` -> `double_`). This test is kept as the
/// END-TO-END statement — the real binary, with `-o`, so gcc actually runs. The
/// other positions a keyword can appear in, the injectivity of the escape, and
/// the controls on what must NOT be renamed are in
/// `tests/m1_c_keyword_idents.rs`.
#[test]
fn test_c_keyword_identifier_still_links() {
    let module = unique_module_name("ckeyword");
    let output = compile_and_link_with_pdc(
        &module,
        r#"
fn double(x: i64) -> i64 {
    return x * 2;
}

fn main() {
    print_int(double(21));
}
"#,
    );

    assert!(
        output.status.success(),
        "a function named after a C keyword must still produce valid C:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// D3b, closed. A function whose body ends in a tail `if` used to emit no
/// `return` at all:
///
///     long long fib(long long n) {
///         if ((n <= 1)) { n; } else { (fib((n - 1)) + fib((n - 2))); }
///     }
///
/// The parser lowered a tail *expression* to `Stmt::Return`, but not a tail
/// `if`, so the caller read whatever was in the return register. gcc only
/// warns, so nothing in the pipeline stopped it.
///
/// The `#[ignore = "XFAIL: …"]` this test used to carry is gone: the fix is in
/// `src/parser/mod.rs` (`lower_tail_to_return`), so an ignored test here would
/// be a stale expectation and `make test-xfail` reports that as an XPASS.
///
/// The assertion is deliberately narrow — one fixture, one function. The
/// general invariant ("every non-void function's body must definitely return on
/// every path", as a terminator analysis over the emitted C, which also covers
/// tail `match` and constructs nobody has hit yet) landed as
/// `scripts/check-c-returns.py` and runs over every `tests/stdlib/` driver in
/// `make stdlib-gate`. This test is kept anyway because it is the *end-to-end*
/// statement — .pd in, C out, one named function — and it costs one compile.
#[test]
fn test_tail_if_function_emits_a_return() {
    let module = unique_module_name("tailif");
    let output = compile_with_pdc(
        &module,
        r#"
fn fib(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn main() {
    print_int(fib(10));
}
"#,
    );

    assert!(
        output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let c_source = fs::read_to_string(generated_c(&module)).unwrap();
    let body = c_function_body(&c_source, "long long fib(long long n)");
    assert!(
        body.contains("return"),
        "`fib` ends in a tail `if` and emits no return at all:\n{}",
        body
    );
}
