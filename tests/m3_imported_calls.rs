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
//! AST (`src/driver/mod.rs:137-138`) while `resolved_modules` sat live and unused in
//! the same scope. Its function table was seeded from `crate::builtins::BUILTINS` and
//! nothing else (`src/ownership/borrow_checker.rs:114-118`), and its first pass walks
//! `program.items` only — `Program.imports` (`src/ast/mod.rs:9`) is never read, and
//! `Item` (`src/ast/mod.rs:24-32`) has no `Import` variant, so no walk over items could
//! have reached one. `helper()` therefore fell out of the function table at
//! `Expr::Ident` (`:502`), was looked up as a *value*, was not found, and died at
//! `:527` as `UseOfUninitializedValue`.
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
// They are `#[ignore = "XFAIL: … (owned by M3)"]` because that is this repo's
// mechanism for a measured, owned debt: `scripts/test-xfail.py` (via
// `make test-xfail`) runs every ignored test and fails the gate if a declared
// failure PASSES. So paying one off is a TRANSITION — delete the `#[ignore]`
// and let the test join the regression net — and never a deletion. M3 is the
// owner because M3 is where modules live (docs/contributing/MILESTONES.md:112-121,
// "no enums, no `match`, no `for`, no generics and no modules"), and it is the
// owner both existing module rows in tests/conformance-manifest.txt already
// carry.
//
// TWO OF THESE ARE NOT MISSING FEATURES. #1 and #2 below are holes in checks
// that already exist and that the language already enforces on the same code
// written locally. Read their reasons before assuming this whole block is
// "modules aren't finished yet".

/// SOUNDNESS HOLE — not a missing feature.
///
/// `import` currently launders an immutable binding into a mutated one. The
/// borrow checker's second pass walks `program.items` only
/// (`src/ownership/borrow_checker.rs:256-270`), so no imported body is ever
/// visited. The signature is registered — that is what makes the call
/// check — but the body behind it is trusted.
///
/// The control is `test_local_twin_of_the_unchecked_import_is_rejected` below:
/// the byte-identical program with `bad`/`caller` written locally IS rejected.
/// So this is not "the checker cannot see this yet"; it is the same checker,
/// the same program, reaching the opposite verdict depending on which file the
/// function lives in. Generated C: `void bad(long long* x)` and `bad(&n)` —
/// the write really does reach the caller's storage.
#[test]
#[ignore = "XFAIL: SOUNDNESS — imported function bodies are never borrow-checked; the second pass walks `program.items` only (src/ownership/borrow_checker.rs:256-270), so a module can mutate an immutable binding: `pub fn bad(mut x: i64) { x = 42; }` called from `caller` with `let n = 1;` compiles, links and prints 42, while the identical program written locally is rejected with \"cannot borrow `n` as mutable\" (owned by M3, cross-file module imports)"]
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

/// The same shape one pass over: imported bodies are not TYPE-checked either
/// (`src/typeck/mod.rs:783-788`, second pass over `program.items`). The user is
/// told `✅ Compilation successful!` and then handed a gcc error against C they
/// never wrote, which is precisely the class of failure M1 exists to remove.
#[test]
#[ignore = "XFAIL: imported function bodies are never type-checked; the second pass walks `program.items` only (src/typeck/mod.rs:783-788), so `pub fn broken() -> i64 { let s = \"x\"; return s; }` in a module reports \"Compilation successful\" and then dies in gcc with \"incompatible pointer to integer conversion returning 'const char *'\" (owned by M3, cross-file module imports)"]
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
/// `src/resolver/mod.rs:190` is `let _sub_modules = self.resolve_program(&ast)?;`,
/// and `resolve_program` (`:70-95`) builds its returned map from the TOP-LEVEL
/// program's `program.imports` alone. The sub-module lands in the resolver's
/// private cache and never reaches typeck, the borrow checker or codegen, so
/// `outer`'s body is emitted calling a `base` that was never emitted.
///
/// It works if `main` also imports `liba` directly — which is the diamond case,
/// and is why this is a missing hop rather than a missing feature.
#[test]
#[ignore = "XFAIL: transitive imports are resolved and thrown away — src/resolver/mod.rs:190 discards the recursive result into `_sub_modules` and src/resolver/mod.rs:70-95 returns only the top-level program's own imports, so `main -> libb -> liba` emits `outer`'s body calling `base` and gcc reports \"call to undeclared function 'base'\" (owned by M3, cross-file module imports)"]
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

/// Selective import is a no-op, and `ModuleInfo.exports` is dead data.
/// `filter_module_info` (`src/resolver/mod.rs:105-118`) narrows the `exports`
/// set and leaves `ast` complete — and `.exports` is read nowhere but its own
/// filter (`src/resolver/mod.rs:113` is the only hit in `src/`). Every consumer
/// re-derives visibility from `ast.items` instead: `src/typeck/mod.rs:411`,
/// `src/codegen/mod.rs:1114`/`:1193`/`:1278`/`:1741`, and the borrow checker's
/// `register_imported_functions`. So `import lib2::{helper};` imports the whole
/// module.
#[test]
#[ignore = "XFAIL: `import m::{a};` imports all of `m` — src/resolver/mod.rs:105-118 filters the `exports` set but not `ast`, and `.exports` is read nowhere but its own filter (src/resolver/mod.rs:113), so every consumer re-derives visibility from `ast.items`; a name the import did not list compiles and runs (owned by M3, cross-file module imports)"]
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
/// checkers (the local one wins; see `register_imported_functions`) and then
/// contradicted by codegen, which emits every public imported function
/// (`src/codegen/mod.rs:1278-1289`) and then every local one (`:1291+`) with no
/// shadowing check at all. The front end's answer is right and unenforceable.
#[test]
#[ignore = "XFAIL: a local definition that shadows an imported one emits BOTH into the C — src/codegen/mod.rs:1278-1289 emits every public imported function and :1291+ every local one, with no shadowing check, so gcc reports \"redefinition of 'helper'\" even though both checkers correctly resolved the call to the local definition (owned by M3, cross-file module imports)"]
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
/// `register_imported_functions` picks a winner in sorted key order so the
/// choice is at least deterministic, but it is still arbitrary — and codegen
/// emits both bodies regardless, so the user meets it as a C redefinition
/// against code they never wrote.
///
/// The assertion is deliberately about WHERE the refusal comes from, not
/// whether one happens: gcc already refuses this. A compiler that hands invalid
/// C to gcc and lets gcc explain has not diagnosed anything.
#[test]
#[ignore = "XFAIL: two imported modules exporting the same name are not diagnosed — the front end silently picks one (sorted-key order, src/ownership/borrow_checker.rs `register_imported_functions`) and codegen emits both bodies (src/codegen/mod.rs:1278-1289), so the user meets it as gcc's \"redefinition of 'dup'\" against C they never wrote (owned by M3, cross-file module imports)"]
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

/// A qualified call cannot be written. `src/parser/mod.rs:2504-2545` turns any
/// `a::b(...)` into `Expr::EnumConstructor`, and `src/typeck/mod.rs:2175-2180`
/// then reports `Undefined enum type: lib2`. The same holds for an alias
/// (`import lib2 as m;` → `Undefined enum type: m`), which makes `alias`
/// unusable too. `register_imported_functions` registers `module::name` for
/// parity with the type checker, but nothing can currently reach it.
#[test]
#[ignore = "XFAIL: a qualified call `lib2::helper()` is unreachable — src/parser/mod.rs:2504-2545 turns every `a::b(...)` into Expr::EnumConstructor and src/typeck/mod.rs:2175-2180 then reports \"Undefined enum type: lib2\"; the same makes `import lib2 as m;` unusable, since `m::helper()` reports \"Undefined enum type: m\" (owned by M3, cross-file module imports)"]
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
/// resolver. `src/parser/mod.rs:213-224`: after `::`, if the token after the
/// next one is `;`, `,` or `{`, the segment is consumed as an ITEM name. So
/// `import util::math;` parses as `path=["util"], items=["math"]` and the
/// resolver looks for `util.pd`. The last segment of a path can never be a
/// module, which means a module tree deeper than one level is unexpressible.
#[test]
#[ignore = "XFAIL: nested module paths are unexpressible — src/parser/mod.rs:213-224 consumes the segment after `::` as an ITEM name whenever the following token is `;`/`,`/`{`, so `import util::math;` parses as path=[\"util\"] items=[\"math\"] and the resolver reports \"Module 'util' not found\" for the directory (owned by M3, cross-file module imports)"]
fn test_a_module_in_a_subdirectory_can_be_imported() {
    let (compiled, output, stdout) = compile_and_run(
        &[("util/math.pd", "pub fn sq(x: i64) -> i64 { return x * x; }\n")],
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
/// (`src/codegen/mod.rs:2723`), while the type checker
/// (`src/typeck/mod.rs:455-456`) and `register_imported_functions` both insert
/// the imported signature OVER the built-in.
///
/// With a matching signature the imported definition is merely silent dead code.
/// With a differing one — the case here — the divergence is load-bearing: the
/// type checker accepts `print_int("hello")` against the module's `String`
/// signature and codegen emits `__pd_print_int("hello")` against the built-in's
/// `long long`.
#[test]
#[ignore = "XFAIL: an imported name that shadows a built-in is registered by the checkers but ignored by codegen — src/typeck/mod.rs:455-456 and the borrow checker insert the imported signature over the built-in, while src/codegen/mod.rs:2723 tests is_builtin() first and unconditionally, so `pub fn print_int(s: String)` type-checks `print_int(\"hello\")` and then emits `__pd_print_int(\"hello\")`, which gcc rejects as \"incompatible pointer to integer conversion\" (owned by M3, cross-file module imports)"]
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
