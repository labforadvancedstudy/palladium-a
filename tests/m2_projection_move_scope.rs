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

/// Re-initialising a base must not erase a LIVE borrow below it: `r` still
/// borrows `d.inner`, so moving out of `d.inner` stays refused whatever was
/// assigned to `d` in between.
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
        matches!(
            result.as_ref().err().map(CompileError::peel),
            Some(CompileError::CannotMoveOutOfBorrowedContent { .. })
        ),
        "re-initialising a base dropped a live borrow of its field: {:?}",
        result
    );
}

/// A MOVE OF THE PARENT OUTRANKS A BORROW OF THE CHILD THAT OUTLIVES IT.
///
/// `r` borrows `d.inner.w` until the branch closes, `d.inner` is moved while
/// it does, and the branch's exit then ends the borrow — which used to
/// write `d.inner.w: Owned` back over a field whose parent had been moved
/// out. Measured before: compiled and printed `1 1`, the second read
/// through a moved-out `d.inner`.
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
        is_use_of_moved_value(&result),
        "a field of a moved parent was revived by the end of its borrow: {:?}",
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
/// spelled. `r` is still live, so moving out of what it borrows stays refused.
/// The literal spellings were refused before and must stay refused once an
/// assignment from a literal writes state; the PLACE spellings were ACCEPTED
/// before (measured: both printed 2), because writing `Owned` to the target
/// overwrote the borrow — the spelling of the right-hand side decided the
/// verdict.
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
            matches!(
                result.as_ref().err().map(CompileError::peel),
                Some(CompileError::CannotMoveOutOfBorrowedContent { .. })
            ),
            "an assignment ended a live borrow: {:?}\n{}",
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
