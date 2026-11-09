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
