//! A reference to a TUPLE as a function parameter (issue #84).
//!
//! THE DEFECT, MEASURED ON d5166e8 BEFORE THE FIX
//!
//! ```text
//! type X = (i64, i64);
//! fn h(p: &X) -> i64 { return 0; }
//! fn main() { print_int(0); }
//! -> error: Unsupported type in reference parameter
//! ```
//!
//! The reference-parameter arm of `function_signature` (`src/codegen/mod.rs`)
//! spelled a pointer for seven referents -- four integer widths, `bool`,
//! `String`, a struct or enum name -- and refused every other one. A tuple was
//! one of the others. Before W2a (cf3f6c4) the alias kept its name and the
//! program above ran; W2a expands aliases before code generation, so `&X`
//! arrives as `&(i64, i64)` and meets the refusal its alias-free twin always
//! met. A tuple is already a C struct (N4-12), so a reference to one is a
//! pointer to that struct: `const T*` for `&`, `T*` for `&mut`.
//!
//! The parameter is only worth having if it can be USED, so the receipts below
//! are the uses, each linked and run against a printed answer whose digits
//! name the element they came from: a wrong element or a lost write prints a
//! wrong number. The refusals assert that THIS compiler refused, never gcc.
//!
//! WHERE A WRITE CAN COME FROM. `place` in grammar.ebnf is `identifier |
//! place '[' expression ']' | place '.' identifier | '*' identifier`: there is
//! no tuple-element place, and `t.0 = v` is refused by the parser for every
//! tuple, a local's as much as a parameter's. A write through `&mut (A, B)` is
//! therefore `*p = (a, b)`, and that is what is supported here.

mod common;

use common::unique_module_name;
use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// `RUN: <stdout>`, `GCC: <first line>` when this compiler accepted the source
/// and the C it emitted did not build, or `REFUSED: <first line>` when this
/// compiler refused the source itself.
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
                format!("GCC: {}", first)
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
        let err = String::from_utf8_lossy(&out.stderr);
        return format!(
            "GCC: {}",
            err.lines().find(|l| l.contains("error")).unwrap_or("")
        );
    }
    let run = Command::new(&exe).output().expect("run");
    assert!(
        run.status.success(),
        "the program did not exit 0: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    format!("RUN: {}", String::from_utf8_lossy(&run.stdout))
}

/// The emitted C of `source`, for the declarator receipts.
fn emitted_c(source: &str, prefix: &str) -> String {
    let name = unique_module_name(prefix);
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    let c_file = Driver::new()
        .compile_file(&src)
        .unwrap_or_else(|e| panic!("the front end refused a legal program: {}", e));
    fs::read_to_string(c_file).unwrap()
}

/// Run every row, and compare the whole table at once so one failure shows
/// every other row's verdict beside it.
fn check(rows: &[(&str, &str, &str)], prefix: &str) {
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program, out) in rows {
        got.push(format!("{}: {}", shape, outcome(program, prefix)));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}

/// The issue's own program, and its alias-free twin, which main refused too.
#[test]
fn the_issue_program_and_its_alias_free_twin_run() {
    check(
        &[
            (
                "issue #84: `&X`, `type X = (i64, i64)`",
                "type X = (i64, i64);\nfn h(p: &X) -> i64 { return 0; }\nfn main() { print_int(0); }\n",
                "RUN: 0\n",
            ),
            (
                "the alias-free twin, `&(i64, i64)`",
                "fn h(p: &(i64, i64)) -> i64 { return 0; }\nfn main() { print_int(0); }\n",
                "RUN: 0\n",
            ),
            (
                "`&mut X`, never called",
                "type X = (i64, i64);\nfn h(p: &mut X) -> i64 { return 0; }\nfn main() { print_int(5); }\n",
                "RUN: 5\n",
            ),
        ],
        "ref_tuple_issue",
    );
}

/// `&` is a pointer to const, `&mut` a pointer: the C declarator carries the
/// permission the signature states, so a write the front end missed would be
/// gcc's error and not a silent one.
#[test]
fn a_shared_tuple_reference_is_a_pointer_to_const_and_a_mutable_one_is_not() {
    let c = emitted_c(
        "fn r(p: &(i64, i64)) -> i64 { return p.0; }\n\
         fn w(p: &mut (i64, i64)) { *p = (1, 2); }\n\
         fn main() { let mut t = (3, 4); w(&mut t); print_int(r(&t)); }\n",
        "ref_tuple_decl",
    );
    assert!(
        c.contains("long long r(const __pd_tuple2_long_long_long_long* p)"),
        "shared: {}",
        c
    );
    assert!(
        c.contains("void w(__pd_tuple2_long_long_long_long* p)"),
        "mutable: {}",
        c
    );
}

/// Reading through the reference, at every element type the brief names: a
/// nested tuple, a struct, an enum, a `String`, and mixed integer widths.
#[test]
fn a_tuple_reference_parameter_is_read_through() {
    check(
        &[
            (
                "`p.0`, `p.1`",
                "fn h(p: &(i64, i64)) -> i64 { return p.0 * 10 + p.1; }\nfn main() { let t = (3, 4); print_int(h(&t)); }\n",
                "RUN: 34\n",
            ),
            (
                "through `&mut`",
                "fn h(p: &mut (i64, i64)) -> i64 { return p.0 - p.1; }\nfn main() { let mut t = (9, 2); print_int(h(&mut t)); }\n",
                "RUN: 7\n",
            ),
            (
                "the argument without `&`",
                "fn h(p: &(i64, i64)) -> i64 { return p.0 * 10 + p.1; }\nfn main() { let t = (3, 4); print_int(h(t)); }\n",
                "RUN: 34\n",
            ),
            (
                "an alias's value given to `&X`",
                "type X = (i64, i64);\nfn h(p: &X) -> i64 { return 7; }\nfn main() { let t: X = (5, 6); print_int(h(&t)); }\n",
                "RUN: 7\n",
            ),
            (
                "two of them",
                "fn dot(a: &(i64, i64), b: &(i64, i64)) -> i64 { return a.0 * b.0 + a.1 * b.1; }\nfn main() { let x = (2, 3); let y = (5, 7); print_int(dot(&x, &y)); }\n",
                "RUN: 31\n",
            ),
            (
                "a nested tuple",
                "fn h(p: &((i64, i64), i64)) -> i64 { return (p.0).0 * 100 + (p.0).1 * 10 + p.1; }\nfn main() { let t = ((1, 2), 3); print_int(h(&t)); }\n",
                "RUN: 123\n",
            ),
            (
                "a tuple holding a struct",
                "struct P { a: i64, b: i64 }\nfn h(p: &(P, i64)) -> i64 { return p.0.a * 100 + p.0.b * 10 + p.1; }\nfn main() { let s = P { a: 4, b: 5 }; let t = (s, 6); print_int(h(&t)); }\n",
                "RUN: 456\n",
            ),
            (
                "a tuple holding an enum",
                "enum E { A(i64), B }\nfn h(p: &(E, i64)) -> i64 {\n    match p.0 {\n        E::A(n) => { return n * 10 + p.1; }\n        E::B => { return p.1; }\n    }\n}\nfn main() { let t = (E::A(7), 3); print_int(h(&t)); let u = (E::B, 5); print_int(h(&u)); }\n",
                "RUN: 73\n5\n",
            ),
            (
                "a tuple holding a String",
                "fn h(p: &(String, i64)) -> i64 { print(p.0); return p.1 + string_len(p.0); }\nfn main() { let t = (\"hello\", 4); print_int(h(&t)); }\n",
                "RUN: hello\n9\n",
            ),
            (
                "an integer, a bool and a String",
                "fn h(p: &(i64, bool, String)) { print_int(p.0); if p.1 { print(p.2); } }\nfn main() { let t = (42, true, \"yes\"); h(&t); }\n",
                "RUN: 42\nyes\n",
            ),
            (
                "elements bound by `let`",
                "fn h(p: &(i64, i64)) -> i64 { let a = p.0; let b = p.1; let q = p; return a * 100 + b * 10 + q.1; }\nfn main() { let t = (3, 4); print_int(h(&t)); }\n",
                "RUN: 344\n",
            ),
            (
                "`match` on the parameter",
                "fn h(p: &(i64, i64)) -> i64 { match p { (a, b) => { return a * 10 + b; } } }\nfn main() { let t = (3, 4); print_int(h(&t)); }\n",
                "RUN: 34\n",
            ),
            (
                "`*p` read",
                "fn h(p: &(i64, i64)) -> i64 { let q = *p; return q.0 * 10 + q.1 + (*p).0 * 1000; }\nfn main() { let t = (3, 4); print_int(h(&t)); }\n",
                "RUN: 3034\n",
            ),
            (
                "handed on by value",
                "fn v(t: (i64, i64)) -> i64 { return t.0 * 10 + t.1; }\nfn h(p: &(i64, i64)) -> i64 { return v(p); }\nfn main() { let t = (3, 4); print_int(h(&t)); }\n",
                "RUN: 34\n",
            ),
            (
                "returned by value",
                "fn h(p: &(i64, i64)) -> (i64, i64) { return p; }\nfn main() { let t = (3, 4); let u = h(&t); print_int(u.0 * 10 + u.1); }\n",
                "RUN: 34\n",
            ),
            (
                "a method's parameter",
                "struct S { k: i64 }\nimpl S { fn m(&self, p: &(i64, i64)) -> i64 { return self.k + p.0 * 10 + p.1; } }\nfn main() { let s = S { k: 100 }; let t = (3, 4); print_int(s.m(&t)); }\n",
                "RUN: 134\n",
            ),
        ],
        "ref_tuple_read",
    );
}

/// The reference is handed on as the pointer it is: to a shared parameter
/// from either kind, and to a mutable one from a mutable one.
#[test]
fn a_tuple_reference_parameter_is_forwarded_as_the_pointer() {
    check(
        &[
            (
                "shared to shared",
                "fn inner(p: &(i64, i64)) -> i64 { return p.1; }\nfn outer(p: &(i64, i64)) -> i64 { return inner(p) + 100; }\nfn main() { let t = (3, 4); print_int(outer(&t)); }\n",
                "RUN: 104\n",
            ),
            (
                "mutable to mutable, twice",
                "fn inner(p: &mut (i64, i64)) { *p = (p.0 + 1, p.1 + 2); }\nfn outer(p: &mut (i64, i64)) { inner(p); inner(p); }\nfn main() { let mut t = (10, 20); outer(&mut t); print_int(t.0); print_int(t.1); }\n",
                "RUN: 12\n24\n",
            ),
            (
                "mutable to shared, after a write",
                "fn peek(p: &(i64, i64)) -> i64 { return p.0 * 1000 + p.1; }\nfn bump(p: &mut (i64, i64)) -> i64 { *p = (p.0 + 5, p.1); return peek(p); }\nfn main() { let mut t = (1, 2); print_int(bump(&mut t)); print_int(t.0); }\n",
                "RUN: 6002\n6\n",
            ),
        ],
        "ref_tuple_fwd",
    );
}

/// `*p = v` through `&mut` reaches the caller's tuple: once, repeatedly, from
/// the old value of the same tuple, nested, and holding a struct or a String.
#[test]
fn a_write_through_a_mutable_tuple_reference_reaches_the_caller() {
    check(
        &[
            (
                "`*p = (p.1, p.0 + 100)`",
                "fn set(p: &mut (i64, i64)) { *p = (p.1, p.0 + 100); }\nfn main() { let mut t = (1, 2); set(&mut t); print_int(t.0); print_int(t.1); }\n",
                "RUN: 2\n101\n",
            ),
            (
                "through `&mut X`",
                "type X = (i64, i64);\nfn set(p: &mut X) { *p = (7, 8); }\nfn main() { let mut t: X = (1, 2); set(&mut t); print_int(t.0 * 10 + t.1); }\n",
                "RUN: 78\n",
            ),
            (
                "a swap, three times",
                "fn swap(p: &mut (i64, i64)) { *p = (p.1, p.0); }\nfn main() { let mut t = (11, 22); swap(&mut t); swap(&mut t); swap(&mut t); print_int(t.0); print_int(t.1); }\n",
                "RUN: 22\n11\n",
            ),
            (
                "the argument without `&mut`",
                "fn set(p: &mut (i64, i64)) { *p = (7, 8); }\nfn main() { let mut t = (1, 2); set(t); print_int(t.0 * 10 + t.1); }\n",
                "RUN: 78\n",
            ),
            (
                "a nested tuple and a String",
                "fn set(p: &mut ((i64, i64), String)) { *p = (((p.0).1, (p.0).0), \"new\"); }\nfn main() { let mut t = ((1, 2), \"old\"); set(&mut t); print_int((t.0).0 * 10 + (t.0).1); print(t.1); }\n",
                "RUN: 21\nnew\n",
            ),
            (
                "a tuple holding a struct",
                "struct P { a: i64 }\nfn set(p: &mut (P, i64)) { *p = (P { a: p.1 }, p.0.a); }\nfn main() { let mut t = (P { a: 1 }, 2); set(&mut t); print_int(t.0.a * 10 + t.1); }\n",
                "RUN: 21\n",
            ),
        ],
        "ref_tuple_write",
    );
}

/// A write through `&(A, B)` is refused by THIS compiler, by name. The C
/// declarator is `const`, so without the refusal it reached gcc as an
/// assignment to a read-only lvalue.
///
/// Both spellings that write the pointee: `*p = v`, and `p = v` where the
/// binding is `mut p: &(A, B)` (the name reads as `(*p)` on either side).
#[test]
fn a_write_through_a_shared_tuple_reference_is_refused_by_name() {
    for (shape, program) in [
        (
            "`*p = v`",
            "fn set(p: &(i64, i64)) { *p = (7, 8); }\nfn main() { let t = (1, 2); set(&t); print_int(t.0); }\n",
        ),
        (
            "`p = v` under `mut p`",
            "fn set(mut p: &(i64, i64)) { p = (7, 8); }\nfn main() { let t = (1, 2); set(&t); print_int(t.0); }\n",
        ),
    ] {
        let verdict = outcome(program, "ref_tuple_shared_write");
        assert!(
            verdict.starts_with("REFUSED: ") && verdict.contains("shared reference"),
            "{}: {}",
            shape,
            verdict
        );
    }
}

/// A SHARED tuple reference is never lent on as a writable pointer (review
/// round 1). The type checker does not tell `&` from `&mut`, and C drops a
/// pointer's `const` with a warning only, so each of these turned the caller's
/// immutable `(11, 29)` into `(97, 83)` before it was refused — silently. Every
/// one is refused by THIS compiler's code generation, by name.
#[test]
fn a_shared_tuple_reference_is_never_lent_on_as_writable() {
    let set = "fn set(q: &mut (i64, i64)) { *q = (97, 83); }\n";
    let main = "fn main() { let t = (11, 29); f(&t); print_int(t.0); print_int(t.1); }\n";
    let rows: [(&str, String, &str); 10] = [
        ("`set(&p)`: a reborrow handed to `&mut`", format!("{set}fn f(p: &(i64, i64)) {{ set(&p); }}\n{main}"), "a reference taken from it may only be lent on to a `&` parameter"),
        ("`let r = &p; *r = ..`: a local alias", format!("fn f(p: &(i64, i64)) {{ let r = &p; *r = (97, 83); }}\n{main}"), "may only be handed straight to a `&` parameter"),
        ("`set(&p, 97)` into a generic `&mut (T, T)`", format!("fn set<T>(q: &mut (T, T), x: T) {{ *q = (x, x); }}\nfn f(p: &(i64, i64)) {{ set(&p, 97); }}\n{main}"), "a reference taken from it may only be lent on to a `&` parameter"),
        ("`set(p)` from `mut p: &(..)`", format!("{set}fn f(mut p: &(i64, i64)) {{ set(p); }}\n{main}"), "that parameter may write through what it is given"),
        ("`set(&mut p)` from `mut p: &(..)`", format!("{set}fn f(mut p: &(i64, i64)) {{ set(&mut p); }}\n{main}"), "a reference taken from it may only be lent on to a `&` parameter"),
        ("`m(p)` from `mut p: &(..)` to a `mut` by-value parameter", format!("fn m(mut u: (i64, i64)) {{ u = (97, 83); }}\nfn f(mut p: &(i64, i64)) {{ m(p); }}\n{main}"), "that parameter may write through what it is given"),
        ("a method's `&mut (..)` parameter", format!("struct S {{ k: i64 }}\nimpl S {{ fn m(&self, q: &mut (i64, i64)) {{ *q = (97, 83); }} }}\nfn f(p: &(i64, i64)) {{ let s = S {{ k: 1 }}; s.m(&p); }}\n{main}"), "a reference taken from it may only be lent on to a `&` parameter"),
        ("`&p` inside a tuple literal", format!("fn f(p: &(i64, i64)) {{ let u = (&p, 1); print_int(u.1); }}\n{main}"), "may only be handed straight to a `&` parameter"),
        ("`set(&t)`: a shared borrow of an immutable local handed to `&mut`", format!("{set}fn main() {{ let t = (11, 29); set(&t); print_int(t.0); print_int(t.1); }}\n"), "a shared borrow does not permit mutation"),
        ("`set(&a[0])`: a shared borrow of an element handed to `&mut`", format!("{set}fn main() {{ let a = [(11, 29), (1, 2)]; set(&a[0]); print_int(a[0].0); }}\n"), "a shared borrow does not permit mutation"),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program, fragment) in &rows {
        let verdict = outcome(program, "ref_tuple_lend");
        let refused_here = verdict.starts_with("REFUSED: ") && verdict.contains(fragment);
        let seen = if refused_here {
            "REFUSED by name"
        } else {
            &verdict
        };
        got.push(format!("{}: {}", shape, seen));
        want.push(format!("{}: REFUSED by name", shape));
    }
    assert_eq!(got, want);
}

/// A PLACE inside a shared tuple reference never reaches a writable parameter
/// (review round 2), however it is spelled: the call emitter takes the place's
/// address itself for a `&mut` or `mut` parameter, so `set(*p)` from
/// `mut p: &(i64, i64)` was emitted `set(&(*p))` — a `const T*` where `T*` is
/// declared — and printed `97 83` through each of these four parameter kinds.
#[test]
fn a_place_inside_a_shared_tuple_reference_never_reaches_a_writable_parameter() {
    let set = "fn set(q: &mut (i64, i64)) { *q = (97, 83); }\n";
    let main = "fn main() { let mut t = (11, 29); f(t); print_int(t.0); print_int(t.1); }\n";
    let rows: [(&str, String); 6] = [
        ("`set(*p)` into `&mut (..)`", format!("{set}fn f(mut p: &(i64, i64)) {{ set(*p); }}\n{main}")),
        ("`m(*p)` into a `mut` by-value parameter", format!("fn m(mut u: (i64, i64)) {{ u = (97, 83); }}\nfn f(mut p: &(i64, i64)) {{ m(*p); }}\n{main}")),
        ("`set(*p, 97)` into a generic `&mut (T, T)`", format!("fn set<T>(q: &mut (T, T), x: T) {{ *q = (x, x); }}\nfn f(mut p: &(i64, i64)) {{ set(*p, 97); }}\n{main}")),
        ("`s.set(*p)` into a method's `&mut (..)`", format!("struct S {{ k: i64 }}\nimpl S {{ fn set(&self, q: &mut (i64, i64)) {{ *q = (97, 83); }} }}\nfn f(mut p: &(i64, i64)) {{ let s = S {{ k: 1 }}; s.set(*p); }}\n{main}")),
        ("`set((*p))`, parenthesised", format!("{set}fn f(mut p: &(i64, i64)) {{ set((*p)); }}\n{main}")),
        ("`two(*p, next(..))`, the hoisted argument path", format!("fn next(c: &mut i64) -> i64 {{ *c = *c + 1; return *c; }}\nfn two(q: &mut (i64, i64), b: i64) {{ *q = (97, b); }}\nfn f(mut p: &(i64, i64)) {{ let mut k = 82; two(*p, next(&mut k)); }}\n{main}")),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program) in &rows {
        let verdict = outcome(program, "ref_tuple_place");
        let refused_here = verdict.starts_with("REFUSED: ")
            && verdict.contains("or a place inside it, and that parameter may write");
        let seen = if refused_here {
            "REFUSED by name"
        } else {
            &verdict
        };
        got.push(format!("{}: {}", shape, seen));
        want.push(format!("{}: REFUSED by name", shape));
    }
    assert_eq!(got, want);
}

/// A tuple place reached through a LOCAL reference reaches a writable
/// parameter only when that reference is PROVEN to be a `&mut` (review round
/// 3). Unknown never passes: before this, `set(*r)` with `let mut r = &t;`
/// printed `97 83` for an immutable `t` — annotated or not, through a generic
/// and through a method — and the round-2 rule, which followed only tuple
/// reference PARAMETERS, called the local "origin unknown" and let it through.
/// A reference re-pointed anywhere in the function is never proven.
#[test]
fn a_place_reached_through_a_local_reference_needs_a_proven_writable_origin() {
    let set = "fn set(q: &mut (i64, i64)) { *q = (97, 83); }\n";
    let out = "print_int(t.0); print_int(t.1); }\n";
    let rows: [(&str, String); 10] = [
        ("`let mut r: &(..) = &t; set(*r)`", format!("{set}fn main() {{ let t = (11, 29); let mut r: &(i64, i64) = &t; set(*r); {out}")),
        ("`let mut r = &t; set(*r)`", format!("{set}fn main() {{ let t = (11, 29); let mut r = &t; set(*r); {out}")),
        ("into a generic `&mut (T, T)`", format!("fn set<T>(q: &mut (T, T), x: T) {{ *q = (x, x); }}\nfn main() {{ let t = (11, 29); let mut r: &(i64, i64) = &t; set(*r, 97); {out}")),
        ("into a method's `&mut (..)`", format!("struct S {{ k: i64 }}\nimpl S {{ fn set(&self, q: &mut (i64, i64)) {{ *q = (97, 83); }} }}\nfn main() {{ let t = (11, 29); let mut r: &(i64, i64) = &t; let s = S {{ k: 1 }}; s.set(*r); {out}")),
        ("`&mut *r` of a shared local", format!("{set}fn main() {{ let t = (11, 29); let mut r = &t; set(&mut *r); {out}")),
        ("re-pointed to a shared borrow", format!("{set}fn main() {{ let mut t = (11, 29); let u = (1, 2); let mut r = &mut t; r = &u; set(*r); {out}")),
        ("re-pointed in a branch", format!("{set}fn main() {{ let mut t = (11, 29); let u = (1, 2); let mut r = &mut t; if true {{ r = &u; }} set(*r); {out}")),
        ("re-pointed later in a loop", format!("{set}fn main() {{ let mut t = (11, 29); let u = (1, 2); let mut r = &mut t; let mut i = 0; while i < 2 {{ set(*r); r = &u; i = i + 1; }} {out}")),
        ("a shared shadow in an inner block", format!("{set}fn main() {{ let mut t = (11, 29); let u = (1, 2); let mut r = &mut t; if true {{ let mut r = &u; set(*r); }} {out}")),
        ("`m(*r)` into a `mut` by-value parameter (ran on main, writing the immutable `t`)", format!("fn m(mut u: (i64, i64)) {{ u = (97, 83); }}\nfn main() {{ let t = (11, 29); let mut r = &t; m(*r); {out}")),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program) in &rows {
        let verdict = outcome(program, "ref_tuple_local");
        let refused_here = verdict.starts_with("REFUSED: ")
            && verdict.contains("that parameter writes the tuple it is given");
        let seen = if refused_here {
            "REFUSED by name"
        } else {
            &verdict
        };
        got.push(format!("{}: {}", shape, seen));
        want.push(format!("{}: REFUSED by name", shape));
    }
    assert_eq!(got, want);
}

/// Review round 4 (astra): a reference can be re-pointed where no `r = ..`
/// names it — through its own address (`let q = &mut r; *q = &u;`, or `&r`,
/// since a write through a shared local alias is not checked: found f3), or by
/// a call whose parameter receives its address (`mut r: R`). Each of these
/// printed `11 97 83` on 6e1ef59: the immutable `u = (1, 2)` overwritten. A
/// reference whose address is taken, or which is passed where its address is
/// taken, is never proven. The last row ran on main too, writing `u` through a
/// by-value `mut` parameter.
#[test]
fn a_reference_that_may_be_re_pointed_out_of_sight_is_never_proven() {
    let set = "fn set(q: &mut (i64, i64)) { *q = (97, 83); }\n";
    let head = "fn main() { let mut t = (11, 29); let u = (1, 2); let mut r = &mut t;";
    let out = "print_int(t.0); print_int(u.0); print_int(u.1); }\n";
    let rows: [(&str, String); 8] = [
        ("`let q = &mut r; *q = &u;` in an inner block", format!("{set}{head} if true {{ let q = &mut r; *q = &u; }} set(*r); {out}")),
        ("`let q = &r; *q = &u;` in an inner block", format!("{set}{head} if true {{ let q = &r; *q = &u; }} set(*r); {out}")),
        ("`&mut r` in a value block", format!("{set}{head} let k = {{ let q = &mut r; *q = &u; 1 }}; set(*r); {out}")),
        ("`&mut r` stored in an array", format!("{set}{head} if true {{ let a = [&mut r]; *a[0] = &u; }} set(*r); {out}")),
        ("`&mut r` in a loop", format!("{set}{head} let mut i = 0; while i < 1 {{ let q = &mut r; *q = &u; i = i + 1; }} set(*r); {out}")),
        ("passed to `mut r: R`, `type R = &(i64, i64)`", format!("type R = &(i64, i64);\nfn redirect(mut r: R, q: R) {{ r = q; }}\n{set}{head} redirect(r, &u); set(*r); {out}")),
        ("passed to `mut r: R`, `type R = &mut (i64, i64)`", format!("type R = &mut (i64, i64);\nfn redirect(mut r: R, q: R) {{ r = q; }}\n{set}{head} redirect(r, &u); set(*r); {out}")),
        ("into a `mut` by-value parameter (ran on main, writing the immutable `u`)", format!("fn m(mut x: (i64, i64)) {{ x = (97, 83); }}\n{head} if true {{ let q = &mut r; *q = &u; }} m(*r); {out}")),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program) in &rows {
        let verdict = outcome(program, "ref_tuple_repoint");
        let refused_here = verdict.starts_with("REFUSED: ")
            && verdict.contains("that parameter writes the tuple it is given");
        got.push(format!(
            "{}: {}",
            shape,
            if refused_here { "REFUSED by name" } else { &verdict }
        ));
        want.push(format!("{}: REFUSED by name", shape));
    }
    assert_eq!(got, want);
}

/// Review round 4 (astra): a binding of a reference-to-tuple PARAMETER's name.
/// Code generation reads that name as the parameter in the whole body (found
/// f6), so `let mut p = &u; set(*p);` inside `fn f(p: &mut (..))` was judged
/// as the writable parameter and wrote the immutable `u` (`97 83`); a value or
/// a pattern binding of the name reached gcc. Refused by name, at the binding.
/// main refused every one of these programs (the parameter itself).
#[test]
fn a_binding_that_shadows_a_tuple_reference_parameter_is_refused_by_name() {
    let set = "fn set(q: &mut (i64, i64)) { *q = (97, 83); }\n";
    let rows: [(&str, String); 5] = [
        ("`let mut p = &u; set(*p)` under `p: &mut (..)`", format!("{set}fn f(p: &mut (i64, i64)) {{ let u = (1, 2); if true {{ let mut p = &u; set(*p); }} print_int(u.0); }}\nfn main() {{ let mut t = (11, 29); f(&mut t); print_int(t.0); }}\n")),
        ("`let p = &u; *p = v` under `p: &mut (..)`", "fn f(p: &mut (i64, i64)) { let u = (1, 2); if true { let p = &u; *p = (97, 83); } print_int(u.0); }\nfn main() { let mut t = (11, 29); f(&mut t); print_int(t.0); }\n".to_string()),
        ("a `for` variable", "fn f(p: &mut (i64, i64)) { for p in [1, 2] { print_int(p); } }\nfn main() { let mut t = (11, 29); f(&mut t); print_int(t.0); }\n".to_string()),
        ("a `match` binding", "fn f(p: &(i64, i64)) { match (1, 2) { (p, q) => { print_int(p); } } }\nfn main() { let t = (11, 29); f(&t); print_int(t.0); }\n".to_string()),
        ("a `let mut` value under `p: &(..)`", format!("{set}fn f(p: &(i64, i64)) {{ if true {{ let mut p = (1, 2); set(&mut p); print_int(p.0); }} }}\nfn main() {{ let t = (11, 29); f(&t); print_int(t.0); }}\n")),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program) in &rows {
        let verdict = outcome(program, "ref_tuple_shadow");
        let refused_here = verdict.starts_with("REFUSED: ")
            && verdict.contains("is a reference-to-tuple parameter of this function");
        got.push(format!(
            "{}: {}",
            shape,
            if refused_here { "REFUSED by name" } else { &verdict }
        ));
        want.push(format!("{}: REFUSED by name", shape));
    }
    assert_eq!(got, want);
}

/// Review round 4 (astra, fable): what the proof keeps, judged by BINDING, not
/// by name. A same-named binding in another scope is another binding (its `r =
/// 4` re-points nothing); a reference re-pointed only ever at `&mut` of a
/// writable place — directly, in a branch, in a loop, or as the arms of an
/// `if` initializer — stays proven. The first two rows (and fable's d11, d13,
/// d15) ran on main and were refused by 6e1ef59.
#[test]
fn a_reference_re_pointed_only_at_writable_places_stays_proven() {
    let m = "fn m(mut x: (i64, i64)) { x = (97, 83); }\n";
    let set = "fn set(q: &mut (i64, i64)) { *q = (97, 83); }\n";
    let two = "let mut t = (11, 29); let mut u = (1, 2);";
    let rows: Vec<(&str, String, &str)> = vec![
        ("an inner `let mut r = 3; r = 4;` beside a by-value `mut` (main: 4 97 83)", format!("{m}fn main() {{ let mut t = (11, 29); let mut r = &mut t; if true {{ let mut r = 3; r = 4; print_int(r); }} m(*r); print_int(t.0); print_int(t.1); }}\n"), "RUN: 4\n97\n83\n"),
        ("`r = &mut u` beside a by-value `mut` (main: 11 97 83)", format!("{m}fn main() {{ {two} let mut r = &mut t; r = &mut u; m(*r); print_int(t.0); print_int(u.0); print_int(u.1); }}\n"), "RUN: 11\n97\n83\n"),
        ("used, re-pointed at `&mut u`, used (fable d11; main: 9783 9783)", format!("{m}fn main() {{ {two} let mut r = &mut t; m(*r); r = &mut u; m(*r); print_int(t.0 * 100 + t.1); print_int(u.0 * 100 + u.1); }}\n"), "RUN: 9783\n9783\n"),
        ("an `if` initializer of two `&mut` (fable d13; main: 9783)", format!("{m}fn main() {{ {two} let mut r = if t.0 > 5 {{ &mut t }} else {{ &mut u }}; m(*r); print_int(t.0 * 100 + t.1); }}\n"), "RUN: 9783\n"),
        ("re-pointed in a branch not taken (fable d15; main: 9783 102)", format!("{m}fn main() {{ {two} let mut r = &mut t; if t.0 > 50 {{ r = &mut u; }} m(*r); print_int(t.0 * 100 + t.1); print_int(u.0 * 100 + u.1); }}\n"), "RUN: 9783\n102\n"),
        ("an inner `let mut r = 3; r = 4;` beside `&mut (..)`", format!("{set}fn main() {{ let mut t = (11, 29); let mut r = &mut t; if true {{ let mut r = 3; r = 4; print_int(r); }} set(*r); print_int(t.0); print_int(t.1); }}\n"), "RUN: 4\n97\n83\n"),
        ("an inner shared `r`, re-pointed, beside the proven outer one", format!("{set}fn main() {{ let mut t = (11, 29); let u = (1, 2); let mut r = &mut t; if true {{ let mut r = &u; r = &u; print_int(9); }} set(*r); print_int(t.0); print_int(u.0); }}\n"), "RUN: 9\n97\n1\n"),
        ("`r = &mut u` beside `&mut (..)`", format!("{set}fn main() {{ {two} let mut r = &mut t; r = &mut u; set(*r); print_int(t.0); print_int(u.0); print_int(u.1); }}\n"), "RUN: 11\n97\n83\n"),
        ("re-pointed at `&mut u` in a loop", format!("{set}fn main() {{ {two} let mut r = &mut t; let mut i = 0; while i < 2 {{ r = &mut u; i = i + 1; }} set(*r); print_int(t.0); print_int(u.0); }}\n"), "RUN: 11\n97\n"),
        ("an `if` initializer of two `&mut` beside `&mut (..)`", format!("{set}fn main() {{ {two} let mut r = if t.0 > 5 {{ &mut t }} else {{ &mut u }}; set(*r); print_int(t.0); print_int(u.0); }}\n"), "RUN: 97\n1\n"),
        ("a block initializer `{ &mut t }`", format!("{set}fn main() {{ let mut t = (11, 29); let mut r = {{ &mut t }}; set(*r); print_int(t.0); print_int(t.1); }}\n"), "RUN: 97\n83\n"),
    ];
    let table: Vec<(&str, &str, &str)> = rows
        .iter()
        .map(|(shape, program, out)| (*shape, program.as_str(), *out))
        .collect();
    check(&table, "ref_tuple_proven");
}

/// Review rounds 5-6 (astra, fable): a `let` whose initializer reads its OWN
/// name means the OUTER binding, and now reads it (found f10, fixed at its
/// cause). A C declaration's name is in scope in its own initializer, so
/// `T* r = r;` read the new, uninitialized pointer: on f87fb3b astra's first
/// row died on signal 5 (rc 133), and on 1f21115 the read rows still did while
/// the write rows were refused. The `let` is lowered through a fresh temporary
/// computed before the new binding exists, so reads and writes are the source's.
/// The inner-block rows, whose `r` is ANOTHER binding of the name (main: 97 83),
/// and fable's a05/a06 (main: 9783) were refused by 1f21115's name-only check.
#[test]
fn a_let_that_reads_its_own_name_reads_the_outer_binding() {
    let set = "fn set(q: &mut (i64, i64)) { *q = (97, 83); }\n";
    let m = "fn m(mut x: (i64, i64)) { x = (97, 83); }\n";
    let get = "fn get(q: &(i64, i64)) -> i64 { return q.0 * 100 + q.1; }\n";
    let out = "print_int(a.0); print_int(a.1); }\n";
    let rows: Vec<(&str, String, &str)> = vec![
        ("`let mut r = r;` under `&mut *p` of a tuple parameter (astra r5)", format!("{set}fn f(p: &mut (i64, i64)) {{ let mut r = &mut *p; if true {{ let mut r = r; set(*r); }} }}\nfn main() {{ let mut a = (11, 29); f(&mut a); {out}"), "RUN: 97\n83\n"),
        ("`let mut r = &mut *r;`", format!("{set}fn main() {{ let mut a = (11, 29); let mut r = &mut a; if true {{ let mut r = &mut *r; set(*r); }} {out}"), "RUN: 97\n83\n"),
        ("`let mut r = r;` into a by-value `mut` (died on main)", format!("{m}fn main() {{ let mut a = (11, 29); let mut r = &mut a; if true {{ let mut r = r; m(*r); }} {out}"), "RUN: 97\n83\n"),
        ("`let mut r = r;` in a loop body", format!("{set}fn main() {{ let mut a = (11, 29); let mut r = &mut a; let mut i = 0; while i < 1 {{ let mut r = r; set(*r); i = i + 1; }} {out}"), "RUN: 97\n83\n"),
        ("a READ: `let r = r; get(*r)` (astra r6)", format!("{get}fn main() {{ let a = (11, 29); let r = &a; if true {{ let r = r; print_int(get(*r)); }} }}\n"), "RUN: 1129\n"),
        ("a READ through a copy: `let q = *r; get(&q)` (astra r6)", format!("{get}fn main() {{ let a = (11, 29); let r = &a; if true {{ let r = r; let q = *r; print_int(get(&q)); }} }}\n"), "RUN: 1129\n"),
        ("an inner block's own `r`: `let mut r = {{ let mut r = &mut a; r }};` (astra r6)", format!("{set}fn main() {{ let mut a = (11, 29); let mut r = {{ let mut r = &mut a; r }}; set(*r); {out}"), "RUN: 97\n83\n"),
        ("each `if` arm's own `r` (astra r6)", format!("{m}fn main() {{ let mut a = (11, 29); let mut b = (1, 2); let mut r = if true {{ let r = &mut a; r }} else {{ let r = &mut b; r }}; m(*r); print_int(a.0); print_int(b.0); }}\n"), "RUN: 97\n1\n"),
        ("`let mut r = if true {{ r }} else {{ r }};` (fable a05)", format!("{m}fn main() {{ let mut t = (11, 29); let mut r = &mut t; if true {{ let mut r = if true {{ r }} else {{ r }}; m(*r); }} print_int(t.0 * 100 + t.1); }}\n"), "RUN: 9783\n"),
        ("`let mut r = {{ r }};` (fable a06)", format!("{m}fn main() {{ let mut t = (11, 29); let mut r = &mut t; if true {{ let mut r = {{ r }}; m(*r); }} print_int(t.0 * 100 + t.1); }}\n"), "RUN: 9783\n"),
        ("renamed control: `let mut s = r;`", format!("{set}fn f(p: &mut (i64, i64)) {{ let mut r = &mut *p; if true {{ let mut s = r; set(*s); }} }}\nfn main() {{ let mut a = (11, 29); f(&mut a); {out}"), "RUN: 97\n83\n"),
    ];
    let table: Vec<(&str, &str, &str)> = rows
        .iter()
        .map(|(shape, program, want)| (*shape, program.as_str(), *want))
        .collect();
    check(&table, "ref_tuple_selfinit");
}

/// Found f10 itself, measured on main 2ca5eb5: every value below was garbage
/// or wrong there (`-16` for 6; the tuple swap `11 29`; a crash for the
/// string), because the shadowing `let` was emitted `T x = <reads x>;`. Lowered
/// through a fresh temporary, each prints the source's value.
#[test]
fn a_shadowing_let_that_reads_the_outer_binding_prints_the_source_value() {
    check(
        &[
            ("an integer, `let x = x + 1;`", "fn main() { let x = 5; if true { let x = x + 1; print_int(x); } print_int(x); }\n", "RUN: 6\n5\n"),
            ("nested twice", "fn main() { let x = 5; if true { let x = x * 10; if true { let x = x + 3; print_int(x); } print_int(x); } print_int(x); }\n", "RUN: 53\n50\n5\n"),
            ("a tuple swap", "fn main() { let t = (11, 29); if true { let t = (t.1, t.0); print_int(t.0); print_int(t.1); } print_int(t.0); }\n", "RUN: 29\n11\n11\n"),
            ("a struct", "struct P { a: i64, b: i64 }\nfn main() { let p = P { a: 3, b: 4 }; if true { let p = P { a: p.b, b: p.a + 10 }; print_int(p.a); print_int(p.b); } print_int(p.a); }\n", "RUN: 4\n13\n3\n"),
            ("a string", "fn main() { let s = \"abc\"; if true { let s = s; print(s); } print(s); }\n", "RUN: abc\nabc\n"),
            ("an array literal (copied with memcpy)", "fn main() { let xs = [1, 2]; if true { let xs = [xs[1], xs[0] + 10]; print_int(xs[0]); print_int(xs[1]); } print_int(xs[0]); }\n", "RUN: 2\n11\n1\n"),
            ("in a loop body", "fn main() { let mut i = 0; let k = 7; while i < 3 { let k = k + i; print_int(k); i = i + 1; } print_int(k); }\n", "RUN: 7\n8\n9\n7\n"),
            ("a parameter", "fn f(x: i64) -> i64 { if true { let x = x + 1; return x; } return 0; }\nfn main() { print_int(f(41)); }\n", "RUN: 42\n"),
            ("a call argument, then reassigned", "fn inc(v: i64) -> i64 { return v + 1; }\nfn main() { let mut x = 5; if true { let mut x = inc(x); x = x * 2; print_int(x); } print_int(x); }\n", "RUN: 12\n5\n"),
        ],
        "let_shadow_reads_outer",
    );
}

/// Review round 7 (astra): the temporary a self-reading `let` is lowered
/// through is named against every symbol the program EMITS, not only its
/// source names. `enum __pd { shadow0 }` emits the constructor `__pd_shadow0`;
/// d827a7e declared a local `long long __pd_shadow0` and the constructor call
/// beside it reached gcc ("called object type 'long long' is not a function").
/// Each row emits `__pd_shadow0` (or more) itself and lowers a `let` beside it.
#[test]
fn a_lowering_temporary_never_takes_a_name_the_program_emits() {
    check(
        &[
            ("an enum constructor `__pd::shadow0` (astra r7)", "enum __pd { shadow0 }\nfn main() { let x = { let x = 5; x }; let e = __pd::shadow0; match e { __pd::shadow0 => { print_int(x); } } }\n", "RUN: 5\n"),
            ("an enum constructor beside `let x = x + 1;`", "enum __pd { shadow0, shadow1 }\nfn main() { let x = 5; if true { let x = x + 1; let e = __pd::shadow0; match e { __pd::shadow0 => { print_int(x); } __pd::shadow1 => { print_int(0); } } } }\n", "RUN: 6\n"),
            ("two constructors, two lowerings", "enum __pd { shadow0, shadow1 }\nfn main() { let x = 5; let y = 7; if true { let x = x + 1; let y = y * 2; let e = __pd::shadow1; match e { __pd::shadow0 => { print_int(0); } __pd::shadow1 => { print_int(x + y); } } } }\n", "RUN: 20\n"),
            ("a function `__pd_shadow0`", "fn __pd_shadow0() -> i64 { return 7; }\nfn main() { let x = 5; if true { let x = x + __pd_shadow0(); print_int(x); } }\n", "RUN: 12\n"),
            ("a struct `__pd_shadow0`", "struct __pd_shadow0 { a: i64 }\nfn main() { let x = 5; if true { let x = x + 1; let s = __pd_shadow0 { a: x }; print_int(s.a); } }\n", "RUN: 6\n"),
            ("a const `__pd_shadow0`", "const __pd_shadow0: i64 = 30;\nfn main() { let x = 5; if true { let x = x + __pd_shadow0; print_int(x); } }\n", "RUN: 35\n"),
            ("a local `__pd_shadow0`", "fn main() { let __pd_shadow0 = 100; let x = 5; if true { let x = x + __pd_shadow0; print_int(x); } }\n", "RUN: 105\n"),
            ("a method of `struct __pd`", "struct __pd { k: i64 }\nimpl __pd { fn shadow0(&self) -> i64 { return self.k; } }\nfn main() { let p = __pd { k: 9 }; let x = 5; if true { let x = x + p.shadow0(); print_int(x); } }\n", "RUN: 14\n"),
        ],
        "let_shadow_hygiene",
    );
}

/// What the local-reference rule leaves running: a reference bound once with
/// `&mut` (even in a `mut` binding), a proven inner shadow beside a shared
/// outer one, and reads through a shared local.
#[test]
fn a_proven_local_mut_reference_still_writes_and_a_shared_one_still_reads() {
    check(
        &[
            (
                "`let mut r = &mut t; set(*r)`",
                "fn set(q: &mut (i64, i64)) { *q = (97, 83); }\nfn main() { let mut t = (11, 29); let mut r = &mut t; set(*r); print_int(t.0); print_int(t.1); }\n",
                "RUN: 97\n83\n",
            ),
            (
                "a proven inner shadow writes, the shared outer one is unchanged",
                "fn set(q: &mut (i64, i64)) { *q = (97, 83); }\nfn main() { let t = (11, 29); let mut u = (1, 2); let mut r = &t; if true { let mut r = &mut u; set(*r); } let q = *r; print_int(q.0); print_int(u.0); }\n",
                "RUN: 11\n97\n",
            ),
            (
                "`get(*r)` through a shared local",
                "fn get(q: &(i64, i64)) -> i64 { return q.0 * 100 + q.1; }\nfn main() { let t = (11, 29); let r = &t; print_int(get(*r)); }\n",
                "RUN: 1129\n",
            ),
        ],
        "ref_tuple_local_ok",
    );
}

/// `*p = v` through `&mut` stores only a value of the referent's C struct
/// (review round 1). The type checker does not compare the two, and a tuple
/// literal's struct is named from its elements' own C types, so each of these
/// reached gcc as an assignment between incompatible structs.
#[test]
fn a_value_of_another_c_struct_is_not_written_through_a_tuple_reference() {
    let rows = [
        ("another shape", "fn set(p: &mut ((i64, i64), (i64, i64))) { *p = (1, 2, 3, 4); }\nfn main() { let mut t = ((11, 29), (37, 41)); set(&mut t); print_int((t.0).0); }\n"),
        ("a scalar", "fn set(p: &mut (i64, i64)) { *p = 5; }\nfn main() { let mut t = (1, 2); set(&mut t); print_int(t.0); }\n"),
        ("another integer width", "fn set(p: &mut (i32, i64)) { *p = (1, 2); }\nfn main() { let a: i32 = 5; let mut t = (a, 6); set(&mut t); print_int(t.1); }\n"),
        ("a struct parameter's value as an element", "struct P { a: i64 }\nfn set(p: &mut (P, i64), q: P) { *p = (q, 1); }\nfn main() { let mut t = (P { a: 5 }, 2); let s = P { a: 9 }; set(&mut t, s); print_int(t.1); }\n"),
    ];
    for (shape, program) in rows {
        let verdict = outcome(program, "ref_tuple_value");
        assert!(
            verdict.starts_with("REFUSED: ") && verdict.contains("points at the C struct"),
            "{}: {}",
            shape,
            verdict
        );
    }
}

/// What the lending rule leaves alone: a shared reference lent to `&`
/// parameters (by name, as `&p`, as `&*p`, through a hoisted argument and a
/// method's parameter), and a mutable one reborrowed into `&mut`.
#[test]
fn a_tuple_reference_is_still_lent_where_it_cannot_be_written() {
    check(
        &[
            (
                "`&p`, `&*p`, and `&p` beside an argument that writes",
                "fn inner(q: &(i64, i64)) -> i64 { return q.0 * 100 + q.1; }\nfn next(c: &mut (i64, i64)) -> i64 { *c = (c.0 + 1, c.1); return c.0; }\nfn two(a: &(i64, i64), b: i64) -> i64 { return a.0 * 1000 + b; }\nfn f(p: &(i64, i64)) -> i64 { let mut c = (5, 0); return inner(&p) + two(&p, next(&mut c)) + two(&*p, 0); }\nfn main() { let t = (11, 29); print_int(f(&t)); print_int(t.0); }\n",
                "RUN: 23135\n11\n",
            ),
            (
                "a method's `&(..)` parameter, as `&p` and as `p`",
                "struct S { k: i64 }\nimpl S { fn m(&self, q: &(i64, i64)) -> i64 { return self.k + q.0; } }\nfn f(p: &(i64, i64)) -> i64 { let s = S { k: 100 }; return s.m(&p) + s.m(p); }\nfn main() { let t = (11, 29); print_int(f(&t)); }\n",
                "RUN: 222\n",
            ),
            (
                "`*p` of a `&mut` parameter into `&mut`",
                "fn set(q: &mut (i64, i64)) { *q = (97, 83); }\nfn f(p: &mut (i64, i64)) { set(*p); }\nfn main() { let mut t = (11, 29); f(&mut t); print_int(t.0); print_int(t.1); }\n",
                "RUN: 97\n83\n",
            ),
            (
                "`*p` and `p` of a shared one, by value and into `&`",
                "fn v(u: (i64, i64)) -> i64 { return u.0 * 100 + u.1; }\nfn peek(q: &(i64, i64)) -> i64 { return q.0 * 100 + q.1; }\nfn f(mut p: &(i64, i64)) -> i64 { return v(*p) + v(p) + peek(*p) + peek(p); }\nfn main() { let mut t = (11, 29); print_int(f(t)); print_int(t.0); }\n",
                "RUN: 4516\n11\n",
            ),
            (
                "`&mut *p` of a `&mut` parameter into `&mut`",
                "fn set(q: &mut (i64, i64)) { *q = (97, 83); }\nfn f(p: &mut (i64, i64)) { set(&mut *p); }\nfn main() { let mut t = (11, 29); f(&mut t); print_int(t.0); print_int(t.1); }\n",
                "RUN: 97\n83\n",
            ),
        ],
        "ref_tuple_lend_ok",
    );
}

/// A tuple reference result beside a tuple reference parameter is refused by
/// name. `return p;` would hand back the parameter's pointee where the result
/// is a pointer, and the caller would hold a `T*` to what it lent `const`.
#[test]
fn a_tuple_reference_result_beside_a_tuple_reference_parameter_is_refused_by_name() {
    for (shape, program) in [
        (
            "returned, called",
            "fn id(p: &(i64, i64)) -> &(i64, i64) { return p; }\nfn main() { let t = (3, 4); let r = id(&t); print_int(r.1); }\n",
        ),
        (
            "never called",
            "fn id(p: &mut (i64, i64)) -> &mut (i64, i64) { return p; }\nfn main() { print_int(1); }\n",
        ),
    ] {
        let verdict = outcome(program, "ref_tuple_ret");
        assert!(
            verdict.starts_with("REFUSED: ") && verdict.contains("returns a reference"),
            "{}: {}",
            shape,
            verdict
        );
    }
}

/// A tuple holding an array has no C struct this backend can emit (gcc refuses
/// `long long[3] f0;` for a local as well), so the reference to one is refused
/// by name, as main refused it, rather than handed to gcc.
#[test]
fn a_reference_to_a_tuple_holding_an_array_is_refused_by_name() {
    let verdict = outcome(
        "fn h(p: &([i64; 3], i64)) -> i64 { return p.1; }\nfn main() { let t = ([1, 2, 3], 4); print_int(h(&t)); }\n",
        "ref_tuple_array",
    );
    assert!(
        verdict.starts_with("REFUSED: ") && verdict.contains("array"),
        "{}",
        verdict
    );
}

/// What other passes decide, and what main ran, is unchanged: each row is
/// main's verdict, measured on d5166e8.
#[test]
fn the_neighbouring_verdicts_are_main_s() {
    check(
        &[
            (
                "`p.0` through `&X`: the type checker's, tests/xfail/alias_tuple_behind_reference.pd",
                "type X = (i64, i64);\nfn h(p: &X) -> i64 { return p.0; }\nfn main() { let t: X = (5, 6); print_int(h(&t)); }\n",
                "REFUSED: Type mismatch: expected a tuple, which is what `.0` reads an element of, found X",
            ),
            (
                "`p.0 = v`: no tuple-element place in the grammar",
                "fn set(p: &mut (i64, i64)) { p.0 = 7; }\nfn main() { let mut t = (1, 2); set(&mut t); print_int(t.0); }\n",
                "REFUSED: Syntax error: Invalid assignment target",
            ),
            (
                "a shared reference lent to `&mut`: the borrow checker's",
                "fn bump(p: &mut (i64, i64)) { *p = (9, 9); }\nfn f(p: &(i64, i64)) { bump(p); }\nfn main() { let t = (1, 2); f(&t); print_int(t.0); }\n",
                "REFUSED: Borrow checker error: cannot borrow `p` as mutable: it is not declared mutable. Declare it `let mut p` (or take the parameter as `&mut`) if it is meant to be modified.",
            ),
            (
                "a reference alias kept by name, never called",
                "type A = &(i64, i64);\nfn h(p: A) -> i64 { return 0; }\nfn main() { print_int(0); }\n",
                "RUN: 0\n",
            ),
            (
                "a reference alias kept by name, returned as `&(i64, i64)`",
                "type A = &(i64, i64);\nfn f(p: A) -> &(i64, i64) { return p; }\nfn main() { let t = (3, 4); let r = f(&t); let q = *r; print_int(q.0 * 10 + q.1); }\n",
                "RUN: 34\n",
            ),
            (
                "a tuple reference result without a tuple reference parameter",
                "fn f(t: (i64, i64)) -> &(i64, i64) { return &t; }\nfn main() { print_int(1); }\n",
                "RUN: 1\n",
            ),
        ],
        "ref_tuple_neighbours",
    );
}
