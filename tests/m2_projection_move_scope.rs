//! A MOVE, ASSIGNMENT OR BORROW OF A FIELD BELONGS TO THE BINDING IT TOUCHED.
//!
//! The borrow checker keys ownership state by `Place`, and a `Place` is a NAME.
//! Moving `d.inner` writes an entry for `d.inner`, which `resolve_place` finds
//! before `d` itself — and `exit_scope` used to retire only the declared local,
//! so the entry outlived its function. Measured on 2563001: a field move in one
//! function refused a sibling function's freshly owned parameter of the same
//! name ("Use of moved value: d.inner"), and in the other direction a stale
//! `Owned` entry left by a borrow or an assignment masked a MOVED parent, so a
//! value moved out of `d.inner` was read again through `d.inner.w` and the
//! program compiled. The repair is in `src/ownership/mod.rs` (`Shadowed`,
//! `mark_moved`, `reinitialize`); the end-to-end witness is
//! `tests/regression/projection_move_scope.pd`.
//!
//! These drive the front end directly rather than through `pdc`: a refusal is a
//! use-after-move, which has no GI-12 code, so the corpus cannot pin the must-
//! refuse half as `reject` rows (src/errors/codes.rs, PD0012's note).

use palladium::lexer::Lexer;
use palladium::macros::MacroExpander;
use palladium::ownership::BorrowChecker;
use palladium::parser::Parser;
use palladium::{CompileError, Result};

/// Run the real front end up to and including borrow checking. The defects
/// below are about which STATE survives which scope, and only a parsed program
/// exercises the order in which the checker declares, moves and retires places.
fn borrow_check(source: &str) -> Result<()> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.collect_tokens()?;
    let mut parser = Parser::new(tokens);
    let mut program = parser.parse()?;
    MacroExpander::new().expand_program(&mut program)?;
    BorrowChecker::new().check_program(&program)
}

fn is_move_out_of_borrowed_content(result: &Result<()>) -> bool {
    matches!(
        result.as_ref().err().map(CompileError::peel),
        Some(CompileError::CannotMoveOutOfBorrowedContent { .. })
    )
}

/// The refusal of a write to a place a live borrow overlaps. It is a
/// `ConflictingBorrows` — the write conflicts with the borrow — and it names
/// the assignment, which is what tells it apart from a borrow-vs-borrow
/// conflict of the same variant.
fn is_assignment_to_a_borrowed_place(result: &Result<()>) -> bool {
    matches!(
        result.as_ref().err().map(CompileError::peel),
        Some(CompileError::ConflictingBorrows { message, .. }) if message.starts_with("cannot assign to")
    )
}

fn is_use_of_moved_value(result: &Result<()>) -> bool {
    matches!(
        result.as_ref().err().map(CompileError::peel),
        Some(CompileError::UseOfMovedValue { .. })
    )
}

/// A MOVE OUT OF A FIELD DIES WITH THE FUNCTION THAT MADE IT.
///
/// Measured before the fix: `a` moving its own parameter's field `d.inner`
/// left the key `d.inner: Moved` in the context after `a` returned, because
/// `exit_scope` retired only the declared local `d`. `b`'s parameter is a
/// different, freshly owned `d`, and `resolve_place` found the stale
/// projection before reaching it: "Use of moved value: d.inner". With `b`
/// alone the program compiles.
#[test]
fn test_a_field_move_does_not_outlive_its_function() {
    let result = borrow_check(
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn a(d: D) -> i64 { let x: In = d.inner; return x.v; }
        fn b(d: D) -> i64 { let y: In = d.inner; return y.v; }
        fn main() {
            let d1: D = D { inner: In { v: 4 } };
            let d2: D = D { inner: In { v: 5 } };
            print_int(a(d1));
            print_int(b(d2));
        }
        "#,
    );
    assert!(
        result.is_ok(),
        "a field move in one function poisoned another function's parameter: {:?}",
        result
    );
}

/// The same leak through every other writer of a projection key: a field
/// passed BY VALUE to a call (`move_value` into a temporary), a field on the
/// right of an assignment (`move_value` into a local), an array element,
/// and the receiver of a by-value method.
#[test]
fn test_no_projection_writer_leaks_into_a_later_function() {
    let programs = [
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn take(i: In) -> i64 { return i.v; }
        fn a(d: D) -> i64 { return take(d.inner); }
        fn b(d: D) -> i64 { return take(d.inner); }
        fn main() { print_int(a(D { inner: In { v: 6 } }) + b(D { inner: In { v: 7 } })); }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn a(d: D) -> i64 { let mut x: In = In { v: 0 }; x = d.inner; return x.v; }
        fn b(d: D) -> i64 { let mut y: In = In { v: 0 }; y = d.inner; return y.v; }
        fn main() { print_int(a(D { inner: In { v: 1 } }) + b(D { inner: In { v: 2 } })); }
        "#,
        r#"
        struct In { v: i64 }
        fn a(xs: [In; 2]) -> i64 { let x: In = xs[0]; return x.v; }
        fn b(xs: [In; 2]) -> i64 { let y: In = xs[0]; return y.v; }
        fn main() {
            let p: [In; 2] = [In { v: 1 }, In { v: 2 }];
            let q: [In; 2] = [In { v: 3 }, In { v: 4 }];
            print_int(a(p) + b(q));
        }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        impl D {
            fn a(self) -> i64 { let x: In = self.inner; return x.v; }
            fn b(self) -> i64 { let y: In = self.inner; return y.v; }
        }
        fn main() {
            let d1: D = D { inner: In { v: 4 } };
            let d2: D = D { inner: In { v: 5 } };
            print_int(d1.a() + d2.b());
        }
        "#,
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            result.is_ok(),
            "a projection's state leaked into a later function: {:?}\n{}",
            result,
            program
        );
    }
}

/// A BINDING IN AN INNER SCOPE OWNS ITS WHOLE SUBTREE, in both directions.
///
/// An inner `d` that shadows the outer one must not leave its field moves
/// behind for the outer `d` (measured before: the outer `d.inner` was
/// refused after the block), and must not inherit the outer `d`'s field
/// moves either (measured before: the inner, freshly built `d` was refused).
/// Two sibling blocks binding the same name are the same question twice.
#[test]
fn test_a_shadowing_binding_neither_leaks_nor_inherits_field_moves() {
    let programs = [
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let d: D = D { inner: In { v: 1 } };
            let c: bool = true;
            if c { let d: D = D { inner: In { v: 2 } }; let t: In = d.inner; print_int(t.v); }
            let u: In = d.inner;
            print_int(u.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let d: D = D { inner: In { v: 1 } };
            let a: In = d.inner;
            let c: bool = true;
            if c { let d: D = D { inner: In { v: 2 } }; let t: In = d.inner; print_int(t.v); }
            print_int(a.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let c: bool = true;
            if c { let d: D = D { inner: In { v: 1 } }; let t: In = d.inner; print_int(t.v); }
            else { let d: D = D { inner: In { v: 2 } }; let u: In = d.inner; print_int(u.v); }
        }
        "#,
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            result.is_ok(),
            "a shadowing binding leaked or inherited a field move: {:?}\n{}",
            result,
            program
        );
    }
}

/// RE-INITIALISING THE WHOLE VALUE RE-INITIALISES ITS FIELDS. `d = e;`
/// gives `d` a new value, so a move out of the OLD `d.inner` no longer
/// says anything about the new one. Measured before: refused, because the
/// stale `d.inner: Moved` key was found before the re-initialised `d`.
#[test]
fn test_assigning_the_base_revives_its_moved_fields() {
    let result = borrow_check(
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let mut d: D = D { inner: In { v: 1 } };
            let e: D = D { inner: In { v: 2 } };
            let a: In = d.inner;
            d = e;
            let b: In = d.inner;
            print_int(a.v + b.v);
        }
        "#,
    );
    assert!(
        result.is_ok(),
        "a field of a re-assigned base was still moved: {:?}",
        result
    );
}

/// A STALE LEAF MUST NOT MASK A MOVED PARENT — the unsound half.
///
/// `check_expr` refuses a path whose ROOT is moved, so the hole is one level
/// down: `peek(&d.inner.w)` leaves `d.inner.w: Owned` once its borrow ends
/// (and `d.inner.w = x` writes the same key), `let a = d.inner` then marks
/// `d.inner` moved, and `resolve_place` answers for `d.inner.w` from the
/// stale leaf before it reaches the moved parent. Measured before: all three
/// programs COMPILED and ran, reading `w` through both `a` and `z`. The
/// third one gets its stale leaf from a different function altogether.
#[test]
fn test_a_stale_leaf_does_not_mask_a_moved_parent() {
    let programs = [
        r#"
        struct W { n: i64 }
        struct In { w: W }
        struct D { inner: In }
        fn peek(w: &W) -> i64 { return w.n; }
        fn main() {
            let d: D = D { inner: In { w: W { n: 1 } } };
            print_int(peek(&d.inner.w));
            let a: In = d.inner;
            let z: W = d.inner.w;
            print_int(a.w.n + z.n);
        }
        "#,
        r#"
        struct W { n: i64 }
        struct In { w: W }
        struct D { inner: In }
        fn main() {
            let mut d: D = D { inner: In { w: W { n: 1 } } };
            let x: W = W { n: 9 };
            d.inner.w = x;
            let a: In = d.inner;
            let z: W = d.inner.w;
            print_int(a.w.n + z.n);
        }
        "#,
        r#"
        struct W { n: i64 }
        struct In { w: W }
        struct D { inner: In }
        fn peek(w: &W) -> i64 { return w.n; }
        fn a(d: D) -> i64 { return peek(&d.inner.w); }
        fn b(d: D) -> i64 { let i: In = d.inner; let z: W = d.inner.w; return i.w.n + z.n; }
        fn main() {
            print_int(a(D { inner: In { w: W { n: 1 } } }));
            print_int(b(D { inner: In { w: W { n: 2 } } }));
        }
        "#,
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            is_use_of_moved_value(&result),
            "a stale field state masked a moved parent: {:?}\n{}",
            result,
            program
        );
    }
}

/// THE CONTRACT ON ALL OF THE ABOVE: every genuine use after a field move
/// stays refused. A second move of the same field; a move inside a branch
/// followed by a use after it (the move belongs to the ENCLOSING `d`, so the
/// branch's exit must not retire it); and an outer move that a shadowing
/// inner block must hand back intact.
#[test]
fn test_field_moves_that_are_real_stay_refused() {
    let programs = [
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let d: D = D { inner: In { v: 1 } };
            let a: In = d.inner;
            let b: In = d.inner;
            print_int(a.v + b.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let d: D = D { inner: In { v: 1 } };
            let c: bool = true;
            if c { let t: In = d.inner; print_int(t.v); }
            let u: In = d.inner;
            print_int(u.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let d: D = D { inner: In { v: 1 } };
            let c: bool = false;
            if c { print_int(0); } else { let t: In = d.inner; print_int(t.v); }
            let u: In = d.inner;
            print_int(u.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let d: D = D { inner: In { v: 1 } };
            let a: In = d.inner;
            let c: bool = true;
            if c { let d: D = D { inner: In { v: 2 } }; let t: In = d.inner; print_int(t.v); }
            let u: In = d.inner;
            print_int(a.v + u.v);
        }
        "#,
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            is_use_of_moved_value(&result),
            "a genuine use after a field move was accepted: {:?}\n{}",
            result,
            program
        );
    }
}

/// Re-assigning a base while a borrow below it is LIVE is refused at the
/// assignment: `r` still borrows `d.inner`, and `d = e;` overwrites what it
/// points at. Before review round 1 this program was refused one statement
/// later, at the move out of `d.inner`, and the assignment itself passed.
#[test]
fn test_assigning_the_base_keeps_a_live_field_borrow() {
    let result = borrow_check(
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let mut d: D = D { inner: In { v: 1 } };
            let e: D = D { inner: In { v: 2 } };
            let r: &In = &d.inner;
            d = e;
            let f: In = d.inner;
            print_int(f.v);
        }
        "#,
    );
    assert!(
        is_assignment_to_a_borrowed_place(&result),
        "re-assigning a base under a live borrow of its field was not refused at the assignment: {:?}",
        result
    );
}

/// A PARENT CANNOT BE MOVED OUT FROM UNDER A LIVE BORROW OF ITS CHILD.
///
/// `r` borrows `d.inner.w` until the branch closes and `d.inner` is moved
/// while it does. On 2563001 the branch's exit ended the borrow and wrote
/// `d.inner.w: Owned` back over a field whose parent had been moved out:
/// compiled and printed `1 1`. 6a3d028 refused the later read instead; the
/// move itself is the error, because `r` still points into what it moves.
#[test]
fn test_ending_a_borrow_does_not_revive_a_field_of_a_moved_parent() {
    let result = borrow_check(
        r#"
        struct W { n: i64 }
        struct In { w: W }
        struct D { inner: In }
        fn main() {
            let d: D = D { inner: In { w: W { n: 1 } } };
            let c: bool = true;
            if c {
                let r: &W = &d.inner.w;
                let a: In = d.inner;
                print_int(a.w.n);
            }
            let z: W = d.inner.w;
            print_int(z.n);
        }
        "#,
    );
    assert!(
        is_move_out_of_borrowed_content(&result),
        "a parent was moved out from under a live borrow of its child: {:?}",
        result
    );
}

/// A SHADOWING BINDING DOES NOT INHERIT THE OUTER BINDING'S BORROWS either.
/// The inner `d` is a different variable; nothing borrows it. Measured
/// before: the first program was refused with "Conflicting borrows" (the
/// outer `&d` matched by name in `borrows`), the second with "Cannot move
/// out of borrowed content" (the outer `d.inner: Borrowed` key).
#[test]
fn test_a_shadowing_binding_does_not_inherit_outer_borrows() {
    let programs = [
        r#"
        struct In { v: i64 }
        fn bump(i: &mut In) { i.v = i.v + 1; }
        fn main() {
            let d: In = In { v: 1 };
            let r: &In = &d;
            let c: bool = true;
            if c { let mut d: In = In { v: 5 }; bump(&mut d); print_int(d.v); }
            print_int(r.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let d: D = D { inner: In { v: 1 } };
            let r: &In = &d.inner;
            let c: bool = true;
            if c { let d: D = D { inner: In { v: 5 } }; let f: In = d.inner; print_int(f.v); }
            print_int(r.v);
        }
        "#,
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            result.is_ok(),
            "a shadowing binding inherited the outer binding's borrow: {:?}\n{}",
            result,
            program
        );
    }
}

/// ...and the outer binding gets them BACK when the shadow ends: the outer
/// `&d` is still live after the branch, so a mutable borrow of the outer `d`
/// there is still a conflict.
#[test]
fn test_outer_borrows_survive_a_shadowing_block() {
    let result = borrow_check(
        r#"
        struct In { v: i64 }
        fn bump(i: &mut In) { i.v = i.v + 1; }
        fn main() {
            let mut d: In = In { v: 1 };
            let r: &In = &d;
            let c: bool = true;
            if c { let mut d: In = In { v: 5 }; bump(&mut d); }
            bump(&mut d);
            print_int(r.v);
        }
        "#,
    );
    assert!(
        matches!(
            result.as_ref().err().map(CompileError::peel),
            Some(CompileError::ConflictingBorrows { .. })
        ),
        "the outer borrow was lost across a shadowing block: {:?}",
        result
    );
}

/// AN ASSIGNMENT FROM A VALUE THAT IS NOT A PLACE GIVES A NEW VALUE TOO. `d = e;`
/// re-initialised `d` and `d = D { … };` did not: the checker called into the
/// ownership context only when the right-hand side was a place it could move
/// out of, so a struct literal or a call wrote no state at all. Measured
/// before: all three refused — "Use of moved value: d.inner" for a field moved
/// before the whole was re-assigned, "Use of moved value: d" for a whole move
/// re-assigned from a call, and "Use of moved value: d.inner" for the moved
/// field itself re-assigned from a literal.
#[test]
fn test_reassigning_from_a_literal_or_call_revives_what_was_moved() {
    let programs = [
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let mut d: D = D { inner: In { v: 1 } };
            let a: In = d.inner;
            d = D { inner: In { v: 2 } };
            let b: In = d.inner;
            print_int(a.v + b.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn make(n: i64) -> D { return D { inner: In { v: n } }; }
        fn main() {
            let mut d: D = make(1);
            let e: D = d;
            d = make(2);
            let b: In = d.inner;
            print_int(e.inner.v + b.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let mut d: D = D { inner: In { v: 1 } };
            let a: In = d.inner;
            d.inner = In { v: 2 };
            let b: In = d.inner;
            print_int(a.v + b.v);
        }
        "#,
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            result.is_ok(),
            "a re-assigned place was still moved: {:?}\n{}",
            result,
            program
        );
    }
}

/// AN ASSIGNMENT DOES NOT END A BORROW, whichever way its right-hand side is
/// spelled, and it is refused where it happens. On 2563001 the PLACE spellings
/// were ACCEPTED (measured: both printed 2), because writing `Owned` to the
/// target overwrote the borrow; 6a3d028 refused every spelling one statement
/// later, at the move out of what `r` borrows, so the write itself still
/// passed (#53).
#[test]
fn test_an_assignment_does_not_end_a_borrow() {
    let programs = [
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let mut d: D = D { inner: In { v: 1 } };
            let r: &In = &d.inner;
            d = D { inner: In { v: 2 } };
            let f: In = d.inner;
            print_int(f.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        fn main() {
            let mut d: In = In { v: 1 };
            let r: &In = &d;
            d = In { v: 2 };
            let f: In = d;
            print_int(f.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        fn main() {
            let mut d: In = In { v: 1 };
            let e: In = In { v: 2 };
            let r: &In = &d;
            d = e;
            let f: In = d;
            print_int(f.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let mut d: D = D { inner: In { v: 1 } };
            let r: &D = &d;
            d.inner = In { v: 2 };
            let f: In = d.inner;
            print_int(f.v);
        }
        "#,
        r#"
        struct In { v: i64 }
        struct D { inner: In }
        fn main() {
            let mut d: D = D { inner: In { v: 1 } };
            let x: In = In { v: 2 };
            let r: &D = &d;
            d.inner = x;
            let f: In = d.inner;
            print_int(f.v);
        }
        "#,
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            is_assignment_to_a_borrowed_place(&result),
            "an assignment under a live borrow was not refused at the assignment: {:?}\n{}",
            result,
            program
        );
    }
}

/// A WRITE BELOW A MOVED-OUT PARENT DOES NOT BRING THE PARENT BACK. `d.inner`
/// was moved into `a`; giving `d.inner.w` a value does not make `d.inner`
/// whole again, so reading `d.inner.w` stays a use of a moved value. Measured
/// before: the literal spelling was refused and the place spelling COMPILED and
/// printed 10 — p18's stale leaf with the write after the move instead of
/// before it.
#[test]
fn test_an_assignment_below_a_moved_parent_does_not_revive_it() {
    let programs = [
        r#"
        struct W { n: i64 }
        struct In { w: W }
        struct D { inner: In }
        fn main() {
            let mut d: D = D { inner: In { w: W { n: 1 } } };
            let a: In = d.inner;
            d.inner.w = W { n: 9 };
            let z: W = d.inner.w;
            print_int(a.w.n + z.n);
        }
        "#,
        r#"
        struct W { n: i64 }
        struct In { w: W }
        struct D { inner: In }
        fn main() {
            let mut d: D = D { inner: In { w: W { n: 1 } } };
            let a: In = d.inner;
            let x: W = W { n: 9 };
            d.inner.w = x;
            let z: W = d.inner.w;
            print_int(a.w.n + z.n);
        }
        "#,
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            is_use_of_moved_value(&result),
            "a write below a moved parent revived it: {:?}\n{}",
            result,
            program
        );
    }
}

/// Each program is wrapped in the same two struct declarations.
fn with_d(main_body: &str) -> String {
    format!(
        "struct In {{ v: i64 }}\nstruct D {{ inner: In }}\nfn mk(n: i64) -> D {{ return D {{ inner: In {{ v: n }} }}; }}\n{}",
        main_body
    )
}

/// A RE-INITIALISATION ON ONE PATH DOES NOT COUNT AFTER THE BRANCH.
///
/// Review round 1 on 6a3d028 measured the first four COMPILING (and running)
/// where 2563001 refused them: the walk visited the arms in sequence and
/// whatever the last write said survived the branch, so `d = D { … }` inside
/// an `if` with no `else` made `d.inner` usable on the path that skipped it.
/// The rest are the same question asked through every other branching form the
/// checker walks: an `else if` chain with no final `else`, an inner `if` that
/// re-initialises on both of ITS paths inside one arm of an outer `if`, a
/// statement `match`, a value `if`, a value `match`, the right operand of
/// `&&` (evaluated only when the left one is true), and an arm that
/// re-initialises and then leaves the function.
#[test]
fn test_a_re_initialisation_on_one_path_does_not_count_after_the_branch() {
    let bodies = [
        // branch_literal (gpt-6-astra)
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let c: bool = false;
           if c { d = mk(2); } let b: In = d.inner; print_int(a.v + b.v); }",
        // branch_place
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let e: D = mk(2); let c: bool = false;
           if c { d = e; } let b: In = d.inner; print_int(a.v + b.v); }",
        // branch_whole
        "fn main() { let mut d: D = mk(1); let a: D = d; let c: bool = false;
           if c { d = mk(2); } let b: D = d; print_int(a.inner.v + b.inner.v); }",
        // one_arm_moved_other_reinit
        "fn main() { let mut d: D = mk(1); let c: bool = true;
           if c { let a: In = d.inner; print_int(a.v); } else { d = mk(2); }
           let b: In = d.inner; print_int(b.v); }",
        // c35: the re-initialisation is in the ELSE arm
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let c: bool = true;
           if c { print_int(0); } else { d = mk(2); } let b: In = d.inner; print_int(a.v + b.v); }",
        // a re-initialised FIELD on one path
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let c: bool = false;
           if c { d.inner = In { v: 2 }; } let b: In = d.inner; print_int(a.v + b.v); }",
        // else-if chain, no final else
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let k: i64 = 5;
           if k == 0 { d = mk(2); } else if k == 1 { d = mk(3); }
           let b: In = d.inner; print_int(a.v + b.v); }",
        // inner if re-initialises on both paths, inside ONE arm of the outer if
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let c: bool = true; let e: bool = false;
           if c { if e { d = mk(2); } else { d = mk(3); } }
           let b: In = d.inner; print_int(a.v + b.v); }",
        // statement match, one arm
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let k: i64 = 1;
           match k { 0 => { d = mk(2); } _ => { print_int(0); } }
           let b: In = d.inner; print_int(a.v + b.v); }",
        // value if
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let c: bool = false;
           let n: i64 = if c { d = mk(2); 1 } else { 2 };
           let b: In = d.inner; print_int(a.v + b.v + n); }",
        // value match
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let k: i64 = 1;
           let n: i64 = match k { 0 => { d = mk(2); 1 } _ => 2, };
           let b: In = d.inner; print_int(a.v + b.v + n); }",
        // right operand of &&
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let c: bool = false;
           let ok: bool = c && { d = mk(2); true };
           let b: In = d.inner; print_int(a.v + b.v); }",
        // the re-initialising arm leaves the function; the fall-through path did not re-initialise
        "fn f(c: bool) -> i64 { let mut d: D = mk(1); let a: In = d.inner;
           if c { d = mk(2); return 1; }
           let b: In = d.inner; return a.v + b.v; }
         fn main() { print_int(f(true)); }",
    ];
    for body in bodies {
        let program = with_d(body);
        let result = borrow_check(&program);
        assert!(
            is_use_of_moved_value(&result),
            "a re-initialisation on one path counted after the branch: {:?}\n{}",
            result,
            program
        );
    }
}

/// ...AND ONE ON EVERY PATH DOES. The join is "owned only if owned on all
/// paths", not "never owned again": both arms of an `if`/`else`, every link of
/// an `else if` chain that ends in `else`, every arm of a `match`, both arms of
/// a value `if`, and an outer `if` whose arms both re-initialise (one of them
/// through an inner `if`). An arm that LEAVES THE FUNCTION is not a path to the
/// code after the branch, so it does not have to re-initialise. All of these
/// were refused on 2563001.
#[test]
fn test_a_re_initialisation_on_every_path_counts_after_the_branch() {
    let bodies = [
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let c: bool = false;
           if c { d = mk(2); } else { d = mk(3); } let b: In = d.inner; print_int(a.v + b.v); }",
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let k: i64 = 5;
           if k == 0 { d = mk(2); } else if k == 1 { d = mk(3); } else { d = mk(4); }
           let b: In = d.inner; print_int(a.v + b.v); }",
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let k: i64 = 1;
           match k { 0 => { d = mk(2); } _ => { d = mk(3); } }
           let b: In = d.inner; print_int(a.v + b.v); }",
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let c: bool = false;
           let n: i64 = if c { d = mk(2); 1 } else { d = mk(3); 2 };
           let b: In = d.inner; print_int(a.v + b.v + n); }",
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let c: bool = true; let e: bool = false;
           if c { if e { d = mk(2); } else { d = mk(3); } } else { d = mk(4); }
           let b: In = d.inner; print_int(a.v + b.v); }",
        "fn f(c: bool) -> i64 { let mut d: D = mk(1); let a: In = d.inner;
           if c { d = mk(2); } else { return a.v; }
           let b: In = d.inner; return a.v + b.v; }
         fn main() { print_int(f(true)); }",
    ];
    for body in bodies {
        let program = with_d(body);
        let result = borrow_check(&program);
        assert!(
            result.is_ok(),
            "a re-initialisation on every path did not count: {:?}\n{}",
            result,
            program
        );
    }
}

/// EACH ARM STARTS FROM THE STATE BEFORE THE BRANCH. Both arms moving the same
/// field is one move on every path, not two moves; so is every arm of a
/// `match` doing it; and an arm that moves and then leaves the function does
/// not reach the code after the branch. All three were refused on 2563001 and
/// on 6a3d028 ("Use of moved value: d.inner" in the second arm, or after the
/// branch), because the walk ran the arms one after the other.
#[test]
fn test_each_arm_starts_from_the_state_before_the_branch() {
    let bodies = [
        "fn main() { let d: D = mk(1); let c: bool = true;
           if c { let a: In = d.inner; print_int(a.v); } else { let b: In = d.inner; print_int(b.v); } }",
        "fn main() { let d: D = mk(1); let k: i64 = 2;
           match k { 0 => { let a: In = d.inner; print_int(a.v); }
                     1 => { let b: In = d.inner; print_int(b.v); }
                     _ => { let e: In = d.inner; print_int(e.v); } } }",
        "fn f(c: bool) -> i64 { let d: D = mk(1);
           if c { let a: In = d.inner; return a.v; }
           let b: In = d.inner; return b.v; }
         fn main() { print_int(f(true)); }",
    ];
    for body in bodies {
        let program = with_d(body);
        let result = borrow_check(&program);
        assert!(
            result.is_ok(),
            "an arm saw another arm's move: {:?}\n{}",
            result,
            program
        );
    }
}

/// A LOOP BODY MAY RUN ZERO TIMES, OR STOP AT A `break` OR `continue` BEFORE
/// THE RE-INITIALISATION. The checker walks a loop body once (the iteration
/// question is #51), so it cannot see those paths; what it must not do is let
/// an assignment inside a loop body count after the loop. All four were
/// refused on 2563001 and COMPILED on 6a3d028.
#[test]
fn test_a_re_initialisation_inside_a_loop_does_not_count_after_it() {
    let bodies = [
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let mut i: i64 = 0;
           while i < 0 { d = mk(2); i = i + 1; } let b: In = d.inner; print_int(a.v + b.v); }",
        "fn main() { let mut d: D = mk(1); let a: In = d.inner; let c: bool = true;
           loop { if c { break; } d = mk(2); break; } let b: In = d.inner; print_int(a.v + b.v); }",
        "fn main() { let mut d: D = mk(1); let mut i: i64 = 0;
           while i < 2 { i = i + 1; let a: In = d.inner; print_int(a.v); if i == 2 { continue; } d = mk(5); }
           let b: In = d.inner; print_int(b.v); }",
        "fn main() { let mut d: D = mk(1); let c: bool = true;
           loop { let a: In = d.inner; print_int(a.v); if c { break; } d = mk(2); break; }
           let b: In = d.inner; print_int(b.v); }",
    ];
    for body in bodies {
        let program = with_d(body);
        let result = borrow_check(&program);
        assert!(
            is_use_of_moved_value(&result),
            "a re-initialisation inside a loop counted after it: {:?}\n{}",
            result,
            program
        );
    }
}

/// A PLACE CANNOT BE MOVED WHILE A LIVE BORROW REACHES INTO IT.
///
/// `mark_moved` erases every entry below the moved place, and on 6a3d028 that
/// included a child's `Borrowed` entry while the borrow itself stayed live —
/// so after a re-initialisation the child read as owned again. Measured on
/// 6a3d028, all COMPILED (2563001 refused all but the last two): the borrow of
/// `d.inner.w` lost to a move of `d.inner` (gpt-6-astra's
/// borrow_erased_by_parent_move; fable's c9b/c9c), the same with a whole-value
/// move (c9a), a re-initialised field whose child is borrowed and then moved
/// with its parent (c34), and the plain case of #52's F2b — moving `d` while
/// `d.inner` is borrowed, which 2563001 accepted as well.
#[test]
fn test_moving_a_place_under_a_live_borrow_is_refused() {
    let programs = [
        "struct W { n: i64 } struct In { w: W } struct D { inner: In }
         fn main() { let mut d: D = D { inner: In { w: W { n: 1 } } };
           let r: &W = &d.inner.w; let a: In = d.inner; let x: In = In { w: W { n: 2 } };
           d.inner = x; let f: W = d.inner.w; print_int(f.n); }",
        "struct W { n: i64 } struct In { w: W } struct D { inner: In }
         fn main() { let mut d: D = D { inner: In { w: W { n: 1 } } };
           let r: &W = &d.inner.w; let a: In = d.inner; d.inner = In { w: W { n: 2 } };
           let z: W = d.inner.w; print_int(z.n); }",
        "struct In { v: i64 } struct D { inner: In }
         fn main() { let mut d: D = D { inner: In { v: 1 } };
           let r: &In = &d.inner; let e: D = d; d = D { inner: In { v: 2 } };
           let f: In = d.inner; print_int(f.v + e.inner.v); }",
        "struct W { n: i64 } struct In { w: W } struct D { inner: In }
         fn main() { let mut d: D = D { inner: In { w: W { n: 1 } } };
           let a: In = d.inner; d.inner = In { w: W { n: 2 } };
           let r: &W = &d.inner.w; let b: In = d.inner; print_int(a.w.n + b.w.n); }",
        "struct In { v: i64 } struct D { inner: In }
         fn peek(i: &In) -> i64 { return i.v; }
         fn main() { let d: D = D { inner: In { v: 1 } };
           let r: &In = &d.inner; let e: D = d; print_int(peek(r) + e.inner.v); }",
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            is_move_out_of_borrowed_content(&result),
            "a place was moved under a live borrow: {:?}\n{}",
            result,
            program
        );
    }
}

/// A BORROW THAT ENDED IN A BLOCK DOES NOT COME BACK WITH THE BINDING A SHADOW
/// HID. `r` is taken inside the branch and dies at its `}`; a shadowing `let d`
/// after it hid the borrow from the release at that `}`, and `exit_scope` then
/// restored it. Measured: gpt-6-astra's shadow_restores_dead_borrow COMPILED on
/// 2563001 and was refused on 6a3d028; fable's c36 likewise (2563001 printed
/// `1 2 2`); the root-borrow variant was refused on both.
#[test]
fn test_a_borrow_that_ended_in_a_block_stays_ended_after_a_shadow() {
    let programs = [
        "struct In { v: i64 } struct D { inner: In }
         fn main() { let mut d: D = D { inner: In { v: 1 } }; let c: bool = true;
           if c { let r: &In = &d.inner; let d: D = D { inner: In { v: 2 } }; print_int(d.inner.v); }
           let f: In = d.inner; print_int(f.v); }",
        "struct In { v: i64 } struct D { inner: In }
         fn main() { let mut d: D = D { inner: In { v: 1 } }; let c: bool = true;
           if c { let r: &D = &d; let d: D = D { inner: In { v: 2 } }; print_int(d.inner.v); }
           let f: D = d; print_int(f.inner.v); }",
        "struct In { v: i64 }
         fn bump(i: &mut In) { i.v = i.v + 1; }
         fn main() { let mut d: In = In { v: 1 }; let c: bool = true;
           if c { let r: &In = &d; print_int(d.v); let d: In = In { v: 2 }; print_int(d.v); }
           bump(&mut d); print_int(d.v); }",
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            result.is_ok(),
            "a borrow that ended in a block came back after it: {:?}\n{}",
            result,
            program
        );
    }
}

/// AN ASSIGNMENT TO A PLACE A LIVE BORROW OVERLAPS IS REFUSED AT THE
/// ASSIGNMENT (#53), whatever the right-hand side and whichever side of the
/// overlap the borrow is on: the whole value under a borrow of the whole (with
/// the borrow then read, grok's e1 shape), a field under a borrow of its
/// parent, a parent under a borrow of its field, an element written through a
/// DYNAMIC index under a borrow of a fixed element (the dynamic index may be
/// that element), and fable's c32. On 2563001 every one of these passed the
/// borrow checker.
#[test]
fn test_an_assignment_under_a_live_borrow_is_refused() {
    let programs = [
        "struct In { v: i64 }
         fn main() { let mut d: In = In { v: 1 }; let e: In = In { v: 2 };
           let r: &In = &d; d = e; print_int(r.v); }",
        "struct In { v: i64 }
         fn main() { let mut d: In = In { v: 1 }; let r: &In = &d; d = In { v: 2 }; print_int(r.v); }",
        "struct In { v: i64 } struct D { inner: In }
         fn main() { let mut d: D = D { inner: In { v: 1 } }; let r: &D = &d;
           d.inner = In { v: 2 }; print_int(r.inner.v); }",
        "struct In { v: i64 } struct D { inner: In }
         fn main() { let mut d: D = D { inner: In { v: 1 } }; let r: &In = &d.inner;
           d = D { inner: In { v: 2 } }; print_int(r.v); }",
        "struct In { v: i64 }
         fn main() { let mut xs: [In; 2] = [In { v: 1 }, In { v: 2 }]; let r: &In = &xs[0];
           let i: i64 = 0; xs[i] = In { v: 5 }; print_int(r.v); }",
        "struct In { v: i64 } struct D { inner: In }
         fn main() { let mut d: D = D { inner: In { v: 1 } }; let e: D = D { inner: In { v: 2 } };
           let r: &In = &d.inner; d = e; let g: D = d; print_int(g.inner.v); }",
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            is_assignment_to_a_borrowed_place(&result),
            "an assignment under a live borrow was not refused at the assignment: {:?}\n{}",
            result,
            program
        );
    }
}

/// ...AND A BORROW THAT HAS ENDED DOES NOT BLOCK ONE. A call's argument borrow
/// ends with the call; a block's borrow ends at its `}` (also when a shadow
/// hid it); a borrow of a DIFFERENT field does not overlap the one assigned.
#[test]
fn test_an_assignment_after_its_borrow_ended_is_accepted() {
    let programs = [
        "struct In { v: i64 }
         fn peek(i: &In) -> i64 { return i.v; }
         fn main() { let mut d: In = In { v: 1 }; let e: In = In { v: 2 };
           print_int(peek(&d)); d = e; print_int(d.v); }",
        "struct In { v: i64 }
         fn main() { let mut d: In = In { v: 1 }; let c: bool = true;
           if c { let r: &In = &d; print_int(r.v); } d = In { v: 2 }; print_int(d.v); }",
        "struct In { v: i64 } struct D { inner: In }
         fn main() { let mut d: D = D { inner: In { v: 1 } }; let c: bool = true;
           if c { let r: &In = &d.inner; let d: D = D { inner: In { v: 2 } }; print_int(d.inner.v); }
           d.inner = In { v: 3 }; print_int(d.inner.v); }",
        "struct In { v: i64 } struct P { a: In, b: In }
         fn main() { let mut p: P = P { a: In { v: 1 }, b: In { v: 2 } }; let r: &In = &p.a;
           p.b = In { v: 3 }; print_int(r.v + p.b.v); }",
    ];
    for program in programs {
        let result = borrow_check(program);
        assert!(
            result.is_ok(),
            "an assignment was refused although no live borrow overlaps it: {:?}\n{}",
            result,
            program
        );
    }
}

/// THE DYNAMIC-INDEX KEY STAYS AS STRICT AS IT WAS. `xs[i] = …` is recorded
/// under one `xs[dynamic]` key (borrow_checker.rs, the `AssignTarget::Index`
/// arm), which does not say WHICH element was written, so it must not revive
/// `xs[0]` (grok's z1). Refused on 2563001 and on 6a3d028.
#[test]
fn test_a_dynamic_index_write_does_not_revive_a_fixed_element() {
    let result = borrow_check(
        "struct In { v: i64 }
         fn main() { let mut xs: [In; 2] = [In { v: 1 }, In { v: 2 }]; let a: In = xs[0];
           let i: i64 = 0; xs[i] = In { v: 5 }; let b: In = xs[0]; print_int(a.v + b.v); }",
    );
    assert!(
        is_use_of_moved_value(&result),
        "a write through a dynamic index revived a fixed element: {:?}",
        result
    );
}
