//! A reference is a type of its own in the type checker (#88, #90).
//!
//! THE DEFECT, MEASURED ON d5166e8 BEFORE THE FIX
//!
//! `check_expression` answered `&m` with `m`'s type ("references have the same
//! type as their inner value / TODO"), and a written `&T` was read as `T`, with
//! the integer width erased below it. So every place a reference value met a
//! typed place compared a pointer with a value, or a `&u32` with a `&i64`, as
//! one type — and the C said otherwise:
//!
//! ```text
//! let r = if c { a } else { sb.get() };    // a: i64, get() -> &i64
//!   -> error: gcc compilation failed                       (#88)
//! let m = 4294967297; let v: &u32 = &m;    // and `type A = &u32; let v: A`
//!   -> internal compiler error: the C backend emitted ill-typed C   (#90)
//! let r = &a; let s = &b; if r == s ...    // a == b == 5
//!   -> ran, printed `ne`: two ADDRESSES compared
//! s - r                                    // 7 - 5
//!   -> ran, printed `-1`: a pointer difference
//! let r = &b; while r { .. }               // b == false
//!   -> never ended: a pointer is always true
//! ```
//!
//! THE MODEL IS CODE GENERATION'S, measured rather than invented: a parameter
//! WRITTEN `&T` is read as its referent (C `(*p)`) and takes a place (C adds
//! the `&`); every other reference value — `&e`, a binding holding one, a
//! `-> &T` result, a field or a parameter declared by a reference alias — is a
//! C pointer, and is a `CheckerType::Reference` here. Nothing is dereferenced
//! that the program did not dereference.
//!
//! Every refused row is refused by THIS compiler (never gcc); every accepted
//! row links and runs against a printed answer.

mod common;

use common::unique_module_name;
use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Compile, link and run, and say how it went: `RUN: ` and the program's
/// output, `REFUSED[PD####]: ` (or `REFUSED[-]: ` for an uncoded refusal) and
/// the first line of this compiler's refusal, or `GCC` when the C it emitted
/// did not compile, or `HANG` when the program ran for ten seconds (`while r`
/// over a `&bool` never ended). A row's expectation is a PREFIX of its
/// outcome, so a refusal is pinned by its code and the start of its sentence.
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
            if text.contains("gcc") || text.contains("ill-typed C") {
                return "GCC".to_string();
            }
            let code = e.code().map_or("-".to_string(), |c| c.to_string());
            let first = text.lines().next().unwrap_or("").to_string();
            return format!("REFUSED[{}]: {}", code, first);
        }
    };
    let out = link_command(&c_file, &exe, OptLevel::Default)
        .expect("link_command")
        .output()
        .expect("gcc");
    if !out.status.success() {
        return "GCC".to_string();
    }
    let mut child = Command::new(&exe)
        .stdout(Stdio::piped())
        .spawn()
        .expect("run");
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().expect("wait").is_none() {
        if Instant::now() > deadline {
            child.kill().ok();
            child.wait().ok();
            return "HANG".to_string();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let run = child.wait_with_output().expect("run");
    format!("RUN: {}", String::from_utf8_lossy(&run.stdout))
}

/// `outcome`, for a program that imports `modules`. The module resolver
/// searches the WORKING directory (`src/resolver/mod.rs`), which an in-process
/// compile cannot own per test, so this drives the real `pdc` binary with the
/// scratch directory as its `current_dir` (as tests/m3_imported_calls.rs does)
/// and reads the refusal's code from its printed `error[PD####]` header.
fn outcome_with(modules: &[(&str, &str)], source: &str, prefix: &str) -> String {
    let dir = TempDir::new().unwrap();
    for (name, text) in modules {
        fs::write(dir.path().join(name), text).unwrap();
    }
    let name = unique_module_name(prefix);
    fs::write(dir.path().join(format!("{}.pd", name)), source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .args(["compile", &format!("{}.pd", name), "-o", &name])
        .current_dir(dir.path())
        .output()
        .expect("failed to run pdc");
    if !out.status.success() {
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if text.contains("gcc") || text.contains("ill-typed C") {
            return "GCC".to_string();
        }
        // The header is `error[PD0022]: ...` or `error: ...`, between ANSI colours.
        let plain: String = text
            .split('\x1b')
            .enumerate()
            .map(|(i, part)| if i == 0 { part } else { part.split_once('m').map_or("", |(_, rest)| rest) })
            .collect();
        let header = plain
            .lines()
            .find(|l| l.starts_with("error"))
            .unwrap_or("");
        let (code, first) = match header.strip_prefix("error[") {
            Some(rest) => rest.split_once("]: ").unwrap_or(("-", rest)),
            None => ("-", header.strip_prefix("error: ").unwrap_or(header)),
        };
        return format!("REFUSED[{}]: {}", code, first);
    }
    let exe = dir.path().join("build_output").join(&name);
    let run = Command::new(&exe).output().expect("run");
    format!("RUN: {}", String::from_utf8_lossy(&run.stdout))
}

/// Run every row and compare the table whole, so one run shows every row
/// that differs.
fn check_table(rows: &[(&str, &str, &str)], prefix: &str) {
    let mut wrong = Vec::new();
    for (what, source, expected) in rows {
        let got = outcome(source, prefix);
        if !got.starts_with(expected) {
            wrong.push(format!("{}\n    expected: {}\n    got:      {}", what, expected, got));
        }
    }
    assert!(wrong.is_empty(), "{} row(s) differ:\n{}", wrong.len(), wrong.join("\n"));
}

/// #88: a reference value meets a place of its referent's type. Each of these
/// reached gcc (or the ill-typed-C trap) on d5166e8.
#[test]
fn a_reference_meets_a_value_place_and_is_refused_by_name() {
    let get = "struct S1 { n: i64 }\nimpl S1 { fn get(&self) -> i64 { return 4294967297; } }\n\
               struct S2 { n: i64 }\nimpl S2 { fn get(&self) -> &i64 { return &self.n; } }\n";
    let merge = format!(
        "{}fn main() {{ let sa = S1 {{ n: 0 }}; let a = sa.get(); let sb = S2 {{ n: 9 }}; \
         let b = sb.get(); let c = true; let r = if c {{ a }} else {{ b }}; print_int(0); }}\n",
        get
    );
    let merge_alias = format!("type N = i64;\n{}", merge.replace("-> i64 { return 4", "-> N { return 4"));
    let rows: Vec<(&str, &str, &str)> = vec![
        ("an `if` merging an `i64` and a `&i64` result (#88)", &merge, "REFUSED[PD0058]"),
        ("the same under `type N = i64` (#88)", &merge_alias, "REFUSED[PD0058]"),
        (
            "an `if` merging `&m` and `n`",
            "fn main() { let m: i64 = 5; let n: i64 = 6; let c = true; let v = if c { &m } else { n }; print_int(v); }\n",
            "REFUSED[PD0058]",
        ),
        (
            "a `match` merging `&m` and `n`",
            "fn main() { let m: i64 = 5; let n: i64 = 6; let k: i64 = 1; let v = match k { 1 => &m, _ => n }; print_int(v); }\n",
            "REFUSED[-]",
        ),
        (
            "an array of `&m` and `n`",
            "fn main() { let m: i64 = 5; let n: i64 = 6; let a = [&m, n]; print_int(a[1]); }\n",
            "REFUSED[-]",
        ),
        (
            "`let v: i64 = &m`",
            "fn main() { let m: i64 = 5; let v: i64 = &m; print_int(v); }\n",
            "REFUSED[PD0022]",
        ),
        (
            "`let v: &i64 = m`",
            "fn main() { let m: i64 = 5; let v: &i64 = m; print_int(*v); }\n",
            "REFUSED[PD0022]",
        ),
        (
            "`let b: i64 = s.get()` over `-> &i64`",
            "struct S { n: i64 }\nimpl S { fn get(&self) -> &i64 { return &self.n; } }\nfn main() { let s = S { n: 9 }; let b: i64 = s.get(); print_int(b); }\n",
            "REFUSED[PD0022]",
        ),
        (
            "`&m` for an `i64` parameter",
            "fn f(x: i64) -> i64 { x + 1 }\nfn main() { let m: i64 = 5; print_int(f(&m)); }\n",
            "REFUSED[PD0042]",
        ),
        (
            "`&m` for a method's `i64` parameter",
            "struct S { n: i64 }\nimpl S { fn add(&self, k: i64) -> i64 { self.n + k } }\nfn main() { let s = S { n: 1 }; let m: i64 = 5; print_int(s.add(&m)); }\n",
            "REFUSED[PD0042]",
        ),
        (
            "a bound reference for an `i64` parameter",
            "fn f(x: i64) -> i64 { x + 1 }\nfn main() { let m: i64 = 5; let r = &m; print_int(f(r)); }\n",
            "REFUSED[PD0042]",
        ),
        (
            "a bound reference handed to a parameter written `&i64`: code generation borrows it again",
            "fn f(x: &i64) -> i64 { x + 1 }\nfn main() { let m: i64 = 5; let r = &m; print_int(f(r)); }\n",
            "REFUSED[PD0084]",
        ),
        (
            "a method called through a bound reference",
            "struct S { n: i64 }\nimpl S { fn get(&self) -> i64 { self.n } }\nfn main() { let s = S { n: 4 }; let r = &s; print_int(r.get()); }\n",
            "REFUSED[-]: `r` has no method `get`",
        ),
        (
            "`return &k` from `-> i64`",
            "fn f(m: i64) -> i64 { let k = m; return &k; }\nfn main() { print_int(f(5)); }\n",
            "REFUSED[-]",
        ),
        (
            "a `&i64` parameter (read as `i64`) returned from `-> &i64`",
            "fn f(x: &i64) -> &i64 { x }\nfn main() { let m: i64 = 5; print_int(*f(&m)); }\n",
            "REFUSED[-]",
        ),
        (
            "`&m` for a struct's `i64` field",
            "struct S { n: i64 }\nfn main() { let m: i64 = 5; let s = S { n: &m }; print_int(s.n); }\n",
            "REFUSED[-]",
        ),
        (
            "`&m` for an enum's `i64` payload",
            "enum E { A(i64), B }\nfn main() { let m: i64 = 5; let e = E::A(&m); match e { E::A(x) => print_int(x), E::B => print_int(0) } }\n",
            "REFUSED[-]",
        ),
        (
            "`n = &m` over an `i64`",
            "fn main() { let m: i64 = 5; let mut n: i64 = 0; n = &m; print_int(n); }\n",
            "REFUSED[-]",
        ),
        (
            "`r = k` over a bound reference",
            "fn main() { let m: i64 = 5; let k: i64 = 7; let mut r = &m; r = k; print_int(*r); }\n",
            "REFUSED[-]",
        ),
        (
            "a bound reference handed to a generic",
            "fn id<T>(a: T) -> T { a }\nfn main() { let m: i64 = 5; let r = &m; print_int(id(r)); }\n",
            "REFUSED[-]",
        ),
        (
            "a `&i64` result and an `i64` at one type parameter",
            "fn g<T>(a: T, b: T) -> i64 { 1 }\nstruct S { n: i64 }\nimpl S { fn get(&self) -> &i64 { return &self.n; } }\nfn main() { let s = S { n: 9 }; let a = s.get(); let n: i64 = 6; print_int(g(a, n)); }\n",
            "REFUSED[-]",
        ),
    ];
    check_table(&rows, "ref_value_place");
}

/// #90: a reference to one referent meets a reference to another. Integer
/// width and signedness are erased in this checker's `Int`; behind a reference
/// they are a C pointer type, so the referent is compared as the source spells
/// it wherever both sides say.
#[test]
fn a_reference_to_one_width_meets_a_reference_to_another_and_is_refused() {
    let rows: Vec<(&str, &str, &str)> = vec![
        (
            "`let v: A = &m` under `type A = &u32`, `m` an i64 (#90)",
            "type A = &u32;\nfn main() { let m = 4294967297; let v: A = &m; print_int(0); }\n",
            "REFUSED[PD0022]",
        ),
        (
            "`let v: &u32 = &m`, `m` an i64 (#90)",
            "fn main() { let m = 4294967297; let v: &u32 = &m; print_int(0); }\n",
            "REFUSED[PD0022]",
        ),
        (
            "`let v: &i32 = &m`",
            "fn main() { let m: i64 = 5; let v: &i32 = &m; print_int(*v); }\n",
            "REFUSED[PD0022]",
        ),
        (
            "`let v: &i64 = &m`, `m` a u64 (signedness)",
            "fn main() { let m: u64 = 5; let v: &i64 = &m; print_int(*v); }\n",
            "REFUSED[PD0022]",
        ),
        (
            "`let v: &(i32, i32) = &p`, `p` an `(i64, i64)`",
            "fn main() { let p: (i64, i64) = (1, 2); let v: &(i32, i32) = &p; print_int((*v).0); }\n",
            "REFUSED[PD0022]",
        ),
        (
            "`&m` for `x: &u32`",
            "fn f(x: &u32) -> i64 { 7 }\nfn main() { let m: i64 = 5; print_int(f(&m)); }\n",
            "REFUSED[PD0042]",
        ),
        (
            "the place `m` for `x: &u32` (code generation adds the `&`)",
            "fn f(x: &u32) -> i64 { 7 }\nfn main() { let m: i64 = 5; print_int(f(m)); }\n",
            "REFUSED[PD0042]",
        ),
        (
            "`&m` for `x: A` under `type A = &u32`",
            "type A = &u32;\nfn f(x: A) -> i64 { 7 }\nfn main() { let m: i64 = 5; print_int(f(&m)); }\n",
            "REFUSED[PD0042]",
        ),
        (
            "`&m` for a method's `k: &u32`",
            "struct S { n: i64 }\nimpl S { fn add(&self, k: &u32) -> i64 { self.n } }\nfn main() { let s = S { n: 1 }; let m: i64 = 5; print_int(s.add(&m)); }\n",
            "REFUSED[PD0042]",
        ),
        (
            "an `if` merging `&m` (i64) and `&n` (u32)",
            "fn main() { let m: i64 = 5; let n: u32 = 6; let c = true; let v = if c { &m } else { &n }; print_int(*v); }\n",
            "REFUSED[PD0058]",
        ),
        (
            "`r = &k` (i64) over `let mut r = &m` (u32)",
            "fn main() { let m: u32 = 5; let k: i64 = 7; let mut r = &m; r = &k; print_int(*r); }\n",
            "REFUSED[-]",
        ),
        (
            "returning a `&i64` parameter's borrow from `-> &u32`",
            "fn f(x: &i64) -> &u32 { &x }\nfn main() { print_int(1); }\n",
            "REFUSED[-]",
        ),
    ];
    check_table(&rows, "ref_width");
}

/// A reference used where its referent's VALUE is wanted. These are the worst
/// rows: on d5166e8 the ones that compiled RAN and printed an address, a
/// pointer difference or a pointer's truth — or never ended.
#[test]
fn a_reference_used_as_its_referent_value_is_refused() {
    let rows: Vec<(&str, &str, &str)> = vec![
        (
            "`r == s` over two bound references (printed `ne` for 5 == 5)",
            "fn main() { let a: i64 = 5; let b: i64 = 5; let r = &a; let s = &b; if r == s { print(\"eq\"); } else { print(\"ne\"); } }\n",
            "REFUSED[PD0083]",
        ),
        (
            "`&a == &b` (printed `ne`)",
            "fn main() { let a: i64 = 5; let b: i64 = 5; if &a == &b { print(\"eq\"); } else { print(\"ne\"); } }\n",
            "REFUSED[PD0083]",
        ),
        (
            "`r == 5` (printed `ne`)",
            "fn main() { let a: i64 = 5; let r = &a; if r == 5 { print(\"eq\"); } else { print(\"ne\"); } }\n",
            "REFUSED[PD0083]",
        ),
        (
            "`s.get() == t.get()` over `-> &i64` (printed `ne` for 9 == 9)",
            "struct S { n: i64 }\nimpl S { fn get(&self) -> &i64 { return &self.n; } }\nfn main() { let s = S { n: 9 }; let t = S { n: 9 }; if s.get() == t.get() { print(\"eq\"); } else { print(\"ne\"); } }\n",
            "REFUSED[PD0083]",
        ),
        (
            "`s - r` (printed `-1` for 7 - 5)",
            "fn main() { let a: i64 = 5; let b: i64 = 7; let r = &a; let s = &b; print_int(s - r); }\n",
            "REFUSED[PD0083]",
        ),
        (
            "`r < s` (compared addresses)",
            "fn main() { let a: i64 = 9; let b: i64 = 5; let r = &a; let s = &b; if r < s { print(\"lt\"); } else { print(\"ge\"); } }\n",
            "REFUSED[PD0083]",
        ),
        (
            "`r += 1` (stepped the pointer, printed garbage)",
            "fn main() { let a: i64 = 5; let mut r = &a; r += 1; print_int(*r); }\n",
            "REFUSED[PD0083]",
        ),
        (
            "`r + 1`",
            "fn main() { let m: i64 = 5; let r = &m; let k = r + 1; print_int(k); }\n",
            "REFUSED[PD0083]",
        ),
        (
            "`-r`",
            "fn main() { let a: i64 = 5; let r = &a; print_int(-r); }\n",
            "REFUSED[PD0083]",
        ),
        (
            "`!r` over a `&bool` (read the pointer's truth)",
            "fn main() { let b = true; let r = &b; if !r { print(\"t\"); } else { print(\"f\"); } }\n",
            "REFUSED[PD0083]",
        ),
        (
            "`print_int(&m)`",
            "fn main() { let m: i64 = 5; print_int(&m); }\n",
            "REFUSED[PD0042]",
        ),
        (
            "`print_int(r)`",
            "fn main() { let m: i64 = 5; let r = &m; print_int(r); }\n",
            "REFUSED[PD0042]",
        ),
        (
            "`print(r)` over a `&String`",
            "fn main() { let s: String = \"hi\"; let r = &s; print(r); }\n",
            "REFUSED[PD0042]",
        ),
        (
            "`r as i64` (printed the address)",
            "fn main() { let m: i64 = 5; let r = &m; print_int(r as i64); }\n",
            "REFUSED[PD0003]",
        ),
        (
            "`if r` over a `&bool` (always true)",
            "fn main() { let b = false; let r = &b; if r { print(\"t\"); } else { print(\"f\"); } }\n",
            "REFUSED[-]",
        ),
        (
            "`while r` over a `&bool` (never ended)",
            "fn main() { let b = false; let r = &b; while r { print(\"x\"); } print(\"done\"); }\n",
            "REFUSED[-]",
        ),
        (
            "`match r { 3 => .. }` (printed `other` for 3)",
            "fn main() { let n: i64 = 3; let r = &n; match r { 3 => print(\"three\"), _ => print(\"other\") } }\n",
            "REFUSED[PD0009]",
        ),
        (
            "`a[r]`",
            "fn main() { let a: [i64; 3] = [1, 2, 3]; let i: i64 = 1; let r = &i; print_int(a[r]); }\n",
            "REFUSED[-]",
        ),
        (
            "`r.n` over a bound `&S`",
            "struct S { n: i64 }\nfn main() { let s = S { n: 4 }; let r = &s; print_int(r.n); }\n",
            "REFUSED[-]",
        ),
    ];
    check_table(&rows, "ref_as_value");
}

/// `*e` needs a reference VALUE. A parameter written `&T` is already read as
/// its referent, so `*x` on one asked for a second dereference (gcc refused it
/// on d5166e8, exactly as it refused `*self` before PD0050).
#[test]
fn a_dereference_needs_a_reference_value() {
    let rows: Vec<(&str, &str, &str)> = vec![
        (
            "`*m` over an `i64`",
            "fn main() { let m: i64 = 5; print_int(*m); }\n",
            "REFUSED[PD0082]",
        ),
        (
            "`*x` over a parameter written `&i64`",
            "fn f(x: &i64) -> i64 { *x }\nfn main() { let m: i64 = 5; print_int(f(&m)); }\n",
            "REFUSED[PD0082]",
        ),
        (
            "`*x = *x + 1` over a parameter written `&mut i64`",
            "fn bump(x: &mut i64) { *x = *x + 1; }\nfn main() { let mut m: i64 = 5; bump(&mut m); print_int(m); }\n",
            "REFUSED[PD0082]",
        ),
        (
            "`**r` over one reference",
            "fn main() { let m: i64 = 5; let r = &m; print_int(**r); }\n",
            "REFUSED[PD0082]",
        ),
    ];
    check_table(&rows, "ref_deref");
}

/// Every reference shape that RAN on d5166e8 still runs, with the same output.
/// Not a RED test — the guard that the class was cut out and nothing else.
#[test]
fn references_that_ran_before_still_run() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("`let v: &i64 = &m; *v`", "fn main() { let m: i64 = 5; let v: &i64 = &m; print_int(*v); }\n", "RUN: 5\n"),
        ("`let v: &u32 = &m`, `m` a u32", "fn main() { let m: u32 = 5; let v: &u32 = &m; print_int(*v); }\n", "RUN: 5\n"),
        ("`type A = &i64; let v: A = &m`", "type A = &i64;\nfn main() { let m: i64 = 5; let v: A = &m; print_int(*v); }\n", "RUN: 5\n"),
        ("`let r = &m; *r + 1`", "fn main() { let m: i64 = 5; let r = &m; print_int(*r + 1); }\n", "RUN: 6\n"),
        ("`&String`, dereferenced", "fn main() { let s: String = \"hi\"; let v: &String = &s; print(*v); }\n", "RUN: hi\n"),
        ("an `if` of two references", "fn main() { let m: i64 = 5; let n: i64 = 6; let c = true; let v = if c { &m } else { &n }; print_int(*v); }\n", "RUN: 5\n"),
        ("an `if` of two bound references", "fn main() { let m: i64 = 5; let n: i64 = 6; let r = &m; let s = &n; let c = false; let v = if c { r } else { s }; print_int(*v); }\n", "RUN: 6\n"),
        ("`r = &k` over `let mut r = &m`", "fn main() { let m: i64 = 5; let k: i64 = 7; let mut r = &m; r = &k; print_int(*r); }\n", "RUN: 7\n"),
        ("`x + 1` over `x: &i64`", "fn f(x: &i64) -> i64 { x + 1 }\nfn main() { let m: i64 = 5; print_int(f(&m)); print_int(f(m)); }\n", "RUN: 6\n6\n"),
        ("`x == y` over two `&i64` parameters", "fn f(x: &i64, y: &i64) -> bool { x == y }\nfn main() { let a: i64 = 5; let b: i64 = 5; if f(&a, &b) { print(\"eq\"); } else { print(\"ne\"); } }\n", "RUN: eq\n"),
        ("a `&i64` parameter passed on, and read", "fn g(x: &i64) -> i64 { x * 2 }\nfn h(x: i64) -> i64 { x * 3 }\nfn f(x: &i64) -> i64 { g(x) + h(x) }\nfn main() { let m: i64 = 5; print_int(f(&m)); }\n", "RUN: 25\n"),
        ("a `&S` parameter, by place and by borrow", "struct S { n: i64 }\nfn f(s: &S) -> i64 { s.n }\nfn main() { let s = S { n: 3 }; print_int(f(&s)); print_int(f(s)); }\n", "RUN: 3\n3\n"),
        ("`&mut S`, written through", "struct C { n: i64 }\nfn bump(c: &mut C) { c.n = c.n + 1; }\nfn main() { let mut c = C { n: 1 }; bump(&mut c); print_int(c.n); }\n", "RUN: 2\n"),
        ("`&u32` given a `u32` place and its borrow", "fn f(x: &u32) -> i64 { 7 }\nfn main() { let m: u32 = 5; print_int(f(m)); print_int(f(&m)); }\n", "RUN: 7\n7\n"),
        ("`&String` parameter", "fn f(s: &String) -> i64 { string_len(s) }\nfn main() { let t: String = \"abcd\"; print_int(f(&t)); }\n", "RUN: 4\n"),
        ("`&[i64; 2]` and `&mut [i64; 2]` parameters", "fn f(a: &[i64; 2]) -> i64 { a[1] }\nfn w(a: &mut [i64; 2]) { a[0] = 9; }\nfn main() { let mut a: [i64; 2] = [1, 2]; w(&mut a); print_int(f(&a) + a[0]); }\n", "RUN: 11\n"),
        ("`&K` parameter matched", "enum K { A, B }\nfn f(k: &K) -> i64 { match k { K::A => 1, K::B => 2 } }\nfn main() { let k = K::B; print_int(f(&k)); }\n", "RUN: 2\n"),
        ("`let y = x` over `x: &i64`", "fn f(x: &i64) -> i64 { let y = x; y + 1 }\nfn main() { let m: i64 = 5; print_int(f(&m)); }\n", "RUN: 6\n"),
        ("`let r = &x; *r` over `x: &i64`", "fn f(x: &i64) -> i64 { let r = &x; *r }\nfn main() { let m: i64 = 5; print_int(f(&m)); }\n", "RUN: 5\n"),
        ("a reference to a reference", "fn main() { let m: i64 = 5; let r = &m; let rr = &r; print_int(**rr); }\n", "RUN: 5\n"),
        ("`(*r).n` and `(*r).1`", "struct S { n: i64 }\nfn main() { let s = S { n: 4 }; let r = &s; let p: (i64, i64) = (1, 2); let q = &p; print_int((*r).n + (*q).1); }\n", "RUN: 6\n"),
        ("`*r = 5` through `let r = &mut m`", "fn main() { let mut m: i64 = 1; let r = &mut m; *r = 5; print_int(m); }\n", "RUN: 5\n"),
        ("an array and a tuple of references", "fn main() { let m: i64 = 5; let n: i64 = 6; let a = [&m, &n]; let t = (&m, &n); print_int(*a[1] + *t.0); }\n", "RUN: 11\n"),
        ("`-> &i64` from `&self.n`, dereferenced and bound", "struct S { n: i64 }\nimpl S { fn get(&self) -> &i64 { return &self.n; } }\nfn main() { let s = S { n: 9 }; let b: &i64 = s.get(); print_int(*s.get() + *b); }\n", "RUN: 18\n"),
        ("`-> &i64` from a `&i64` parameter's borrow", "fn pick(a: &i64, b: &i64) -> &i64 { &a }\nfn main() { let m: i64 = 3; let n: i64 = 4; print_int(*pick(&m, &n)); }\n", "RUN: 3\n"),
        ("a reference alias's parameter returned and bound", "type A = &i64;\nfn w(r0: A) -> A { return r0; }\nfn pk(r0: A) -> i64 { let x: A = r0; let r = x; return *r; }\nfn main() { let m = 4294967297; let r = w(&m); print_int(*r); print_int(pk(&m)); }\n", "RUN: 4294967297\n4294967297\n"),
        ("a bound reference for a reference alias's parameter", "type A = &i64;\nfn kk(x: A) -> i64 { return *x; }\nfn main() { let m = 4; let r0: A = &m; let r = r0; print_int(kk(r)); }\n", "RUN: 4\n"),
        ("a reference alias's struct field", "type A = &i64;\nstruct H { p: A }\nfn main() { let m = 4; let hh = H { p: &m }; let r = hh.p; print_int(*r); }\n", "RUN: 4\n"),
        ("a reference alias's value handed to a generic", "type A = &i64;\nfn g<U>(u: U) -> i64 { return *u + *u; }\nfn main() { let m = 4; let r0: A = &m; print_int(g(r0)); }\n", "RUN: 8\n"),
    ];
    check_table(&rows, "ref_still_runs");
}

/// REVIEW ROUND 1: A NAME IS READ IN THE SCOPE IT IS BOUND IN. The referent of
/// `&v` was derived by re-reading `v`'s initialiser after its scopes had
/// closed, so `{ let x: i64 = ..; x }` under an outer `x: u32` was read
/// through the OUTER `x` and refused. Each of these ran on main (903ba02)
/// and printed what it prints here.
#[test]
fn a_name_inside_a_value_is_read_in_its_own_scope() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("a block's `let` shadowing an outer `u32`", "fn main() { let x: u32 = 2; let v = { let x: i64 = 4294967297; x }; let r: &i64 = &v; print_int(*r); }\n", "RUN: 4294967297\n"),
        ("the same, into a parameter written `&i64`", "fn f(x: &i64) -> i64 { x }\nfn main() { let x: u32 = 2; let v = { let x: i64 = 4294967297; x }; print_int(f(&v)); }\n", "RUN: 4294967297\n"),
        ("the reverse: an inner `u32` under an outer `i64`", "fn main() { let x: i64 = 2; let v = { let x: u32 = 4294967295; x }; let r: &u32 = &v; print_int(*r); }\n", "RUN: 4294967295\n"),
        ("an `if` branch's `let`", "fn main() { let x: u32 = 2; let c = true; let v = if c { let x: i64 = 4294967297; x } else { 7 }; let r: &i64 = &v; print_int(*r); }\n", "RUN: 4294967297\n"),
        ("a `match` arm's binding", "fn main() { let x: u32 = 2; let v = match 4294967297 { x => x }; let r: &i64 = &v; print_int(*r); }\n", "RUN: 4294967297\n"),
    ];
    check_table(&rows, "ref_scope");
}

/// REVIEW ROUND 1: AN UNKNOWN REFERENT IS NEVER A MATCH. Where nothing was
/// known about a referent it was taken to agree, so a 64-bit value reached a
/// `&u32` through an array repeat, a `loop`, a block, a generic's result or a
/// generic struct's field — the ill-typed-C trap on main and on round 1. Each
/// is now derived exactly, as code generation types it, and refused where it
/// disagrees; the `&i64` twins run as they did.
#[test]
fn a_referent_is_derived_exactly_or_refused() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("an array repeat's element for `&u32`", "fn main() { let a = [4294967297; 2]; let r: &u32 = &a[0]; print_int(*r); }\n", "REFUSED[PD0022]"),
        ("an array repeat's element for `&i64`", "fn main() { let a = [4294967297; 2]; let r: &i64 = &a[0]; print_int(*r); }\n", "RUN: 4294967297\n"),
        ("a `loop`'s `break` value for `&u32`", "fn main() { let v = loop { break 4294967297; }; let r: &u32 = &v; print_int(*r); }\n", "REFUSED[PD0022]"),
        ("a block's value for `&u32`", "fn main() { let v = { let x: i64 = 4294967297; x }; let r: &u32 = &v; print_int(*r); }\n", "REFUSED[PD0022]"),
        ("a `match` on an `i64` for `&u32`", "fn main() { let v = match 4294967297 { x => x }; let r: &u32 = &v; print_int(*r); }\n", "REFUSED[PD0022]"),
        ("a generic's `-> i64` result for `&u32`", "fn f<T>(x: T) -> i64 { 4294967297 }\nfn main() { let v = f(1); let r: &u32 = &v; print_int(*r); }\n", "REFUSED[PD0022]"),
        ("a generic's `-> i64` result for `&i64`", "fn f<T>(x: T) -> i64 { 4294967297 }\nfn main() { let v = f(1); let r: &i64 = &v; print_int(*r); }\n", "RUN: 4294967297\n"),
        ("a generic struct's field for `&u32`", "struct G<T> { n: T }\nfn main() { let g = G { n: 5 }; let r: &u32 = &g.n; print_int(*r); }\n", "REFUSED[PD0022]"),
        ("a generic struct's field for `&i64`", "struct G<T> { n: T }\nfn main() { let g = G { n: 5 }; let r: &i64 = &g.n; print_int(*r); }\n", "RUN: 5\n"),
        ("a generic struct's field for a parameter written `&u32`", "struct G<T> { n: T }\nfn f(x: &u32) -> i64 { x as i64 }\nfn main() { let g = G { n: 5 }; print_int(f(g.n)); }\n", "REFUSED[PD0042]"),
        ("a generic struct's field beside a `&u32` in an `if`", "struct G<T> { n: T }\nfn main() { let g = G { n: 5 }; let m: u32 = 7; let c = true; let r = if c { &g.n } else { &m }; print_int(*r); }\n", "REFUSED[PD0058]"),
        ("an array repeat's element beside a `&u32`, assigned", "fn main() { let a = [4294967297; 2]; let m: u32 = 7; let mut r = &a[0]; r = &m; print_int(*r); }\n", "REFUSED[-]"),
    ];
    check_table(&rows, "ref_exact");
    let module = [("zwidth.pd", "pub struct S { n: i64 }\npub enum E { A(i64) }\n")];
    let imported = [
        ("an imported struct's field for `&u32`", "import zwidth;\nfn main() { let s = S { n: 4294967297 }; let r: &u32 = &s.n; print_int(*r); }\n", "REFUSED[PD0022]"),
        ("an imported struct's field for `&i64`", "import zwidth;\nfn main() { let s = S { n: 4294967297 }; let r: &i64 = &s.n; print_int(*r); }\n", "RUN: 4294967297\n"),
        ("an imported enum's payload for `&u32`", "import zwidth;\nfn main() { let e = E::A(4294967297); match e { E::A(n) => { let r: &u32 = &n; print_int(*r); } } }\n", "REFUSED[PD0022]"),
    ];
    let wrong: Vec<String> = imported
        .iter()
        .filter_map(|(what, source, expected)| {
            let got = outcome_with(&module, source, "ref_exact_import");
            (!got.starts_with(expected))
                .then(|| format!("{}\n    expected: {}\n    got:      {}", what, expected, got))
        })
        .collect();
    assert!(wrong.is_empty(), "{} row(s) differ:\n{}", wrong.len(), wrong.join("\n"));
}

/// REVIEW ROUND 1: THE RULES HOLD IN A GENERIC BODY, at each instantiation.
/// A generic body is monomorphized, not walked, so `r == s` over two borrows
/// compared ADDRESSES there and printed `ne` for `same(5, 5)`, where the
/// non-generic twin is refused. Generic bodies that ran still run.
#[test]
fn the_reference_rules_hold_in_an_instantiated_generic_body() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("`r == s` over two borrows, in a generic", "fn same<T>(a: T, b: T) -> bool { let r = &a; let s = &b; r == s }\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]: in `same` instantiated at `i64`"),
        ("a width mismatch in a generic body", "fn f<T>(n: T) -> i64 { let m: i64 = 4294967297; let r: &u32 = &m; *r }\nfn main() { print_int(f(5)); }\n", "REFUSED[PD0022]: in `f` instantiated at `i64`"),
        ("a generic reached only through another generic", "fn same<T>(a: T, b: T) -> bool { let r = &a; let s = &b; r == s }\nfn outer<U>(u: U) -> bool { same(u, u) }\nfn main() { if outer(5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a generic never instantiated is not checked", "fn same<T>(a: T, b: T) -> bool { let r = &a; let s = &b; r == s }\nfn main() { print_int(1); }\n", "RUN: 1\n"),
        ("a generic body with `*u` over a reference alias", "type A = &i64;\nfn g<U>(u: U) -> i64 { return *u + *u; }\nfn main() { let m = 4; let r0: A = &m; print_int(g(r0)); }\n", "RUN: 8\n"),
        ("a generic body that names its type parameter", "fn id<T>(x: T) -> T { let y = x; y }\nfn main() { print_int(id(4)); }\n", "RUN: 4\n"),
        ("a generic body comparing values", "fn same<T>(a: T, b: T) -> bool { a == b }\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "RUN: eq\n"),
        ("a generic body reading a parameter written `&T`", "fn first<T>(n: T, x: &T) -> T { x }\nfn main() { let n = 5; print_int(first(n, &n)); }\n", "RUN: 5\n"),
        // `let x: T` is declared `void *x` (a body is not substituted), which C
        // accepts from a `String`; the borrows of it were compared unchecked
        // and printed `ne` for `same("k", "k")` on main, at any depth.
        ("`r == s` over borrows of `let x: T` bindings", "fn same<T>(a: T, b: T) -> bool { let x: T = a; let y: T = b; let r = &x; let s = &y; r == s }\nfn main() { if same(\"k\", \"k\") { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]: in `same` instantiated at `String`"),
        ("the same inside an `if`", "fn same<T>(a: T, b: T, c: bool) -> bool { if c { let x: T = a; let y: T = b; let r = &x; let s = &y; return r == s; } false }\nfn main() { if same(\"k\", \"k\", true) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a `let x: T` binding that is only passed on", "fn pass<T>(a: T) -> T { let x: T = a; let y = x; y }\nfn main() { print(pass(\"ok\")); }\n", "RUN: ok\n"),
    ];
    check_table(&rows, "ref_generic_body");
}

/// REVIEW ROUND 2: A VALUE `loop`'S BINDING IS READ AS CODE GENERATION
/// DECLARES IT. `let v = loop { .. }` is declared with the C type of the first
/// `break` value READ IN THE SCOPES OUTSIDE THE LOOP (`try_infer_expr_type` at
/// the `let`, src/codegen/mod.rs). Every name the body bound anywhere — even in
/// a closed `if false { .. }`, a `for`, a `match` arm — was taken to shadow the
/// one the `break` reads, so each `&i64` row below was refused (PD0022) though
/// main ran it and printed this. The `&u32` rows are the ill-typed-C trap on
/// main and stay refused: `v` is the outer `i64` whatever the body binds.
#[test]
fn a_loop_value_is_read_as_code_generation_declares_it() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("an unused `let n: u32` in a closed `if`", "fn main() { let n: i64 = 4294967297; let v = loop { if false { let n: u32 = 1; } break n; }; let r: &i64 = &v; print_int(*r); }\n", "RUN: 4294967297\n"),
        ("a `break` before the body's own `let n`", "fn main() { let n: i64 = 4294967297; let v = loop { if true { break n; } let n: i64 = 1; break n; }; let r: &i64 = &v; print_int(*r); }\n", "RUN: 4294967297\n"),
        ("a `match` arm's `let n: u32`", "fn main() { let n: i64 = 4294967297; let v = loop { match 1 { 1 => { let n: u32 = 3; print_int(n); } _ => {} } break n; }; let r: &i64 = &v; print_int(*r); }\n", "RUN: 3\n4294967297\n"),
        ("a `while` body's `let n: u32`", "fn main() { let n: i64 = 4294967297; let v = loop { let mut i = 0; while i < 1 { let n: u32 = 3; print_int(n); i = i + 1; } break n; }; let r: &i64 = &v; print_int(*r); }\n", "RUN: 3\n4294967297\n"),
        ("an `unsafe` block's `let n: u32`", "fn main() { let n: i64 = 4294967297; let v = loop { unsafe { let n: u32 = 1; print_int(n); } break n; }; let r: &i64 = &v; print_int(*r); }\n", "RUN: 1\n4294967297\n"),
        ("a `for` variable of the same name", "fn main() { let x: i64 = 4294967297; let v = loop { for x in 0..1 { print_int(x); } break x; }; let r: &i64 = &v; print_int(*r); }\n", "RUN: 0\n4294967297\n"),
        ("the body's own `let n: u32` in scope at the `break`: `v` is the outer `i64`", "fn main() { let n: i64 = 4294967297; let v = loop { let n: u32 = 7; break n; }; let r: &i64 = &v; print_int(*r); }\n", "RUN: 7\n"),
        ("the same inside an `if`", "fn main() { let n: i64 = 4294967297; let v = loop { if true { let n: u32 = 9; break n; } break n; }; let r: &i64 = &v; print_int(*r); }\n", "RUN: 9\n"),
        ("a closed `let n: u32`, for `&u32`", "fn main() { let n: i64 = 4294967297; let v = loop { if false { let n: u32 = 1; } break n; }; let r: &u32 = &v; print_int(*r); }\n", "REFUSED[PD0022]"),
        ("an open `let n: u32`, for `&u32`: still the outer `i64`", "fn main() { let n: i64 = 4294967297; let v = loop { let n: u32 = 7; break n; }; let r: &u32 = &v; print_int(*r); }\n", "REFUSED[PD0022]"),
    ];
    check_table(&rows, "ref_loop_value");
}

/// REVIEW ROUND 2: AN UNRELATED REFUSAL IN A GENERIC BODY HIDES NOTHING. A
/// statement of an instantiated body refused for a reason the reference rules
/// do not own (`let n: i64 = true`, `if 1`, `return 7` at an alias of `u32`)
/// was skipped WHOLE, so the branches, arms, blocks and later statements inside
/// it were never checked: `r == s` over two borrowed fives ran and printed `ne`
/// on main and on round 2. Each part is now walked in its own scope.
#[test]
fn an_unrelated_refusal_in_a_generic_body_hides_no_reference_rule() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("`return 7` at an alias of `u32`, then the other branch", "type N = u32;\nfn f<T>(x: T) -> N { if false { return 7; } else { let a = 5; let b = 5; let r = &a; let s = &b; if r == s { print(\"eq\"); } else { print(\"ne\"); } } 7 }\nfn main() { print_int(f(1)); }\n", "REFUSED[PD0083]: in `f` instantiated at `i64`"),
        ("the other branch of an `if`", "fn same<T>(a: T, b: T, c: bool) -> bool { if c { let n: i64 = true; print_int(n); } else { let r = &a; let s = &b; return r == s; } false }\nfn main() { if same(5, 5, false) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]: in `same` instantiated at `i64`"),
        ("a block value after the refused `let`", "fn same<T>(a: T, b: T) -> bool { let k = { let n: i64 = true; let r = &a; let s = &b; r == s }; k }\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a `match` arm", "fn same<T>(a: T, b: T) -> bool { match true { true => { let n: i64 = true; let r = &a; let s = &b; return r == s; } false => { return false; } } }\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a `for` body", "fn same<T>(a: T, b: T) -> bool { for i in 0..1 { let n: i64 = true; let r = &a; let s = &b; return r == s; } false }\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a `while` whose condition is refused", "fn same<T>(a: T, b: T) -> bool { while 1 { let r = &a; let s = &b; return r == s; } false }\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a value `if` whose condition is refused", "fn same<T>(a: T, b: T) -> bool { let v = if 1 { let r = &a; let s = &b; r == s } else { false }; v }\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("an `else if` after a refused branch", "fn same<T>(a: T, b: T, c: i64) -> bool { if c == 0 { return 1; } else if c == 1 { let r = &a; let s = &b; return r == s; } false }\nfn main() { if same(5, 5, 1) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a width mismatch after a refused `let`", "fn f<T>(n: T) -> i64 { if true { let k: i64 = true; let a: i64 = 4294967297; let r: &u32 = &a; return *r; } 0 }\nfn main() { print_int(f(5)); }\n", "REFUSED[PD0022]"),
        // `&T` in a body that is not substituted is C `void **`: a borrow of a
        // concrete parameter meets it at no instantiation (an internal compiler
        // error on main); a borrow of a `let x: T` (`void *`) meets it, and runs.
        ("a `&T` annotation over a borrow of a concrete parameter", "fn g<T>(a: T) -> i64 { let r: &T = &a; 7 }\nfn main() { print_int(g(5)); }\n", "REFUSED[PD0022]: in `g` instantiated at `i64`"),
        ("a `&T` annotation over a borrow of a `let x: T`", "fn g<T>(a: T) -> T { let x: T = a; let r: &T = &x; *r }\nfn main() { print(g(\"ok\")); }\n", "RUN: ok\n"),
        ("an unrelated refusal alone still runs", "type N = u32;\nfn f<T>(x: T) -> N { if false { return 7; } else { print(\"x\"); } 7 }\nfn main() { print_int(f(1)); }\n", "RUN: x\n7\n"),
        ("a valid borrow after an unrelated refusal still runs", "fn f<T>(x: T) -> i64 { let k: i64 = true; let m: i64 = 4294967297; let r = &m; *r }\nfn main() { print_int(f(1)); }\n", "RUN: 4294967297\n"),
    ];
    check_table(&rows, "ref_generic_walk");
    let module = [("zskip.pd", "pub fn same<T>(a: T, b: T) -> bool { if true { let n: i64 = true; let r = &a; let s = &b; return r == s; } false }\n")];
    let got = outcome_with(&module, "import zskip;\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "ref_generic_walk_import");
    assert!(got.starts_with("REFUSED[PD0083]: in `same` instantiated at `i64`"), "an imported generic body\n    got: {}", got);
}

/// REVIEW ROUND 2: WHAT A REFUSAL IS ABOUT IS READ FROM ITS CODE AND THE TYPES
/// IT JUDGED, NEVER FROM ITS TEXT. The walk of a generic body kept a refusal
/// whose message held a word naming a reference alias, so an unused `type Type
/// = &i64;` turned the unrelated `Type mismatch: expected N, found Int` into a
/// refusal of a program main ran. Its twin with an alias that IS used is still
/// refused, by the reference rule it breaks.
#[test]
fn a_refusal_is_classified_by_its_types_not_its_words() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("an unused alias `Type`", "type Type = &i64;\ntype N = u32;\nfn f<T>(x: T) -> N { 7 }\nfn main() { print_int(f(1)); }\n", "RUN: 7\n"),
        ("an unused alias `expected`", "type expected = &i64;\ntype N = u32;\nfn f<T>(x: T) -> N { 7 }\nfn main() { print_int(f(1)); }\n", "RUN: 7\n"),
        ("an unused alias `found`", "type found = &i64;\ntype N = u32;\nfn f<T>(x: T) -> N { 7 }\nfn main() { print_int(f(1)); }\n", "RUN: 7\n"),
        ("an unused alias `mismatch`", "type mismatch = &i64;\ntype N = u32;\nfn f<T>(x: T) -> N { 7 }\nfn main() { print_int(f(1)); }\n", "RUN: 7\n"),
        ("an unused alias `Int`", "type Int = &i64;\ntype N = u32;\nfn f<T>(x: T) -> N { 7 }\nfn main() { print_int(f(1)); }\n", "RUN: 7\n"),
        ("the alias `Type` used for two borrows compared", "type Type = &i64;\nfn f<T>(x: T) -> i64 { let m: i64 = 4294967297; let r: Type = &m; let s: Type = &m; if r == s { 1 } else { 0 } }\nfn main() { print_int(f(1)); }\n", "REFUSED[PD0083]: in `f` instantiated at `i64`"),
    ];
    check_table(&rows, "ref_refusal_kind");
}

/// REVIEW ROUND 3: A WALKED EXPRESSION KEEPS ITS TYPE. Where the walk of a
/// generic body met an expression refused for an unrelated reason, it walked
/// the parts and answered no type, so `let r = { if false { return 7; } &a };`
/// bound nothing and the `r == s` after it went unjudged: two borrowed fives
/// compared by address, `ne` on main and on round 3. The type is recovered
/// from the parts — a block's value, a branch, an arm, a `loop`'s `break`, a
/// tuple's elements — and the binding is made.
#[test]
fn a_walked_expression_keeps_its_type() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("a value block refused for an unrelated `return 7`", "type N = u32;\nfn f<T>(a: T, b: T) -> N { let r = { if false { return 7; } &a }; let s = &b; if r == s { print(\"eq\"); } else { print(\"ne\"); } 7 }\nfn main() { print_int(f(5, 5)); }\n", "REFUSED[PD0083]: in `f` instantiated at `i64`"),
        ("a value block refused for its `let`", "fn same<T>(a: T, b: T) -> bool { let r = { let k: i64 = true; &a }; let s = &b; r == s }\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a value `if`", "fn same<T>(a: T, b: T) -> bool { let r = if true { let k: i64 = true; &a } else { &b }; let s = &b; r == s }\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a value `match`", "fn same<T>(a: T, b: T) -> bool { let r = match true { true => { let k: i64 = true; &a } false => &b }; let s = &b; r == s }\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a value `loop`", "fn same<T>(a: T, b: T) -> bool { let r = loop { let k: i64 = true; break &a; }; let s = &b; r == s }\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a tuple element", "type N = u32;\nfn f<T>(a: T, b: T) -> N { let pair = ({ if false { return 7; } &a }, &b); if pair.0 == pair.1 { print(\"eq\"); } else { print(\"ne\"); } 7 }\nfn main() { print_int(f(5, 5)); }\n", "REFUSED[PD0083]"),
        ("a recovered value meets its annotation", "fn f<T>(x: T) -> i64 { let r: &i64 = { let k: i64 = true; 7 }; 7 }\nfn main() { print_int(f(1)); }\n", "REFUSED[PD0022]"),
        ("a recovered value meets the return type", "fn f<T>(x: T) -> &i64 { { let k: i64 = true; 7 } }\nfn main() { let r = f(1); print_int(*r); }\n", "REFUSED[-]: in `f` instantiated at `i64`"),
        ("a recovered branch meets the other branch", "fn f<T>(x: T) -> i64 { let m = 5; let z = if true { let k: i64 = true; &m } else { 0 }; print_int(z); 7 }\nfn main() { print_int(f(1)); }\n", "REFUSED[PD0058]"),
        ("a recovered element meets the next element", "fn f<T>(x: T) -> i64 { let m = 5; let a = [{ let k: i64 = true; 1 }, &m]; 7 }\nfn main() { print_int(f(1)); }\n", "REFUSED[-]: in `f` instantiated at `i64`"),
        ("the same blocks, no reference compared: still runs", "fn f<T>(x: T) -> i64 { let n: i64 = 4294967297; let r = if true { let k: i64 = true; &n } else { &n }; *r }\nfn main() { print_int(f(1)); }\n", "RUN: 4294967297\n"),
    ];
    check_table(&rows, "ref_kept_type");
    let module = [("zlost.pd", "pub fn same<T>(a: T, b: T) -> bool { let r = { let k: i64 = true; &a }; let s = &b; r == s }\n")];
    let got = outcome_with(&module, "import zlost;\nfn main() { if same(5, 5) { print(\"eq\"); } else { print(\"ne\"); } }\n", "ref_kept_type_import");
    assert!(got.starts_with("REFUSED[PD0083]: in `same` instantiated at `i64`"), "an imported generic body\n    got: {}", got);
}

/// REVIEW ROUND 3: EVERY ACTUAL MEETS ITS OWN EXPECTED TYPE. Where a
/// construct was refused for an unrelated reason, its comparisons with
/// declared types were not asked: an integer reached a field or payload
/// declared `&i64`, and a `&i64` reached a method's, a path call's or a
/// generic's parameter declared `&u32` or `i64` — gcc or the ill-typed-C trap
/// on main and on round 3. Each pair is asked again.
#[test]
fn every_actual_meets_its_own_expected_type() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("an integer for a struct field declared `&i64`", "type A = &i64;\nstruct H { p: A }\nfn f<T>(x: T) -> i64 { let h = H { p: 5 }; 7 }\nfn main() { print_int(f(1)); }\n", "REFUSED[-]: in `f` instantiated at `i64`: Type mismatch: expected &i64, found Int"),
        ("the same after an unrelated `return 7`", "type A = &i64;\ntype N = u32;\nstruct H { p: A }\nfn f<T>(x: T) -> N { if false { return 7; } let h = H { p: 5 }; 7 }\nfn main() { print_int(f(1)); }\n", "REFUSED[-]: in `f` instantiated at `i64`: Type mismatch: expected &i64, found Int"),
        ("an integer for an enum payload declared `&i64`", "type A = &i64;\nenum E { A(A), B }\nfn f<T>(x: T) -> i64 { let e = E::A(5); 7 }\nfn main() { print_int(f(1)); }\n", "REFUSED[-]: in `f` instantiated at `i64`: Type mismatch: expected &i64, found Int"),
        ("a `&i64` for a method's `&u32`, beside a refused argument", "type N = u32;\nstruct S { n: i64 }\nimpl S { fn take(&self, n: i64, p: &u32) -> i64 { p as i64 } }\nfn f<T>(x: T) -> N { let s = S { n: 1 }; let n: i64 = 4294967297; let v = s.take({ if false { return 7; } 1 }, &n); print_int(v); 7 }\nfn main() { print_int(f(1)); }\n", "REFUSED[PD0042]: in `f` instantiated at `i64`"),
        ("a held `&i64` for a method's `i64`", "struct S { n: i64 }\nimpl S { fn use(&self, a: i64, b: i64) -> i64 { b } }\nfn f<T>(x: T) -> i64 { let s = S { n: 1 }; let m = 5; let r = &m; s.use({ let k: i64 = true; 1 }, r) }\nfn main() { print_int(f(1)); }\n", "REFUSED[PD0042]"),
        ("a `&i64` for a path call's `&u32`", "struct S { n: i64 }\nimpl S { fn take(n: i64, p: &u32) -> i64 { p as i64 } }\nfn f<T>(x: T) -> i64 { let n: i64 = 4294967297; S::take({ let k: i64 = true; 1 }, &n) }\nfn main() { print_int(f(1)); }\n", "REFUSED[PD0042]"),
        ("a held `&i64` for a generic's concrete `i64`", "fn take<T>(a: T, b: i64) -> i64 { b }\nfn f<U>(x: U) -> i64 { let m = 5; let r = &m; take({ let k: i64 = true; 1 }, r) }\nfn main() { print_int(f(1)); print_int(take(1, 1)); }\n", "REFUSED[-]: in `f` instantiated at `i64`: Type mismatch: expected Int, found &i64"),
        ("a generic instantiated at `&i64`, beside an unrelated argument", "fn pair<T>(x: T, k: i64) -> T { x }\nfn g<T>(a: T) -> i64 { let r = &a; let s = pair(r, true); *s }\nfn main() { print_int(g(5)); }\n", "REFUSED[-]: in `g` instantiated at `i64`: a generic function instantiated at `&i64`"),
        ("an integer `0` for a field declared `&i64` (C's null pointer)", "type A = &i64;\nstruct H { p: A }\nfn f<T>(x: T) -> i64 { let h = H { p: 0 }; 7 }\nfn main() { print_int(f(1)); }\n", "REFUSED[-]: in `f` instantiated at `i64`: Type mismatch: expected &i64, found Int"),
    ];
    check_table(&rows, "ref_every_pair");
}

/// REVIEW ROUND 3: A REFUSAL IS JUDGED BY ITS OWN PAIR, NEVER BY ITS
/// SIBLINGS. A construct refused for an unrelated reason was judged by the
/// types of ALL its parts, so a correct reference argument or field beside a
/// `true` for an `i64` made the `true` a refusal of the reference rules:
/// each of these ran on main and printed this, and round 3 refused it.
#[test]
fn a_refusal_is_judged_by_its_own_pair() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("a method: `true` for `i64` beside a correct `&i64`", "type A = &i64;\nstruct S { n: i64 }\nimpl S { fn add(&self, k: i64, p: A) -> i64 { *p + 2 } }\nfn f<T>(x: T) -> i64 { let a = 5; let s = S { n: 1 }; s.add(true, &a) }\nfn main() { print_int(f(1)); }\n", "RUN: 7\n"),
        ("a method: an integer for `N = u32` beside a correct `&i64`", "type A = &i64;\ntype N = u32;\nstruct S { n: i64 }\nimpl S { fn take(&self, n: N, p: A) -> i64 { *p } }\nfn f<T>(x: T) -> i64 { let n = 5; let s = S { n: 1 }; s.take(7, &n) }\nfn main() { print_int(f(1)); }\n", "RUN: 5\n"),
        ("a path call", "type A = &i64;\nstruct S { n: i64 }\nimpl S { fn take(n: i64, p: A) -> i64 { *p } }\nfn f<T>(x: T) -> i64 { let n = 5; S::take(true, &n) }\nfn main() { print_int(f(1)); }\n", "RUN: 5\n"),
        ("a struct literal", "type A = &i64;\nstruct H { n: i64, p: A }\nfn f<T>(x: T) -> i64 { let m = 5; let h = H { n: true, p: &m }; *h.p }\nfn main() { print_int(f(1)); }\n", "RUN: 5\n"),
        ("an enum payload", "type A = &i64;\nenum E { A(i64, A), B }\nfn f<T>(x: T) -> i64 { let m = 5; let e = E::A(true, &m); match e { E::A(n, r) => *r, E::B => 0 } }\nfn main() { print_int(f(1)); }\n", "RUN: 5\n"),
        ("a free function", "type A = &i64;\nfn take(n: i64, p: A) -> i64 { *p }\nfn f<T>(x: T) -> i64 { let n = 5; take(true, &n) }\nfn main() { print_int(f(1)); }\n", "RUN: 5\n"),
    ];
    check_table(&rows, "ref_own_pair");
}

/// REVIEW ROUND 4: A RECOVERED TYPE IS EXACTLY THE CHECKER'S, OR UNKNOWN. The
/// walk typed `1 + 2.5` by its LEFT operand — an `Int` — where the checker
/// gives mixed operands no type and code generation declares `double`, so
/// `let r: &f64 = &v` was refused though main ran it; a binding of an unknown
/// type is made at the C type code generation declares. And it answered no
/// type for a generic call returning `T` or a generic struct literal, so two
/// borrows compared through them went unjudged (`ne`): the instantiation is
/// substituted. A `&T` inside an annotation's array or tuple is `void **` as
/// one at the top is.
#[test]
fn a_recovered_type_is_the_checkers_or_unknown() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("`1 + 2.5`, borrowed as `&f64`", "fn f<T>(a: T) -> i64 { let v = 1 + 2.5; let r: &f64 = &v; ((*r) * 2.0) as i64 }\nfn main() { print_int(f(1)); }\n", "RUN: 7\n"),
        ("`true + 4`, borrowed as `&i64`", "fn f<T>(a: T) -> i64 { let v = true + 4; let r: &i64 = &v; *r }\nfn main() { print_int(f(1)); }\n", "RUN: 5\n"),
        ("`1 + 2.5` as a recovered block's value", "type N = u32;\nfn f<T>(a: T) -> N { let v = { if false { return 7; } 1 + 2.5 }; let r: &f64 = &v; print_int(((*r) * 2.0) as i64); 7 }\nfn main() { print_int(f(1)); }\n", "RUN: 7\n7\n"),
        ("two recovered `id(..)` of reference aliases, compared", "type A = &i64;\ntype N = u32;\nfn id<T>(a: T) -> T { a }\nfn f<U>(x: U, p: A, q: A) -> N { if id({ if false { return 7; } p }) == id({ if false { return 7; } q }) { print(\"eq\"); } else { print(\"ne\"); } 7 }\nfn main() { let n = 5; let m = 5; let p: A = &n; let q: A = &m; let seed = id(p); print_int(f(1, p, q)); }\n", "REFUSED[PD0083]: in `f` instantiated at `i64`"),
        ("a recovered `id(..)` for `print_int`", "type A = &i64;\nfn id<T>(a: T) -> T { a }\nfn f<U>(x: U, p: A) -> i64 { print_int(id({ let k: i64 = true; p })); 7 }\nfn main() { let n = 5; let p: A = &n; let seed = id(p); print_int(f(1, p)); }\n", "REFUSED[PD0042]"),
        ("two recovered generic struct literals' fields, compared", "type A = &i64;\nstruct G<T> { tag: i64, p: T }\nfn f<U>(x: U, p: A, q: A) -> bool { G { tag: { let k: i64 = true; 1 }, p: p }.p == G { tag: { let k: i64 = true; 1 }, p: q }.p }\nfn main() { let n = 5; let m = 5; let p: A = &n; let q: A = &m; let seed = G { tag: 1, p: p }; if f(1, p, q) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("`[&T; 1]` over a borrow of a concrete parameter", "fn f<T>(a: T) -> i64 { let r: [&T; 1] = [&a]; 7 }\nfn main() { print_int(f(5)); }\n", "REFUSED[PD0022]: in `f` instantiated at `i64`"),
        ("`(&T, i64)` over a borrow of a concrete parameter", "fn f<T>(a: T) -> i64 { let r: (&T, i64) = (&a, 7); 7 }\nfn main() { print_int(f(5)); }\n", "REFUSED[PD0022]"),
        ("`[&T; 1]` over a borrow of a `let x: T`", "fn f<T>(a: T) -> i64 { let x: T = a; let r: [&T; 1] = [&x]; 7 }\nfn main() { print_int(f(\"ok\")); }\n", "RUN: 7\n"),
    ];
    check_table(&rows, "ref_exact_recovery");
    let module = [("znum.pd", "pub fn f<T>(a: T) -> i64 { let v = 1 + 2.5; let r: &f64 = &v; ((*r) * 2.0) as i64 }\n")];
    let got = outcome_with(&module, "import znum;\nfn main() { print_int(f(1)); }\n", "ref_exact_recovery_import");
    assert!(got.starts_with("RUN: 7\n"), "an imported generic body\n    got: {}", got);
}

/// REVIEW ROUND 5. (1) AN INFERENCE THE CHECKER CANNOT MAKE IS NO REFUSAL OF
/// THE REFERENCE RULES: `id({ .. p })` is "Cannot infer type from complex
/// expression", and judged by the reference it was given it refused
/// `*id({ .. p })`, which main ran — the parameters are fixed by the arguments
/// instead. (2) A BORROW IS A REFERENCE WHATEVER IT BORROWS: `&x` over an `x`
/// the walk could not type was typeless, and `&x == &y` compared addresses.
/// (3) A GENERIC CALL'S ARGUMENTS MEET THE INSTANTIATED PARAMETERS: a
/// parameter naming `T` was only inferred from, so `take(1, &n)` gave a `&u32`
/// for `&T` at `T = i64`. (4) BEHIND A REFERENCE NOTHING CONVERTS: `& &T` was
/// asked at its top layer only, and `void ***` met a `long long **`. Each
/// refused row reached gcc, the ill-typed-C trap or a wrong answer on main
/// (2ca5eb5) and on round 5; each running row runs on both.
#[test]
fn an_inference_limit_an_unknown_referent_an_instantiated_parameter_and_a_nested_reference() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("`*id({ if false { return 7; } p })` (main 5)", "type A = &i64;\nfn id<T>(a: T) -> T { a }\nfn f<U>(x: U, p: A) -> i64 { let s = id({ if false { return 7; } p }); *s }\nfn main() { let n = 5; let p: A = &n; let seed = id(p); print_int(f(1, p)); }\n", "RUN: 5\n"),
        ("`*id({ print(\"x\"); p })` (main x, 5)", "type A = &i64;\nfn id<T>(a: T) -> T { a }\nfn f<U>(x: U, p: A) -> i64 { let s = id({ print(\"x\"); p }); *s }\nfn main() { let n = 5; let p: A = &n; let seed = id(p); print_int(f(1, p)); }\n", "RUN: x\n5\n"),
        ("`id({ .. 5 })`, no reference (main 5)", "fn id<T>(a: T) -> T { a }\nfn f<U>(x: U) -> i64 { let s = id({ if false { return 7; } 5 }); s }\nfn main() { let seed = id(1); print_int(f(1)); }\n", "RUN: 5\n"),
        ("`&x == &y` over two characters read from strings", "fn f<T>(a: T) -> bool { let s = \"a\"; let t = \"a\"; let x = s[0]; let y = t[0]; &x == &y }\nfn main() { if f(1) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]: in `f` instantiated at `i64`: `&x` is a reference"),
        ("a reference `if` beside a branch of no known type, compared", "type A = &i64;\nfn f<T>(x: T, p: A, q: A) -> bool { (if true { p } else { 0 + false }) == (if true { q } else { 0 + false }) }\nfn main() { let n = 5; let m = 5; let p: A = &n; let q: A = &m; if f(1, p, q) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]"),
        ("a value of no known type cast to a reference alias", "type A = &i64;\nfn f<T>(x: T, p: A, q: A) -> bool { ((if true { p } else { 0 + false }) as A) == ((if true { q } else { 0 + false }) as A) }\nfn main() { let n = 5; let m = 5; let p: A = &n; let q: A = &m; if f(1, p, q) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0003]"),
        ("a borrowed string character, dereferenced (main 97)", "fn f<T>(a: T) -> i64 { let s = \"a\"; let x = s[0]; let r = &x; (*r) as i64 }\nfn main() { print_int(f(1)); }\n", "RUN: 97\n"),
        ("a reference read out of a tuple beside an element of no known type", "fn f<T>(a: T) -> bool { let m = 5; let n = 5; let s = \"a\"; let t = (s[0], &m); let u = (s[0], &n); t.1 == u.1 }\nfn main() { if f(1) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]: in `f` instantiated at `i64`: `t.1` is a reference"),
        ("the same tuple, read by value (main 102)", "fn f<T>(a: T) -> i64 { let m = 5; let s = \"a\"; let t = (s[0], &m); *t.1 + (t.0 as i64) }\nfn main() { print_int(f(1)); }\n", "RUN: 102\n"),
        ("two borrowed string characters, compared by value (main eq)", "fn f<T>(a: T) -> bool { let s = \"a\"; let t = \"a\"; let x = s[0]; let y = t[0]; let r = &x; let q = &y; *r == *q }\nfn main() { if f(1) { print(\"eq\"); } else { print(\"ne\"); } }\n", "RUN: eq\n"),
        ("a `&u32` for a generic's `&T` at `T = i64`", "fn take<T>(a: T, b: &T) -> i64 { 7 }\nfn f<U>(x: U) -> i64 { let n: u32 = 5; take(1, &n) }\nfn main() { let n: i64 = 5; print_int(take(1, &n)); print_int(f(1)); }\n", "REFUSED[-]: in `f` instantiated at `i64`: Type mismatch: expected &i64, found &u32"),
        ("the same beside a refused argument", "fn take<T>(a: T, b: &T) -> i64 { 7 }\nfn f<U>(x: U) -> i64 { let n: u32 = 5; take({ let k: i64 = true; 1 }, &n) }\nfn main() { let n: i64 = 5; print_int(take(1, &n)); print_int(f(1)); }\n", "REFUSED[-]: in `f` instantiated at `i64`: Type mismatch: expected &i64, found &u32"),
        ("a held reference for a generic's `&T`, beside a refused argument", "fn take<T>(a: T, b: &T) -> i64 { 7 }\nfn f<U>(x: U) -> i64 { let n = 5; let r = &n; take({ let k: i64 = true; 1 }, r) }\nfn main() { let n = 5; print_int(take(1, &n)); print_int(f(1)); }\n", "REFUSED[PD0084]: in `f` instantiated at `i64`"),
        ("`T` fixed at an integer and at a reference alias", "type A = &i64;\nfn take<T>(a: T, b: T) -> i64 { 7 }\nfn f<U>(x: U, p: A) -> i64 { take({ let k: i64 = true; 1 }, p) }\nfn main() { let n = 5; let p: A = &n; print_int(take(1, 1)); print_int(f(1, p)); }\n", "REFUSED[-]: in `f` instantiated at `i64`: Type parameter 'T' has conflicting types"),
        ("a `&i64` for a generic's `&T` at `T = i64` (main 7, 7)", "fn take<T>(a: T, b: &T) -> i64 { 7 }\nfn f<U>(x: U) -> i64 { let n: i64 = 5; take(1, &n) }\nfn main() { let n: i64 = 5; print_int(take(1, &n)); print_int(f(1)); }\n", "RUN: 7\n7\n"),
        ("`let r: & &T = &p` over `p = &a` at `T = i64`", "fn f<T>(a: T) -> i64 { let p = &a; let r: & &T = &p; 7 }\nfn main() { print_int(f(5)); }\n", "REFUSED[PD0022]: in `f` instantiated at `i64`: Type mismatch: expected &&T, found &&i64"),
        ("`let r: &(&T, i64) = &p` over `p = (&a, 7)`", "fn f<T>(a: T) -> i64 { let p = (&a, 7); let r: &(&T, i64) = &p; 7 }\nfn main() { print_int(f(5)); }\n", "REFUSED[PD0022]"),
        ("`let r: &[&T; 1] = &p` over `p = [&a]`", "fn f<T>(a: T) -> i64 { let p = [&a]; let r: &[&T; 1] = &p; 7 }\nfn main() { print_int(f(5)); }\n", "REFUSED[PD0022]"),
        ("`let r: & &T = &p` over `p = &x`, `x: T` (main 7)", "fn f<T>(a: T) -> i64 { let x: T = a; let p = &x; let r: & &T = &p; 7 }\nfn main() { print_int(f(\"ok\")); }\n", "RUN: 7\n"),
        ("`let r: & &i64 = &p` over `p = &a` (main 7)", "fn f<T>(a: T) -> i64 { let p = &a; let r: & &i64 = &p; 7 }\nfn main() { print_int(f(5)); }\n", "RUN: 7\n"),
    ];
    check_table(&rows, "ref_round5");
    let chars = [("zchar.pd", "pub fn f<T>(a: T) -> bool { let s = \"a\"; let t = \"a\"; let x = s[0]; let y = t[0]; &x == &y }\n")];
    let got = outcome_with(&chars, "import zchar;\nfn main() { if f(1) { print(\"eq\"); } else { print(\"ne\"); } }\n", "ref_round5_import_chars");
    let nested = [("zdeep.pd", "pub fn f<T>(a: T) -> i64 { let p = &a; let r: & &T = &p; 7 }\n")];
    let deep = outcome_with(&nested, "import zdeep;\nfn main() { print_int(f(5)); }\n", "ref_round5_import_deep");
    let wrong: Vec<String> = [
        ("an imported body's `&x == &y`", got, "REFUSED[PD0083]"),
        ("an imported body's `& &T`", deep, "REFUSED[PD0022]"),
    ]
    .into_iter()
    .filter(|(_, got, expected)| !got.starts_with(expected))
    .map(|(what, got, expected)| format!("{}\n    expected: {}\n    got:      {}", what, expected, got))
    .collect();
    assert!(wrong.is_empty(), "{} imported row(s) differ:\n{}", wrong.len(), wrong.join("\n"));
}

/// REVIEW ROUND 6. (1) A BRANCH NEVER TYPED ADDS NOTHING TO A KNOWN ONE: the
/// walk made `if true { p } else { 0 + false }` a reference of an unknown
/// referent, so `let r: &i64 = ..` was refused though main ran it, and
/// `if true { &p } ..` lost the reference below its `&` — `*rp == *rq`
/// compared two addresses. Code generation declares such a value by its first
/// part, and so does the walk now. (2) TWO UNKNOWN REFERENTS ARE NOT ONE: a
/// character read out of a string is a C `char`, and a reference to it agrees
/// with another such and nothing else. (3) A COMPOSITE IS JUDGED POSITION BY
/// POSITION: an unknown element excused `z = (c, p)` over `z: (i64, i64)`.
/// Each refused row reached gcc, the ill-typed-C trap or a wrong answer on main
/// (2ca5eb5) and on round 6, or was refused by round 6 where main ran it.
#[test]
fn a_part_never_typed_adds_nothing_and_unknown_referents_are_not_one() {
    let rows: Vec<(&str, &str, &str)> = vec![
        ("`let r: &i64 = if true { p } else { 0 + false }` (main 5)", "type A = &i64;\nfn f<T>(x: T, p: A) -> i64 { let r: &i64 = if true { p } else { 0 + false }; *r }\nfn main() { let n = 5; let p: A = &n; print_int(f(1, p)); }\n", "RUN: 5\n"),
        ("the same value given to a parameter declared `A` (main 5)", "type A = &i64;\nfn take(p: A) -> i64 { *p }\nfn f<T>(x: T, p: A) -> i64 { let q = if true { p } else { 0 + false }; take(q) }\nfn main() { let n = 5; let p: A = &n; print_int(f(1, p)); }\n", "RUN: 5\n"),
        ("the same value stored in a field declared `A` (main 5)", "type A = &i64;\nstruct H { p: A }\nfn f<T>(x: T, p: A) -> i64 { let q = if true { p } else { 0 + false }; let h = H { p: q }; *h.p }\nfn main() { let n = 5; let p: A = &n; print_int(f(1, p)); }\n", "RUN: 5\n"),
        ("`*rp == *rq` over `rp = if true { &p } ..`", "type A = &i64;\nfn f<T>(x: T, p: A, q: A) -> bool { let rp = if true { &p } else { 0 + false }; let rq = if true { &q } else { 0 + false }; *rp == *rq }\nfn main() { let n = 5; let m = 5; let p: A = &n; let q: A = &m; if f(1, p, q) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]: in `f` instantiated at `i64`: `*rp` is a reference"),
        ("the same through two bindings", "type A = &i64;\nfn f<T>(x: T, p: A, q: A) -> bool { let rp = if true { &p } else { 0 + false }; let rq = if true { &q } else { 0 + false }; let p2 = *rp; let q2 = *rq; p2 == q2 }\nfn main() { let n = 5; let m = 5; let p: A = &n; let q: A = &m; if f(1, p, q) { print(\"eq\"); } else { print(\"ne\"); } }\n", "REFUSED[PD0083]: in `f` instantiated at `i64`: `p2` is a reference"),
        ("`**rp == **rq`, the values (main eq)", "type A = &i64;\nfn f<T>(x: T, p: A, q: A) -> bool { let rp = if true { &p } else { 0 + false }; let rq = if true { &q } else { 0 + false }; **rp == **rq }\nfn main() { let n = 5; let m = 5; let p: A = &n; let q: A = &m; if f(1, p, q) { print(\"eq\"); } else { print(\"ne\"); } }\n", "RUN: eq\n"),
        ("a `char *` borrow assigned a `long long *`", "type A = &i64;\nfn f<T>(x: T, p: A) -> i64 { let s = \"a\"; let c = s[0]; let mut r = &c; let q = if true { p } else { 0 + false }; r = q; 7 }\nfn main() { let n = 5; let p: A = &n; print_int(f(1, p)); }\n", "REFUSED[-]: in `f` instantiated at `i64`: Type mismatch: expected &C char, found A"),
        ("the two merged in a value `if`", "type A = &i64;\nfn f<T>(x: T, p: A) -> i64 { let s = \"a\"; let c = s[0]; let r = &c; let q = if true { p } else { 0 + false }; let z = if true { r } else { q }; 7 }\nfn main() { let n = 5; let p: A = &n; print_int(f(1, p)); }\n", "REFUSED[PD0058]"),
        ("two `char *` borrows, one assigned the other (main 98)", "fn f<T>(x: T) -> i64 { let s = \"a\"; let t = \"b\"; let c = s[0]; let d = t[0]; let mut r = &c; let q = &d; r = q; (*r) as i64 }\nfn main() { print_int(f(1)); }\n", "RUN: 98\n"),
        ("two `char *` borrows merged in a value `if` (main 97)", "fn f<T>(x: T) -> i64 { let s = \"a\"; let t = \"b\"; let c = s[0]; let d = t[0]; let r = if true { &c } else { &d }; (*r) as i64 }\nfn main() { print_int(f(1)); }\n", "RUN: 97\n"),
        ("`z = (c, p)` over `z: (i64, i64)`, `c` a string character", "type A = &i64;\nfn f<T>(x: T, p: A) -> i64 { let s = \"a\"; let c = s[0]; let mut z = (1, 2); z = (c, p); 7 }\nfn main() { let n = 5; let p: A = &n; print_int(f(1, p)); }\n", "REFUSED[-]: in `f` instantiated at `i64`: Type mismatch: expected (Int, Int), found (C char, A)"),
        ("`let z: (char, i64) = (c, p)`", "type A = &i64;\nfn f<T>(x: T, p: A) -> i64 { let s = \"a\"; let c = s[0]; let v = (c, p); let z: (char, i64) = v; 7 }\nfn main() { let n = 5; let p: A = &n; print_int(f(1, p)); }\n", "REFUSED[PD0022]"),
        ("`*z` over the tuple `(c, p)`", "type A = &i64;\nfn f<T>(x: T, p: A) -> i64 { let s = \"a\"; let c = s[0]; let z = (c, p); let y = *z; 7 }\nfn main() { let n = 5; let p: A = &n; print_int(f(1, p)); }\n", "REFUSED[PD0082]"),
    ];
    check_table(&rows, "ref_round6");
    let module = [("zu.pd", "pub fn f<T>(x: T) -> bool { let a = 5; let b = 5; let p = &a; let q = &b; let rp = if true { &p } else { 0 + false }; let rq = if true { &q } else { 0 + false }; *rp == *rq }\n")];
    let got = outcome_with(&module, "import zu;\nfn main() { if f(1) { print(\"eq\"); } else { print(\"ne\"); } }\n", "ref_round6_import");
    assert!(got.starts_with("REFUSED[PD0083]"), "an imported body's `*rp == *rq`\n    got: {}", got);
}
