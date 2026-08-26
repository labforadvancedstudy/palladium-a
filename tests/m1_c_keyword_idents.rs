//! M1: an identifier that is a C keyword must not become invalid C.
//!
//! THE DEFECT THIS CLOSES, MEASURED ON `main` BEFORE THE FIX
//!
//! ```text
//! fn double(x: i64) -> i64 { return x * 2; }
//! fn main() { print_int(double(21)); }
//! ```
//!
//! emitted `long long double(long long x);` — `double` is a C type specifier,
//! so that declares nothing and gcc answered "'long long double' is invalid".
//! The compiler printed "✅ Compilation successful" first, and a
//! `pdc compile` with no `-o` never runs gcc at all, so the whole failure was
//! invisible from inside Palladium. The declared XFAIL was
//! `tests/e2e_test.rs::test_c_keyword_identifier_still_links`.
//!
//! EVERY identifier position was affected, not only function names. Measured on
//! `main`, one program:
//!
//! ```text
//! typedef struct register { long long signed; long long volatile; } register;
//! long long static = x;
//! __pd_print_int(double(short));
//! ```
//!
//! so the receipts below drive a keyword through each position and RUN the
//! result. A test that only greps the C would have passed on `main`: the text
//! was exactly what was asked for. Only gcc disagreed.
//!
//! WHY THE CONTROLS ARE ABOUT WHAT DID *NOT* CHANGE
//! The fix renames identifiers, so its failure mode is renaming one it should
//! not have — either changing the C emitted for a program that was already
//! fine, or mapping two source names onto one C name, which would replace a
//! loud gcc error with a silent duplicate definition. Both directions are
//! asserted here; the escape's injectivity is also proved exhaustively over the
//! reserved list in `src/codegen/c_ident.rs`'s own unit tests.

mod common;

use common::unique_module_name;
use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Compile with the real driver and return the emitted C.
fn compile_to_c(source: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    let c_file: PathBuf = Driver::new()
        .compile_file(&src)
        .map_err(|e| format!("compilation failed: {}", e))?;
    fs::read_to_string(&c_file).map_err(|e| e.to_string())
}

/// Compile, hand the C to gcc through the SAME invocation `pdc compile -o`
/// uses, run it, and return stdout.
///
/// gcc is the whole point: every other test in this suite greps the generated
/// text, and on `main` the text was exactly what was asked for. Only handing it
/// to a C compiler failed.
fn compile_and_run(source: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    let exe = dir.path().join(name);
    fs::write(&src, source).unwrap();

    let c_file = Driver::new()
        .compile_file(&src)
        .map_err(|e| format!("compilation failed: {}", e))?;

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

// ---------------------------------------------------------------------------
// The repro, and the positions around it
// ---------------------------------------------------------------------------

/// The exact program from the declared expectation, taken past linking to a
/// number: `double(21)` is 42, not a link failure and not garbage.
#[test]
fn a_function_named_after_a_c_keyword_links_and_runs() {
    let out = compile_and_run(
        r#"
fn double(x: i64) -> i64 {
    return x * 2;
}

fn main() {
    print_int(double(21));
}
"#,
        &unique_module_name("ckw_fn"),
    )
    .expect("a function named `double` must still produce valid C");
    assert_eq!(out.trim(), "42");
}

/// A C keyword in every position the emitted C spells out: struct tag, struct
/// field, function name, parameter name, local name, call site, field access
/// and a struct literal's field designator.
///
/// THE LOCAL USED TO BE NAMED `static`, and N3-10 took that spelling away: a C
/// keyword can only be tested in identifier position while it is NOT also a
/// Palladium keyword, and `static` now introduces a top-level item, so
/// `let static: i64 = x;` is a parse error before any C is emitted. `goto`
/// replaces it and is the better witness anyway — code generation emits real
/// `goto` labels for a guarded `match`, so a local of that name is a spelling
/// the output already contains for its own reasons.
#[test]
fn a_c_keyword_in_every_identifier_position_links_and_runs() {
    let out = compile_and_run(
        r#"
struct register {
    signed: i64,
    volatile: i64,
}

fn double(x: i64) -> i64 {
    let goto: i64 = x;
    return goto * 2;
}

fn extern(union: i64) -> i64 {
    return union + 1;
}

fn main() {
    let short: i64 = 3;
    let r = register { signed: 1, volatile: 2 };
    print_int(double(short));
    print_int(extern(r.signed));
    print_int(r.volatile);
}
"#,
        &unique_module_name("ckw_all"),
    )
    .expect("every identifier position must survive gcc");
    assert_eq!(out.trim(), "6\n2\n2");
}

/// Enums spell several DERIVED names — the tag constant, the payload struct,
/// the constructor, and the union member. Only unit variants here, which is
/// exactly why this test could not see the defect the next two are for.
#[test]
fn an_enum_named_after_a_c_keyword_links_and_runs() {
    let out = compile_and_run(
        r#"
enum switch {
    Case,
    Default,
}

fn auto(default: switch) -> i64 {
    match default {
        switch::Case => 1,
        switch::Default => 2,
    }
}

fn main() {
    print_int(auto(switch::Case));
    print_int(auto(switch::Default));
}
"#,
        &unique_module_name("ckw_enum"),
    )
    .expect("an enum named `switch` must still produce valid C");
    assert_eq!(out.trim(), "1\n2");
}

/// A DATA-BEARING variant, which is the shape the unit-only test above cannot
/// see. Measured before the fix, on the reviewer's own program:
///
/// ```text
/// enum E { Register(i64), Plain }
///   ->  E__Register_Data register;
///       gcc: error: expected identifier
/// ```
///
/// The union member was `variant.name.to_lowercase()`, computed AFTER the AST
/// escape, so the escape compared the source spelling `Register` against the
/// reserved list, found no match, and passed it through — and the derivation
/// then produced a reserved word. The thing protected was not the thing
/// emitted. Four write sites and two read sites spell this member; they must
/// all move together, which is why the assertion runs the program instead of
/// grepping the C.
#[test]
fn a_data_bearing_variant_that_folds_onto_a_keyword_links_and_runs() {
    let out = compile_and_run(
        r#"
enum E {
    Register(i64),
    Plain,
}

fn payload(e: E) -> i64 {
    match e {
        E::Register(v) => { return v; }
        E::Plain => { return 0; }
    }
}

fn main() {
    print_int(payload(E::Register(7)));
    print_int(payload(E::Plain));
}
"#,
        &unique_module_name("ckw_variant"),
    )
    .expect("`Register` folds to the reserved word `register`");
    assert_eq!(out.trim(), "7\n0", "the payload must survive the rename");
}

/// The other half of the same defect, and the worse half: case folding is not
/// INJECTIVE. `Register` and `register` as two variants of one enum both folded
/// to the member `register` — a duplicate union member, which is the
/// loud-to-silent trade the escape's injectivity exists to prevent.
///
/// The two payloads must stay two payloads, so this asserts the VALUES, not the
/// spelling: a collision that gcc happened to accept would return the wrong
/// field and a text assertion would not notice.
#[test]
fn two_variants_that_differ_only_in_case_stay_two_members() {
    let out = compile_and_run(
        r#"
enum E {
    Register(i64),
    register(i64),
}

fn label(e: E) -> i64 {
    match e {
        E::Register(v) => { return v; }
        E::register(v) => { return v + 100; }
    }
}

fn main() {
    print_int(label(E::Register(1)));
    print_int(label(E::register(1)));
}
"#,
        &unique_module_name("ckw_case"),
    )
    .expect("two variants differing only in case must not share a union member");
    assert_eq!(out.trim(), "1\n101", "each variant kept its own payload");
}

/// Monomorphisation templates do not come from the `Program` `compile` escapes.
/// They come from the type checker, which is handed the UNESCAPED AST
/// (`src/driver/mod.rs:109`), and `monomorphize_function` clones their names,
/// parameters and bodies straight into `generate_function`. Measured on `main`:
///
/// ```text
/// fn double<T>(register: T) -> T { return register; }
///   ->  long long double__i64(long long register) { return register; }
///       gcc: error: expected expression
/// ```
///
/// Both names here are reserved: the generic function itself (`double`) and its
/// parameter (`register`), so this covers the stored KEY as well as the body —
/// the key matters because `get_mangled_name_for_call` resolves a call by
/// comparing the (escaped) call name against these entries.
#[test]
fn a_keyword_named_generic_function_links_and_runs() {
    let out = compile_and_run(
        r#"
fn double<T>(register: T) -> T {
    return register;
}

fn main() {
    print_int(double(42));
}
"#,
        &unique_module_name("ckw_generic"),
    )
    .expect("a monomorphised body must be escaped like any other");
    assert_eq!(out.trim(), "42");
}

/// The generic STRUCT template travels the same unescaped route
/// (`set_generic_struct_instantiations` <- the type checker), and its fields are
/// escaped by the same fix.
///
/// A TEXT ASSERTION, DELIBERATELY, AND HERE IS WHY IT IS NOT A LINK-AND-RUN
/// LIKE EVERY OTHER TEST IN THIS FILE: a binding of a generic struct type does
/// not compile TODAY, for a reason that has nothing to do with identifiers.
/// `type_to_c` erases `Type::Generic { .. }` to `void*`, so
/// `let b: Box<i64> = Box { register: 7 };` emits
/// `void* b = (struct Box_i64){…}` and gcc refuses it — reproduced on `main`
/// with an ORDINARY field name, so it is a pre-existing generics defect and not
/// this one. Asserting the text is the strongest statement available until that
/// lands; when it does, this test should become a `compile_and_run`.
#[test]
fn a_keyword_named_generic_struct_field_is_escaped() {
    let c = compile_to_c(
        r#"
struct Box<T> {
    register: T,
}

fn main() {
    let b: Box<i64> = Box { register: 7 };
    print_int(b.register);
}
"#,
        &unique_module_name("ckw_gstruct"),
    )
    .expect("the program compiles to C; it is gcc that refuses the `void*`");

    assert!(
        c.contains("long long register_;"),
        "the monomorphised field must be escaped:\n{}",
        c
    );
    assert!(
        !c.contains("long long register;"),
        "the unescaped spelling is what gcc rejects:\n{}",
        c
    );
    assert!(
        c.contains(".register_ = 7"),
        "the struct literal's field designator must move with the declaration:\n{}",
        c
    );
}

/// A `for` binder, an assignment target, a mutable struct parameter and a
/// struct literal bound to a keyword-named local.
#[test]
fn a_c_keyword_as_a_binder_and_an_assignment_target_links_and_runs() {
    let out = compile_and_run(
        r#"
struct inline {
    goto: i64,
}

fn typedef(mut union: inline) -> i64 {
    union.goto = union.goto + 1;
    return union.goto;
}

fn main() {
    let mut register: i64 = 0;
    let float: [i64; 3] = [10, 20, 30];
    for double in float {
        register = register + double;
    }
    print_int(register);
    let mut sizeof = inline { goto: 41 };
    print_int(typedef(sizeof));
}
"#,
        &unique_module_name("ckw_binder"),
    )
    .expect("binders and assignment targets must survive gcc");
    assert_eq!(out.trim(), "60\n42");
}

/// THE PROPERTY THAT MAKES THE RENAME SAFE. Two source names may not become one
/// C name: that would turn gcc's loud "'long long double' is invalid" into a
/// silent second definition of one symbol. `double` and `double_` must stay
/// two functions with two answers.
#[test]
fn two_names_that_could_collide_stay_two_functions() {
    let out = compile_and_run(
        r#"
fn double(x: i64) -> i64 {
    return x * 2;
}

fn double_(x: i64) -> i64 {
    return x * 3;
}

fn main() {
    print_int(double(10));
    print_int(double_(10));
}
"#,
        &unique_module_name("ckw_collide"),
    )
    .expect("the escape must be injective");
    assert_eq!(out.trim(), "20\n30", "each function kept its own body");
}

/// THE GNU ALTERNATE SPELLINGS. `__asm__` is a keyword, and it is INSIDE the
/// declared debt ("generated identifiers are not mangled against C keywords"),
/// not beside it: the reserved list already reached for GNU by carrying `asm`,
/// `typeof`, `inline` and `restrict`, and stopped before their `__`-wrapped
/// alternates, which gcc rejects just as hard. Measured on the previous commit:
///
/// ```text
/// __asm__      gcc compilation failed
/// __inline__   gcc compilation failed
/// __const__    gcc compilation failed
/// __restrict__ gcc compilation failed
/// typeof       LINKS   (already covered)
/// ```
#[test]
fn the_gnu_alternate_keywords_link_and_run() {
    let out = compile_and_run(
        r#"
fn __asm__(x: i64) -> i64 { x + 1 }
fn __inline__(x: i64) -> i64 { x + 2 }
fn __const__(x: i64) -> i64 { x + 4 }
fn __restrict__(x: i64) -> i64 { x + 8 }
fn __volatile__(x: i64) -> i64 { x + 16 }
fn __signed__(x: i64) -> i64 { x + 32 }
fn __typeof__(x: i64) -> i64 { x + 64 }
fn __attribute__(x: i64) -> i64 { x + 128 }
fn __extension__(x: i64) -> i64 { x + 256 }
fn __label__(x: i64) -> i64 { x + 512 }
fn __alignof__(x: i64) -> i64 { x + 1024 }
fn __complex__(x: i64) -> i64 { x + 2048 }
fn __real__(x: i64) -> i64 { x + 4096 }
fn __imag__(x: i64) -> i64 { x + 8192 }
fn __thread(x: i64) -> i64 { x + 16384 }
fn __int128(x: i64) -> i64 { x + 32768 }
fn __auto_type(x: i64) -> i64 { x + 65536 }
fn __func__(x: i64) -> i64 { x + 131072 }
fn __has_include(x: i64) -> i64 { x + 262144 }

fn main() {
    let mut total: i64 = 0;
    total = total + __asm__(0);
    total = total + __inline__(0);
    total = total + __const__(0);
    total = total + __restrict__(0);
    total = total + __volatile__(0);
    total = total + __signed__(0);
    total = total + __typeof__(0);
    total = total + __attribute__(0);
    total = total + __extension__(0);
    total = total + __label__(0);
    total = total + __alignof__(0);
    total = total + __complex__(0);
    total = total + __real__(0);
    total = total + __imag__(0);
    total = total + __thread(0);
    total = total + __int128(0);
    total = total + __auto_type(0);
    total = total + __func__(0);
    total = total + __has_include(0);
    print_int(total);
}
"#,
        &unique_module_name("ckw_gnu"),
    )
    .expect("the GNU alternate keywords must still produce valid C");
    // 2^19 - 1: every one of the nineteen ran and returned its own bit, so no
    // two of them collapsed onto one C name either.
    assert_eq!(out.trim(), "524287");
}

/// THE DERIVATION, EXECUTED. `RESERVED` is a list, and a list is recall unless
/// something re-derives it. This asks the REAL `cc` — the one
/// `src/linker.rs` shells out to — whether it will accept each candidate as an
/// identifier, and requires every refusal to be in the list.
///
/// It fails if the list is SHORT, which is the direction that ships invalid C.
/// It does not fail if the list is long: three entries (`_Float32`, `_Accum`, …)
/// are keywords in GCC and identifiers in this checkout's clang, and the
/// asymmetry is deliberate — see `RESERVED`'s own comment for which entries are
/// measured and which are documentation.
///
/// The corpus is the GCC manual's *Alternate Keywords* and *C Extensions*
/// keyword lists plus the C standard keywords, which is the only recall left;
/// what it buys is that recall about the LIST is replaced by a measurement.
///
/// THE PROBE IS A LOCAL DECLARATION, NOT A FUNCTION DEFINITION, and the
/// difference was measured rather than guessed: over this whole corpus the two
/// shapes disagree about exactly ONE name, `main`, which a function-shaped
/// probe reports as unusable (redefining the entry point with the wrong
/// signature) while it is a perfectly ordinary identifier. `main` is not a
/// keyword, and code generation deliberately keeps it — so a probe that called
/// it one would have demanded the escape rename the entry point. A local
/// declaration asks the narrower question this list is actually about: can the
/// token be an identifier at all.
#[test]
fn the_reserved_list_covers_every_keyword_this_toolchain_has() {
    use palladium::codegen::c_ident::RESERVED;

    // Not `RESERVED` itself: a corpus taken from the thing under test proves
    // nothing. These are candidates, and the compiler adjudicates.
    let candidates: Vec<&str> = vec![
        // C standard
        "auto",
        "break",
        "case",
        "char",
        "const",
        "continue",
        "default",
        "do",
        "double",
        "else",
        "enum",
        "extern",
        "float",
        "for",
        "goto",
        "if",
        "inline",
        "int",
        "long",
        "register",
        "restrict",
        "return",
        "short",
        "signed",
        "sizeof",
        "static",
        "struct",
        "switch",
        "typedef",
        "union",
        "unsigned",
        "void",
        "volatile",
        "while",
        "_Alignas",
        "_Alignof",
        "_Atomic",
        "_BitInt",
        "_Bool",
        "_Complex",
        "_Generic",
        "_Imaginary",
        "_Noreturn",
        "_Static_assert",
        "_Thread_local",
        "_Decimal32",
        "_Decimal64",
        "_Decimal128",
        // C23
        "alignas",
        "alignof",
        "bool",
        "constexpr",
        "false",
        "nullptr",
        "static_assert",
        "thread_local",
        "true",
        "typeof",
        "typeof_unqual",
        // GNU alternate keywords and extensions
        "asm",
        "__asm",
        "__asm__",
        "__attribute",
        "__attribute__",
        "__const",
        "__const__",
        "__volatile",
        "__volatile__",
        "__signed",
        "__signed__",
        "__inline",
        "__inline__",
        "__restrict",
        "__restrict__",
        "__typeof",
        "__typeof__",
        "__alignof",
        "__alignof__",
        "__complex",
        "__complex__",
        "__real",
        "__real__",
        "__imag",
        "__imag__",
        "__label__",
        "__extension__",
        "__thread",
        "__int128",
        "__auto_type",
        "__func__",
        "__FUNCTION__",
        "__PRETTY_FUNCTION__",
        "__fp16",
        "__float128",
        "__bf16",
        "__ibm128",
        "__has_include",
        // clang nullability and calling conventions
        "_Nonnull",
        "_Nullable",
        "_Null_unspecified",
        "__nullable",
        "__cdecl",
        "__stdcall",
        "__fastcall",
        "__thiscall",
        "__vectorcall",
        // The other direction: NOT keywords, and the list must not grow to
        // cover them. `__label` next to `__label__` is the whole point.
        "__label",
        "__extension",
        "__func",
        "__auto",
        "__int",
        "__foo__",
        "__bar",
        "_leading",
        "doubled",
        "my_double",
        "fibonacci",
        "main",
        "print_int",
        "__pd_print",
    ];

    let dir = TempDir::new().unwrap();
    let mut missing: Vec<&str> = Vec::new();
    let mut renamed_without_cause: Vec<&str> = Vec::new();

    for name in &candidates {
        let src = dir.path().join("probe.c");
        fs::write(
            &src,
            format!(
                "int f(void) {{ long long {n} = 0; return (int){n}; }}\n",
                n = name
            ),
        )
        .unwrap();
        let out = Command::new("cc")
            .args(["-std=gnu17", "-c"])
            .arg(&src)
            .args(["-o", "/dev/null"])
            .output()
            .expect("cc must be available: it is what src/linker.rs shells out to");
        let is_keyword = !out.status.success();
        let is_listed = RESERVED.binary_search(name).is_ok();

        if is_keyword && !is_listed {
            missing.push(name);
        }
        // A name this toolchain accepts AND that the list does not carry must
        // survive untouched. (A listed-but-accepted name is the documented
        // GCC-portability group and is expected.)
        if !is_keyword && !is_listed && palladium::codegen::c_ident::c_ident(name) != **name {
            renamed_without_cause.push(name);
        }
    }

    assert!(
        missing.is_empty(),
        "this toolchain refuses these as identifiers and RESERVED does not \
         carry them, so the emitted C for a program using one is invalid: {:?}",
        missing
    );
    assert!(
        renamed_without_cause.is_empty(),
        "these are ordinary identifiers here and are being renamed anyway: {:?}",
        renamed_without_cause
    );
}

/// Every escape must land somewhere the compiler actually accepts. `__asm__`
/// becoming `__asm___` is only a fix if `__asm___` is a legal identifier — and
/// the list now contains entries whose escape is another `__`-prefixed name, so
/// that is not self-evident.
#[test]
fn every_escaped_spelling_is_accepted_by_the_toolchain() {
    use palladium::codegen::c_ident::{c_ident, RESERVED};

    let dir = TempDir::new().unwrap();
    let mut rejected: Vec<String> = Vec::new();
    for word in RESERVED {
        let escaped = c_ident(word).into_owned();
        let src = dir.path().join("probe.c");
        fs::write(
            &src,
            format!(
                "int f(void) {{ long long {n} = 0; return (int){n}; }}\n",
                n = escaped
            ),
        )
        .unwrap();
        let out = Command::new("cc")
            .args(["-std=gnu17", "-c"])
            .arg(&src)
            .args(["-o", "/dev/null"])
            .output()
            .expect("cc must be available");
        if !out.status.success() {
            rejected.push(format!("{} -> {}", word, escaped));
        }
    }
    assert!(
        rejected.is_empty(),
        "the escape produced spellings the compiler still refuses: {:?}",
        rejected
    );
}

// ---------------------------------------------------------------------------
// Controls: what the rename must NOT touch
// ---------------------------------------------------------------------------

/// An ordinary program's C is spelled exactly as before. This is the assertion
/// that goes red if the escape ever widens to "rename everything".
#[test]
fn an_ordinary_program_is_spelled_unchanged() {
    let c = compile_to_c(
        r#"
fn fibonacci(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    return fibonacci(n - 1) + fibonacci(n - 2);
}

fn main() {
    print_int(fibonacci(10));
}
"#,
        &unique_module_name("ckw_plain"),
    )
    .expect("must compile");

    assert!(
        c.contains("long long fibonacci(long long n)"),
        "an ordinary name must reach the C unchanged:\n{}",
        c
    );
    assert!(
        !c.contains("fibonacci_"),
        "nothing here is a reserved word, so nothing may be escaped:\n{}",
        c
    );
    // The runtime's own symbols travel through the same pass.
    assert!(
        c.contains("__pd_print_int"),
        "the runtime prefix must be untouched:\n{}",
        c
    );
}

/// Names that merely CONTAIN a reserved word, or end in the escape character
/// without being an escape of anything, are not reserved words.
#[test]
fn a_name_that_only_resembles_a_keyword_is_left_alone() {
    let c = compile_to_c(
        r#"
fn doubled(x: i64) -> i64 { x * 2 }
fn my_double(x: i64) -> i64 { x * 4 }
fn value_(x: i64) -> i64 { x * 8 }
fn __foo__(x: i64) -> i64 { x * 16 }
fn __bar(x: i64) -> i64 { x * 32 }
fn __label(x: i64) -> i64 { x * 64 }
fn __extension(x: i64) -> i64 { x * 128 }

fn main() {
    print_int(doubled(1));
    print_int(my_double(1));
    print_int(value_(1));
    print_int(__foo__(1));
    print_int(__bar(1));
    print_int(__label(1));
    print_int(__extension(1));
}
"#,
        &unique_module_name("ckw_near"),
    )
    .expect("must compile");

    // `__label` next to `__label__`, and `__extension` next to `__extension__`:
    // the widened list carries the keyword and must leave the ordinary name
    // alone. Both measured with the real `cc`.
    for name in [
        "doubled",
        "my_double",
        "value_",
        "__foo__",
        "__bar",
        "__label",
        "__extension",
    ] {
        assert!(
            c.contains(&format!("long long {}(long long x)", name)),
            "`{}` is not a reserved word and must be spelled as written:\n{}",
            name,
            c
        );
    }
}

/// THE DECLARED RESIDUAL, recorded with its measurement rather than asserted
/// away in a comment.
///
/// The escape covers C KEYWORDS. It does not cover library identifiers
/// (`strlen`, `printf`, `malloc`), and it should not: that list would have to
/// track every header the emitted prelude includes, and the failure is not the
/// one M1 exists to remove — gcc reports a CONFLICTING DECLARATION, loudly, and
/// nothing is silently miscompiled. This test pins that it is loud. If a future
/// change makes such a program link, this goes red and whoever made it link
/// owes an answer about which definition won.
#[test]
fn a_library_name_is_still_rejected_and_rejected_loudly() {
    let err = compile_and_run(
        r#"
fn strlen(x: i64) -> i64 {
    return x + 1;
}

fn main() {
    print_int(strlen(41));
}
"#,
        &unique_module_name("ckw_libname"),
    )
    .expect_err("`strlen` collides with the C library, which the escape does not cover");

    assert!(
        err.contains("gcc rejected the C"),
        "the failure must come from the C compiler, not from a silent wrong answer:\n{}",
        err
    );
    assert!(
        err.contains("strlen"),
        "and it must name the symbol that collided:\n{}",
        err
    );
}

// ---------------------------------------------------------------------------
// The two enumerations, EXECUTED rather than written down
//
// A hand-written navigation map omitting one site is how a release blocker got
// through on a sibling branch, and it is the same shape as this file's own
// defect: the map is not the territory, the comment is not the code. Both
// claims below are therefore derived from `src/codegen/mod.rs` at test time.
// ---------------------------------------------------------------------------

fn codegen_source() -> String {
    fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/codegen/mod.rs"))
        .expect("src/codegen/mod.rs is the file these claims are about")
}

/// THERE IS NO SEVENTH PAYLOAD-MEMBER SITE.
///
/// Every line of code generation that writes the enum payload union member into
/// the C is found here by the shape of what it emits — the member's declaration
/// inside `union { … } data`, and the four `…data.{}…` accesses — and each is
/// required to compute that name with `c_enum_payload_member`. A new site that
/// spells the member some other way fails this test rather than shipping.
#[test]
fn every_payload_member_emission_uses_the_one_derivation() {
    let src = codegen_source();
    let lines: Vec<&str> = src.lines().collect();

    // The two emission shapes, by the C they produce:
    //   `{}__{}_Data {};`   the member's declaration
    //   `…data.{}…`         the constructor writes and the match reads
    let emitters: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("_Data {};") || l.contains("data.{}"))
        .map(|(i, _)| i)
        .collect();

    assert!(
        emitters.len() >= 6,
        "expected at least the six known payload-member emissions, found {} — \
         if a site was deleted, delete its entry here too",
        emitters.len()
    );

    for i in &emitters {
        let window = lines[*i..(*i + 8).min(lines.len())].join("\n");
        assert!(
            window.contains("c_enum_payload_member"),
            "src/codegen/mod.rs:{} emits the payload union member without going \
             through `c_ident::c_enum_payload_member`, so it can derive a \
             reserved or colliding name:\n{}",
            i + 1,
            window
        );
    }
}

/// AND NOTHING CASE-FOLDS A NAME ANY MORE.
///
/// `to_lowercase()` on a variant name IS the defect — not reserved-safe and not
/// injective. This is the cheapest possible statement that it has not come
/// back, and unlike the test above it needs no knowledge of the emission shapes.
#[test]
fn code_generation_never_case_folds_an_identifier() {
    let src = codegen_source();
    for (i, line) in src.lines().enumerate() {
        for fold in [
            "to_lowercase",
            "to_uppercase",
            "to_ascii_lowercase",
            "to_ascii_uppercase",
        ] {
            assert!(
                !line.contains(fold),
                "src/codegen/mod.rs:{} case-folds an identifier (`{}`). That is \
                 how `Register` became the reserved member `register`, and it \
                 is not injective either — `Register` and `register` would \
                 collide:\n{}",
                i + 1,
                fold,
                line
            );
        }
    }
}

/// THERE IS NO FIFTH INGRESS.
///
/// Code generation can only be handed a Palladium AST or a type-checker
/// template through these four methods; each escapes what it is given. The set
/// is derived from the source, so adding a fifth without escaping it fails
/// here.
#[test]
fn every_codegen_ingress_escapes_what_it_is_given() {
    let src = codegen_source();
    let ingresses: Vec<String> = src
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            t.strip_prefix("pub fn ")
                .map(|r| r.split(['(', '<']).next().unwrap_or("").to_string())
        })
        .filter(|n| n == "compile" || n.starts_with("set_"))
        .collect();

    assert_eq!(
        ingresses,
        vec![
            "set_imported_modules",
            "set_generic_instantiations",
            "set_generic_struct_instantiations",
            "compile",
        ],
        "the set of ways an AST or a template enters code generation changed. \
         Every one of them must escape reserved words before anything is \
         emitted — see src/codegen/c_ident.rs's header, which lists them."
    );

    // Each of the four names an escape entry point in its body. Checked by
    // reading the source rather than by trusting the list above.
    for (name, escape) in [
        ("pub fn compile", "escape_reserved_names"),
        ("pub fn set_imported_modules", "escape_reserved_names"),
        (
            "pub fn set_generic_instantiations",
            "escape_generic_function",
        ),
        (
            "pub fn set_generic_struct_instantiations",
            "escape_generic_struct",
        ),
    ] {
        let at = src
            .find(name)
            .unwrap_or_else(|| panic!("no `{}` in src/codegen/mod.rs", name));
        let body = &src[at..(at + 1200).min(src.len())];
        assert!(
            body.contains(escape),
            "`{}` does not call `{}`, so what it receives reaches the C \
             unescaped",
            name,
            escape
        );
    }
}
