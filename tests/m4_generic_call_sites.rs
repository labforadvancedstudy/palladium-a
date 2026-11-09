//! Which monomorphization a generic use denotes is the one the type checker
//! recorded for THAT use (issues #86 and #87).
//!
//! THE DEFECT, MEASURED ON d5166e8 BEFORE THE FIX
//!
//! ```text
//! fn half<T>(x: T) -> T { return x / 2; }
//! fn main() { let b = half(true); let v: u32 = 3000000000; print_int(half(v)); }
//! -> -647483648            (without the `half(true)` line: 1500000000)
//! ```
//!
//! Code generation chose among every recorded instantiation of a generic
//! function by the C type of the call's FIRST argument, with substring tests
//! (`"unsigned int"` contains `"int"`, so the `bool` instantiation), accepted a
//! type argument in ANY position (`snd(1, 2)` called `snd__i64_String`), and
//! when nothing matched returned the first instantiation with no diagnostic.
//! A generic struct literal inside a generic function's body had no record at
//! all, because the checker never walked those bodies, so another literal's
//! instantiation decided what it built (#87) — and a call inside such a body
//! could name an instantiation nobody had recorded, which gcc met as an
//! undeclared function.
//!
//! The type checker now records the type arguments of each generic call by the
//! call's identity (`CallId`, as a struct literal's is its `LiteralId`), walks
//! each instantiated generic body once in its own type parameters so a use
//! there has a record too, and closes the set of instantiations over those
//! records. Code generation emits exactly the recorded instantiation, a body's
//! record resolved through the instantiation being emitted. A use no record
//! decides is emitted only where exactly one instantiation can take its
//! arguments, and refused by name where more than one can.
//!
//! Every accepted program LINKS AND RUNS against a printed answer. The
//! refusals assert that THIS compiler refused, never gcc.

mod common;

use common::unique_module_name;
use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// What `source` does: `RUN: <stdout>`, `REFUSED: <first line>`, or `GCC`.
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

/// `outcome` for a program of several files, compiled by `pdc` in a scratch
/// directory (the module resolver searches `.`).
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
        .expect("failed to run pdc");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success() {
        return if said.contains("gcc") {
            "GCC".to_string()
        } else {
            "REFUSED".to_string()
        };
    }
    let run = Command::new(dir.path().join("build_output").join("prog"))
        .current_dir(dir.path())
        .output()
        .expect("run");
    format!("RUN: {}", String::from_utf8_lossy(&run.stdout))
}

fn table(rows: &[(&str, String, &str)], prefix: &str) {
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program, expected) in rows {
        got.push(format!("{}: {}", shape, outcome(program, prefix)));
        want.push(format!("{}: {}", shape, expected));
    }
    assert_eq!(got, want);
}

const HALF: &str = "fn half<T>(x: T) -> T { return x / 2; }\n";
const ID: &str = "fn id<T>(x: T) -> T { return x; }\n";

/// #86, and every shape of it a call written in an ordinary function can take.
///
/// Each row printed something else on d5166e8: the `u32` beside a `bool`
/// instantiation was halved as an `int` (`-647483648`), and `snd`, and a type
/// parameter that only a LATER parameter mentions, were lowered to an
/// instantiation with the right type in the wrong position and refused by
/// gcc; so was the second of two struct instantiations (`id(q)` called
/// `id__P`). The controls (one instantiation, or none to confuse) ran on main
/// and still print the same.
#[test]
fn a_generic_call_is_emitted_as_the_instantiation_recorded_for_it() {
    let snd = "fn snd<A, B>(a: A, b: B) -> B { return b; }\n";
    let pick = "fn pick<T>(n: i64, x: T) -> T { return x; }\n";
    let rows: Vec<(&str, String, &str)> = vec![
        (
            "#86: a u32 beside a bool instantiation",
            format!("{HALF}fn main() {{ let b = half(true); let v: u32 = 3000000000; print_int(half(v)); }}\n"),
            "RUN: 1500000000\n",
        ),
        (
            "#86: the bool instantiation after the u32 one",
            format!("{HALF}fn main() {{ let v: u32 = 3000000000; print_int(half(v)); let b = half(true); }}\n"),
            "RUN: 1500000000\n",
        ),
        (
            "#86 through a macro: each expansion is its own call",
            format!("{HALF}macro h!(x) {{ half($x) }}\n\
                     fn main() {{ let t = true; let b = h!(t); let v: u32 = 3000000000; \
                     print_int(h!(v)); }}\n"),
            "RUN: 1500000000\n",
        ),
        (
            "#86 control: no bool instantiation",
            format!("{HALF}fn main() {{ let v: u32 = 3000000000; print_int(half(v)); }}\n"),
            "RUN: 1500000000\n",
        ),
        (
            "snd at (i64, String) and (i64, i64)",
            format!("{snd}fn main() {{ let s = snd(1, \"hello\"); print(s); print_int(snd(1, 2)); }}\n"),
            "RUN: hello\n2\n",
        ),
        (
            "the type parameter only in a later parameter",
            format!("{pick}fn main() {{ print(pick(1, \"a\")); print_int(pick(2, 7)); let b = pick(3, true); }}\n"),
            "RUN: a\n7\n",
        ),
        (
            "two struct instantiations",
            format!("struct P {{ x: i64 }}\nstruct Q {{ y: i64 }}\n{ID}\
                     fn main() {{ let p = P {{ x: 4 }}; let q = Q {{ y: 9 }}; let a = id(p); \
                     let b = id(q); print_int(a.x); print_int(b.y); }}\n"),
            "RUN: 4\n9\n",
        ),
        (
            "two calls of one instantiation (control)",
            format!("{ID}fn main() {{ print_int(id(3)); print_int(id(4)); }}\n"),
            "RUN: 3\n4\n",
        ),
    ];
    table(&rows, "site_record");
}

/// A call inside a generic body is the record the checker made for it in the
/// body's own type parameters, resolved through the instantiation emitted.
///
/// On d5166e8 a body was never walked: `wrap<T>` calling `id` emitted a call
/// to `id`, which nothing had instantiated (gcc: undeclared function); a `u32`
/// halved in a body beside a `half(true)` elsewhere printed `-647483648`. The
/// controls — a body whose callee's instantiations happen to exist, and
/// recursion — ran on main and still print the same.
#[test]
fn a_call_in_a_generic_body_is_its_record_through_the_instantiation() {
    let wrap = "fn wrap<T>(x: T) -> T { return id(x); }\n";
    let rows: Vec<(&str, String, &str)> = vec![
        (
            "a body reaches an instantiation nothing else records",
            format!("{ID}{wrap}fn main() {{ print_int(wrap(5)); print(wrap(\"s\")); }}\n"),
            "RUN: 5\ns\n",
        ),
        (
            "a u32 halved in a body beside a bool instantiation",
            format!("{HALF}fn wrap<T>(x: T) -> i64 {{ let v: u32 = 3000000000; return half(v); }}\n\
                     fn main() {{ let b = half(true); print_int(wrap(1)); }}\n"),
            "RUN: 1500000000\n",
        ),
        (
            "both instantiations of each exist (control)",
            format!("{ID}{wrap}fn main() {{ print_int(id(3)); print(id(\"a\")); print_int(wrap(5)); print(wrap(\"b\")); }}\n"),
            "RUN: 3\na\n5\nb\n",
        ),
        (
            "recursion at two instantiations (control)",
            "fn count<T>(x: T, n: i64) -> i64 { if n == 0 { return 0; } return 1 + count(x, n - 1); }\n\
             fn main() { print_int(count(1, 3)); print_int(count(\"a\", 2)); }\n"
                .to_string(),
            "RUN: 3\n2\n",
        ),
    ];
    table(&rows, "body_record");
}

/// #87: a generic struct literal inside a generic body is of ITS OWN record.
///
/// `G<i64, z>`, built at top level from a value typed by the alias's name,
/// decided on d5166e8 what the body's `G { tag: t, p: v }` built: `G_i64_z`,
/// so `~gg.p` printed `4294967288` where the same literal written in `main`
/// prints `-8` (the record is `G<i64, i64>`: the checker keeps no width, #82).
/// Without the decoy the body's instantiation was recorded by nobody and gcc
/// met `incomplete type 'struct G'`; and an `if` meeting two body literals put
/// `4294967297` into the decoy's `unsigned int` field and printed `1`.
#[test]
fn a_generic_struct_literal_in_a_generic_body_is_of_its_own_record() {
    let head = "type z = u32;\nstruct S { n: i64 }\nimpl S { fn get(&self) -> z { return 7; } }\n\
                struct G<X, Y> { tag: X, p: Y }\n";
    let build =
        "fn build<T>(t: T) -> i64 { let v: u32 = 7; let gg = G { tag: t, p: v }; return ~gg.p; }\n";
    let meet = "fn pick<T>(t: T) -> i64 { let v: u32 = 7; let w: i64 = 4294967297; let c = false; \
                let r = if c { G { tag: t, p: v } } else { G { tag: t, p: w } }; return r.p; }\n";
    let decoy = "let s = S { n: 0 }; let unused = G { tag: 1, p: s.get() }; ";
    let rows: Vec<(&str, String, &str)> = vec![
        (
            "#87: the body's literal beside a decoy G<i64, z>",
            format!("{head}{build}fn main() {{ {decoy}print_int(build(1)); }}\n"),
            "RUN: -8\n",
        ),
        (
            "the body's literal, no decoy",
            format!("{head}{build}fn main() {{ print_int(build(1)); }}\n"),
            "RUN: -8\n",
        ),
        (
            "an `if` meeting two body literals beside the decoy",
            format!("{head}{meet}fn main() {{ {decoy}print_int(pick(1)); }}\n"),
            "RUN: 4294967297\n",
        ),
        (
            "the same literal written in main (control)",
            format!("{head}fn main() {{ {decoy}let v: u32 = 7; let gg = G {{ tag: 1, p: v }}; print_int(~gg.p); }}\n"),
            "RUN: -8\n",
        ),
        (
            "a body literal of its own type parameter at two instantiations",
            "struct Box<T> { v: T }\nfn boxed<T>(x: T) -> T { let b = Box { v: x }; return b.v; }\n\
             fn main() { print_int(boxed(4)); print(boxed(\"w\")); }\n"
                .to_string(),
            "RUN: 4\nw\n",
        ),
    ];
    table(&rows, "body_literal");
}

/// An imported generic is selected by the same record: #86 across a module.
#[test]
fn an_imported_generic_call_is_its_own_instantiation() {
    let modules = [("halfmod.pd", "pub fn half<T>(x: T) -> T { return x / 2; }\n")];
    let got = outcome_of_files(
        &modules,
        "import halfmod;\n\nfn main() { let b = half(true); let v: u32 = 3000000000; \
         print_int(half(v)); }\n",
    );
    assert_eq!(got, "RUN: 1500000000\n");
}

/// A use no record decides is never given the first instantiation.
///
/// The checker infers a call's type arguments from literals and variables, so
/// `half(id(x))` in a generic body has no record (the argument is a call), and
/// neither does the `id(x)` inside it. `id` has one instantiation that takes
/// `x`'s `int`, and is emitted as that one; `half` has two that take an `int`
/// (`bool` exactly, `i64` as C widens it), and the call is refused by name. On
/// d5166e8 it was lowered to the first — `half__bool` — with no diagnostic.
#[test]
fn a_use_no_record_decides_is_refused_by_name_when_two_instantiations_take_it() {
    let program = format!(
        "{HALF}{ID}fn wrap<T>(x: T) -> T {{ return half(id(x)); }}\n\
         fn main() {{ let k = half(true); let j = half(4); let i = id(true); let w = wrap(true); \
         print_int(j); }}\n"
    );
    let got = outcome(&program, "unrecorded");
    assert!(
        got.starts_with("REFUSED: ") && got.contains("half"),
        "a call no record decides, with two instantiations that take its \
         arguments, must be refused by name; got {}",
        got
    );

    // One that only ONE instantiation takes is emitted as that one: here the
    // argument is a `long long`, which `half__bool`'s `int` does not take.
    let program = format!(
        "{HALF}{ID}fn wrap<T>(x: T) -> T {{ return half(id(x)); }}\n\
         fn main() {{ let k = half(true); let j = half(4); let i = id(1); let w = wrap(8); \
         print_int(w); }}\n"
    );
    assert_eq!(outcome(&program, "unrecorded_one"), "RUN: 4\n");
}

/// A recorded call whose ARRAY argument is not the array its parameter is in
/// the recorded instantiation is refused by name.
///
/// The type checker records every integer as `i64` (#82, not this unit), so
/// `first(xs)` with `xs: [u32; 2]` is `first<i64>`, whose parameter is a
/// `long long[2]`. On d5166e8 the call was lowered to `first__bool` (the
/// substring match met `"unsigned int"` first, beside a `first(bs)`) and
/// printed `-647483648` for `3000000000 / 2`; emitted as recorded it is C the
/// backend's own check refuses, so it is refused here, before any C exists.
#[test]
fn an_array_argument_its_recorded_instantiation_does_not_take_is_refused_by_name() {
    let program = "fn first<T>(a: [T; 2]) -> T { return a[0]; }\n\
                   fn main() { let bs = [true, false]; let b: bool = first(bs); \
                   let xs: [u32; 2] = [3000000000, 1]; let r: i64 = first(xs); \
                   print_int(r / 2); }\n";
    let got = outcome(program, "array_arg");
    assert!(
        got.starts_with("REFUSED: ") && got.contains("first<i64>") && got.contains("unsigned int[2]"),
        "an array argument of another element width must be refused by name; got {}",
        got
    );
    // The same call over an `[i64; 2]` runs.
    let program = "fn first<T>(a: [T; 2]) -> T { return a[0]; }\n\
                   fn main() { let bs = [true, false]; let b: bool = first(bs); \
                   let xs: [i64; 2] = [3000000000, 1]; print_int(first(xs) / 2); }\n";
    assert_eq!(outcome(program, "array_arg_i64"), "RUN: 1500000000\n");
}
