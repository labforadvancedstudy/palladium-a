//! #89 and #92: a comparison operator on a type that has no comparison.
//!
//! THE DEFECT. The type checker admitted `==` `!=` `<` `<=` `>` `>=` between
//! any two operands that merge to one type, and code generation emitted C's
//! operator between them whatever they were. Two outcomes, both measured on
//! d5166e8, neither a diagnostic of this compiler's:
//!
//!   * gcc refused the C (`invalid operands to binary expression ('struct K'
//!     and 'struct K')`, pdc exit 3) for a tuple, a struct, an enum with or
//!     without payloads, a generic struct, a range and a unit-returning call;
//!   * gcc ACCEPTED the C and the program answered wrong, silently: a String is
//!     a `const char*`, so `int_to_string(12) == "12"` was 0 and `"b" < "a"`
//!     was 1; an array decays to its address, so `[1, 2] == [1, 2]` was 0; a
//!     function name compared addresses.
//!
//! No requirement row and no specification text defines a comparison on any
//! of these (N5-10 is witnessed over the primitives only), so each is now
//! REFUSED BY NAME (PD0082) with the way to compare instead. Nothing invents a
//! structural equality. The types that did compare correctly — the integers,
//! the floats, `bool`, `char`, aliases of those, a reference PARAMETER (code
//! generation reads through it) — keep exactly the output they had, which the
//! controls at the bottom pin.
//!
//! A GENERIC BODY IS NEVER TYPE-CHECKED (`check_function` returns early for
//! one), so `fn same<T>(a: T, c: T) -> bool { a == c }` reached gcc at a
//! struct and answered 0 at two equal Strings. Each instantiation's body is
//! now checked, on a copy of the checker, at the types it was instantiated
//! with, for this rule alone.

mod common;

use common::unique_module_name;
use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Compile, link, run — and return stdout. Any failure on the way is the
/// test's failure, named at the step it happened.
fn run(source: &str, prefix: &str) -> String {
    let name = unique_module_name(prefix);
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    let exe = dir.path().join(&name);
    fs::write(&src, source).unwrap();

    let c_file = Driver::new()
        .compile_file(&src)
        .unwrap_or_else(|e| panic!("the front end refused a legal program: {}", e));
    let out = link_command(&c_file, &exe, OptLevel::Default)
        .expect("link_command")
        .output()
        .expect("gcc");
    assert!(
        out.status.success(),
        "the front end accepted this and gcc refused the C it emitted: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let run = Command::new(&exe).output().expect("run");
    assert!(
        run.status.success(),
        "the program did not exit 0: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).to_string()
}

/// Compile only, expecting THIS compiler's refusal under PD0082 — not gcc's,
/// and not an incidental refusal that happens to share some words. Returns the
/// message so a test can read which type it named and what it advised.
fn refused(source: &str, prefix: &str) -> String {
    let name = unique_module_name(prefix);
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    match Driver::new().compile_file(&src) {
        Ok(c) => panic!(
            "the front end accepted a comparison it must refuse: {}",
            c.display()
        ),
        Err(e) => {
            let text = e.to_string();
            assert!(
                !text.contains("gcc"),
                "the refusal came from the C compiler, not from this one: {}",
                text
            );
            assert_eq!(
                e.code().map(|c| c.to_string()),
                Some("PD0082".to_string()),
                "refused, but not under the comparison rule: {}",
                text
            );
            text
        }
    }
}

// ---------------------------------------------------------------------------
// #89 — tuples.
// ---------------------------------------------------------------------------

/// The issue's own program. On d5166e8: gcc, `invalid operands to binary ==`.
#[test]
fn a_tuple_equality_is_refused_by_name() {
    let text = refused(
        "fn main() { let a = (1, 2); let b = (1, 2); \
         if a == b { print_int(1); } else { print_int(0); } }",
        "cmp_tuple_eq",
    );
    assert!(text.contains("`==` is not defined on `(Int, Int)`"), "{}", text);
    assert!(text.contains("element by element"), "{}", text);
}

/// Every comparison operator is the one rule, not only `==`.
#[test]
fn every_comparison_operator_on_a_tuple_is_refused() {
    for op in ["==", "!=", "<", "<=", ">", ">="] {
        let text = refused(
            &format!(
                "fn main() {{ let a = (1, 2); let b = (1, 3); \
                 if a {} b {{ print_int(1); }} else {{ print_int(0); }} }}",
                op
            ),
            "cmp_tuple_ops",
        );
        assert!(
            text.contains(&format!("`{}` is not defined on `(Int, Int)`", op)),
            "{}",
            text
        );
    }
}

/// An alias names the type it aliases, so it has that type's comparison —
/// here none. On d5166e8: gcc.
#[test]
fn a_tuple_alias_is_refused_too() {
    refused(
        "type Pair = (i64, i64);\n\
         fn main() { let a: Pair = (1, 2); let b: Pair = (1, 2); \
         if a == b { print_int(1); } else { print_int(0); } }",
        "cmp_tuple_alias",
    );
}

// ---------------------------------------------------------------------------
// #92 — enums and structs.
// ---------------------------------------------------------------------------

/// The issue's enum. On d5166e8: gcc, `invalid operands to binary expression
/// ('struct K' and 'struct K')`, pdc exit 3.
#[test]
fn a_fieldless_enum_equality_is_refused_by_name() {
    let text = refused(
        "enum K { A, B }\n\
         fn main() { let a = K::A; let b = K::A; \
         if a == b { print_int(1); } else { print_int(0); } }",
        "cmp_enum_eq",
    );
    assert!(text.contains("`==` is not defined on `K`"), "{}", text);
    assert!(text.contains("`match`"), "{}", text);
}

#[test]
fn an_enum_with_a_payload_is_refused_too() {
    let text = refused(
        "enum E { A(i64), B }\n\
         fn main() { let a = E::A(1); let b = E::A(1); \
         if a != b { print_int(1); } else { print_int(0); } }",
        "cmp_enum_payload",
    );
    assert!(text.contains("`!=` is not defined on `E`"), "{}", text);
}

#[test]
fn a_struct_equality_is_refused_by_name() {
    let text = refused(
        "struct P { x: i64, y: i64 }\n\
         fn main() { let a = P { x: 1, y: 2 }; let b = P { x: 1, y: 2 }; \
         if a == b { print_int(1); } else { print_int(0); } }",
        "cmp_struct_eq",
    );
    assert!(text.contains("`==` is not defined on `P`"), "{}", text);
    assert!(text.contains("field by field"), "{}", text);
}

/// A generic struct's instantiation is a struct. On d5166e8: gcc.
#[test]
fn a_generic_struct_instantiation_is_refused() {
    let text = refused(
        "struct W<T> { v: T }\n\
         fn main() { let a = W { v: 1 }; let b = W { v: 1 }; \
         if a == b { print_int(1); } else { print_int(0); } }",
        "cmp_generic_struct",
    );
    assert!(text.contains("field by field"), "{}", text);
}

// ---------------------------------------------------------------------------
// The well-typed-but-wrong half: C accepted these and answered wrong.
// ---------------------------------------------------------------------------

/// MEASURED on d5166e8: `int_to_string(12) == "12"` printed 0. The C was
/// `(s == t)` over two `const char*`, which is right only when gcc happens to
/// pool two identical literals into one address.
#[test]
fn a_string_equality_is_refused_and_points_at_string_eq() {
    let text = refused(
        "fn main() { if int_to_string(12) == \"12\" { print_int(1); } else { print_int(0); } }",
        "cmp_string_eq",
    );
    assert!(text.contains("`==` is not defined on `String`"), "{}", text);
    assert!(text.contains("string_eq"), "{}", text);
}

/// MEASURED on d5166e8: `"b" < "a"` printed 1 — an address order.
#[test]
fn a_string_ordering_is_refused() {
    let text = refused(
        "fn main() { let s = \"b\"; let t = \"a\"; \
         if s < t { print_int(1); } else { print_int(0); } }",
        "cmp_string_lt",
    );
    assert!(text.contains("`<` is not defined on `String`"), "{}", text);
}

/// MEASURED on d5166e8: `[1, 2] == [1, 2]` printed 0 — both arrays decayed to
/// their addresses.
#[test]
fn an_array_equality_is_refused_by_name() {
    let text = refused(
        "fn main() { let a = [1, 2]; let b = [1, 2]; \
         if a == b { print_int(1); } else { print_int(0); } }",
        "cmp_array_eq",
    );
    assert!(text.contains("`==` is not defined on `[Int; 2]`"), "{}", text);
    assert!(text.contains("addresses"), "{}", text);
}

/// `fn(A) -> B` is unimplemented (language-spec, the type table), so a
/// function name in a comparison is an accident of name lookup, and C
/// compared the two functions' addresses.
#[test]
fn a_function_name_comparison_is_refused() {
    refused(
        "fn f() -> i64 { 1 }\nfn g() -> i64 { 2 }\n\
         fn main() { if f == g { print_int(1); } else { print_int(0); } }",
        "cmp_fn",
    );
}

/// A range has no operation beyond being iterated (N5-14). On d5166e8: gcc.
#[test]
fn a_range_comparison_is_refused() {
    refused(
        "fn main() { let r = 0..3; let s = 0..3; \
         if r == s { print_int(1); } else { print_int(0); } }",
        "cmp_range",
    );
}

/// Two unit-returning calls. On d5166e8: gcc compared two `void` values.
#[test]
fn a_unit_comparison_is_refused() {
    refused(
        "fn u() { }\nfn main() { if u() == u() { print_int(1); } else { print_int(0); } }",
        "cmp_unit",
    );
}

// ---------------------------------------------------------------------------
// Every POSITION a comparison is type-checked in — one rule, so one site in
// the checker, and these prove each position reaches it.
// ---------------------------------------------------------------------------

#[test]
fn a_match_guard_is_checked() {
    refused(
        "enum K { A, B }\n\
         fn main() { let k = K::A; let x = 3; \
         match x { n if k == K::A => { print_int(n); } _ => { print_int(0); } } }",
        "cmp_guard",
    );
}

#[test]
fn a_while_condition_is_checked() {
    refused(
        "struct P { x: i64 }\n\
         fn main() { let a = P { x: 1 }; let b = P { x: 2 }; let mut i = 0; \
         while a != b { i = i + 1; if i > 2 { break; } } print_int(i); }",
        "cmp_while",
    );
}

#[test]
fn compound_positions_are_checked() {
    for body in [
        "let c = a == K::B; if c { print_int(1); }",
        "if !(a == K::B) { print_int(1); }",
        "if true && a == K::A { print_int(1); }",
        "print_int(pick(a == K::A));",
    ] {
        refused(
            &format!(
                "enum K {{ A, B }}\nfn pick(b: bool) -> i64 {{ if b {{ 1 }} else {{ 0 }} }}\n\
                 fn main() {{ let a = K::A; {} }}",
                body
            ),
            "cmp_compound",
        );
    }
}

#[test]
fn a_return_value_is_checked() {
    refused(
        "struct P { x: i64 }\n\
         fn eq(a: P, b: P) -> bool { return a == b; }\n\
         fn main() { if eq(P { x: 1 }, P { x: 1 }) { print_int(1); } else { print_int(0); } }",
        "cmp_return",
    );
}

/// A struct field that is itself a struct, and a tuple element that is a
/// tuple: the operand's type decides, wherever it came from.
#[test]
fn a_projected_composite_is_checked() {
    refused(
        "struct I { v: i64 }\nstruct O { i: I }\n\
         fn main() { let a = O { i: I { v: 1 } }; let b = O { i: I { v: 1 } }; \
         if a.i == b.i { print_int(1); } else { print_int(0); } }",
        "cmp_field_struct",
    );
    refused(
        "fn main() { let a = ((1, 2), 3); let b = ((1, 2), 3); \
         if a.0 == b.0 { print_int(1); } else { print_int(0); } }",
        "cmp_tuple_elem_tuple",
    );
}

/// A macro body is re-parsed and type-checked at its call site, so `<` there
/// (a single-character operator, which the token stream does carry) meets the
/// same rule. On d5166e8 a tuple reached gcc and an array answered 0.
#[test]
fn a_macro_body_is_checked() {
    refused(
        "macro lt!(a, b) { $a < $b }\n\
         fn main() { let x = (1, 2); let y = (1, 3); \
         if lt!(x, y) { print_int(1); } else { print_int(0); } }",
        "cmp_macro_tuple",
    );
    refused(
        "macro lt!(a, b) { $a < $b }\n\
         fn main() { let x = [1, 2]; let y = [1, 2]; \
         if lt!(x, y) { print_int(1); } else { print_int(0); } }",
        "cmp_macro_array",
    );
}

// ---------------------------------------------------------------------------
// Generic bodies, checked at each instantiation.
// ---------------------------------------------------------------------------

/// On d5166e8: gcc, `invalid operands` inside `same_P`.
#[test]
fn a_generic_body_instantiated_at_a_struct_is_refused() {
    let text = refused(
        "struct P { x: i64 }\n\
         fn same<T>(a: T, c: T) -> bool { a == c }\n\
         fn main() { let p = P { x: 1 }; let q = P { x: 1 }; \
         if same(p, q) { print_int(1); } else { print_int(0); } }",
        "cmp_generic_struct_arg",
    );
    assert!(text.contains("`same`"), "{}", text);
    assert!(text.contains("T = P"), "{}", text);
    assert!(text.contains("`==` is not defined on `P`"), "{}", text);
}

/// On d5166e8 this printed 0: two equal Strings, two addresses.
#[test]
fn a_generic_body_instantiated_at_a_string_is_refused() {
    let text = refused(
        "fn same<T>(a: T, c: T) -> bool { a == c }\n\
         fn main() { let p = \"ab\"; let q = string_concat(\"a\", \"b\"); \
         if same(p, q) { print_int(1); } else { print_int(0); } }",
        "cmp_generic_string_arg",
    );
    assert!(text.contains("T = String"), "{}", text);
}

/// The generic is defined AFTER the call that instantiates it, and the enum
/// case: the check runs once every body has been walked, so order is not a
/// way around it.
#[test]
fn a_generic_defined_after_its_caller_is_refused_at_an_enum() {
    refused(
        "enum K { A, B }\n\
         fn main() { let p = K::A; let q = K::A; \
         if same(p, q) { print_int(1); } else { print_int(0); } }\n\
         fn same<T>(a: T, c: T) -> bool { a == c }",
        "cmp_generic_after",
    );
}

/// An element of a `[T; 2]` parameter is a `T`.
#[test]
fn a_generic_array_element_is_checked() {
    refused(
        "struct P { x: i64 }\n\
         fn first_same<T>(a: [T; 2], c: [T; 2]) -> bool { a[0] == c[0] }\n\
         fn main() { let p = [P { x: 1 }, P { x: 2 }]; let q = [P { x: 1 }, P { x: 2 }]; \
         if first_same(p, q) { print_int(1); } else { print_int(0); } }",
        "cmp_generic_elem",
    );
}

// ---------------------------------------------------------------------------
// CONTROLS — what compared correctly on d5166e8 still does, with the same
// output. Each line's expected digit is the one d5166e8 printed.
// ---------------------------------------------------------------------------

/// Every integer width the checker reaches with a value (`i8`, `i16`, `u8`,
/// `u16`, `usize`, `isize` refuse a literal with PD0042 before any comparison
/// is written), the floats, `bool` and `char`, through aliases too.
#[test]
fn the_scalar_comparisons_keep_their_answers() {
    let out = run(
        r#"
type N = i64;
type B = bool;
type C = char;
type F = f64;
fn b(x: bool) { if x { print_int(1); } else { print_int(0); } }
fn m32(v: i32) -> i32 { v }
fn mu32(v: u32) -> u32 { v }
fn mu64(v: u64) -> u64 { v }
fn main() {
    let a = 3; let c = 4;
    b(a == c); b(a != c); b(a < c); b(a <= c); b(a > c); b(a >= c);
    let i: i32 = m32(5); let j: i32 = m32(7); b(i < j); b(i == j);
    let u: u32 = mu32(4000000000); let v: u32 = mu32(1); b(u > v);
    let w: u64 = mu64(3); let x: u64 = mu64(3); b(w == x);
    let t = true; let f = false; b(t == f); b(f < t); b(t != f);
    let p = 'a'; let q = 'b'; b(p == q); b(p < q); b(q >= 'b');
    let g = 1.5; let h = 2.5; b(g == h); b(g < h); b(h >= 2.5);
    let n: N = 3; b(n == 3); b(n < 4);
    let y: B = true; let z: C = 'q'; let e: F = 2.0; b(y == true); b(z == 'q'); b(e < 3.0);
}
"#,
        "cmp_scalars",
    );
    assert_eq!(
        out,
        "0\n1\n1\n1\n0\n0\n1\n0\n1\n1\n0\n1\n1\n0\n1\n1\n0\n1\n1\n1\n1\n1\n1\n1\n"
    );
}

/// The ways to compare that the refusals point at, and the positions a
/// scalar comparison reaches through: a reference PARAMETER (code generation
/// reads through it), a field, a tuple element, an array element, a `match`
/// on an enum, `string_eq`, a `for` header, a guard and a `while`.
#[test]
fn the_suggested_comparisons_and_scalar_projections_run() {
    let out = run(
        r#"
struct P { x: i64, y: i64 }
enum K { A, B }
fn b(x: bool) { if x { print_int(1); } else { print_int(0); } }
fn eq(a: &i64, c: &i64) -> bool { a == c }
fn is_a(k: K) -> bool { match k { K::A => true, K::B => false } }
fn same<T>(a: T, c: T) -> bool { a == c }
fn main() {
    let x = 5; let y = 5; b(eq(&x, &y));
    let p = P { x: 1, y: 2 }; let q = P { x: 1, y: 2 }; b(p.x == q.x && p.y == q.y);
    let s = (1, 2); let t = (1, 3); b(s.0 == t.0 && s.1 == t.1);
    let u = [1, 2]; let v = [1, 2]; b(u[0] == v[0] && u[1] == v[1]);
    b(is_a(K::A)); b(is_a(K::B));
    b(string_eq(int_to_string(12), "12"));
    b(same(1, 1)); b(same(true, false));
    let mut acc = 0; for i in 0..4 { if i != 2 { acc = acc + i; } } print_int(acc);
    match x { n if n > 2 => { print_int(n); } _ => { print_int(0); } }
    let mut k = 0; while k < 3 { k = k + 1; } print_int(k);
}
"#,
        "cmp_suggested",
    );
    assert_eq!(out, "1\n1\n0\n1\n1\n0\n1\n1\n0\n4\n5\n3\n");
}

/// A string literal PATTERN is the one String equality code generation does
/// implement (`__pd_string_eq`), so it is not this rule's: it still runs, and
/// still answers by characters.
#[test]
fn a_string_literal_pattern_still_compares_characters() {
    let out = run(
        "fn main() { let s = string_concat(\"h\", \"i\"); \
         match s { \"hi\" => { print_int(1); } _ => { print_int(0); } } }",
        "cmp_string_pattern",
    );
    assert_eq!(out, "1\n");
}

// ---------------------------------------------------------------------------
// Review round 1: the instantiated body is the WHOLE body, and another rule's
// refusal does not end the walk. Each program below compiled on 71c4d6f and
// printed 0 for two equal Strings (or reached gcc), with no PD0082.
// ---------------------------------------------------------------------------

/// A generic body over `T`, called at two equal Strings bound to locals (a
/// call expression as the argument is a different, older refusal: the type
/// argument cannot be inferred from it).
fn generic_at_strings(body: &str) -> String {
    format!(
        "fn marker() -> i64 {{ 1 }}\n\
         fn same<T>(a: T, b: T) -> bool {{ {} }}\n\
         fn main() {{ let aa = int_to_string(12); let bb = \"12\"; \
         if same(aa, bb) {{ print_int(1); }} else {{ print_int(0); }} }}",
        body
    )
}

/// `let x: T = a;` — only the signature was substituted, so `x` stayed a
/// type parameter (which compares) while code generation emitted `void* x`.
#[test]
fn a_local_written_with_the_type_parameter_is_instantiated_too() {
    let text = refused(
        &generic_at_strings("let x: T = a; let y: T = b; x == y"),
        "cmp_generic_typed_locals",
    );
    assert!(text.contains("T = String"), "{}", text);
    assert!(text.contains("`==` is not defined on `String`"), "{}", text);
}

/// The walk stopped at the first refusal of ANY rule, and main never checked
/// these bodies, so each of these hid the comparison after it.
#[test]
fn another_rules_refusal_before_the_comparison_does_not_hide_it() {
    for body in [
        "let n: bool = 1; a == b",
        "if false { return 1; } a == b",
        "let n: i8 = 1; a == b",
        "let marker = 1; a == b",
        "if true { let n: bool = 1; return a == b; } false",
        "let n: bool = 1; let c = n; if c { return a != b; } a == b",
    ] {
        let text = refused(&generic_at_strings(body), "cmp_generic_after_refusal");
        assert!(text.contains("T = String"), "{}: {}", body, text);
    }
}

/// A generic called from a generic is instantiated at its caller's types and
/// its body checked in turn. On d5166e8 and 71c4d6f this reached gcc.
#[test]
fn a_generic_called_from_a_generic_is_checked_at_the_callers_types() {
    let text = refused(
        "fn same<T>(a: T, b: T) -> bool { a == b }\n\
         fn forward<T>(a: T, b: T) -> bool { same(a, b) }\n\
         fn main() { let aa = int_to_string(12); let bb = \"12\"; \
         if forward(aa, bb) { print_int(1); } else { print_int(0); } }",
        "cmp_generic_nested",
    );
    assert!(text.contains("in `same`, instantiated with T = String"), "{}", text);
}

/// The other half of "another rule's refusal does not end the walk": it is not
/// RAISED either. At `i64` each of these bodies ran on d5166e8 and printed 1,
/// although the checker would refuse a statement in every one of them.
#[test]
fn another_rules_refusal_in_a_generic_body_is_still_not_raised() {
    for body in [
        "let n: bool = 1; a == b",
        "if false { return 1; } a == b",
        "let marker = 1; a == b",
        "if true { let n: bool = 1; return a == b; } false",
    ] {
        let out = run(
            &format!(
                "fn marker() -> i64 {{ 1 }}\n\
                 fn same<T>(a: T, b: T) -> bool {{ {} }}\n\
                 fn main() {{ if same(3, 3) {{ print_int(1); }} else {{ print_int(0); }} }}",
                body
            ),
            "cmp_generic_other_rule",
        );
        assert_eq!(out, "1\n", "{}", body);
    }
}

/// An imported public body is checked whether or not anything calls it, so its
/// refusal names the module and the function it is in (review round 1).
#[test]
fn an_imported_bodys_refusal_names_its_module_and_function() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("lib2.pd"),
        "pub fn pair_is_one(a: i64) -> bool { let t = (a, a); t == (1, 1) }\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("main.pd"),
        "import lib2;\n\nfn main() {\n    print_int(1);\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .args(["compile", "main.pd", "-o", "prog"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run pdc");
    let text = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success(), "accepted: {}", text);
    assert!(
        text.contains("error[PD0082]") && text.contains("in `lib2::pair_is_one`, imported:"),
        "{}",
        text
    );
}

// ---------------------------------------------------------------------------
// Review round 2: every `let` binds its name, and every operand's type is
// recovered from its form when the checker refuses it under another rule.
// ---------------------------------------------------------------------------

/// `same<T>` with `body`, called after `setup` binds `x` and `y`.
fn generic_with(body: &str, setup: &str) -> String {
    format!(
        "fn pick(s: String) -> String {{ s }}\n\
         fn same<T>(a: T, b: T) -> bool {{ {} }}\n\
         fn main() {{ {} if same(x, y) {{ print_int(1); }} else {{ print_int(0); }} }}",
        body, setup
    )
}

const AT_STRINGS: &str = "let x = int_to_string(12); let y = \"12\";";
const AT_INTS: &str = "let x = 3; let y = 3;";

/// Each operand below is a String whose type the checker could not give —
/// a block, a local inferred from one, an `if`, `loop` or `match` value, a
/// cast, a call whose argument is refused — so on bb324cd the comparison was
/// never judged, and C compared two addresses (0 for two equal Strings).
#[test]
fn an_operand_whose_type_is_recovered_is_judged() {
    for body in [
        "({ let n: bool = 1; a }) == b",
        "a == ({ let n: bool = 1; b })",
        "let v = { let n: bool = 1; a }; v == b",
        "(if true { let n: bool = 1; a } else { b }) == b",
        "(loop { let n: bool = 1; break a; }) == b",
        "(match 1 { _ => { let n: bool = 1; a } }) == b",
        "let v = a as String; v == b",
        "pick({ let n: bool = 1; a }) == b",
    ] {
        let text = refused(&generic_with(body, AT_STRINGS), "cmp_recovered");
        assert!(
            text.contains("T = String") && text.contains("`==` is not defined on `String`"),
            "{}: {}",
            body,
            text
        );
    }
}

/// The controls: the same shapes at `i64` have a comparison, and each printed
/// 1 on d5166e8. Recovery must not turn them into refusals.
#[test]
fn the_same_recovered_operands_at_an_integer_still_run() {
    for body in [
        "({ let n: bool = 1; a }) == b",
        "a == ({ let n: bool = 1; b })",
        "let v = { let n: bool = 1; a }; v == b",
        "(if true { let n: bool = 1; a } else { b }) == b",
        "(loop { let n: bool = 1; break a; }) == b",
        "(match 1 { _ => { let n: bool = 1; a } }) == b",
    ] {
        let out = run(&generic_with(body, AT_INTS), "cmp_recovered_int");
        assert_eq!(out, "1\n", "{}", body);
    }
}

/// THE FALSE REFUSAL bb324cd made. The String parameters `a` and `b` are
/// shadowed by integers whose `let` the checker refuses (the block holds a
/// `let n: bool = 1`); a failed `let` bound nothing, so the comparison read the
/// OUTER Strings and was refused. d5166e8 printed 1 for each.
#[test]
fn a_shadowing_local_whose_let_is_refused_still_shadows() {
    for body in [
        "{ let a = { let n: bool = 1; 12 }; let b = { let n: bool = 1; 12 }; a == b }",
        "if true { let a = { let n: bool = 1; 12 }; let b = { let n: bool = 1; 12 }; \
         return a == b; } false",
        "{ let a = { let n: bool = 1; 12 }; a == a }",
        "{ let a = { let n: bool = 1; 12 }; let b = a; a == b }",
    ] {
        let out = run(&generic_with(body, AT_STRINGS), "cmp_shadow");
        assert_eq!(out, "1\n", "{}", body);
    }
}

/// The same in ONE scope: `let a = 1;` over the parameter `a` is refused by
/// the checker (`already defined in this scope`), and the parameter's String
/// binding it left was what the comparison read. Not this rule's program to
/// refuse: d5166e8 handed it to gcc, which refuses the redefinition, and so
/// does this compiler now — with no PD0082 naming a String that is not there.
#[test]
fn a_same_scope_rebinding_is_not_read_as_the_parameter() {
    let name = unique_module_name("cmp_rebind");
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(
        &src,
        generic_with("let a = 1; let b = 1; a == b", AT_STRINGS),
    )
    .unwrap();
    if let Err(e) = Driver::new().compile_file(&src) {
        assert_ne!(
            e.code().map(|c| c.to_string()),
            Some("PD0082".to_string()),
            "{}",
            e
        );
    }
}

// ---------------------------------------------------------------------------
// Review round 3: aggregates, a `break` inside a branch, a `match` whose arms
// disagree — and the code-generation BACKSTOP that makes the next such shape
// impossible rather than the next round's finding.
// ---------------------------------------------------------------------------

/// `same<T>` beside a struct and an enum over String, called after `setup`.
fn generic_r3(body: &str, setup: &str) -> String {
    format!(
        "struct S {{ v: String }}\nenum E {{ V(String), N }}\n\
         fn same<T>(a: T, b: T) -> bool {{ {} }}\n\
         fn main() {{ {} if same(x, y) {{ print_int(1); }} else {{ print_int(0); }} }}",
        body, setup
    )
}

/// An aggregate built from a refused block lost its part's String, and C
/// compared `v[0]` (or `v.0`, `v.v`, the payload) by address: 0 on 3a9567e.
#[test]
fn an_aggregate_of_a_refused_block_keeps_its_parts_types() {
    for body in [
        "let v = [{ let n: bool = 1; a }]; v[0] == b",
        "let v = [{ let n: bool = 1; a }; 2]; v[0] == b",
        "let v = ({ let n: bool = 1; a }, 1); v.0 == b",
        "let v = S { v: { let n: bool = 1; a } }; v.v == b",
        "let v = E::V({ let n: bool = 1; a }); match v { E::V(s) => s == b, E::N => false }",
    ] {
        let text = refused(&generic_r3(body, AT_STRINGS), "cmp_r3_aggregate");
        assert!(text.contains("`==` is not defined on `String`"), "{}: {}", body, text);
    }
    for body in [
        "let v = [{ let n: bool = 1; a }]; v[0] == b",
        "let v = [{ let n: bool = 1; a }; 2]; v[0] == b",
        "let v = ({ let n: bool = 1; a }, 1); v.0 == b",
    ] {
        assert_eq!(run(&generic_r3(body, AT_INTS), "cmp_r3_aggregate_int"), "1\n", "{}", body);
    }
}

/// A `break` inside an `if` or `match` arm gives its loop a value; the walk
/// restored the loop frames wholesale on leaving the arm and lost it.
#[test]
fn a_break_inside_a_branch_still_gives_its_loop_a_type() {
    for body in [
        "(loop { if true { let n: bool = 1; break a; } else { break b; } }) == b",
        "(loop { match 1 { _ => { let n: bool = 1; break a; } } }) == b",
    ] {
        let text = refused(&generic_r3(body, AT_STRINGS), "cmp_r3_break");
        assert!(text.contains("`==` is not defined on `String`"), "{}: {}", body, text);
        assert_eq!(run(&generic_r3(body, AT_INTS), "cmp_r3_break_int"), "1\n", "{}", body);
    }
}

/// Arms that disagree have no type the checker can name. The two shapes the
/// panel measured: C lowered the first to a `const char*` temporary and printed
/// 0 (astra), and the second reached gcc (fable). Either way the comparison is
/// emitted with a non-scalar operand, which the code-generation backstop refuses
/// — that is what this test witnesses, so it asserts the backstop's own words.
#[test]
fn a_match_whose_arms_disagree_is_judged_by_the_c_it_becomes() {
    for body in [
        "(match 1 { 1 => { let n: bool = 1; a }, _ => 0 }) == b",
        "(match 1 { 1 => { let n: bool = 1; a } _ => { 2 } }) == b",
    ] {
        let text = refused(&generic_r3(body, AT_STRINGS), "cmp_r3_disagree");
        assert!(text.contains("Code generation failed"), "{}: {}", body, text);
    }
    let out = run(
        &generic_r3("(match 1 { 1 => { let n: bool = 1; a }, _ => 0 }) == b", AT_INTS),
        "cmp_r3_disagree_int",
    );
    assert_eq!(out, "1\n");
}

/// THE BACKSTOP, on a program the type checker passes. A reference LOCAL is
/// typed as its referent by the checker, and code generation keeps it a
/// pointer: `r == s` compared two addresses and printed 0 for `x == y == 5`.
/// Comparing what they point at is a comparison of two integers, and runs.
#[test]
fn two_references_compared_as_addresses_are_refused_by_the_backstop() {
    let text = refused(
        "fn main() { let x = 5; let y = 5; let r = &x; let s = &y; \
         if r == s { print_int(1); } else { print_int(0); } }",
        "cmp_r3_pointer",
    );
    assert!(text.contains("Code generation failed"), "{}", text);
    assert!(text.contains("a pointer"), "{}", text);
    let out = run(
        "fn main() { let x = 5; let y = 5; let r = &x; let s = &y; \
         if *r == *s { print_int(1); } else { print_int(0); } }",
        "cmp_r3_pointer_deref",
    );
    assert_eq!(out, "1\n");
}

/// The same rule over an imported generic's body (review round 3).
#[test]
fn an_imported_generic_is_held_to_the_rule() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("libcmp.pd"),
        "pub fn same<T>(a: T, b: T) -> bool { let x = [{ let n: bool = 1; a }]; x[0] == b }\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("main.pd"),
        "import libcmp;\nfn main() { let a = int_to_string(12); let b = \"12\"; \
         if same(a, b) { print_int(1); } else { print_int(0); } }\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .args(["compile", "main.pd", "-o", "prog"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run pdc");
    let text = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success(), "accepted: {}", text);
    assert!(text.contains("error[PD0082]"), "{}", text);
    assert!(!text.contains("gcc"), "{}", text);
}

// ---------------------------------------------------------------------------
// Review round 4: the backstop judges the carrier C actually RECEIVES — the
// type each hoisted value form was declared with — not a reconstruction.
// ---------------------------------------------------------------------------

/// THE FALSE REFUSAL 9dca568 made. The reconstruction typed `break x` outside
/// the loop, found the outer String `x`, and refused; the C declared the loop's
/// temporary `long long` and compared `__pd_val0 == 1LL`. d5166e8 printed 1.
#[test]
fn a_loop_value_is_judged_by_the_temporary_it_is_declared_as() {
    let out = run(
        "fn main() { let x = \"hi\"; \
         if (loop { let x = 1; break x; }) == 1 { print_int(1); } else { print_int(0); } }",
        "cmp_r4_loop_shadow",
    );
    assert_eq!(out, "1\n");
    let out = run(
        "fn main() { let x = \"hi\"; \
         let v = { let x = 1; x }; if v == 1 { print_int(1); } else { print_int(0); } }",
        "cmp_r4_block_shadow",
    );
    assert_eq!(out, "1\n");
}

/// Two loop values whose break operands the reconstruction could not type:
/// the temporaries were declared `const char*` and compared by address (0 for
/// two equal Strings on 9dca568). At `i64` the same program runs.
#[test]
fn loop_values_the_reconstruction_could_not_type_are_still_judged() {
    let body = "(loop { let x = match 1 { 1 => a, _ => 0 }; break x; }) == \
                (loop { let y = match 1 { 1 => b, _ => 0 }; break y; })";
    let text = refused(&generic_with(body, AT_STRINGS), "cmp_r4_unknown_loops");
    assert!(text.contains("`==` is not defined on `String`"), "{}", text);
    assert_eq!(run(&generic_with(body, AT_INTS), "cmp_r4_unknown_loops_int"), "1\n");
}

/// No generic and no refused statement: two loop values that are references
/// are `long long*` temporaries, compared by address (0 for `12 == 12` on
/// d5166e8 and 9dca568). The dereferenced comparison is of two integers.
#[test]
fn loop_values_that_are_pointers_are_refused_and_their_targets_compare() {
    let text = refused(
        "fn main() { let a = 12; let b = 12; \
         if (loop { let x = &a; break x; }) == (loop { let y = &b; break y; }) \
         { print_int(1); } else { print_int(0); } }",
        "cmp_r4_loop_pointer",
    );
    assert!(text.contains("a pointer"), "{}", text);
    let out = run(
        "fn main() { let a = 12; let b = 12; \
         if *(loop { let x = &a; break x; }) == *(loop { let y = &b; break y; }) \
         { print_int(1); } else { print_int(0); } }",
        "cmp_r4_loop_pointer_deref",
    );
    assert_eq!(out, "1\n");
}

/// A `break` in a statement `loop` nested in a value `loop` belongs to the
/// inner loop. The type checker's walk recorded it on the OUTER one, typed both
/// operands `(Int, Int)` and refused under this rule (fable, review round 4);
/// the program is refused for its own reason, as d5166e8 refused it.
#[test]
fn a_break_in_a_nested_statement_loop_is_not_the_outer_loops_value() {
    let name = unique_module_name("cmp_r4_nested_loop");
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(
        &src,
        generic_with(
            "(loop { loop { break (1, 2); } break a; }) == \
             (loop { loop { break (1, 2); } break b; })",
            AT_INTS,
        ),
    )
    .unwrap();
    let e = Driver::new()
        .compile_file(&src)
        .expect_err("d5166e8 refused this program too");
    assert_ne!(e.code().map(|c| c.to_string()), Some("PD0082".to_string()), "{}", e);
}

// ---------------------------------------------------------------------------
// Review round 5: arithmetic on an address, and the C compiler as the judge.
// ---------------------------------------------------------------------------

/// astra's witness, verbatim. `x` and `y` are `const char*` temporaries (the
/// `match` arms disagree), and `x + 0` was typed `long long` by code
/// generation, so `(x + 0) == (y + 0)` compared two addresses: d5166e8 and
/// be3efd0 printed `1` then `0` — `string_eq` right, `==` wrong.
#[test]
fn arithmetic_on_an_address_is_still_an_address() {
    let text = refused(
        "fn same<T>(a: T, b: T) -> bool {\n\
             let x = match 1 { 1 => a, _ => 0 };\n\
             let y = match 1 { 1 => b, _ => 0 };\n\
             (x + 0) == (y + 0)\n\
         }\n\
         fn main() {\n\
             let a = int_to_string(12);\n\
             let b = \"12\";\n\
             if string_eq(a, b) { print_int(1); } else { print_int(0); }\n\
             if same(a, b) { print_int(1); } else { print_int(0); }\n\
         }\n",
        "cmp_r5_arithmetic_hole",
    );
    assert!(text.contains("`==` is not defined on `String`"), "{}", text);
}

/// The forms around a String operand, each at Strings (refused) and at `i64`
/// (runs, and printed 1 on d5166e8): arithmetic in either order, a cast, a
/// call, a field.
#[test]
fn every_form_around_a_string_operand_is_judged() {
    let setup = "let x = match 1 { 1 => a, _ => 0 }; let y = match 1 { 1 => b, _ => 0 };";
    for tail in [
        "(x + 0) == (y + 0)",
        "(0 + x) == (0 + y)",
        "(x - 0) == (y - 0)",
        "(x + 0) == y",
        "pick(x) == pick(y)",
        "(x as T) == y",
    ] {
        let body = format!("{} {}", setup, tail);
        let program = generic_with(&body, AT_STRINGS).replace(
            "fn pick(s: String) -> String { s }",
            "fn pick<P>(s: P) -> P { s }",
        );
        let text = refused(&program, "cmp_r5_string_forms");
        assert!(text.contains("PD0082") || text.contains("is not defined on"), "{}: {}", tail, text);
    }
    for tail in ["(x + 0) == (y + 0)", "(0 + x) == (0 + y)", "(x - 0) == (y - 0)"] {
        let body = format!("{} {}", setup, tail);
        assert_eq!(run(&generic_with(&body, AT_INTS), "cmp_r5_int_forms"), "1\n", "{}", tail);
    }
    let text = refused(
        "struct S { v: String }\n\
         fn main() { let s = S { v: int_to_string(12) }; let t = S { v: \"12\" }; \
         if s.v == t.v { print_int(1); } else { print_int(0); } }",
        "cmp_r5_string_field",
    );
    assert!(text.contains("`==` is not defined on `String`"), "{}", text);
}

/// Two addresses SUBTRACTED are a distance — an integer in C (`ptrdiff_t`) —
/// and compare by value. These printed 1 on d5166e8 and must keep printing it;
/// a reference plus an integer is still an address, and is refused.
#[test]
fn a_pointer_distance_is_a_number_and_a_pointer_sum_is_not() {
    assert_eq!(
        run(
            "fn main() { let a = 12; let r = &a; \
             if (r - r) == 0 { print_int(1); } else { print_int(0); } }",
            "cmp_r5_distance",
        ),
        "1\n"
    );
    let text = refused(
        "fn main() { let a = 12; let b = 12; let r = &a; let s = &b; \
         if (r + 0) == (s + 0) { print_int(1); } else { print_int(0); } }",
        "cmp_r5_pointer_sum",
    );
    assert!(text.contains("a pointer"), "{}", text);
}

/// Every comparison is emitted with the static check the C compiler judges
/// (its refusal path is `linker::tests::a_comparison_the_c_compiler_refuses_
/// is_reported_as_pd0082`), and a program that compares integers prints what
/// it printed before.
#[test]
fn every_comparison_is_emitted_with_the_c_compilers_check() {
    let name = unique_module_name("cmp_r5_emitted");
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, "fn main() { let a = 1; let b = 2; if a < b { print_int(1); } }").unwrap();
    let c_file = Driver::new().compile_file(&src).expect("compiles");
    let c = fs::read_to_string(&c_file).unwrap();
    assert!(
        c.contains("_Static_assert((0x11E >> __builtin_classify_type(a)) & (0x11E >> __builtin_classify_type(b)) & 1")
            && c.contains("&& (a < b))"),
        "{}",
        c
    );
    assert_eq!(
        run("fn main() { let a = 1; let b = 2; if a < b { print_int(1); } }", "cmp_r5_emitted_run"),
        "1\n"
    );
}

// ---------------------------------------------------------------------------
// Review round 6: the C compiler's refusal names the operator's own place.
// ---------------------------------------------------------------------------

/// The four left-operand shapes a comparison is written with, each on its own
/// line, the operator at a column no operand starts at: a name, a
/// parenthesised sum, a call, a name again under `==`. A name and a literal
/// have no position in the tree, and the comparison took its line and column
/// from its LEFT OPERAND, so be3efd0's check said `PD0082|0|0|<|main` for
/// every one of the first, second and fourth, and the call's column for the
/// third — never the operator's.
const PLACED: &str = "fn f(x: i64) -> i64 { x }\n\
fn main() {\n\
    let a = 1;\n\
    let b = 2;\n\
    if a    <   b { print_int(1); }\n\
    let c = (a + 1) >= b;\n\
    if f(a) != 3 { print_int(3); }\n\
    if b == a { print_int(2); }\n\
}\n";

/// `source` emitted as C by this compiler: the C file and its text. The
/// four-space indentation `\n\` drops is put back on every statement inside
/// `main`, which the columns the tests count include.
fn emitted_c(source: &str, prefix: &str) -> (TempDir, std::path::PathBuf, String) {
    let name = unique_module_name(prefix);
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    let source = source
        .lines()
        .map(|l| if l.starts_with("let") || l.starts_with("if") { format!("    {}", l) } else { l.to_string() })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&src, source).unwrap();
    let c_file = Driver::new().compile_file(&src).expect("a legal program");
    let c = fs::read_to_string(&c_file).unwrap();
    (dir, c_file, c)
}

/// Line 5 `    if a    <   b`, column 13; line 6 `    let c = (a + 1) >= b;`,
/// column 21; line 7 `    if f(a) != 3`, column 13; line 8 `    if b == a`,
/// column 10 — 1-based, as every other diagnostic of this compiler counts.
/// (`c == true` would not do for the last: the optimizer folds it to `c`.)
#[test]
fn the_c_compilers_check_names_the_operators_line_and_column() {
    let (_dir, _c_file, c) = emitted_c(PLACED, "cmp_r6_placed");
    for marker in [
        "\"PD0082|5|13|<|main\"",
        "\"PD0082|6|21|>=|main\"",
        "\"PD0082|7|13|!=|main\"",
        "\"PD0082|8|10|==|main\"",
    ] {
        assert!(c.contains(marker), "{} is not in the emitted C:\n{}", marker, c);
    }
    assert!(!c.contains("PD0082|0|"), "a comparison with no place:\n{}", c);
}

/// THE JUDGE'S REFUSAL, END TO END. No program measured so far reaches the C
/// compiler's check with an address — the backstop refuses first — so this
/// takes the C this compiler emitted for `a < b` over two integers and makes
/// the two operands addresses, which is exactly what the check exists for (an
/// operand whose carrier code generation got wrong). The real link must refuse
/// it as PD0082 at the operator's source line and column, with a front-end
/// refusal's exit code — not gcc's `compilation failed`, not an ICE — and the
/// driver must carry it to the user under PD0082.
#[test]
fn a_comparison_only_the_c_compiler_can_judge_is_pd0082_at_the_operator() {
    use palladium::driver::RunOutcome;
    use palladium::linker::{link, LinkError, EXIT_REFUSED};
    let (dir, c_file, c) = emitted_c(
        "fn main() {\n\
         let a = 1;\n\
         let b = 2;\n\
         if a    <   b { print_int(1); }\n\
         }\n",
        "cmp_r6_judged",
    );
    for decl in ["long long a = 1LL;", "long long b = 2LL;"] {
        assert!(c.contains(decl), "{} is not in the emitted C:\n{}", decl, c);
    }
    fs::write(
        &c_file,
        c.replace("long long a = 1LL;", "const char* a = \"x\";")
            .replace("long long b = 2LL;", "const char* b = \"y\";"),
    )
    .unwrap();
    let err = link(&c_file, &dir.path().join("judged"), OptLevel::Default)
        .expect_err("two addresses must not link");
    assert!(matches!(err, LinkError::Refused(_)), "not the named refusal: {:?}", err);
    assert_eq!(err.exit_code(), EXIT_REFUSED);
    let text = err.to_string();
    assert!(text.contains("`<`") && text.contains("line 4, column 13, in `main`"), "{}", text);
    let e = RunOutcome::Link(err).into_compile_error();
    assert_eq!(e.code().map(|c| c.to_string()), Some("PD0082".to_string()), "{}", e);
    assert!(e.to_string().contains("line 4, column 13"), "{}", e);
    assert!(!e.to_string().contains("gcc"), "{}", e);
}

/// AND THE JUDGE CLAIMS NOTHING ELSE. Two panel programs whose C this compiler
/// gets wrong for reasons that are not this rule — a generic called from a
/// generic is never emitted (`id` undeclared), and a `let` that shadows a
/// parameter is emitted as a C redeclaration — both comparing INTEGERS. On
/// main both are `gcc compilation failed`. Round 5 named both PD0082, because
/// the tag was read anywhere in gcc's stderr: in the source line the compiler
/// echoes under the first, and in a check that failed only as the
/// redeclaration's cascade in the second. Both must stay the link failure
/// they were, never a comparison refusal.
#[test]
fn a_gcc_failure_that_is_not_the_check_is_not_pd0082() {
    use palladium::linker::{link, LinkError};
    for (program, prefix) in [
        (
            "fn id<X>(x:X)->X{x} fn same<T>(a:T,b:T)->bool { id({let n:bool=1; a}) == b }\n\
             fn main() { if same(12,12) {print_int(1);} else {print_int(0);} }\n",
            "cmp_r6_echoed",
        ),
        (
            "fn b(x: bool) { if x { print_int(1); } else { print_int(0); } }\n\
             fn same<T>(a: T, c: T) -> bool { let a = 1; let c = 1; a == c }\n\
             fn main() { let aa = int_to_string(12); let bb = \"12\"; b(same(aa, bb)); }\n",
            "cmp_r6_cascade",
        ),
    ] {
        let (dir, c_file, _c) = emitted_c(program, prefix);
        let err = link(&c_file, &dir.path().join("x"), OptLevel::Default)
            .expect_err("main's gcc refused this C too");
        assert!(matches!(err, LinkError::GccRejected(_)), "{}: {:?}", prefix, err);
        // gcc's own stderr rides along, echoed source line, tag and all; the
        // refusal's words must not.
        assert!(!err.to_string().contains("does not compare by value"), "{}: {}", prefix, err);
    }
}

// ---------------------------------------------------------------------------
// Review round 7: one door for every C comparison, and C linear in nesting.
// ---------------------------------------------------------------------------

/// astra's round-6 shape: `tick()` compared with `yes()`, `depth` times, each
/// comparison the left operand of the next. main prints `7` then `1`.
fn nested_calls(depth: usize) -> String {
    let mut condition = "tick()".to_string();
    for _ in 0..depth {
        condition = format!("({} == yes())", condition);
    }
    format!(
        "fn tick() -> bool {{ print_int(7); true }}\nfn yes() -> bool {{ true }}\n\
         fn main() {{ if {} {{ print_int(1); }} else {{ print_int(0); }} }}\n",
        condition
    )
}

/// THE CHECK NAMES EACH OPERAND ONCE. Round 6 put both operands inside the
/// check AND in the comparison, so a comparison that was an operand appeared
/// twice and its own operands four times: measured on 76734e5, depth 17
/// emitted 23,405,093 bytes of C (main: 9,107) and clang timed out. Each level
/// must now cost a constant, and the deepest program must still run once.
#[test]
fn nested_comparisons_emit_c_linear_in_their_depth() {
    let size = |depth: usize| emitted_c(&nested_calls(depth), "cmp_r7_depth").2.len();
    let (one, sixteen, seventeen) = (size(1), size(16), size(17));
    assert!(seventeen - sixteen < 1024, "one more level cost {} bytes", seventeen - sixteen);
    assert!(seventeen < one + 17 * 1024, "depth 1: {} bytes, depth 17: {}", one, seventeen);
    assert!(seventeen < 2 * sixteen, "depth 16: {} bytes, depth 17: {}", sixteen, seventeen);
    assert_eq!(run(&nested_calls(17), "cmp_r7_depth_run"), "7\n1\n");
}

/// A pattern this generic body compares its subject against, at a String and
/// at a type the pattern belongs to.
fn generic_match(arms: &str, argument: &str) -> String {
    format!(
        "fn test<T>(a: T) -> bool {{ match a {{ {} => true, _ => false }} }}\n\
         fn main() {{ let a = {}; if test(a) {{ print_int(1); }} else {{ print_int(0); }} }}\n",
        arms, argument
    )
}

/// THE LOWERINGS ARE JUDGED TOO. A generic body's refusals other than PD0082
/// are not raised, so a String RANGE pattern lowered to `_match_expr >= "a"`
/// and compared two addresses (astra, round 6: main and 76734e5 printed 0 for a
/// run-time "m"), and an integer or bool pattern compared the String's address
/// with a number. Every one now meets the C compiler's check where it is
/// lowered — the first programs only that check catches — and is refused at
/// the `match` as PD0082; at the pattern's own type each still runs.
#[test]
fn every_pattern_a_generic_body_compares_is_judged() {
    use palladium::driver::RunOutcome;
    use palladium::linker::{link, LinkError};
    for (arms, construct) in [
        ("\"a\"..=\"z\"", "a range pattern"),
        ("0..=9", "a range pattern"),
        ("0", "a literal pattern"),
        ("true", "a literal pattern"),
    ] {
        let program = generic_match(arms, "string_concat(\"\", \"m\")");
        let (dir, c_file, _c) = emitted_c(&program, "cmp_r7_pattern");
        let err = link(&c_file, &dir.path().join("p"), OptLevel::Default)
            .expect_err("a String compared by its address must not link");
        assert!(matches!(err, LinkError::Refused(_)), "{}: {:?}", arms, err);
        let text = err.to_string();
        assert!(text.contains(construct) && text.contains("line 1, column 28, in `test__String`"), "{}: {}", arms, text);
        let e = RunOutcome::Link(err).into_compile_error();
        assert_eq!(e.code().map(|c| c.to_string()), Some("PD0082".to_string()), "{}", arms);
    }
    for (arms, argument) in [("1..=9", "5"), ("5", "5"), ("true", "true")] {
        assert_eq!(run(&generic_match(arms, argument), "cmp_r7_pattern_ok"), "1\n", "{}", arms);
    }
    // A char range, outside a generic: a generic body at `char` is refused
    // before any of this ("expected char, found Char" — not this rule).
    assert_eq!(
        run(
            "fn main() { let c = 'm'; match c { 'a'..='z' => { print_int(1); } _ => { print_int(0); } } }",
            "cmp_r7_char_range"
        ),
        "1\n"
    );
    // A String LITERAL pattern is `__pd_string_eq`, a comparison of characters.
    assert_eq!(
        run(&generic_match("\"m\"", "string_concat(\"\", \"m\")"), "cmp_r7_string_eq"),
        "1\n"
    );
}

/// `as bool` is a comparison with zero, and a String's is its address's
/// (main printed 1 for any String). Judged where it is lowered.
#[test]
fn as_bool_is_judged_where_it_is_lowered() {
    use palladium::linker::{link, LinkError};
    let program = |argument: &str| {
        format!(
            "fn t<T>(x: T) -> bool {{ x as bool }}\n\
             fn main() {{ let v = {}; if t(v) {{ print_int(1); }} else {{ print_int(0); }} }}\n",
            argument
        )
    };
    let (dir, c_file, _c) = emitted_c(&program("int_to_string(1)"), "cmp_r7_as_bool");
    let err = link(&c_file, &dir.path().join("b"), OptLevel::Default).expect_err("an address is not a bool");
    assert!(matches!(err, LinkError::Refused(_)) && err.to_string().contains("`as bool`"), "{:?}", err);
    assert_eq!(run(&program("5"), "cmp_r7_as_bool_ok"), "1\n");
}

/// A `for` loop's bound tests are comparisons code generation writes, and
/// they meet the same check; every loop shape still visits what it did.
#[test]
fn a_for_loops_bound_tests_are_judged_and_loops_still_run() {
    let source = "fn main() {\n\
                  for i in 0..3 { print_int(i); }\n\
                  for j in 5..=6 { print_int(j); }\n\
                  let xs = [7, 8];\n\
                  for x in xs { print_int(x); }\n\
                  }\n";
    let (_dir, _c_file, c) = emitted_c(source, "cmp_r7_for");
    for test in ["&& (i < __pd_hi", "&& (__pd_lo", "&& (__pd_k", "&& (_i < 2))"] {
        assert!(c.contains(test), "{} is not judged:\n{}", test, c);
    }
    assert_eq!(run(source, "cmp_r7_for_run"), "0\n1\n2\n5\n6\n7\n8\n");
}

/// AT FILE SCOPE C HAS NO STATEMENT EXPRESSION, and a `const` or `static`
/// initialiser may compare (fable, round 6: they compile today). The
/// initialiser is emitted plain — a constant expression — and the same
/// expression, judged, in a function nothing calls, where the
/// `_Static_assert`s are still evaluated. Nested 17 deep it stays linear.
#[test]
fn a_file_scope_comparison_is_judged_in_a_twin_and_runs() {
    let source = "const B: bool = 1 < 2;\n\
                  const C: bool = (1 < 2) == true;\n\
                  static D: bool = 3 >= 2;\n\
                  fn main() { if B { print_int(1); } if C { print_int(2); } if D { print_int(3); } }\n";
    let (_dir, _c_file, c) = emitted_c(source, "cmp_r7_file_scope");
    assert!(c.contains("static const int B = (1LL < 2LL);"), "{}", c);
    assert!(c.contains(&format!("static void __attribute__((unused)) {}(void)", palladium::codegen::c_ident::composed('j', &["B"]))), "{}", c);
    assert!(c.contains("\"PD0082|1|19|<|B\"") && c.contains("|==|C\""), "{}", c);
    assert_eq!(run(source, "cmp_r7_file_scope_run"), "1\n2\n3\n");

    let deep = |depth: usize| {
        let mut e = "1 < 2".to_string();
        for _ in 0..depth {
            e = format!("({}) == true", e);
        }
        format!("const K: bool = {};\nfn main() {{ if K {{ print_int(1); }} }}\n", e)
    };
    let size = |depth: usize| emitted_c(&deep(depth), "cmp_r7_file_scope_depth").2.len();
    let (sixteen, seventeen) = (size(16), size(17));
    assert!(seventeen - sixteen < 1024, "one more level cost {} bytes", seventeen - sixteen);
    assert_eq!(run(&deep(17), "cmp_r7_file_scope_depth_run"), "1\n");
}

// ---------------------------------------------------------------------------
// Review round 8: a program may name anything code generation invents.
// ---------------------------------------------------------------------------

/// EVERY NAME CODE GENERATION INVENTS, DECLARED BY A PROGRAM. Round 7's
/// comparison temporary `__pd_cl0` captured a program's own `let __pd_cl0`
/// (astra: main printed 0, b442a21 printed 1), and its file-scope judge
/// `__pd_judge_A` redefined a program's function. The class was older than
/// round 7 — measured on main 2ca5eb5: a program's `_match_expr`, `_i`,
/// `__pd_hi0`, `__pd_n0`, `__pd_val0` or enum tag constant `__C__R` was
/// captured by the generated one and the program printed the wrong number
/// (3, 1 3, 2 3, 1 3, 0, the wrong arm), and a program defining
/// `__pd_print_int`, `__pd_P_m`, `__pd_r0`, `__pd_range` or `_match_expr` as a
/// function did not compile. Every source name beginning with `_` is now
/// emitted as `_u_…`, where nothing is generated (`c_ident::c_name`), so each
/// of these prints what the program says.
#[test]
fn a_program_may_name_anything_code_generation_invents() {
    let yes = "fn yes() -> bool { true }\n";
    let cases: Vec<(String, &str)> = vec![
        // round 7's own temporaries and judge (astra's four)
        (format!("{}fn main() {{ let __pd_cl0 = false; if (yes() == yes()) == __pd_cl0 {{ print_int(1); }} else {{ print_int(0); }} }}", yes), "0\n"),
        (format!("{}fn main() {{ let __pd_cl0 = true; if (__pd_cl0 == yes()) == yes() {{ print_int(1); }} else {{ print_int(0); }} }}", yes), "1\n"),
        (format!("{}fn main() {{ let __pd_cr0 = false; if (yes() == yes()) == __pd_cr0 {{ print_int(1); }} else {{ print_int(0); }} }}", yes), "0\n"),
        ("const A: bool = 1 < 2;\nfn __pd_judge_A() { print_int(9); }\nfn main() { __pd_judge_A(); if A { print_int(1); } }".to_string(), "9\n1\n"),
        (format!("{}fn f(__pd_cl0: bool, __pd_cr0: bool) -> bool {{ (yes() == yes()) == __pd_cl0 }}\nfn main() {{ if f(false, true) {{ print_int(1); }} else {{ print_int(0); }} }}", yes), "0\n"),
        ("static __pd_judge_B: i64 = 5;\nconst A: bool = 1 < 2;\nfn main() { if A { print_int(__pd_judge_B); } }".to_string(), "5\n"),
        // the temporaries, subject and index main already emitted
        ("fn main() { let __pd_val0 = 7; let c = true; if c { let y = if c { __pd_val0 } else { 0 }; print_int(y); } }".to_string(), "7\n"),
        ("fn main() { let _match_expr = 5; let k = 3; match k { 3 => { print_int(_match_expr); } _ => { print_int(0); } } }".to_string(), "5\n"),
        ("fn main() { let _i = 7; let xs = [1, 2]; for x in xs { print_int(_i + x); } }".to_string(), "8\n9\n"),
        ("fn main() { let __pd_hi0 = 9; for i in 0..2 { print_int(__pd_hi0 + i); } }".to_string(), "9\n10\n"),
        ("fn main() { let __pd_n0 = 5; let __pd_k0 = 6; let __pd_done0 = 7; let __pd_lo0 = 8; for i in 0..=1 { print_int(__pd_n0 + __pd_k0 + __pd_done0 + __pd_lo0 + i); } }".to_string(), "26\n27\n"),
        ("fn main() { let __pd_r0 = 4; let r = 0..2; for i in r { print_int(__pd_r0 + i); } }".to_string(), "4\n5\n"),
        // an enum tag constant, a runtime symbol, method mangling, typedefs, functions, fields
        ("enum C { R, G }\nfn main() { let __C__R = 1; let c = C::G; match c { C::R => { print_int(1); } C::G => { print_int(2); } } print_int(__C__R); }".to_string(), "2\n1\n"),
        ("fn __pd_print_int(x: i64) { print_int(x + 100); }\nfn main() { __pd_print_int(1); }".to_string(), "101\n"),
        ("struct P { v: i64 }\nimpl P { fn m(self) -> i64 { self.v } }\nfn __pd_P_m(p: P) -> i64 { 99 }\nfn main() { let p = P { v: 1 }; print_int(p.m()); }".to_string(), "1\n"),
        ("struct __pd_range { a: i64 }\nstruct __pd_tuple2 { b: i64 }\nfn main() { let x = __pd_range { a: 1 }; let y = __pd_tuple2 { b: 2 }; let t = (3, 4); print_int(x.a + y.b + t.0 + t.1); }".to_string(), "10\n"),
        ("fn _match_expr() -> i64 { 1 }\nfn _i() -> i64 { 2 }\nfn __pd_argc() -> i64 { 4 }\nfn main() { let k = 1; match k { 1 => { print_int(_match_expr() + _i() + __pd_argc()); } _ => {} } }".to_string(), "7\n"),
        ("struct S { _match_expr: i64, __pd_val0: i64, _i: i64 }\nfn main() { let s = S { _match_expr: 1, __pd_val0: 2, _i: 3 }; let k = 4; match k { 4 => { print_int(s._match_expr + s.__pd_val0 + s._i); } _ => { print_int(0); } } }".to_string(), "6\n"),
    ];
    for (program, expected) in &cases {
        assert_eq!(run(program, "cmp_r8_names"), *expected, "{}", program);
    }
}

/// THE TYPE ARGUMENTS ARE NAMES TOO (review round 9). Round 8 spelled a
/// program's `_S` as `_u_S` where it was declared but left the instantiations'
/// type arguments — strings beside the escaped template — as written, so
/// `id<T>` was instantiated at an undeclared `struct _S` and a generic struct
/// literal matched no instantiation (astra: main printed 7 and 1; round 8
/// failed in gcc or refused). Every type argument, and every struct-literal
/// record, now takes the same spelling at the same door
/// (`c_ident::escape_type_argument`). Each of these printed this on main.
#[test]
fn a_generic_instantiated_at_an_underscore_name_runs() {
    let cases = [
        ("struct _S{x:i64}fn id<T>(x:T)->T{x}fn main(){let s=_S{x:7};let q=id(s);print_int(q.x);}", "7\n"),
        ("enum _E{A,B}fn id<T>(x:T)->T{x}fn main(){let e=_E::A;let q=id(e);match q{_E::A=>{print_int(1);}_E::B=>{print_int(0);}}}", "1\n"),
        ("struct _S{x:i64}struct Box<T>{v:T}fn main(){let x=Box{v:_S{x:7}};print_int(x.v.x);}", "7\n"),
        ("struct _S { x: i64 }\nfn _id<T>(x: T) -> T { x }\nfn main() { let s = _S { x: 7 }; let q = _id(s); print_int(q.x); }", "7\n"),
        ("struct _S { x: i64 }\nstruct _B<T> { v: T }\nfn main() { let b = _B { v: _S { x: 7 } }; print_int(b.v.x); }", "7\n"),
        ("struct _S { x: i64 }\nimpl _S { fn get(self) -> i64 { self.x } }\nfn id<T>(x: T) -> T { x }\nfn main() { let s = _S { x: 7 }; let q = id(s); print_int(q.get()); }", "7\n"),
        ("enum _E { _A(i64), B }\nfn id<T>(x: T) -> T { x }\nfn main() { let e = _E::_A(7); let q = id(e); match q { _E::_A(v) => { print_int(v); } _E::B => { print_int(0); } } }", "7\n"),
        ("struct _S { x: i64 }\nenum _E { A, B }\nfn first<A, B>(a: A, b: B) -> A { a }\nfn main() { let s = _S { x: 7 }; let e = _E::B; let q = first(s, e); print_int(q.x); }", "7\n"),
        ("struct _S{_v:i64}type _A=_S;fn f(x:_A)->i64{x._v}fn main(){print_int(f(_S{_v:7}));}", "7\n"),
        ("fn main(){let _x=1;let _u_x=2;let __x=3;let _u__x=4;let _Bool=5;let _Bool_=6;print_int(_x+_u_x+__x+_u__x+_Bool+_Bool_);}", "21\n"),
    ];
    for (program, expected) in cases {
        assert_eq!(run(program, "cmp_r9_generic"), expected, "{}", program);
    }
}

/// An imported module's `_`-named items, generic and not, at a program's
/// `_`-named type: the module's AST and the instantiation's type arguments
/// take the one spelling (astra r8: `import libx` with `_S` and `id` printed 7
/// on main and failed in gcc on round 8).
#[test]
fn an_imported_underscore_item_and_generic_run() {
    for (module, module_source, program, expected) in [
        (
            "libx",
            "pub struct _S{x:i64}pub fn id<T>(x:T)->T{x}\n",
            "import libx;fn main(){let s=_S{x:7};let q=id(s);print_int(q.x);}\n",
            "7\n",
        ),
        (
            "_lib",
            "pub fn _f(x:i64)->i64{x+1} pub struct _S{_v:i64}\n",
            "import _lib;fn main(){print_int(_f(6));let s=_S{_v:7};print_int(s._v);}\n",
            "7\n7\n",
        ),
    ] {
        // The resolver searches `.` and the driver writes `build_output/` there
        // (tests/m3_imported_calls.rs does the same), so the scratch directory
        // is the working directory.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(format!("{}.pd", module)), module_source).unwrap();
        fs::write(dir.path().join("main.pd"), program).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
            .args(["compile", "main.pd", "-o", "prog"])
            .current_dir(dir.path())
            .output()
            .expect("pdc");
        assert!(
            out.status.success(),
            "{}: {}{}",
            module,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let ran = Command::new(dir.path().join("build_output").join("prog"))
            .current_dir(dir.path())
            .output()
            .expect("run");
        assert_eq!(String::from_utf8_lossy(&ran.stdout), expected, "{}", module);
    }
}

/// EVERY NAME CODE GENERATION COMPOSES FROM SOURCE NAMES, DEFINED BY A
/// PROGRAM (review round 10). A composition of source names can be a source
/// name: round 9's `id___u_S` (generic `id` at `_S`) took a program's own
/// `fn id___u_S`, and `Box__u_S` its `struct Box__u_S` (astra: main printed 7,
/// round 9 failed in gcc). The class was older: on main a program's
/// `same__i64` beside `same<T>` at `i64`, `Box_i64` beside `Box<i64>`, and
/// `E_V`, `E_W__new`, `E__W_Data` or `ETag` beside `enum E` were redefinitions.
/// Every composed name now begins with `_g_` (`c_ident::COMPOSED_PREFIX`),
/// where no source name can be.
#[test]
fn a_composed_c_name_never_meets_a_source_name() {
    let cases = [
        ("struct _S{x:i64}fn id<T>(x:T)->T{x}fn id___u_S(x:_S)->_S{x}fn main(){let x=_S{x:7};let q=id(x);print_int(q.x);}", "7\n"),
        ("struct _S{x:i64}struct Box<T>{v:T}struct Box__u_S{q:i64}fn main(){let x=Box{v:_S{x:7}};print_int(x.v.x);}", "7\n"),
        ("struct _S{x:i64}fn id<T>(x:T)->T{x}fn id___u_S()->i64{9}fn main(){let x=_S{x:7};let q=id(x);print_int(q.x);print_int(id___u_S());}", "7\n9\n"),
        ("fn same<T>(a:T,b:T)->bool{true}\nfn same__i64(a:i64,b:i64)->bool{false}\nfn main(){ if same(1,2) {print_int(1);} if same__i64(1,2) {print_int(2);} else {print_int(3);} }", "1\n3\n"),
        ("struct Box<T>{v:T}\nstruct Box_i64{w:i64}\nfn main(){let a=Box{v:1};let b=Box_i64{w:2};print_int(a.v+b.w);}", "3\n"),
        ("enum E { V, W(i64) }\nfn E_V() -> i64 { 5 }\nfn E_W__new(x: i64) -> i64 { x }\nstruct E__W_Data { z: i64 }\nfn ETag() -> i64 { 1 }\nfn main() { let e = E::W(3); let d = E__W_Data { z: 2 }; match e { E::W(v) => { print_int(v + E_V() + E_W__new(4) + d.z + ETag()); } E::V => { print_int(0); } } }", "15\n"),
        ("struct P { v: i64 }\nimpl P { fn get(self) -> i64 { self.v } }\nfn P_get(p: P) -> i64 { 100 }\nfn __pd_P_get() -> i64 { 200 }\nfn main() { let p = P { v: 1 }; let q = P { v: 2 }; print_int(p.get() + P_get(q) + __pd_P_get()); }", "301\n"),
        ("enum _E { _V, W(i64) }\nfn _g__u__E__u__V() -> i64 { 5 }\nfn main() { let e = _E::_V; match e { _E::_V => { print_int(_g__u__E__u__V()); } _E::W(x) => { print_int(x); } } }", "5\n"),
    ];
    for (program, expected) in cases {
        assert_eq!(run(program, "cmp_r10_composed"), expected, "{}", program);
    }
}

/// COMPOSED NAMES ARE AN INJECTIVE ENCODING OF THEIR COMPONENTS (review round
/// 11). Round 10 kept `_` joins under `_g_`, and a join is not injective: with
/// `_u` in the components, `f<_S>`/`f_<u_S>`, `B<_S>`/`B_<u_S>`, `E::_V`/`E_::u_V`
/// and methods `E::_m`/`E_::u_m` met where main's names did not (astra: main
/// `1 2`, `7 8`, `1 2`, `1 2`; round 10 gcc), and a file-scope judge
/// `__pd_judge_K` met the method `judge::K` (fable: main `3 1`, `4 1`, `7 1`).
/// Every composed name is now `c_ident::composed` — length-delimited components
/// under `_g_` — so these print what main printed, and the pairs that met on
/// main too (`a<b__c>`/`a__b<c>`, `A<B_i64>`/`A_B<i64>`, `A_B::m`/`A::B_m`,
/// `A_B::C`/`A::B_C`, `A__B::C`/`A::B__C`, a method `string::eq` beside the
/// runtime's `__pd_string_eq`; gcc refused them) now run.
#[test]
fn composed_names_meet_nothing_main_kept_apart() {
    let cases = [
        ("struct _S{x:i64}struct u_S{x:i64}fn f<T>(x:T)->i64{1}fn f_<T>(x:T)->i64{2}fn main(){let a=_S{x:7};let b=u_S{x:8};print_int(f(a));print_int(f_(b));}", "1\n2\n"),
        ("struct _S{x:i64}struct u_S{x:i64}struct B<T>{v:T}struct B_<T>{w:T}fn main(){let a=B{v:_S{x:7}};let b=B_{w:u_S{x:8}};print_int(a.v.x);print_int(b.w.x);}", "7\n8\n"),
        ("enum E{_V}enum E_{u_V}fn main(){match E::_V{E::_V=>{print_int(1);}}match E_::u_V{E_::u_V=>{print_int(2);}}}", "1\n2\n"),
        ("struct E{x:i64}struct E_{x:i64}impl E{fn _m(self)->i64{1}}impl E_{fn u_m(self)->i64{2}}fn main(){let a=E{x:1};let b=E_{x:2};print_int(a._m());print_int(b.u_m());}", "1\n2\n"),
        ("struct judge { n: i64 }\nimpl judge { fn K(&self) -> i64 { self.n } }\nconst K: bool = 1 == 1;\nfn b(x: bool) { if x { print_int(1); } else { print_int(0); } }\nfn main() { let j = judge { n: 3 }; print_int(j.K()); b(K); }", "3\n1\n"),
        ("struct judge { n: i64 }\nimpl judge { fn S(&self) -> i64 { self.n } }\nstatic S: bool = 2 > 1;\nfn b(x: bool) { if x { print_int(1); } else { print_int(0); } }\nfn main() { let j = judge { n: 4 }; print_int(j.S()); b(S); }", "4\n1\n"),
        ("struct judge { n: i64 }\nimpl judge { fn K(&self) -> i64 { self.n } }\nconst K: bool = (1 + 1) == 2;\nfn b(x: bool) { if x { print_int(1); } else { print_int(0); } }\nfn main() { let j = judge { n: 7 }; print_int(j.K()); b(K); }", "7\n1\n"),
        ("struct b__c { x: i64 }\nfn a<T>(x: T) -> T { x }\nfn a__b<T>(x: T) -> T { x }\nstruct c { y: i64 }\nfn main() { let v = b__c { x: 1 }; print_int(a(v).x); let w = c { y: 2 }; print_int(a__b(w).y); }", "1\n2\n"),
        ("struct A<T> { v: T }\nstruct A_B<T> { w: T }\nstruct B_i64 { x: i64 }\nfn main() { let p = A { v: B_i64 { x: 1 } }; print_int(p.v.x); let q = A_B { w: 2 }; print_int(q.w); }", "1\n2\n"),
        ("struct A_B { x: i64 }\nstruct A { y: i64 }\nimpl A_B { fn m(&self) -> i64 { self.x } }\nimpl A { fn B_m(&self) -> i64 { self.y } }\nfn main() { let p = A_B { x: 1 }; let q = A { y: 2 }; print_int(p.m()); print_int(q.B_m()); }", "1\n2\n"),
        ("enum A_B { C(i64) }\nenum A { B_C(i64) }\nfn main() { let x = A_B::C(1); match x { A_B::C(n) => { print_int(n); } } let y = A::B_C(2); match y { A::B_C(n) => { print_int(n); } } }", "1\n2\n"),
        ("enum A__B { C, D }\nenum A { B__C, E }\nfn main() { let x = A__B::C; match x { A__B::C => { print_int(1); } A__B::D => { print_int(0); } } let y = A::B__C; match y { A::B__C => { print_int(2); } A::E => { print_int(0); } } }", "1\n2\n"),
        ("struct string { n: i64 }\nimpl string { fn eq(&self, o: string) -> bool { self.n == o.n } }\nfn b(x: bool) { if x { print_int(1); } else { print_int(0); } }\nfn main() { let s = string { n: 1 }; let t = string { n: 1 }; b(s.eq(t)); b(string_eq(\"a\", \"a\")); }", "1\n1\n"),
    ];
    for (program, expected) in cases {
        assert_eq!(run(program, "cmp_r11_composed"), expected, "{}", program);
    }
}
