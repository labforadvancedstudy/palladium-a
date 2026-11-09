//! A generic ARGUMENT is a type or a const by what its name RESOLVES to, never
//! by how the name is spelled (#93, #91).
//!
//! THE DEFECT, MEASURED ON d5166e8 BEFORE THE FIX
//!
//! ```text
//! enum K { A, B }
//! struct Wrap<T> { v: T }
//! fn main() { let w: Wrap<K> = Wrap { v: K::B }; ... }
//! -> error[PD0022]: Type mismatch: expected Wrap<const K>, found Wrap<K>
//!
//! struct A { n: i64 }
//! struct Box<T> { v: T }
//! fn main() { let b: Box<A> = Box { v: 5 }; ... }
//! -> error[PD0022]: Type mismatch: expected Box<const A>, found Box<Int>
//! ```
//!
//! The generic-argument loop of a written type (`parse_type`,
//! `src/parser/mod.rs`) ASSUMED that an identifier made only of capitals and
//! `_` was a const parameter. So `K`, `KIND`, `A` — and `T`, the name every
//! generic declaration uses — inside `Name<…>` became const arguments at every
//! written position: a `let`, a signature, a field, a payload, an `impl`
//! header, an alias's right-hand side. The same rule read a lowercase const
//! parameter (`const n: i64`, then `Arr<n>`) as a type.
//!
//! A name is now a const argument exactly when it is a const parameter of the
//! item being parsed — `fn`, `struct` and `enum` are the only lists that can
//! declare one, and each is parsed before the body it scopes — and a type
//! otherwise. A type argument prints as the name written (`Box<A>`); a const
//! parameter still prints as a const (`Arr<const N>`).
//!
//! WHAT THIS DOES NOT CLAIM. The #93 program now passes the type checker on
//! the path `Wrap<Kind>` takes on main, and like `Wrap<Kind>` it is then
//! refused by gcc: a written generic type's binding is emitted as `void*`.
//! That is N4-15 (M3), not this defect, so the test below asserts the front
//! end's verdict and the C it emits — the same C as the `Kind` spelling — and
//! does not link.

mod common;

use common::unique_module_name;
use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

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

/// Compile through C emission under the module name `name`; return the C.
fn emitted_c(source: &str, name: &str) -> String {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    let c_file = Driver::new()
        .compile_file(&src)
        .unwrap_or_else(|e| panic!("the front end refused a program it must accept: {}", e));
    fs::read_to_string(&c_file).expect("the emitted C")
}

/// Compile, link, run — and return stdout.
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

/// #91. The type argument is printed as the name written. On d5166e8 the
/// header was `expected Box<const A>, found Box<Int>`; the refusal itself is
/// right (an `Int` is not an `A`), so only its text is pinned.
#[test]
fn a_one_letter_type_argument_is_printed_as_the_name_written() {
    let text = refused(
        "struct A { n: i64 }\nstruct Box<T> { v: T }\nfn main() { let b: Box<A> = Box { v: 5 }; print_int(0); }\n",
        "gak_box_a",
    );
    assert!(
        text.contains("expected Box<A>, found Box<Int>"),
        "the argument `A` is a type and must print as one: {}",
        text
    );
    assert!(!text.contains("const A"), "`A` is no const: {}", text);
}

/// #93. The repro passes the front end, and the C it emits is the C the
/// `Kind` spelling emits on the same compiler, name for name: one path.
#[test]
fn a_one_letter_type_argument_takes_the_path_a_longer_name_takes() {
    let program = |kind: &str| {
        format!(
            "enum {k} {{ A, B }}\nstruct Wrap<T> {{ v: T }}\nfn main() {{ let w: Wrap<{k}> = Wrap {{ v: {k}::B }}; match w.v {{ {k}::A => {{ print_int(0); }} {k}::B => {{ print_int(1); }} }} }}\n",
            k = kind
        )
    };
    let short = unique_module_name("gak_wrap_short");
    let long = unique_module_name("gak_wrap_long");
    let c_short = emitted_c(&program("K"), &short);
    let c_long = emitted_c(&program("Kind"), &long);
    assert_eq!(
        c_short,
        c_long.replace("Kind", "K").replace(&long, &short),
        "`Wrap<K>` must be emitted exactly as `Wrap<Kind>` is"
    );
}

/// Every written position reads a capital name as a type: the header names
/// the type the program wrote. Each expected text is the one d5166e8 printed
/// for the same program spelled with a mixed-case name; on d5166e8 each of
/// these printed `const` before the name.
#[test]
fn every_written_position_reads_a_capital_name_as_a_type() {
    let cases: [(&str, &str); 6] = [
        (
            "enum K { A, B }\nstruct Wrap<T> { v: T }\nfn main() { let w: Wrap<K> = 5; print_int(0); }\n",
            "expected Wrap<K>, found Int",
        ),
        (
            "enum KIND { A, B }\nstruct Wrap<T> { v: T }\nfn f(w: Wrap<KIND>) -> i64 { return 1; }\nfn main() { print_int(f(7)); }\n",
            "expected Wrap<KIND>, found Int",
        ),
        (
            "enum K_A { A, B }\nstruct Wrap<T> { v: T }\nfn main() { let w: Wrap<Wrap<K_A>> = 5; print_int(0); }\n",
            "expected Wrap<Wrap<K_A>>, found Int",
        ),
        (
            "struct T { n: i64 }\nstruct Wrap<U> { v: U }\nfn main() { let w: [Wrap<T>; 1] = 5; print_int(0); }\n",
            "expected [Wrap<T>; 1], found Int",
        ),
        (
            "enum A { X }\nenum B { Y }\nstruct Pair<L, R> { l: L, r: R }\nfn main() { let p: Pair<A, B> = 5; print_int(0); }\n",
            "expected Pair<A, B>, found Int",
        ),
        (
            "enum K { A, B }\nstruct Wrap<T> { v: T }\ntype KW = Wrap<K>;\nfn main() { let w: KW = 5; print_int(0); }\n",
            "found Int",
        ),
    ];
    for (i, (source, expected)) in cases.iter().enumerate() {
        let text = refused(source, &format!("gak_position_{}", i));
        assert!(
            text.contains(expected),
            "case {}: expected `{}` in: {}",
            i,
            expected,
            text
        );
        assert!(!text.contains("const "), "case {}: no const here: {}", i, text);
    }
}

/// A generic alias's parameter written in capitals — `T`, on the right-hand
/// side — is the alias's TYPE parameter and is substituted. On d5166e8 the
/// argument `K` was a const, dropped from the alias's type arguments, and the
/// alias came out as its own parameter: `expected T, found Int`.
#[test]
fn a_generic_alias_substitutes_an_argument_written_in_capitals() {
    let text = refused(
        "enum K { A, B }\ntype Id<T> = T;\nfn main() { let k: Id<K> = 5; print_int(0); }\n",
        "gak_alias_id",
    );
    assert!(
        text.contains("expected K, found Int"),
        "`Id<K>` is `K`: {}",
        text
    );
}

/// A const parameter is still a const argument, in capitals or not, in a
/// function and in a struct's or an enum's own body; and it prints as one.
#[test]
fn a_const_parameter_is_a_const_argument_however_it_is_spelled() {
    let cases: [(&str, &str); 4] = [
        (
            "struct Arr<const N: i64> { x: i64 }\nfn f<const N: i64>() -> i64 { let a: Arr<N> = 5; return 0; }\nfn main() { print_int(f()); }\n",
            "expected Arr<const N>, found Int",
        ),
        // Lowercase: a type on d5166e8 (`expected Arr<n>, found Int`).
        (
            "struct Arr<const n: i64> { x: i64 }\nfn f<const n: i64>() -> i64 { let a: Arr<n> = 5; return 0; }\nfn main() { print_int(f()); }\n",
            "expected Arr<const n>, found Int",
        ),
        // An enum's own const parameter, in its payload (`Arr<n>` on d5166e8).
        (
            "struct Arr<const n: i64> { x: i64 }\nenum E<const n: i64> { X(Arr<n>), Y }\nfn main() { let e = E::X(Arr { x: 8 }); print_int(0); }\n",
            "expected Arr<const n>, found Arr",
        ),
        // A const parameter shadows a type of the same name.
        (
            "struct K { n: i64 }\nstruct Arr<const N: i64> { x: i64 }\nfn f<const K: i64>() -> i64 { let a: Arr<K> = 5; return 0; }\nfn main() { print_int(f()); }\n",
            "expected Arr<const K>, found Int",
        ),
    ];
    for (i, (source, expected)) in cases.iter().enumerate() {
        let text = refused(source, &format!("gak_const_{}", i));
        assert!(
            text.contains(expected),
            "case {}: expected `{}` in: {}",
            i,
            expected,
            text
        );
    }
}

/// A const parameter's scope ends with its item: after `fn f<const k: …>`, a
/// later `Wrap<k>` names the struct `k`.
#[test]
fn a_const_parameter_does_not_outlive_its_item() {
    let text = refused(
        "fn f<const k: i64>() -> i64 { return 0; }\nstruct k { x: i64 }\nstruct Wrap<T> { v: T }\nfn main() { let w: Wrap<k> = 5; print_int(f()); }\n",
        "gak_scope",
    );
    assert!(
        text.contains("expected Wrap<k>, found Int"),
        "`k` is the struct here: {}",
        text
    );
}

/// The programs that ran on d5166e8 where the reading of an argument flips
/// still run, with the same output: an unused signature at an enum (const ->
/// type), an enum's lowercase const parameter in its payload (type -> const),
/// an unannotated literal at an enum, and a function with a type and a const
/// parameter.
#[test]
fn the_programs_that_ran_where_the_reading_flips_still_run() {
    let out = run(
        r#"
enum K { A, B }
struct Wrap<T> { v: T }
struct Arr<const n: i64> { x: i64 }
enum E<const n: i64> { X(Arr<n>), Y }
fn unused(w: Wrap<K>) -> i64 { return 1; }
fn pick<T, const N: u64>(x: T) -> T { return x; }
fn main() {
    let w = Wrap { v: K::B };
    match w.v { K::A => { print_int(0); } K::B => { print_int(1); } }
    let e = E::Y;
    match e { E::X(a) => { print_int(0); } E::Y => { print_int(2); } }
    print_int(pick(3));
}
"#,
        "gak_still_runs",
    );
    assert_eq!(out, "1\n2\n3\n");
}
