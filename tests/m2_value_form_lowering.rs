//! Pins around the value forms (N5-03…N5-07, N5-12…N5-17) that a conformance
//! fixture cannot express: the SHAPE of emitted C, a parse that must still be
//! refused, and two gaps recorded as gaps.

mod common;
use common::unique_source_name;
use palladium::{CompileError, Driver};

fn compile_source(source: &str) -> Result<String, CompileError> {
    let driver = Driver::new();
    driver
        .compile_string(source, &unique_source_name("m2vf"))
        .map(|path| std::fs::read_to_string(path).unwrap_or_else(|_| String::new()))
}

fn c_of(source: &str) -> String {
    compile_source(source)
        .unwrap_or_else(|e| panic!("failed to compile:\n{}\nerror: {}", source, e))
}

fn err_of(source: &str) -> String {
    match compile_source(source) {
        Ok(c) => panic!("expected a refusal, got C:\n{}", c),
        Err(e) => e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// The enum/path predicate, and why the two passes may disagree about GENERIC
// enums without that being a defect.
//
// The type checker asks `path_names_an_enum`, which consults BOTH `enums` and
// `generic_enums`. Code generation asks only `self.enums`. A reviewer called
// that a divergence; the counter-argument is that codegen DIVERTS to a call
// only on a POSITIVE hit in `functions`/`impl_methods`, so a generic-enum
// constructor — which is in neither — falls through to the constructor path
// whatever the enum table says. That is an argument about a code path, and an
// argument is not a test. This is the test.

#[test]
fn a_generic_enum_constructor_is_refused_rather_than_emitted() {
    // THIS TEST USED TO ASSERT THE WRONG THING, and the way it was wrong is
    // worth keeping: it checked that a generic-enum constructor was emitted as
    // a CONSTRUCTOR rather than as a path call, and treated that as the right
    // answer. It was not — code generation skips generic enums at every
    // emission site, so the "right" constructor was a call to
    // `Holder_Full__new`, which nothing emits, and a `match` on the result
    // named `__Holder__Empty`, which nothing defines. The C compiler refused
    // C this compiler had approved. Neither emission was correct; the
    // construction is.
    let e = err_of(
        "enum Holder<T> { Empty, Full(T) }\n\
         fn main() { let h: Holder<i64> = Holder::Full(7); }",
    );
    assert!(
        e.contains("generic enums are not implemented"),
        "constructing a generic-enum variant should be refused by name: {}",
        e
    );
}

#[test]
fn the_path_call_diversion_needs_a_positive_function_hit() {
    // The other half of the same argument: a `Type::name(...)` whose name IS in
    // the function table becomes a call. Together with the case above, this pins
    // that the diversion is keyed on the FUNCTION table and not on the absence
    // of an enum.
    let c = c_of(
        "struct Rect { w: i64 }\n\
         impl Rect { fn area(self) -> i64 { self.w } }\n\
         fn main() { let r = Rect { w: 3 }; print_int(Rect::area(r)); }",
    );
    assert!(
        c.contains("__pd_Rect_area"),
        "a static-path method call should be mangled as a call:\n{}",
        c
    );
}

// ---------------------------------------------------------------------------
// N5-12's `>>`, which is not a token.

#[test]
fn a_spaced_double_angle_is_not_a_shift() {
    // `>>` is recognised from two `>` whose SPANS TOUCH. Written apart they are
    // two comparisons, and `a > > b` has no meaning.
    let e = err_of("fn main() { let a = 8; let b = 2; print_int(a > > b); }");
    assert!(
        e.contains("expected expression") || e.contains("Expected expression"),
        "`a > > b` should not parse as a shift: {}",
        e
    );
}

#[test]
fn a_nested_generic_still_closes_with_two_angles() {
    // The reason `>>` is not a token. This must reach the type checker or
    // beyond — what it must NOT do is fail in the lexer or the parser.
    let source = "struct Holder<T> { value: T }\n\
                  fn main() { let h: Holder<Holder<i64>> = Holder { value: Holder { value: 7 } }; }";
    if let Err(e) = compile_source(source) {
        let msg = e.to_string();
        assert!(
            !msg.contains("Unexpected token") && !msg.contains("Unexpected character"),
            "a nested generic failed to PARSE, which is what a `>>` token would do: {}",
            msg
        );
    }
}

// ---------------------------------------------------------------------------
// Two gaps, recorded as gaps. Neither is fixed here; both are named so that
// closing one has a place to turn green.

/// A `self`-by-value method MOVES its receiver, in every spelling.
///
/// This was a documenting test asserting the opposite — that the double move
/// was ACCEPTED — because the borrow checker consulted a signature only for a
/// bare-identifier callee. `impl` methods were registered under `Type::method`
/// the whole time and nothing looked them up, and the path spelling never
/// reached the call check at all, arriving as an `EnumConstructor`. Both are
/// resolved now, so the test asserts the refusal instead of the gap.
///
/// The free-function spelling is the control: it was always refused, and a
/// method call must not be a way around it.
#[test]
fn a_by_value_receiver_is_moved_by_the_dot_form() {
    let e = err_of(
        "struct S { v: i64 }\n\
         impl S { fn take(self) -> i64 { self.v } }\n\
         fn main() { let s = S { v: 1 }; print_int(s.take()); print_int(s.take()); }",
    );
    assert!(
        e.contains("moved") && e.contains('s'),
        "the second `s.take()` should be a use after move: {}",
        e
    );
}

#[test]
fn a_by_value_receiver_is_moved_by_the_path_form_too() {
    let e = err_of(
        "struct S { v: i64 }\n\
         impl S { fn take(self) -> i64 { self.v } }\n\
         fn main() { let s = S { v: 1 }; print_int(S::take(s)); print_int(S::take(s)); }",
    );
    assert!(
        e.contains("moved"),
        "the path spelling must move the receiver too: {}",
        e
    );
}

#[test]
fn one_call_on_a_by_value_receiver_is_still_fine() {
    // The other direction, so the refusal above is not simply "methods are
    // refused".
    let c = c_of(
        "struct S { v: i64 }\n\
         impl S { fn take(self) -> i64 { self.v } }\n\
         fn main() { let s = S { v: 1 }; print_int(s.take()); }",
    );
    assert!(
        c.contains("__pd_S_take"),
        "a single call should compile:\n{}",
        c
    );
}

/// AN ENUM-OWNED METHOD IS UNREACHABLE BY ITS PATH FORM.
///
/// `path_names_an_enum` short-circuits before the function lookup, so
/// `Color::code(c)` is read as a constructor for a variant named `code` even
/// when an `impl Color` declares that method. The DOT form works.
///
/// DECLARED AS A FAILURE, not pinned as a success. It was a green test that
/// REQUIRED the defect — `panic!` if the program ever compiled — which inverts
/// the moment the compiler improves: the day this is fixed, the suite goes red
/// for the fix. An `#[ignore]`d XFAIL with a row in
/// `tests/rust-debt-manifest.txt` says the same thing in the direction the
/// repository can act on, and `scripts/test-xfail.py` checks it is still
/// failing for the reason named.
#[test]
#[ignore = "XFAIL: an enum-owned method is unreachable through `Type::method(x)` — `path_names_an_enum` short-circuits before the function lookup, so the path is read as a constructor for a variant that does not exist. The DOT form works (owned by M4, 'Traits with real dispatch')"]
fn an_enum_owned_method_is_reachable_by_its_path_form() {
    let c = c_of(
        "enum Color { Red, Blue }\n\
         impl Color { fn code(self) -> i64 { 1 } }\n\
         fn main() { let c = Color::Red; print_int(Color::code(c)); }",
    );
    assert!(
        c.contains("__pd_Color_code"),
        "the path form should reach the method:\n{}",
        c
    );
}

// ---------------------------------------------------------------------------
// Macro expansion reaches inside the value forms.

#[test]
fn a_macro_inside_a_value_if_branch_is_expanded() {
    // The expander's expression walker had no arm for `Expr::If`/`Block`/
    // `Loop`/`Match`, so an invocation inside one fell to the catch-all and was
    // left in the tree — reaching a code generator that is documented never to
    // see a macro. What is asserted is the ABSENCE of that outcome: whatever
    // happens, it is not "Unexpected macro invocation in code generation".
    let source = "fn main() { let x = if 1 < 2 { vec!(1) } else { vec!(2) }; }";
    if let Err(e) = compile_source(source) {
        let msg = e.to_string();
        assert!(
            !msg.contains("Unexpected macro invocation"),
            "a macro inside a value-`if` branch reached code generation unexpanded: {}",
            msg
        );
    }
}

#[test]
fn a_macro_inside_a_value_block_is_expanded() {
    let source = "fn main() { let x = { vec!(1) }; }";
    if let Err(e) = compile_source(source) {
        let msg = e.to_string();
        assert!(
            !msg.contains("Unexpected macro invocation"),
            "a macro inside a value block reached code generation unexpanded: {}",
            msg
        );
    }
}
