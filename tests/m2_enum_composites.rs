//! A user `enum` inside every composite the language can spell: arrays first.
//!
//! THE DEFECT, MEASURED ON 2563001 BEFORE THE FIX
//!
//! ```text
//! enum K { A, B, C }
//! fn main() { let xs: [K; 4] = [K::A; 4]; }
//! -> error[PD0022]: Type mismatch: expected [K; 4], found [K; 4]
//! ```
//!
//! `ast_type_to_checker_type` (`src/typeck/mod.rs`) decided enum-vs-struct for
//! a bare named type and sent every composite it had no arm for to
//! `CheckerType::from`, whose `Custom` arm has no table to consult and answers
//! `Struct` for every name. The annotation was `[Struct K; 4]`, the literal was
//! `[Enum K; 4]`, and both print as `[K; 4]`.
//!
//! The same hole was reached one container over by three registrations that
//! never went through the context-aware path at all — methods in an `impl`,
//! generic functions at instantiation, and generic structs at instantiation —
//! so a bare `K` in any of those signatures failed the same way. Each has a
//! test here, because each was its own call site and could be reverted alone.
//!
//! Every program LINKS AND RUNS against a printed answer — a program that
//! type-checks and computes the wrong variant is the defect, not the cure —
//! except the last, whose receipt is the emitted C for the reason given there.

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

/// The enum every program below is about, and the function that proves which
/// variant came out: three variants so a wrong index is a wrong number.
const K: &str = r#"
enum K { A, B, C }
fn kcode(k: K) -> i64 {
    match k { K::A => { return 0; } K::B => { return 1; } K::C => { return 2; } }
}
"#;

fn with_k(body: &str) -> String {
    format!("{}{}", K, body)
}

/// The brief's program: both array literal forms against an annotation.
#[test]
fn an_array_of_an_enum_is_an_array_of_that_enum() {
    let out = run(
        &with_k(
            r#"
fn main() {
    let xs: [K; 4] = [K::A; 4];
    let ys: [K; 3] = [K::A, K::B, K::C];
    print_int(kcode(xs[0]));
    print_int(kcode(ys[2]));
}
"#,
        ),
        "enum_array_let",
    );
    assert_eq!(out, "0\n2\n");
}

/// An array of an enum as a parameter, behind a reference, walked by `for` and
/// written through an index — every position the annotation used to be wrong
/// in, in one program, each contributing a number. NOT as a return type:
/// `fn make() -> [i64; 3]` is refused with "Returning arrays from functions is
/// not yet supported" for every element type, which is not this defect.
#[test]
fn an_array_of_an_enum_crosses_every_position() {
    let out = run(
        &with_k(
            r#"
fn first(ks: [K; 3]) -> i64 { return kcode(ks[0]); }
fn second(ks: &[K; 3]) -> i64 { return kcode(ks[1]); }
fn main() {
    let mut ys: [K; 3] = [K::C, K::A, K::B];
    print_int(first(ys));
    print_int(second(&ys));
    ys[1] = K::B;
    for k in ys { print_int(kcode(k)); }
}
"#,
        ),
        "enum_array_positions",
    );
    assert_eq!(out, "2\n0\n2\n1\n1\n");
}

/// Nested: the element of the element is still the enum.
#[test]
fn an_array_of_enum_arrays_is_indexed_twice() {
    let out = run(
        &with_k(
            r#"
fn main() {
    let g: [[K; 2]; 2] = [[K::A, K::B], [K::C, K::A]];
    print_int(kcode(g[1][0]));
    print_int(kcode(g[0][1]));
}
"#,
        ),
        "enum_nested_array",
    );
    assert_eq!(out, "2\n1\n");
}

/// A struct field of array-of-enum type, and that struct as an enum payload.
/// Both are registered through the context-aware path already; what was wrong
/// was the array inside them. (The payload goes through a struct because an
/// ARRAY payload is emitted as `long long[2] field0` for every element type
/// and gcc refuses it — `enum E { Many([i64; 2]) }` does on 2563001 — which is
/// code generation's, not this.)
#[test]
fn a_field_and_a_payload_may_hold_an_array_of_an_enum() {
    let out = run(
        &with_k(
            r#"
struct S { ks: [K; 3], n: i64 }
struct Ks { ks: [K; 2] }
enum E { Many(Ks), Nothing }
fn main() {
    let s: S = S { ks: [K::A, K::B, K::C], n: 4 };
    print_int(kcode(s.ks[1]));
    print_int(s.n);
    let e: E = E::Many(Ks { ks: [K::C, K::A] });
    match e { E::Many(w) => { print_int(kcode(w.ks[0])); } E::Nothing => { print_int(9); } }
}
"#,
        ),
        "enum_array_field_payload",
    );
    assert_eq!(out, "1\n4\n2\n");
}

/// A tuple component and a reference parameter. `&K` was converted as its
/// inner type through the context-free path, so it was `Struct K` too.
#[test]
fn a_tuple_and_a_reference_may_carry_an_enum() {
    let out = run(
        &with_k(
            r#"
fn by_ref(k: &K) -> i64 { return 7; }
fn by_mut(k: &mut K) -> i64 { return 8; }
fn main() {
    let t: (K, i64) = (K::B, 5);
    print_int(kcode(t.0));
    print_int(t.1);
    let mut k: K = K::C;
    print_int(by_ref(&k));
    print_int(by_mut(&mut k));
}
"#,
        ),
        "enum_tuple_ref",
    );
    assert_eq!(out, "1\n5\n7\n8\n");
}

/// METHODS were registered through `CheckerType::from` and nothing else, so a
/// method naming an enum — as a parameter, as a return type, or as the type the
/// `impl` is FOR — was a method no call could satisfy. The receiver of a method
/// on an enum is the last case: `&self` is `&K`, a reference, so it too went
/// through the fallback, and `match self { K::A => ... }` inside `impl K` was
/// refused with `expected enum K, found K`. (An ARRAY parameter on a
/// method is refused by code generation for every element type, in its own
/// words, so it is not here.)
#[test]
fn a_method_signature_may_name_an_enum() {
    let out = run(
        &with_k(
            r#"
struct S { k: K }
impl S {
    fn get(&self) -> K { return self.k; }
    fn code_of(&self, k: K) -> i64 { return kcode(k); }
}
impl K {
    fn twice(&self, other: K) -> i64 { return kcode(other) * 2; }
    fn code(&self) -> i64 {
        match self { K::A => { return 10; } K::B => { return 11; } K::C => { return 12; } }
    }
}
fn main() {
    let s: S = S { k: K::C };
    let k: K = s.get();
    print_int(k.twice(K::C));
    print_int(k.code());
    print_int(kcode(k));
    print_int(s.code_of(K::B));
}
"#,
        ),
        "enum_method_signature",
    );
    assert_eq!(out, "4\n12\n2\n1\n");
}

/// GENERIC FUNCTIONS were instantiated through `CheckerType::from`, so a type
/// argument that is an enum — and any enum the signature names outright — came
/// back a struct.
#[test]
fn a_generic_function_may_be_instantiated_at_an_enum() {
    let out = run(
        &with_k(
            r#"
fn id<T>(x: T) -> T { return x; }
fn pick<T>(x: T, k: K) -> i64 { return kcode(k); }
fn main() {
    let b: K = K::B;
    let k: K = id(b);
    print_int(kcode(k));
    print_int(pick(5, K::C));
}
"#,
        ),
        "enum_generic_fn",
    );
    assert_eq!(out, "1\n2\n");
}

/// GENERIC STRUCTS were instantiated through `CheckerType::from` for every
/// field that is not a bare type parameter.
///
/// The local is UNANNOTATED because `let g: G<i64> = ...` emits `void* g` for
/// every generic struct, enum field or not — `struct G<T> { v: T, n: i64 }`
/// fails the same way at gcc on 2563001. That is the open "type parameters are
/// not substituted in a generic body" defect in CLAUDE.md, not this one.
#[test]
fn a_generic_struct_may_hold_an_enum() {
    let out = run(
        &with_k(
            r#"
struct G<T> { v: T, k: K }
fn main() {
    let g = G { v: 3, k: K::C };
    print_int(kcode(g.k));
    print_int(g.v);
}
"#,
        ),
        "enum_generic_struct",
    );
    assert_eq!(out, "2\n3\n");
}

/// THE NEXT BREAK BEHIND THE ONE ABOVE: a generic struct whose type ARGUMENT is
/// an enum. The type checker records each struct instantiation for code
/// generation as a list of type-argument strings, and that stringification
/// named `Struct(n)` and dropped `Enum(n)` to the literal `"Unknown"` — so on
/// 2563001 this program passed the front end and the monomorphised struct was
/// `Box2_Unknown { struct Unknown v; }`, which gcc refused. It has nothing to do
/// with how the type was SPELLED (no annotation here at all): the kind was lost
/// at the last conversion before code generation.
///
/// ASSERTED ON THE C TEXT, NOT BY RUNNING IT, and that is a measured limit
/// rather than a preference: code generation picks a generic struct
/// literal's instantiation by matching the first field's C type against
/// `i64`, `bool` and `String` only, so `Box2 { v: Qq { n: 6 } }` with a
/// STRUCT argument falls back to `(struct Box2){...}` and fails at gcc on
/// 2563001 exactly as this one does. An enum argument now fails at the same
/// place a struct argument does, and no earlier. The same reason, and the same
/// form of receipt, as `a_keyword_named_generic_struct_field_is_escaped`.
///
/// Spelled `Kk`, not `K`: a type argument written in all capitals is parsed as
/// a const parameter, which is a different refusal.
#[test]
fn a_generic_struct_instantiated_at_an_enum_names_the_enum() {
    let name = unique_module_name("enum_generic_struct_arg");
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(
        &src,
        r#"
enum Kk { A, B }
struct Box2<T> { v: T }
fn main() {
    let b = Box2 { v: Kk::B };
    match b.v { Kk::A => { print_int(0); } Kk::B => { print_int(1); } }
}
"#,
    )
    .unwrap();
    let c_file = Driver::new()
        .compile_file(&src)
        .unwrap_or_else(|e| panic!("the front end refused a legal program: {}", e));
    let c = fs::read_to_string(&c_file).unwrap();
    assert!(
        !c.contains("Unknown"),
        "an enum type argument was lost on the way to code generation:\n{}",
        c
    );
    assert!(
        c.contains("typedef struct Box2_Kk {\n    struct Kk v;\n} Box2_Kk;"),
        "the instantiation at `Kk` was not emitted as `Box2_Kk` holding a `Kk`:\n{}",
        c
    );
}
