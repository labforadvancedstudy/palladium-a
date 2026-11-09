//! EVERY BODY THE PROGRAM HAS IS A BODY THE MACRO EXPANDER WALKS (issue #85).
//!
//! THE DEFECT, MEASURED ON d5166e8 BEFORE THE FIX
//!
//! ```text
//! macro two!() { 2 }
//! struct S { n: i64 }
//! impl S { fn get(&self) -> i64 { return two!() + self.n; } }
//! fn main() { let s = S { n: 40 }; print_int(s.get()); }
//! -> error: Unexpected macro invocation in type checking - macros should be
//!    expanded before this phase
//! ```
//!
//! The same body in a free function printed 42. `MacroExpander::expand_item`
//! (`src/macros/mod.rs`) had one arm, `Item::Function`, and a `_ => {}` with a
//! TODO for the rest, so an invocation in a method — inherent, static or trait
//! impl — stayed in the tree and reached the type checker's internal assertion.
//! A trait's default method body was skipped the same way. And an imported
//! module was never expanded at all: the resolver (`src/resolver/mod.rs`,
//! `load_module`) lexed and parsed each file and handed the AST on, so an
//! invocation in ANY of its bodies — even a free function — met the same
//! assertion.
//!
//! What a refusal says in a free function it now says in a method, word for
//! word: the rows below compare the two spellings rather than pinning a
//! sentence each, because "the same diagnostic" is the claim.

mod common;

use common::unique_module_name;
use palladium::ast::{Expr, Item, Stmt};
use palladium::lexer::Lexer;
use palladium::linker::{link_command, OptLevel};
use palladium::macros::MacroExpander;
use palladium::parser::Parser;
use palladium::Driver;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// `RUN: <stdout>` for a program that compiles, links and runs; `REFUSED:
/// <first line>` for one this compiler refused; `GCC` for C it emitted and gcc
/// refused (never acceptable, and never expected below).
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
                "GCC".to_string()
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
        return "GCC".to_string();
    }
    let run = Command::new(&exe).output().expect("run");
    format!("RUN: {}", String::from_utf8_lossy(&run.stdout))
}

/// The same verdict for a program of several files: `main.pd` and the modules
/// it imports, compiled by `pdc` in a scratch directory, because the module
/// resolver searches `.` (`tests/m3_imported_calls.rs` says why).
fn outcome_of_files(modules: &[(&str, &str)], main: &str) -> String {
    let dir = TempDir::new().unwrap();
    for (name, src) in modules {
        fs::write(dir.path().join(name), src).unwrap();
    }
    fs::write(dir.path().join("main.pd"), main).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .args(["compile", "main.pd", "-o", "prog"])
        .current_dir(dir.path())
        .output()
        .expect("pdc");
    let text = strip_ansi(&format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    ));
    if !out.status.success() {
        return if text.contains("gcc") {
            "GCC".to_string()
        } else {
            let first = text.lines().find(|l| l.contains("error")).unwrap_or("");
            format!("REFUSED: {}", first)
        };
    }
    let run = Command::new(dir.path().join("build_output").join("prog"))
        .current_dir(dir.path())
        .output()
        .expect("run");
    format!("RUN: {}", String::from_utf8_lossy(&run.stdout))
}

/// `pdc` colours its diagnostics whether or not stderr is a terminal; the
/// verdict compares the words.
fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for d in chars.by_ref() {
                if d == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A MACRO IN A METHOD EXPANDS AS IT DOES IN A FREE FUNCTION — the issue's
/// program, then every kind of method and every position in one.
///
/// Each row is a program main refused with "Unexpected macro invocation in
/// type checking", and each prints the number the same body prints as a free
/// function. The struct-literal row is round 11's (`tests/m2_enum_composites.rs`,
/// `a_struct_literal_is_recorded_as_itself_not_as_its_span`) moved into a
/// method: each expansion is parsed anew, so each literal it builds has its own
/// `LiteralId`, and an unused `pack!(a)` does not decide what `pack!(w)` is.
#[test]
fn a_macro_in_a_method_body_expands_as_it_does_in_a_free_function() {
    let s = "struct S { n: i64 }\n";
    let rows: Vec<(&str, String, &str)> = vec![
        (
            "inherent method, value position (the issue)",
            format!(
                "macro two!() {{ 2 }}\n{s}impl S {{ fn get(&self) -> i64 {{ return two!() + self.n; }} }}\n\
                 fn main() {{ let s = S {{ n: 40 }}; print_int(s.get()); }}"
            ),
            "RUN: 42\n",
        ),
        (
            "static method, a struct literal around a macro",
            format!(
                "macro two!() {{ 2 }}\n{s}impl S {{ fn make() -> S {{ return S {{ n: two!() + 40 }}; }} }}\n\
                 fn main() {{ let s = S::make(); print_int(s.n); }}"
            ),
            "RUN: 42\n",
        ),
        (
            "trait impl method",
            format!(
                "macro two!() {{ 2 }}\ntrait Get {{ fn get(s: S) -> i64; }}\n{s}\
                 impl Get for S {{ fn get(&self) -> i64 {{ return two!() + self.n; }} }}\n\
                 fn main() {{ let s = S {{ n: 40 }}; print_int(s.get()); }}"
            ),
            "RUN: 42\n",
        ),
        (
            "statement position, a statement body with a parameter",
            format!(
                "macro say!(v) {{ print_int($v); print_int($v + 1); }}\n{s}\
                 impl S {{ fn show(&self) {{ let n = self.n; say!(n); }} }}\n\
                 fn main() {{ let s = S {{ n: 41 }}; s.show(); }}"
            ),
            "RUN: 41\n42\n",
        ),
        (
            "nested: if, while, loop break, match arm, value block",
            format!(
                "macro two!() {{ 2 }}\nenum P {{ A, B }}\n{s}\
                 impl S {{ fn walk(&self) -> i64 {{\n\
                     let mut t = 0;\n\
                     if self.n > two!() {{ t = t + two!(); }} else {{ t = t + 100; }}\n\
                     let mut i = 0;\n\
                     while i < two!() {{ i = i + 1; t = t + 1; }}\n\
                     let e = loop {{ break two!(); }};\n\
                     let p = P::B;\n\
                     match p {{ P::A => {{ t = t + 100; }} P::B => {{ t = t + two!(); }} }}\n\
                     let k = match p {{ P::A => 100, P::B => two!() }};\n\
                     let b = {{ two!() }};\n\
                     let v = if self.n > 0 {{ two!() }} else {{ 100 }};\n\
                     return t + e + k + b + v + self.n;\n\
                 }} }}\n\
                 fn main() {{ let s = S {{ n: 30 }}; print_int(s.walk()); }}"
            ),
            // t = 2 + 2 (the loop) + 2 = 6; e = k = b = v = 2; 6 + 8 + 30 = 44.
            "RUN: 44\n",
        ),
        (
            // `b` was a `u32`: since #86/#87 review round 2 a literal of a `u32` the
            // record spells `i64` is refused by name (PD0082, tests/m4_generic_call_sites.rs),
            // and this row is about the expansions, so `b` is the `i64` the record holds.
            "struct literal in the macro body, two expansions in one method",
            "type z = u32;\nstruct S { n: i64 }\nimpl S { fn get(&self) -> z { return 7; } }\n\
             struct G<X, Y> { tag: X, p: Y }\nmacro pack!(x) { G { tag: 1, p: $x } }\n\
             struct H { k: i64 }\n\
             impl H { fn run(&self) -> i64 { let s = S { n: 0 }; let a = s.get(); let d = pack!(a); \
             let b: i64 = 7; let w: i64 = 4294967297; let c = false; \
             let r = if c { pack!(b) } else { pack!(w) }; return r.p; } }\n\
             fn main() { let h = H { k: 0 }; print_int(h.run()); }"
                .to_string(),
            "RUN: 4294967297\n",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program, out) in &rows {
        got.push(format!("{}: {}", shape, outcome(program, "macro_reach")));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}

/// EVERY EXPRESSION POSITION IS WALKED, NOT ONLY EVERY BODY (#85, extended).
///
/// The same symptom one level down: `expand_expr` and `expand_stmt` each ended
/// in a `_ => {}`, so an invocation inside a tuple, under a tuple index, in a
/// match arm's guard (statement and value `match`), or anywhere in an
/// assignment's TARGET (`a[two!()] = …`, `*id!(p) = …`, `id!(s).n = …`) was
/// left in the tree and met the type checker's internal assertion, in a free
/// function as in a method. Each row was that assertion on d5166e8 and prints
/// 42 now. The walk names every variant, so a new expression or statement kind
/// fails to compile in the expander until somebody says what it holds. The
/// tuple-index row is annotated because `let a = (2, 40).0;` with no macro at
/// all is refused by code generation ("no type rule for this tuple element
/// access expression"), on main as here; that is not the expander's.
#[test]
fn a_macro_in_every_expression_position_expands() {
    let two = "macro two!() { 2 }\n";
    let id = "macro id!(x) { $x }\n";
    let rows: Vec<(&str, String)> = vec![
        ("a tuple element", format!("{two}fn main() {{ let t = (two!(), 40); print_int(t.0 + t.1); }}")),
        ("a tuple nested in a tuple", format!("{two}fn main() {{ let t = ((two!(), 10), 30); print_int((t.0).0 + (t.0).1 + t.1); }}")),
        ("the tuple a tuple index reads", "macro pair!() { (2, 40) }\nfn main() { let a: i64 = pair!().0; let b: i64 = pair!().1; print_int(a + b); }".to_string()),
        ("a guard, statement match", format!("{two}fn main() {{ let n = 2; match n {{ k if k == two!() => {{ print_int(42); }} _ => {{ print_int(0); }} }} }}")),
        ("a guard, value match", format!("{two}fn main() {{ let n = 3; let r = match n {{ k if k > two!() => 42, _ => 0 }}; print_int(r); }}")),
        ("an assignment target's index", format!("{two}fn main() {{ let mut a = [0, 0, 0]; a[two!()] = 42; print_int(a[2]); }}")),
        ("a compound assignment target's index", format!("{two}fn main() {{ let mut a = [0, 0, 40]; a[two!()] += 2; print_int(a[2]); }}")),
        ("an assignment target's array", format!("{id}fn main() {{ let mut a = [0, 0]; id!(a)[1] = 42; print_int(a[1]); }}")),
        ("an index under a field, as a target", format!("{two}struct S {{ v: [i64; 3] }}\nfn main() {{ let mut s = S {{ v: [0, 0, 0] }}; s.v[two!()] = 42; print_int(s.v[2]); }}")),
        ("an assignment target's object", format!("{id}struct S {{ n: i64 }}\nfn main() {{ let mut s = S {{ n: 0 }}; id!(s).n = 42; print_int(s.n); }}")),
        ("an assignment target's dereference", format!("{id}fn main() {{ let mut v = 1; let p = &mut v; *id!(p) = 42; print_int(v); }}")),
        ("a tuple and a guard in a method", format!("{two}struct S {{ n: i64 }}\nimpl S {{ fn f(&self) -> i64 {{ let t = (self.n, two!()); match t.1 {{ k if k == two!() => {{ return t.0 + k; }} _ => {{ return 0; }} }} }} }}\nfn main() {{ let s = S {{ n: 40 }}; print_int(s.f()); }}")),
        ("a tuple a method returns", format!("{two}struct S {{ n: i64 }}\nimpl S {{ fn both(&self) -> (i64, i64) {{ return (two!(), self.n); }} }}\nfn main() {{ let s = S {{ n: 40 }}; let t = s.both(); print_int(t.0 + t.1); }}")),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, program) in &rows {
        got.push(format!("{}: {}", shape, outcome(program, "macro_position")));
        want.push(format!("{}: RUN: 42\n", shape));
    }
    assert_eq!(got, want);
}

/// `.await` OF A MACRO: THE OPERAND IS EXPANDED, AND `.await` IS STILL REFUSED.
///
/// The type checker refuses `.await` without reading its operand (D5), so no
/// program prints the difference and the AST is the receipt, as for a trait's
/// default body. Main left `two!()` under the `Await`.
#[test]
fn the_operand_of_await_is_expanded_and_await_is_still_refused() {
    let source = "macro two!() { 2 }\nfn main() { let x = two!().await; print_int(x); }\n";
    let tokens = Lexer::new(source).collect_tokens().expect("lexes");
    let mut program = Parser::new(tokens).parse().expect("parses");
    MacroExpander::new()
        .expand_program(&mut program)
        .expect("expands");
    let body = match &program.items[0] {
        Item::Function(f) => f.body.clone(),
        other => panic!("not a function: {:?}", other),
    };
    let operand = match body.first() {
        Some(Stmt::Let {
            value: Expr::Await { expr, .. },
            ..
        }) => expr.clone(),
        other => panic!("not `let x = <e>.await;`: {:?}", other),
    };
    assert!(
        matches!(*operand, Expr::Integer(2)),
        "the operand of `.await` still holds {:?}",
        operand
    );
    assert_eq!(
        outcome(source, "macro_await"),
        "REFUSED: `.await` is not implemented"
    );
}

/// A FAILURE THAT COMES OUT OF AN EXPANSION IS REFUSED BY NAME (#85 review
/// round 1, PD0079).
///
/// #85 expands bodies main never read: a trait's default body, a module's
/// private function, a module's uninstantiated generic. Main RAN every program
/// below (the bad invocation sat in a body nothing looked at) and printed 1. The
/// first tip of #85 refused them with the parser's bare words — "Unexpected end
/// of file", "Expected ';' …, but found '{'" — naming no macro, with a caret at
/// an offset into the expansion. A run may become a refusal only by name, so
/// each must now say `the expansion of `X!` does not parse as …`, carry PD0079,
/// keep the parser's reason, and (in a module) name the module's file. The
/// free-function column is the same refusal main always gave, now named: the
/// reason main printed is still in it, word for word.
#[test]
fn an_expansion_that_does_not_parse_is_refused_by_name_in_every_body() {
    let cases = [
        ("println!", "", "println!(1);", "statements", "Unexpected end of file"),
        ("dbg!", "", "dbg!(1);", "statements", "Unexpected end of file"),
        ("assert!", "", "assert!(1);", "statements", "Expected ';' (Expected ';' after expression), but found '{'"),
        ("a!", "macro a!() { + }\n", "let q = a!();", "an expression", "Expected expression, but found '+'"),
        ("set!", "macro set!(x) { $x = 42; }\n", "let mut y = 0; set!(y);", "statements", "Expected ';' (Expected ';' after expression), but found '='"),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (name, defs, call, position, reason) in cases {
        let bodies: [(&str, Vec<(&str, String)>, String); 4] = [
            (
                "trait default body",
                vec![],
                format!("{defs}struct S {{ n: i64 }}\ntrait T {{ fn f(s: S) {{ {call} }} }}\nfn main() {{ print_int(1); }}\n"),
            ),
            (
                "unused private fn of an imported module",
                vec![("lib1.pd", format!("{defs}fn hidden() {{ {call} }}\npub fn used() -> i64 {{ return 1; }}\n"))],
                "import lib1;\nfn main() { print_int(used()); }\n".to_string(),
            ),
            (
                "uninstantiated generic of an imported module",
                vec![("lib1.pd", format!("{defs}pub fn hidden<T>(x: T) -> T {{ {call} return x; }}\npub fn used() -> i64 {{ return 1; }}\n"))],
                "import lib1;\nfn main() { print_int(used()); }\n".to_string(),
            ),
            (
                "free function (refused on main too, unnamed)",
                vec![],
                format!("{defs}fn f() {{ {call} }}\nfn main() {{ print_int(1); }}\n"),
            ),
        ];
        for (body, modules, main) in bodies {
            let files: Vec<(&str, &str)> = modules.iter().map(|(n, s)| (*n, s.as_str())).collect();
            let verdict = outcome_of_files(&files, &main);
            let named = format!("the expansion of `{name}` does not parse as {position}, where it is invoked: {reason}");
            let ok = verdict.starts_with("REFUSED: error[PD0079]: ")
                && verdict.contains(&named)
                // The module's file and the invocation's line in it: the call is
                // on line 1, or line 2 under a macro definition.
                && (files.is_empty()
                    || verdict.contains(&format!("lib1.pd:{}:", if defs.is_empty() { 1 } else { 2 })));
            got.push(format!("{name} in a {body}: {}", if ok { "named PD0079".to_string() } else { verdict }));
            want.push(format!("{name} in a {body}: named PD0079"));
        }
    }
    assert_eq!(got, want);
}

/// A REFUSAL A MACRO GETS IN A FREE FUNCTION IT GETS IN A METHOD, WORD FOR WORD.
///
/// Main answered every one of these in a method with the type checker's
/// internal assertion, which names a compiler phase and not the rule. Each row
/// compiles the same invocation twice — once in `fn f`, once in a method — and
/// requires both refusals to be the same line, and that line to name the rule.
/// The two definition-side refusals (a body that invokes a macro, a body that
/// writes `let`) were already location-independent; they are here so that a
/// future change moving them to expansion time cannot split them.
#[test]
fn a_macro_refusal_in_a_method_is_the_free_function_refusal() {
    let rows = [
        (
            "an argument that invokes a macro",
            "macro double!(x) { $x * 2 }\n",
            "double!(double!(1))",
            "as an argument, and expansion is a single pass",
        ),
        (
            "a macro nothing defines",
            "",
            "nope!()",
            "Unknown macro 'nope'",
        ),
        (
            "arguments that do not match the parameters",
            "macro double!(x) { $x * 2 }\n",
            "double!(1, 2)",
            "arguments don't match pattern",
        ),
        (
            "a body that invokes a macro",
            "macro double!(x) { $x * 2 }\nmacro quad!(x) { double!(double!($x)) }\n",
            "quad!(1)",
            "in its body, and expansion is a single pass",
        ),
        (
            "a body that writes `let`",
            "macro bind!() { let q = 1; }\n",
            "bind!()",
            "the macro token stream can carry identifiers",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, defs, call, fragment) in rows {
        let free = format!(
            "{defs}fn f() -> i64 {{ return {call}; }}\nfn main() {{ print_int(f()); }}"
        );
        let method = format!(
            "{defs}struct S {{ n: i64 }}\nimpl S {{ fn f(&self) -> i64 {{ return {call}; }} }}\n\
             fn main() {{ let s = S {{ n: 0 }}; print_int(s.f()); }}"
        );
        let in_free = outcome(&free, "macro_refusal_free");
        let in_method = outcome(&method, "macro_refusal_method");
        got.push(format!(
            "{}: names the rule {}, method == free fn {}",
            shape,
            in_free.starts_with("REFUSED:") && in_free.contains(fragment),
            in_method == in_free
        ));
        want.push(format!(
            "{}: names the rule true, method == free fn true",
            shape
        ));
        if in_method != in_free {
            got.push(format!("  free fn: {}\n  method:  {}", in_free, in_method));
        }
    }
    assert_eq!(got, want);
}

/// A TRAIT'S DEFAULT METHOD BODY IS EXPANDED.
///
/// No pass consumes a default body yet (the type checker registers its
/// signature only, `src/typeck/mod.rs`, "Type check trait methods with bodies"),
/// so no program can print the difference; the AST is the receipt. Main left
/// `two!()` in the tree, where the first pass to read default bodies would have
/// met it as the internal assertion this issue is about.
#[test]
fn a_trait_default_method_body_is_expanded() {
    let source = "macro two!() { 2 }\nstruct S { n: i64 }\n\
                  trait Get { fn get(s: S) -> i64 { return two!() + 40; } }\n\
                  fn main() { print_int(1); }\n";
    let tokens = Lexer::new(source).collect_tokens().expect("lexes");
    let mut program = Parser::new(tokens).parse().expect("parses");
    MacroExpander::new()
        .expand_program(&mut program)
        .expect("expands");
    let body = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Trait(t) => t.methods[0].body.clone(),
            _ => None,
        })
        .expect("the trait's default body");
    let left = match body.as_slice() {
        [Stmt::Return(Some(Expr::Binary { left, .. }))] => left,
        other => panic!("not `return <binary>;`: {:?}", other),
    };
    assert!(
        matches!(**left, Expr::Integer(2)),
        "the default body still holds {:?}",
        left
    );
    // The one program this makes refuse that main ran: an invocation of a
    // macro nothing defines, in a default body nothing reads. Main printed 1,
    // because no pass looked; it is refused now with the free function's
    // sentence, which is the rule the issue asks for in every method.
    let unknown = "struct S { n: i64 }\n\
                   trait Get { fn get(s: S) -> i64 { return nope!() + 40; } }\n\
                   fn main() { print_int(1); }\n";
    assert_eq!(
        outcome(unknown, "macro_trait_default"),
        "REFUSED: Unknown macro 'nope'"
    );
}

/// AN IMPORTED MODULE IS EXPANDED WITH ITS OWN MACROS.
///
/// Main never expanded a module: the resolver parsed it and passed it on, so
/// a module's own macro in its own `pub fn` reached the type checker
/// unexpanded. A module's macros are its own — the importing file's are not
/// visible in it, which is refused by name — and the refusals of a single file
/// apply inside a module unchanged.
#[test]
fn an_imported_module_is_expanded_with_its_own_macros() {
    let rows: [(&str, &str, &str, &str); 4] = [
        (
            "a module's macro in its own pub fn",
            "macro two!() { 2 }\npub fn forty_two() -> i64 { return two!() + 40; }\n",
            "import lib1;\nfn main() { print_int(forty_two()); }\n",
            "RUN: 42\n",
        ),
        (
            "a module's statement macro, nested in a loop",
            "macro say!(v) { print_int($v); }\n\
             pub fn count() { let mut i = 0; while i < 2 { say!(i); i = i + 1; } }\n",
            "import lib1;\nfn main() { count(); }\n",
            "RUN: 0\n1\n",
        ),
        (
            "a module invoking a macro only the importing file defines",
            "pub fn forty_two() -> i64 { return two!() + 40; }\n",
            "import lib1;\nmacro two!() { 2 }\nfn main() { print_int(forty_two()); }\n",
            "REFUSED: error: ./lib1.pd: Unknown macro 'two'",
        ),
        (
            "a module's argument that invokes a macro",
            "macro double!(x) { $x * 2 }\npub fn f() -> i64 { return double!(double!(1)); }\n",
            "import lib1;\nfn main() { print_int(f()); }\n",
            "REFUSED: error[PD0067]: ./lib1.pd: the macro `double` is given `double!(…)` as an argument, \
             and expansion is a single pass: an invocation that an expansion produces is never \
             expanded. Bind it to a `let` first, or call a function",
        ),
    ];
    let mut got = Vec::new();
    let mut want = Vec::new();
    for (shape, lib, main, out) in rows {
        got.push(format!(
            "{}: {}",
            shape,
            outcome_of_files(&[("lib1.pd", lib)], main)
        ));
        want.push(format!("{}: {}", shape, out));
    }
    assert_eq!(got, want);
}
