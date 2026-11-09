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
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
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

// ---------------------------------------------------------------------------
// Review round 1. A type ARGUMENT is now resolved as a type, so a type alias
// that names itself through one is expanded — and, on the tip of round 0,
// expanded FOREVER: `type Id<T> = T; type A = Id<A>; let a: A = 1;` overflowed
// the stack. Main refused that program only because it read the capital `A`
// as a const and stopped; `type Aa = Id<Aa>` overflowed main's stack too. An
// alias cycle is now refused by name (PD0080) wherever a use of it would be
// expanded, and nowhere else: an alias nothing uses still compiles, as on
// main. Second, a const parameter's name is resolved BEFORE a primitive
// spelling, so `Arr<i64>` under `const i64` is the const argument.
// ---------------------------------------------------------------------------

/// `pdc compile` in a CHILD process under a 20 s deadline, so a compiler that
/// overflows its stack or never finishes fails one assertion, not the binary.
/// Returns the verdict (`exit N`, `signal`, `no verdict in 20 s`) and the
/// compiler's output with its colour codes removed.
fn pdc_verdict(source: &str) -> (String, String) {
    pdc_verdict_files(&[("main.pd", source)])
}

/// `pdc_verdict` over several files, `main.pd` among them (an `import`).
fn pdc_verdict_files(files: &[(&str, &str)]) -> (String, String) {
    let dir = TempDir::new().unwrap();
    for (name, source) in files {
        fs::write(dir.path().join(name), source).unwrap();
    }
    let out = fs::File::create(dir.path().join("out.txt")).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .args(["compile", "main.pd", "-o", "prog"])
        .current_dir(dir.path())
        .stdout(Stdio::from(out.try_clone().unwrap()))
        .stderr(Stdio::from(out))
        .spawn()
        .expect("pdc");
    let deadline = Instant::now() + Duration::from_secs(20);
    let verdict = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break match status.code() {
                Some(n) => format!("exit {}", n),
                None => "signal".to_string(),
            };
        }
        if Instant::now() > deadline {
            child.kill().ok();
            child.wait().ok();
            break "no verdict in 20 s".to_string();
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let raw = fs::read_to_string(dir.path().join("out.txt")).unwrap_or_default();
    let mut text = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c == 'm' {
                    break;
                }
            }
        } else {
            text.push(c);
        }
    }
    (verdict, text)
}

/// Every alias cycle a program USES is refused by this compiler, by code, and
/// the message names the alias used and the cycle. On d5166e8 these died
/// three ways: a stack overflow (`Aa = Id<Aa>` and every capital case on the
/// round-0 tip), a hang (`type A = B; type B = A;`, #79), or a refusal that
/// named no cycle (`expected T, found Int`, where `A` was misread as a const).
#[test]
fn an_alias_cycle_a_program_uses_is_refused_by_name() {
    let id = "type Id<T> = T;\n";
    let unused_fn = "fn main() { print_int(0); }\n";
    let cases: Vec<(String, &str, &str)> = vec![
        // Through a generic alias's argument: capital, mixed case, a second
        // parameter, an indirect cycle in both spellings.
        (format!("{}type A = Id<A>;\nfn main() {{ let a: A = 1; print_int(0); }}\n", id), "A", "A -> A"),
        (format!("{}type Aa = Id<Aa>;\nfn main() {{ let a: Aa = 1; print_int(0); }}\n", id), "Aa", "Aa -> Aa"),
        ("type Pick<T, U> = T;\ntype A = Pick<A, i64>;\nfn main() { let a: A = 1; print_int(0); }\n".to_string(), "A", "A -> A"),
        (format!("{}type A = Id<B>;\ntype B = A;\nfn main() {{ let a: A = 1; print_int(0); }}\n", id), "A", "A -> B -> A"),
        (format!("{}type Aa = Id<Bb>;\ntype Bb = Aa;\nfn main() {{ let a: Aa = 1; print_int(0); }}\n", id), "Aa", "Aa -> Bb -> Aa"),
        // A generic alias that names itself, and a generic STRUCT's argument.
        ("type W<T> = W<T>;\nfn main() { let x: W<i64> = 1; print_int(0); }\n".to_string(), "W", "W -> W"),
        ("struct Wrap<T> { v: T }\ntype Aa = Wrap<Aa>;\nfn f(a: Aa) -> i64 { return 1; }\nfn main() { print_int(0); }\n".to_string(), "Aa", "Aa -> Aa"),
        // Every position a use can stand in: an uncalled signature (main RAN
        // the capital spelling, reading `A` as a const), a field, a payload.
        (format!("{}type A = Id<A>;\nfn f(a: A) -> i64 {{ return 1; }}\n{}", id, unused_fn), "A", "A -> A"),
        (format!("{}type Aa = Id<Aa>;\nstruct S {{ a: Aa }}\n{}", id, unused_fn), "Aa", "Aa -> Aa"),
        (format!("{}type Aa = Id<Aa>;\nenum E {{ X(Aa), Y }}\n{}", id, unused_fn), "Aa", "Aa -> Aa"),
        // A chain INTO a cycle names the alias used and the cycle it reaches.
        (format!("{}type A = Id<A>;\ntype C = A;\nfn main() {{ let c: C = 1; print_int(0); }}\n", id), "C", "A -> A"),
        // An array, a tuple and a reference are positions an expansion walks.
        ("type Aa = [Aa; 2];\nfn f(a: Aa) -> i64 { return 1; }\nfn main() { print_int(0); }\n".to_string(), "Aa", "Aa -> Aa"),
        ("type Aa = (Aa, i64);\nfn f(a: Aa) -> i64 { return 1; }\nfn main() { print_int(0); }\n".to_string(), "Aa", "Aa -> Aa"),
        ("type Aa = &Aa;\nfn f(a: Aa) -> i64 { return 1; }\nfn main() { print_int(0); }\n".to_string(), "Aa", "Aa -> Aa"),
        // #79, the cycle with no generic in it: it hung type checking.
        ("type A = B;\ntype B = A;\nfn main() { let x: A = 1; print_int(x); }\n".to_string(), "A", "A -> B -> A"),
        ("type A = A;\nfn main() { let x: A = 1; print_int(x); }\n".to_string(), "A", "A -> A"),
        ("type A = B;\ntype B = C;\ntype C = A;\nfn main() { let x: A = 1; print_int(x); }\n".to_string(), "A", "A -> B -> C -> A"),
        // A cycle that branches, used in a generic function: the pass that
        // runs before type checking expanded it 2^64 times.
        ("type Pp = (Pp, Pp);\nfn f<T>(x: Pp) -> i64 { return 1; }\nfn main() { print_int(0); }\n".to_string(), "Pp", "Pp -> Pp"),
    ];
    let mut wrong = Vec::new();
    for (i, (source, used, cycle)) in cases.iter().enumerate() {
        let (verdict, text) = pdc_verdict(source);
        let ok = verdict == "exit 1"
            && text.contains("error[PD0080]")
            && text.contains(&format!("`{}`", used))
            && text.contains(cycle);
        if !ok {
            wrong.push(format!("case {} ({} / {}): {}: {}", i, used, cycle, verdict, text.lines().filter(|l| l.contains("error")).collect::<Vec<_>>().join(" | ")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// An alias cycle NOTHING uses is not refused: each of these compiled and ran
/// on d5166e8 (`(Pp, Pp)` aside, which hung it), and each still runs. A cycle
/// through a parameter the generic alias never reads is no cycle at all — the
/// checker never expands that argument — and it ran on main from a signature.
#[test]
fn an_alias_cycle_nothing_expands_still_compiles_and_runs() {
    let main7 = "fn main() { print_int(7); }\n";
    let cases = [
        format!("type Id<T> = T;\ntype A = Id<A>;\n{}", main7),
        format!("type Id<T> = T;\ntype Aa = Id<Aa>;\n{}", main7),
        format!("type Id<T> = T;\ntype A = Id<B>;\ntype B = A;\n{}", main7),
        format!("type A = B;\ntype B = A;\n{}", main7),
        format!("type A = A;\n{}", main7),
        format!("type W<T> = W<T>;\n{}", main7),
        format!("struct Wrap<T> {{ v: T }}\ntype Aa = Wrap<Aa>;\n{}", main7),
        format!("type Pp = (Pp, Pp);\n{}", main7),
        format!("type Id<A> = A;\ntype A = Id<i64>;\n{}", main7),
        "type Pick<T, U> = T;\ntype Aa = Pick<i64, Aa>;\nfn f(a: Aa) -> i64 { return 1; }\nfn main() { print_int(7); }\n".to_string(),
    ];
    let mut wrong = Vec::new();
    for (i, source) in cases.iter().enumerate() {
        let (verdict, text) = pdc_verdict(source);
        if verdict != "exit 0" {
            wrong.push(format!("case {}: {}: {}", i, verdict, text.lines().filter(|l| l.contains("error")).collect::<Vec<_>>().join(" | ")));
            continue;
        }
        let out = run(source, &format!("gak_unused_cycle_{}", i));
        if out != "7\n" {
            wrong.push(format!("case {}: printed {:?}", i, out));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A const parameter's name is resolved before a primitive spelling: under
/// `const i64`, `Arr<i64>` is the const argument (on the round-0 tip and on
/// d5166e8, `Arr<Int>`: the primitive table answered first). A bare `i64`
/// outside `<…>` is still the type, the only thing that position can hold.
#[test]
fn a_const_parameter_named_like_a_primitive_is_a_const_argument() {
    let mut wrong = Vec::new();
    for name in ["i32", "i64", "int", "u32", "u64", "f32", "f64", "bool", "char", "String"] {
        let source = format!(
            "struct Arr<const N: i64> {{ x: i64 }}\nfn f<const {n}: i64>() -> i64 {{ let a: Arr<{n}> = 5; return 0; }}\nfn main() {{ print_int(f()); }}\n",
            n = name
        );
        let text = refused(&source, &format!("gak_prim_{}", name));
        if !text.contains(&format!("expected Arr<const {}>, found Int", name)) {
            wrong.push(format!("{}: {}", name, text));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // The same names, declared and never written as an argument, still run.
    let out = run(
        "fn f<const i64: i64>() -> i64 { let x: i64 = 5; return x; }\nstruct S<const bool: i64> { x: i64 }\nenum E<const String: i64> { A }\nfn main() { let s = S { x: 1 }; print_int(f() + s.x); }\n",
        "gak_prim_declared",
    );
    assert_eq!(out, "6\n");
}

/// The imported code the checker type-checks is a use too: a cyclic alias in
/// an imported `pub fn`'s signature overflowed main's stack (and the round-0
/// tip's) with nothing in the importing program naming it. A private imported
/// function is never checked, and the program still runs, as on main.
#[test]
fn an_alias_cycle_in_checked_imported_code_is_refused_and_in_unchecked_code_is_not() {
    let lib = |f: &str| format!("pub type Id<T> = T;\npub type Aa = Id<Aa>;\n{}\npub fn one() -> i64 {{ return 1; }}\n", f);
    let cases = [
        (lib(""), "import lib;\nfn main() { let a: Aa = 1; print_int(0); }\n", "exit 1"),
        (lib("pub fn take(a: Aa) -> i64 { return 1; }"), "import lib;\nfn main() { print_int(0); }\n", "exit 1"),
        (lib("fn hidden(a: Aa) -> i64 { return 1; }"), "import lib;\nfn main() { print_int(one()); }\n", "exit 0"),
    ];
    let mut wrong = Vec::new();
    for (i, (lib_source, main_source, want)) in cases.iter().enumerate() {
        let (verdict, text) = pdc_verdict_files(&[("lib.pd", lib_source), ("main.pd", main_source)]);
        let refused_by_name = text.contains("error[PD0080]") && text.contains("`Aa -> Aa`");
        if verdict != *want || (*want == "exit 1") != refused_by_name {
            wrong.push(format!("case {}: {} (wanted {}): {}", i, verdict, want, text.lines().filter(|l| l.contains("error")).collect::<Vec<_>>().join(" | ")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

// ---------------------------------------------------------------------------
// Review round 2. The round-1 refusal was a PRE-CHECK over the program's
// uses, and two paths ran past it into the checker's own expansion: an
// instantiated generic's signature (imported or local), and a generic alias
// whose parameter a filtered const argument left unsubstituted, which the
// checker then resolves globally. Both overflowed the stack. The checker's
// expansion is now itself guarded, so a path the pre-check does not see is
// refused where it is met. And the pre-check had refused positions the
// checker never expands — a trait's methods, a generic item nothing
// instantiates — every one of which ran on main; those run again.
// ---------------------------------------------------------------------------

/// Every path to a cyclic alias the checker expands is refused by name. On
/// 6fa392e each of these overflowed the stack (the growth, the two-argument
/// struct and the imported chain had been refused there already, and stay).
#[test]
fn every_expansion_of_a_cyclic_alias_the_checker_performs_is_refused_by_name() {
    let id = "pub type Id<T> = T;\n";
    let cases: Vec<(Vec<(&str, String)>, &str, &str)> = vec![
        // An instantiated generic's result, imported, in both spellings.
        (vec![("lib.pd", format!("{}pub type A = Id<A>;\npub fn f<T>(x: T) -> A {{ return x; }}\n", id)), ("main.pd", "import lib;\nfn main() { print_int(f(1)); }\n".to_string())], "A", "A -> A"),
        (vec![("lib.pd", format!("{}pub type Aa = Id<Aa>;\npub fn f<T>(x: T) -> Aa {{ return x; }}\n", id)), ("main.pd", "import lib;\nfn main() { print_int(f(1)); }\n".to_string())], "Aa", "Aa -> Aa"),
        // ... and local.
        (vec![("main.pd", "type Id<T> = T;\ntype Aa = Id<Aa>;\nfn f<T>(x: T) -> Aa { return x; }\nfn main() { print_int(f(1)); }\n".to_string())], "Aa", "Aa -> Aa"),
        // A const argument leaves `T` unsubstituted; `T` is then the global
        // alias. Used, in an uncalled signature, and under a const parameter.
        (vec![("main.pd", "type T = G<1>;\ntype G<T> = T;\nfn main() { let x: T = 1; print_int(7); }\n".to_string())], "T", "T -> G -> T"),
        (vec![("main.pd", "type T = G<1>;\ntype G<T> = T;\nfn f(x: T) -> i64 { return 1; }\nfn main() { print_int(7); }\n".to_string())], "T", "T -> G -> T"),
        (vec![("main.pd", "type T = Id<T>;\ntype Id<T> = T;\nfn f<const n: i64>() { let x: Id<n> = 1; print_int(0); }\nfn main() { f(); }\n".to_string())], "Id", "T -> T"),
        (vec![("main.pd", "type T = Id<T>;\ntype Id<T> = T;\nfn f<const n: i64>(x: Id<n>) -> i64 { return 1; }\nfn main() { print_int(7); }\n".to_string())], "Id", "T -> T"),
        // An alias whose argument grows, a two-argument struct, an imported chain.
        (vec![("main.pd", "type G<T> = G<(T, T)>;\nfn f(x: G<i64>) -> i64 { return 7; }\nfn main() { print_int(7); }\n".to_string())], "G", "G -> G"),
        (vec![("main.pd", "struct P2<X, Y> { x: X, y: Y }\ntype Aa = P2<Aa, Aa>;\nfn f(a: Aa) -> i64 { return 1; }\nfn main() { print_int(7); }\n".to_string())], "Aa", "Aa -> Aa"),
        (vec![("lib.pd", format!("{}pub type Aa = Id<Aa>;\npub type C = Aa;\npub fn one() -> i64 {{ return 1; }}\n", id)), ("main.pd", "import lib;\nfn main() { let c: C = 1; print_int(0); }\n".to_string())], "C", "Aa -> Aa"),
    ];
    let mut wrong = Vec::new();
    for (i, (files, used, cycle)) in cases.iter().enumerate() {
        let files: Vec<(&str, &str)> = files.iter().map(|(n, s)| (*n, s.as_str())).collect();
        let (verdict, text) = pdc_verdict_files(&files);
        let ok = verdict == "exit 1"
            && text.contains("error[PD0080]")
            && text.contains(&format!("`{}`", used))
            && text.contains(cycle);
        if !ok {
            wrong.push(format!("case {} ({} / {}): {}: {}", i, used, cycle, verdict, text.lines().filter(|l| l.contains("error") || l.contains("fatal")).collect::<Vec<_>>().join(" | ")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A cyclic alias in a position the checker never expands is not refused:
/// a trait's method (signature, result, default body), a generic function
/// nothing instantiates (signature and body, local and imported), a generic
/// struct's field and enum's payload, an imported generic shadowed by a local
/// one. Every one ran on main and was refused on 6fa392e; each runs again.
#[test]
fn a_cyclic_alias_the_checker_never_expands_still_compiles_and_runs() {
    let cm = "type Id<T> = T;\ntype Aa = Id<Aa>;\n";
    let main7 = "fn main() { print_int(7); }\n";
    let lib = "pub type Id<T> = T;\npub type A = Id<A>;\npub fn f<T>(x: T) -> A { return x; }\n";
    let cases: Vec<Vec<(&str, String)>> = vec![
        vec![("main.pd", format!("type Id<T> = T;\ntype A = Id<A>;\ntrait Tr {{ fn m(x: A) -> i64; }}\n{}", main7))],
        vec![("main.pd", format!("{}trait Tr {{ fn m(x: Aa) -> i64; }}\n{}", cm, main7))],
        vec![("main.pd", format!("{}trait Tr {{ fn m(x: i64) -> Aa; }}\n{}", cm, main7))],
        vec![("main.pd", format!("{}trait Tr {{ fn m(x: i64) -> i64 {{ let a: Aa = 1; return 1; }} }}\n{}", cm, main7))],
        vec![("main.pd", format!("{}fn f<T>(x: T) -> Aa {{ return x; }}\n{}", cm, main7))],
        vec![("main.pd", format!("{}fn g<T>(x: T) -> i64 {{ let a: Aa = 1; return 1; }}\n{}", cm, main7))],
        vec![("main.pd", format!("{}struct S<T> {{ a: Aa, v: T }}\n{}", cm, main7))],
        vec![("main.pd", format!("{}enum E<T> {{ X(Aa), Y(T) }}\n{}", cm, main7))],
        vec![("lib.pd", lib.to_string()), ("main.pd", format!("import lib;\n{}", main7))],
        vec![("lib.pd", lib.to_string()), ("main.pd", "import lib;\nfn f<T>(x: T) -> i64 { return 7; }\nfn main() { print_int(f(1)); }\n".to_string())],
        vec![("lib.pd", "pub type Id<T> = T;\npub type Aa = Id<Aa>;\npub fn g<T>(x: T) -> i64 { let a: Aa = 1; return 1; }\n".to_string()), ("main.pd", format!("import lib;\n{}", main7))],
    ];
    let mut wrong = Vec::new();
    for (i, files) in cases.iter().enumerate() {
        let files: Vec<(&str, &str)> = files.iter().map(|(n, s)| (*n, s.as_str())).collect();
        let (verdict, text) = pdc_verdict_files(&files);
        if verdict != "exit 0" {
            wrong.push(format!("case {}: {}: {}", i, verdict, text.lines().filter(|l| l.contains("error") || l.contains("fatal")).collect::<Vec<_>>().join(" | ")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

// ---------------------------------------------------------------------------
// Review round 3. The round-2 guard took a repeated generic alias NAME on a
// deep open path for a cycle, so a FINITE expansion that reopens `Id` with a
// smaller argument each time was refused: `Id<Id<a000>>` over a chain of 256
// distinct aliases (main prints hello) and `Id` nested 270 deep both reported
// `Id -> Id`. A cycle is now judged by the alias graph alone — an alias whose
// route through bodies reaches a cycle — and a name repeat counts only for an
// alias with no type parameter, whose name is its whole state. There is no
// depth limit: main's front end takes these at 4000 deep, and so does this one.
// ---------------------------------------------------------------------------

/// `pdc compile` then the program, both in child processes (a deep expansion
/// recurses deeper than a test thread's stack allows in-process). Returns the
/// compile verdict and the program's stdout ("" unless it compiled).
fn pdc_compile_and_run(source: &str) -> (String, String) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("main.pd"), source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .args(["compile", "main.pd", "-o"])
        .arg(dir.path().join("prog"))
        .current_dir(dir.path())
        .output()
        .expect("pdc");
    let verdict = match out.status.code() {
        Some(n) => format!("exit {}", n),
        None => "signal".to_string(),
    };
    if !out.status.success() {
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let first = text.lines().find(|l| l.contains("error") || l.contains("fatal")).unwrap_or("").to_string();
        return (format!("{}: {}", verdict, first), String::new());
    }
    let run = Command::new(dir.path().join("prog")).output().expect("run");
    (verdict, String::from_utf8_lossy(&run.stdout).to_string())
}

fn nested_id(depth: usize, leaf: &str) -> String {
    (0..depth).fold(leaf.to_string(), |t, _| format!("Id<{}>", t))
}

/// Every one of these is FINITE, ran on main (d5166e8) with this output, and
/// was refused as an `Id -> Id` cycle on 3a44639 (the 2000-deep stresses aside,
/// which round 2 refused too).
#[test]
fn a_finite_expansion_that_reopens_a_generic_alias_is_no_cycle() {
    let chain_to = |n: usize, prefix: &str, end: &str, through_id: bool| -> String {
        let mut s = String::from("type Id<T> = T;\n");
        for i in 0..n {
            let next = format!("{}{:04}", prefix, i + 1);
            let body = if through_id { format!("Id<{}>", next) } else { next };
            s.push_str(&format!("type {}{:04} = {};\n", prefix, i, body));
        }
        s.push_str(&format!("type {}{:04} = {};\n", prefix, n, end));
        s
    };
    let sig = |ty: &str| format!("fn f(x: {}) -> i64 {{ return 1; }}\nfn main() {{ print_int(7); }}\n", ty);
    let cases: Vec<(&str, String, &str)> = vec![
        // astra (a): 256 distinct aliases ending in String, used through `Id<Id<…>>`.
        ("astra a", format!("{}fn main() {{ let x: Id<Id<a0000> > = \"hello\"; print(x); }}\n", chain_to(256, "a", "String", false)), "hello\n"),
        ("astra a, 1000", format!("{}fn main() {{ let x: Id<Id<a0000> > = \"hello\"; print(x); }}\n", chain_to(1000, "a", "String", false)), "hello\n"),
        // astra (b) and fable d10: `Id` nested 270 and 300 deep in a signature.
        ("astra b", format!("type Id<T> = T;\n{}", sig(&nested_id(270, "i64"))), "7\n"),
        ("fable d10", format!("type Id<T> = T;\n{}", sig(&nested_id(300, "i64"))), "7\n"),
        // fable d07: a chain of 150 distinct aliases, each through `Id`.
        ("fable d07", format!("{}{}", chain_to(150, "B", "i64", true), sig("B0000")), "7\n"),
        // Stresses: 2000 deep, nested and chained; a plain chain of 2000 used.
        ("nested 2000", format!("type Id<T> = T;\n{}", sig(&nested_id(2000, "i64"))), "7\n"),
        ("chain through Id 2000", format!("{}{}", chain_to(2000, "B", "i64", true), sig("B0000")), "7\n"),
        ("plain chain 2000", format!("{}fn main() {{ let x: A0000 = 5; print_int(x); }}\n", chain_to(2000, "A", "i64", false)), "5\n"),
    ];
    let mut wrong = Vec::new();
    for (name, source, want) in &cases {
        let (verdict, out) = pdc_compile_and_run(source);
        if verdict != "exit 0" || out != *want {
            wrong.push(format!("{}: {} / printed {:?}", name, verdict, out));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The genuine cycles through generic aliases are still refused by name: two
/// generic aliases that name each other, one whose argument grows, and a pair
/// through a generic STRUCT's argument (fable m04 — main printed 0 from an
/// uncalled signature; it is a real cycle, and a lost run this unit owns).
#[test]
fn a_genuine_cycle_through_generic_aliases_is_still_refused_by_name() {
    let cases = [
        ("type F<T> = G<T>;\ntype G<T> = F<T>;\nfn f(x: F<i64>) -> i64 { return 1; }\nfn main() { print_int(0); }\n", "F", "F -> G -> F"),
        ("type G<T> = G<(T, T)>;\nfn f(x: G<i64>) -> i64 { return 1; }\nfn main() { print_int(0); }\n", "G", "G -> G"),
        ("struct W<T> { v: T }\ntype A = W<B>;\ntype B = W<A>;\nfn f(a: A) -> i64 { return 1; }\nfn main() { print_int(0); }\n", "A", "A -> B -> A"),
        // ... and through an instantiated generic, which only the guard sees.
        ("type F<T> = G<T>;\ntype G<T> = F<T>;\nfn g<U>(x: U) -> F<U> { return x; }\nfn main() { print_int(g(1)); }\n", "F", "F -> G -> F"),
    ];
    let mut wrong = Vec::new();
    for (i, (source, used, cycle)) in cases.iter().enumerate() {
        let (verdict, text) = pdc_verdict(source);
        if !(verdict == "exit 1" && text.contains("error[PD0080]") && text.contains(&format!("`{}`", used)) && text.contains(cycle)) {
            wrong.push(format!("case {}: {}: {}", i, verdict, text.lines().filter(|l| l.contains("error") || l.contains("fatal")).collect::<Vec<_>>().join(" | ")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

// ---------------------------------------------------------------------------
// Review round 4. The round-3 pre-check MODELLED which positions the checker
// expands and how an argument binds a parameter, and the model drifted twice:
// a struct or enum with a lifetime parameter is skipped by the checker and was
// judged by the pre-check (`type A = A; struct S<'a> { v: A }` — main prints
// 7, 20ed375 refused it), and `type Pick<T, T>` binds its LAST `T` in the
// checker and the FIRST in the model. The pre-check is gone: an alias is
// judged only where the checker or code generation opens it, by a graph built
// from the checker's own tables and its own substitution. And a parameter
// list naming one parameter twice is refused where it is written (PD0081).
// ---------------------------------------------------------------------------

/// PD0081 at every kind of generic parameter list, lifetimes, type and const
/// parameters alike, in an imported module too — and astra's three programs
/// whose duplicate binder decided whether a cycle was reached (two of them
/// overflowed main's stack and 20ed375's) are refused by the binder, before
/// any alias is expanded. Each ran or crashed on main.
#[test]
fn a_generic_parameter_list_naming_a_parameter_twice_is_refused_by_name() {
    let main7 = "fn main() { print_int(7); }\n";
    let cases: Vec<(Vec<(&str, String)>, &str)> = vec![
        (vec![("main.pd", format!("fn f<T, T>(x: T) -> i64 {{ return 1; }}\n{}", main7))], "`T`"),
        (vec![("main.pd", format!("fn f<T, const T: i64>(x: T) -> i64 {{ return 2; }}\n{}", main7))], "`T`"),
        (vec![("main.pd", format!("struct S<T, T> {{ v: T }}\n{}", main7))], "`T`"),
        (vec![("main.pd", format!("enum E<T, T> {{ A(T), B }}\n{}", main7))], "`T`"),
        (vec![("main.pd", format!("type Pick<T, T> = T;\n{}", main7))], "`T`"),
        (vec![("main.pd", format!("struct W<T> {{ v: T }}\nimpl<T, T> W<T> {{ fn m(x: i64) -> i64 {{ return 1; }} }}\n{}", main7))], "`T`"),
        (vec![("main.pd", format!("trait Tr<T, T> {{ fn m(x: i64) -> i64; }}\n{}", main7))], "`T`"),
        (vec![("main.pd", format!("trait Tr {{ fn m<T, T>(x: T) -> i64; }}\n{}", main7))], "`T`"),
        (vec![("main.pd", format!("fn f<'a, 'a>(x: i64) -> i64 {{ return 1; }}\n{}", main7))], "`'a`"),
        (vec![("main.pd", format!("fn f<const N: i64, const N: i64>() -> i64 {{ return 1; }}\n{}", main7))], "`N`"),
        (vec![("lib.pd", "pub fn g<T, T>(x: T) -> i64 { return 1; }\npub fn one() -> i64 { return 1; }\n".to_string()), ("main.pd", format!("import lib;\n{}", main7))], "`T`"),
        // astra's three: finite on main, a cycle on main, a cycle through an instantiation.
        (vec![("main.pd", "type Pick<T,T> = T; type A = Pick<A,String>; fn main(){let x:A = \"hello\"; print(x);}\n".to_string())], "`T`"),
        (vec![("main.pd", "type Pick<T,T> = T; type G<X> = Pick<String,G<X> >; fn f(x:G<i64>)->i64{return 1;} fn main(){print_int(7);}\n".to_string())], "`T`"),
        (vec![("main.pd", "type Pick<T,T> = T; type G<X> = Pick<String,G<X> >; fn f<U>(x:U)->G<U>{return x;} fn main(){f(1);}\n".to_string())], "`T`"),
    ];
    let mut wrong = Vec::new();
    for (i, (files, name)) in cases.iter().enumerate() {
        let files: Vec<(&str, &str)> = files.iter().map(|(n, s)| (*n, s.as_str())).collect();
        let (verdict, text) = pdc_verdict_files(&files);
        if !(verdict == "exit 1" && text.contains("error[PD0081]") && text.contains(name) && !text.contains("PD0080")) {
            wrong.push(format!("case {}: {}: {}", i, verdict, text.lines().filter(|l| l.contains("error") || l.contains("fatal")).collect::<Vec<_>>().join(" | ")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // Distinct names are not refused: a lifetime and a type of one letter, and
    // a method's own parameter of its impl's name (another list).
    for (source, want) in [
        ("fn f<'a, a>(x: a) -> i64 { return 4; }\nfn main() { print_int(f(1)); }\n", "4\n"),
        ("struct W<T> { v: T }\nimpl<T> W<T> { fn m<T>(x: T) -> i64 { return 1; } }\nfn main() { print_int(7); }\n", "7\n"),
        ("fn f<'a, T, const N: i64>(x: T) -> i64 { return 3; }\nfn main() { print_int(f(1)); }\n", "3\n"),
    ] {
        let (verdict, out) = pdc_compile_and_run(source);
        assert_eq!((verdict.as_str(), out.as_str()), ("exit 0", want), "{}", source);
    }
}

/// The checker skips a struct or an enum with a lifetime parameter (it is
/// registered as generic and converted only when instantiated), so a cyclic
/// alias in its field or payload is never expanded and the program runs, as on
/// main. And a cycle the checker's expansion never follows — through a tuple
/// alias's element, behind a reference — is not judged (fable r05, r08). Each
/// printed 7 on main and was refused on 20ed375.
#[test]
fn an_alias_the_checker_never_opens_is_never_judged() {
    let cases = [
        "type A=A; struct S<'a> { v:A } fn main(){print_int(7);}\n",
        "type A=A; enum S<'a> { V(A), Z } fn main(){print_int(7);}\n",
        "type Pair<T> = (T, i64);\ntype Q<T> = Pair<T>;\ntype Aa = Q<Aa>;\nfn f(a: Aa) -> i64 { return 1; }\nfn main() { print_int(7); }\n",
        "type Id<T> = T;\ntype Aa = &Id<Aa>;\nfn f(a: Aa) -> i64 { return 1; }\nfn main() { print_int(7); }\n",
    ];
    for source in cases {
        let (verdict, out) = pdc_compile_and_run(source);
        assert_eq!((verdict.as_str(), out.as_str()), ("exit 0", "7\n"), "{}", source);
    }
}

/// Where the checker DOES open the alias it is judged, whatever the item: a
/// struct with only a const parameter (non-generic to the checker), a function
/// with only a lifetime parameter, an impl's generic method, a generic impl's
/// method. Main hung or overflowed its stack on each.
#[test]
fn an_alias_the_checker_opens_is_judged_in_every_kind_of_item() {
    let cases = [
        "type A = A;\nstruct S<const N: i64> { v: A }\nfn main() { print_int(7); }\n",
        "type A = A;\nfn f<'a>(x: A) -> i64 { return 1; }\nfn main() { print_int(7); }\n",
        "type A = A;\nstruct S { n: i64 }\nimpl S { fn m<T>(x: A) -> i64 { return 1; } }\nfn main() { print_int(7); }\n",
        "type A = A;\nstruct W<T> { v: T }\nimpl<T> W<T> { fn m(x: A) -> i64 { return 1; } }\nfn main() { print_int(7); }\n",
    ];
    let mut wrong = Vec::new();
    for (i, source) in cases.iter().enumerate() {
        let (verdict, text) = pdc_verdict(source);
        if !(verdict == "exit 1" && text.contains("error[PD0080]") && text.contains("`A -> A`")) {
            wrong.push(format!("case {}: {}: {}", i, verdict, text.lines().filter(|l| l.contains("error") || l.contains("fatal")).collect::<Vec<_>>().join(" | ")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
