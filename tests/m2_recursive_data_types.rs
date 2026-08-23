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

// ---------------------------------------------------------------------------
// Declaration order: a program's validity must not depend on it
// ---------------------------------------------------------------------------

/// The reviewer's program, verbatim. `struct S { e: E }` above `enum E`.
///
/// The front end accepted this and gcc did not:
///
/// ```text
/// build_output/declorder.c:271:14: error: field has incomplete type 'struct E'
///   271 |     struct E e;
/// ```
///
/// Reversing the two declarations made it compile and run. So the program's
/// validity depended on the order its declarations happened to be written in,
/// and the failure landed in gcc rather than in a diagnostic — the same
/// "front end successful, gcc failed" shape the layout refusal exists to remove,
/// reached by a program with no recursion in it at all.
#[test]
fn a_struct_may_be_declared_above_the_enum_it_stores() {
    assert_eq!(
        compile_and_run(
            r#"
struct S { e: E }
enum E { A, B }
fn main() { let s: S = S { e: E::A }; print("ok"); }
"#,
            "declorder_struct_over_enum",
        )
        .expect("declaration order must not decide whether a program compiles"),
        "ok\n"
    );
}

/// The same question one container over: a struct above the struct it stores.
#[test]
fn a_struct_may_be_declared_above_the_struct_it_stores() {
    assert_eq!(
        compile_and_run(
            r#"
struct Outer { inner: Inner, tag: i64 }
struct Inner { x: i64 }
fn main() {
    let o: Outer = Outer { inner: Inner { x: 7 }, tag: 1 };
    print_int(o.inner.x);
}
"#,
            "declorder_struct_over_struct",
        )
        .expect("declaration order must not decide whether a program compiles"),
        "7\n"
    );
}

/// A chain, declared in exactly the wrong order end to end.
///
/// One swap can be got right by an ordering rule that only looks at pairs; a
/// three-link chain reversed needs a real traversal.
#[test]
fn a_reversed_containment_chain_is_still_emitted_dependencies_first() {
    assert_eq!(
        compile_and_run(
            r#"
struct A { b: B }
struct B { c: C }
struct C { n: i64 }
fn main() {
    let a: A = A { b: B { c: C { n: 42 } } };
    print_int(a.b.c.n);
}
"#,
            "declorder_chain",
        )
        .expect("declaration order must not decide whether a program compiles"),
        "42\n"
    );
}

/// EVERY order of one program's declarations gives one answer.
///
/// The property, rather than an instance of it. Three declarations have six
/// orders and exactly one of them is the order source-order emission needed; the
/// other five are what the old code got wrong, and asserting one of them would
/// have left the shape of the fix unpinned.
#[test]
fn every_declaration_order_of_one_program_gives_the_same_answer() {
    let decls = [
        "struct Holder { p: Pair }",
        "struct Pair { k: Kind, n: i64 }",
        "enum Kind { First, Second }",
    ];
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for (i, order) in orders.iter().enumerate() {
        let source = format!(
            "{}\n{}\n{}\nfn main() {{\n    let h: Holder = Holder {{ p: Pair {{ k: Kind::First, n: 9 }} }};\n    print_int(h.p.n);\n}}\n",
            decls[order[0]], decls[order[1]], decls[order[2]]
        );
        assert_eq!(
            compile_and_run(&source, &format!("declorder_perm_{}", i))
                .unwrap_or_else(|e| panic!("order {:?} was refused: {}", order, e)),
            "9\n",
            "order {:?} gave a different answer",
            order
        );
    }
}

/// The ordering is over the CUT graph, so a pointer payload does not constrain
/// it — and the recursive enum still works when declared before its user.
///
/// `enum V`'s own payload is a `struct V*`, which needs only the tag, so `V`
/// imposes no order on itself; `struct Root { v: V }` stores a `V` by value and
/// does. If the ordering had been derived from the UNCUT graph, `V -> V` would
/// have been read as a cycle and this program refused.
#[test]
fn a_pointer_payload_imposes_no_order_but_a_by_value_field_does() {
    assert_eq!(
        compile_and_run(
            r#"
struct Root { v: V, label: i64 }
enum V { Leaf(i64), Pair(V, V) }
fn sum(v: V) -> i64 {
    match v {
        V::Leaf(x) => { return x; }
        V::Pair(a, b) => { return sum(a) + sum(b); }
    }
}
fn main() {
    let l1: V = V::Leaf(4);
    let l2: V = V::Leaf(5);
    let r: Root = Root { v: V::Pair(l1, l2), label: 1 };
    print_int(sum(r.v));
}
"#,
            "declorder_cut_graph",
        )
        .expect("a cut payload slot must not constrain emission order"),
        "9\n"
    );
}

// ---------------------------------------------------------------------------
// The zero-length array: an excluded case, said out loud
// ---------------------------------------------------------------------------

/// `struct Z { xs: [Z; 0] }` is refused, and the message says the cycle IS
/// bounded and why the declaration is refused anyway.
///
/// The refusal itself is not the interesting half — the message is. The old one
/// said "Only an `enum` payload slot can be stored behind a pointer, and this
/// cycle has no such slot to store there", which reads as a theorem about what
/// can bound a recursive type, and as a theorem it is FALSE for this program:
/// `[Z; 0]` stores no `Z`, so the size is finite with no enum anywhere. A
/// diagnostic that names a mechanism the code does not implement is worse than a
/// terse one, because it is acted on.
#[test]
fn a_zero_length_array_self_reference_is_refused_and_the_message_says_why() {
    let err = compile(
        r#"
struct Z { xs: [Z; 0] }
fn main() { print("ok"); }
"#,
        "zero_len_self",
    )
    .expect_err("a `[Z; 0]` field is an array of an incomplete element type");

    assert!(
        err.contains("has no layout"),
        "expected a layout refusal: {}",
        err
    );
    for expected in [
        // The exclusion, named, with the row that actually carries it. N4-23 is
        // the non-overstatement row; N4-22 is the positive one and says nothing
        // about arrays. This string is the conformance fingerprint too, so the
        // two move together or `make conformance` goes red.
        "the refusal is a deliberate exclusion (requirement N4-23)",
        // Reason one: there is no C to emit.
        "incomplete element type",
        // Reason two: there is no value to lay out.
        "Empty array literals are not supported (cannot infer type)",
        // The concession that makes the message honest rather than a
        // restatement of the general rule -- and the absence of the head that
        // used to contradict it in the same run of sentences.
        "the size IS bounded",
    ] {
        assert!(
            err.contains(expected),
            "the refusal must say `{}`, got: {}",
            expected,
            err
        );
    }
}

/// No layout refusal claims that ONLY an enum can stop a cycle.
///
/// The false sentence, pinned by its text, over every shape that reaches the
/// refusal — including the ones for which it happens to be true. A theorem is
/// not allowed to be stated conditionally on the reader not having found the
/// counterexample.
#[test]
fn no_layout_refusal_claims_only_an_enum_can_bound_a_cycle() {
    for (source, name) in [
        (
            "struct Node { next: Node }\nfn main() { print_int(3); }",
            "false_thm_self",
        ),
        (
            "struct A { b: B }\nstruct B { a: A }\nfn main() { print_int(3); }",
            "false_thm_pair",
        ),
        (
            "struct Z { xs: [Z; 0] }\nfn main() { print(\"ok\"); }",
            "false_thm_zero",
        ),
        (
            "enum V { Leaf(i64), Many([V; 3]) }\nfn main() { print_int(3); }",
            "false_thm_array",
        ),
    ] {
        let err = compile(source, name).expect_err("this shape has no layout");
        assert!(
            !err.contains("Only an `enum` payload slot can be stored behind a pointer"),
            "`{}` still asserts the false theorem: {}",
            name,
            err
        );
        // EXACTLY ONE of the two explanations, and never both. The message used
        // to be a head plus an appended clause, and the head asserted "so this
        // compiler cannot give it a size" while the clause said "does bound the
        // size" — two contradictory causal claims about one program in one run of
        // sentences. It is built as one explanation per branch now, so asserting
        // that the branches are MUTUALLY EXCLUSIVE is what pins that.
        let unbounded = err.contains("so the size\n                     is unbounded")
            || err.contains("so the size is unbounded");
        let bounded = err.contains("the size IS bounded");
        assert!(
            unbounded != bounded,
            "`{}` must give exactly one account of the size, and gave {}: {}",
            name,
            if bounded { "both" } else { "neither" },
            err
        );
        assert!(
            err.contains("The one indirection this compiler introduces") || bounded,
            "`{}` should describe the mechanism this compiler has: {}",
            name,
            err
        );
    }
}

/// A zero-length array that is NOT on the cycle must not drag the carve-out
/// sentence into an unrelated refusal.
#[test]
fn the_zero_length_sentence_appears_only_when_the_cycle_uses_one() {
    let err = compile(
        r#"
struct Pad { zs: [i64; 0] }
struct Node { pad: Pad, next: Node }
fn main() { print_int(3); }
"#,
        "zero_len_elsewhere",
    )
    .expect_err("`Node` stores itself by value");
    assert!(
        err.contains("has no layout"),
        "expected a layout refusal: {}",
        err
    );
    assert!(
        !err.contains("zero-length arrays are OUT OF SCOPE"),
        "this cycle is Node -> Node and uses no zero-length array: {}",
        err
    );
}

/// An unevaluated array length is NOT treated as zero.
///
/// `[Z; N]` with a const parameter is a length this compiler has not computed,
/// and calling an unknown zero would be the unsound direction — it would label
/// the edge as bounded on no evidence.
#[test]
fn an_unevaluated_array_length_is_not_labelled_zero() {
    let err = compile(
        r#"
struct Z<const N: usize> { xs: [Z; N] }
fn main() { print_int(3); }
"#,
        "zero_len_const_param",
    )
    .expect_err("a self-storing struct is not laid out");
    // Asserted FIRST, so this test cannot pass by the program dying somewhere
    // else: a parse error contains no zero-length sentence either.
    assert!(
        err.contains("has no layout"),
        "this must reach the layout refusal to say anything about it: {}",
        err
    );
    assert!(
        !err.contains("zero-length arrays are OUT OF SCOPE"),
        "an unevaluated length must not be reported as zero: {}",
        err
    );
}

// ---------------------------------------------------------------------------
// The arena, past its first capacity
// ---------------------------------------------------------------------------

/// The arena's `realloc` path, which nothing else here reaches.
///
/// `__pd_rec_alloc` starts at capacity 0, takes 64 on the first allocation and
/// DOUBLES when the count reaches it. Every other test in this file builds a
/// handful of nodes, so all of them stop inside the first block and the growth
/// branch — the one place a pointer to already-recorded cells is moved — was
/// exercised by nothing. A manual sanitizer run is not regression protection;
/// this is.
///
/// Both sides of the boundary, because "it grew" and "it did not grow one
/// allocation too early" are two claims: 64 exactly fills the first block, and
/// 65 is the first allocation that cannot fit in it. The sums are the check —
/// a `realloc` that lost the cells would take the recursive walk through freed
/// or moved memory rather than merely returning early.
#[test]
fn the_recursive_arena_grows_past_its_first_capacity() {
    const LIST: &str = r#"
enum L { Nil, Cons(i64, L) }

fn total(l: L) -> i64 {
    match l {
        L::Nil => { return 0; }
        L::Cons(v, rest) => { return v + total(rest); }
    }
}

fn main() {
    let mut l: L = L::Nil;
    let mut i: i64 = 0;
    while i < COUNT {
        l = L::Cons(i, l);
        i = i + 1;
    }
    print_int(total(l));
}
"#;

    // n cells for n `Cons` nodes; sum is 0 + 1 + ... + (n-1).
    for (count, sum) in [(64, 2016), (65, 2080), (200, 19900)] {
        let source = LIST.replace("COUNT", &count.to_string());
        assert_eq!(
            compile_and_run(&source, &format!("arena_{}", count))
                .unwrap_or_else(|e| panic!("{} nodes: {}", count, e)),
            format!("{}\n", sum),
            "{} recursive cells did not survive the arena",
            count
        );
    }
}
