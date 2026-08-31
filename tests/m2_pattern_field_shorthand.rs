//! N6 FIELD SHORTHAND — `Move { x, y }` is `Move { x: x, y: y }`.
//!
//! WHY THIS IS A PARSE-TREE TEST AND NOT ANOTHER BEHAVIOURAL ONE. The shorthand
//! is desugared where it is parsed (`src/parser/mod.rs`, the struct-pattern arm
//! of `parse_pattern_primary`), and the claim that matters is not "shorthand
//! behaves the same" — a fixture can show that for the cases it happens to
//! contain — but that there is NO SECOND SHAPE downstream at all. `Pattern`
//! carries no span, so the two spellings are not merely equivalent: they are the
//! same value, and `assert_eq!` can say so. That is what licenses the twelve
//! `PatternData::Struct` consumers in typeck, codegen, the borrow checker and
//! exhaustiveness to have gone unchanged.
//!
//! The behavioural half is `tests/06_field_shorthand.pd`; the two boundaries are
//! `tests/reject/field_shorthand_needs_a_struct_variant.pd` and
//! `tests/reject/brace_pattern_needs_a_variant_path.pd`.

use palladium::ast::{Item, Pattern, PatternData, PatternLiteral, Stmt};
use palladium::lexer::Lexer;
use palladium::parser::Parser;

/// The patterns of the arms of the first `match` in the first function.
fn arm_patterns(source: &str) -> Vec<Pattern> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.collect_tokens().expect("lexes");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().expect("parses");

    for item in &ast.items {
        if let Item::Function(func) = item {
            for stmt in &func.body {
                if let Stmt::Match { arms, .. } = stmt {
                    return arms.iter().map(|a| a.pattern.clone()).collect();
                }
            }
        }
    }
    panic!("no match statement in the fixture");
}

fn program(pattern: &str) -> String {
    format!(
        "enum M {{ Move {{ x: i64, y: i64 }}, Quit }}
         fn f(m: M) {{
             match m {{
                 {} => {{ print(\"hit\"); }}
                 M::Quit => {{ print(\"quit\"); }}
             }}
         }}",
        pattern
    )
}

#[test]
fn shorthand_and_the_explicit_form_parse_to_the_same_value() {
    let short = arm_patterns(&program("M::Move { x, y }"));
    let long = arm_patterns(&program("M::Move { x: x, y: y }"));
    assert_eq!(
        short, long,
        "the shorthand must desugar to the explicit form, not to a shape of its own"
    );
}

#[test]
fn a_mixed_pattern_desugars_only_the_bare_fields() {
    // `x` bare, `h` explicit and bound to a DIFFERENT name, so this cannot pass
    // by the two sides being trivially identical.
    let mixed = arm_patterns(&program("M::Move { x, y: why }"));
    let long = arm_patterns(&program("M::Move { x: x, y: why }"));
    assert_eq!(mixed, long);
}

#[test]
fn the_shorthand_binds_the_field_name_as_an_ident_pattern() {
    // The desugaring stated positively rather than by comparison: the field is
    // `x` and the pattern under it is `Pattern::Ident("x")`.
    let arms = arm_patterns(&program("M::Move { x, y }"));
    match &arms[0] {
        Pattern::EnumPattern {
            data: Some(PatternData::Struct(fields)),
            ..
        } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].0, "x");
            assert_eq!(fields[0].1, Pattern::Ident("x".to_string()));
            assert_eq!(fields[1].0, "y");
            assert_eq!(fields[1].1, Pattern::Ident("y".to_string()));
        }
        other => panic!("expected a struct-variant pattern, got {:?}", other),
    }
}

#[test]
fn field_order_is_the_names_and_not_the_positions() {
    // Written backwards against the declaration. The desugaring must follow the
    // NAME, or `w`/`h` would silently swap — which no `assert_eq!` between two
    // spellings of the same order could catch.
    let arms = arm_patterns(&program("M::Move { y, x }"));
    match &arms[0] {
        Pattern::EnumPattern {
            data: Some(PatternData::Struct(fields)),
            ..
        } => {
            assert_eq!(fields[0].0, "y");
            assert_eq!(fields[0].1, Pattern::Ident("y".to_string()));
            assert_eq!(fields[1].0, "x");
            assert_eq!(fields[1].1, Pattern::Ident("x".to_string()));
        }
        other => panic!("expected a struct-variant pattern, got {:?}", other),
    }
}

#[test]
fn the_explicit_form_is_untouched_by_the_shorthand_arm() {
    // The control on the change itself: an explicit field whose pattern is NOT a
    // bare identifier still parses as THAT pattern. Asserted by VALUE and not as
    // `assert_ne!(…, Ident("x"))`, which a parser that corrupted `x: 1` into any
    // other shape at all would still satisfy — the negative form pins nothing
    // about what the field actually holds.
    let arms = arm_patterns(&program("M::Move { x: 1, y: _ }"));
    match &arms[0] {
        Pattern::EnumPattern {
            data: Some(PatternData::Struct(fields)),
            ..
        } => {
            assert_eq!(fields[0].0, "x");
            assert_eq!(fields[0].1, Pattern::Literal(PatternLiteral::Int(1)));
            assert_eq!(fields[1].0, "y");
            assert_eq!(fields[1].1, Pattern::Wildcard);
        }
        other => panic!("expected a struct-variant pattern, got {:?}", other),
    }
}
