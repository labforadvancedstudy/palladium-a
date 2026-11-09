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
//! records. Code generation emits a use as its record, resolved through the
//! instantiation being emitted, WHERE THE RECORD HOLDS ITS VALUES EXACTLY:
//! each argument's or field value's C type the C type of its parameter or field
//! there (review round 2). The record keeps no integer width (#82), and a
//! parameter's or field's type decides what `~`, `>>`, `/` and a comparison do
//! with the value. Where it does not, a call is of the ONE instantiation of the
//! record's kind whose parameters are the arguments' exact C types; anything
//! else — a literal its record does not hold exactly, a value whose C type is
//! not known exactly, a use no record decides and not exactly one
//! instantiation takes — is refused by name (PD0082). Never the first entry,
//! and never a value converted into a parameter or field of another type.
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

/// The refusal `source` meets, asserted to be THIS compiler's PD0082 and to
/// carry `fragment` — never gcc, never another rule.
fn refused_pd0080(source: &str, prefix: &str, fragment: &str, shape: &str) {
    let name = unique_module_name(prefix);
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    let err = match Driver::new().compile_file(&src) {
        Ok(_) => panic!("{}: compiled; it must be refused by name (PD0082)", shape),
        Err(e) => e,
    };
    let text = err.to_string();
    assert_eq!(
        err.code().map(|c| c.to_string()).as_deref(),
        Some("PD0082"),
        "{}: refused, but not by PD0082: {}",
        shape,
        text
    );
    assert!(
        text.contains(fragment) && !text.contains("gcc"),
        "{}: the refusal must name `{}`: {}",
        shape,
        fragment,
        text
    );
}

const HALF: &str = "fn half<T>(x: T) -> T { return x / 2; }\n";
const ID: &str = "fn id<T>(x: T) -> T { return x; }\n";

/// `type z = <width>` and a method returning a value typed by its NAME, which
/// the type checker records at `z` — an instantiation whose C type is the
/// width's own.
fn by_name(width: &str, value: &str) -> String {
    format!(
        "type z = {width};\nstruct S {{ n: i64 }}\n\
         impl S {{ fn get(&self) -> z {{ return {value}; }} }}\n"
    )
}

/// #86, and the shapes of it a call written in an ordinary function can take.
///
/// On d5166e8: a `u64` typed by an alias's name beside a `half(true)` fell
/// through to the FIRST instantiation, `half__bool`, and was halved as an
/// `int`; `snd(1, 2)` beside `snd(1, "hello")`, and a type parameter only a
/// later parameter mentions, were lowered to an instantiation with the right
/// type in the wrong position (gcc). The controls ran on main and still print
/// the same. The #86 repro itself — `half(v)`, `v: u32`, recorded `half<i64>`
/// — is refused by name: its record does not hold a `u32`, and no `u32`
/// instantiation exists (main printed -647483648 beside `half(true)`, and
/// 1500000000 alone, a run this rule gives up because the record's `long long`
/// would read `~`, `>>` and comparisons of the `u32` as an i64).
#[test]
fn a_generic_call_is_emitted_as_the_instantiation_recorded_for_it() {
    let snd = "fn snd<A, B>(a: A, b: B) -> B { return b; }\n";
    let pick = "fn pick<T>(n: i64, x: T) -> T { return x; }\n";
    let u64_head = by_name("u64", "9223372036854775807");
    let rows: Vec<(&str, String, &str)> = vec![
        (
            "a u64 by its alias's name beside a bool instantiation (main: half__bool)",
            format!("{u64_head}{HALF}fn main() {{ let b = half(true); let s = S {{ n: 0 }}; \
                     let zval = s.get(); print_int(half(zval)); }}\n"),
            "RUN: 4611686018427387903\n",
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
            "an i64 through a macro beside a bool instantiation",
            format!("{HALF}macro h!(x) {{ half($x) }}\n\
                     fn main() {{ let t = true; let b = h!(t); let v: i64 = 3000000000; \
                     print_int(h!(v)); }}\n"),
            "RUN: 1500000000\n",
        ),
        (
            "two calls of one instantiation (control)",
            format!("{ID}fn main() {{ print_int(id(3)); print_int(id(4)); }}\n"),
            "RUN: 3\n4\n",
        ),
    ];
    table(&rows, "site_record");

    for (shape, program) in [
        (
            "#86: a u32 beside a bool instantiation (main: -647483648)",
            format!("{HALF}fn main() {{ let b = half(true); let v: u32 = 3000000000; print_int(half(v)); }}\n"),
        ),
        (
            "#86: the bool instantiation after the u32 one (main: -647483648)",
            format!("{HALF}fn main() {{ let v: u32 = 3000000000; print_int(half(v)); let b = half(true); }}\n"),
        ),
        (
            "#86 without the bool instantiation (main: 1500000000)",
            format!("{HALF}fn main() {{ let v: u32 = 3000000000; print_int(half(v)); }}\n"),
        ),
        (
            "#86 through a macro (main: -647483648)",
            format!("{HALF}macro h!(x) {{ half($x) }}\nfn main() {{ let t = true; let b = h!(t); \
                     let v: u32 = 3000000000; print_int(h!(v)); }}\n"),
        ),
    ] {
        refused_pd0080(&program, "site_refused", "the call of `half`", shape);
    }
}

/// A call inside a generic body is the record the checker made for it in the
/// body's own type parameters, resolved through the instantiation emitted.
///
/// On d5166e8 a body was never walked: `wrap<T>` calling `id` emitted a call
/// to `id`, which nothing had instantiated (gcc: undeclared function). A `u32`
/// halved in a body is recorded `half<i64>`, which does not hold it: refused
/// by name (main printed -647483648 beside a `half(true)`).
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
    refused_pd0080(
        &format!("{HALF}fn wrap<T>(x: T) -> i64 {{ let v: u32 = 3000000000; return half(v); }}\n\
                  fn main() {{ let b = half(true); print_int(wrap(1)); }}\n"),
        "body_refused",
        "the call of `half`",
        "a u32 halved in a body beside a bool instantiation (main: -647483648)",
    );
}

/// #87: a generic struct literal inside a generic body is of ITS OWN record,
/// where that record holds its values exactly.
///
/// An unread `G<i64, z>` (`z = u32`, by the alias's name) decided on d5166e8
/// what a body's literal built: `G { tag: t, p: v }` with `v: i64` became that
/// `G_i64_z`, its `unsigned int p` held the i64 7, and `~gg.p` printed
/// 4294967288 where `~7` of an i64 is -8; without the unread literal gcc met
/// `incomplete type 'struct G'`. A literal whose record does NOT hold it
/// exactly — a `u32` recorded `G<i64, i64>` — is refused by name, in a body or
/// in `main`: its `long long p` would `~` the `u32` 7 as -8 where it is
/// 4294967288 (main printed 4294967288 in a body beside the unread literal,
/// gcc without it, and -8 in `main`). Nor is it built as the unread literal's
/// `G<i64, z>`: an `if` joining it with a literal of the record would be two C
/// structs where one is declared (W2a review round 9 measured that join).
#[test]
fn a_generic_struct_literal_in_a_generic_body_is_of_its_own_record() {
    let head = format!("{}struct G<X, Y> {{ tag: X, p: Y }}\n", by_name("u32", "7"));
    let decoy = "let s = S { n: 0 }; let unused = G { tag: 1, p: s.get() }; ";
    let rows: Vec<(&str, String, &str)> = vec![
        (
            "#87: an i64 body literal beside a decoy G<i64, z> (main: 4294967288)",
            format!("{head}fn build<T>(t: T) -> i64 {{ let v: i64 = 7; let gg = G {{ tag: t, p: v }}; return ~gg.p; }}\n\
                     fn main() {{ {decoy}print_int(build(1)); }}\n"),
            "RUN: -8\n",
        ),
        (
            "an `if` joining two i64 body literals beside the decoy",
            format!("{head}fn pick<T>(t: T) -> i64 {{ let v: i64 = 7; let w: i64 = 4294967297; let c = false; \
                     let r = if c {{ G {{ tag: t, p: v }} }} else {{ G {{ tag: t, p: w }} }}; return r.p; }}\n\
                     fn main() {{ {decoy}print_int(pick(1)); }}\n"),
            "RUN: 4294967297\n",
        ),
        (
            "a u32 by the alias's name in a body is of G<T, z>",
            format!("{head}fn named<T>(t: T) -> i64 {{ let s = S {{ n: 0 }}; let v = s.get(); \
                     let gg = G {{ tag: t, p: v }}; return ~gg.p; }}\n\
                     fn main() {{ print_int(named(1)); }}\n"),
            "RUN: 4294967288\n",
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

    let build_u32 =
        "fn build<T>(t: T) -> i64 { let v: u32 = 7; let gg = G { tag: t, p: v }; return ~gg.p; }\n";
    for (shape, program) in [
        (
            "#87: a u32 body literal beside the decoy (main: 4294967288)",
            format!("{head}{build_u32}fn main() {{ {decoy}print_int(build(1)); }}\n"),
        ),
        (
            "a u32 body literal, no decoy (main: gcc)",
            format!("{head}{build_u32}fn main() {{ print_int(build(1)); }}\n"),
        ),
        (
            "a u32 literal in main (main: -8)",
            format!("{head}fn main() {{ let v: u32 = 7; let gg = G {{ tag: 1, p: v }}; print_int(~gg.p); }}\n"),
        ),
        (
            "an `if` joining a u32 and an i64 body literal (main: 1)",
            format!("{head}fn pick<T>(t: T) -> i64 {{ let v: u32 = 7; let w: i64 = 4294967297; let c = false; \
                     let r = if c {{ G {{ tag: t, p: v }} }} else {{ G {{ tag: t, p: w }} }}; return r.p; }}\n\
                     fn main() {{ {decoy}print_int(pick(1)); }}\n"),
        ),
    ] {
        refused_pd0080(&program, "literal_refused", "is a literal of the generic struct", shape);
    }
}

/// An imported generic is selected by the same rule, across a module.
#[test]
fn an_imported_generic_call_is_its_own_instantiation() {
    let modules = [("halfmod.pd", "pub fn half<T>(x: T) -> T { return x / 2; }\n")];
    let got = outcome_of_files(
        &modules,
        "import halfmod;\n\nfn main() { let b = half(true); let v: i64 = 3000000000; \
         print_int(half(v)); }\n",
    );
    assert_eq!(got, "RUN: 1500000000\n");
    // #86 across a module (main: -647483648): refused by name.
    let got = outcome_of_files(
        &modules,
        "import halfmod;\n\nfn main() { let b = half(true); let v: u32 = 3000000000; \
         print_int(half(v)); }\n",
    );
    assert_eq!(got, "REFUSED");
}

/// A use no record decides is never given the first instantiation.
///
/// The type checker infers a call's type arguments from literals and
/// variables, so `half(id(x))` in a generic body has no record (the argument
/// is a call). It is of the one instantiation whose parameters are its
/// arguments' exact C types, where that C type says which kind of value the
/// argument is; an `int` is a `bool` or an `i32` and a `long long` an `i64` or
/// a `char`, so such a call is refused by name (main lowered it to the first
/// instantiation, `half__bool`, with no diagnostic). A `u64` names one kind.
/// And a generic whose type parameter is in no parameter's type cannot be
/// inferred: called in a body it has no record and no instantiation, and was
/// emitted as the bare `mk()` (gcc on main); it is refused by name.
#[test]
fn a_use_no_record_decides_is_refused_by_name_when_two_instantiations_take_it() {
    for (shape, program) in [
        (
            "an int argument: a bool or an i32 (main: half__bool, 2)",
            format!("{HALF}{ID}fn wrap<T>(x: T) -> T {{ return half(id(x)); }}\n\
                     fn main() {{ let k = half(true); let j = half(4); let i = id(true); let w = wrap(true); \
                     print_int(j); }}\n"),
        ),
        (
            "a long long argument: an i64 or a char",
            format!("{HALF}{ID}fn wrap<T>(x: T) -> T {{ return half(id(x)); }}\n\
                     fn main() {{ let k = half(true); let j = half(4); let i = id(1); let w = wrap(8); \
                     print_int(w); }}\n"),
        ),
    ] {
        refused_pd0080(&program, "unrecorded", "cannot tell which instantiation", shape);
    }
    let program = format!(
        "{}{HALF}{ID}fn wrap<T>(x: T) -> T {{ return half(id(x)); }}\n\
         fn main() {{ let s = S {{ n: 0 }}; let zval = s.get(); let k = half(true); let h = half(zval); \
         let i = id(zval); print_int(wrap(zval)); }}\n",
        by_name("u64", "9223372036854775807")
    );
    assert_eq!(outcome(&program, "unrecorded_u64"), "RUN: 4611686018427387903\n");

    refused_pd0080(
        "fn mk<U>() -> i64 { return 3; }\nfn wrap<T>(t: T) -> i64 { return mk(); }\n\
         fn main() { print_int(wrap(1)); }\n",
        "unrecorded_none",
        "`mk`",
        "a generic whose parameter is in no parameter's type, called in a body (main: gcc)",
    );
}

/// A recorded call whose ARRAY argument is not the array its parameter is in
/// the recorded instantiation is refused by name.
///
/// `first(xs)` with `xs: [u32; 2]` is recorded `first<i64>`, whose parameter
/// is a `long long[2]`. On d5166e8 the call was lowered to `first__bool` (the
/// substring match met `"unsigned int"` first, beside a `first(bs)`) and
/// printed -647483648 for `3000000000 / 2`.
#[test]
fn an_array_argument_its_recorded_instantiation_does_not_take_is_refused_by_name() {
    let program = "fn first<T>(a: [T; 2]) -> T { return a[0]; }\n\
                   fn main() { let bs = [true, false]; let b: bool = first(bs); \
                   let xs: [u32; 2] = [3000000000, 1]; let r: i64 = first(xs); \
                   print_int(r / 2); }\n";
    refused_pd0080(program, "array_arg", "unsigned int[2]", "a [u32; 2] recorded first<i64>");
    // The same call over an `[i64; 2]` runs.
    let program = "fn first<T>(a: [T; 2]) -> T { return a[0]; }\n\
                   fn main() { let bs = [true, false]; let b: bool = first(bs); \
                   let xs: [i64; 2] = [3000000000, 1]; print_int(first(xs) / 2); }\n";
    assert_eq!(outcome(program, "array_arg_i64"), "RUN: 1500000000\n");
}

/// A RECORD IS USED ONLY WHERE IT HOLDS ITS VALUES EXACTLY (review rounds 1-2).
///
/// The type checker records every integer as `i64` and a reference as its
/// referent (#82), so `half(v)` with `v: u64` is recorded `half<i64>` and
/// `id(r)` with `r: &i64` is `id<i64>`. Round 0 emitted the record: `half__i64`
/// halved 9223372036854775808 as a `long long`, where main — whose only
/// instantiation was the `unsigned long long` one — printed 4611686018427387904;
/// and `id__i64` was handed a pointer (gcc, where main printed 8 and 7). Round
/// 1 also accepted a record C converts the value into without changing it; but
/// the parameter's type decides what the body does with the value, so a `u32`
/// in a `long long` is `~`ed as an i64. A call is now of its record where each
/// argument is exactly its parameter's C type, else of the one instantiation
/// of the record's kind that is (never the `bool` one for an `i32`: both are C
/// `int`), and refused by name otherwise. An argument's C type is known, not
/// guessed: `v + 0` over a `u64` is an `unsigned long long`, and a value
/// `if` or `match` has no known type.
#[test]
fn a_record_is_used_only_where_it_holds_its_arguments_exactly() {
    let u64_head = by_name("u64", "8");
    let u32_head = by_name("u32", "8");
    let i32_head = by_name("i32", "8");
    let invert = "fn invert<T>(x: T) -> T { return ~x; }\n";
    let dbl = "fn dbl<T>(x: T) -> T { return x * 2; }\n";
    let big = "let mut v: u64 = 9223372036854775807; v = v + 1; ";
    let rows: Vec<(&str, String, &str)> = vec![
        (
            "a u64 halved in a body beside the alias's u64 instantiation (main: right)",
            format!("{u64_head}{HALF}fn wrap<T>(x: T) -> i64 {{ {big}return half(v); }}\n\
                     fn main() {{ let s = S {{ n: 0 }}; let zval = s.get(); print_int(half(zval)); \
                     print_int(wrap(1)); }}\n"),
            "RUN: 4\n4611686018427387904\n",
        ),
        (
            "a u64 halved in main beside the alias's u64 instantiation (main: wrong)",
            format!("{u64_head}{HALF}fn main() {{ let s = S {{ n: 0 }}; let zval = s.get(); \
                     let h = half(zval); {big}print_int(half(v)); }}\n"),
            "RUN: 4611686018427387904\n",
        ),
        (
            "`v + 0` over a u64 is an unsigned long long, in a body (main: right)",
            format!("{u64_head}{HALF}fn wrap<T>(x: T) -> i64 {{ let a = half(8); {big}return half(v + 0); }}\n\
                     fn main() {{ let s = S {{ n: 0 }}; let zval = s.get(); let d = half(zval); \
                     print_int(wrap(1)); }}\n"),
            "RUN: 4611686018427387904\n",
        ),
        (
            "a reference given to `id` in a body beside a reference alias's instantiation",
            format!("type R = &i64;\n{ID}fn wrap<T>(x: T) -> i64 {{ let n = 7; let r = &n; \
                     let p = id(r); return *p; }}\n\
                     fn main() {{ let n = 8; let r: R = &n; let p = id(r); print_int(*p); \
                     print_int(wrap(1)); }}\n"),
            "RUN: 8\n7\n",
        ),
        (
            "a u32 inverted in a body beside the alias's u32 instantiation (main: right)",
            format!("{u32_head}{invert}fn wrap<T>(x: T) -> i64 {{ let v: u32 = 7; return invert(v); }}\n\
                     fn main() {{ let s = S {{ n: 0 }}; let zval = s.get(); print_int(invert(zval)); \
                     print_int(wrap(1)); }}\n"),
            "RUN: 4294967287\n4294967288\n",
        ),
        (
            "an i32 beside a bool and an i32 alias's instantiation is the i32 one",
            format!("{i32_head}{dbl}fn main() {{ let s = S {{ n: 0 }}; let wv = s.get(); \
                     let b = dbl(true); let c = dbl(wv); let v: i32 = -7; print_int(dbl(v)); }}\n"),
            "RUN: -14\n",
        ),
        (
            "a parameter written as a concrete type is the type checker's business",
            "fn half<T>(x: T, n: i32) -> T { return x / n; }\nfn main() { print_int(half(10, 2)); }\n"
                .to_string(),
            "RUN: 5\n",
        ),
    ];
    table(&rows, "faithful");

    for (shape, program, fragment) in [
        (
            "a u64 with only the record's i64 instantiation (main: -4611686018427387904)",
            format!("{HALF}fn main() {{ {big}print_int(half(v)); }}\n"),
            "unsigned long long",
        ),
        (
            "a small u64 with only the record's i64 instantiation (main: 4, a lost run)",
            format!("{HALF}fn main() {{ let v: u64 = 8; print_int(half(v)); }}\n"),
            "unsigned long long",
        ),
        (
            "an i32 beside only a bool instantiation is not the bool one (main: -14)",
            format!("{dbl}fn main() {{ let b = dbl(true); let v: i32 = -7; print_int(dbl(v)); }}\n"),
            "`int`",
        ),
        (
            "a reference with only the record's i64 instantiation (main: gcc)",
            format!("{ID}fn main() {{ let v: i64 = 5; let r = &v; let p = id(r); print_int(*p); }}\n"),
            "long long*",
        ),
        (
            "an i32 halved in a body: no exact instantiation (main: gcc)",
            format!("{HALF}fn wrap<T>(t: T) -> i64 {{ let v: i32 = -10; return half(v); }}\n\
                     fn main() {{ print_int(wrap(1)); }}\n"),
            "`int`",
        ),
    ] {
        refused_pd0080(&program, "lossy_record", fragment, shape);
    }
}

/// `~`, `/`, `>>` and `<` of a `u32`, a `u64` and an `i32`, through a call and
/// through a struct literal's field (review round 2): at the instantiation of
/// the value's own width — reached by a value typed by an alias's name — each
/// computes that width's answer; at the record's `i64`, which does not hold
/// the value exactly, the program is refused by name rather than computing an
/// i64's answer.
#[test]
fn width_sensitive_operators_compute_at_the_values_own_width_or_are_refused() {
    let ops = [
        ("~", "~x"),
        ("/", "x / 2"),
        (">>", "x >> 1"),
        ("<", "if x < 8 { return 1; } return 0"),
    ];
    // (width, the value, the answer of each operator at that width)
    let widths: [(&str, &str, [Option<&str>; 4]); 3] = [
        ("u32", "7", [Some("4294967288"), Some("3"), Some("3"), Some("1")]),
        ("u64", "9223372036854775807", [None, Some("4611686018427387903"), Some("4611686018427387903"), Some("0")]),
        ("i32", "-7", [Some("6"), Some("-3"), Some("-4"), Some("1")]),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (width, value, answers) in widths {
        let head = format!("{}struct G<X> {{ v: X }}\n", by_name(width, value));
        for ((op, expr), answer) in ops.iter().zip(answers) {
            let Some(answer) = answer else { continue };
            let body = |x: &str| {
                if op == &"<" {
                    format!("if {x} < 8 {{ print_int(1); }} else {{ print_int(0); }}")
                } else {
                    format!("print_int({});", expr.replace('x', x))
                }
            };
            let call = format!("fn f<T>(x: T) -> i64 {{ return {expr}; }}\n");
            let call = if op == &"<" {
                format!("fn f<T>(x: T) -> i64 {{ {expr}; }}\n")
            } else {
                call
            };
            let exact_call = format!(
                "{head}{call}fn main() {{ let s = S {{ n: 0 }}; let v = s.get(); print_int(f(v)); }}\n"
            );
            let exact_literal = format!(
                "{head}fn main() {{ let s = S {{ n: 0 }}; let v = s.get(); let g = G {{ v: v }}; {} }}\n",
                body("g.v")
            );
            // `g.v < 8` compares a field typed by the alias's name with a literal, which
            // the type checker refuses by its own rule (as main did); the call path
            // compares inside the generic body.
            let paths = if op == &"<" {
                vec![("call", exact_call)]
            } else {
                vec![("call", exact_call), ("literal", exact_literal)]
            };
            for (path, program) in paths {
                got.push(format!("{width} {op} by {path}: {}", outcome(&program, "width_ops")));
                want.push(format!("{width} {op} by {path}: RUN: {answer}\n"));
            }
            let plain_call = format!(
                "{head}{call}fn main() {{ let v: {width} = {value}; print_int(f(v)); }}\n"
            );
            let plain_literal = format!(
                "{head}fn main() {{ let v: {width} = {value}; let g = G {{ v: v }}; {} }}\n",
                body("g.v")
            );
            refused_pd0080(&plain_call, "width_ops_refused", "the call of `f`", &format!("{width} {op}, call"));
            refused_pd0080(
                &plain_literal,
                "width_ops_refused",
                "is a literal of the generic struct",
                &format!("{width} {op}, literal"),
            );
        }
    }
    assert_eq!(got, want);
}

/// A value computed with an untyped literal is of its OTHER operand's width:
/// `v + 0` with `v: u32` is a `u32`, though C computes `v + 0LL` as a `long
/// long`. The rule above read that `long long` as proof the value is an i64,
/// so the dispatcher's p87 shape written `p: v + 0` printed `~` of an i64, -8,
/// where `~7` of a `u32` is 4294967288 (main printed 4294967288 beside the
/// unread literal and met gcc without it), and so did `let w = v + 0;
/// invert(w)` in a body (gcc on main): the binding is declared `long long`.
/// Each is refused by name, as `p: v` is. The controls run: an i64 operand, and
/// a `u64` one with its own instantiation by the alias's name.
#[test]
fn a_value_computed_with_a_literal_is_of_its_operands_width() {
    let decoy = "type z = u32;\nstruct S { n: i64 }\nimpl S { fn get(&self) -> z { return 7; } }\n";
    let g2 = "struct G<X, Y> { tag: X, p: Y }\n";
    let refused_literal = [
        (
            "p87 shape, `p: v + 0` in a body beside the unread G<i64, z> (main: 4294967288)",
            format!(
                "{decoy}{g2}fn build<T>(t: T) -> i64 {{ let v: u32 = 7; let gg = G {{ tag: t, p: v + 0 }}; return ~gg.p; }}\n\
                 fn main() {{ let s = S {{ n: 0 }}; let unused = G {{ tag: 1, p: s.get() }}; print_int(build(1)); }}\n"
            ),
        ),
        (
            "body_nodecoy shape, `p: v + 0` (main: gcc)",
            format!(
                "{g2}fn build<T>(t: T) -> i64 {{ let v: u32 = 7; let gg = G {{ tag: t, p: v + 0 }}; return ~gg.p; }}\n\
                 fn main() {{ print_int(build(1)); }}\n"
            ),
        ),
        (
            "a literal in main, `v: v + 0` (main: -8)",
            "struct G<T> { v: T }\nfn main() { let v: u32 = 7; let g = G { v: v + 0 }; print_int(~g.v); }\n"
                .to_string(),
        ),
        (
            "a literal in main, `v: 1 + v` (main: -9)",
            "struct G<T> { v: T }\nfn main() { let v: u32 = 7; let g = G { v: 1 + v }; print_int(~g.v); }\n"
                .to_string(),
        ),
        (
            "a literal of a binding `let w = v + 0` (main: -8)",
            "struct G<T> { v: T }\nfn main() { let v: u32 = 7; let w = v + 0; let g = G { v: w }; print_int(~g.v); }\n"
                .to_string(),
        ),
    ];
    for (shape, program) in &refused_literal {
        refused_pd0080(program, "literal_operand", "is a literal of the generic struct", shape);
    }
    let invert = "fn invert<T>(x: T) -> T { return ~x; }\n";
    let refused_call = [
        (
            "`let w = v + 0; invert(w)` in a body (main: gcc)",
            format!(
                "{invert}fn wrap<T>(t: T) -> i64 {{ let v: u32 = 7; let w = v + 0; return invert(w); }}\n\
                 fn main() {{ print_int(wrap(1)); }}\n"
            ),
        ),
        (
            "`let w = v + 0; invert(w)` in main (main: -8)",
            format!("{invert}fn main() {{ let v: u32 = 7; let w = v + 0; print_int(invert(w)); }}\n"),
        ),
        (
            "`let w = v * 2; half(w)` in main, `v: u32` (main: 3000000000)",
            "fn half<T>(x: T) -> T { return x / 2; }\n\
             fn main() { let v: u32 = 3000000000; let w = v * 2; print_int(half(w)); }\n"
                .to_string(),
        ),
    ];
    for (shape, program) in &refused_call {
        refused_pd0080(program, "literal_operand", "the call of `", shape);
    }
    let rows: [(&str, String, &str); 3] = [
        (
            "an i64 operand, `p: v + 0` in a body",
            format!(
                "{decoy}{g2}fn build<T>(t: T) -> i64 {{ let v: i64 = 7; let gg = G {{ tag: t, p: v + 0 }}; return ~gg.p; }}\n\
                 fn main() {{ let s = S {{ n: 0 }}; let unused = G {{ tag: 1, p: s.get() }}; print_int(build(1)); }}\n"
            ),
            "RUN: -8\n",
        ),
        (
            "an i64 binding `let w = v + 0`, forwarded",
            format!("{invert}fn main() {{ let v: i64 = 7; let w = v + 0; print_int(invert(w)); }}\n"),
            "RUN: -8\n",
        ),
        (
            "a u64 `let w = v + 0` with the alias's own instantiation",
            "type z = u64;\nstruct S { n: i64 }\nimpl S { fn get(&self) -> z { return 8; } }\n\
             fn half<T>(x: T) -> T { return x / 2; }\n\
             fn main() { let s = S { n: 0 }; let zv = s.get(); let d = half(zv); \
             let mut v: u64 = 9223372036854775807; v = v + 1; let w = v + 0; print_int(half(w)); }\n"
                .to_string(),
            "RUN: 4611686018427387904\n",
        ),
    ];
    table(&rows, "literal_operand_runs");
}
