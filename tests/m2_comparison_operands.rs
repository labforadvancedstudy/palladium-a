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
//! REFUSED BY NAME (PD0080) with the way to compare instead. Nothing invents a
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

/// Compile only, expecting THIS compiler's refusal under PD0080 — not gcc's,
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
                Some("PD0080".to_string()),
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
// printed 0 for two equal Strings (or reached gcc), with no PD0080.
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
        text.contains("error[PD0080]") && text.contains("in `lib2::pair_is_one`, imported:"),
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
/// does this compiler now — with no PD0080 naming a String that is not there.
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
            Some("PD0080".to_string()),
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
    assert!(text.contains("error[PD0080]"), "{}", text);
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
    assert_ne!(e.code().map(|c| c.to_string()), Some("PD0080".to_string()), "{}", e);
}
