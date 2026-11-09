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
            "REFUSED[PD0082]",
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
            "REFUSED[PD0081]",
        ),
        (
            "`&a == &b` (printed `ne`)",
            "fn main() { let a: i64 = 5; let b: i64 = 5; if &a == &b { print(\"eq\"); } else { print(\"ne\"); } }\n",
            "REFUSED[PD0081]",
        ),
        (
            "`r == 5` (printed `ne`)",
            "fn main() { let a: i64 = 5; let r = &a; if r == 5 { print(\"eq\"); } else { print(\"ne\"); } }\n",
            "REFUSED[PD0081]",
        ),
        (
            "`s.get() == t.get()` over `-> &i64` (printed `ne` for 9 == 9)",
            "struct S { n: i64 }\nimpl S { fn get(&self) -> &i64 { return &self.n; } }\nfn main() { let s = S { n: 9 }; let t = S { n: 9 }; if s.get() == t.get() { print(\"eq\"); } else { print(\"ne\"); } }\n",
            "REFUSED[PD0081]",
        ),
        (
            "`s - r` (printed `-1` for 7 - 5)",
            "fn main() { let a: i64 = 5; let b: i64 = 7; let r = &a; let s = &b; print_int(s - r); }\n",
            "REFUSED[PD0081]",
        ),
        (
            "`r < s` (compared addresses)",
            "fn main() { let a: i64 = 9; let b: i64 = 5; let r = &a; let s = &b; if r < s { print(\"lt\"); } else { print(\"ge\"); } }\n",
            "REFUSED[PD0081]",
        ),
        (
            "`r += 1` (stepped the pointer, printed garbage)",
            "fn main() { let a: i64 = 5; let mut r = &a; r += 1; print_int(*r); }\n",
            "REFUSED[PD0081]",
        ),
        (
            "`r + 1`",
            "fn main() { let m: i64 = 5; let r = &m; let k = r + 1; print_int(k); }\n",
            "REFUSED[PD0081]",
        ),
        (
            "`-r`",
            "fn main() { let a: i64 = 5; let r = &a; print_int(-r); }\n",
            "REFUSED[PD0081]",
        ),
        (
            "`!r` over a `&bool` (read the pointer's truth)",
            "fn main() { let b = true; let r = &b; if !r { print(\"t\"); } else { print(\"f\"); } }\n",
            "REFUSED[PD0081]",
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
            "REFUSED[PD0080]",
        ),
        (
            "`*x` over a parameter written `&i64`",
            "fn f(x: &i64) -> i64 { *x }\nfn main() { let m: i64 = 5; print_int(f(&m)); }\n",
            "REFUSED[PD0080]",
        ),
        (
            "`*x = *x + 1` over a parameter written `&mut i64`",
            "fn bump(x: &mut i64) { *x = *x + 1; }\nfn main() { let mut m: i64 = 5; bump(&mut m); print_int(m); }\n",
            "REFUSED[PD0080]",
        ),
        (
            "`**r` over one reference",
            "fn main() { let m: i64 = 5; let r = &m; print_int(**r); }\n",
            "REFUSED[PD0080]",
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
