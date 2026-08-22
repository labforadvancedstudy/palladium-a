//! N7-18: the async producer. `async fn` is refused; it no longer emits a
//! runtime representation of an effect.
//!
//! THE VIOLATION, measured on `main` at acda322:
//!
//! ```text
//! async fn g() { print("x"); }
//! fn main() { print_int(1); }
//! -> ✅ Compilation successful
//!
//! // Future struct for async function g
//! typedef struct g_Future { int state; } g_Future;
//! int g_poll(g_Future *future) { … }
//! g_Future g() { … }
//! ```
//!
//! §N7 of the language specification says effect tracking "is entirely static
//! and has no runtime representation". A struct with a `state` field and a poll
//! routine, emitted into the program's own C, is a runtime representation — and
//! nothing calls `g_poll`, so the body never ran either.
//!
//! WHY REFUSAL AND NOT DELETION OF THE EMISSION. Dropping the Future struct and
//! compiling the body as an ordinary function would leave `async` a keyword the
//! compiler silently ignores. That is the class M1 exists to remove, not a fix.
//! The refusal is shaped exactly like `?` and `.await`: a
//! `CompileError::Unimplemented` naming the construct, what would have been
//! emitted, and a workaround that compiles. The keyword dies at M5.
//!
//! WHAT THIS FILE SPENDS MOST OF ITS LINES ON IS THE OTHER POLARITY. A refusal
//! that over-approximates fails closed onto valid programs, which is how the
//! `async fn main` rule rejected two good programs when it fired on "declared
//! somewhere" instead of "is the entry point". Every control below is a program
//! that must still COMPILE, LINK AND RUN.

use palladium::linker::{link_command, OptLevel};
use palladium::{CompileError, Driver};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

mod common;
use common::unique_module_name;

fn rendered(e: CompileError) -> String {
    let d = e.to_diagnostic();
    let mut out = vec![d.message.clone()];
    out.extend(d.notes.iter().cloned());
    out.extend(d.suggestions.iter().map(|s| s.message.clone()));
    out.join("\n")
}

/// Run the full driver over `source`; Ok carries the emitted C.
fn compile_to_c(source: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", unique_module_name(name)));
    fs::write(&src, source).unwrap();
    let c_file: PathBuf = Driver::new().compile_file(&src).map_err(rendered)?;
    fs::read_to_string(&c_file).map_err(|e| format!("reading {}: {}", c_file.display(), e))
}

/// Compile, link against the real runtime, run, and return stdout.
///
/// The accept side is not "it compiled": D3 printed 8261746944 and exited 0.
/// Every control in this file therefore goes all the way to a number.
fn compile_and_run(source: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let stem = unique_module_name(name);
    let src = dir.path().join(format!("{}.pd", stem));
    let exe = dir.path().join(&stem);
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
            "program exited {:?}: {}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).to_string())
}

/// Compile a two-file program (`lib.pd` imported by `app.pd`) with the real
/// `pdc`, run it, and return its stdout.
///
/// A SUBPROCESS for the reason `tests/d3b_tail_if.rs` gives: the resolver looks
/// for `<module>.pd` beside the file being compiled and codegen writes into
/// `build_output/` relative to the CURRENT DIRECTORY, so an in-process test
/// would have to change the working directory of a process running other tests
/// on parallel threads.
fn compile_and_run_with_import(lib: &str, app: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("lib.pd"), lib).unwrap();
    fs::write(dir.path().join("app.pd"), app).unwrap();
    let stem = unique_module_name(name);

    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .args(["compile", "app.pd", "-o", &stem])
        .current_dir(dir.path())
        .output()
        .expect("failed to run pdc");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success() {
        return Err(text);
    }
    let exe = dir.path().join("build_output").join(&stem);
    let run = Command::new(&exe)
        .output()
        .map_err(|e| format!("running {}: {}", exe.display(), e))?;
    if !run.status.success() {
        return Err(format!(
            "{} exited {:?}: {}",
            exe.display(),
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).to_string())
}

// ---------------------------------------------------------------------------
// The reproduction
// ---------------------------------------------------------------------------

/// THE REPRO. Red before this branch, refused after.
///
/// Two assertions, not one. "It is refused" alone would be satisfied by a
/// refusal for the wrong reason, and the second half of a refusal is that
/// nothing was emitted: a compiler that both diagnoses and writes the file is
/// the shape the D5 arms were added to prevent.
#[test]
fn the_async_producer_is_refused_and_emits_nothing() {
    let err = compile_to_c(
        "async fn g() { print(\"x\"); }\nfn main() { print_int(1); }\n",
        "m2_async_producer",
    )
    .expect_err("`async fn g` emitted a Future struct and a poll function");

    assert!(
        err.contains("`async fn` is not implemented"),
        "the refusal must name the construct:\n{}",
        err
    );
    assert!(
        err.contains("_Future") && err.contains("_poll"),
        "and say what WOULD have been emitted — that is why this is a refusal \
         rather than a silent drop:\n{}",
        err
    );
    assert!(
        err.contains("§N7"),
        "and cite the rule it enforces, since the reason is normative rather \
         than technical:\n{}",
        err
    );
    assert!(
        err.contains("delete the `async` keyword"),
        "and name a workaround, like every other Unimplemented refusal:\n{}",
        err
    );
}

/// The workaround the diagnostic names COMPILES, LINKS AND RUNS.
///
/// A help string is advice until it is executed. This is the same program with
/// the keyword deleted, run to a number.
#[test]
fn the_workaround_compiles_and_runs() {
    let out = compile_and_run(
        "fn g() { print(\"x\"); }\nfn main() { g(); print_int(1); }\n",
        "m2_async_workaround",
    )
    .expect("deleting `async` is the advice; it has to work");
    assert_eq!(out, "x\n1\n", "got: {:?}", out);
}

/// EVERY spelling that reaches code generation, not the one I could think of.
///
/// The plain unit function is the repro. The others are the shapes that made
/// the earlier async refusals partial: a call site (so the function is not dead
/// code), parameters (they became fields of the Future struct), a declared unit
/// return (`-> ()` is a different `Option<Type>` from no annotation), `pub`
/// (visibility is not part of the rule), and a generic — its own ingress into
/// code generation, instantiated and not.
#[test]
fn no_async_declaration_reaches_code_generation() {
    for (n, src) in [
        "async fn g() { print(\"x\"); }\nfn main() { print_int(1); }",
        "async fn g() { print(\"x\"); }\nfn main() { g(); }",
        "async fn g(a: i64, b: i64) { print_int(a + b); }\nfn main() { g(1, 2); }",
        "async fn g() -> () { print(\"x\"); }\nfn main() { g(); }",
        "pub async fn g() { print(\"x\"); }\nfn main() { print_int(1); }",
        "async fn g<T>(x: T) { print(\"x\"); }\nfn main() { g(7); }",
        "async fn g<T>(x: T) { print(\"x\"); }\nfn main() { print_int(1); }",
    ]
    .iter()
    .enumerate()
    {
        match compile_to_c(src, &format!("m2_async_reach_{}", n)) {
            Ok(c) => panic!(
                "spelling {} reached code generation:\n{}\n--- emitted C ---\n{}",
                n, src, c
            ),
            Err(e) => assert!(
                e.contains("async fn"),
                "spelling {} was refused for something other than being async:\n{}",
                n,
                e
            ),
        }
    }
}

/// An IMPORTED `pub async fn` that IS part of the emitted program is refused.
///
/// This is a separate route: only local functions used to reach
/// `check_function`. The refusal is reached through `check`'s third pass, which
/// hands every public, non-generic, unshadowed imported function to the same
/// predicate — so there is one rule here, not a copy of it.
#[test]
fn an_imported_async_fn_that_is_emitted_is_refused() {
    let err = compile_and_run_with_import(
        "pub async fn g() { print(\"x\"); }\npub fn ok() { print_int(1); }\n",
        "import lib;\n\nfn main() { ok(); }\n",
        "m2_imported_async",
    )
    .expect_err("codegen emits this imported body, so it is the program's own violation");
    assert!(
        err.contains("`async fn` is not implemented"),
        "got:\n{}",
        err
    );
}

/// An imported GENERIC `async fn` that is INSTANTIATED is refused, and the
/// diagnostic names it.
#[test]
fn an_instantiated_imported_generic_async_fn_is_refused() {
    let err = compile_and_run_with_import(
        "pub async fn ag<T>(x: T) { print(\"x\"); }\npub fn ok() { print_int(1); }\n",
        "import lib;\n\nfn main() { ag(7); ok(); }\n",
        "m2_imported_generic_async",
    )
    .expect_err("an instantiated imported generic IS emitted");
    assert!(
        err.contains("`async fn`") && err.contains("not implemented"),
        "got:\n{}",
        err
    );
    assert!(
        err.contains("`ag`"),
        "an import refusal must name the offending declaration, because the \
         programmer did not write it in the file being compiled:\n{}",
        err
    );
}

// ---------------------------------------------------------------------------
// WHAT THE REFUSAL MUST NOT TOUCH
//
// Over-approximating a refusal fails closed onto valid programs. That has
// already happened twice on this exact rule, and each control below is a
// program that must still compile, link and RUN.
// ---------------------------------------------------------------------------

/// Ordinary functions are untouched: the predicate is `Function::is_async` and
/// nothing else.
///
/// Deliberately includes the things a name- or type-based rule would have
/// caught: a function CALLED `async_thing`, one that returns a value, one with
/// an early return, and `main` itself.
#[test]
fn ordinary_functions_still_compile_link_and_run() {
    let out = compile_and_run(
        "fn async_thing(n: i64) -> i64 {\n\
         \x20   if n <= 1 { return n; }\n\
         \x20   return n * 2;\n\
         }\n\
         fn shout() { print(\"hi\"); }\n\
         fn main() {\n\
         \x20   shout();\n\
         \x20   print_int(async_thing(1));\n\
         \x20   print_int(async_thing(21));\n\
         }\n",
        "m2_ordinary_fns",
    )
    .expect("no ordinary function may be caught by an async rule");
    assert_eq!(out, "hi\n1\n42\n", "got: {:?}", out);
}

/// Generic functions — the other ingress — still compile, link and run.
///
/// `monomorphize_function` now carries `is_async` from the template instead of
/// hardcoding `false`. If that had been mistranscribed, every generic would be
/// refused; this is the line that would say so.
#[test]
fn ordinary_generics_still_compile_link_and_run() {
    let out = compile_and_run(
        "fn twice<T>(x: T) -> T { return x; }\n\
         fn main() { print_int(twice(21) + twice(21)); }\n",
        "m2_ordinary_generic",
    )
    .expect("a non-async generic must be unaffected by the async rule");
    assert_eq!(out.trim(), "42", "got: {:?}", out);
}

/// `.await` keeps its OWN diagnostic. The two refusals are not merged.
///
/// Written on an ordinary call, so `async fn` is not in the program at all: if
/// the new rule had swallowed this one, the message would change.
#[test]
fn await_keeps_its_own_refusal() {
    let err = compile_to_c(
        "fn work() -> i64 { return 1; }\nfn main() { let v: i64 = work().await; }\n",
        "m2_await_untouched",
    )
    .expect_err("`.await` is still unimplemented");
    assert!(err.contains("`.await`"), "got:\n{}", err);
    assert!(
        !err.contains("`async fn` is not implemented"),
        "there is no `async fn` in this program:\n{}",
        err
    );
}

/// `async fn main` keeps its OWN, more specific diagnostic.
///
/// The general arm is ordered after it deliberately: this message names what
/// the entry point would have been emitted as, which the general one cannot.
#[test]
fn async_main_keeps_its_more_specific_refusal() {
    let err = compile_to_c(
        "async fn main() { print_int(7); }\n",
        "m2_async_main_specific",
    )
    .expect_err("`async fn main` is still refused");
    assert!(err.contains("`async fn main`"), "got:\n{}", err);
    assert!(
        err.contains("main_Future main()"),
        "the entry-point message says what would have been emitted; the general \
         arm must not have displaced it:\n{}",
        err
    );
}

/// A value-carrying return inside an `async fn` keeps its OWN diagnostic.
///
/// `tests/conformance-manifest.txt` fingerprints this exact text for
/// `tests/reject/async_fn.pd`, so a general arm that fired first would move a
/// fingerprint while claiming to add coverage.
#[test]
fn an_async_value_return_keeps_its_more_specific_refusal() {
    let err = compile_to_c(
        "async fn g() -> i64 { return 1; }\nfn main() { print_int(1); }\n",
        "m2_async_value_specific",
    )
    .expect_err("a value return inside an async fn is still refused");
    assert!(
        err.contains("a `return` with a value inside an `async fn`"),
        "got:\n{}",
        err
    );
}

/// A PRIVATE imported `async fn` does not reject the importing program.
///
/// It is never registered, so it can never be called and is never emitted.
/// Refusing it would diagnose a declaration the output cannot contain — the
/// polarity that killed a valid program at fbcfc39.
#[test]
fn a_private_imported_async_fn_does_not_reject_the_program() {
    let out = compile_and_run_with_import(
        "async fn hidden() { print(\"x\"); }\npub fn ok() { print_int(1); }\n",
        "import lib;\n\nfn main() { ok(); }\n",
        "m2_private_imported_async",
    )
    .expect("a private import is not part of the emitted program");
    assert_eq!(out.trim(), "1", "got: {:?}", out);
}

/// An imported `pub async fn` that a LOCAL definition SHADOWS does not reject
/// the importing program.
///
/// Code generation skips the shadowed body, so it is not in the output, and the
/// refusal asks `crate::ast::local_definition_shadows_import` — the same
/// predicate codegen asks — rather than a second copy of the question.
#[test]
fn a_shadowed_imported_async_fn_does_not_reject_the_program() {
    let out = compile_and_run_with_import(
        "pub async fn g() { print(\"x\"); }\n",
        "import lib;\n\nfn g() { print_int(9); }\nfn main() { g(); }\n",
        "m2_shadowed_imported_async",
    )
    .expect("the local `fn g` is what is emitted; the import is not");
    assert_eq!(out.trim(), "9", "got: {:?}", out);
}

/// An imported GENERIC `async fn` that is NEVER INSTANTIATED does not reject
/// the importing program.
///
/// The condition is instantiation, not genericity: an uninstantiated generic is
/// emitted by nobody. This is the ASYMMETRY with a local generic, which IS
/// refused at its declaration — a local declaration is the programmer's own
/// source and the construct cannot be honoured wherever it sits, while an
/// import may carry declarations this program never puts in its output.
#[test]
fn an_uninstantiated_imported_generic_async_fn_does_not_reject_the_program() {
    let out = compile_and_run_with_import(
        "pub async fn ag<T>(x: T) { print(\"x\"); }\npub fn ok() { print_int(1); }\n",
        "import lib;\n\nfn main() { ok(); }\n",
        "m2_uninstantiated_generic_async",
    )
    .expect("nothing instantiates `ag`, so it is not in the output");
    assert_eq!(out.trim(), "1", "got: {:?}", out);
}

/// `Future` written as an ordinary type is not the async rule's business.
///
/// It has its own failure — nothing defines the type — and that failure must
/// not be renamed by a rule about the keyword.
#[test]
fn a_written_future_type_is_not_diagnosed_as_an_async_fn() {
    let err = compile_to_c(
        "fn g() -> Future<i64> { panic(\"x\"); }\nfn main() { g(); }\n",
        "m2_written_future_type",
    )
    .err()
    .unwrap_or_default();
    assert!(
        !err.contains("`async fn` is not implemented"),
        "no `async fn` appears in this program:\n{}",
        err
    );
}

// ---------------------------------------------------------------------------
// THE SITE LIST, EXECUTED
//
// A hand-written map of "where Future machinery is emitted" inherits its
// author's blind spot; a sibling branch shipped a release blocker that way and
// closed it only because the coverage assertion derived its site list at test
// time. Both claims below are derived from src/codegen/mod.rs when they run.
// ---------------------------------------------------------------------------

fn codegen_source() -> String {
    fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/codegen/mod.rs"))
        .expect("src/codegen/mod.rs is the file these claims are about")
}

/// NOTHING IN CODE GENERATION WRITES FUTURE OR POLL MACHINERY INTO THE C.
///
/// Found by the shape of the emission — a `self.output` write mentioning
/// `_Future` or `_poll` — rather than by knowing which function used to do it.
/// `generate_async_function_with_name` had eleven such lines; a new one fails
/// here instead of shipping.
#[test]
fn code_generation_emits_no_future_or_poll_machinery() {
    let src = codegen_source();
    let offenders: Vec<String> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("self.output") || l.contains("push_str"))
        .filter(|(_, l)| l.contains("_Future") || l.contains("_poll"))
        .map(|(i, l)| format!("src/codegen/mod.rs:{}: {}", i + 1, l.trim()))
        .collect();
    assert!(
        offenders.is_empty(),
        "§N7 says effect tracking has no runtime representation, and these \
         lines write one into the emitted C:\n{}",
        offenders.join("\n")
    );
}

/// THE ONE REMAINING MENTION OF A `_Future` TYPE NAME IS THE DECLARED RESIDUAL.
///
/// `try_infer_expr_type` still types a call to a name in `async_functions` as
/// `<name>_Future`. That set is insert-only and never asks
/// `local_definition_shadows_import`, so a local `fn f` shadowing an imported
/// `pub async fn f` is still typed as a future — the emitted C carries
/// `f_Future v = f();` beside `long long f()`. It is NOT reachable from an
/// `async fn` this compiler will now accept, because there is none; it is
/// reachable only through the shadowing hole, which is M4's module-system debt
/// and is pinned by `tests/rust-debt-manifest.txt` /
/// `a_local_fn_shadowing_an_imported_async_fn_is_not_typed_as_a_future`.
///
/// Pinned at ONE occurrence so that closing that row, or growing a second
/// producer, both fail here and have to say which.
#[test]
fn the_only_future_type_name_left_is_the_declared_m4_residual() {
    let src = codegen_source();
    let producers: Vec<String> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("{}_Future"))
        .map(|(i, l)| format!("src/codegen/mod.rs:{}: {}", i + 1, l.trim()))
        .collect();
    assert_eq!(
        producers.len(),
        1,
        "exactly one `<name>_Future` producer is declared (the M4 shadowing \
         residual in `try_infer_expr_type`); found:\n{}",
        producers.join("\n")
    );
    assert!(
        producers[0].contains("format!"),
        "the declared residual is a TYPE NAME handed to `let` inference, not an \
         emission:\n{}",
        producers[0]
    );
}

/// EVERY WAY A FUNCTION BODY REACHES THE C GOES THROUGH ONE REFUSAL.
///
/// `generate_statement` is what turns a body into C. Its callers are derived
/// from the source here, and the claim is that the only one that can be entered
/// with a `Function` is `generate_function_with_name`, which refuses `is_async`
/// on its first lines. There used to be a second — the Future/poll emitter —
/// and that is exactly how an `async fn` body got into the output.
#[test]
fn one_refusal_covers_every_route_a_function_body_takes_into_the_c() {
    let src = codegen_source();

    let body_generators: Vec<&str> = src
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            t.strip_prefix("fn ")
                .or_else(|| t.strip_prefix("pub fn "))
                .and_then(|r| r.split(['(', '<']).next())
        })
        .filter(|n| n.starts_with("generate_") && n.contains("function"))
        .collect();
    assert_eq!(
        body_generators,
        vec![
            "generate_function",
            "generate_function_prototypes",
            "generate_function_with_name",
        ],
        "the set of `generate_*function*` entry points changed; a new one that \
         emits a body must refuse `is_async` too (this list is derived from the \
         source, not recalled). `generate_function` delegates to \
         `generate_function_with_name`, and `generate_function_prototypes` \
         emits signatures only — it already skips `is_async`"
    );

    // CODE, NOT COMMENTS. Measured: with the refusal deleted and the comment
    // above it left in place, the first version of this assertion still passed
    // — the comment says `CompileError::async_fn_unimplemented` and a substring
    // search cannot tell a mention from a call. That is this file's own
    // "the map is not the territory" defect, in the test that exists to prevent
    // it, so the window is filtered to non-comment lines.
    let at = src
        .find("fn generate_function_with_name")
        .expect("generate_function_with_name is where the refusal lives");
    let head: String = src[at..(at + 800).min(src.len())]
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        head.contains("if func.is_async")
            && head.contains("CompileError::async_fn_unimplemented"),
        "the single dispatch point for every function body must refuse an async \
         one before it emits anything:\n{}",
        head
    );

    // Comment lines excluded for the reason `monomorphisation_does_not_erase_
    // the_async_flag` gives: two comments NAME the deleted emitter in order to
    // say it is gone, and a check that cannot tell a name from a call would
    // make the record of the deletion unwritable.
    let mentions: Vec<String> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with("//") && !l.trim_start().starts_with("///"))
        .filter(|(_, l)| l.contains("generate_async_function_with_name"))
        .map(|(i, l)| format!("src/codegen/mod.rs:{}: {}", i + 1, l.trim()))
        .collect();
    assert!(
        mentions.is_empty(),
        "the Future/poll emitter is deleted, not merely unreachable: a private \
         method nothing calls is one edit away from being called again:\n{}",
        mentions.join("\n")
    );
}

/// THE GENERIC INGRESS CARRIES THE FLAG RATHER THAN ERASING IT.
///
/// `monomorphize_function` used to hardcode `is_async: false` under the comment
/// "monomorphized functions are not async" — true by erasure, which made every
/// downstream `is_async` guard on an instantiation dead code and let an
/// instantiated `async fn g<T>` emit an ordinary `g__i64`.
#[test]
fn monomorphisation_does_not_erase_the_async_flag() {
    let src = codegen_source();
    // Comment lines are excluded on purpose: the doc comment on
    // `monomorphize_function` QUOTES the erasing line it replaced, and a check
    // that cannot tell a quotation from an assignment would make that comment
    // unwritable.
    let erasures: Vec<String> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with("//"))
        .filter(|(_, l)| l.contains("is_async: false"))
        .map(|(i, l)| format!("src/codegen/mod.rs:{}: {}", i + 1, l.trim()))
        .collect();
    assert!(
        erasures.is_empty(),
        "code generation constructs a Function that DECLARES itself synchronous; \
         if that is a monomorphisation, it is erasing the property the N7-18 \
         refusal reads:\n{}",
        erasures.join("\n")
    );
    let at = src
        .find("fn monomorphize_function")
        .expect("monomorphize_function is the generic ingress");
    let body = &src[at..(at + 4000).min(src.len())];
    assert!(
        body.contains("is_async: generic_func.is_async"),
        "the instantiation must inherit `is_async` from the template:\n{}",
        &body[body.len().saturating_sub(600)..]
    );
}
