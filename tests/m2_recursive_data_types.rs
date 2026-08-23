//! M2: a recursive data type is laid out, or it is refused before any C exists.
//!
//! THE DEFECT THIS CLOSES, MEASURED ON `main` (8df1928) BEFORE THE FIX
//!
//! ```text
//! enum V { Leaf(i64), Pair(V, V) }
//! fn main() { print_int(1); }
//! ```
//!
//! reported `✅ Compilation successful!` — the parser, the type checker AND the
//! borrow checker all passed it — and then died in the C compiler:
//!
//! ```text
//! build_output/r.c:281:14: error: field has incomplete type 'struct V'
//!   281 |     struct V field0;
//! ```
//!
//! and constructing one was refused by the type checker with
//!
//! ```text
//! error: Type mismatch: expected V, found V
//! ```
//!
//! naming the same type on both sides, which no reader can act on. That second
//! symptom was NOT a recursion defect and is receipted separately below: it hit
//! every enum carrying an enum and every struct holding one, recursive or not.
//!
//! WHY THE ACCEPT SIDE IS MOST OF THIS FILE
//!
//! Half of the change is a REFUSAL, so its errors land on VALID programs — the
//! direction this repository has been bitten in twice. Every shape that must
//! still compile is asserted to compile, LINK and RUN against a number here,
//! because a program that compiles and prints the wrong answer is the defect
//! rather than the cure.
//!
//! WHAT IS DELIBERATELY NOT ASSERTED
//!
//! `scripts/check-c-returns.py` reports `non-void function may fall off its end`
//! for every function whose body is a `match`, because code generation lowers
//! `match` to an `if`/`else if` chain with no final `else`. That is a declared
//! open defect (`tests/stdlib/stdlib_tail_match.pd`, `known_violation`), it is
//! reproduced on the UNMODIFIED compiler with a non-recursive `match`, and it is
//! not owned here. The programs below are still linked and run, which is the
//! stronger check.

use palladium::linker::{link_command, OptLevel};
use palladium::{CompileError, Driver};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Headline + notes + suggestions, i.e. everything the user is shown.
fn rendered(e: CompileError) -> String {
    let d = e.to_diagnostic();
    let mut out = vec![d.message.clone()];
    out.extend(d.notes.iter().cloned());
    out.extend(d.suggestions.iter().map(|s| s.message.clone()));
    out.join("\n")
}

/// Compile, link against the real runtime, run, and return stdout.
fn compile_and_run(source: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    let exe = dir.path().join(name);
    fs::write(&src, source).unwrap();

    let c_file = Driver::new()
        .compile_file(&src)
        .map_err(|e| format!("compilation failed: {}", rendered(e)))?;

    let out = link_command(&c_file, &exe, OptLevel::Default)
        .map_err(|e| format!("link_command: {}", e))?
        .output()
        .map_err(|e| format!("gcc: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "gcc rejected the C: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    let run = Command::new(&exe)
        .output()
        .map_err(|e| format!("run: {}", e))?;
    if !run.status.success() {
        return Err(format!(
            "program failed: {}",
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).to_string())
}

/// Compile only, and return the emitted C on success.
fn compile_to_c(source: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    let c_file = Driver::new().compile_file(&src).map_err(rendered)?;
    Ok(fs::read_to_string(&c_file).unwrap())
}

/// Compile only, expecting refusal.
fn compile(source: &str, name: &str) -> Result<(), String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    Driver::new()
        .compile_file(&src)
        .map(|_| ())
        .map_err(rendered)
}

const TREE: &str = r#"
enum V { Leaf(i64), Pair(V, V) }

fn sum(v: V) -> i64 {
    match v {
        V::Leaf(x) => { return x; }
        V::Pair(a, b) => { return sum(a) + sum(b); }
    }
}

fn main() {
    let l1: V = V::Leaf(1);
    let l2: V = V::Leaf(2);
    let l3: V = V::Leaf(30);
    let inner: V = V::Pair(l1, l2);
    let root: V = V::Pair(inner, l3);
    print_int(sum(root));
}
"#;

// ---------------------------------------------------------------------------
// The accept side: recursive data now exists
// ---------------------------------------------------------------------------

/// The headline. A tree is built, taken apart by `match`, and summed.
///
/// The number matters: `33` is `1 + 2 + 30`, so a run that reaches the end
/// having shared or dropped the wrong subtree cannot pass by arriving at the
/// end at all.
#[test]
fn a_recursive_enum_compiles_links_and_runs() {
    let out = compile_and_run(TREE, "rec_tree").expect("a recursive enum must build and run");
    assert_eq!(out.trim(), "33", "sum over the tree");
}

/// A `match` arm binds a SUBTREE and recurses on it, which is the operation the
/// pointer layout exists for: the binding is a value, so it must read through
/// the cell rather than copy the cell.
#[test]
fn a_match_arm_binds_a_subtree_and_recurses_on_it() {
    let out = compile_and_run(
        r#"
enum V { Leaf(i64), Pair(V, V) }

fn depth(v: V) -> i64 {
    match v {
        V::Leaf(x) => { return 1; }
        V::Pair(a, b) => {
            let da: i64 = depth(a);
            let db: i64 = depth(b);
            if da > db { return da + 1; }
            return db + 1;
        }
    }
}

fn main() {
    let a: V = V::Leaf(1);
    let b: V = V::Leaf(2);
    let c: V = V::Leaf(4);
    let d: V = V::Pair(a, b);
    let e: V = V::Pair(d, c);
    print_int(depth(e));
}
"#,
        "rec_depth",
    )
    .expect("binding a subtree must work");
    assert_eq!(out.trim(), "3", "depth of ((leaf,leaf),leaf)");
}

/// A named-payload recursive variant, because the tuple form and the named form
/// are two emission sites and only one of them was exercised above.
#[test]
fn a_named_payload_recursive_variant_is_laid_out_too() {
    let out = compile_and_run(
        r#"
enum V { Leaf(i64), Node { left: V, right: V } }

fn sum(v: V) -> i64 {
    match v {
        V::Leaf(x) => { return x; }
        V::Node { left: l, right: r } => { return sum(l) + sum(r); }
    }
}

fn main() {
    let a: V = V::Leaf(4);
    let b: V = V::Leaf(38);
    let n: V = V::Node { left: a, right: b };
    print_int(sum(n));
}
"#,
        "rec_named",
    )
    .expect("a named recursive payload must build and run");
    assert_eq!(out.trim(), "42");
}

/// Mutual recursion that TERMINATES, through a struct.
///
/// This is why the constructors are emitted after every type definition rather
/// than each one straight after its own enum: `S` stores an `E` by value so `S`
/// must be defined second, and `E_Node__new` takes a `struct S` by value and
/// asks for its `sizeof`, so it must come after `S`. In place, gcc reported
/// `variable has incomplete type 'struct S'`.
#[test]
fn mutual_recursion_through_a_struct_is_laid_out() {
    let out = compile_and_run(
        r#"
enum E { Leaf(i64), Node(S) }
struct S { e: E }

fn main() {
    let base: E = E::Leaf(7);
    let wrapper: S = S { e: base };
    let outer: E = E::Node(wrapper);
    match outer {
        E::Leaf(x) => { print_int(0); }
        E::Node(s) => { print_int(1); }
    }
}
"#,
        "rec_mutual",
    )
    .expect("a terminating mutual recursion must build and run");
    assert_eq!(out.trim(), "1");
}

// ---------------------------------------------------------------------------
// The control: what the emitted C must look like, so a revert shows up here
// ---------------------------------------------------------------------------

/// The layout, the allocation and the read, asserted on the emitted C.
///
/// This is the test that goes RED on a revert of any one of the four payload
/// emission sites, INDEPENDENTLY of whether gcc happens to accept the result.
#[test]
fn the_recursive_slot_is_a_pointer_that_is_allocated_and_read_through() {
    let c = compile_to_c(TREE, "rec_shape").expect("the tree must compile");

    assert!(
        c.contains("struct V* field0;") && c.contains("struct V* field1;"),
        "the recursive payload slots must be pointers; emitted:\n{}",
        c
    );
    assert!(
        !c.contains("    struct V field0;"),
        "no recursive payload slot may be stored by value — that is the incomplete \
         type gcc refused;\nemitted:\n{}",
        c
    );
    assert!(
        c.contains("result.data.Pair.field0 = (struct V*)__pd_rec_alloc(sizeof(struct V));")
            && c.contains("*result.data.Pair.field0 = arg0;"),
        "the constructor must take a cell and store the value into it;\nemitted:\n{}",
        c
    );
    assert!(
        c.contains("= *_match_expr.data.Pair.field0;"),
        "a match binding must read THROUGH the cell;\nemitted:\n{}",
        c
    );
    assert!(
        c.contains("atexit(__pd_cleanup_rec_nodes);"),
        "the cells must be registered for release, like the string pool;\nemitted:\n{}",
        c
    );
}

/// A program with no recursive type must not gain the arena.
///
/// The emitted C is the artefact this compiler is judged on, and 125 of the 128
/// tracked programs the front end accepts declare no enum at all. If the arena
/// appeared unconditionally, the corpus-wide C differential would stop being a
/// measurement of this change.
#[test]
fn a_program_with_no_recursive_type_gains_no_arena() {
    let c = compile_to_c(
        r#"
enum Colour { Red, Green }
struct P { x: i64, y: i64 }
fn main() {
    let p: P = P { x: 1, y: 2 };
    print_int(p.x + p.y);
}
"#,
        "no_rec",
    )
    .expect("an ordinary program must compile");
    assert!(
        !c.contains("__pd_rec_alloc") && !c.contains("__pd_cleanup_rec_nodes"),
        "a program with no recursive payload must emit no recursive-value arena"
    );
}

// ---------------------------------------------------------------------------
// Polarity: the refusal must not reach any of these
// ---------------------------------------------------------------------------

/// A non-recursive enum with a payload. The most ordinary shape there is.
#[test]
fn a_non_recursive_enum_with_a_payload_still_compiles() {
    let out = compile_and_run(
        r#"
enum V { Leaf(i64), Two(i64, i64) }
fn sum(v: V) -> i64 {
    match v {
        V::Leaf(x) => { return x; }
        V::Two(a, b) => { return a + b; }
    }
}
fn main() {
    print_int(sum(V::Leaf(11)));
    print_int(sum(V::Two(30, 4)));
}
"#,
        "nonrec_payload",
    )
    .expect("a non-recursive enum with a payload must still work");
    assert_eq!(out.split_whitespace().collect::<Vec<_>>(), vec!["11", "34"]);
}

/// An enum carrying a DIFFERENT enum, with no recursion anywhere.
///
/// THIS NEVER COMPILED. On `main` it was refused with
/// `Type mismatch: expected W, found W`, because the payload was registered
/// through `CheckerType::from` (which calls every named type a struct) while
/// the expression producing it was typed through the context-aware path. It is
/// in this file because it is the same fix, not because it is recursive.
#[test]
fn an_enum_may_carry_another_enum() {
    let out = compile_and_run(
        r#"
enum W { A, B }
enum V { Leaf(i64), Wrap(W) }
fn main() {
    let w: W = W::A;
    let v: V = V::Wrap(w);
    match v {
        V::Leaf(x) => { print_int(0); }
        V::Wrap(inner) => { print_int(5); }
    }
}
"#,
        "enum_in_enum",
    )
    .expect("an enum payload of enum type must compile");
    assert_eq!(out.trim(), "5");
}

/// A struct holding an enum. Same defect, one container over, also never
/// compiled.
#[test]
fn a_struct_may_hold_an_enum() {
    let out = compile_and_run(
        r#"
enum W { A, B }
struct S { w: W, n: i64 }
fn main() {
    let s: S = S { w: W::A, n: 9 };
    print_int(s.n);
}
"#,
        "enum_in_struct",
    )
    .expect("a struct field of enum type must compile");
    assert_eq!(out.trim(), "9");
}

/// A type that merely MENTIONS itself in a signature is not a recursive type.
/// Nothing about `fn combine(x: V, y: V) -> V` puts a `V` inside a `V`.
#[test]
fn a_type_that_only_mentions_itself_in_a_signature_is_untouched() {
    let out = compile_and_run(
        r#"
enum V { A, B }
fn pick(x: V, y: V) -> V { return x; }
fn main() {
    let a: V = V::A;
    let chosen: V = pick(a, V::B);
    match chosen {
        V::A => { print_int(1); }
        V::B => { print_int(2); }
    }
}
"#,
        "mentions_self",
    )
    .expect("a self-mentioning signature must not be refused");
    assert_eq!(out.trim(), "1");
}

/// A struct stored by value inside another struct, with no cycle.
#[test]
fn a_struct_may_still_store_a_struct_by_value() {
    let out = compile_and_run(
        r#"
struct Inner { a: i64 }
struct Outer { inner: Inner, b: i64 }
fn main() {
    let i: Inner = Inner { a: 20 };
    let o: Outer = Outer { inner: i, b: 22 };
    print_int(o.inner.a + o.b);
}
"#,
        "struct_in_struct",
    )
    .expect("nested structs must still compile");
    assert_eq!(out.trim(), "42");
}

/// A struct stored by value in an enum payload, no cycle. The cut must not fire
/// on a payload merely because it is a named type.
#[test]
fn an_enum_may_still_store_a_struct_by_value() {
    let out = compile_and_run(
        r#"
struct P { x: i64 }
enum V { Leaf(i64), Wrap(P) }
fn main() {
    let p: P = P { x: 12 };
    let v: V = V::Wrap(p);
    match v {
        V::Leaf(n) => { print_int(0); }
        V::Wrap(q) => { print_int(q.x); }
    }
}
"#,
        "struct_in_enum",
    )
    .expect("a struct payload must stay by value");
    assert_eq!(out.trim(), "12");
}

// ---------------------------------------------------------------------------
// The refusal: what genuinely has no layout
// ---------------------------------------------------------------------------

/// A struct that stores itself. No enum on the cycle, so nothing can stop, so
/// no value of it can ever exist — refused by name instead of by gcc.
#[test]
fn a_struct_that_stores_itself_is_refused_by_the_type_checker() {
    let err = compile(
        r#"
struct Node { val: i64, next: Node }
fn main() { print_int(3); }
"#,
        "rec_struct",
    )
    .expect_err("a struct storing itself has no layout");

    assert!(
        err.contains("recursive type `Node` has no layout"),
        "the refusal must name the type: {}",
        err
    );
    assert!(
        err.contains("Node -> Node"),
        "the refusal must show the cycle rather than assert it: {}",
        err
    );
    assert!(
        !err.contains("incomplete type"),
        "the refusal must be the compiler's, not gcc's: {}",
        err
    );
}

/// Two structs storing each other. Same reason, one hop longer, and the cycle
/// in the message has to show both.
#[test]
fn a_struct_cycle_with_no_enum_on_it_is_refused() {
    let err = compile(
        r#"
struct A { b: B }
struct B { a: A }
fn main() { print_int(3); }
"#,
        "rec_struct_pair",
    )
    .expect_err("a struct cycle has no layout");
    assert!(
        err.contains("has no layout"),
        "expected a layout refusal, got: {}",
        err
    );
    assert!(
        err.contains("A -> B -> A") || err.contains("B -> A -> B"),
        "the cycle must be shown: {}",
        err
    );
}

/// Recursion reached through an ARRAY is refused rather than laid out.
///
/// The cut makes a SLOT a pointer. `[V; 3]` would need the ELEMENT to become
/// one, which is a different edit to a different declarator, so this scheme
/// does not claim it — and the direction it fails in is the refusing one.
#[test]
fn recursion_through_an_array_is_refused_rather_than_guessed_at() {
    let err = compile(
        r#"
enum V { Leaf(i64), Many([V; 3]) }
fn main() { print_int(3); }
"#,
        "rec_array",
    )
    .expect_err("array-mediated recursion is not laid out by this scheme");
    assert!(
        err.contains("has no layout"),
        "expected a layout refusal, got: {}",
        err
    );
}

// ---------------------------------------------------------------------------
// The diagnostic itself
// ---------------------------------------------------------------------------

/// No diagnostic may name the same type on both sides.
///
/// Asserted over the shapes that produced it — constructing a recursive enum,
/// an enum carrying an enum, and a struct holding one — rather than over the
/// message text of one of them, because the defect was a KIND confusion that
/// every named type was exposed to.
#[test]
fn no_diagnostic_names_the_same_type_on_both_sides() {
    for (source, name) in [
        (
            "enum V { Leaf(i64), Pair(V, V) }\nfn main() { let a: V = V::Leaf(1); let b: V = V::Leaf(2); let p: V = V::Pair(a, b); print_int(3); }",
            "diag_rec",
        ),
        (
            "enum W { A, B }\nenum V { Wrap(W) }\nfn main() { let w: W = W::A; let v: V = V::Wrap(w); print_int(3); }",
            "diag_enum_in_enum",
        ),
        (
            "enum W { A, B }\nstruct S { w: W }\nfn main() { let s: S = S { w: W::A }; print_int(3); }",
            "diag_enum_in_struct",
        ),
    ] {
        match compile(source, name) {
            Ok(()) => {}
            Err(err) => {
                for word in ["V", "W"] {
                    assert!(
                        !err.contains(&format!("expected {}, found {}", word, word)),
                        "`{}` produced a diagnostic naming one type on both sides: {}",
                        name,
                        err
                    );
                }
                panic!("`{}` should compile, but was refused: {}", name, err);
            }
        }
    }
}
