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
