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
