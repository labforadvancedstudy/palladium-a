//! The SHAPE of the C emitted for a range `for` header (N5-14).
//!
//! WHY A SHAPE TEST AND NOT A BEHAVIOURAL ONE. The defect these pin is
//! `..=` reaching the maximum `i64`: `for (v = lo; v <= hi; v++)` increments
//! past `hi` on the last iteration, which is signed overflow — undefined
//! behaviour, and in practice a loop with no exit. That case cannot be RUN:
//! iterating to `i64::MAX` does not terminate on any machine. So what is
//! checked is the emitted C, which is where the property lives.
//!
//! The companion behavioural evidence is `tests/04_range_endpoints.pd`, a
//! conformance fixture proving the ordinary behaviour of `..=` is unchanged.

mod common;
use common::unique_source_name;
use palladium::{CompileError, Driver};

fn compile_source(source: &str) -> Result<String, CompileError> {
    let driver = Driver::new();
    driver
        .compile_string(source, &unique_source_name("m2rl"))
        .map(|path| std::fs::read_to_string(path).unwrap_or_else(|_| String::new()))
}

fn c_of(source: &str) -> String {
    compile_source(source)
        .unwrap_or_else(|e| panic!("failed to compile:\n{}\nerror: {}", source, e))
}

/// The line that used to be emitted, and must never be again.
///
/// Written as a search for `<=` beside `++` on ONE line rather than for the
/// whole old string: the defect is the combination, and a reformatting of the
/// emitter should not be able to hide it.
fn has_post_visit_increment(c: &str) -> bool {
    c.lines()
        .filter(|l| l.contains("for ("))
        .any(|l| l.contains("<=") && l.contains("++") && !l.contains("unsigned long long"))
}

#[test]
fn an_inclusive_range_never_increments_past_its_last_value() {
    let c = c_of("fn main() { let mut s = 0; for i in 0..=3 { s = s + i; } print_int(s); }");
    assert!(
        !has_post_visit_increment(&c),
        "an inclusive `for` still tests with `<=` and increments a signed counter, \
         which overflows when the endpoint is i64::MAX:\n{}",
        c
    );
    assert!(
        c.contains("unsigned long long"),
        "the inclusive form should count with an unsigned index:\n{}",
        c
    );
}

#[test]
fn an_exclusive_range_keeps_the_plain_counted_loop() {
    // Not merely allowed — WANTED. `v < hi` cannot overflow, so the exclusive
    // form pays nothing for the fix, and this pins that it was not changed
    // along with its neighbour.
    let c = c_of("fn main() { let mut s = 0; for i in 0..3 { s = s + i; } print_int(s); }");
    assert!(
        c.contains("< __pd_hi") || c.contains("< __pd_hi0"),
        "the exclusive form should still be a plain `v < hi` counted loop:\n{}",
        c
    );
}

#[test]
fn both_endpoints_are_read_into_temporaries_before_the_loop() {
    // The PRE-EXISTING half: the endpoint used to sit in the `for` test, so a
    // call there ran once per iteration.
    let c = c_of("fn f() -> i64 { 4 } fn main() { for i in 0..f() { print_int(i); } }");
    let header = c
        .lines()
        .find(|l| l.contains("for ("))
        .unwrap_or_else(|| panic!("no `for` header in:\n{}", c));
    assert!(
        !header.contains("f()"),
        "the endpoint call is still inside the loop test, so it runs every iteration: {}",
        header
    );
    assert!(
        c.contains("__pd_hi"),
        "the endpoint should be read into a temporary before the loop:\n{}",
        c
    );
}

#[test]
fn a_range_value_loop_reads_its_bounds_once_and_counts_unsigned() {
    let c =
        c_of("fn main() { let r = 1..=4; let mut s = 0; for i in r { s = s + i; } print_int(s); }");
    assert!(
        c.contains("__pd_last"),
        "the range-value loop should compute its last value once:\n{}",
        c
    );
    assert!(
        !has_post_visit_increment(&c),
        "the range-value loop still increments a signed counter past its last value:\n{}",
        c
    );
}

#[test]
fn nested_range_loops_do_not_share_a_temporary() {
    // `__pd_r` was a fixed name. Two range-VALUE loops one inside the other
    // would have shadowed it; the names are numbered so they cannot.
    let c = c_of(
        "fn main() { let a = 0..2; let b = 0..2; let mut n = 0; \
         for i in a { for j in b { n = n + 1; } } print_int(n); }",
    );
    assert!(
        c.contains("__pd_r0") && c.contains("__pd_r1"),
        "nested range-value loops should get distinct range temporaries:\n{}",
        c
    );
}

// ---------------------------------------------------------------------------
// The two properties a run test cannot reach: a span wider than the signed
// maximum, and a span that is the whole 64-bit domain.

#[test]
fn the_inclusive_span_is_subtracted_in_unsigned_arithmetic() {
    // `(unsigned long long)(hi - lo)` does the SUBTRACTION first, in signed
    // arithmetic, and overflows for any span wider than `i64::MAX` — UBSan on
    // `-1..=i64::MAX`: "9223372036854775807 - -1 cannot be represented in type
    // 'long long'". Each end has to be converted before the subtraction.
    let c = c_of("fn main() { let mut s = 0; for i in 0..=3 { s = s + i; } print_int(s); }");
    assert!(
        !c.contains("(unsigned long long)(__pd_hi"),
        "the span is still computed by subtracting in signed arithmetic:\n{}",
        c
    );
    assert!(
        c.contains("(unsigned long long)__pd_hi") && c.contains("(unsigned long long)__pd_lo"),
        "both ends should be converted before the subtraction:\n{}",
        c
    );
}

#[test]
fn the_visited_value_is_added_in_unsigned_arithmetic() {
    // The counter is unsigned and can run up to the span, so `lo + (long long)k`
    // is a SIGNED addition that overflows for any span wider than the signed
    // maximum: `-1..=i64::MAX` reaches `-1 + 9223372036854775808` on its last
    // iteration. No run test can see it — getting there takes 2^63 iterations —
    // which is exactly why the shape is pinned here instead.
    for source in [
        "fn main() { let mut s = 0; for i in 0..=3 { s = s + i; } print_int(s); }",
        "fn main() { let r = 0..=3; let mut s = 0; for i in r { s = s + i; } print_int(s); }",
    ] {
        let c = c_of(source);
        assert!(
            c.contains("(long long)((unsigned long long)"),
            "the visited value should be added as `unsigned long long` and converted back:\n{}",
            c
        );
        assert!(
            !c.contains("+ (long long)__pd_k0"),
            "the signed `lo + (long long)k` addition is still emitted:\n{}",
            c
        );
    }
}

#[test]
fn a_full_domain_inclusive_range_terminates() {
    // With `k <= n; k++` and `n == ULLONG_MAX`, `k++` wraps to 0 and the test
    // is true forever. The exit condition must be "was the one just visited the
    // last one", which cannot wrap.
    for source in [
        "fn main() { let mut s = 0; for i in 0..=3 { s = s + i; } print_int(s); }",
        "fn main() { let r = 0..=3; let mut s = 0; for i in r { s = s + i; } print_int(s); }",
    ] {
        let c = c_of(source);
        assert!(
            c.contains("__pd_done"),
            "the loop should exit on having visited the last value, not on `k <= n`:\n{}",
            c
        );
        assert!(
            !c.contains("__pd_k0 <= __pd_n0"),
            "the wrapping `k <= n` test is still emitted:\n{}",
            c
        );
    }
}
