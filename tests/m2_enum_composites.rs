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
/// ASSERTED ON THE REFUSAL, NOT BY RUNNING IT, and that is a measured limit
/// rather than a preference: code generation picks a generic struct
/// literal's instantiation by matching the first field's C type against
/// `i64`, `bool` and `String` only, so `Box2 { v: Qq { n: 6 } }` with a
/// STRUCT argument fell back to `(struct Box2){...}` and failed at gcc (#63).
/// That literal is refused by name now (review round 3), and the refusal
/// lists the instantiations code generation was handed — so `Box2<Kk>` in
/// it, and no `Unknown`, is the receipt that the enum argument survived to
/// code generation.
///
/// Spelled `Kk`, not `K`: a type argument written in all capitals is parsed as
/// a const parameter, which is a different refusal.
#[test]
fn a_generic_struct_instantiated_at_an_enum_names_the_enum() {
    let text = refused(
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
    assert!(
        !text.contains("Unknown"),
        "an enum type argument was lost on the way to code generation: {}",
        text
    );
    assert!(
        text.contains("is a literal of the generic struct `Box2`") && text.contains("`Box2<Kk>`"),
        "the literal was not refused as an instantiation at `Kk`: {}",
        text
    );
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

/// The three capture shapes that stay refused, each by the refusal its
/// capture-free twin gets: a function cannot return an array at all; a struct
/// field cannot be a tuple (`struct G { a: (i64, i64) }` is refused the same
/// way on main); and an array literal cannot be passed directly as an argument
/// (#62 — bind it to a `let` first, as the row above does).
#[test]
fn an_alias_capture_that_meets_an_existing_refusal_is_refused_by_it() {
    let rows: [(&str, &str, &str); 3] = [
        (
            "array return",
            "struct T { n: i64 }\ntype A = [T; 2];\n\
             fn f<T>(x: T) -> A { return [T { n: 7 }, T { n: 8 }]; }\n\
             fn main() { let a = f(1); print_int(a[1].n); }",
            "Returning arrays from functions is not yet supported",
        ),
        (
            "tuple body, struct field",
            "struct T { n: i64 }\ntype A = (T, i64);\nstruct G<T> { v: T, a: A }\n\
             fn main() { let g = G { v: 3, a: (T { n: 7 }, 8) }; print_int(g.a.1); print_int(g.v); }",
            "Tuple types in structs not yet supported",
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
/// is refused by name (#63). The pick reads the C type of the literal's FIRST
/// field and matches it to an `i64`, `bool` or `String` argument and nothing
/// else; every other literal fell back to the uninstantiated `struct G`,
/// which gcc refused as an incomplete type. Each row below reached gcc on
/// 12042e0; on main (ad63231) four did, and the enum-first-field row was
/// refused before code generation as `expected K, found K`.
#[test]
fn a_generic_struct_literal_code_generation_cannot_instantiate_is_refused_by_name() {
    let rows: [(&str, &str, &str); 10] = [
        (
            "a struct argument (#63)",
            "struct Qq { n: i64 }\nstruct Box2<T> { v: T }\n\
             fn main() { let b = Box2 { v: Qq { n: 6 } }; print_int(b.v.n); }",
            "`Box2<Qq>`",
        ),
        (
            "a struct argument read in place",
            "struct S { n: i64 }\nstruct G<T> { v: T }\n\
             fn main() { print_int(G { v: S { n: 6 } }.v.n); }",
            "`G<S>`",
        ),
        (
            "a bool argument behind an i64 first field",
            "struct G<T> { n: i64, v: T }\n\
             fn main() { let g = G { n: 1, v: true }; print_int(g.n); }",
            "`G<bool>`",
        ),
        (
            "an enum first field",
            "enum K { A, B }\nstruct G<T> { k: K, v: T }\n\
             fn main() { let g = G { k: K::A, v: 5 }; print_int(g.v); }",
            "`G<i64>`",
        ),
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
        // The record keeps no integer width: this program never writes
        // `G<i64>`, and the refusal must not say it does.
        (
            "a u32 argument",
            "struct G<T> { v: T }\n\
             fn main() { let x: u32 = 5; let g = G { v: x }; print_int(1); }",
            "The record keeps no integer width: i32, u32 and u64 all appear as `i64`.",
        ),
        (
            "an i32 argument",
            "struct G<T> { v: T }\n\
             fn main() { let x: i32 = 5; let g = G { v: x }; print_int(1); }",
            "The record keeps no integer width: i32, u32 and u64 all appear as `i64`.",
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
/// side.
#[test]
fn a_generic_struct_literal_at_i64_bool_or_string_still_runs() {
    let rows: [(&str, &str, &str); 6] = [
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
    ];
    for (shape, source, want) in rows {
        assert_eq!(run(source, "generic_literal_runs"), want, "{}", shape);
    }
}

/// A user type may be NAMED `Unknown`. The instantiation record's placeholder
/// for an argument it cannot name is not a type name (review round 4), so a
/// struct called `Unknown` is a struct argument like any other (#63), never an
/// argument with no name.
#[test]
fn a_struct_named_unknown_is_a_struct_argument() {
    let text = refused(
        "struct Unknown { n: i64 }\nstruct G<T> { v: T }\n\
         fn main() { let g = G { v: Unknown { n: 3 } }; print_int(g.v.n); }",
        "struct_named_unknown",
    );
    assert!(
        text.contains("The instantiations the type checker recorded for `G` are `G<Unknown>`."),
        "{}",
        text
    );
    assert!(!text.contains("has no name"), "{}", text);
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
