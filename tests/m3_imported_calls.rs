//! M3: calling a function that came from an `import`.
//!
//! The defect: **no program that calls an imported function compiled.**
//!
//! ```text
//! lib2.pd:  pub fn helper() -> i64 { return 5; }
//! call.pd:  import lib2;
//!           fn main() { print_int(helper()); }
//! -> error: Use of uninitialized value: helper
//! ```
//!
//! The type checker was handed the resolver's output (`src/driver/mod.rs:104-107`,
//! `type_checker.set_imported_modules(...)`), so `helper` type-checked. The borrow
//! checker was constructed with `BorrowChecker::new()` and handed the *pre-resolution*
//! AST (`src/driver/mod.rs:137`, `src/driver/mod.rs:163`) while `resolved_modules` sat
//! live and unused in the same scope. Its function table was seeded from
//! `crate::builtins::BUILTINS` and nothing else
//! (`src/ownership/borrow_checker.rs:157-160`), and its first pass walks
//! `program.items` only — `Program.imports` (`src/ast/mod.rs:9`) is never read, and
//! `Item` (`src/ast/mod.rs:24-40`) has no `Import` variant, so no walk over items could
//! have reached one. `helper()` therefore fell out of the function table at
//! `Expr::Ident` (`src/ownership/borrow_checker.rs:904`), was looked up as a *value*,
//! was not found, and died at `src/ownership/borrow_checker.rs:957` as
//! `UseOfUninitializedValue`.
//!
//! The pass was not wrong; it was structurally single-file. These tests drive the real
//! `pdc` binary over two real files in a scratch directory, because the resolver reads
//! the filesystem and the claim under test is about a two-file program.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Compile `main.pd` in a scratch directory that also holds the given modules,
/// then run the produced executable. Returns (compiler output, run result).
///
/// `current_dir` is the scratch directory because the module resolver searches
/// `.` (`src/resolver/mod.rs:37`) and the driver writes `build_output/` relative
/// to the working directory — so both the input side and the output side of this
/// test belong to a directory it owns.
fn compile_and_run(modules: &[(&str, &str)], main_src: &str) -> (bool, String, Option<String>) {
    let dir = TempDir::new().unwrap();
    for (name, src) in modules {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, src).unwrap();
    }
    fs::write(dir.path().join("main.pd"), main_src).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .args(["compile", "main.pd", "-o", "prog"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run pdc");

    let compiler_output = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let compiled = out.status.success();

    let exe = dir.path().join("build_output").join("prog");
    let stdout = if compiled && Path::new(&exe).exists() {
        let run = Command::new(&exe)
            .current_dir(dir.path())
            .output()
            .expect("failed to run the compiled program");
        Some(String::from_utf8_lossy(&run.stdout).to_string())
    } else {
        None
    };

    (compiled, compiler_output, stdout)
}

/// The measured defect, verbatim. Fails on `main` with
/// `Use of uninitialized value: helper`.
#[test]
fn test_calling_an_imported_function_compiles() {
    let (compiled, output, _) = compile_and_run(
        &[("lib2.pd", "pub fn helper() -> i64 { return 5; }\n")],
        "import lib2;\n\nfn main() {\n    print_int(helper());\n}\n",
    );
    assert!(
        compiled,
        "a call to an imported function was rejected; compiler said:\n{}",
        output
    );
    assert!(
        !output.contains("Use of uninitialized value"),
        "the borrow checker still treats the imported callee as a variable:\n{}",
        output
    );
}

/// Compiling is not the claim; *running correctly* is. A borrow-check fix that
/// let the program through while codegen emitted no body for `helper` would
/// satisfy the test above and still hand the user garbage — which is exactly the
/// failure mode this repo has been burned by (D3 tail return).
#[test]
fn test_calling_an_imported_function_prints_its_result() {
    let (compiled, output, stdout) = compile_and_run(
        &[("lib2.pd", "pub fn helper() -> i64 { return 5; }\n")],
        "import lib2;\n\nfn main() {\n    print_int(helper());\n}\n",
    );
    assert!(compiled, "compilation failed:\n{}", output);
    assert_eq!(
        stdout.as_deref(),
        Some("5\n"),
        "the imported function did not run; compiler said:\n{}",
        output
    );
}

/// THE POSITIVE TWIN, and the reason the two tests above are not enough.
///
/// Both of them call `helper()` with ZERO arguments, so they pass under any
/// registration that merely puts the name in the table — a name-only insert
/// would satisfy both. What registration is actually FOR is the signature:
/// `collect_function_sig` maps `mut x: i64` to `ParamOwnership::BorrowMut`, and
/// `check_call_args` then refuses an immutable argument in that position. None of
/// that was asserted for an imported function, so the new check could have been
/// passing while the thing it checks was false.
///
/// This is the assertion that the imported SIGNATURE carries ownership
/// information across the file boundary: the call must be refused, and refused
/// with the borrow checker's own message. Its control is that reverting
/// `register_imported_functions` changes the diagnostic to
/// "Use of uninitialized value: bad" — a different refusal, for the wrong reason,
/// which is why the assertion is on the message and not merely on `!compiled`.
#[test]
fn test_an_imported_signature_carries_its_ownership_requirement() {
    let (compiled, output, stdout) = compile_and_run(
        &[("lib2.pd", "pub fn bad(mut x: i64) { x = 42; }\n")],
        "import lib2;\n\nfn main() {\n    let n = 1;\n    bad(n);\n    print_int(n);\n}\n",
    );
    assert!(
        !compiled,
        "an imported `mut` parameter accepted an immutable argument; it printed {:?}",
        stdout
    );
    assert!(
        output.contains("cannot borrow `n` as mutable"),
        "the refusal must be the borrow checker reading the IMPORTED signature, \
         not a missing-name error; compiler said:\n{}",
        output
    );
}

// ---------------------------------------------------------------------------
// Name resolution: a binding is not a function
// ---------------------------------------------------------------------------
//
// Registering imported signatures put every name any imported module exports into
// the borrow checker's function table, and that table was consulted BEFORE the
// ownership context when an identifier was read. So the fix's own surface became
// a way to launder a local binding past the move check.
//
// Both tests below are live assertions, not declarations: the first fails on this
// branch without the ordering fix, and the second fails on `main` too — the hole
// already existed for a local `fn` of the same name, and importing merely widened
// it to every exported name in the program's dependencies.

/// The imported form: `helper` is a local `S` that has been moved, and `lib2`
/// exports a function of the same name.
///
/// Measured on this branch before the ordering fix: compiles, links, prints 1.
/// Measured on `main` (where no imported signature is registered): refused with
/// "Use of moved value: helper". Return position matters — `let`-RHS and
/// call-argument positions have their own move handling and never reach the
/// identifier check.
#[test]
fn test_a_local_binding_is_not_laundered_by_an_imported_function() {
    let (compiled, output, stdout) = compile_and_run(
        &[("lib2.pd", "pub fn helper() -> i64 { return 5; }\n")],
        "import lib2;\n\n\
         struct S { v: i64 }\n\n\
         fn f() -> S {\n    let helper: S = S { v: 1 };\n    let b = helper;\n    return helper;\n}\n\n\
         fn main() { print_int(f().v); }\n",
    );
    assert!(
        !compiled,
        "a moved local was accepted because an imported module exports a function \
         of the same name; it printed {:?}",
        stdout
    );
    assert!(
        output.contains("Use of moved value"),
        "expected the move checker's refusal; compiler said:\n{}",
        output
    );
}

/// The local form of the same hole, which predates this branch: no import at all,
/// just a local `fn helper` for the binding's name to collide with. Measured on
/// `main`: "✅ Compilation successful!".
///
/// It is here rather than in a file of its own because it is the same line of
/// code and the same fix, and separating them would let one be repaired while the
/// other silently returned.
#[test]
fn test_a_local_binding_is_not_laundered_by_a_local_function() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "struct S { v: i64 }\n\n\
         fn helper() -> i64 { return 5; }\n\n\
         fn f() -> S {\n    let helper: S = S { v: 1 };\n    let b = helper;\n    return helper;\n}\n\n\
         fn main() { print_int(f().v); }\n",
    );
    assert!(
        !compiled,
        "a moved local was accepted because a function of the same name exists; \
         it printed {:?}",
        stdout
    );
    assert!(
        output.contains("Use of moved value"),
        "expected the move checker's refusal; compiler said:\n{}",
        output
    );
}

/// The widest form of the same hole: no user function at all, just a BUILT-IN
/// for the binding's name to collide with.
///
/// `BorrowChecker::default` preloads `functions` from `crate::builtins`, so
/// every built-in name is a laundering site in a program that defines nothing.
/// Measured on `main`: "✅ Compilation successful!", and the executable printed
/// `1` — a use-after-move that ran.
///
/// It is asserted directly, rather than left implied by the two tests above,
/// because the two above route through `functions` entries that a *program*
/// puts there, and this one routes through entries the *compiler* puts there.
/// A change to built-in registration could reopen this without touching either.
#[test]
fn test_a_local_binding_is_not_laundered_by_a_builtin() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "struct S { v: i64 }\n\n\
         fn f() -> S {\n    let print: S = S { v: 1 };\n    let b = print;\n    return print;\n}\n\n\
         fn main() { print_int(f().v); }\n",
    );
    assert!(
        !compiled,
        "a moved local was accepted because a BUILT-IN of the same name exists; \
         it printed {:?}",
        stdout
    );
    assert!(
        output.contains("Use of moved value"),
        "expected the move checker's refusal; compiler said:\n{}",
        output
    );
}

// ---------------------------------------------------------------------------
// The other polarity: the ownership context is not allowed to outlive its scope
// ---------------------------------------------------------------------------
//
// The four tests above make a name that IS a binding here beat a function of the
// same name. That is only correct while "has an entry in the ownership map"
// means "is a binding here" — and it did not. `OwnershipContext::ownership` was
// a flat map that nothing ever removed from, and `exit_scope` dropped only
// borrows whose lifetime was `Lifetime::Scope(n)`, a variant constructed NOWHERE
// in the tree (its only two mentions were that comparison and a `Display` arm).
// So both halves of the context accumulated for the whole of `check_program`.
//
// Made the ordering rule above into a false REJECT, which is the worse polarity
// and the second time this branch traded one for the other. The three tests
// below are the controls for the scope fix: each is a VALID program that must
// compile and run, and each was refused before `exit_scope` learned to retire
// the bindings and borrows its scope created.

/// The cross-function form. `a()` moves a local named `helper`; `main` never
/// binds that name at all, it only CALLS the real `fn helper`.
///
/// This is why "moved states are self-healing, every binder re-inits" does not
/// save it: `main` has no binder for `helper` to heal. Measured before the fix:
/// "error: Use of moved value: helper" — pointing at a call to a function.
/// Measured on `main`: compiles and prints `1` then `5`.
#[test]
fn test_a_move_in_one_function_does_not_poison_a_call_in_another() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "struct S { v: i64 }\n\n\
         fn helper() -> i64 { return 5; }\n\n\
         fn a() -> i64 {\n    let helper: S = S { v: 1 };\n    let b: S = helper;\n    return b.v;\n}\n\n\
         fn main() { print_int(a()); print_int(helper()); }\n",
    );
    assert!(
        compiled,
        "a move inside `a` made `main`'s call to the real `helper` fail; \
         compiler said:\n{}",
        output
    );
    assert_eq!(
        stdout.as_deref(),
        Some("1\n5\n"),
        "expected `a()` then the real `helper()`; compiler said:\n{}",
        output
    );
}

/// The same defect one scope in: the move happens in an `if` body, and the call
/// is in the enclosing function scope. There is no bare-block form to test —
/// `{ … }` as a statement is not in the grammar — so `if` is the narrowest
/// block this can be written with.
///
/// Kept separate from the cross-function test because the two are fixed by
/// different halves of `exit_scope`: this one needs the per-scope declaration
/// frame, and would still fail if only the function boundary were cleared.
#[test]
fn test_a_move_in_a_block_does_not_poison_a_call_after_it() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "struct S { v: i64 }\n\n\
         fn helper() -> i64 { return 5; }\n\n\
         fn main() {\n    \
         if true { let helper: S = S { v: 1 }; let b: S = helper; print_int(b.v); }\n    \
         print_int(helper());\n}\n",
    );
    assert!(
        compiled,
        "a move inside an `if` body made the call after it fail; compiler said:\n{}",
        output
    );
    assert_eq!(
        stdout.as_deref(),
        Some("1\n5\n"),
        "expected the block's value then the real `helper()`; compiler said:\n{}",
        output
    );
}

/// The borrow half, which the move tests above cannot see.
///
/// `borrows` accumulated for the same reason `ownership` did, and nothing ends a
/// borrow taken outside argument position — `Expr::Reference` takes
/// `new_lifetime()` and only `check_call_args` ever calls `end_borrows`. So two
/// sibling functions that each borrow their OWN `v` collided with each other.
///
/// Measured on `main` (728779b), so this is a pre-existing false reject rather
/// than one this branch introduced: "error: Conflicting borrows: cannot borrow
/// `v` as mutable because it is also borrowed as mutable" — naming a `v` in a
/// different function.
#[test]
fn test_a_borrow_in_one_function_does_not_conflict_with_the_next() {
    let (compiled, output, _stdout) = compile_and_run(
        &[],
        "fn a() { let mut v: i64 = 1; let r: &mut i64 = &mut v; }\n\n\
         fn b() { let mut v: i64 = 1; let r: &mut i64 = &mut v; }\n\n\
         fn main() { a(); b(); }\n",
    );
    assert!(
        compiled,
        "`a`'s borrow of its own `v` outlived `a` and conflicted with `b`'s \
         borrow of a different `v`; compiler said:\n{}",
        output
    );
}

/// The control for the three tests above, and the reason they are not simply
/// "make the borrow checker quieter".
///
/// Retiring a scope's bindings must not retire the EFFECTS a scope had on
/// bindings that outlive it. Here the `if` body moves the enclosing `s`, so `s`
/// is genuinely gone afterwards and the use must still be refused. Restoring a
/// snapshot of the ownership map at scope exit — the obvious implementation of
/// "undo the scope" — passes all three tests above and turns this one into an
/// accepted use-after-move.
#[test]
fn test_a_move_out_of_an_enclosing_binding_survives_the_block() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "struct S { v: i64 }\n\n\
         fn main() {\n    \
         let s: S = S { v: 1 };\n    \
         if true { let t: S = s; print_int(t.v); }\n    \
         print_int(s.v);\n}\n",
    );
    assert!(
        !compiled,
        "the `if` body moved `s`, and the use after the block was accepted; \
         it printed {:?}",
        stdout
    );
    assert!(
        output.contains("Use of moved value"),
        "expected the move checker's refusal; compiler said:\n{}",
        output
    );
}

/// The name is not the binding, and this is what proves the scope machinery
/// knows it.
///
/// `Place::Local` is a NAME. In `let s: S = s;` the source and the destination
/// are therefore the same key, and `move_value` writes `Moved` to the source
/// and then `Owned` to the destination — the second write cancelling the first.
/// Nested in a block that shadows an outer `s`, the binder had already
/// snapshotted the outer `Owned`, so scope exit restored it and the outer
/// binding survived being moved out of.
///
/// Measured before the fix, and the pair is the whole point — the ONLY
/// difference between these two programs is whether the inner binding reuses
/// the outer's name:
///
///     if true { let s: S = s; }  then use outer s  ->  ACCEPTED
///     if true { let u: S = s; }  then use outer s  ->  Use of moved value: s
///
/// The differently-named half already had a control
/// (`test_a_move_out_of_an_enclosing_binding_survives_the_block`), which is
/// exactly why this went unseen: every move test in this file happened to avoid
/// the one case where the name collides.
#[test]
fn test_a_same_named_shadow_does_not_launder_the_move() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "struct S { v: i64 }\n\n\
         fn main() {\n    \
         let s: S = S { v: 1 };\n    \
         if true { let s: S = s; print_int(s.v); }\n    \
         print_int(s.v);\n}\n",
    );
    assert!(
        !compiled,
        "an inner binding that REUSES the outer name moved out of it and the \
         outer binding survived; it printed {:?}",
        stdout
    );
    assert!(
        output.contains("Use of moved value"),
        "expected the move checker's refusal; compiler said:\n{}",
        output
    );
}

/// The guard for the test above: shadowing itself must stay legal.
///
/// A fix that refused every same-named inner binding would pass the test above
/// and break this one. Here the inner `s` is initialized from a FRESH value, so
/// the outer `s` is never moved out of and must still be usable after the block.
#[test]
fn test_a_same_named_shadow_of_a_fresh_value_leaves_the_outer_binding_alone() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "struct S { v: i64 }\n\n\
         fn main() {\n    \
         let s: S = S { v: 1 };\n    \
         if true { let s: S = S { v: 2 }; print_int(s.v); }\n    \
         print_int(s.v);\n}\n",
    );
    assert!(
        compiled,
        "shadowing with a fresh value was refused; compiler said:\n{}",
        output
    );
    assert_eq!(
        stdout.as_deref(),
        Some("2\n1\n"),
        "expected the inner binding then the untouched outer one; compiler said:\n{}",
        output
    );
}

// ---------------------------------------------------------------------------
// The imported bodies that get EMITTED are the ones that get CHECKED
// ---------------------------------------------------------------------------
//
// The third pass below skipped every generic imported body, on the stated
// ground that codegen emits only public NON-GENERIC imported functions
// (`src/codegen/mod.rs:1776-1776`) and so a skipped body "produces no C". That
// is true of the DIRECT imported-emission path and FALSE of monomorphization,
// which is a separate path emitting `name__T` from the same template. The
// guarantee was read off the stated reason instead of off the mechanism, and
// the result was a fail-open.
//
// The predicate is now the emission set itself: a generic imported body is
// checked exactly when the compilation instantiates it, which is the list
// codegen monomorphizes from (`TypeChecker::get_instantiations`, handed over
// in `src/driver/mod.rs`).

/// A plain use-after-move inside an imported GENERIC body.
///
/// Measured under the skip-all-generics guard: compiled, emitted `bad__i64`
/// three times, linked, and printed `7`.
#[test]
fn test_a_use_after_move_in_an_instantiated_imported_generic_is_refused() {
    let (compiled, output, stdout) = compile_and_run(
        &[(
            "g.pd",
            "pub struct S { v: i64 }\n\
             pub fn bad<T>(x: T) -> i64 { let a: S = S{v:7}; let b: S = a; let c: S = a; return c.v; }\n",
        )],
        "import g;\n\nfn main() { print_int(bad(1)); }\n",
    );
    assert!(
        !compiled,
        "a use-after-move inside an imported generic body was accepted, \
         monomorphized and run; it printed {:?}",
        stdout
    );
    assert!(
        output.contains("Use of moved value"),
        "expected the move checker's refusal; compiler said:\n{}",
        output
    );
}

/// The LOCAL twin, which is what makes the test above a statement about
/// imports rather than about generics.
///
/// This one is refused on `main` too. If it ever stops being refused, the test
/// above is no longer measuring an import-specific hole.
#[test]
fn test_the_local_twin_of_the_generic_use_after_move_is_also_refused() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "struct S { v: i64 }\n\
         fn bad<T>(x: T) -> i64 { let a: S = S{v:7}; let b: S = a; let c: S = a; return c.v; }\n\n\
         fn main() { print_int(bad(1)); }\n",
    );
    assert!(
        !compiled,
        "the local generic twin was accepted; it printed {:?}",
        stdout
    );
    assert!(
        output.contains("Use of moved value"),
        "expected the move checker's refusal; compiler said:\n{}",
        output
    );
}

/// The guard on the other side, and the reason the predicate is "instantiated"
/// rather than "check every generic body".
///
/// Imported ASTs are never macro-expanded — the driver expands the top-level
/// AST before it resolves modules — so walking a generic body that holds a
/// macro reports the internal "macros should be expanded before this phase".
/// Checking every generic body unconditionally turned a compilation `main`
/// COMPLETES into a hard error, on a module function nothing calls.
///
/// Nothing emits this body either, so skipping it is the emission rule and not
/// an exception to it. Instantiating the same body fails in both trees, so the
/// rule costs no accepted program.
#[test]
fn test_an_uninstantiated_imported_generic_with_a_macro_still_compiles() {
    let (compiled, output, _stdout) = compile_and_run(
        &[(
            "g2.pd",
            "pub fn gen<T>(x: T) -> T { let v = vec!(7); return x; }\n",
        )],
        "import g2;\n\nfn main() { print_int(1); }\n",
    );
    assert!(
        compiled,
        "a generic imported body nothing instantiates was walked anyway, and its \
         unexpanded macro killed a compilation that emits none of it; \
         compiler said:\n{}",
        output
    );
}

/// A DISPLACED imported template must not veto the build.
///
/// `TypeChecker::generic_functions` is keyed by bare name and is
/// last-writer-wins, with locals walked after imports, so a local `pick<T>`
/// displaces an imported one and codegen monomorphizes the LOCAL. A predicate
/// that asks only "is the name `pick` instantiated" cannot tell the winner from
/// the loser, and checked both.
///
/// Measured before the origin was carried across the boundary — the emitted C
/// contains no trace of the imported body:
///
///     error: Use of moved value: a
///
/// and renaming the imported function, changing nothing else, compiled and ran.
/// The name collision was the entire difference.
///
/// This is the control the previous round did not have: its four reverts
/// bracketed empty-versus-all selection, which is a different axis from
/// same-name identity, so all four passed while this was broken.
#[test]
fn test_a_local_generic_is_not_vetoed_by_a_displaced_imported_twin() {
    let (compiled, output, stdout) = compile_and_run(
        &[(
            "lib.pd",
            "pub struct S { v: i64 }\n\
             pub fn pick<T>(x: T) -> i64 { let a: S = S{v:7}; let b: S = a; let c: S = a; return c.v; }\n",
        )],
        "import lib;\n\n\
         fn pick<T>(x: T) -> i64 { return 3; }\n\n\
         fn main() { print_int(pick(1)); }\n",
    );
    assert!(
        compiled,
        "an imported generic that the local definition DISPLACED — and that the \
         emitted C contains no trace of — vetoed the build; compiler said:\n{}",
        output
    );
    assert_eq!(
        stdout.as_deref(),
        Some("3\n"),
        "the LOCAL template is the one codegen monomorphizes; compiler said:\n{}",
        output
    );
}

/// The same defect with a macro instead of an ownership error, because the two
/// reach this pass by different routes and a fix could close one and not the
/// other.
///
/// An imported body is never macro-expanded, so walking a displaced one reports
/// the internal "macros should be expanded before this phase" — again over a
/// body nothing emits.
#[test]
fn test_a_displaced_imported_twin_with_a_macro_does_not_veto_the_build() {
    let (compiled, output, stdout) = compile_and_run(
        &[(
            "libm.pd",
            "pub fn pick<T>(x: T) -> i64 { let v = vec!(7); return v[0]; }\n",
        )],
        "import libm;\n\n\
         fn pick<T>(x: T) -> i64 { return 3; }\n\n\
         fn main() { print_int(pick(1)); }\n",
    );
    assert!(
        compiled,
        "a displaced imported generic's unexpanded macro vetoed the build; \
         compiler said:\n{}",
        output
    );
    assert_eq!(stdout.as_deref(), Some("3\n"), "compiler said:\n{}", output);
}

/// The same defect with NO local definition at all: two modules exporting the
/// name, where the loser is displaced exactly as a local definition displaces
/// an import.
///
/// Modules are walked in sorted order and the map is last-writer-wins, so
/// `libb` wins and `liba`'s body — which carries the ownership error — is the
/// one nothing emits. Which of the two wins is a real ambiguity nothing
/// diagnoses (`test_ambiguous_import_is_diagnosed_by_the_compiler_not_by_gcc`);
/// what this asserts is only that the LOSER is not checked, since it is not
/// emitted.
#[test]
fn test_the_losing_module_of_a_name_clash_does_not_veto_the_build() {
    let (compiled, output, stdout) = compile_and_run(
        &[
            (
                "liba.pd",
                "pub struct S { v: i64 }\n\
                 pub fn pick<T>(x: T) -> i64 { let a: S = S{v:7}; let b: S = a; let c: S = a; return c.v; }\n",
            ),
            ("libb.pd", "pub fn pick<T>(x: T) -> i64 { return 222; }\n"),
        ],
        "import liba;\nimport libb;\n\nfn main() { print_int(pick(1)); }\n",
    );
    assert!(
        compiled,
        "the LOSING module's generic body vetoed a build that emits the other \
         module's; compiler said:\n{}",
        output
    );
    assert_eq!(
        stdout.as_deref(),
        Some("222\n"),
        "sorted order makes libb the winner; compiler said:\n{}",
        output
    );
}

/// An imported GENERIC struct's layout is needed by the bodies this pass checks.
///
/// Registration used to skip generic imported structs, on the reasoning that
/// codegen emits only non-generic imported ones — the same reasoning, one item
/// kind across, that was false for functions because of monomorphization.
/// Structs have that path too. Once the walk started checking instantiated
/// imported bodies, the missing layout stopped being a harmless omission: field
/// Copy classification falls back to "not Copy" for a layout it cannot resolve,
/// so the SECOND read of an `i64` field is reported as a use of a moved value.
///
/// Measured, the only difference being `<T>` on the struct:
///
///     pub struct P<T> { a: i64 }   -> error: Use of moved value: p.a
///     pub struct Q    { a: i64 }   -> compiles
///
/// The local walk never had this guard, so registering here is what makes the
/// two sides agree rather than a new rule.
///
/// ASSERTED ON THE DIAGNOSTIC, NOT ON `compiled`, and that is not a weaker
/// claim dressed up. This program does not link, on this branch or on `main`,
/// and neither does the byte-identical LOCAL version: a generic struct
/// referenced by its bare name is never emitted, so gcc reports
/// `variable has incomplete type 'struct P'`. That failure is shared by both
/// sides and predates this branch, so it is not what this fix owns and is
/// declared separately in
/// `test_a_generic_struct_referenced_by_its_bare_name_is_emitted`. What this
/// fix owns is the ownership verdict, and asserting `compiled` here would tie
/// this control to an unrelated codegen gap that can be fixed or worsened
/// without touching registration at all.
#[test]
fn test_an_imported_generic_structs_layout_is_registered() {
    let (_compiled, output, _stdout) = compile_and_run(
        &[(
            "libp.pd",
            "pub struct P<T> { a: i64 }\n\
             pub fn use2<T>(x: T) -> i64 { let p: P = P { a: 1 }; let m: i64 = p.a; let n: i64 = p.a; return m + n; }\n",
        )],
        "import libp;\n\nfn main() { print_int(use2(1)); }\n",
    );
    assert!(
        !output.contains("Use of moved value"),
        "a generic imported struct's layout was missing, so the second read of \
         its i64 field was called a move; compiler said:\n{}",
        output
    );
}

/// The codegen gap the control above steps around, declared rather than left
/// as a sentence in a doc comment.
///
/// A generic struct referenced by its bare name type-checks and then emits C
/// that names a type nothing defines. Measured on both trees, and on the LOCAL
/// form as well as the imported one, so it is neither this branch's doing nor
/// import-specific:
///
///     struct P<T> { a: i64 }
///     fn use2<T>(x: T) -> i64 { let p: P = P { a: 1 }; ... }
///
///     -> build_output/main.c: variable has incomplete type 'struct P'
#[test]
#[ignore = "XFAIL: a generic struct referenced by its BARE name is never emitted — typeck accepts `let p: P = P { a: 1 }` for `struct P<T>`, and codegen emits `struct P p = (struct P){.a = 1}` with no definition of `struct P` anywhere, so gcc reports \"variable has incomplete type 'struct P'\"; reproduces on main and for a LOCAL generic struct as well as an imported one (owned by M4, generics and monomorphization)"]
fn test_a_generic_struct_referenced_by_its_bare_name_is_emitted() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "struct P<T> { a: i64 }\n\
         fn use2<T>(x: T) -> i64 { let p: P = P { a: 1 }; let m: i64 = p.a; let n: i64 = p.a; return m + n; }\n\n\
         fn main() { print_int(use2(1)); }\n",
    );
    assert!(
        compiled,
        "a generic struct named without type arguments emitted C for an \
         undefined type; compiler said:\n{}",
        output
    );
    assert_eq!(stdout.as_deref(), Some("2\n"), "compiler said:\n{}", output);
}

// ---------------------------------------------------------------------------
// What is behind this wall
// ---------------------------------------------------------------------------
//
// The two tests above removed the reason nobody could see past the borrow
// checker: until they passed, EVERY multi-file program died in one place, so
// nothing downstream of it had ever been exercised. The tests below are what a
// walk past that point found. Each was measured — the failure quoted in its
// `#[ignore]` reason is the observed one, not a prediction — and each is left
// failing on purpose.
//
// They are `#[ignore = "XFAIL: … (owned by M4)"]` because that is this repo's
// mechanism for a measured, owned debt: `scripts/test-xfail.py` (via
// `make test-xfail`) runs every ignored test and fails the gate if a declared
// failure PASSES. So paying one off is a TRANSITION — delete the `#[ignore]`
// and let the test join the regression net — and never a deletion.
//
// THE OWNER IS M4, AND IT WAS M3 UNTIL THIS BRANCH MERGED `d2d5bd4`. That merge
// restructured the milestones and split modules out into their own
// (`docs/contributing/MILESTONES.md:978-984`, "M4 — Modules", which claims the
// module rows explicitly: "plus the corpus's one `xfail` … cross-file imports
// — and the vacuous `12_modules_imports`"). M3 is now traits and generics
// (`docs/contributing/MILESTONES.md:944`), which is not what these rows are
// about.
//
// BOTH CITATIONS ABOVE WERE WRONG BEFORE THEY WERE MOVED, and are corrected
// rather than re-pinned. At `acda322` the first resolved to M3's items 3-6 and
// the second to M2's builtin item — each off by the height of the M2 section,
// against sentences that say "M4 — Modules" and "traits and generics". The pin
// file held the fingerprint of whatever was on those lines, which is exactly how
// a citation that points at the wrong thing survives a review: `--update`
// re-pins whatever occupies the line. They now name the `## M4 — Modules`
// section and the `## M3 — Traits and generics` heading, which is what the
// sentences claim.
//
// AND `--update` WILL RE-PIN THEM TO THE WRONG CONTENT WITHOUT SAYING SO. Caught
// on 2026-08-23: an edit above these lines moved the two headings, the pin file
// was regenerated, and `make check-doc-evidence` went GREEN with both citations
// resolving to unrelated prose — because `--update` re-pins whatever now occupies
// the cited LINE, and a citation that has drifted onto a new target looks exactly
// like a citation whose target was edited. The gate cannot tell them apart; only
// re-reading the target can. That is why these two are re-derived by hand every
// time this file's line numbers move, and why the audit after any `--update` is
// "did a fingerprint change, and can I name why".
//
// AND IT HAPPENED AGAIN IN THE MERGE THAT FOLLOWED. `fix/m2-async-producer`
// carried these two citations forward with line numbers shifted to track its own
// edits, and they still resolved to M3's items and to M2's builtin item. Both
// sides of the conflict were therefore "correct" against their own tree and only
// one was correct about the CONTENT — which is the whole argument for
// re-deriving a moved citation instead of re-pinning it.
//
// The file is still named `m3_imported_calls.rs`. Renaming it would move every
// citation that points into it, so the name is left as a historical artefact and
// the OWNER STRINGS are what carry the attribution — those are what
// `CONFORMANCE_FORBID_OWNER` and the milestone-exit targets read.
//
// NOT FIXED HERE, AND REPORTED INSTEAD: the two module rows in
// `tests/conformance-manifest.txt` still say M3, while the MILESTONES section
// above names those exact two rows as M4's. That inconsistency arrived with
// `d2d5bd4` — both rows are byte-identical at `728779b` and this branch has
// never touched that file — so it is main's to resolve, and the owner column is
// machine-read by `CONFORMANCE_FORBID_OWNER`.
//
// TWO OF THEM WERE NOT MISSING FEATURES, AND THOSE TWO ARE NOW PAID OFF. The
// first two tests below — imported bodies being borrow-checked and type-checked —
// were holes in checks that already existed and that the language already
// enforced on the same code written locally. They are the reason this branch
// could not ship as a pure feature enablement: making an imported signature
// callable without checking the body behind it turns a compiler that REFUSED
// multi-file programs (fail-closed) into one that ACCEPTS unchecked code
// (fail-open). Both passes now take a third walk over the imported modules, and
// both tests are live assertions rather than declarations.
//
// What is left below really is missing work, plus two soundness rows that this
// branch measured and could not close: the unchecked monomorphization choice
// (owned by M4, not by modules) and a module's lack of a private scope.

/// TRANSITIONED from XFAIL — this was the soundness hole registration opened.
///
/// Registering an imported signature makes the CALL check. It says nothing about
/// the body behind it, and until the borrow checker grew a third pass over the
/// imported modules its second pass walked `program.items` only, so no imported
/// body was ever visited: `pub fn bad(mut x: i64) { x = 42; }` in a module
/// compiled, linked and printed 42, while the byte-identical program written
/// locally was refused. The generated C was `void bad(long long* x)` and
/// `bad(&n)` — the write really did reach the caller's storage.
///
/// That is why this test is not "modules do not support X yet": it is the same
/// checker reaching the opposite verdict on the same program depending on which
/// file the function lives in. A compiler that ACCEPTS code it has never checked
/// is worse than one that refuses to compile it at all, which is what the import
/// wall was doing before this branch.
///
/// The control is `test_local_twin_of_the_unchecked_import_is_rejected` below:
/// the same program written locally must still be refused, or this test is
/// asserting nothing about imports in particular.
#[test]
fn test_imported_function_bodies_are_borrow_checked() {
    let (compiled, output, stdout) = compile_and_run(
        &[(
            "lib2.pd",
            "pub fn bad(mut x: i64) { x = 42; }\n\
             pub fn caller() -> i64 { let n = 1; bad(n); return n; }\n",
        )],
        "import lib2;\n\nfn main() {\n    print_int(caller());\n}\n",
    );
    assert!(
        !compiled,
        "an imported body mutated an immutable binding and was accepted; it printed {:?}",
        stdout
    );
    assert!(
        output.contains("cannot borrow `n` as mutable"),
        "the refusal must be the borrow checker's, not gcc's; compiler said:\n{}",
        output
    );
}

/// The control for the test above, and the reason it is called a hole rather
/// than a gap: this is the same source with the two functions written locally,
/// and it is refused. If this ever starts compiling, the XFAIL above stops
/// being evidence of anything.
#[test]
fn test_local_twin_of_the_unchecked_import_is_rejected() {
    let (compiled, output, _) = compile_and_run(
        &[],
        "fn bad(mut x: i64) { x = 42; }\n\
         fn caller() -> i64 { let n = 1; bad(n); return n; }\n\
         fn main() { print_int(caller()); }\n",
    );
    assert!(
        !compiled,
        "the LOCAL form of the unchecked-import program was accepted, which would \
         make the XFAIL above meaningless:\n{}",
        output
    );
    assert!(
        output.contains("cannot borrow `n` as mutable"),
        "expected the borrow checker's refusal; compiler said:\n{}",
        output
    );
}

/// THE OTHER POLARITY, and the defect the third pass introduced.
///
/// Walking imported bodies is only half of the job; the walk has to be handed the
/// same ENVIRONMENT the local walk gets. It was not. `register_imported_functions`
/// registered signatures and nothing else, so `struct_fields` — the map that
/// `place_type` (`src/ownership/borrow_checker.rs:1563-1565`) consults to decide
/// whether `p.x` is Copy — held local struct layouts only. An imported struct's
/// `i64` field therefore had no resolvable type, `is_expr_copy` fell into its
/// conservative `false` default, and the FIRST read of the field MOVED it:
///
/// ```text
/// pub struct P { x: i64 }
/// pub fn twice(p: P) -> i64 { let a: i64 = p.x; let b: i64 = p.x; return a + b; }
/// -> error: Use of moved value: p.x        (imported)
/// -> Compilation successful                (byte-identical, declared locally)
/// ```
///
/// This is the mirror image of `test_imported_function_bodies_are_borrow_checked`
/// above: that one was a false ACCEPT, this one was a false REJECT of a valid
/// program. Conservatism is not a safe direction here — an over-approximating
/// refusal fails closed onto correct code, and the user cannot work around it.
///
/// Both directions are asserted in ONE test on purpose. A test that only checked
/// the imported side would keep passing if the local side regressed to the same
/// refusal, and "both files are rejected identically" is not the property under
/// test — "the verdict does not depend on which file the struct lives in" is.
/// Running both is part of the claim too, for the D3 reason: accepting a program
/// and then handing it garbage is not a fix.
#[test]
fn test_a_primitive_field_is_read_twice_wherever_its_struct_is_declared() {
    const BODY: &str =
        "fn twice(p: P) -> i64 { let a: i64 = p.x; let b: i64 = p.x; return a + b; }";

    let (local_compiled, local_output, local_stdout) = compile_and_run(
        &[],
        &format!(
            "struct P {{ x: i64 }}\n\
             {BODY}\n\
             fn main() {{ let p: P = P {{ x: 21 }}; print_int(twice(p)); }}\n"
        ),
    );
    let (imported_compiled, imported_output, imported_stdout) = compile_and_run(
        &[(
            "lib2.pd",
            &format!("pub struct P {{ x: i64 }}\npub {BODY}\n"),
        )],
        "import lib2;\n\nfn main() { let p: P = P { x: 21 }; print_int(twice(p)); }\n",
    );

    assert!(
        local_compiled,
        "the LOCAL form was rejected, so the imported assertion below would be \
         measuring nothing; compiler said:\n{}",
        local_output
    );
    assert_eq!(
        local_stdout.as_deref(),
        Some("42\n"),
        "the LOCAL form compiled but did not run correctly; compiler said:\n{}",
        local_output
    );

    assert!(
        imported_compiled,
        "reading a primitive field twice is accepted when the struct is declared \
         locally and refused when it is imported — the borrow checker's verdict \
         must not depend on which file the struct lives in; compiler said:\n{}",
        imported_output
    );
    assert!(
        !imported_output.contains("Use of moved value"),
        "the imported struct's `i64` field is still classified as non-Copy, so the \
         first read moves it; compiler said:\n{}",
        imported_output
    );
    assert_eq!(
        imported_stdout.as_deref(),
        Some("42\n"),
        "the imported form compiled but did not run correctly; compiler said:\n{}",
        imported_output
    );
}

/// TRANSITIONED from XFAIL — the same shape one pass over.
///
/// Imported bodies were not TYPE-checked either: the type checker's second pass
/// walked `program.items` only, so `pub fn broken() -> i64 { let s = "x";
/// return s; }` in a module reported `✅ Compilation successful!` and then died
/// in gcc with "incompatible pointer to integer conversion returning
/// 'const char *'" — a C diagnostic against code the user never wrote, which is
/// precisely the class of failure M1 exists to remove. The type checker now
/// takes a third pass over the imported modules, next to the borrow checker's.
#[test]
fn test_imported_function_bodies_are_type_checked() {
    let (compiled, output, _) = compile_and_run(
        &[(
            "lib2.pd",
            "pub fn broken() -> i64 { let s = \"x\"; return s; }\n",
        )],
        "import lib2;\n\nfn main() {\n    print_int(broken());\n}\n",
    );
    assert!(!compiled, "a type error in a module body was accepted");
    assert!(
        !output.contains("gcc compilation failed"),
        "the type error must be caught before any C exists; compiler said:\n{}",
        output
    );
}

/// A module's own imports are resolved and then discarded:
/// `src/resolver/mod.rs:203` is `let _sub_modules = self.resolve_program(&ast)?;`,
/// and `resolve_program` (`src/resolver/mod.rs:70-95`) builds its returned map
/// from the TOP-LEVEL program's `program.imports` alone. The sub-module lands in
/// the resolver's private cache and never reaches typeck, the borrow checker or
/// codegen, so
/// `outer`'s body is emitted calling a `base` that was never emitted.
///
/// It works if `main` also imports `liba` directly — which is the diamond case,
/// and is why this is a missing hop rather than a missing feature.
#[test]
#[ignore = "XFAIL: transitive imports are resolved and thrown away — src/resolver/mod.rs:203 discards the recursive result into `_sub_modules` and src/resolver/mod.rs:70-95 returns only the top-level program's own imports, so `main -> libb -> liba` emits `outer`'s body calling `base` and gcc reports \"call to undeclared function 'base'\" (owned by M4, cross-file module imports)"]
fn test_a_module_can_use_what_it_imports() {
    let (compiled, output, stdout) = compile_and_run(
        &[
            ("liba.pd", "pub fn base() -> i64 { return 7; }\n"),
            (
                "libb.pd",
                "import liba;\npub fn outer() -> i64 { return base() + 1; }\n",
            ),
        ],
        "import libb;\n\nfn main() {\n    print_int(outer());\n}\n",
    );
    assert!(
        compiled,
        "a module could not call what it imported; compiler said:\n{}",
        output
    );
    assert_eq!(stdout.as_deref(), Some("8\n"));
}

/// A MODULE HAS NO PRIVATE SCOPE, and adding the imported-body checks is what
/// made that visible instead of leaving it to the linker.
///
/// Every consumer of an imported module — both checkers and codegen — filters
/// `ast.items` by `Visibility::Public` and drops the rest on the floor. There is
/// nowhere in the compiler where "the names visible INSIDE module m" exists as a
/// scope, so a public function that calls a private sibling names something no
/// table holds.
///
/// Measured, three ways on the same program:
///   - on `main`, and on this branch before the imported-body checks: it reached
///     gcc, which said "call to undeclared function 'priv_helper'";
///   - with the imported-body checks: the type checker says "Undefined function:
///     priv_helper" before any C exists;
///   - with a private STRUCT instead of a private function: "Unknown struct
///     type: P". Same missing scope, one type of item over.
///
/// So this row is NOT a regression: no program moved from working to broken, and
/// the diagnosis moved from the linker into the compiler. What it is, is the next
/// break on the path, declared before someone hits it — which is why the
/// imported-body checks skip private items rather than registering them. Doing
/// that instead would silence THIS diagnostic while leaving codegen still emitting
/// only public functions, and the program would go back to failing in gcc.
#[test]
#[ignore = "XFAIL: a module has no private scope — every consumer filters `ast.items` by Visibility::Public, so `pub fn outer() { return priv_helper() + 1; }` beside a private `fn priv_helper` reports \"Undefined function: priv_helper\" (and a private struct reports \"Unknown struct type\"); before the imported-body checks the same program reached gcc as \"call to undeclared function 'priv_helper'\" (owned by M4, cross-file module imports)"]
fn test_a_module_can_use_its_own_private_items() {
    let (compiled, output, stdout) = compile_and_run(
        &[(
            "lib2.pd",
            "fn priv_helper() -> i64 { return 3; }\n\
             pub fn outer() -> i64 { return priv_helper() + 1; }\n",
        )],
        "import lib2;\n\nfn main() {\n    print_int(outer());\n}\n",
    );
    assert!(
        compiled,
        "a module could not call its own private function; compiler said:\n{}",
        output
    );
    assert_eq!(stdout.as_deref(), Some("4\n"), "compiler said:\n{}", output);
}

/// An imported module's AST is never macro-expanded, and no pass says so.
///
/// The driver expands macros over the top-level AST (`src/driver/mod.rs:80-81`)
/// and resolves modules AFTER that (`src/driver/mod.rs:89-97`); the resolver
/// itself only lexes and parses (`src/resolver/mod.rs:145-148`). So every
/// imported body still carries `Expr::MacroInvocation` nodes when the checkers
/// walk it, and every pass that meets one reports the internal error "macros
/// should be expanded before this phase" — a message about compiler phases,
/// shown to a user who wrote a macro in a library.
///
/// THIS IS DECLARED RATHER THAN GUARDED. The generic case WAS a regression this
/// branch introduced and is fixed: the third pass now skips generic imported
/// bodies, matching `TypeChecker::check_function` and codegen, so
/// `pub fn gen<T>(..) { vec!(7) }` no longer kills a compilation that never
/// calls it. The non-generic case below is broken on `main` too — the macro
/// reaches codegen there instead — so nothing regressed, and a guard would only
/// move which phase says it. The honest fix is expanding module ASTs in the
/// resolver, which is M3's work.
#[test]
#[ignore = "XFAIL: an imported module's AST is never macro-expanded — src/driver/mod.rs:80-81 expands the top-level AST BEFORE module resolution at src/driver/mod.rs:89-97, and src/resolver/mod.rs:145-148 only lexes and parses, so a macro in a public imported body reaches the checkers as Expr::MacroInvocation and reports the internal \"macros should be expanded before this phase\"; on main the same program reaches codegen and reports it there (owned by M4, cross-file module imports)"]
fn test_a_macro_in_an_imported_body_is_never_expanded() {
    let (compiled, output, stdout) = compile_and_run(
        &[(
            "lib2.pd",
            "pub fn boxed() -> i64 { let v = vec!(7); return v[0]; }\n",
        )],
        "import lib2;\n\nfn main() {\n    print_int(boxed());\n}\n",
    );
    assert!(
        compiled,
        "a macro in an imported body was not expanded; compiler said:\n{}",
        output
    );
    assert_eq!(stdout.as_deref(), Some("7\n"), "compiler said:\n{}", output);
}

/// Move checking depends on whether the author wrote a type annotation.
///
/// `BorrowChecker::expr_type` is a stub that returns `Type::I64` for every
/// expression that is not an integer, string or bool literal — including a
/// struct literal and every identifier (`src/ownership/borrow_checker.rs`, the
/// `Expr::Ident(_) => Type::I64, // TODO: Proper type lookup` arm). `Stmt::Let`
/// consults it only when the binding has NO declared type, so `let a = S { .. }`
/// records `a: I64`, `is_copy_type` answers true, and the move never happens.
///
/// Measured, on `main` as well as here — the same program twice, differing only
/// by the annotation:
///
///     let a: S = S { v: 1 };  let b: S = a;  let c: S = a;   -> Use of moved value: a
///     let a  = S { v: 1 };    let b  = a;    let c  = a;     -> compiles and runs
///
/// THIS IS THE FAIL-OPEN HALF OF THE SAME STUB whose fail-CLOSED half this branch
/// already reports (a field read of an unresolvable projection moves, so the
/// second read is refused). It is the worse polarity, and it is invisible to the
/// existing corpus because every move test in this file carries the annotation —
/// which is the recurring shape here: the corpus cannot see what nothing
/// exercises. Pre-existing, not introduced by this branch, and it is a row rather
/// than a paragraph because nothing gates a doc comment: a comment cannot fail
/// when the defect worsens, and nothing notices when it is fixed.
#[test]
fn test_use_after_move_is_rejected_without_a_type_annotation() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "struct S { v: i64 }\n\n\
         fn main() {\n    \
         let a = S { v: 1 };\n    \
         let b = a;\n    \
         let c = a;\n    \
         print_int(b.v);\n    \
         print_int(c.v);\n}\n",
    );
    assert!(
        !compiled,
        "a use-after-move was accepted because neither binding was annotated; \
         it printed {:?}",
        stdout
    );
    assert!(
        output.contains("Use of moved value"),
        "expected the move checker's refusal; compiler said:\n{}",
        output
    );
}

/// `local_types` is per-FUNCTION where the language is per-BLOCK, so a shadow
/// inside an `if` changes how the enclosing binding is classified after it.
///
/// The map is cleared in `check_function` and written by every binder, but
/// nothing restores it at block exit — unlike `mutable_bindings`, which has
/// `open_mutability_scope`/`close_mutability_scope` (and even that is only
/// applied to `for` and `match`, not to `if`/`while`/`unsafe`). So an inner
/// `let s: i64` overwrites the outer `s: S`, `is_expr_copy` then answers true
/// for a struct, and the move that should follow never happens:
///
///     let s: S = S { v: 7 };
///     if true { let s: i64 = 1; print_int(s); }
///     let t: S = s;          // classified Copy, so `s` is not moved out of
///     print_int(s.v);        // accepted, and prints 7
///
/// Its control is the same program with the inner block deleted, which IS
/// refused with "Use of moved value: s" — so the `if` body is doing the whole
/// of it.
///
/// PRE-EXISTING: reproduces identically on `main` (measured), and this branch
/// neither introduced nor widened it. It is a row rather than a note because a
/// note cannot fail — nothing would notice this getting worse, and nothing
/// would notice it being fixed. The scope machinery for `ownership` landed on
/// this branch; doing the same for `local_types` is a narrowing change to a
/// classification every pass reads, which is its own unit of work.
#[test]
#[ignore = "XFAIL: local_types is per-function where the language is per-block — src/ownership/borrow_checker.rs clears it in check_function and no block restores it (unlike mutable_bindings, which has open/close_mutability_scope, itself applied only to `for` and `match`), so an inner `let s: i64` inside an `if` leaves the outer `s: S` classified as i64, is_expr_copy answers true for the struct, and the following `let t: S = s` does not move: `print_int(s.v)` after it is accepted and prints 7, while the same program without the inner block is refused with \"Use of moved value: s\"; reproduces on main (owned by M4, the type model the ownership pass reads)"]
fn test_a_block_local_shadow_does_not_change_the_outer_bindings_copy_class() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "struct S { v: i64 }\n\n\
         fn main() {\n    \
         let s: S = S { v: 7 };\n    \
         if true { let s: i64 = 1; print_int(s); }\n    \
         let t: S = s;\n    \
         print_int(s.v);\n    \
         print_int(t.v);\n}\n",
    );
    assert!(
        !compiled,
        "an inner `let s: i64` made the outer struct `s` look Copy, so the move \
         into `t` never happened and `s` was read after it; it printed {:?}",
        stdout
    );
    assert!(
        output.contains("Use of moved value"),
        "expected the move checker's refusal; compiler said:\n{}",
        output
    );
}

/// Selective import is a no-op, and `ModuleInfo.exports` is dead data.
/// `filter_module_info` (`src/resolver/mod.rs:105-118`) narrows the `exports`
/// set and leaves `ast` complete — and `.exports` is read nowhere but its own
/// filter (`src/resolver/mod.rs:113` is the only hit in `src/`). Every consumer
/// re-derives visibility from `ast.items` instead: `src/typeck/mod.rs:1450-1450`,
/// `src/codegen/mod.rs:1776-1776`, `src/codegen/mod.rs:1699-1699`,
/// `src/codegen/mod.rs:2009-2009`, `src/codegen/mod.rs:2924-2924`, and the borrow checker's
/// `register_imported_functions`. So `import lib2::{helper};` imports the whole
/// module.
#[test]
#[ignore = "XFAIL: `import m::{a};` imports all of `m` — src/resolver/mod.rs:105-118 filters the `exports` set but not `ast`, and `.exports` is read nowhere but its own filter (src/resolver/mod.rs:113), so every consumer re-derives visibility from `ast.items`; a name the import did not list compiles and runs (owned by M4, cross-file module imports)"]
fn test_selective_import_does_not_import_the_rest() {
    let (compiled, output, stdout) = compile_and_run(
        &[(
            "lib2.pd",
            "pub fn helper() -> i64 { return 5; }\npub fn other() -> i64 { return 9; }\n",
        )],
        "import lib2::{helper};\n\nfn main() {\n    print_int(other());\n}\n",
    );
    assert!(
        !compiled,
        "`other` was not imported but was callable; it printed {:?}\n{}",
        stdout, output
    );
}

/// A local definition that shadows an import is decided correctly by both
/// checkers (the local one wins; see `register_imported_functions`) AND by code
/// generation, which asks `local_definition_shadows_import` before emitting an
/// imported function (`src/codegen/mod.rs:2170-2180`) and emits the local one
/// unconditionally (`src/codegen/mod.rs:2187-2193`). This test is green.
///
/// THE SENTENCE ABOVE USED TO SAY THE OPPOSITE — "contradicted by codegen …
/// with no shadowing check at all. The front end's answer is right and
/// unenforceable" — and it was stale twice over. The check is right there at
/// `src/codegen/mod.rs:2176-2176`, and BOTH of the citations it leaned on had
/// drifted onto unrelated code: at `d20b759` line 1378 was a bare `}` and
/// 1557-1566 was `type_to_c`'s primitive-type match. Neither had anything to do
/// with function emission.
///
/// It survived because a citation only fails the evidence gate when its TARGET
/// MOVES. These two were fingerprint-stable on the wrong lines, which is
/// exactly the failure mode `scripts/check_doc_evidence.py`'s docstring names
/// ("A PIN WHOSE TARGET CARRIES NO CONTENT IS NOT A CITATION") — except a bare
/// `}` is caught by that rule and a plausible-looking `match ty {` is not.
/// `fix/m2-lexical` shifted the lines, the machine could no longer relocate the
/// range, and only then did anyone read it.
#[test]
fn test_a_local_definition_shadows_an_imported_one() {
    let (compiled, output, stdout) = compile_and_run(
        &[("lib2.pd", "pub fn helper() -> i64 { return 5; }\n")],
        "import lib2;\n\nfn helper() -> i64 { return 99; }\n\nfn main() {\n    print_int(helper());\n}\n",
    );
    assert!(
        compiled,
        "a local definition shadowing an import was rejected; compiler said:\n{}",
        output
    );
    assert_eq!(
        stdout.as_deref(),
        Some("99\n"),
        "the local definition must be the one that runs"
    );
}

/// Two modules exporting one name is a real ambiguity that nothing diagnoses.
/// Every front-end table that holds imported names is now filled in sorted module
/// order, so the winner is at least the same one every run — but it is still
/// arbitrary, and codegen emits both bodies regardless, so the user meets it as a
/// C redefinition against code they never wrote.
///
/// AN EARLIER VERSION OF THIS REASON WAS AN OVER-CLAIM, and the correction is the
/// point: it said the front end picks in sorted-key order, citing only the borrow
/// checker's `register_imported_functions`. The type checker is also the front
/// end, and it was filling its tables from a raw `HashMap` iteration — so for a
/// GENERIC name the choice was made by the hash seed, and the compiled program's
/// answer flipped between runs. `test_two_modules_exporting_one_generic_name_are_stable`
/// below is the measurement that found it. Cite every table, not the one you
/// happened to write.
///
/// The assertion is deliberately about WHERE the refusal comes from, not
/// whether one happens: gcc already refuses this. A compiler that hands invalid
/// C to gcc and lets gcc explain has not diagnosed anything.
#[test]
#[ignore = "XFAIL: two imported modules exporting the same name are not diagnosed — the front end silently picks one (sorted module order, in the borrow checker's `register_imported_functions` and in `TypeChecker::set_imported_modules`) and codegen emits both bodies, so the user meets it as gcc's \"redefinition of 'dup'\" against C they never wrote (owned by M4, cross-file module imports)"]
fn test_ambiguous_import_is_diagnosed_by_the_compiler_not_by_gcc() {
    let (compiled, output, _) = compile_and_run(
        &[
            ("liba.pd", "pub fn dup() -> i64 { return 1; }\n"),
            ("libb.pd", "pub fn dup() -> i64 { return 2; }\n"),
        ],
        "import liba;\nimport libb;\n\nfn main() {\n    print_int(dup());\n}\n",
    );
    assert!(!compiled, "an ambiguous imported name was accepted");
    assert!(
        !output.contains("gcc compilation failed"),
        "the ambiguity must be named by the compiler, not discovered by gcc:\n{}",
        output
    );
}

/// A qualified call cannot be written. `src/parser/mod.rs:4392-4433` turns any
/// `a::b(...)` into `Expr::EnumConstructor`, and `src/typeck/mod.rs:4184-4188`
/// then reports `Undefined enum type: lib2`. The same holds for an alias
/// (`import lib2 as m;` → `Undefined enum type: m`), which makes `alias`
/// unusable too. `register_imported_functions` registers `module::name` for
/// parity with the type checker, but nothing can currently reach it.
#[test]
#[ignore = "XFAIL: a qualified call `lib2::helper()` is unreachable — src/parser/mod.rs:4392-4433 turns every `a::b(...)` into Expr::EnumConstructor and src/typeck/mod.rs:4184-4188 then reports \"Undefined enum type: lib2\"; the same makes `import lib2 as m;` unusable, since `m::helper()` reports \"Undefined enum type: m\" (owned by M4, cross-file module imports)"]
fn test_a_qualified_call_reaches_the_imported_function() {
    let (compiled, output, stdout) = compile_and_run(
        &[("lib2.pd", "pub fn helper() -> i64 { return 5; }\n")],
        "import lib2;\n\nfn main() {\n    print_int(lib2::helper());\n}\n",
    );
    assert!(
        compiled,
        "a qualified call was rejected; compiler said:\n{}",
        output
    );
    assert_eq!(stdout.as_deref(), Some("5\n"));
}

/// A nested module path cannot be written either, one level below the
/// resolver. `src/parser/mod.rs:804-815`: after `::`, if the token after the
/// next one is `;`, `,` or `{`, the segment is consumed as an ITEM name. So
/// `import util::math;` parses as `path=["util"], items=["math"]` and the
/// resolver looks for `util.pd`. The last segment of a path can never be a
/// module, which means a module tree deeper than one level is unexpressible.
#[test]
#[ignore = "XFAIL: nested module paths are unexpressible — src/parser/mod.rs:804-815 consumes the segment after `::` as an ITEM name whenever the following token is `;`/`,`/`{`, so `import util::math;` parses as path=[\"util\"] items=[\"math\"] and the resolver reports \"Module 'util' not found\" for the directory (owned by M4, cross-file module imports)"]
fn test_a_module_in_a_subdirectory_can_be_imported() {
    let (compiled, output, stdout) = compile_and_run(
        &[(
            "util/math.pd",
            "pub fn sq(x: i64) -> i64 { return x * x; }\n",
        )],
        "import util::math;\n\nfn main() {\n    print_int(sq(4));\n}\n",
    );
    assert!(
        compiled,
        "a module in a subdirectory could not be imported; compiler said:\n{}",
        output
    );
    assert_eq!(stdout.as_deref(), Some("16\n"));
}

/// An imported name can shadow a built-in in the checkers but never in codegen,
/// so the two disagree about which function a call means. The call lowering
/// tests `crate::builtins::is_builtin(name)` FIRST and unconditionally
/// (`src/codegen/mod.rs:5533-5535`), while the type checker
/// (`src/typeck/mod.rs:1564-1564`) and `register_imported_functions` both insert
/// the imported signature OVER the built-in.
///
/// With a matching signature the imported definition is merely silent dead code.
/// With a differing one — the case here — the divergence is load-bearing: the
/// type checker accepts `print_int("hello")` against the module's `String`
/// signature and codegen emits `__pd_print_int("hello")` against the built-in's
/// `long long`.
#[test]
#[ignore = "XFAIL: an imported name that shadows a built-in is registered by the checkers but ignored by codegen — src/typeck/mod.rs:1564-1564 and the borrow checker insert the imported signature over the built-in, while src/codegen/mod.rs:5533-5535 tests is_builtin() first and unconditionally, so `pub fn print_int(s: String)` type-checks `print_int(\"hello\")` and then emits `__pd_print_int(\"hello\")`, which gcc rejects as \"incompatible pointer to integer conversion\" (owned by M4, cross-file module imports)"]
fn test_an_import_may_not_silently_disagree_with_a_builtin() {
    let (compiled, output, _) = compile_and_run(
        &[("lib2.pd", "pub fn print_int(s: String) { }\n")],
        "import lib2;\n\nfn main() {\n    print_int(\"hello\");\n}\n",
    );
    assert!(
        !output.contains("gcc compilation failed"),
        "the checkers and codegen disagreed about which `print_int` was meant, and \
         gcc is what noticed; compiler said (compiled={}):\n{}",
        compiled,
        output
    );
}

// ---------------------------------------------------------------------------
// Determinism of the emitted C
// ---------------------------------------------------------------------------
//
// Not an M3 feature question. `make selfhost`'s fixed point is the claim that
// stage1 and stage2 produce byte-identical C, and that claim is the project's
// thesis. It holds today only because `bootstrap/pdc.pd` imports nothing
// (`grep -c '^import' bootstrap/pdc.pd` -> 0), so the moment the self-hosting
// compiler is rewritten in a dialect with modules, a compiler whose output
// depends on the hash seed cannot reach a fixed point at all.
//
// Measured before the fix, on ONE unchanged two-module program: 8 compiles
// produced two distinct SHA-1s, four each; the diff was a pure ordering swap of
// the two imported functions. Cause: `HashMap` with `RandomState`, which
// reseeds per process, iterated at four sites over `imported_modules`.
//
// WHY EVERY ASSERTION BELOW IS OVER EIGHT COMPILES. Eight is what is committed —
// `(0..8)` at each call site — and the number has to be defended rather than
// narrated, because a determinism test that draws too few samples is exactly the
// vacuous test this corpus already has a class for.
//
// The defence is the PRE-FIX measurement, not an assumption of uniformity: with
// the sort removed, this same six-module program produced 22 DISTINCT definition
// orders in 24 compiles. So the pre-fix distribution is spread near the whole
// 720-order space, and the probability that eight independent compiler processes
// agree by luck is on the order of (1/720)^7. Eight is not a compromise between
// runtime and rigour; past two or three samples the exponent has already made the
// question uninteresting, and the remaining cost is 8 pdc + 8 gcc invocations per
// test. A larger N buys nothing this measurement does not already have.
//
// Each test runs the compiler in a fresh process — `RandomState` reseeds per
// process, so N compiles inside one process would sample the seed once and prove
// nothing, whatever N was.

/// Six modules, each contributing a struct and two functions.
fn stress_modules() -> Vec<(String, String)> {
    ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"]
        .iter()
        .map(|n| {
            (
                format!("{}.pd", n),
                format!(
                    "pub struct S_{n} {{ x: i64, y: i64 }}\n\
                     pub fn f_{n}(a: i64) -> i64 {{ return a + 1; }}\n\
                     pub fn g_{n}() -> S_{n} {{ return S_{n} {{ x: 1, y: 2 }}; }}\n",
                    n = n
                ),
            )
        })
        .collect()
}

fn stress_main() -> String {
    let names = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"];
    let imports: String = names.iter().map(|n| format!("import {};\n", n)).collect();
    let calls: String = names
        .iter()
        .map(|n| {
            format!(
                "    print_int(f_{n}(1));\n    print_int(g_{n}().y);\n",
                n = n
            )
        })
        .collect();
    format!("{}\nfn main() {{\n{}}}\n", imports, calls)
}

/// Compile the stress program `n` times and return the generated C of each run.
fn emitted_c_over_runs(n: usize) -> Vec<String> {
    let modules = stress_modules();
    let borrowed: Vec<(&str, &str)> = modules
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let main_src = stress_main();

    (0..n)
        .map(|_| {
            let dir = TempDir::new().unwrap();
            for (name, src) in &borrowed {
                fs::write(dir.path().join(name), src).unwrap();
            }
            fs::write(dir.path().join("main.pd"), &main_src).unwrap();
            let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
                .args(["compile", "main.pd", "-o", "prog"])
                .current_dir(dir.path())
                .output()
                .expect("failed to run pdc");
            assert!(
                out.status.success(),
                "the stress program must compile:\n{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            fs::read_to_string(dir.path().join("build_output").join("main.c")).unwrap()
        })
        .collect()
}

/// The order in which DEFINITIONS are emitted: every `typedef struct X {` and
/// every function definition opener (a line ending in `) {`).
///
/// This is deliberately narrower than the whole file. It isolates the two
/// emission sites that produce DEFINITIONS — imported struct definitions
/// (`src/codegen/mod.rs:2012-2040`) and imported function bodies
/// (`src/codegen/mod.rs:2170-2182`) — from the prototype block
/// (`src/codegen/mod.rs:2921-2932`), which is a fourth site and emits
/// declarations, not definitions. All four are ordered now, so the narrowing no
/// longer isolates a fixed site from a broken one; it survives because the two
/// assertions answer different questions, and this one localises a regression to
/// the definition order instead of reporting "the file differs".
///
/// Column zero only, so an indented `if (...) {` inside a body is not mistaken
/// for a definition; and `__pd_` names are dropped because those are the fixed
/// C runtime prelude, emitted from a literal, which would bury the twelve lines
/// this test is about under 47 entries that never move.
fn definition_order(c: &str) -> Vec<&str> {
    c.lines()
        .map(str::trim_end)
        .filter(|l| !l.starts_with(char::is_whitespace))
        .filter(|l| l.starts_with("typedef struct ") || l.ends_with(") {"))
        .filter(|l| !l.contains("__pd_"))
        .collect()
}

/// REGRESSION CONTROL for the ordering of the imported-module iteration.
///
/// Measured with the sort removed: 22 distinct definition orders over 24
/// compiles of this same program. With it, the 8 compiles asserted here are
/// identical.
#[test]
fn test_imported_definitions_are_emitted_in_a_stable_order() {
    let runs = emitted_c_over_runs(8);
    let first = definition_order(&runs[0]);
    // The same guard the combined test carries: `definition_order` filters hard,
    // and an empty-vs-empty comparison would pass while measuring nothing. Twelve
    // imported functions and six imported structs must be in there.
    assert!(
        first.len() >= 18,
        "definition_order matched {} lines, so this test is comparing almost \
         nothing; the six modules contribute 6 structs and 12 functions:\n{:#?}",
        first.len(),
        first
    );
    for (i, c) in runs.iter().enumerate().skip(1) {
        assert_eq!(
            definition_order(c),
            first,
            "compile #{} emitted the imported definitions in a different order than \
             compile #0 — the generated C depends on the hash seed",
            i
        );
    }
}

/// The WHOLE file, which is what a fixed point actually requires.
///
/// TRANSITIONED from XFAIL. It was declared failing while only three of the four
/// `imported_modules` iteration sites were ordered: `generate_function_prototypes`
/// still iterated `.values()`, so the block of imported prototypes permuted per
/// process and the residual diff was exactly a swap of two prototype lines. That
/// fourth site is now ordered too, so this is a live assertion.
///
/// The four sites are the complete set for the import path, established two ways.
/// Structurally: `CodeGenerator` holds twelve `HashMap`/`HashSet` fields and
/// `imported_modules` is the only one ever *iterated* — the other eleven are
/// lookup-only, so their internal order cannot reach the output. Empirically:
/// the assertion below, over 6 modules (720 possible orders) and 8 independent
/// compiler processes.
#[test]
fn test_the_whole_emitted_c_is_byte_stable() {
    let runs = emitted_c_over_runs(8);
    for (i, c) in runs.iter().enumerate().skip(1) {
        assert_eq!(
            c, &runs[0],
            "compile #{} emitted different C than compile #0",
            i
        );
    }
}

/// NOT ON THE IMPORT PATH — and that is the point of recording it here.
///
/// Auditing codegen for what else could put the hash seed into the output
/// turned up a second, independent source: monomorphized generic
/// instantiations. `TypeChecker::get_instantiations` builds its `Vec` by
/// iterating `self.instantiations.keys()` (`src/typeck/mod.rs:5730-5730`), which is a
/// `HashMap`, and `get_struct_instantiations` does the same
/// (`src/typeck/mod.rs:5792-5792`). Codegen then emits in that Vec's order
/// (`src/codegen/mod.rs:1959-1959`, `src/codegen/mod.rs:2140-2140`).
///
/// This program imports NOTHING, which is how the two sources were told apart:
/// with all four `imported_modules` sites ordered, a six-module program with no
/// generics is byte-identical over 30 compiles, while this six-generic program
/// with no modules produced 30 DISTINCT outputs in 30 compiles. Those two
/// measurements are what established the second source; the committed assertions
/// draw 8, for the reason given at the head of this section.
///
/// It matters for the same reason the import one does: `make selfhost`'s fixed
/// point is a byte-identity claim, and it survives today only because
/// `bootstrap/pdc.pd` has neither modules (`grep -c '^import'` -> 0) nor generics
/// (excluded from PBS-1, `docs/specification/bootstrap-subset.md:97`). Today's
/// fixed point is therefore not evidence that the compiler is deterministic; it
/// is evidence that PBS-1 avoids both sources.
///
/// TRANSITIONED from XFAIL: both `keys()` iterations are now sorted by
/// `(name, type_args)`.
///
/// NOT COVERED by any test: the sibling sort in `get_struct_instantiations`
/// (`src/typeck/mod.rs:5792-5792`). Generic *structs* cannot be compiled at all right
/// now — `struct Box<T> { v: T }` lowers to `void*` and gcc rejects
/// "initializing 'void *' with an expression of incompatible type
/// 'struct Box_alpha_i64'" — so there is no program whose output that ordering
/// could be observed through. It is sorted for symmetry, and this paragraph is
/// the honest statement of its coverage.
#[test]
fn test_generic_instantiations_are_emitted_in_a_stable_order() {
    let names = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"];
    let src = format!(
        "{}\nfn main() {{\n{}}}\n",
        names
            .iter()
            .map(|n| format!("fn id_{n}<T>(v: T) -> T {{ return v; }}\n", n = n))
            .collect::<String>(),
        names
            .iter()
            .map(|n| format!("    print_int(id_{n}(7));\n", n = n))
            .collect::<String>(),
    );

    let runs: Vec<String> = (0..8)
        .map(|_| {
            let dir = TempDir::new().unwrap();
            fs::write(dir.path().join("main.pd"), &src).unwrap();
            let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
                .args(["compile", "main.pd", "-o", "prog"])
                .current_dir(dir.path())
                .output()
                .expect("failed to run pdc");
            assert!(out.status.success(), "the generic program must compile");
            fs::read_to_string(dir.path().join("build_output").join("main.c")).unwrap()
        })
        .collect();

    // Parity with the combined test's guard: the monomorphizations must actually
    // be in the C, or an equal-and-empty comparison would report a stable order
    // for a program that emitted none.
    let first = definition_order(&runs[0]);
    let monos = first.iter().filter(|l| l.contains("id_")).count();
    assert!(
        monos >= 6,
        "only {} monomorphized definition(s) reached the C, so this test proves \
         nothing about their order:\n{:#?}",
        monos,
        first
    );

    for (i, c) in runs.iter().enumerate().skip(1) {
        assert_eq!(
            definition_order(c),
            first,
            "compile #{} emitted the monomorphized generics in a different order",
            i
        );
    }
}

/// THE 1.0 PRECONDITION, measured directly.
///
/// The two tests above each isolate one source and are blind to the other: the
/// module one uses no generics, the generic one imports nothing. Neither speaks
/// to the case the thesis actually needs, because the standing definition of 1.0
/// is `bootstrap/pdc.pd` rewritten in the differentiated dialect and still
/// reaching a byte-identical fixed point — and that dialect uses BOTH.
///
/// So this one uses both at once: six modules, each exporting a plain struct, a
/// constructor and a generic function, with the generics instantiated at `i64` in
/// half the call sites and at `String` in the other half. The emitted C carries
/// six monomorphizations alongside the six imported structs and twelve imported
/// functions.
///
/// IT DOES NOT EXERCISE THE SORT'S TIE-BREAK, and an earlier version of this
/// comment claimed it did. The construction below builds `id_alpha … id_foxtrot`
/// — six DISTINCT names, one instantiation each — so every comparison the sort
/// makes is decided on `name` and `type_args` is never reached. The keys do
/// "differ in `type_args` as well as in `name`", which is what the old comment
/// said and is true; it is also irrelevant, because only two keys with the SAME
/// name and different `type_args` can put the second component of the sort key to
/// work. That case is `test_one_generic_name_with_two_instantiations_is_stable`
/// below, which is why it exists.
///
/// Measured: 30 of 30 identical, before the committed assertion was set at 8 for
/// the reason given at the head of this section.
#[test]
fn test_modules_and_generics_together_are_byte_stable() {
    let names = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"];
    let modules: Vec<(String, String)> = names
        .iter()
        .map(|n| {
            (
                format!("{}.pd", n),
                format!(
                    "pub struct S_{n} {{ x: i64 }}\n\
                     pub fn g_{n}() -> S_{n} {{ return S_{n} {{ x: 5 }}; }}\n\
                     pub fn id_{n}<T>(v: T) -> T {{ return v; }}\n",
                    n = n
                ),
            )
        })
        .collect();

    let imports: String = names.iter().map(|n| format!("import {};\n", n)).collect();
    let calls: String = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            // Alternate the type argument, so both concrete types reach the
            // monomorphizer. This does NOT reach the sort's `type_args`
            // tie-break — see the doc comment; every name here is unique.
            let use_generic = if i % 2 == 0 {
                format!("    print_int(id_{n}(7));\n", n = n)
            } else {
                format!("    print(id_{n}(\"s\"));\n", n = n)
            };
            format!("    print_int(g_{n}().x);\n{}", use_generic, n = n)
        })
        .collect();
    let main_src = format!("{}\nfn main() {{\n{}}}\n", imports, calls);

    let runs: Vec<String> = (0..8)
        .map(|_| {
            let dir = TempDir::new().unwrap();
            for (name, src) in &modules {
                fs::write(dir.path().join(name), src).unwrap();
            }
            fs::write(dir.path().join("main.pd"), &main_src).unwrap();
            let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
                .args(["compile", "main.pd", "-o", "prog"])
                .current_dir(dir.path())
                .output()
                .expect("failed to run pdc");
            assert!(
                out.status.success(),
                "the combined program must compile:\n{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            fs::read_to_string(dir.path().join("build_output").join("main.c")).unwrap()
        })
        .collect();

    // The monomorphizations must actually be there, or this test would pass by
    // exercising nothing — the same trap `tests/07_traits_basic.pd` fell into.
    let monos = runs[0].matches("id_alpha_").count() + runs[0].matches("id_bravo_").count();
    assert!(
        monos > 0,
        "no monomorphized generic reached the C, so this test proves nothing:\n{}",
        runs[0]
    );

    for (i, c) in runs.iter().enumerate().skip(1) {
        assert_eq!(
            c, &runs[0],
            "compile #{} of a program using BOTH modules and generics emitted \
             different C than compile #0",
            i
        );
    }
}

/// THE TIE-BREAK, actually exercised: ONE generic name, TWO instantiations.
///
/// `(&a.name, &a.type_args).cmp(...)` reaches its second component only when the
/// first is equal, and no test above ever gave it two keys with the same name.
/// `id` instantiated at `i64` and at `String` is the smallest program that does:
/// the sort must put `["String"]` before `["i64"]`, and it must do so in every
/// process.
///
/// The assertion is on the ORDER OF THE MONOMORPHIZATIONS, not on the whole file,
/// so it cannot be satisfied by a program that emits one of them and pins the
/// rest — the guard below requires both to be present before anything is compared.
#[test]
fn test_one_generic_name_with_two_instantiations_is_stable() {
    let src = "fn id<T>(v: T) -> T { return v; }\n\n\
               fn main() {\n    print_int(id(7));\n    print(id(\"s\"));\n}\n";

    let runs: Vec<String> = (0..8)
        .map(|_| {
            let dir = TempDir::new().unwrap();
            fs::write(dir.path().join("main.pd"), src).unwrap();
            let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
                .args(["compile", "main.pd", "-o", "prog"])
                .current_dir(dir.path())
                .output()
                .expect("failed to run pdc");
            assert!(
                out.status.success(),
                "the two-instantiation program must compile:\n{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            fs::read_to_string(dir.path().join("build_output").join("main.c")).unwrap()
        })
        .collect();

    let first = definition_order(&runs[0]);
    let monos: Vec<&&str> = first.iter().filter(|l| l.contains("id__")).collect();
    assert!(
        monos.len() == 2,
        "expected both `id__i64` and `id__String` to be emitted, so the sort has \
         two same-name keys to break a tie between; found {}:\n{:#?}",
        monos.len(),
        first
    );

    for (i, c) in runs.iter().enumerate().skip(1) {
        assert_eq!(
            definition_order(c),
            first,
            "compile #{} ordered the two instantiations of `id` differently — the \
             `type_args` component of the sort key is not doing its job",
            i
        );
    }
}

// ---------------------------------------------------------------------------
// Which monomorphization a call resolves to
// ---------------------------------------------------------------------------
//
// NOT AN IMPORT DEFECT, and recorded here for the same reason the generic
// ordering above is: sorting `get_instantiations` made this reproducible, and
// reproducible is what made it findable. Owned by M4, which owns generics.

/// A call with more than one candidate monomorphization resolves to the WRONG one,
/// deterministically.
///
/// `get_mangled_name_for_call` matches if ANY type argument equals the inferred
/// type of the FIRST argument, in ANY position. So for `fn snd<A, B>(a: A, b: B)`
/// instantiated at both `(i64, String)` and `(i64, i64)`, the call `snd(1, 2)`
/// matches the first sorted key on its `i64` in position 0 and resolves to
/// `snd__i64_String`. Measured: the C contains both
/// `const char* snd__i64_String(long long a, const char* b)` and
/// `long long snd__i64_i64(long long a, long long b)`, and emits
/// `__pd_print_int(snd__i64_String(1, 2))` — the correct monomorphization is
/// emitted and never called.
///
/// THIS IS A DECLARED ROW RATHER THAN A DOC COMMENT ON PURPOSE. Nothing gates a
/// doc comment: it cannot fail when the defect worsens, and nothing notices when
/// it is fixed. `make test-xfail` does both.
///
/// The end of the same function is a second, quieter half of the defect: when
/// nothing matches at all it returns `instantiations_for_func[0]` rather than
/// `None`, so a call with no candidate silently gets the first one and no
/// diagnostic is produced anywhere.
#[test]
#[ignore = "XFAIL: a generic call with several candidate monomorphizations resolves to the wrong one — `get_mangled_name_for_call` matches any type argument in any position against the FIRST argument's inferred type, so with `fn snd<A, B>(a: A, b: B) -> B` instantiated at (i64, String) and (i64, i64), `snd(1, 2)` lowers to `snd__i64_String(1, 2)` and gcc reports \"incompatible pointer to integer conversion\"; the correct `snd__i64_i64` is emitted and never called, and when nothing matches the function silently returns the first instantiation instead of None (owned by M4, generics and monomorphization)"]
fn test_a_generic_call_resolves_to_its_own_monomorphization() {
    let (compiled, output, stdout) = compile_and_run(
        &[],
        "fn snd<A, B>(a: A, b: B) -> B { return b; }\n\n\
         fn main() {\n    print(snd(1, \"x\"));\n    print_int(snd(1, 2));\n}\n",
    );
    assert!(
        !output.contains("gcc compilation failed"),
        "the call was lowered to the wrong monomorphization and gcc is what \
         noticed; compiler said (compiled={}):\n{}",
        compiled,
        output
    );
    assert_eq!(
        stdout.as_deref(),
        Some("x\n2\n"),
        "each call must run its own monomorphization; compiler said:\n{}",
        output
    );
}

// ---------------------------------------------------------------------------
// The seventh site: the type checker's own import tables
// ---------------------------------------------------------------------------

/// TWO MODULES, ONE GENERIC NAME — and before the fix, the compiled program's
/// ANSWER depended on the hash seed.
///
/// This is the one site where the ordering defect reached past the shape of the
/// emitted C and into what the program computes. `TypeChecker::set_imported_modules`
/// inserts under the BARE name, last writer wins, and `get_instantiations` looks
/// generic bodies up by bare name and hands the winner to codegen's monomorphizer.
/// So with `pick` exported by both `liba` and `libb`, whichever module the
/// `HashMap` happened to visit second supplied the body that got compiled.
///
/// Measured before the sort, on ONE unchanged program: 20 runs printed `111` ten
/// times and `222` ten times. After it: 20 of 20 the same.
///
/// N IS 20 HERE, NOT 8, AND THE DIFFERENCE IS THE POINT. The other determinism
/// tests draw from a space of 720 orderings, where eight agreeing compiles is
/// (1/720)^7. This outcome space is BINARY, so eight draws would leave a 2^-7
/// (~0.8%) chance of a false pass — three orders of magnitude worse than every
/// other assertion in this file. Twenty draws puts it at 2^-19. The sample size
/// belongs to the measurement, not to the file.
#[test]
fn test_two_modules_exporting_one_generic_name_are_stable() {
    let modules = [
        ("liba.pd", "pub fn pick<T>(v: T) -> i64 { return 111; }\n"),
        ("libb.pd", "pub fn pick<T>(v: T) -> i64 { return 222; }\n"),
    ];
    let main_src = "import liba;\nimport libb;\n\nfn main() {\n    print_int(pick(7));\n}\n";

    let mut answers: Vec<String> = Vec::new();
    for i in 0..20 {
        let (compiled, output, stdout) = compile_and_run(&modules, main_src);
        assert!(
            compiled,
            "run #{} did not compile; compiler said:\n{}",
            i, output
        );
        let printed = stdout.unwrap_or_default();
        assert!(
            printed == "111\n" || printed == "222\n",
            "run #{} printed {:?}, which is neither module's body — the \
             monomorphizer picked something else entirely",
            i,
            printed
        );
        answers.push(printed);
    }

    let first = &answers[0];
    for (i, a) in answers.iter().enumerate().skip(1) {
        assert_eq!(
            a, first,
            "run #{} printed {:?} where run #0 printed {:?}: which module's body \
             gets monomorphized is decided by the per-process hash seed",
            i, a, first
        );
    }
}

// ---------------------------------------------------------------------------
// An imported ENUM is an enum on both sides of the import
// ---------------------------------------------------------------------------

/// An imported enum used as a parameter and a return type.
///
/// `set_imported_modules` collects imported enum NAMES into `enum_names` and
/// carried a comment saying the registration below reads them. It did not: every
/// conversion went through `CheckerType::from`, whose `Custom` arm is an
/// associated function that can consult no set at all and calls every named type
/// a struct. So `red`'s return type was `Struct("Color")` while `let c: Color`
/// went through the context-aware path and was `Enum("Color")`, and this program
/// was refused with
///
/// ```text
/// error: Type mismatch: expected Color, found Color
/// ```
///
/// — the same one-type-on-both-sides diagnostic this branch removed for local
/// declarations, reached one container over, by an import.
///
/// The SECOND break, behind it: nothing put an imported enum into `self.enums`,
/// which is where the variant path expression and the `match` pattern both look
/// a variant up, so with the kinds fixed the program failed with
/// `Undefined enum type: Color` instead. Neither fix has a witness without the
/// other, so this test drives the whole chain and asserts the ANSWER.
#[test]
fn test_an_imported_enum_is_an_enum_in_a_signature() {
    let (compiled, output, stdout) = compile_and_run(
        &[(
            "lib9.pd",
            "pub enum Color { Red, Green }\n\
             pub fn kind(c: Color) -> i64 { return 7; }\n\
             pub fn red() -> Color { return Color::Red; }\n",
        )],
        "import lib9;\n\nfn main() {\n    let c: Color = red();\n    print_int(kind(c));\n}\n",
    );
    assert!(
        !output.contains("expected Color, found Color"),
        "an imported enum is still registered as a struct:\n{}",
        output
    );
    assert!(compiled, "compilation failed:\n{}", output);
    assert_eq!(
        stdout.as_deref(),
        Some("7\n"),
        "the imported enum did not survive the round trip; compiler said:\n{}",
        output
    );
}

/// All three variant shapes of an imported enum, constructed and matched in the
/// IMPORTING program.
///
/// The test above never writes `Color::Red` outside the module, so it passes
/// under a registration that only reaches imported bodies. This one constructs a
/// unit, a tuple and a named variant in `main` and matches on one there, which is
/// what makes `self.enums` — not just `self.functions` — the thing under test.
#[test]
fn test_an_imported_enums_variants_are_constructible_and_matchable() {
    let (compiled, output, stdout) = compile_and_run(
        &[(
            "libshape.pd",
            "pub enum Shape { Dot, Line(i64), Box2 { w: i64, h: i64 } }\n\
             pub fn area(s: Shape) -> i64 {\n\
             \x20   match s {\n\
             \x20       Shape::Dot => { return 0; }\n\
             \x20       Shape::Line(n) => { return n; }\n\
             \x20       Shape::Box2 { w: ww, h: hh } => { return ww * hh; }\n\
             \x20   }\n\
             }\n",
        )],
        "import libshape;\n\n\
         fn main() {\n\
         \x20   let a: Shape = Shape::Line(5);\n\
         \x20   let b: Shape = Shape::Box2 { w: 3, h: 4 };\n\
         \x20   let c: Shape = Shape::Dot;\n\
         \x20   print_int(area(a));\n\
         \x20   print_int(area(b));\n\
         \x20   match c {\n\
         \x20       Shape::Dot => { print(\"dot\"); }\n\
         \x20       Shape::Line(n) => { print_int(n); }\n\
         \x20       Shape::Box2 { w: ww, h: hh } => { print(\"box\"); }\n\
         \x20   }\n\
         }\n",
    );
    assert!(
        !output.contains("Undefined enum type"),
        "an imported enum's variants are not registered:\n{}",
        output
    );
    assert!(compiled, "compilation failed:\n{}", output);
    assert_eq!(
        stdout.as_deref(),
        Some("5\n12\ndot\n"),
        "an imported enum's variants did not round-trip; compiler said:\n{}",
        output
    );
}

/// A LOCAL declaration wins over an imported one, in the TYPE namespace.
///
/// The enum-kind decision was a bare-name UNION of local and imported enum
/// names, so an imported enum reached into a downstream program's name space
/// and misclassified an unrelated local type:
///
/// ```text
/// lib.pd:   pub enum Color { Red, Green }
/// main.pd:  import lib;
///           struct Color { v: i64 }
/// -> error: Type mismatch: expected Color, found Color
/// ```
///
/// That is the same one-type-on-both-sides diagnostic the recursive-data-types
/// work removed for local declarations, REOPENED through the import path by the
/// fix that made imported enums usable at all. `let c: Color` was classified
/// `Enum("Color")` because an imported enum somewhere had claimed the name.
///
/// TWO PASSES HAD TO LEARN IT, which is why `crate::ast::local_type_shadows_import`
/// lives beside its function-namespace sibling rather than inside either pass.
/// With only the type checker fixed the refusal moved to
/// `main.c:280:16: error: redefinition of 'Color'`, because code generation was
/// still emitting the imported enum AND the local struct under one C tag.
#[test]
fn test_a_local_struct_wins_over_an_imported_enum_of_the_same_name() {
    let (compiled, output, stdout) = compile_and_run(
        &[("lib.pd", "pub enum Color { Red, Green }\n")],
        "import lib;\n\nstruct Color { v: i64 }\n\nfn main() {\n    let c: Color = Color { v: 7 };\n    print_int(c.v);\n}\n",
    );
    assert!(
        !output.contains("expected Color, found Color"),
        "an imported enum still decides the kind of a local struct:\n{}",
        output
    );
    assert!(
        !output.contains("redefinition of 'Color'"),
        "code generation still emits both definitions under one C tag:\n{}",
        output
    );
    assert!(compiled, "compilation failed:\n{}", output);
    assert_eq!(
        stdout.as_deref(),
        Some("7\n"),
        "the local struct did not win; compiler said:\n{}",
        output
    );
}

/// The same, with a PRIVATE imported enum, which must not reach the program at
/// all.
///
/// Two claims in one program, and the second is why this is not a duplicate of
/// the test above. First: a module's `enum` that did not say `pub` cannot
/// misclassify a downstream local type. Second: it could not have been written
/// before 2026-08-23, because `EnumDef` carried no visibility field and
/// `src/parser/mod.rs` dropped the `pub` it had just parsed for this one item
/// kind — `enum Color` and `pub enum Color` were the same AST, so no filter
/// anywhere could tell them apart.
#[test]
fn test_a_private_imported_enum_does_not_reach_a_local_type_of_the_same_name() {
    let (compiled, output, stdout) = compile_and_run(
        &[("lib.pd", "enum Color { Red, Green }\n")],
        "import lib;\n\nstruct Color { v: i64 }\n\nfn main() {\n    let c: Color = Color { v: 7 };\n    print_int(c.v);\n}\n",
    );
    assert!(
        !output.contains("expected Color, found Color"),
        "a private imported enum still decides the kind of a local struct:\n{}",
        output
    );
    assert!(compiled, "compilation failed:\n{}", output);
    assert_eq!(stdout.as_deref(), Some("7\n"), "compiler said:\n{}", output);
}

/// `pub` on an enum is load-bearing, in the refusing direction, AND the refusal
/// is a diagnostic rather than a gcc error.
///
/// The control for the two tests above: they prove a private enum does not
/// misclassify anything, which a filter that dropped ALL imported enums would
/// also satisfy. This proves the filter is a VISIBILITY filter — the same
/// program is accepted with `pub` and refused without it.
///
/// The stage matters as much as the verdict. With only the emission side
/// filtered, the type checker had stopped calling the name an enum while
/// `set_imported_modules` still registered the variant constructor, so
/// `Color::Red` resolved and the program died in gcc — "front end successful,
/// gcc failed", the class this whole branch exists to remove. The registration
/// tests visibility now too, so the refusal is `Undefined enum type: Color`
/// before any C exists.
#[test]
fn test_pub_on_an_imported_enum_decides_whether_it_can_be_used() {
    const MAIN: &str =
        "import lib;\n\nfn main() {\n    let c: Color = Color::Red;\n    print_int(kind(c));\n}\n";

    let (compiled, output, stdout) = compile_and_run(
        &[(
            "lib.pd",
            "pub enum Color { Red, Green }\npub fn kind(c: Color) -> i64 { return 7; }\n",
        )],
        MAIN,
    );
    assert!(
        compiled,
        "a `pub enum` must be usable downstream:\n{}",
        output
    );
    assert_eq!(stdout.as_deref(), Some("7\n"), "compiler said:\n{}", output);

    let (compiled, output, _) = compile_and_run(
        &[(
            "lib.pd",
            "enum Color { Red, Green }\npub fn kind(c: Color) -> i64 { return 7; }\n",
        )],
        MAIN,
    );
    assert!(
        !compiled,
        "an enum with no `pub` must not be usable downstream:\n{}",
        output
    );
    assert!(
        output.contains("Undefined enum type: Color"),
        "the refusal must come from the front end by name, not from gcc:\n{}",
        output
    );
    assert!(
        !output.contains("gcc compilation failed"),
        "the refusal reached gcc instead of a diagnostic:\n{}",
        output
    );
}

// ---------------------------------------------------------------------------
// The layout analysis sees the same item set the other two passes do
// ---------------------------------------------------------------------------

/// AN IMPORT OF PRIVATE TYPES THE PROGRAM NEVER MENTIONS DECIDED WHETHER A VALID
/// PROGRAM COMPILED.
///
/// `RecursiveLayout::analyze` keys every declaration by BARE NAME, and both
/// callers handed it `program.items` chained onto every imported module's items,
/// unfiltered. `local_type_shadows_import` governed registration and emission and
/// did not govern this — the third consumer, and the one that decides whether a
/// program is ACCEPTED.
///
/// ```text
/// lib.pd    struct B { x: i64 }        // private, never named downstream
///           struct A { b: B }          // private, never named downstream
/// ```
///
/// The local `enum A` cuts at its payload slot, so the local graph has no
/// unbroken cycle. Merging the hidden imported `struct A { b: B }` in by name
/// creates `A -> B -> A`, and the program was refused with
/// `recursive type `A` has no layout`. Deleting the `import` line — changing
/// nothing the program says about `A` or `B` — made it compile and run.
///
/// The polarity is the bad one: the refusal fails CLOSED onto a valid program.
///
/// This lives here rather than in `tests/m2_recursive_data_types.rs`, which owns
/// the layout analysis, because the case needs two files and that file's harness
/// compiles one.
#[test]
fn test_a_private_imported_type_does_not_decide_a_local_layout() {
    const MAIN: &str = "enum A { End, More(B) }\n\
                        struct B { a: A }\n\
                        fn main() {\n\
                        \x20   let b: B = B { a: A::End };\n\
                        \x20   print(\"built\");\n\
                        }\n";

    let (compiled, output, stdout) = compile_and_run(
        &[("lib.pd", "struct B { x: i64 }\nstruct A { b: B }\n")],
        &format!("import lib;\n{}", MAIN),
    );
    assert!(
        !output.contains("has no layout"),
        "a private imported declaration is still merged into the local graph:\n{}",
        output
    );
    assert!(compiled, "compilation failed:\n{}", output);
    assert_eq!(
        stdout.as_deref(),
        Some("built\n"),
        "compiler said:\n{}",
        output
    );

    // THE CONTROL, so this test fails for the right reason if the filter is
    // removed. Byte-identical program with no `import` line: if this ever stops
    // compiling, the defect is in the layout analysis itself and not in what the
    // import contributes to it, and the assertion above would be blaming the
    // wrong pass.
    let (compiled, output, stdout) = compile_and_run(&[], MAIN);
    assert!(
        compiled,
        "the same program without the import must compile:\n{}",
        output
    );
    assert_eq!(
        stdout.as_deref(),
        Some("built\n"),
        "compiler said:\n{}",
        output
    );
}

/// The filter must not blind the analysis to imports the program CAN name.
///
/// Three shapes, because "ignore the imports" would satisfy the test above
/// completely. A cycle wholly inside the module, a cycle that spans the seam, and
/// a private cycle that is nobody's problem — the first two must still be
/// refused, the third must not be, and only a filter that is a VISIBILITY filter
/// gets all three right.
#[test]
fn test_a_public_imported_cycle_is_still_refused() {
    // Wholly inside the module, and public, so the program could name either end.
    let (compiled, output, _) = compile_and_run(
        &[("liba.pd", "pub struct A { b: B }\npub struct B { a: A }\n")],
        "import liba;\n\nfn main() {\n    print(\"x\");\n}\n",
    );
    assert!(
        !compiled,
        "a public imported cycle must be refused:\n{}",
        output
    );
    assert!(
        output.contains("has no layout"),
        "and refused as a layout defect, by name:\n{}",
        output
    );

    // Spanning the seam: half the cycle is imported, half is local.
    let (compiled, output, _) = compile_and_run(
        &[("libp.pd", "pub struct P { q: Q }\n")],
        "import libp;\n\nstruct Q { p: P }\n\nfn main() {\n    print(\"x\");\n}\n",
    );
    assert!(
        !compiled,
        "a cycle spanning the import seam must be refused:\n{}",
        output
    );
    assert!(
        output.contains("has no layout"),
        "and refused as a layout defect, by name:\n{}",
        output
    );

    // Private, and therefore unnameable and unconstructible downstream. Code
    // generation does not emit it either, so there is nothing for the refusal to
    // protect.
    let (compiled, output, stdout) = compile_and_run(
        &[("libr.pd", "struct R { s: S }\nstruct S { r: R }\n")],
        "import libr;\n\nfn main() {\n    print(\"x\");\n}\n",
    );
    assert!(
        compiled,
        "a private imported cycle the program cannot name is not its problem:\n{}",
        output
    );
    assert_eq!(stdout.as_deref(), Some("x\n"), "compiler said:\n{}", output);
}
