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
//! Every accepted program LINKS AND RUNS against a printed answer — a program
//! that type-checks and computes the wrong variant is the defect, not the cure
//! — except `a_generic_struct_instantiated_at_an_enum_names_the_enum`, whose
//! receipt is the emitted C for the reason given there. The refusals assert
//! that THIS compiler refused, never gcc.

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
/// It was asserted on the refusal (review round 3, #63), because code
/// generation picked a literal's instantiation by matching the first field's C
/// type against `i64`, `bool` and `String` only. Since review round 9 it picks
/// the instantiation whose fields of a type parameter are exactly the C types
/// of the literal's values (`literal_instantiation`), so the literal is of
/// `Box2_Kk` — the receipt that the enum argument survived to code generation
/// is now that the program runs and the `match` reads the variant. gcc on main.
///
/// Spelled `Kk`, because until #93 a type argument written in all capitals was
/// parsed as a const parameter; `K` now takes this same path.
#[test]
fn a_generic_struct_instantiated_at_an_enum_names_the_enum() {
    let out = run(
        r#"
enum Kk { A, B }
struct Box2<T> { v: T }
fn main() {
    let b = Box2 { v: Kk::B };
    match b.v { Kk::A => { print_int(0); } Kk::B => { print_int(1); } }
}
"#,
        "enum_generic_struct_arg",
    );
    assert_eq!(out, "1\n");
}

// ---------------------------------------------------------------------------
// Review round 1 (af5d6ac): type parameters before global names, and aliases
// expanded before C is named.
// ---------------------------------------------------------------------------

/// Compile only, expecting a refusal from this compiler and NOT from gcc.
fn refused(source: &str, prefix: &str) -> String {
    let name = unique_module_name(prefix);
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    match Driver::new().compile_file(&src) {
        Ok(c) => panic!(
            "the front end accepted a program it must refuse: {}",
            c.display()
        ),
        Err(e) => {
            let text = e.to_string();
            assert!(
                !text.contains("gcc"),
                "the refusal came from the C compiler, not from this one: {}",
                text
            );
            text
        }
    }
}

/// A generic struct's `[T; 2]` is `[i64; 2]` when `T` is `i64` — also when a
/// global `enum T` exists. On af5d6ac the array of the ENUM type-checked here
/// and gcc refused the C; on 2563001 both programs were refused, the first as
/// `expected [T; 2], found [T; 2]`.
#[test]
fn a_generic_structs_parameter_is_substituted_inside_its_array_field() {
    let out = run(
        r#"
enum T { A, B }
struct G<T> { v: T, k: [T; 2] }
fn main() {
    let g = G { v: 3, k: [1, 2] };
    print_int(g.v);
    print_int(g.k[1]);
}
"#,
        "tparam_array_field",
    );
    assert_eq!(out, "3\n2\n");
    let text = refused(
        r#"
enum T { A, B }
struct G<T> { v: T, k: [T; 2] }
fn main() { let g = G { v: 3, k: [T::A, T::B] }; print_int(g.v); }
"#,
        "tparam_array_field_enum",
    );
    assert!(text.contains("expected [Int; 2], found [T; 2]"), "{}", text);
}

/// A generic function's `(T, i64)` is substituted too, in the type checker
/// AND in the monomorphised C; before, the checker refused it and, once the
/// checker substituted, the C declared the tuple with `void*` in it.
#[test]
fn a_generic_functions_parameter_is_substituted_inside_a_tuple() {
    let out = run(
        r#"
enum T { A, B }
fn f<T>(x: T, p: (T, i64)) -> i64 { return p.1; }
fn main() {
    let t: (i64, i64) = (4, 6);
    print_int(f(5, t));
}
"#,
        "tparam_tuple_param",
    );
    assert_eq!(out, "6\n");
    let text = refused(
        r#"
enum T { A, B }
fn f<T>(x: T, ys: [T; 2]) -> i64 { return 1; }
fn main() { let ks: [T; 2] = [T::A, T::B]; print_int(f(5, ks)); }
"#,
        "tparam_conflict",
    );
    assert!(
        text.contains("Type parameter 'T' has conflicting types: 'i64' and 'T'"),
        "{}",
        text
    );
}

/// An alias of an array names the array in every declarator: a local, a
/// parameter, a struct field, a loop, an alias of the alias. Each was
/// `long long[2] xs` — brackets before the name — and gcc refused it on
/// 2563001 for `[i64; 2]` and `[S; 2]`; for `[K; 2]` the type checker refused
/// it first, as `expected [K; 2], found [K; 2]`. A tuple alias is the control
/// that already ran.
#[test]
fn an_alias_of_an_array_is_declared_as_the_array_it_names() {
    let out = run(
        r#"
enum K { A, B }
struct S { n: i64 }
type IA = [i64; 2];
type SA = [S; 2];
type KA = [K; 2];
type KB = KA;
type KT = (K, i64);
struct H { xs: IA, n: i64 }
fn kc(k: K) -> i64 { match k { K::A => { return 0; } K::B => { return 1; } } }
fn second(xs: IA) -> i64 { return xs[1]; }
fn main() {
    let a: IA = [1, 2];
    let s: SA = [S { n: 3 }, S { n: 4 }];
    let k: KA = [K::A, K::B];
    let kb: KB = [K::B, K::A];
    let t: KT = (K::B, 5);
    let h: H = H { xs: [6, 7], n: 8 };
    print_int(a[1]);
    print_int(s[1].n);
    print_int(kc(k[1]));
    print_int(kc(kb[0]));
    print_int(kc(t.0));
    print_int(t.1);
    print_int(h.xs[1]);
    for x in a { print_int(x); }
    print_int(second(a));
}
"#,
        "alias_of_array",
    );
    assert_eq!(out, "2\n4\n1\n1\n1\n5\n7\n1\n2\n2\n");
}

/// An ARRAY payload is refused by this compiler, by name, for every element
/// type and through an alias. `enum E { Many([i64; 2]) }` reached gcc as
/// `long long[2] field0` on 2563001.
#[test]
fn an_array_payload_is_refused_before_any_c_exists() {
    for (shape, source) in [
        (
            "i64",
            "enum E { Many([i64; 2]), Nothing }\nfn main() { print_int(1); }",
        ),
        (
            "enum",
            "enum K { A, B }\nenum E { Many([K; 2]), Nothing }\nfn main() { print_int(1); }",
        ),
        (
            "alias",
            "type IA = [i64; 2];\nenum E { Many(IA), Nothing }\nfn main() { print_int(1); }",
        ),
    ] {
        let text = refused(source, "array_payload");
        assert!(
            text.contains("carries an ARRAY in its payload"),
            "{}: {}",
            shape,
            text
        );
    }
}

// ---------------------------------------------------------------------------
// Review round 2 (f841b99): a type parameter is never expanded as a global
// alias of the same name, and an alias that names nothing bound still is.
// ---------------------------------------------------------------------------

/// THE SILENT WRONG ANSWER. With `type T = bool;` beside `struct G<T>`, f841b99
/// expanded the field's `T` as the alias before monomorphising, declared
/// `G_i64.v` as an `int`, and this printed `1` — exit 0, no diagnostic. main
/// (ad63231) prints 4294967297. Every row is a valid program that must run with
/// the parameter's type, not the alias's.
#[test]
fn a_type_parameter_is_never_expanded_as_a_global_alias_of_its_name() {
    let rows: [(&str, &str, &str); 6] = [
        (
            "struct field, alias = bool",
            "type T = bool;\nstruct G<T> { v: T }\n\
             fn main() { let g = G { v: 4294967297 }; print_int(g.v); }",
            "4294967297\n",
        ),
        (
            "struct field, alias = String",
            "type T = String;\nstruct G<T> { v: T }\n\
             fn main() { let g = G { v: 3 }; print_int(g.v); }",
            "3\n",
        ),
        (
            "struct field at String, alias = i64",
            "type T = i64;\nstruct G<T> { v: T, n: i64 }\n\
             fn main() { let g = G { v: \"hi\", n: 1 }; print(g.v); print_int(g.n); }",
            "hi\n1\n",
        ),
        (
            "array field, alias = bool",
            "type T = bool;\nstruct G<T> { v: T, k: [T; 2] }\n\
             fn main() { let g = G { v: 3, k: [4294967297, 2] }; print_int(g.k[0]); }",
            "4294967297\n",
        ),
        (
            "second of two parameters",
            "type U = bool;\nstruct G<T, U> { a: T, b: U }\n\
             fn main() { let g = G { a: 1, b: 4294967297 }; print_int(g.b); }",
            "4294967297\n",
        ),
        (
            "generic fn parameter and return",
            "type T = bool;\nfn id<T>(x: T) -> T { return x; }\n\
             fn main() { print_int(id(4294967297)); }",
            "4294967297\n",
        ),
    ];
    for (shape, source, want) in rows {
        let out = run(source, "alias_vs_tparam");
        assert_eq!(out, want, "{}", shape);
    }
}

/// The same shadowing at `bool`, where the PRINTED value cannot tell: `true`
/// stored in a `long long` and read back is still 1. So the receipt is the
/// field's C type — `type T = i64;` must not make `G_bool.v` a `long long`.
#[test]
fn a_bool_instantiation_shadowing_an_i64_alias_declares_a_bool_field() {
    let source = "type T = i64;\nstruct G<T> { v: T, n: i64 }\n\
                  fn main() { let g = G { v: true, n: 1 }; \
                  if g.v { print_int(1); } else { print_int(0); } }";
    assert_eq!(run(source, "alias_vs_tparam_bool"), "1\n");
    let name = unique_module_name("alias_vs_tparam_bool_c");
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    let c = fs::read_to_string(Driver::new().compile_file(&src).unwrap()).unwrap();
    assert!(
        c.contains("typedef struct G_bool {\n    int v;\n"),
        "G_bool.v is not declared as the bool it is:\n{}",
        c
    );
}

/// The reverse control: an alias that names nothing bound expands inside a
/// generic item — and the type checker resolves it there too, through the
/// GLOBAL scope its body was written in (main refused every row here, e.g.
/// `expected A, found [Int; 2]`). An alias whose body names a parameter the
/// item binds is no longer in this list: it is refused by name below.
#[test]
fn an_alias_inside_a_generic_item_is_still_the_type_it_names() {
    let rows: [(&str, &str, &str); 5] = [
        (
            "array alias field",
            "type A = [i64; 2];\nstruct G<T> { v: T, a: A }\n\
             fn main() { let g = G { v: 3, a: [1, 2] }; print_int(g.v); print_int(g.a[1]); }",
            "3\n2\n",
        ),
        (
            "scalar alias field",
            "type N = i64;\nstruct G<T> { v: T, n: N }\n\
             fn main() { let g = G { v: 3, n: 5 }; print_int(g.n); }",
            "5\n",
        ),
        (
            "alias of an alias",
            "type N = i64;\ntype M = N;\nstruct G<T> { v: T, m: M }\n\
             fn main() { let g = G { v: 3, m: 4294967297 }; print_int(g.m); }",
            "4294967297\n",
        ),
        (
            "enum array alias field",
            "enum K { A, B }\ntype KA = [K; 2];\nstruct G<T> { v: T, ks: KA }\n\
             fn kc(k: K) -> i64 { match k { K::A => { return 0; } K::B => { return 1; } } }\n\
             fn main() { let g = G { v: 3, ks: [K::B, K::A] }; print_int(kc(g.ks[0])); }",
            "1\n",
        ),
        (
            "alias parameter of a generic fn",
            "type A = [i64; 2];\nfn f<T>(x: T, a: A) -> i64 { return a[1]; }\n\
             fn main() { let a: A = [5, 6]; print_int(f(1, a)); }",
            "6\n",
        ),
    ];
    for (shape, source, want) in rows {
        let out = run(source, "alias_in_generic");
        assert_eq!(out, want, "{}", shape);
    }
}

// ---------------------------------------------------------------------------
// Review rounds 3 and 4: an alias that names a parameter its item binds.
// ---------------------------------------------------------------------------

/// `type A = [T; 2];` written at the top level names the global `T`; inside
/// `struct G<T>` or `fn f<T>` the name `T` is the item's parameter. The alias
/// means the GLOBAL type there too: the colliding parameter is renamed before
/// type checking (`rename_capturing_binders`, src/ast/mod.rs), so the
/// expanded `T` cannot be captured by it. Round 3 refused every such use by
/// name, which lost programs main ran — a generic function returning the
/// alias, its result discarded or handed to another generic (review round 4).
/// Every row reads the aliased value back, through each consumer of a result:
/// a discarded call, a forward to another generic, an annotated binding and a
/// field read.
#[test]
fn an_alias_that_names_a_parameter_its_item_binds_means_the_global_type() {
    let rows: [(&str, &str, &str); 12] = [
        (
            "scalar body, struct field",
            "struct T { n: i64 }\ntype A = T;\nstruct G<T> { v: T, a: A }\n\
             fn main() { let g = G { v: 3, a: T { n: 7 } }; print_int(g.a.n); print_int(g.v); }",
            "7\n3\n",
        ),
        (
            "array body, struct field",
            "struct T { n: i64 }\ntype A = [T; 2];\nstruct G<T> { v: T, a: A }\n\
             fn main() { let g = G { v: 3, a: [T { n: 7 }, T { n: 8 }] }; \
             print_int(g.a[1].n); print_int(g.v); }",
            "8\n3\n",
        ),
        (
            "array of an enum, struct field",
            "enum T { A, B }\ntype X = [T; 2];\nstruct G<T> { v: T, a: X }\n\
             fn tc(t: T) -> i64 { match t { T::A => { return 0; } T::B => { return 1; } } }\n\
             fn main() { let g = G { v: 3, a: [T::A, T::B] }; print_int(tc(g.a[1])); print_int(g.v); }",
            "1\n3\n",
        ),
        (
            "alias inside an array field",
            "struct T { n: i64 }\ntype A = T;\nstruct G<T> { v: T, a: [A; 2] }\n\
             fn main() { let g = G { v: 3, a: [T { n: 7 }, T { n: 8 }] }; \
             print_int(g.a[1].n); print_int(g.v); }",
            "8\n3\n",
        ),
        (
            "the middle of a chain is the parameter",
            "type B = bool;\ntype A = B;\nstruct G<B> { v: B, a: A }\n\
             fn main() { let g = G { v: 4294967297, a: true }; print_int(g.v); \
             if g.a { print_int(1); } }",
            "4294967297\n1\n",
        ),
        (
            "through a chain of aliases",
            "struct T { n: i64 }\ntype A = T;\ntype B = A;\nstruct G<T> { v: T, a: B }\n\
             fn main() { let g = G { v: 3, a: T { n: 7 } }; print_int(g.a.n); print_int(g.v); }",
            "7\n3\n",
        ),
        (
            "array body, generic fn parameter, variable argument",
            "struct T { n: i64 }\ntype A = [T; 2];\nfn f<T>(x: T, a: A) -> i64 { return a[1].n; }\n\
             fn main() { let a: A = [T { n: 7 }, T { n: 8 }]; print_int(f(1, a)); }",
            "8\n",
        ),
        (
            "scalar body, generic fn parameter",
            "struct T { n: i64 }\ntype A = T;\nfn f<T>(x: T, a: A) -> i64 { return a.n; }\n\
             fn main() { print_int(f(1, T { n: 9 })); }",
            "9\n",
        ),
        (
            "scalar return, result discarded",
            "struct T { n: i64 }\ntype A = T;\n\
             fn f<T>(x: T) -> A { print_int(7); return T { n: 7 }; }\n\
             fn main() { f(1); }",
            "7\n",
        ),
        (
            "scalar return, result forwarded to another generic",
            "struct T { n: i64 }\ntype A = T;\nfn f<T>(x: T) -> A { return T { n: 7 }; }\n\
             fn read<X>(x: X) -> i64 { return x.n; }\n\
             fn main() { let a = f(1); print_int(read(a)); }",
            "7\n",
        ),
        (
            "scalar return, annotated binding",
            "struct T { n: i64 }\ntype A = T;\nfn f<T>(x: T) -> A { return T { n: 7 }; }\n\
             fn main() { let a: A = f(1); print_int(a.n); }",
            "7\n",
        ),
        (
            "scalar return, field read",
            "struct T { n: i64 }\ntype A = T;\nfn f<T>(x: T) -> A { return T { n: 7 }; }\n\
             fn main() { let a = f(1); print_int(a.n); }",
            "7\n",
        ),
    ];
    for (shape, source, want) in rows {
        let out = run(source, "alias_global");
        assert_eq!(out, want, "{}", shape);
    }
}

/// The two capture shapes that stay refused, each by the refusal its
/// capture-free twin gets: a function cannot return an array at all; and an
/// array literal cannot be passed directly as an argument (#62 — bind it to a
/// `let` first, as the row above does).
///
/// A third, a GENERIC struct's field of a tuple alias, was refused here as
/// its twin `struct G { a: (i64, i64) }` is ("Tuple types in structs not yet
/// supported") until review round 7: a generic struct's field now keeps a
/// tuple alias's name, as main kept it, and is declared with the tuple's
/// struct, which a monomorphised struct is emitted after — main ran
/// `struct G<T> { v: T, n: N }` under `type N = (i64, i64)`, and every
/// round since round 1 refused it. The capture row runs with it (main
/// refused it only because its tuple literal met the alias's NAME).
#[test]
fn an_alias_capture_that_meets_an_existing_refusal_is_refused_by_it() {
    let rows: [(&str, &str, &str); 2] = [
        (
            "array return",
            "struct T { n: i64 }\ntype A = [T; 2];\n\
             fn f<T>(x: T) -> A { return [T { n: 7 }, T { n: 8 }]; }\n\
             fn main() { let a = f(1); print_int(a[1].n); }",
            "Returning arrays from functions is not yet supported",
        ),
        (
            "array literal argument",
            "struct T { n: i64 }\ntype A = [T; 2];\nfn f<T>(x: T, a: A) -> i64 { return a[1].n; }\n\
             fn main() { print_int(f(1, [T { n: 7 }, T { n: 8 }])); }",
            "is passed directly as an argument",
        ),
    ];
    for (shape, source, want) in rows {
        let text = refused(source, "alias_capture_refused");
        assert!(text.contains(want), "{}: {}", shape, text);
        assert!(
            !text.contains("rename the parameter"),
            "{}: {}",
            shape,
            text
        );
    }
    let tuple_field = "struct T { n: i64 }\ntype A = (T, i64);\nstruct G<T> { v: T, a: A }\n\
         fn main() { let g = G { v: 3, a: (T { n: 7 }, 8) }; print_int(g.a.1); print_int(g.v); }";
    assert_eq!(run(tuple_field, "alias_capture_tuple_field"), "8\n3\n");
}

/// The twins that must keep running: the same alias beside a parameter of
/// ANOTHER name, in a non-generic item, a parameter that merely shares an
/// alias's name, and a capture in a generic function's BODY, which the type
/// checker does not reach. There the alias is expanded, and its `T` is the
/// global type: code generation substitutes only the function's own
/// `TypeParam`. main ran the scalar body; the array body reached gcc on main
/// and on 12042e0, where the alias kept its name and lost its size suffix.
#[test]
fn an_alias_beside_a_parameter_of_another_name_still_runs() {
    let rows: [(&str, &str, &str); 6] = [
        (
            "struct G<U>",
            "struct T { n: i64 }\ntype A = [T; 2];\nstruct G<U> { v: U, a: A }\n\
             fn main() { let g = G { v: 3, a: [T { n: 7 }, T { n: 8 }] }; \
             print_int(g.a[1].n); print_int(g.v); }",
            "8\n3\n",
        ),
        (
            "fn f<U>",
            "struct T { n: i64 }\ntype A = [T; 2];\nfn f<U>(v: U, a: A) -> i64 { return a[1].n; }\n\
             fn main() { let s: A = [T { n: 7 }, T { n: 8 }]; print_int(f(3, s)); }",
            "8\n",
        ),
        (
            "non-generic struct",
            "struct T { n: i64 }\ntype A = [T; 2];\nstruct G { a: A }\n\
             fn main() { let g: G = G { a: [T { n: 7 }, T { n: 8 }] }; print_int(g.a[1].n); }",
            "8\n",
        ),
        (
            "the parameter's own name, not an alias of it",
            "type T = bool;\nstruct G<T> { v: T }\n\
             fn main() { let g = G { v: 4294967297 }; print_int(g.v); }",
            "4294967297\n",
        ),
        (
            "scalar capture in a generic fn body",
            "struct T { n: i64 }\ntype A = T;\n\
             fn f<T>(x: T) -> i64 { let a: A = T { n: 7 }; return a.n; }\n\
             fn main() { print_int(f(1)); }",
            "7\n",
        ),
        (
            "array capture in a generic fn body",
            "struct T { n: i64 }\ntype A = [T; 2];\n\
             fn f<T>(x: T) -> i64 { let a: A = [T { n: 7 }, T { n: 8 }]; return a[1].n; }\n\
             fn main() { print_int(f(1)); }",
            "8\n",
        ),
    ];
    for (shape, source, want) in rows {
        let out = run(source, "alias_beside");
        assert_eq!(out, want, "{}", shape);
    }
}

/// A generic struct literal whose instantiation code generation cannot pick
/// is refused by name (#63). Every literal it could not pick fell back to the
/// uninstantiated `struct G`, which gcc refused as an incomplete type; each
/// row below reached gcc on 12042e0 and on main. The pick is now by the C
/// types of the literal's values (review round 9), so a struct, an enum or a
/// `bool` argument behind another field is picked and runs (the next test);
/// what is left is an argument the type checker records with no name (an
/// array, a tuple, a char, a float). An integer the record keeps no width for
/// was refused here too until review round 10: the literal is now of its own
/// record, `G<i64>`, which holds it as C converts it (the next test).
#[test]
fn a_generic_struct_literal_code_generation_cannot_instantiate_is_refused_by_name() {
    let rows: [(&str, &str, &str); 4] = [
        // The four argument kinds the type checker records with no name: the
        // refusal says so, and never prints the placeholder it records.
        (
            "an array argument",
            "struct G<T> { v: T }\n\
             fn main() { let xs: [i64; 2] = [1, 2]; let g = G { v: xs }; print_int(g.v[1]); }",
            "has no name in the instantiation record",
        ),
        (
            "a tuple argument",
            "struct G<T> { v: T }\n\
             fn main() { let p = (1, 2); let g = G { v: p }; print_int(g.v.1); }",
            "has no name in the instantiation record",
        ),
        (
            "a char argument",
            "struct G<T> { v: T }\nfn main() { let g = G { v: 'a' }; print_int(1); }",
            "has no name in the instantiation record",
        ),
        (
            "a float argument",
            "struct G<T> { v: T }\nfn main() { let g = G { v: 1.5 }; print_int(1); }",
            "has no name in the instantiation record",
        ),
    ];
    for (shape, source, want) in rows {
        let text = refused(source, "generic_literal");
        assert!(
            text.contains("is a literal of the generic struct") && text.contains(want),
            "{}: {}",
            shape,
            text
        );
        assert!(!text.contains("Unknown"), "{}: {}", shape, text);
        assert!(!text.contains("in this program at"), "{}: {}", shape, text);
    }
}

/// The literals the pick does match, which must keep running: an `i64`,
/// `bool` or `String` argument in the first field, read from a literal, a
/// variable or a call, in either field order, and two instantiations side by
/// side — and, since review round 9, a struct or an enum argument, a `bool`
/// argument behind an `i64` field and an `i64` one behind an enum field
/// (the four were refused by name from review round 3, #63, and were gcc on
/// main): the instantiation is the one whose fields of a type parameter are
/// exactly the C types of the literal's values. Since review round 10 the
/// literal is of the instantiation recorded for it, so a `u32` or an `i32` the
/// record spells `i64` is of `G<i64>`, as C converts it (refused by name on
/// b0f9c32, gcc on main; `~` reads it as the `i64` the record says it is, as
/// main read `G { tag: 1, p: v }`).
#[test]
fn a_generic_struct_literal_at_i64_bool_or_string_still_runs() {
    let rows: [(&str, &str, &str); 16] = [
        (
            "i64, bool and String",
            "struct G<T> { v: T }\nfn main() { let a = G { v: 4294967297 }; \
             let b = G { v: true }; let c = G { v: \"hi\" }; print_int(a.v); \
             if b.v { print_int(1); } print(c.v); }",
            "4294967297\n1\nhi\n",
        ),
        (
            "an i64 first field that is not the parameter, at i64",
            "struct G<T> { n: i64, v: T }\n\
             fn main() { let g = G { n: 1, v: 5 }; print_int(g.n); print_int(g.v); }",
            "1\n5\n",
        ),
        (
            "fields written in another order",
            "struct G<T> { n: i64, v: T }\n\
             fn main() { let g = G { v: true, n: 1 }; print_int(g.n); if g.v { print_int(2); } }",
            "1\n2\n",
        ),
        (
            "two parameters",
            "struct P<A, B> { a: A, b: B }\n\
             fn main() { let p = P { a: 1, b: true }; print_int(p.a); if p.b { print_int(2); } }",
            "1\n2\n",
        ),
        (
            "a variable and a call",
            "fn five() -> i64 { return 5; }\nstruct G<T> { v: T }\n\
             fn main() { let x = 4; let g = G { v: x }; let h = G { v: five() }; \
             print_int(g.v); print_int(h.v); }",
            "4\n5\n",
        ),
        (
            "a String variable read in place",
            "struct G<T> { v: T }\n\
             fn main() { let s: String = \"hi\"; print(G { v: s }.v); }",
            "hi\n",
        ),
        (
            "a struct argument (#63)",
            "struct Qq { n: i64 }\nstruct Box2<T> { v: T }\n\
             fn main() { let b = Box2 { v: Qq { n: 6 } }; print_int(b.v.n); }",
            "6\n",
        ),
        (
            "a struct argument read in place",
            "struct S { n: i64 }\nstruct G<T> { v: T }\n\
             fn main() { print_int(G { v: S { n: 6 } }.v.n); }",
            "6\n",
        ),
        (
            "a bool argument behind an i64 first field",
            "struct G<T> { n: i64, v: T }\n\
             fn main() { let g = G { n: 1, v: true }; print_int(g.n); }",
            "1\n",
        ),
        (
            "an enum first field (refused on main: `expected K, found K`)",
            "enum K { A, B }\nstruct G<T> { k: K, v: T }\n\
             fn main() { let g = G { k: K::A, v: 5 }; print_int(g.v); }",
            "5\n",
        ),
        (
            "a struct, an i64 and a bool instantiation side by side",
            "struct Qq { n: i64 }\nstruct G<T> { v: T }\n\
             fn main() { let a = G { v: Qq { n: 6 } }; let b = G { v: 7 }; let c = G { v: true }; \
             print_int(a.v.n); print_int(b.v); if c.v { print_int(1); } }",
            "6\n7\n1\n",
        ),
        (
            "a u32 argument",
            "struct G<T> { v: T }\n\
             fn main() { let x: u32 = 5; let g = G { v: x }; print_int(1); }",
            "1\n",
        ),
        (
            "an i32 argument",
            "struct G<T> { v: T }\n\
             fn main() { let x: i32 = 5; let g = G { v: x }; print_int(1); }",
            "1\n",
        ),
        (
            "a u32 argument, read",
            "struct G<T> { v: T }\n\
             fn main() { let x: u32 = 5; let g = G { v: x }; print_int(~g.v); }",
            "-6\n",
        ),
        (
            "an i32 argument, read",
            "struct G<T> { v: T }\n\
             fn main() { let x: i32 = 5; let g = G { v: x }; print_int(~g.v); }",
            "-6\n",
        ),
        (
            "a u32 argument beside an i64 one",
            "struct G<T> { v: T }\n\
             fn main() { let x: u32 = 5; let g = G { v: x }; let h = G { v: 4294967297 }; \
             print_int(~g.v); print_int(h.v); }",
            "-6\n4294967297\n",
        ),
    ];
    for (shape, source, want) in rows {
        assert_eq!(run(source, "generic_literal_runs"), want, "{}", shape);
    }
}

/// A user type may be NAMED `Unknown`. The instantiation record's placeholder
/// for an argument it cannot name is not a type name (review round 4), so a
/// struct called `Unknown` is a struct argument like any other (#63), never an
/// argument with no name: since review round 9 a struct argument is picked
/// (`literal_instantiation`), and this one is `G_Unknown`, which runs. It was
/// asserted on the refusal's record of `G<Unknown>` before; gcc on main.
#[test]
fn a_struct_named_unknown_is_a_struct_argument() {
    let out = run(
        "struct Unknown { n: i64 }\nstruct G<T> { v: T }\n\
         fn main() { let g = G { v: Unknown { n: 3 } }; print_int(g.v.n); }",
        "struct_named_unknown",
    );
    assert_eq!(out, "3\n");
}

/// A tuple reached through an alias is carried to another generic by the
/// alias's NAME (review round 4). main typed the result of `fn f<U>(x: U) ->
/// A` with `type A = (T, i64)` as the opaque `A`, so handing it to `fwd<X>`
/// instantiated `fwd` at `A`, which code generation resolved to the tuple's C
/// struct: the first row printed 5 on main. Since round 2 the result is the
/// tuple itself, and a tuple has no name to carry, so the call was refused as
/// an instantiation at `(T, i64)`. Found by handing the result of every
/// alias-capture shape to each consumer — discard, forward, annotated binding
/// and field read — not only to a field read.
#[test]
fn a_tuple_alias_result_is_carried_to_another_generic_by_its_name() {
    let rows: [(&str, &str, &str); 4] = [
        (
            "a generic's tuple-alias result, forwarded",
            "struct T { n: i64 }\ntype A = (T, i64);\nfn fwd<X>(x: X) -> i64 { return 5; }\n\
             fn f<U>(x: U) -> A { return (T { n: 7 }, 8); }\n\
             fn main() { let r = f(1); print_int(fwd(r)); }",
            "5\n",
        ),
        (
            "the same, the parameter named like the alias's global",
            "struct T { n: i64 }\ntype A = (T, i64);\nfn fwd<X>(x: X) -> i64 { return 5; }\n\
             fn f<T>(x: T) -> A { return (T { n: 7 }, 8); }\n\
             fn main() { let r = f(1); print_int(fwd(r)); }",
            "5\n",
        ),
        (
            "forwarded and handed back",
            "type A = (i64, i64);\nfn id<X>(x: X) -> X { return x; }\n\
             fn f<U>(x: U) -> A { return (1, 2); }\n\
             fn main() { let r = f(1); let s = id(r); print_int(s.1); }",
            "2\n",
        ),
        (
            "an annotated local",
            "type A = (i64, i64);\nfn fwd<X>(x: X) -> i64 { return 5; }\n\
             fn main() { let r: A = (1, 2); print_int(fwd(r)); }",
            "5\n",
        ),
    ];
    for (shape, source, want) in rows {
        assert_eq!(run(source, "tuple_alias_forward"), want, "{}", shape);
    }
}

// ---------------------------------------------------------------------------
// Review round 5: an alias name comes from the declaration that produced the
// value, and a diagnostic prints a renamed parameter as the source wrote it.
// ---------------------------------------------------------------------------

/// Compile, link and run, and say how it went instead of panicking: `RUN:`
/// and the program's output, or the first line of this compiler's refusal,
/// or `GCC` when the C it emitted did not compile. A table of rows is then
/// compared whole, so one run shows every row that differs.
fn outcome(source: &str, prefix: &str) -> String {
    let name = unique_module_name(prefix);
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    let exe = dir.path().join(&name);
    fs::write(&src, source).unwrap();
    let c_file = match Driver::new().compile_file(&src) {
        Ok(c) => c,
        Err(e) => {
            let text = e.to_string();
            let first = text.lines().next().unwrap_or("").to_string();
            return if text.contains("gcc") {
                "GCC".to_string()
            } else {
                format!("REFUSED: {}", first)
            };
        }
    };
    let out = link_command(&c_file, &exe, OptLevel::Default)
        .expect("link_command")
        .output()
        .expect("gcc");
    if !out.status.success() {
        return "GCC".to_string();
    }
    let run = Command::new(&exe).output().expect("run");
    format!("RUN: {}", String::from_utf8_lossy(&run.stdout))
}

/// AN UNUSED ALIAS NEVER DECIDES HOW A VALUE IS PASSED (review round 5).
///
/// Round 4 carried a tuple to another generic by the first alias, in sorted
/// order, whose body read the same in the type checker's spelling — and that
/// spelling keeps no integer width and no reference: `(i32, i32)`,
/// `&(i64, i64)`, `(u64, u64)` and `&mut (i64, i64)` all read `(i64, i64)`.
/// So an alias the program never uses chose the C type: beside the `B` that
/// `f` returns, `type A = (i32, i32)` instantiated `g` at `A` and gcc refused
/// `__pd_tuple2_int_int`; `type A = &(i64, i64)` read a field through a
/// pointer. main ran both and printed `4294967297`. The name now comes from
/// the declaration that produced the value: `f`'s declared return type, the
/// type argument `id` was instantiated at, the binding it was copied from.
///
/// A method's result is the fourth row: main registers a method's signature
/// with the alias's name and ran the forward; since round 1 the generic it is
/// handed to resolves the alias in its parameter, and the call was refused as
/// `expected (Int, Int), found B` (with or without a decoy).
#[test]
fn an_unused_alias_of_the_same_erased_shape_never_carries_a_value() {
    let decoys = [
        ("no decoy", ""),
        ("(i32, i32)", "type A = (i32, i32);\n"),
        ("&(i64, i64)", "type A = &(i64, i64);\n"),
        ("(u64, u64)", "type A = (u64, u64);\n"),
        ("&mut (i64, i64)", "type A = &mut (i64, i64);\n"),
        ("(i32, i32), sorting after B", "type Z = (i32, i32);\n"),
    ];
    let f = "fn f<U>(x: U) -> B { return (4294967297, 8); }\n";
    let method =
        "struct S { n: i64 }\nimpl S { fn pair(&self) -> B { return (4294967297, 8); } }\n";
    let sources = [
        ("a generic's result", f, "let r = f(1);"),
        (
            "handed back by a generic",
            f,
            "let r0 = f(1); let r = id(r0);",
        ),
        ("copied to another binding", f, "let r0 = f(1); let r = r0;"),
        (
            "a method's result",
            method,
            "let s = S { n: 1 }; let r = s.pair();",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (decoy, decl) in decoys {
        for (source, defs, bind) in sources {
            let program = format!(
                "{}type B = (i64, i64);\nfn g<X>(x: X) -> i64 {{ return x.0; }}\n\
                 fn id<X>(x: X) -> X {{ return x; }}\n{}fn main() {{ {} print_int(g(r)); }}\n",
                decl, defs, bind
            );
            let row = format!("{} / {}", decoy, source);
            got.push(format!("{}: {}", row, outcome(&program, "erased_decoy")));
            want.push(format!("{}: RUN: 4294967297\n", row));
        }
    }
    assert_eq!(got, want);
}

/// THE ALIAS NAME TRAVELS WITH THE VALUE, THROUGH EVERY EXPRESSION THAT HANDS
/// THE VALUE ON (review round 5).
///
/// main typed `f(1)` over `fn f<U>(x: U) -> B` as the name `B`, so the name
/// went wherever the value went — an `if`, a `match`, a block, a `loop`'s
/// `break`, an array and its elements, a tuple and its elements, a pattern, a
/// dereference — and `g(r)` was instantiated at `B` from any of them: every
/// row below printed `4294967297` on main. Kept on the `let` alone, the name
/// reached `g` only from a call or a binding, and each of these rows was
/// refused as an instantiation at `(i64, i64)`. The unused `A = (i32, i32)`
/// sorts before `B`, so a name recovered from the tuple would pick it.
///
/// The last rows hand on a value two DIFFERENT declarations produced (`A` and
/// `B`, or `B` and a tuple literal): no one name is the value's, so none is
/// carried, and the program is refused by this compiler — never by gcc — as
/// main refused it.
#[test]
fn a_tuple_alias_name_travels_with_the_value_through_every_expression() {
    let defs = "type A = (i32, i32);\ntype B = (i64, i64);\n\
                fn g<X>(x: X) -> i64 { return x.0; }\nfn id<X>(x: X) -> X { return x; }\n\
                fn f<U>(x: U) -> B { return (4294967297, 8); }\n\
                fn fa<U>(x: U) -> A { return (7, 8); }\n";
    let runs = [
        (
            "an `if`",
            "let c = true; let r = if c { f(1) } else { f(2) }; print_int(g(r));",
        ),
        (
            "a `match`",
            "let k = 1; let r = match k { 1 => f(1), _ => f(2) }; print_int(g(r));",
        ),
        ("a block", "let r = { f(1) }; print_int(g(r));"),
        (
            "a block's own binding",
            "let r = { let q = f(1); q }; print_int(g(r));",
        ),
        (
            "a `loop`'s `break`",
            "let r = loop { break f(1); }; print_int(g(r));",
        ),
        (
            "an array element",
            "let arr = [f(1), f(2)]; let r = arr[0]; print_int(g(r));",
        ),
        (
            "a repeated element",
            "let arr = [f(1); 2]; let r = arr[1]; print_int(g(r));",
        ),
        (
            "a tuple element",
            "let t = (f(1), 5); let r = t.0; print_int(g(r));",
        ),
        (
            "a tuple pattern",
            "let t = (f(1), 5); match t { (a, b) => { print_int(g(a)); } }",
        ),
        (
            "a dereference",
            "let r0 = f(1); let p = &r0; let r = *p; print_int(g(r));",
        ),
        (
            "an array of bindings",
            "let a = f(1); let b = f(2); let arr = [a, b]; let r = arr[1]; print_int(g(r));",
        ),
        (
            "handed back by a generic, from an `if`",
            "let c = true; let r0 = if c { f(1) } else { f(2) }; let r = id(r0); print_int(g(r));",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, body) in runs {
        let program = format!("{}fn main() {{ {} }}\n", defs, body);
        got.push(format!("{}: {}", shape, outcome(&program, "alias_travels")));
        want.push(format!("{}: RUN: 4294967297\n", shape));
    }
    let program = format!(
        "{}fn main() {{ let arr = [f(1), f(2)]; let mut s = 0; \
         for r in arr {{ s = s + g(r); }} print_int(s); }}\n",
        defs
    );
    got.push(format!(
        "a `for` element: {}",
        outcome(&program, "alias_travels")
    ));
    want.push("a `for` element: RUN: 8589934594\n".to_string());
    let two_declarations = [
        (
            "an `if` over A and B",
            "let c = true; let r = if c { fa(1) } else { f(1) };",
        ),
        (
            "a `match` over A and B",
            "let k = 2; let r = match k { 1 => fa(1), _ => f(1) };",
        ),
        (
            "an array of A and B",
            "let arr = [fa(1), f(1)]; let r = arr[1];",
        ),
        (
            "an `if` over B and a literal",
            "let c = false; let r = if c { f(1) } else { (5, 6) };",
        ),
    ];
    for (shape, bind) in two_declarations {
        let program = format!("{}fn main() {{ {} print_int(g(r)); }}\n", defs, bind);
        let verdict = outcome(&program, "alias_travels");
        let refused_here = if verdict.starts_with("REFUSED: ") {
            "REFUSED".to_string()
        } else {
            verdict
        };
        got.push(format!("{}: {}", shape, refused_here));
        want.push(format!("{}: REFUSED", shape));
    }
    assert_eq!(got, want);
}

/// `pdc compile`'s report, as a user reads it (colour codes removed).
fn printed(source: &str, prefix: &str) -> String {
    let dir = TempDir::new().unwrap();
    let src = dir
        .path()
        .join(format!("{}.pd", unique_module_name(prefix)));
    fs::write(&src, source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .current_dir(dir.path())
        .arg("compile")
        .arg(&src)
        .arg("-o")
        .arg("probe")
        .output()
        .expect("run pdc");
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    let mut plain = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for d in chars.by_ref() {
                if d == 'm' {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    plain
}

/// A DIAGNOSTIC NAMES WHAT THE SOURCE WROTE, AND ONLY A GENERATED NAME IS
/// RESPELLED (review round 5).
///
/// Round 4 removed `#<digits>` after every identifier character of the WHOLE
/// report, rename or no rename: `match 1 { "x#1" => .. }` was reported as
/// `found the String literal "x"`, a quotation of something the source does
/// not say (main quotes `"x#1"`). And the error a caller of the driver gets
/// back was never respelled at all — it said `T#1`. Each path is pinned:
/// a literal with no generic in the program; the same literal beside a binder
/// the hygiene pass renamed (`fn f<T>` under `type A = T`); and the four
/// diagnostics that print that renamed binder, by the report and by the error.
#[test]
fn a_diagnostic_names_a_renamed_parameter_as_written_and_quotes_the_source_verbatim() {
    let quoted = [
        (
            "no generic, no rename",
            "fn main() { match 1 { \"x#1\" => { print_int(1); } _ => { print_int(0); } } }",
            "found the String literal `\"x#1\"`",
        ),
        (
            "a renamed binder in the same program",
            "struct T { n: i64 }\ntype A = T;\nfn f<T>(x: T, a: A) -> i64 { return a.n; }\n\
             fn main() { print_int(f(1, T { n: 2 })); \
             match 1 { \"T#1\" => { print_int(1); } _ => { print_int(0); } } }",
            "found the String literal `\"T#1\"`",
        ),
    ];
    for (shape, source, want) in quoted {
        let report = printed(source, "quote_diag");
        assert!(report.contains(want), "{}: {}", shape, report);
        let error = refused(source, "quote_diag");
        assert!(error.contains(want), "{}: {}", shape, error);
    }
    let renamed = [
        (
            "conflicting arguments of a generic fn",
            "struct T { n: i64 } type A = T; fn f<T>(x: T, y: T) -> A { return T { n: 7 }; } \
             fn main() { let a = f(1, true); print_int(a.n); }",
            "Type parameter 'T' has conflicting types: 'i64' and 'bool'",
        ),
        (
            "a struct parameter no field infers",
            "struct T { n: i64 } type A = T; struct G<T> { a: A, v: i64 } \
             fn main() { let g = G { a: T { n: 7 }, v: 1 }; print_int(g.v); }",
            "Could not infer type parameter 'T' for struct 'G'",
        ),
        (
            "a fn parameter no argument infers",
            "struct T { n: i64 } type A = T; fn f<T>(a: A) -> i64 { return a.n; } \
             fn main() { print_int(f(T { n: 7 })); }",
            "Could not infer type parameter 'T' from function arguments",
        ),
        (
            "conflicting fields of a generic struct",
            "struct T { n: i64 } type A = T; struct G<T> { v: T, w: T, a: A } \
             fn main() { let g = G { v: 1, w: true, a: T { n: 7 } }; print_int(g.v); }",
            "Conflicting type constraints for type parameter 'T': Int vs Bool",
        ),
    ];
    for (shape, source, want) in renamed {
        let report = printed(source, "renamed_name");
        assert!(
            report.contains(want) && !report.contains("T#"),
            "{}: {}",
            shape,
            report
        );
        let error = refused(source, "renamed_name");
        assert!(
            error.contains(want) && !error.contains("T#"),
            "{}: {}",
            shape,
            error
        );
    }
}

/// AN ALIAS NESTED IN A WRITTEN TYPE NAMES THE VALUES DECLARED AS IT
/// (review round 5, found by the four-consumer sweep of the fix above).
///
/// A type written in the source keeps an alias nested in a composite as its
/// NAME — `[B; 2]` is an array of `B`, `&B` is `B` — while the alias at the top
/// of an annotation is resolved; the structural comparison is M3's. main
/// typed a generic's result as the name `B`, so `let arr: [B; 2] = [f(1),
/// f(2)]` and `let p: &B = &r` compared a name with a name and ran. Since
/// review round 1 that result is the tuple `B` names, declared as `B`
/// (`DeclaredAs`), and both were refused as `expected [B; 2], found
/// [(Int, Int); 2]`. The other direction is a generic struct's field: its
/// written `[B; 2]` is instantiated to the tuple, a method's result is still
/// the name, and `G { v: 1, a: [s.pair(), s.pair()] }` was refused as
/// `expected [(Int, Int); 2], found [B; 2]`. Every program here ran on main
/// with this output. A tuple LITERAL was declared as no alias, so the array of
/// literals under `[B; 2]` stays refused, as it is on main
/// (tests/xfail/alias_as_array_element.pd).
#[test]
fn an_alias_nested_in_a_written_type_admits_a_value_declared_as_it() {
    let defs = "type B = (i64, i64);\nstruct S { n: i64 }\n\
                impl S { fn pair(&self) -> B { return (4294967297, 8); } }\n\
                fn g<X>(x: X) -> i64 { return x.0; }\n\
                fn f<U>(x: U) -> B { return (4294967297, 8); }\n";
    let rows = [
        (
            "an array annotation, discarded",
            "",
            "let arr: [B; 2] = [f(1), f(2)]; print_int(0);",
            "RUN: 0\n",
        ),
        (
            "an array annotation, an element forwarded",
            "",
            "let arr: [B; 2] = [f(1), f(2)]; let r = arr[0]; print_int(g(r));",
            "RUN: 4294967297\n",
        ),
        (
            "an array annotation beside an unused (i32, i32) alias",
            "type A = (i32, i32);\n",
            "let arr: [B; 2] = [f(1), f(2)]; let r = arr[0]; print_int(g(r));",
            "RUN: 4294967297\n",
        ),
        (
            "an array annotation walked by `for`",
            "",
            "let arr: [B; 2] = [f(1), f(2)]; let mut s = 0; for r in arr { s = s + g(r); } \
             print_int(s); let r = arr[0]; print_int(g(r));",
            "RUN: 8589934594\n4294967297\n",
        ),
        (
            "a reference annotation",
            "",
            "let r0 = f(1); let p: &B = &r0; let r = *p; print_int(g(r));",
            "RUN: 4294967297\n",
        ),
        (
            "a generic struct's array field, from a method",
            "struct G<T> { v: T, a: [B; 2] }\n",
            "let s = S { n: 1 }; let gg = G { v: 1, a: [s.pair(), s.pair()] }; \
             let r = gg.a[0]; print_int(g(r));",
            "RUN: 4294967297\n",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, extra, body, out) in rows {
        let program = format!("{}{}fn main() {{ {} }}\n", extra, defs, body);
        got.push(format!("{}: {}", shape, outcome(&program, "nested_alias")));
        want.push(format!("{}: {}", shape, out));
    }
    let literals = format!(
        "{}fn main() {{ let arr: [B; 2] = [(1, 2), (3, 4)]; print_int(arr[0].0); }}\n",
        defs
    );
    let verdict = outcome(&literals, "nested_alias");
    got.push(format!(
        "tuple literals under [B; 2]: {}",
        if verdict.starts_with("REFUSED: ") {
            "REFUSED"
        } else {
            &verdict
        }
    ));
    want.push("tuple literals under [B; 2]: REFUSED".to_string());
    assert_eq!(got, want);
}

/// A METHOD'S SIGNATURE NAMES AN ALIAS AS A FUNCTION'S SIGNATURE DOES
/// (review round 5, found by measuring the method-result rows against main).
///
/// A function's signature resolves an alias it names at the top
/// (`ast_type_to_checker_type`); a method's was only kinded, so `fn take(&self,
/// b: X)` and `fn get(&self) -> X` kept the NAME `X`. main typed a generic's
/// result as that name too, so the two met as `X` and `X`. Since review round
/// 1 a generic's result is the type `X` names, and every place a method's
/// signature met it was refused — `s.take(f(1))` as `expected X, found
/// (Int, Int)`, `if c { s.get() } else { f(1) }`, `[s.get(), r]`, an
/// assignment either way round — for a tuple, a scalar and a struct alias
/// alike. Every program here ran on main with this output.
#[test]
fn a_method_signature_names_an_alias_as_a_function_signature_does() {
    let kinds = [
        ("a tuple alias", "type X = (i64, i64);\n", "(4294967297, 8)"),
        ("a scalar alias", "type X = i64;\n", "4294967297"),
        (
            "a struct alias",
            "struct T { n: i64 }\ntype X = T;\n",
            "T { n: 4294967297 }",
        ),
    ];
    let bodies = [
        (
            "a generic's result passed to the method",
            "print_int(s.take(f(1))); let z = f(1);",
            "RUN: 5\n3\n",
        ),
        (
            "an `if` over the method's and the generic's result",
            "let m = s.get(); let r = f(1); let c = false; let z = if c { m } else { r };",
            "RUN: 3\n",
        ),
        (
            "a `match` over the two",
            "let m = s.get(); let r = f(1); let k = 2; let z = match k { 1 => r, _ => m };",
            "RUN: 3\n",
        ),
        (
            "an array of the two",
            "let m = s.get(); let r = f(1); let xs = [m, r]; let z = xs[1];",
            "RUN: 3\n",
        ),
        (
            "the generic's result assigned over the method's",
            "let mut z = s.get(); z = f(1);",
            "RUN: 3\n",
        ),
        (
            "the method's result assigned over the generic's",
            "let mut z = f(1); z = s.get();",
            "RUN: 3\n",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (kind, decl, value) in kinds {
        let defs = format!(
            "{}struct S {{ n: i64 }}\n\
             impl S {{ fn get(&self) -> X {{ return {v}; }} \
             fn take(&self, b: X) -> i64 {{ return 5; }} }}\n\
             fn g<Q>(q: Q) -> i64 {{ return 3; }}\n\
             fn f<U>(x: U) -> X {{ return {v}; }}\n",
            decl,
            v = value
        );
        for (shape, body, out) in bodies {
            let program = format!(
                "{}fn main() {{ let s = S {{ n: 1 }}; {} print_int(g(z)); }}\n",
                defs, body
            );
            let row = format!("{} / {}", kind, shape);
            got.push(format!("{}: {}", row, outcome(&program, "method_alias")));
            want.push(format!("{}: {}", row, out));
        }
    }
    assert_eq!(got, want);
}

/// A WRITTEN ALIAS NAME ADMITS WHAT THE ALIAS NAMES, BEHIND A REFERENCE AND
/// THROUGH AN ALIAS OF AN ALIAS (review round 5).
///
/// `let p: &X = &r` keeps `X` as its name, and so does a parameter `p: &X`.
/// main typed a generic's result as the name, so `r = f(1)` met `X` as `X`
/// and ran — for a scalar alias and a struct alias as for a tuple one. The
/// result is now the type `X` names (`Int`, the struct `T`), and both were
/// refused as `expected X, found Int`; once a method's result became the
/// type its signature names, `h(&s.get())`-shaped calls, which ran, would have
/// been refused the same way. `type B2 = B;` declares nothing new: a tuple
/// returned as `B2` is declared as `B`, and the written `B2` names it.
#[test]
fn a_written_alias_name_admits_the_value_it_names_behind_a_reference() {
    let kinds = [
        ("a scalar alias", "type X = i64;\n", "4294967297"),
        (
            "a struct alias",
            "struct T { n: i64 }\ntype X = T;\n",
            "T { n: 4294967297 }",
        ),
    ];
    let bodies = [
        (
            "a generic's result behind `&X`",
            "let m = f(1); let p: &X = &m; let q = *p; print_int(g(q));",
            "RUN: 3\n",
        ),
        (
            "a method's result behind `&X`",
            "let m = s.get(); let p: &X = &m; let q = *p; print_int(g(q));",
            "RUN: 3\n",
        ),
        (
            "a method's result passed as `&X`",
            "let m = s.get(); print_int(h(&m));",
            "RUN: 7\n",
        ),
        (
            "a generic's result passed as `&X`",
            "let m = f(1); print_int(h(&m));",
            "RUN: 7\n",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (kind, decl, value) in kinds {
        let defs = format!(
            "{}struct S {{ n: i64 }}\nimpl S {{ fn get(&self) -> X {{ return {v}; }} }}\n\
             fn h(p: &X) -> i64 {{ return 7; }}\nfn g<Q>(q: Q) -> i64 {{ return 3; }}\n\
             fn f<U>(x: U) -> X {{ return {v}; }}\n",
            decl,
            v = value
        );
        for (shape, body, out) in bodies {
            let program = format!("{}fn main() {{ let s = S {{ n: 1 }}; {} }}\n", defs, body);
            let row = format!("{} / {}", kind, shape);
            got.push(format!("{}: {}", row, outcome(&program, "written_alias")));
            want.push(format!("{}: {}", row, out));
        }
    }
    let chain = "type B = (i64, i64);\ntype B2 = B;\nfn g<X>(x: X) -> i64 { return x.0; }\n\
                 fn f<U>(x: U) -> B2 { return (4294967297, 8); }\n";
    for (shape, body) in [
        (
            "an alias of an alias in an array annotation",
            "let arr: [B2; 2] = [f(1), f(2)]; let r = arr[0]; print_int(g(r));",
        ),
        (
            "an alias of an alias behind `&`",
            "let r0 = f(1); let p: &B2 = &r0; let r = *p; print_int(g(r));",
        ),
    ] {
        let program = format!("{}fn main() {{ {} }}\n", chain, body);
        got.push(format!("{}: {}", shape, outcome(&program, "written_alias")));
        want.push(format!("{}: RUN: 4294967297\n", shape));
    }
    assert_eq!(got, want);
}

/// A GENERIC'S PARAMETER WRITTEN WITH AN ALIAS INSIDE IT ADMITS WHAT THE ALIAS
/// NAMES (review round 5).
///
/// `fn k<U>(u: U, t: (B, i64))` is instantiated with `B` kept as its name
/// inside the tuple, as a written type keeps it. A method's result was that
/// name, so `k(1, (s.pair(), 5))` compared `(B, Int)` with `(B, Int)` and ran
/// on the reviewed tip (main sent it to gcc). Once a method's signature reads
/// `B` as the tuple it names, the same call compared `(B, Int)` with
/// `((Int, Int), Int)` and was refused — unless the instantiated parameter is
/// read as what it is, a written type (`TypeChecker::written_admits`).
#[test]
fn a_generic_parameter_written_with_an_alias_inside_admits_what_it_names() {
    let program = "type B = (i64, i64);\nstruct S { n: i64 }\n\
                   impl S { fn pair(&self) -> B { return (4294967297, 8); } }\n\
                   fn k<U>(u: U, t: (B, i64)) -> i64 { return t.1; }\n\
                   fn main() { let s = S { n: 1 }; print_int(k(1, (s.pair(), 5))); }\n";
    assert_eq!(outcome(program, "generic_written"), "RUN: 5\n");
}

/// A LITERAL IS NEVER ADMITTED THROUGH AN ALIAS NAME; WHAT MAIN TYPED BY THE
/// NAME ALWAYS IS (review round 5).
///
/// `let xs: [N; 2] = [f(1), f(2)]` under `type N = i64` (a scalar, a struct
/// or an enum alias alike) compared main's name `N` with the name and ran; so
/// did `[s.get(), s.get()]`. Both results are now the type `N` names, which
/// the written `N` admits. A literal is not: main gave `5`, `T { .. }`, `K::B`
/// their structural types and refused them under the name — the very refusal
/// tests/xfail/alias_nested_in_tuple_annotation.pd declares (`let e: Edge =
/// (1, 2, 30)` with `type Edge = (NodeId, NodeId, i64)`), which is M3's to
/// lift and stays refused.
#[test]
fn a_literal_is_never_admitted_through_an_alias_name() {
    let kinds = [
        ("a scalar alias", "type N = i64;\n", "4294967297"),
        (
            "a struct alias",
            "struct T { n: i64 }\ntype N = T;\n",
            "T { n: 4294967297 }",
        ),
        ("an enum alias", "enum K { A, B }\ntype N = K;\n", "K::B"),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (kind, decl, value) in kinds {
        let defs = format!(
            "{}struct S {{ n: i64 }}\nimpl S {{ fn get(&self) -> N {{ return {v}; }} }}\n\
             fn g<Q>(q: Q) -> i64 {{ return 3; }}\nfn f<U>(x: U) -> N {{ return {v}; }}\n",
            decl,
            v = value
        );
        for (shape, bind, out) in [
            (
                "generic results in `[N; 2]`",
                "let xs: [N; 2] = [f(1), f(2)];",
                "RUN",
            ),
            (
                "method results in `[N; 2]`",
                "let xs: [N; 2] = [s.get(), s.get()];",
                "RUN",
            ),
            (
                "a generic result in `(N, i64)`",
                "let t: (N, i64) = (f(1), 5);",
                "RUN",
            ),
            (
                "literals in `[N; 2]`",
                &format!("let xs: [N; 2] = [{v}, {v}];", v = value) as &str,
                "REFUSED",
            ),
            (
                "a literal in `(N, i64)`",
                &format!("let t: (N, i64) = ({}, 5);", value) as &str,
                "REFUSED",
            ),
        ] {
            let program = format!(
                "{}fn main() {{ let s = S {{ n: 1 }}; {} print_int(g(s)); }}\n",
                defs, bind
            );
            let verdict = outcome(&program, "alias_literal");
            let class = if verdict.starts_with("RUN: ") {
                "RUN"
            } else if verdict.starts_with("REFUSED: ") {
                "REFUSED"
            } else {
                "GCC"
            };
            got.push(format!("{} / {}: {}", kind, shape, class));
            want.push(format!("{} / {}: {}", kind, shape, out));
        }
    }
    let edge = "type NodeId = i64;\ntype Edge = (NodeId, NodeId, i64);\n\
                fn main() { let e: Edge = (1, 2, 30); print_int(e.2); }\n";
    let verdict = outcome(edge, "alias_literal");
    got.push(format!(
        "the xfail row's literal: {}",
        &verdict[..verdict.len().min(8)]
    ));
    want.push("the xfail row's literal: REFUSED:".to_string());
    assert_eq!(got, want);
}

/// `outcome`, with a refusal reduced to `REFUSED` — the class of a row whose
/// wording is not what the row is about.
fn verdict_class(source: &str, prefix: &str) -> String {
    let verdict = outcome(source, prefix);
    if verdict.starts_with("REFUSED: ") {
        "REFUSED".to_string()
    } else {
        verdict
    }
}

/// A VALUE main TYPED BY AN ALIAS'S NAME INSTANTIATES A GENERIC AT THAT NAME
/// (review round 6).
///
/// main gave a generic's result and a method's the alias's NAME as their type,
/// so `g(r)` was instantiated at the name: under `type N = u32`, main emitted
/// `long long g__N(unsigned int u)` and `~7` printed `4294967288`. Since review
/// round 1 the result is the type `N` names, which this checker spells `Int`
/// for every width, and `g` was instantiated as `g__i64(long long u)`: `-8`,
/// both programs running. The same erasure lost a `u64`'s sign, refused the
/// `f32`, `f64` and `char` aliases main ran (their spelling is no type argument
/// this checker reads back), and sent every `&T` alias to gcc (a pointer
/// passed as its referent). Each row ran on main with this output, including
/// the flows (`id`, `if`, `match`, an array element, a generic struct's field)
/// that carry the value to the generic. Two rows main refused, because its
/// annotation read `N` as `u32` and its value as the name: a binding annotated
/// with the alias its value was declared as now keeps it.
///
/// The second table is main's answer where main read the alias as the type it
/// names — an annotated `let`, a parameter, a function's result, a struct's
/// field — and instantiated `g` at `i64`, exactly as for the alias-free twin:
/// the width is lost there on main too (#82), and this compiler prints what
/// main prints.
#[test]
fn a_value_main_typed_by_an_alias_name_instantiates_a_generic_at_that_name() {
    let bitnot = "fn g<U>(u: U) -> i64 { return ~u; }\n";
    let add = "fn g<U>(u: U) -> i64 { return u + u; }\n";
    let gen =
        |ty: &str, v: &str| format!("type N = {};\nfn f<T>(t: T) -> N {{ return {}; }}\n", ty, v);
    let meth = |ty: &str, field: &str, ret: &str| {
        format!(
            "type N = {};\nstruct S {{ {} }}\nimpl S {{ fn get(&self) -> N {{ return {}; }} }}\n",
            ty, field, ret
        )
    };
    let by_generic = "let r = f(1); print_int(g(r));";
    let by_method = "let r = s.get(); print_int(g(r));";
    let rows: Vec<(&str, String, String, &str)> = vec![
        (
            "u32, a generic's result, `~` forwarded",
            gen("u32", "7") + bitnot,
            by_generic.to_string(),
            "RUN: 4294967288\n",
        ),
        (
            "u32, a method's result, `~` forwarded",
            meth("u32", "n: i64", "7") + bitnot,
            format!("let s = S {{ n: 0 }}; {}", by_method),
            "RUN: 4294967288\n",
        ),
        (
            "u32, a generic's result, `+` forwarded",
            gen("u32", "4294967295") + add,
            by_generic.to_string(),
            "RUN: 4294967294\n",
        ),
        (
            "u32, a method's result, `+` forwarded",
            meth("u32", "n: i64", "4294967295") + add,
            format!("let s = S {{ n: 0 }}; {}", by_method),
            "RUN: 4294967294\n",
        ),
        (
            "u64, a generic's result",
            gen("u64", "-1") + "fn g<U>(u: U) -> i64 { return u / 2 + u; }\n",
            by_generic.to_string(),
            "RUN: 9223372036854775806\n",
        ),
        (
            "f32, a generic's result",
            gen("f32", "1.5") + "fn g<U>(u: U) -> i64 { return (u * 2.0) as i64; }\n",
            by_generic.to_string(),
            "RUN: 3\n",
        ),
        (
            "f64, a method's result",
            meth("f64", "n: i64", "1.5") + "fn g<U>(u: U) -> i64 { return (u * 2.0) as i64; }\n",
            format!("let s = S {{ n: 0 }}; {}", by_method),
            "RUN: 3\n",
        ),
        (
            "char, a generic's result",
            gen("char", "'a'") + "fn g<U>(u: U) -> i64 { return 7; }\n",
            by_generic.to_string(),
            "RUN: 7\n",
        ),
        (
            "&i64, a method's result",
            meth("&i64", "n: i64", "&self.n") + "fn g<U>(u: U) -> i64 { return *u + *u; }\n",
            format!("let s = S {{ n: 4294967297 }}; {}", by_method),
            "RUN: 8589934594\n",
        ),
        (
            "&bool, a method's result",
            meth("&bool", "b: bool", "&self.b")
                + "fn g<U>(u: U) -> i64 { if *u { return 1; } return 0; }\n",
            format!("let s = S {{ b: true }}; {}", by_method),
            "RUN: 1\n",
        ),
        (
            "&String, a method's result",
            meth("&String", "t: String", "&self.t")
                + "fn g<U>(u: U) -> i64 { print(*u); return 1; }\n",
            format!("let s = S {{ t: \"s\" }}; {}", by_method),
            "RUN: s\n1\n",
        ),
        (
            "&T, a method's result",
            "struct T { n: i64 }\n".to_string()
                + &meth("&T", "t: T", "&self.t")
                + "fn g<U>(u: U) -> i64 { return (*u).n + 1; }\n",
            format!("let s = S {{ t: T {{ n: 4294967297 }} }}; {}", by_method),
            "RUN: 4294967298\n",
        ),
        (
            "through `id`",
            gen("u32", "4294967295") + add + "fn id<T>(t: T) -> T { return t; }\n",
            "let a = f(1); let b = id(a); print_int(g(b));".to_string(),
            "RUN: 4294967294\n",
        ),
        (
            "through an `if`",
            gen("u32", "4294967295") + add,
            "let c = true; let a = if c { f(1) } else { f(2) }; print_int(g(a));".to_string(),
            "RUN: 4294967294\n",
        ),
        (
            "through a `match`",
            gen("u32", "4294967295") + add,
            "let k = 2; let a = match k { 1 => f(1), _ => f(2) }; print_int(g(a));".to_string(),
            "RUN: 4294967294\n",
        ),
        (
            "through an array element",
            meth("u32", "n: i64", "4294967295") + add,
            "let s = S { n: 1 }; let xs = [s.get(), s.get()]; let e = xs[1]; print_int(g(e));"
                .to_string(),
            "RUN: 4294967294\n",
        ),
        (
            "a generic struct's alias field",
            gen("u32", "4294967295") + add + "struct G<T> { v: T, n: N }\n",
            "let gg = G { v: 1, n: f(1) }; let q = gg.n; print_int(g(q));".to_string(),
            "RUN: 4294967294\n",
        ),
        (
            "discarded",
            gen("u32", "7") + bitnot,
            "let r = f(1); print_int(0);".to_string(),
            "RUN: 0\n",
        ),
        (
            "bound again under its own alias (main refused)",
            gen("u32", "7") + bitnot,
            "let r = f(1); let z: N = r; print_int(g(z));".to_string(),
            "RUN: 4294967288\n",
        ),
        (
            "a method's result bound again under its own alias (main refused)",
            meth("u32", "n: i64", "7") + bitnot,
            "let s = S { n: 0 }; let r = s.get(); let z: N = r; print_int(g(z));".to_string(),
            "RUN: 4294967288\n",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, defs, body, out) in &rows {
        let program = format!("{}fn main() {{ {} }}\n", defs, body);
        got.push(format!("{}: {}", shape, outcome(&program, "alias_named")));
        want.push(format!("{}: {}", shape, out));
    }
    let erased = [
        (
            "an annotated `let`",
            "type N = u32;\n",
            "let r: N = 7; print_int(g(r));",
        ),
        (
            "a parameter",
            "type N = u32;\nfn k(r: N) -> i64 { return g(r); }\n",
            "print_int(k(7));",
        ),
        (
            "a function's result",
            "type N = u32;\nfn h() -> N { return 7; }\n",
            "let r = h(); print_int(g(r));",
        ),
        (
            "a struct's field",
            "type N = u32;\nstruct V { n: N }\n",
            "let v = V { n: 7 }; let q = v.n; print_int(g(q));",
        ),
        (
            "the alias-free twin",
            "",
            "let r: u32 = 7; print_int(g(r));",
        ),
    ];
    for (shape, defs, body) in erased {
        let program = format!("{}{}fn main() {{ {} }}\n", defs, bitnot, body);
        got.push(format!(
            "main's erased answer, {}: {}",
            shape,
            outcome(&program, "alias_erased")
        ));
        want.push(format!("main's erased answer, {}: RUN: -8\n", shape));
    }
    assert_eq!(got, want);
}

/// AN ASSIGNMENT, A FIELD AND A `return` ADMIT WHAT A `let` ADMITS (review
/// round 6).
///
/// `let mut arr: [B; 2] = [f(1), f(2)]` keeps `B` as its NAME in the binding's
/// type, and the `let` admits the tuples `f` declares as `B`
/// (`written_admits`). The element assignment `arr[0] = f(3)` compared the same
/// two types with `!=` and was refused as `expected B, found (Int, Int)`, and
/// so were a variable assigned a whole array, a non-generic struct's field
/// written `[N; 2]`, and a `return` of `(f(1), 5)` from `-> (N, i64)` — for a
/// tuple, a scalar and a struct alias alike. main ran each of the RUN rows
/// below with this output (the panel's `e1 e2 e5 e6 d1 d2 d3 d8`; `e5`, a
/// method's result, ran on 0c85c72 too). The REFUSED rows are refused on main
/// as well: a value no declaration named as the alias, an element read
/// through a name, a width the alias does not name. A method's parameter is
/// the type its alias names since review round 5, and main read it as the
/// NAME: an element of `[N; 2]` (the name) handed to `b: N` ran on main and
/// was refused; it meets the parameter as at a generic's (`is_or_names`), and
/// a plain function's parameter, which main resolved, still refuses it.
#[test]
fn an_assignment_a_field_and_a_return_admit_what_a_let_admits() {
    let b = "type B = (i64, i64);\nfn g<X>(x: X) -> i64 { return x.0; }\n\
             fn f<U>(x: U) -> B { return (4294967297, 8); }\n\
             struct S { n: i64 }\nimpl S { fn pair(&self) -> B { return (8589934594, 9); } }\n";
    let n32 = "type N = u32;\nfn g<U>(u: U) -> i64 { return u + u; }\n\
               fn f<U>(x: U) -> N { return 4294967295; }\n\
               struct S { n: i64 }\nimpl S { fn get(&self) -> N { return 4294967295; } }\n\
               struct H { a: [N; 2] }\nfn w() -> (N, i64) { return (f(1), 5); }\n";
    let t =
        "struct T { n: i64 }\ntype N = T;\nfn f<U>(x: U) -> N { return T { n: 4294967297 }; }\n\
             struct S { n: i64 }\nimpl S { fn get(&self) -> N { return T { n: 4294967297 }; } }\n\
             struct H { a: [N; 2] }\n";
    let rows = [
        ("e1: an element assigned, discarded", b, "let mut arr: [B; 2] = [f(1), f(2)]; arr[0] = f(3); print_int(0);", "RUN: 0\n"),
        ("e2: an element assigned, forwarded", b, "let mut arr: [B; 2] = [f(1), f(2)]; arr[0] = f(3); let r = arr[0]; print_int(g(r));", "RUN: 4294967297\n"),
        ("e5: a method's result assigned, discarded", b, "let s = S { n: 1 }; let mut arr: [B; 2] = [s.pair(), s.pair()]; arr[0] = s.pair(); print_int(0);", "RUN: 0\n"),
        ("e6: a method's result assigned, forwarded", b, "let s = S { n: 1 }; let mut arr: [B; 2] = [s.pair(), s.pair()]; arr[1] = s.pair(); let r = arr[1]; print_int(g(r));", "RUN: 8589934594\n"),
        ("d1: a scalar alias's element", "type N = i64;\nfn f<U>(x: U) -> N { return 4294967297; }\n", "let mut xs: [N; 2] = [f(1), f(2)]; xs[0] = f(3); print_int(0);", "RUN: 0\n"),
        ("d2: a struct alias's element", t, "let mut xs: [N; 2] = [f(1), f(2)]; xs[0] = f(3); print_int(0);", "RUN: 0\n"),
        ("d3: an `if` assigned", b, "let c = true; let mut arr: [B; 2] = [f(1), f(2)]; arr[0] = if c { f(3) } else { f(4) }; let r = arr[0]; print_int(g(r));", "RUN: 4294967297\n"),
        ("d8: an element of a nested array", b, "let mut m: [[B; 2]; 2] = [[f(1), f(2)], [f(3), f(4)]]; m[1][0] = f(5); let r = m[1][0]; print_int(g(r));", "RUN: 4294967297\n"),
        ("a u32 alias's element, forwarded", n32, "let mut xs: [N; 2] = [f(1), f(2)]; xs[0] = f(3); let r = xs[0]; print_int(g(r));", "RUN: 4294967294\n"),
        ("a u32 alias's element from a method, forwarded", n32, "let s = S { n: 1 }; let mut xs: [N; 2] = [s.get(), s.get()]; xs[1] = s.get(); let r = xs[1]; print_int(g(r));", "RUN: 4294967294\n"),
        ("a field's element assigned", n32, "let s = S { n: 1 }; let mut hh = H { a: [s.get(), s.get()] }; hh.a[1] = s.get(); let r = hh.a[1]; print_int(g(r));", "RUN: 4294967294\n"),
        ("a struct's field written `[N; 2]`", n32, "let hh = H { a: [f(1), f(2)] }; let r = hh.a[0]; print_int(g(r));", "RUN: 4294967294\n"),
        ("a struct alias in a struct's field, from a method", t, "let s = S { n: 1 }; let hh = H { a: [s.get(), s.get()] }; print_int(0);", "RUN: 0\n"),
        ("a `return` of `(f(1), 5)` from `-> (N, i64)`", n32, "let tt = w(); let r = tt.0; print_int(g(r));", "RUN: 4294967294\n"),
        ("e3: an element bound under the alias", b, "let mut arr: [B; 2] = [f(1), f(2)]; arr[0] = f(3); let r: B = arr[0]; print_int(r.1);", "REFUSED"),
        ("e4: an element's field read through the name", b, "let mut arr: [B; 2] = [f(1), f(2)]; arr[0] = f(3); print_int(arr[0].1);", "REFUSED"),
        ("d5: a non-generic function's tuple", "type B = (i64, i64);\nfn mk() -> B { return (4294967297, 8); }\n", "let mut arr: [B; 2] = [mk(), mk()]; arr[0] = mk(); print_int(arr[0].1);", "REFUSED"),
        ("an i64 assigned into an i32 alias's element", "type N = i32;\nfn f<U>(x: U) -> i64 { return 4294967297; }\nfn k<U>(x: U) -> N { return 5; }\n", "let mut xs: [N; 2] = [k(1), k(2)]; xs[0] = f(3); print_int(0);", "REFUSED"),
        ("i64s assigned over an i32 alias's array", "type N = i32;\nfn f<U>(x: U) -> i64 { return 4294967297; }\nfn k<U>(x: U) -> N { return 5; }\n", "let mut xs: [N; 2] = [k(1), k(2)]; xs = [f(3), f(4)]; print_int(0);", "REFUSED"),
        ("a method's `b: N` given an element of `[N; 2]`", "type N = u32;\nfn f<U>(x: U) -> N { return 4294967295; }\nstruct S { n: i64 }\nimpl S { fn take(&self, b: N) -> i64 { return 5; } }\n", "let s = S { n: 1 }; let arr: [N; 2] = [f(1), f(2)]; let e = arr[0]; print_int(s.take(e));", "RUN: 5\n"),
        ("a method's `b: B` given an element of `[B; 2]`", "type B = (i64, i64);\nfn f<U>(x: U) -> B { return (4294967297, 8); }\nstruct S { n: i64 }\nimpl S { fn take(&self, b: B) -> i64 { return 5; } }\n", "let s = S { n: 1 }; let arr: [B; 2] = [f(1), f(2)]; let e = arr[0]; print_int(s.take(e));", "RUN: 5\n"),
        ("a generic's `x: N` given an element of `[N; 2]`, forwarded", "type N = u32;\nfn g<U>(u: U) -> i64 { return u + u; }\nfn f<T>(t: T) -> N { return 4294967295; }\nfn kk<U>(u: U, x: N) -> N { return x; }\n", "let arr: [N; 2] = [f(1), f(2)]; let e = arr[0]; let y = kk(1, e); print_int(g(y));", "RUN: 4294967294\n"),
        ("a function's `b: N` given an element of `[N; 2]`", "type N = u32;\nfn f<T>(t: T) -> N { return 4294967295; }\nfn k(b: N) -> i64 { return 5; }\n", "let arr: [N; 2] = [f(1), f(2)]; let e = arr[0]; print_int(k(e));", "REFUSED"),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, defs, body, out) in rows {
        let program = format!("{}fn main() {{ {} }}\n", defs, body);
        got.push(format!(
            "{}: {}",
            shape,
            verdict_class(&program, "assign_alias")
        ));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}

/// AN ALIAS WHOSE BODY THE CHECKER ERASES ADMITS ONLY A VALUE DECLARED AS IT
/// (review round 6).
///
/// A written alias nested in a composite or behind `&` keeps its NAME, and
/// main admitted under it only a value main typed by that name. Round 5's
/// `written_admits` admitted any value whose type is what the alias names IN
/// THIS CHECKER'S SPELLING — and that spelling has one `Int` for every width,
/// one `Float` for both, and reads a reference as its referent. So an `i64`
/// went in under `type X = i32` (`let p: &X = &m`: `int *` from `long long *`,
/// an internal compiler error; `[X; 2]` of `i64`s: a silent narrowing that
/// ran), an `i32` under `type X = i64`, an `f32` under `type X = f64`, and a
/// `(i64, i64, i64)` under `(NodeId, NodeId, i64)` reached gcc. main refused
/// every REFUSED row by name. Under such an alias a value is now admitted only
/// if it was declared as an alias of the same type — a generic's or a
/// method's result, and a function's result or a binding whose declaration
/// writes the alias — and the RUN rows, which main ran (or refused by name
/// only), still run; a `bool` alias, which the checker does not erase, still
/// admits any `bool`. The same erased comparison let an
/// alias's NAME through where a generic's signature has the type it names
/// (`is_or_names`): `[X; 2]` given to `xs: [i64; 2]` was an ill-typed C call
/// under `type X = i32`, and a `&i64` alias's name went where an `i64` is
/// passed. main refused those by name too.
#[test]
fn an_alias_the_checker_erases_admits_only_a_value_declared_as_it() {
    let rows = [
        ("b3: an i64 behind `&X`, X = i32", "type X = i32;\nfn f<U>(x: U) -> i64 { return 4294967297; }\nfn h(p: &X) -> i64 { return 1; }\nfn main() { let m = f(1); let p: &X = &m; print_int(h(&m)); print_int(h(p)); }\n", "REFUSED"),
        ("b3d: the same, discarded", "type X = i32;\nfn f<U>(x: U) -> i64 { return 4294967297; }\nfn main() { let m = f(1); let p: &X = &m; print_int(0); }\n", "REFUSED"),
        ("b16: an i64 in a generic's `(X, i64)`", "type X = i32;\nfn k<U>(u: U, t: (X, i64)) -> i64 { return t.0; }\nfn f<U>(x: U) -> i64 { return 4294967297; }\nfn main() { print_int(k(1, (f(1), 5))); }\n", "REFUSED"),
        ("b4d: i64s in `[X; 2]`, discarded", "type X = i32;\nfn f<U>(x: U) -> i64 { return 4294967297; }\nfn main() { let xs: [X; 2] = [f(1), f(2)]; print_int(0); }\n", "REFUSED"),
        ("c10d: (i64, i64, i64) as `(NodeId, NodeId, i64)`", "type NodeId = i32;\ntype Edge = (NodeId, NodeId, i64);\nfn mk() -> (i64, i64, i64) { return (4294967297, 2, 30); }\nfn main() { let e: Edge = mk(); print_int(0); }\n", "REFUSED"),
        ("an i32 behind `&X`, X = i64", "type X = i64;\nfn main() { let m: i32 = 5; let p: &X = &m; print_int(0); }\n", "REFUSED"),
        ("an f32 behind `&X`, X = f64", "type X = f64;\nfn main() { let m: f32 = 1.5; let p: &X = &m; print_int(0); }\n", "REFUSED"),
        ("an i32 in `(X, i64)`, X = i64", "type X = i64;\nfn mk() -> i32 { return 5; }\nfn main() { let t: (X, i64) = (mk(), 5); print_int(t.1); }\n", "REFUSED"),
        ("an erased Int behind `&X`, X = i64", "type X = i64;\nfn mk() -> i64 { return 4294967297; }\nfn main() { let m = mk(); let p: &X = &m; print_int(0); }\n", "REFUSED"),
        ("a function's `-> X` result behind `&X`, X = i64", "type X = i64;\nfn mk() -> X { return 4294967297; }\nfn h(p: &X) -> i64 { return 7; }\nfn main() { let m = mk(); let p: &X = &m; print_int(h(&m)); }\n", "RUN: 7\n"),
        ("a binding annotated `X` behind `&X`, X = u32", "type X = u32;\nfn main() { let m: X = 7; let p: &X = &m; print_int(0); }\n", "RUN: 0\n"),
        ("generic results in `[X; 2]`, X = u32", "type X = u32;\nfn g<U>(u: U) -> i64 { return u + u; }\nfn f<T>(t: T) -> X { return 4294967295; }\nfn main() { let xs: [X; 2] = [f(1), f(2)]; let e = xs[0]; print_int(g(e)); }\n", "RUN: 4294967294\n"),
        ("method results in `[X; 2]`, X = u32", "type X = u32;\nfn g<U>(u: U) -> i64 { return u + u; }\nstruct S { n: i64 }\nimpl S { fn get(&self) -> X { return 4294967295; } }\nfn main() { let s = S { n: 1 }; let xs: [X; 2] = [s.get(), s.get()]; let e = xs[0]; print_int(g(e)); }\n", "RUN: 4294967294\n"),
        ("an alias of the alias in `[X; 2]`", "type X = u32;\ntype X2 = X;\nfn g<U>(u: U) -> i64 { return u + u; }\nfn f<T>(t: T) -> X2 { return 4294967295; }\nfn main() { let xs: [X; 2] = [f(1), f(2)]; let e = xs[0]; print_int(g(e)); }\n", "RUN: 4294967294\n"),
        ("b6: a tuple alias behind `&mut`", "type B = (i64, i64);\nfn g<X>(x: X) -> i64 { return x.0; }\nfn f<U>(x: U) -> B { return (4294967297, 8); }\nfn main() { let mut r0 = f(1); let p: &mut B = &mut r0; let r = *p; print_int(g(r)); }\n", "RUN: 4294967297\n"),
        ("a bool behind `&X`, X = bool", "type X = bool;\nfn mk() -> bool { return true; }\nfn main() { let m = mk(); let p: &X = &m; print_int(0); }\n", "RUN: 0\n"),
        ("`[X; 2]`, X = i32, given to a generic's `[i64; 2]`", "type X = i32;\nfn k1<T>(t: T) -> X { return 5; }\nfn kk<U>(u: U, xs: [i64; 2]) -> i64 { return 1; }\nfn main() { let arr: [X; 2] = [k1(1), k1(2)]; print_int(kk(1, arr)); }\n", "REFUSED"),
        ("`[X; 2]`, X = u32, in a generic struct's `[i64; 2]`", "type X = u32;\nfn k1<T>(t: T) -> X { return 4294967295; }\nstruct G<T> { v: T, a: [i64; 2] }\nfn main() { let arr: [X; 2] = [k1(1), k1(2)]; let gg = G { v: 1, a: arr }; print_int(0); }\n", "REFUSED"),
        ("a `&i64` alias's name given to a generic's `x: i64`", "type R = &i64;\nstruct S { n: i64 }\nimpl S { fn get(&self) -> R { return &self.n; } }\nfn kk<U>(u: U, x: i64) -> i64 { return x; }\nfn main() { let s = S { n: 5 }; let arr: [R; 2] = [s.get(), s.get()]; let e = arr[0]; print_int(kk(1, e)); }\n", "REFUSED"),
        ("an i32 alias's name behind `&` given to a generic's `&i64`", "type X = i32;\nfn k1<T>(t: T) -> X { return 5; }\nfn kk<U>(u: U, p: &i64) -> i64 { return 1; }\nfn main() { let arr: [X; 2] = [k1(1), k1(2)]; let e = arr[0]; let q: &X = &e; let w = *q; print_int(kk(1, &w)); }\n", "REFUSED"),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program, out) in rows {
        got.push(format!(
            "{}: {}",
            shape,
            verdict_class(program, "erased_alias")
        ));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}

/// AN ALIAS'S NAME AND A VALUE DECLARED AS THAT ALIAS ARE ONE TYPE WHERE TWO
/// VALUES MEET (review round 7).
///
/// A written type keeps an alias nested in it as its NAME: `xs[0]` over
/// `let xs: [N; 2]`, `*pn` over `pn: &N`. A generic's result, a method's and
/// a generic struct's field are, since review round 1, the type the alias
/// names, declared as the alias. main typed both as the NAME, so an `if`, a
/// `match`, a `loop`'s `break`s, an array literal, a re-assignment, a
/// comparison and the two arguments of one type parameter met a name with a
/// name and ran — under `type N = u32`, `g(r)` printed `4294967288`. Each
/// was refused here as `expected N, found Int` (or `'N' and 'i64'`, the two
/// type-argument spellings of one `type N = i64`). Every `RUN` row ran on
/// main with this output, in both operand orders, for a scalar and a tuple
/// alias. The `REFUSED` rows are main's refusals and stay refused: a literal,
/// a value no declaration names, and another alias's NAME meet no name.
#[test]
fn an_alias_name_and_a_value_declared_as_it_are_one_type_where_they_meet() {
    let head = |ty: &str, v: &str, op: &str| {
        format!(
            "type N = {ty};\nfn f<T>(t: T) -> N {{ return {v}; }}\n\
             fn g<U>(u: U) -> i64 {{ {op} }}\nstruct S {{ n: i64 }}\n\
             impl S {{ fn get(&self) -> N {{ return {v}; }} }}\n"
        )
    };
    let u32 = head("u32", "7", "return ~u;");
    let i64 = head("i64", "4294967297", "return ~u;");
    let pair = head("(i64, i64)", "(4294967297, 1)", "return ~u.0 + u.1;");
    let elem = "let xs: [N; 2] = [f(1), f(2)]; ";
    let name_first = |b: &str| format!("{elem}let a = xs[0]; let b = {b}; ");
    let name_second = |a: &str| format!("{elem}let a = {a}; let b = xs[0]; ");
    let iff = "let c = true; let r = if c { a } else { b }; ";
    let fwd = "print_int(g(r));";
    let cmp = "let q = a == b; if q { print_int(1); } else { print_int(0); }";
    let rows: Vec<(&str, &String, String, &str)> = vec![
        ("u32, `if`: a name, then a generic's result", &u32, name_first("f(3)") + iff + fwd, "RUN: 4294967288\n"),
        ("u32, `if`: a generic's result, then a name", &u32, name_second("f(3)") + iff + fwd, "RUN: 4294967288\n"),
        ("u32, `match`: a name, then a method's result", &u32, name_first("s.get()") + "let k = 1; let r = match k { 1 => a, _ => b }; " + fwd, "RUN: 4294967288\n"),
        ("u32, `match`: a method's result, then a name", &u32, name_second("s.get()") + "let k = 1; let r = match k { 1 => a, _ => b }; " + fwd, "RUN: 4294967288\n"),
        ("u32, an array literal: a name, then a generic's result", &u32, name_first("f(3)") + "let vs = [a, b]; let r = vs[0]; " + fwd, "RUN: 4294967288\n"),
        ("u32, an array literal: a generic's result, then a name", &u32, name_second("f(3)") + "let vs = [a, b]; let r = vs[0]; " + fwd, "RUN: 4294967288\n"),
        ("u32, a `loop`'s `break`s: a name, then a method's result", &u32, name_first("s.get()") + "let c = true; let r = loop { if c { break a; } break b; }; " + fwd, "RUN: 4294967288\n"),
        ("u32, a `loop`'s `break`s: a method's result, then a name", &u32, name_second("s.get()") + "let c = true; let r = loop { if c { break a; } break b; }; " + fwd, "RUN: 4294967288\n"),
        ("u32, arrays of each: `[[a; 2], [b; 2]]`", &u32, name_first("f(3)") + "let vs = [[a; 2], [b; 2]]; let r = vs[0][1]; " + fwd, "RUN: 4294967288\n"),
        ("u32, `if` over blocks: a generic's result, then a name", &u32, name_second("f(3)") + "let c = true; let r = if c { let t1 = a; t1 } else { let t2 = b; t2 }; " + fwd, "RUN: 4294967288\n"),
        ("u32, `if`: a generic struct's field, then a name behind `&N`", &u32, "let n0 = f(1); let pn: &N = &n0; let gg = G { v: 1, n: f(4) }; let a = gg.n; let b = *pn; ".to_string() + iff + fwd, "RUN: 4294967288\n"),
        ("u32, `if`, discarded", &u32, name_first("f(3)") + iff + "print_int(0);", "RUN: 0\n"),
        ("u32, a comparison: a name, then a generic's result", &u32, name_first("f(3)") + cmp, "RUN: 1\n"),
        ("i64, a comparison: a method's result, then a name", &i64, name_second("s.get()") + cmp, "RUN: 1\n"),
        ("i64, `if`: a name, then a generic's result", &i64, name_first("f(3)") + iff + fwd, "RUN: -4294967298\n"),
        ("i64, one type parameter's arguments: a name, then a generic's result", &i64, name_first("f(3)") + "let r = two(a, b); " + fwd, "RUN: -4294967298\n"),
        ("i64, one type parameter's arguments: a generic's result, then a name", &i64, name_second("f(3)") + "let r = two(a, b); " + fwd, "RUN: -4294967298\n"),
        ("(i64, i64), `if`: a name, then a generic's result", &pair, name_first("f(3)") + iff + fwd, "RUN: -4294967297\n"),
        ("(i64, i64), `match`: a method's result, then a name", &pair, name_second("s.get()") + "let k = 1; let r = match k { 1 => a, _ => b }; " + fwd, "RUN: -4294967297\n"),
        ("(i64, i64), re-assigned: a generic's result, then a name", &pair, name_second("f(3)") + "let mut r = a; r = b; " + fwd, "RUN: -4294967297\n"),
        ("(i64, i64), an array literal, discarded", &pair, name_second("f(3)") + "let vs = [a, b]; let r = vs[0]; print_int(0);", "RUN: 0\n"),
        ("u32, both names (as before)", &u32, name_first("xs[1]") + iff + fwd, "RUN: 4294967288\n"),
        ("u32, both generic results (as before)", &u32, "let a = f(3); let b = f(5); ".to_string() + iff + fwd, "RUN: 4294967288\n"),
        ("a name and a literal", &u32, "let xs: [N; 2] = [f(1), f(2)]; let a = xs[0]; let b = 7; ".to_string() + iff + fwd, "REFUSED"),
        ("a name and a binding annotated `N` (main read it as `u32`)", &u32, "let xs: [N; 2] = [f(1), f(2)]; let a = xs[0]; let b: N = 7; ".to_string() + iff + fwd, "REFUSED"),
        ("a name compared with a literal", &u32, "let xs: [N; 2] = [f(1), f(2)]; let a = xs[0]; let b = 7; ".to_string() + cmp, "REFUSED"),
        ("a name and another alias's name", &u32, "let xs: [N; 2] = [f(1), f(2)]; let ys: [M; 2] = [h(1), h(2)]; let a = xs[0]; let b = ys[0]; ".to_string() + iff + fwd, "REFUSED"),
    ];
    let extra = "type M = u32;\nfn h<T>(t: T) -> M { return 7; }\nstruct G<T> { v: T, n: N }\n\
                 fn two<T>(x: T, y: T) -> T { return y; }\n";
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, defs, body, out) in &rows {
        let program = format!("{defs}{extra}fn main() {{ let s = S {{ n: 0 }}; {body} }}\n");
        got.push(format!(
            "{}: {}",
            shape,
            verdict_class(&program, "alias_merge")
        ));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}

/// A REFERENCE ALIAS IS PASSED AS main PASSED IT (review round 7).
///
/// Since review round 1 every alias is expanded before code generation, so
/// `type A = &i64` reached it as `&i64` wherever `A` was written — and a
/// parameter `r0: A` became a reference parameter, dereferenced at every use
/// (`return r0;` was emitted `return (*r0);`), a field `p: A` met "Reference
/// types in structs not yet supported", and `&(i64, i64)` met "Unsupported
/// type in reference parameter". main kept the alias as its NAME there and
/// passed the pointer as a value, and every row but the last two ran on main
/// with this output. The alias-free twins (`r0: &i64`) are refused or gcc on
/// main and stay so: only the name is read as main read it.
///
/// The last two were gcc on main (and gcc or refused on 1dad901): a binding
/// or a field declared `A` handed to a generic instantiated it at the
/// referent, and the generic was given a pointer for a `long long`. A
/// reference alias now carries its value to a generic wherever it was
/// declared.
#[test]
fn a_reference_alias_is_passed_as_main_passed_it() {
    let rows: [(&str, &str, &str); 14] = [
        ("returned, bound under the alias, read", "type A = &i64;\nfn w(r0: A) -> A { return r0; }\nfn main() { let m = 4294967297; let r = w(&m); let z: A = r; print_int(*z); }\n", "RUN: 4294967297\n"),
        ("returned, discarded", "type A = &i64;\nfn w(r0: A) -> A { return r0; }\nfn main() { let m = 4294967297; let r = w(&m); print_int(0); }\n", "RUN: 0\n"),
        ("a struct's field, read", "type A = &i64;\nstruct H { p: A }\nfn main() { let m = 4294967297; let hh = H { p: &m }; let r = hh.p; print_int(*r); }\n", "RUN: 4294967297\n"),
        ("a parameter bound under the alias, read", "type A = &i64;\nfn pk(r0: A) -> i64 { let x: A = r0; let r = x; return *r; }\nfn main() { let m = 4294967297; print_int(pk(&m)); }\n", "RUN: 4294967297\n"),
        ("passed on to another function", "type A = &i64;\nfn kk(x: A) -> i64 { return *x; }\nfn main() { let m = 4294967297; let r0: A = &m; let r = r0; print_int(kk(r)); }\n", "RUN: 4294967297\n"),
        ("`&mut i64`, returned, read", "type A = &mut i64;\nfn w(r0: A) -> A { return r0; }\nfn main() { let mut m = 4294967297; let r = w(&mut m); print_int(*r); }\n", "RUN: 4294967297\n"),
        ("`&mut i64`, a struct's field, bound under the alias", "type A = &mut i64;\nstruct H { p: A }\nfn main() { let mut m = 4294967297; let hh = H { p: &mut m }; let r = hh.p; let z: A = r; print_int(*z); }\n", "RUN: 4294967297\n"),
        ("`&(i64, i64)`, returned, read", "type A = &(i64, i64);\nfn w(r0: A) -> A { return r0; }\nfn main() { let m = (4294967297, 1); let r = w(&m); print_int((*r).0 + (*r).1); }\n", "RUN: 4294967298\n"),
        ("`&T`, a struct's field, read", "type A = &T;\nstruct T { n: i64 }\nstruct H { p: A }\nfn main() { let m = T { n: 4294967297 }; let hh = H { p: &m }; let r = hh.p; print_int((*r).n); }\n", "RUN: 4294967297\n"),
        ("`&bool`, a parameter bound under the alias", "type A = &bool;\nfn bi(b: bool) -> i64 { if b { return 1; } return 0; }\nfn pk(r0: A) -> i64 { let x: A = r0; let r = x; return bi(*r); }\nfn main() { let m = true; print_int(pk(&m)); }\n", "RUN: 1\n"),
        ("a method's result through a generic's alias parameter", "type A = &i64;\nstruct S { n: i64 }\nfn g<U>(u: U) -> i64 { return *u + *u; }\nimpl S { fn get(&self) -> A { return &self.n; } }\nfn gk<T>(t: T, x: A) -> A { return x; }\nfn main() { let s = S { n: 4294967297 }; let y = gk(1, s.get()); let r = y; print_int(g(r)); }\n", "RUN: 8589934594\n"),
        ("the alias-free twin, returned (gcc on main)", "fn w(r0: &i64) -> &i64 { return r0; }\nfn main() { let m = 4294967297; let r = w(&m); let z: &i64 = r; print_int(*z); }\n", "GCC"),
        ("a binding annotated with it, handed to a generic (gcc on main)", "type A = &i64;\nfn g<U>(u: U) -> i64 { return *u + *u; }\nfn main() { let m = 4294967297; let r0: A = &m; print_int(g(r0)); }\n", "RUN: 8589934594\n"),
        ("a struct's field of it, handed to a generic (gcc on main)", "type A = &T;\nstruct T { n: i64 }\nstruct H { p: A }\nfn g<U>(u: U) -> i64 { return (*u).n + 1; }\nfn main() { let m = T { n: 4294967297 }; let hh = H { p: &m }; let r = hh.p; print_int(g(r)); }\n", "RUN: 4294967298\n"),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program, out) in rows {
        got.push(format!(
            "{}: {}",
            shape,
            verdict_class(program, "ref_alias")
        ));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}

/// A VALUE main TYPED BY A NARROWING ALIAS'S NAME MEETS ONLY THAT ALIAS
/// (review round 7).
///
/// main typed a generic's result, a method's and a generic struct's field by
/// the alias's NAME, and refused any other value there or beside it:
/// `expected N, found Int`. Since review round 1 they are the type the alias
/// names, which this checker spells `Int` for every width — and an `i64`
/// compared equal. Under `type N = u32`, an `i64` holding `4294967297` given
/// to a generic's or a method's `x: N`, a generic struct's `n: N`, assigned
/// over `let mut r = f(1)`, or meeting `f(1)` in an `if`, an array or a
/// comparison, ran on 1dad901 and printed `1`: a silent narrowing, where
/// main refused each by name. Refused again, in main's words. Kept: an
/// `i64` alias (its values are this checker's `Int`), and a reference alias
/// given a reference. A literal at a generic struct's or a generic's `N` ran
/// from review round 6 to 144a493 (`4294967288`, `7`); main refused both by
/// the name, and review round 8 refuses them again
/// (`a_literal_meets_a_place_main_typed_by_a_narrowing_alias_name_as_main_did`).
#[test]
fn a_value_typed_by_a_narrowing_alias_name_meets_only_that_alias() {
    let n =
        "type N = u32;\nfn f<T>(t: T) -> N { return 7; }\nfn g<U>(u: U) -> i64 { return ~u; }\n";
    let rows: [(&str, String, &str); 13] = [
        ("an `if`: a generic's result, then an i64", format!("{n}fn main() {{ let k = 4294967297; let c = false; let r = if c {{ f(1) }} else {{ k }}; print_int(r); }}\n"), "REFUSED"),
        ("an array literal", format!("{n}fn main() {{ let k = 4294967297; let vs = [f(1), k]; print_int(vs[1]); }}\n"), "REFUSED"),
        ("a comparison", format!("{n}fn main() {{ let k = 4294967297; let a = f(1); let q = a == k; if q {{ print_int(1); }} else {{ print_int(0); }} }}\n"), "REFUSED"),
        ("assigned over a generic's result", format!("{n}fn main() {{ let k = 4294967297; let mut r = f(1); r = k; print_int(r); }}\n"), "REFUSED"),
        ("assigned over a binding of it annotated `N`", format!("{n}fn main() {{ let k = 4294967297; let mut z: N = f(1); z = k; print_int(z); }}\n"), "REFUSED"),
        ("a generic's `x: N`", format!("{n}fn h<T>(t: T, x: N) -> i64 {{ return x; }}\nfn main() {{ let k = 4294967297; print_int(h(1, k)); }}\n"), "REFUSED"),
        ("a method's `x: N`", format!("{n}struct S {{ n: i64 }}\nimpl S {{ fn put(&self, x: N) -> i64 {{ return x; }} }}\nfn main() {{ let s = S {{ n: 0 }}; let k = 4294967297; print_int(s.put(k)); }}\n"), "REFUSED"),
        ("a generic struct's `n: N`", format!("{n}struct G<T> {{ v: T, n: N }}\nfn main() {{ let k = 4294967297; let gg = G {{ v: 1, n: k }}; print_int(gg.n); }}\n"), "REFUSED"),
        ("one type parameter's arguments", format!("{n}fn two<T>(x: T, y: T) -> T {{ return y; }}\nfn main() {{ let k = 4294967297; let a = f(1); let r = two(a, k); print_int(r); }}\n"), "REFUSED"),
        ("a literal in a generic struct's `n: N` (main's refusal again)", format!("{n}struct G<T> {{ v: T, n: N }}\nfn main() {{ let gg = G {{ v: 1, n: 7 }}; let q = gg.n; print_int(g(q)); }}\n"), "REFUSED"),
        ("a literal for a generic's `x: N` (main's refusal again)", format!("{n}fn h<T>(t: T, x: N) -> i64 {{ return x; }}\nfn main() {{ print_int(h(1, 7)); }}\n"), "REFUSED"),
        ("an i64 alias's value and an i64", "type M = i64;\nfn h<T>(t: T) -> M { return 7; }\nfn main() { let k = 4294967297; let c = false; let r = if c { h(1) } else { k }; print_int(r); }\n".to_string(), "RUN: 4294967297\n"),
        ("a `&i64` alias's parameter given a reference", "type A = &i64;\nfn g<U>(u: U) -> i64 { return *u + *u; }\nfn gk<V>(t: V, x: A) -> A { return x; }\nfn main() { let m = 4294967297; let r = gk(1, &m); print_int(g(r)); }\n".to_string(), "RUN: 8589934594\n"),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program, out) in &rows {
        got.push(format!(
            "{}: {}",
            shape,
            verdict_class(program, "narrowing_alias")
        ));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}

/// A GENERIC STRUCT'S FIELD OF A TUPLE ALIAS IS DECLARED AS main DECLARED IT
/// (review round 7).
///
/// Since review round 1 every alias reached code generation expanded, and a
/// field written as a tuple is refused there ("Tuple types in structs not yet
/// supported"). main never expanded an alias: `struct G<T> { v: T, n: N }`
/// under `type N = (i64, i64)` declared `n` with the tuple's struct, and the
/// generic struct's instantiation, emitted after the tuple structs, compiled
/// and ran. A generic struct's field keeps a tuple alias's name again. A
/// NON-generic struct is emitted before the tuple structs, so its tuple
/// field stays refused by name, as its alias-free twin is.
#[test]
fn a_generic_structs_tuple_alias_field_is_declared_as_main_declared_it() {
    let head = "type N = (i64, i64);\nfn f<T>(t: T) -> N { return (4294967297, 1); }\n\
                fn g<U>(u: U) -> i64 { return ~u.0 + u.1; }\n";
    let rows: [(&str, String, &str); 3] = [
        (
            "a generic struct's field, discarded",
            format!("{head}struct G<T> {{ v: T, n: N }}\nfn main() {{ let gg = G {{ v: 1, n: f(4) }}; let b = gg.n; print_int(0); }}\n"),
            "RUN: 0\n",
        ),
        (
            "a generic struct's field, forwarded",
            format!("{head}struct G<T> {{ v: T, n: N }}\nfn main() {{ let gg = G {{ v: 1, n: f(4) }}; let b = gg.n; print_int(g(b)); }}\n"),
            "RUN: -4294967297\n",
        ),
        (
            "a non-generic struct's field (refused by name, as its twin)",
            format!("{head}struct H {{ n: N }}\nfn main() {{ let h = H {{ n: f(4) }}; let b = h.n; print_int(0); }}\n"),
            "REFUSED",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program, out) in &rows {
        got.push(format!(
            "{}: {}",
            shape,
            verdict_class(program, "tuple_alias_field")
        ));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}

/// TWO ALIASES OF TWO TYPES ARE NOT ONE TYPE WHERE THEY MEET (review round 8).
///
/// A value's type here is this checker's spelling, which has one `Int` for
/// every width and reads a reference as its referent: under `type A = &i64`
/// and `type B = &u32` (or `type B = i64`, or `&bool` beside `bool`) a
/// method's result of each compared equal. main typed both by their NAMES and
/// refused every row below that meets the two, in these words; 144a493 took
/// them for one type and reached C with a `long long *` given for an
/// `unsigned int *` (gcc, or "the C backend emitted ill-typed C"), or ran the
/// comparison of a pointer with an integer. Refused again, as main refused
/// them. Kept: two values of one alias, and two aliases of one type — `B =
/// &i64` beside `A` is `A`'s type (main refused it by the two names; it is
/// the same C type, and has run since review round 7).
///
/// The second table is a `&(i64, i64)` alias's value beside a `(i64, i64)`
/// alias's: refused as main refused it, by the names; two of the first now
/// run (main refused the producer itself, by the name, and 144a493 refused
/// the generic instantiated at the tuple). A non-generic struct's field of
/// the reference alias is refused in code generation, as a tuple field is:
/// it named a tuple struct emitted after it, and was gcc on main and on
/// 144a493.
#[test]
fn two_aliases_of_two_types_are_not_one_type_where_they_meet() {
    let get = |s: &str, alias: &str, body: &str| {
        format!("struct {s} {{ n: {body} }}\nimpl {s} {{ fn get(&self) -> {alias} {{ return &self.n; }} }}\n")
    };
    let value = |s: &str, alias: &str, v: &str| {
        format!(
            "struct {s} {{ n: i64 }}\nimpl {s} {{ fn get(&self) -> {alias} {{ return {v}; }} }}\n"
        )
    };
    let both = "fn two<U>(x: U, y: U) -> U { return x; }\nstruct P2<T> { x: T, y: T }\n";
    let deref = "fn g<U>(u: U) -> i64 { return ~(*u); }\n";
    let rr = format!(
        "type A = &i64;\ntype B = &u32;\n{}{}{deref}{both}",
        get("S1", "A", "i64"),
        get("S2", "B", "u32")
    );
    let rv = format!(
        "type A = &i64;\ntype B = i64;\n{}{}{deref}{both}",
        get("S1", "A", "i64"),
        value("S2", "B", "4294967298")
    );
    let bb = format!(
        "type A = &bool;\ntype B = bool;\n{}{}fn g<U>(u: U) -> i64 {{ if *u {{ return 1; }} return 0; }}\n{both}",
        get("S1", "A", "bool"),
        value("S2", "B", "false")
    );
    let ri = format!("type A = &i64;\n{}{deref}{both}", get("S1", "A", "i64"));
    let se = format!(
        "type A = &i64;\ntype B = &i64;\n{}{}{deref}{both}",
        get("S1", "A", "i64"),
        get("S2", "B", "i64")
    );
    let sa = format!("type A = &i64;\n{}{deref}{both}", get("S1", "A", "i64"));
    let two = "let r = two(a, b); print_int(g(r));";
    let iff = "let c = true; let r = if c { a } else { b }; print_int(g(r));";
    let rows: Vec<(&str, &String, &str, &str, &str)> = vec![
        (
            "&i64 / &u32, one type parameter's arguments",
            &rr,
            "S2 { n: 9 }",
            two,
            "REFUSED: Type parameter 'U' has conflicting types: 'A' and 'B'",
        ),
        (
            "&i64 / &u32, the other order",
            &rr,
            "S2 { n: 9 }",
            "let r = two(b, a); print_int(0);",
            "REFUSED: Type parameter 'U' has conflicting types: 'B' and 'A'",
        ),
        ("&i64 / &u32, `if`", &rr, "S2 { n: 9 }", iff, "REFUSED"),
        (
            "&i64 / &u32, `match`",
            &rr,
            "S2 { n: 9 }",
            "let k = 1; let r = match k { 1 => a, _ => b }; print_int(g(r));",
            "REFUSED",
        ),
        (
            "&i64 / &u32, an array literal",
            &rr,
            "S2 { n: 9 }",
            "let vs = [a, b]; let r = vs[0]; print_int(g(r));",
            "REFUSED",
        ),
        (
            "&i64 / &u32, a comparison",
            &rr,
            "S2 { n: 9 }",
            "let q = a == b; if q { print_int(1); } else { print_int(0); }",
            "REFUSED",
        ),
        (
            "&i64 / &u32, re-assigned",
            &rr,
            "S2 { n: 9 }",
            "let mut r = a; r = b; print_int(g(r));",
            "REFUSED",
        ),
        (
            "&i64 / &u32, a generic struct literal",
            &rr,
            "S2 { n: 9 }",
            "let pp = P2 { x: a, y: b }; let r = pp.x; print_int(g(r));",
            "REFUSED",
        ),
        (
            "&i64 / i64, one type parameter's arguments",
            &rv,
            "S2 { n: 0 }",
            two,
            "REFUSED: Type parameter 'U' has conflicting types: 'A' and 'B'",
        ),
        ("&i64 / i64, `if`", &rv, "S2 { n: 0 }", iff, "REFUSED"),
        (
            "&i64 / i64, a comparison",
            &rv,
            "S2 { n: 0 }",
            "let q = a == b; if q { print_int(1); } else { print_int(0); }",
            "REFUSED",
        ),
        (
            "&bool / bool, one type parameter's arguments",
            &bb,
            "S2 { n: 0 }",
            two,
            "REFUSED: Type parameter 'U' has conflicting types: 'A' and 'B'",
        ),
        (
            "&i64 / an i64 no alias names, one type parameter's arguments",
            &ri,
            "",
            "let b = 4294967298; let r = two(a, b); print_int(g(r));",
            "REFUSED: Type parameter 'U' has conflicting types: 'A' and 'i64'",
        ),
        (
            "&i64 / an i64 no alias names, `if`",
            &ri,
            "",
            "let b = 4294967298; let c = true; let r = if c { a } else { b }; print_int(g(r));",
            "REFUSED",
        ),
        (
            "one alias twice, one type parameter's arguments",
            &sa,
            "S1 { n: 9 }",
            two,
            "RUN: -4294967298\n",
        ),
        (
            "one alias twice, `if`",
            &sa,
            "S1 { n: 9 }",
            iff,
            "RUN: -4294967298\n",
        ),
        (
            "two aliases of `&i64`, one type parameter's arguments",
            &se,
            "S2 { n: 9 }",
            two,
            "RUN: -4294967298\n",
        ),
        (
            "two aliases of `&i64`, `if`",
            &se,
            "S2 { n: 9 }",
            iff,
            "RUN: -4294967298\n",
        ),
        (
            "two aliases of `&i64`, re-assigned",
            &se,
            "S2 { n: 9 }",
            "let mut r = a; r = b; print_int(g(r));",
            "RUN: -10\n",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, defs, second, body, out) in &rows {
        let bind = if second.is_empty() {
            String::new()
        } else {
            format!("let sb = {second}; let b = sb.get(); ")
        };
        let program = format!(
            "{defs}fn main() {{ let sa = S1 {{ n: {} }}; let a = sa.get(); {bind}{body} }}\n",
            if defs.contains("type A = &bool") {
                "true"
            } else {
                "4294967297"
            }
        );
        let verdict = outcome(&program, "two_aliases");
        let verdict = if *out == "REFUSED" && verdict.starts_with("REFUSED: ") {
            "REFUSED".to_string()
        } else {
            verdict
        };
        got.push(format!("{}: {}", shape, verdict));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
    // A `&(i64, i64)` alias's value is declared as it (`declares_tuple`): it
    // was a tuple declared as nothing, one type with a `(i64, i64)` alias's
    // value, and reached C as a pointer where a struct stands. main refused
    // each row below where `fb` returns it, by the name.
    let tuples = "type A = (i64, i64);\ntype B = &(i64, i64);\n\
                  struct S1 { n: i64 }\nimpl S1 { fn get(&self) -> A { return (4294967297, 1); } }\n\
                  fn fb<T>(t: T, r0: B) -> B { return r0; }\n\
                  fn two<U>(x: U, y: U) -> U { return x; }\n\
                  fn g<U>(u: U) -> i64 { return (*u).0 + (*u).1; }\n";
    let bind =
        "let sa = S1 { n: 0 }; let a = sa.get(); let mb = (4294967298, 2); let b = fb(1, &mb); ";
    let tuple_rows: [(&str, String, &str); 5] = [
        ("`(i64, i64)` / `&(i64, i64)`, `if`", format!("{bind}let c = true; let r = if c {{ a }} else {{ b }}; print_int(0);"), "REFUSED"),
        ("`(i64, i64)` / `&(i64, i64)`, one type parameter's arguments", format!("{bind}let r = two(a, b); print_int(0);"), "REFUSED"),
        ("`(i64, i64)` / `&(i64, i64)`, re-assigned", format!("{bind}let mut r = a; r = b; print_int(0);"), "REFUSED"),
        ("one `&(i64, i64)` alias twice, `if`", format!("{bind}let b2 = fb(1, &mb); let c = true; let r = if c {{ b }} else {{ b2 }}; print_int(g(r));"), "RUN: 4294967300\n"),
        ("a struct's field of a `&(i64, i64)` alias (gcc on main)", "let mb = (4294967298, 2); let hh = H { p: &mb }; print_int(0);".to_string(), "REFUSED"),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, body, out) in &tuple_rows {
        let field = if body.contains("H {") {
            "struct H { p: B }\n"
        } else {
            ""
        };
        let program = format!("{tuples}{field}fn main() {{ {body} }}\n");
        got.push(format!(
            "{}: {}",
            shape,
            verdict_class(&program, "two_aliases")
        ));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}

/// A LITERAL MEETS A PLACE main TYPED BY A NARROWING ALIAS'S NAME AS main DID
/// (review round 8).
///
/// main typed a generic's and a method's parameter, a generic struct's field,
/// and a binding holding a generic's or a method's result by an alias's
/// NAME, and refused a literal there: `expected N, found Int`. Since review
/// round 6 a literal was admitted at such a place as an annotation admits it
/// — and the place keeps the alias, so under `type N = u32` the assignment
/// `z = 3` into `let mut z: N = f(1)` ran on 144a493 and printed `4294967292`
/// (`~` in `unsigned int`), where main refused the program and the alias-free
/// twin `let mut z: u32 = 7; z = 3` prints `-4`. A literal is now read as
/// the type this checker spells it, an `i64`, and refused under the name of
/// an `i32`, `u32`, `u64` or `f32` alias — every refusal row is main's
/// refusal. Kept, each as main ran it: an annotation, an assignment into a
/// binding annotated `N`, a non-generic function's parameter and a struct's
/// field (main read each as the type `N` names), and a function's `return`.
#[test]
fn a_literal_meets_a_place_main_typed_by_a_narrowing_alias_name_as_main_did() {
    let head = |ty: &str, v: &str, op: &str| {
        format!(
            "type N = {ty};\nfn f<T>(t: T) -> N {{ return {v}; }}\nfn g<U>(u: U) -> i64 {{ {op} }}\n\
             fn hn(x: N) -> N {{ return x; }}\nfn gen<T>(t: T, x: N) -> N {{ return x; }}\n\
             struct S {{ n: i64 }}\nimpl S {{ fn get(&self) -> N {{ return {v}; }} fn take(&self, x: N) -> N {{ return x; }} }}\n\
             struct G<T> {{ v: T, n: N }}\nstruct H2 {{ n: N }}\nfn fl<T>(t: T) -> N {{ return 3; }}\n\
             struct GA<T> {{ v: T, a: [N; 2] }}\n"
        )
    };
    let u32 = head("u32", "7", "return ~u;");
    let i32 = head("i32", "7", "return ~u;");
    let u64 = head("u64", "7", "return u / 2 + u;");
    let i64 = head("i64", "7", "return ~u;");
    let f32 = "type N = f32;\nfn f<T>(t: T) -> N { return 1.5; }\nfn g<U>(u: U) -> i64 { return (u * 2.0) as i64; }\n\
               fn gen<T>(t: T, x: N) -> N { return x; }\n"
        .to_string();
    let fwd = "print_int(g(r));";
    let rows: Vec<(&str, &String, String, &str)> = vec![
        (
            "assigned into a binding of a generic's result",
            &u32,
            "let r0 = f(1); let mut z: N = r0; z = 3; let r = z; ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "assigned over a generic's result",
            &u32,
            "let mut r = f(1); r = 9; ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "assigned over a method's result",
            &u32,
            "let mut r = s.get(); r = 3; ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "assigned into an array of generic results",
            &u32,
            "let mut xs = [f(1), f(2)]; xs[0] = 3; let r = xs[0]; ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "assigned into a generic struct's `n: N`",
            &u32,
            "let mut gg = G { v: 1, n: f(4) }; gg.n = 3; let r = gg.n; ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "a generic's `x: N`",
            &u32,
            "let r = gen(1, 3); ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "a method's `x: N`",
            &u32,
            "let r = s.take(7); ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "a generic struct's `n: N`",
            &u32,
            "let gg = G { v: 1, n: 7 }; let r = gg.n; ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "`i32`, assigned over a generic's result",
            &i32,
            "let mut r = f(1); r = 4294967297; ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "`u64`, a generic's `x: N`",
            &u64,
            "let r = gen(1, -3); ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "`f32`, assigned over a generic's result",
            &f32,
            "let mut r = f(1); r = 16777217.0; ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "`f32`, a generic's `x: N`",
            &f32,
            "let r = gen(1, 2.5); ".to_string() + fwd,
            "REFUSED",
        ),
        (
            "an annotation (main: the type N names)",
            &u32,
            "let r: N = 3; ".to_string() + fwd,
            "RUN: -4\n",
        ),
        (
            "an annotation of a wider literal",
            &u32,
            "let r: N = 4294967297; ".to_string() + fwd,
            "RUN: -2\n",
        ),
        (
            "assigned into a binding annotated `N`",
            &u32,
            "let mut r: N = 7; r = 3; ".to_string() + fwd,
            "RUN: -4\n",
        ),
        (
            "a non-generic function's `x: N`",
            &u32,
            "let r = hn(3); ".to_string() + fwd,
            "RUN: -4\n",
        ),
        (
            "a struct's `n: N`",
            &u32,
            "let h2 = H2 { n: 3 }; let r = h2.n; ".to_string() + fwd,
            "RUN: -4\n",
        ),
        (
            "returned from a generic's `-> N`",
            &u32,
            "let r = fl(1); ".to_string() + fwd,
            "RUN: 4294967292\n",
        ),
        (
            "`i64`, a generic's `x: N` (an i64 literal is the type N names)",
            &i64,
            "let r = gen(1, 3); ".to_string() + fwd,
            "RUN: -4\n",
        ),
        (
            "an array of `i64`s for a generic struct's `a: [N; 2]` (fable's b12, as before)",
            &u32,
            "let k = 4294967297; let gg = GA { v: 1, a: [k, k] }; let r = gg.a[0]; ".to_string()
                + fwd,
            "REFUSED",
        ),
        (
            "an array of generic results for a generic struct's `a: [N; 2]` (as before)",
            &u32,
            "let gg = GA { v: 1, a: [f(1), f(2)] }; let r = gg.a[0]; ".to_string() + fwd,
            "RUN: 4294967288\n",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, defs, body, out) in &rows {
        let program = format!("{defs}fn main() {{ let s = S {{ n: 0 }}; {body} }}\n");
        let program = if defs.contains("type N = f32") {
            program.replace("let s = S { n: 0 }; ", "")
        } else {
            program
        };
        got.push(format!(
            "{}: {}",
            shape,
            verdict_class(&program, "literal_by_name")
        ));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}

/// A GENERIC STRUCT'S TYPE ARGUMENT IS CARRIED BY THE ALIAS AS A GENERIC
/// FUNCTION'S IS (review round 9).
///
/// main typed a method's result `A` by the alias's NAME and recorded the
/// literal `G { tag: 1, p: a }` of `struct G<X, Y> { tag: X, p: Y }` as `G<i64,
/// A>`: under `type A = &i64` it emitted `G_i64_A { long long tag; long long*
/// p; }` and printed through the pointer. Since review round 1 the record was
/// rebuilt from this checker's spelling — `i64` for a reference alias, the
/// placeholder `?` for a tuple alias — so the struct declared `long long p`
/// for a pointer, or was named `G_i64_?`, and each row main ran was a gcc
/// failure on 6c793f0. A value is now recorded by the alias it is carried by
/// where a generic function's argument is (`carrier`), and code generation
/// picks a literal's instantiation by the C types of its values: it matched
/// the first field against any argument by substring, and `"long
/// long*".contains("long long")` chose an `i64` field for a pointer.
///
/// The rows main refused run with the value the program means: the annotated
/// re-binding and the field read (main's `A` met the type it names), a
/// one-parameter `G { v: a }` and `P2 { x: a, y: b }` (main's code generation
/// could not type the field), two instantiations side by side (main chose one
/// for both). A `u32` alias's value keeps its width through the field (6c793f0
/// printed `-8`). A plain `&i64` is still recorded as `i64` — the type checker
/// reads a reference as its referent — and is refused by name where it was
/// gcc on main and on 6c793f0. The controls (no generic struct; the field
/// declared `A`) ran on every compiler.
#[test]
fn a_generic_structs_type_argument_is_carried_by_the_alias_a_generic_functions_is() {
    let reference = "type A = &i64;\nstruct S { n: i64 }\n\
                     impl S { fn get(&self) -> A { return &self.n; } }\n\
                     fn rd<U>(u: U) -> i64 { return ~(*u); }\n";
    let tuple = "type A = (i64, i64);\nstruct S { n: i64 }\n\
                 impl S { fn get(&self) -> A { return (4294967297, 8); } }\n\
                 fn rd<U>(u: U) -> i64 { return ~u.0 + u.1; }\n";
    let narrow = "type A = u32;\nstruct S { n: i64 }\n\
                  impl S { fn get(&self) -> A { return 7; } }\n\
                  fn rd<U>(u: U) -> i64 { return ~u; }\n";
    let two = "struct G<X, Y> { tag: X, p: Y }\n";
    let one = "struct G<X> { v: X }\n";
    let fixed = "struct G<X> { tag: X, p: A }\n";
    let pair = "struct S2 { n: i64 }\nimpl S2 { fn get(&self) -> A { return &self.n; } }\n\
                struct P2<T> { x: T, y: T }\n";
    let bind = "let s = S { n: 4294967297 }; let a = s.get(); ";
    let lit = "let gg = G { tag: 1, p: a }; ";
    let pp = "let sb = S2 { n: 4294967298 }; let b = sb.get(); let pp = P2 { x: a, y: b }; ";
    let rows: Vec<(&str, &str, &str, String, &str)> = vec![
        ("&i64 alias, discarded", reference, two, format!("{lit}print_int(0);"), "RUN: 0\n"),
        ("&i64 alias, forwarded to a generic", reference, two, format!("{lit}let r = gg.p; print_int(rd(r));"), "RUN: -4294967298\n"),
        ("&i64 alias, re-bound under the alias (refused on main)", reference, two, format!("{lit}let r = gg.p; let z: A = r; print_int(rd(z));"), "RUN: -4294967298\n"),
        ("&i64 alias, the field read (refused on main)", reference, two, format!("{lit}print_int(~(*gg.p));"), "RUN: -4294967298\n"),
        ("&i64 alias in the first field", reference, two, "let gg = G { tag: a, p: 1 }; let r = gg.tag; print_int(rd(r));".to_string(), "RUN: -4294967298\n"),
        ("&i64 alias, one parameter (refused on main)", reference, one, "let gg = G { v: a }; let r = gg.v; print_int(rd(r));".to_string(), "RUN: -4294967298\n"),
        ("&i64 alias beside an i64 instantiation (gcc on main)", reference, two, format!("{lit}let hh = G {{ tag: 2, p: 3 }}; let r = gg.p; print_int(rd(r)); print_int(hh.p);"), "RUN: -4294967298\n3\n"),
        ("&i64 alias, `P2 { x: a, y: b }` (refused on main)", reference, pair, format!("{pp}let r = pp.x; print_int(rd(r));"), "RUN: -4294967298\n"),
        ("&i64 alias, `P2`'s two fields read (refused on main)", reference, pair, format!("{pp}print_int(~(*pp.x) + *pp.y);"), "RUN: 0\n"),
        ("&i64 alias, no generic struct (control)", reference, "", "print_int(rd(a));".to_string(), "RUN: -4294967298\n"),
        ("&i64 alias, the field declared `A` (control)", reference, fixed, format!("{lit}let r = gg.p; print_int(rd(r));"), "RUN: -4294967298\n"),
        ("(i64, i64) alias, discarded", tuple, two, format!("{lit}print_int(0);"), "RUN: 0\n"),
        ("(i64, i64) alias, forwarded to a generic", tuple, two, format!("{lit}let r = gg.p; print_int(rd(r));"), "RUN: -4294967290\n"),
        ("(i64, i64) alias, re-bound under the alias (refused on main)", tuple, two, format!("{lit}let r = gg.p; let z: A = r; print_int(rd(z));"), "RUN: -4294967290\n"),
        ("(i64, i64) alias, the field read (refused on main)", tuple, two, format!("{lit}print_int(~gg.p.0 + gg.p.1);"), "RUN: -4294967290\n"),
        ("(i64, i64) alias, no generic struct (control)", tuple, "", "print_int(rd(a));".to_string(), "RUN: -4294967290\n"),
        ("(i64, i64) alias, the field declared `A` (control)", tuple, fixed, format!("{lit}let r = gg.p; print_int(rd(r));"), "RUN: -4294967290\n"),
        ("u32 alias, forwarded to a generic", narrow, two, format!("{lit}let r = gg.p; print_int(rd(r));"), "RUN: 4294967288\n"),
        ("u32 alias, the field read (refused on main)", narrow, two, format!("{lit}print_int(~gg.p);"), "RUN: 4294967288\n"),
        // main ran these; 6c793f0 sent each to gcc.
        ("&i64 alias beside a String", reference, two, "let gg = G { tag: \"hi\", p: a }; print(gg.tag); let r = gg.p; print_int(rd(r));".to_string(), "RUN: hi\n-4294967298\n"),
        ("&i64 alias, two literals of one instantiation", reference, two, format!("{lit}let hh = G {{ tag: 2, p: a }}; let r = gg.p; let q = hh.p; print_int(rd(r) + rd(q));"), "RUN: -8589934596\n"),
        ("&i64 alias, two literals met in an `if`", reference, two, "let t = S { n: 3 }; let b = t.get(); let c = true; let gg = if c { G { tag: 1, p: a } } else { G { tag: 2, p: b } }; let r = gg.p; print_int(rd(r));".to_string(), "RUN: -4294967298\n"),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, head, decl, body, out) in &rows {
        let program = format!("{head}{decl}fn main() {{ {bind}{body} }}\n");
        got.push(format!(
            "{}: {}",
            shape,
            outcome(&program, "gstruct_carrier")
        ));
        want.push(format!("{}: {}", shape, out));
    }
    // A plain reference is recorded as its referent: refused by name, never
    // a `long long` field initialised from a pointer (gcc on main, 6c793f0).
    // A struct alias's value handed in as a parameter (recorded `P`, its field
    // `struct P`: one type), and a `&bool` alias's: main ran both.
    for (shape, program, out) in [
        (
            "a struct alias's parameter value",
            "type A = P;\nstruct P { n: i64 }\nfn rd<U>(u: U) -> U { return u; }\n\
             struct G<X, Y> { tag: X, p: Y }\n\
             fn h(a: A) { let gg = G { tag: 1, p: a }; let r = gg.p; print_int(rd(r).n); }\n\
             fn main() { h(P { n: 4294967297 }); }\n",
            "RUN: 4294967297\n",
        ),
        (
            "a &bool alias (gcc on 6c793f0)",
            "type A = &bool;\nstruct S { n: bool }\nimpl S { fn get(&self) -> A { return &self.n; } }\n\
             struct G<X, Y> { tag: X, p: Y }\nfn rb<U>(u: U) -> i64 { if *u { return 1; } return 0; }\n\
             fn main() { let s = S { n: true }; let a = s.get(); let gg = G { tag: 1, p: a }; \
             let r = gg.p; print_int(rb(r)); }\n",
            "RUN: 1\n",
        ),
    ] {
        got.push(format!("{}: {}", shape, outcome(program, "gstruct_carrier")));
        want.push(format!("{}: {}", shape, out));
    }
    for (shape, program) in [
        (
            "a plain &i64, one parameter",
            "struct G<X> { v: X }\nfn main() { let m = 7; let b = G { v: &m }; print_int(*b.v); }\n",
        ),
        (
            "two plain &i64, `P2`",
            "struct P2<T> { x: T, y: T }\n\
             fn main() { let m = 7; let n = 8; let pp = P2 { x: &m, y: &n }; print_int(*pp.x); }\n",
        ),
    ] {
        got.push(format!("{}: {}", shape, verdict_class(program, "gstruct_carrier")));
        want.push(format!("{}: REFUSED", shape));
    }
    assert_eq!(got, want);
}

/// A GENERIC STRUCT'S LITERAL IS OF THE INSTANTIATION THE TYPE CHECKER
/// RECORDED FOR IT (review round 10).
///
/// Round 9 had code generation pick, among ALL the recorded instantiations,
/// the first whose fields were exactly the C types of a literal's values. So
/// an unused `G { tag: 1, p: s.get() }` under `type z = u32`, recorded `G<i64,
/// z>`, took `G { tag: 1, p: v }` with `let v: u32 = 7` — recorded `G<i64,
/// i64>`, as `v` is not typed by the alias's name, but of C type `unsigned
/// int` — and an `if`, `match`, `loop` or array that met it with `G { tag: 2,
/// p: w }` (`w: i64`) put a `G_i64_i64` where a `G_i64_z` was declared: gcc,
/// where main and 6c793f0 printed `4294967297`. The same unread literal turned
/// `~gg.p` from main's `-8` into `4294967288`. The type checker now records
/// the instantiation for each literal (`StructLiteralRecords`) and the literal
/// is of that one; without the unused literal (the controls) nothing changes.
///
/// Two such literals meeting — `G<i64, z>` from a value typed by the alias's
/// name and `G<i64, i64>` — in an `if`, a `match`, a `loop`, an array or an
/// assignment compared equal in this checker and reached C as two structs
/// (gcc on b0f9c32); they are refused, as main refused each.
///
/// Kept: a value typed by the alias's name is of `G<i64, z>` (main refused
/// `~` of it); a plain `&i64`, which the record reads as its referent, is of
/// the instantiation keeping the reference where one exists (main ran it
/// there); an `i32` first field beside a `G<bool, i64>` (main chose that one;
/// its own record holds the values as C converts them). Two rows are main's
/// selector choosing another literal's instantiation and narrowing (`1`): a
/// `bool` and an `i64` instantiation side by side (#76), and an unused `G<i64,
/// A>` under `type A = u32`, which sorts before `i64`; each prints the value
/// its literal holds, as its control without the other literal does on main.
#[test]
fn a_generic_struct_literal_is_of_the_instantiation_recorded_for_it() {
    let z = "type z = u32;\nstruct S { n: i64 }\nimpl S { fn get(&self) -> z { return 7; } }\n\
             struct G<X, Y> { tag: X, p: Y }\nfn id<T>(t: T) -> T { return t; }\n";
    let a_ref =
        "type A = &i64;\nstruct S { n: i64 }\nimpl S { fn get(&self) -> A { return &self.n; } }\n\
                 struct G<X, Y> { tag: X, p: Y }\n";
    let bare = "struct G<X, Y> { tag: X, p: Y }\n";
    let a_u32 = "type A = u32;\nstruct S { n: i64 }\nimpl S { fn get(&self) -> A { return 7; } }\n\
                 struct G<X, Y> { tag: X, p: Y }\nfn id<T>(t: T) -> T { return t; }\n";
    let decoy = "let s = S { n: 0 }; let unused = G { tag: 1, p: s.get() }; ";
    let vw = "let v: u32 = 7; let w: i64 = 4294967297; ";
    let iff = "let c = false; let r = if c { G { tag: 1, p: v } } else { G { tag: 2, p: w } }; ";
    let byname = "let s = S { n: 0 }; let v = s.get(); let w: i64 = 4294967297; ";
    let rows: Vec<(&str, &str, String, &str)> = vec![
        ("unused G<i64, z>, an `if`, the field read", z, format!("{decoy}{vw}{iff}print_int(r.p);"), "RUN: 4294967297\n"),
        ("unused G<i64, z>, an `if`, discarded", z, format!("{decoy}{vw}{iff}print_int(0);"), "RUN: 0\n"),
        ("unused G<i64, z>, an `if`, the field forwarded to a generic", z, format!("{decoy}{vw}{iff}let x = r.p; print_int(id(x));"), "RUN: 4294967297\n"),
        ("unused G<i64, z>, an `if`, the field re-bound annotated", z, format!("{decoy}{vw}{iff}let x: i64 = r.p; print_int(x);"), "RUN: 4294967297\n"),
        ("unused G<i64, z>, an array", z, format!("{decoy}{vw}let a = G {{ tag: 1, p: v }}; let b = G {{ tag: 2, p: w }}; let xs = [a, b]; print_int(xs[1].p);"), "RUN: 4294967297\n"),
        ("unused G<i64, z>, a `match`", z, format!("{decoy}{vw}let c = 1; let r = match c {{ 0 => G {{ tag: 1, p: v }}, _ => G {{ tag: 2, p: w }} }}; print_int(r.p);"), "RUN: 4294967297\n"),
        ("unused G<i64, z>, a `loop`", z, format!("{decoy}{vw}let c = false; let r = loop {{ if c {{ break G {{ tag: 1, p: v }}; }} break G {{ tag: 2, p: w }}; }}; print_int(r.p);"), "RUN: 4294967297\n"),
        ("unused G<i64, z> after the `if`", z, format!("{vw}{iff}{decoy}print_int(r.p);"), "RUN: 4294967297\n"),
        ("unused G<String, z>, a `String` tag", z, format!("let s = S {{ n: 0 }}; let unused = G {{ tag: \"x\", p: s.get() }}; {vw}let c = false; let r = if c {{ G {{ tag: \"x\", p: v }} }} else {{ G {{ tag: \"y\", p: w }} }}; print_int(r.p);"), "RUN: 4294967297\n"),
        ("unused G<i64, z>, `~` of a u32 the checker records as i64", z, format!("{decoy}let v: u32 = 7; let gg = G {{ tag: 1, p: v }}; print_int(~gg.p);"), "RUN: -8\n"),
        ("no unused literal, an `if` (control)", z, format!("{vw}{iff}print_int(r.p);"), "RUN: 4294967297\n"),
        ("no unused literal, `~` of the u32 (control)", z, "let v: u32 = 7; let gg = G { tag: 1, p: v }; print_int(~gg.p);".to_string(), "RUN: -8\n"),
        ("a by-name z value is of G<i64, z> (refused on main)", z, "let s = S { n: 0 }; let v = s.get(); let gg = G { tag: 1, p: v }; print_int(~gg.p);".to_string(), "RUN: 4294967288\n"),
        ("a plain &i64 beside an unused G<i64, A>", a_ref, "let s = S { n: 5 }; let unused = G { tag: 1, p: s.get() }; let m: i64 = 4294967297; let g = G { tag: 2, p: &m }; print_int(*g.p);".to_string(), "RUN: 4294967297\n"),
        ("an i32 first field beside a G<bool, i64>", bare, "let d = G { tag: true, p: 5 }; let k: i32 = 3; let w: i64 = 4294967297; let g = G { tag: k, p: w }; print_int(g.p);".to_string(), "RUN: 4294967297\n"),
        ("a bool and an i64 instantiation (main: 1, #76)", bare, "let a = G { tag: 1, p: true }; let b = G { tag: 2, p: 4294967297 }; print_int(b.p);".to_string(), "RUN: 4294967297\n"),
        ("an unused G<i64, A> under `type A = u32` (main: 1)", a_u32, format!("{decoy}{vw}{iff}print_int(r.p);"), "RUN: 4294967297\n"),
        ("two by-name z values met in an `if` (refused on main)", z, format!("{byname}let s2 = S {{ n: 1 }}; let v2 = s2.get(); let c = true; let r = if c {{ G {{ tag: 1, p: v }} }} else {{ G {{ tag: 2, p: v2 }} }}; print_int(~r.p);"), "RUN: 4294967288\n"),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, head, body, out) in &rows {
        let program = format!("{head}fn main() {{ {body} }}\n");
        got.push(format!(
            "{}: {}",
            shape,
            outcome(&program, "literal_record")
        ));
        want.push(format!("{}: {}", shape, out));
    }
    // A value typed by the alias's name (`G<i64, z>`) meeting an `i64` one
    // (`G<i64, i64>`) is two C structs: refused, as main refused each (gcc on
    // b0f9c32, which compared the two equal).
    for (shape, meet) in [
        ("an `if`", "let c = true; let r = if c { G { tag: 1, p: v } } else { G { tag: 2, p: w } };"),
        ("a `match`", "let c = 0; let r = match c { 0 => G { tag: 1, p: v }, _ => G { tag: 2, p: w } };"),
        ("a `loop`", "let c = true; let r = loop { if c { break G { tag: 1, p: v }; } break G { tag: 2, p: w }; };"),
        ("an array", "let a = G { tag: 1, p: v }; let b = G { tag: 2, p: w }; let xs = [a, b]; let r = xs[0];"),
        ("an assignment", "let mut r = G { tag: 2, p: w }; r = G { tag: 1, p: v };"),
        ("an assignment the other way", "let mut r = G { tag: 1, p: v }; r = G { tag: 2, p: w };"),
    ] {
        let program = format!("{z}fn main() {{ {byname}{meet} print_int(r.p); }}\n");
        got.push(format!("by-name z and i64 in {}: {}", shape, verdict_class(&program, "literal_record")));
        want.push(format!("by-name z and i64 in {}: REFUSED", shape));
    }
    // A plain reference no instantiation keeps is still refused by name, never
    // put in a `long long` field (gcc on main), with or without another
    // literal's scalar instantiation beside it.
    for (shape, program) in [
        (
            "a plain &i64 alone",
            "struct G<X, Y> { tag: X, p: Y }\n\
             fn main() { let m: i64 = 7; let g = G { tag: 2, p: &m }; print_int(*g.p); }\n",
        ),
        (
            "a plain &i64 beside an unused G<i64, z>",
            "type z = u32;\nstruct S { n: i64 }\nimpl S { fn get(&self) -> z { return 7; } }\n\
             struct G<X, Y> { tag: X, p: Y }\n\
             fn main() { let s = S { n: 0 }; let unused = G { tag: 1, p: s.get() }; \
             let m: i64 = 7; let g = G { tag: 2, p: &m }; print_int(*g.p); }\n",
        ),
    ] {
        got.push(format!(
            "{}: {}",
            shape,
            verdict_class(program, "literal_record")
        ));
        want.push(format!("{}: REFUSED", shape));
    }
    assert_eq!(got, want);
}

/// The verdict `outcome` gives, for a program of several files: `main.pd` and
/// the modules it imports, compiled by `pdc` in a scratch directory (the module
/// resolver searches `.`, as `tests/m3_imported_calls.rs` says).
fn outcome_of_files(modules: &[(&str, &str)], main: &str) -> String {
    let dir = TempDir::new().unwrap();
    for (name, src) in modules {
        fs::write(dir.path().join(name), src).unwrap();
    }
    fs::write(dir.path().join("main.pd"), main).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .args(["compile", "main.pd", "-o", "prog"])
        .current_dir(dir.path())
        .output()
        .expect("pdc");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success() {
        return if text.contains("gcc") {
            "GCC".to_string()
        } else {
            let first = text.lines().find(|l| l.contains("error")).unwrap_or("");
            format!("REFUSED: {}", first)
        };
    }
    let run = Command::new(dir.path().join("build_output").join("prog"))
        .current_dir(dir.path())
        .output()
        .expect("run");
    format!("RUN: {}", String::from_utf8_lossy(&run.stdout))
}

/// A STRUCT LITERAL IS RECORDED AS ITSELF, NOT AS ITS SPAN (review round 11).
///
/// Round 10 recorded the instantiation of each generic struct literal by the
/// literal's span, and by whether it sat in an imported module. A span is not
/// a literal. The macro expander parses every expansion again from offset 0,
/// so under `macro pack!(x) { G { tag: 1, p: $x } }` the expansions
/// `pack!(a)`, `pack!(b)` and `pack!(w)` were three literals at one span; and
/// two imported files may hold literals at one offset. Their records merged
/// (`[i64, z]` beside `[i64, i64]`), and code generation chose among them by C
/// type again. An unused `pack!(a)` (`a` typed by the name of `type z = u32`)
/// made `pack!(b)` (`b: u32`, recorded `G<i64, i64>`) a `G_i64_z`. An `if`
/// meeting it with `pack!(w)` then assigned one struct to the other: gcc, where
/// main printed `4294967297`. And `~` of it printed `4294967288`, where main
/// printed `-8`. Renaming `a` to `aliaswide` (another span) or one more space
/// in an imported file "fixed" it. Every literal now carries an identity of its
/// own from the parser (`LiteralId`), the record is keyed by it, and a span is
/// for diagnostics only. The controls (another span, no unused literal) print
/// the same value; so do the rows they control.
#[test]
fn a_struct_literal_is_recorded_as_itself_not_as_its_span() {
    let z = "type z = u32;\nstruct S { n: i64 }\nimpl S { fn get(&self) -> z { return 7; } }\n\
             struct G<X, Y> { tag: X, p: Y }\nfn id<T>(t: T) -> T { return t; }\n";
    let pack = "macro pack!(x) { G { tag: 1, p: $x } }\n";
    let two = "macro alpha!(x) { G { tag: 1, p: $x } }\nmacro beta!(x) { G { tag: 1, p: $x } }\n";
    let decoy = "let s = S { n: 0 }; let a = s.get(); let d = pack!(a); ";
    let vw = "let b: u32 = 7; let w: i64 = 4294967297; let c = false; ";
    let iff = "let r = if c { pack!(b) } else { pack!(w) }; ";
    let rows: Vec<(&str, String, &str)> = vec![
        ("one macro, an `if`, the field read", format!("{pack}fn main() {{ {decoy}{vw}{iff}print_int(r.p); }}"), "RUN: 4294967297\n"),
        ("one macro, an `if`, discarded", format!("{pack}fn main() {{ {decoy}{vw}{iff}print_int(0); }}"), "RUN: 0\n"),
        ("one macro, an `if`, the field forwarded", format!("{pack}fn main() {{ {decoy}{vw}{iff}let q = r.p; print_int(id(q)); }}"), "RUN: 4294967297\n"),
        ("one macro, an `if`, the field re-bound annotated", format!("{pack}fn main() {{ {decoy}{vw}{iff}let q: i64 = r.p; print_int(q); }}"), "RUN: 4294967297\n"),
        ("two macros of one body, an `if`", format!("{two}fn main() {{ let s = S {{ n: 0 }}; let a = s.get(); let d = alpha!(a); {vw}let r = if c {{ beta!(b) }} else {{ beta!(w) }}; print_int(r.p); }}"), "RUN: 4294967297\n"),
        ("the unused expansion in another function", format!("{pack}fn other() -> i64 {{ {decoy}return 0; }}\nfn main() {{ {vw}{iff}print_int(r.p); }}"), "RUN: 4294967297\n"),
        ("`~` of a u32 the checker records as i64", format!("{pack}fn main() {{ {decoy}let b: u32 = 7; let r = pack!(b); print_int(~r.p); }}"), "RUN: -8\n"),
        ("`~`, the unused argument named `aliaswide` (control)", format!("{pack}fn main() {{ let s = S {{ n: 0 }}; let aliaswide = s.get(); let d = pack!(aliaswide); let b: u32 = 7; let r = pack!(b); print_int(~r.p); }}"), "RUN: -8\n"),
        ("`~`, no unused expansion (control)", format!("{pack}fn main() {{ let b: u32 = 7; let r = pack!(b); print_int(~r.p); }}"), "RUN: -8\n"),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, body, out) in &rows {
        let program = format!("{z}{body}\n");
        got.push(format!("{}: {}", shape, outcome(&program, "literal_node")));
        want.push(format!("{}: {}", shape, out));
    }
    // Two imported files whose literals sit at one offset: the literal in
    // `one` is padded to start where the first one in `two` does. The controls
    // pad it by one space more.
    let main = "import lib1;\nimport lib2;\ntype z = u32;\nstruct S { n: i64 }\n\
                impl S { fn get(&self) -> z { return 7; } }\nstruct G<X, Y> { tag: X, p: Y }\n\
                fn main() { print_int(two()); }\n";
    let one = |head: &str, tail: &str, two: &str, extra: usize| {
        let pad = two.find("G {").unwrap() - head.len() + extra;
        format!("{head}{}{tail}", " ".repeat(pad))
    };
    let unused = "pub fn one() -> i64 { let s = S { n: 0 }; let a = s.get(); let gg =";
    let bitnot =
        "pub fn two() -> i64 { let a: u32 = 7; let gg =                       G { tag: 1, p: a }; \
                  return ~gg.p; }\n";
    let iff = "pub fn two() -> i64 { let b: u32 = 7; let w = 4294967297; let c = false; \
               let r = if c { G { tag: 1, p: b } } else { G { tag: 2, p: w } }; return r.p; }\n";
    for (shape, two, extra, out) in [
        (
            "two files, one offset, `~` of a u32",
            bitnot,
            0,
            "RUN: -8\n",
        ),
        (
            "two files, another offset, `~` (control)",
            bitnot,
            1,
            "RUN: -8\n",
        ),
        (
            "two files, one offset, an `if`",
            iff,
            0,
            "RUN: 4294967297\n",
        ),
        (
            "two files, another offset, an `if` (control)",
            iff,
            1,
            "RUN: 4294967297\n",
        ),
    ] {
        let one = one(unused, "G { tag: 1, p: a }; return 0; }\n", two, extra);
        let files = [("lib1.pd", one.as_str()), ("lib2.pd", two)];
        got.push(format!("{}: {}", shape, outcome_of_files(&files, main)));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}
