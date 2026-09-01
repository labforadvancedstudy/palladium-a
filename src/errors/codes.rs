// Stable diagnostic codes — GI-12.
//
// WHAT A CODE IS. A `PD####` names a SEMANTIC REJECTION CONDITION: the language
// rule this compiler enforces, not the sentence it currently says and not the
// `CompileError` variant or the construction site it happens to be raised from.
// That is the whole purchase: a manifest row that pins `code=PD0003` survives a
// rewording of the message and a refactor of the site, and an incidental refusal
// that merely happens to contain the same words cannot satisfy it.
//
// NUMBERS ARE MEANINGLESS AND MONOTONIC. There are no family bands, because a
// band invites renumbering when a rule moves between families and renumbering is
// exactly what a stable code may not do. Nothing in this repository may read
// arithmetic into a code.
//
// THE STABILITY CONTRACT (spec D7), stated so it can be checked rather than
// remembered:
//   * the code<->condition association is permanent;
//   * a rewording of the message never changes a code;
//   * splitting a condition MINTS a new code and leaves the old one meaning
//     exactly what it meant for the rows that remain;
//   * a merge never re-purposes the absorbed code — it RETIRES to a tombstone;
//   * a tombstoned number is never reused, and `TOMBSTONES` below is the
//     machine-readable form of that promise.
//
// SCOPE OF THIS FILE TODAY. su1 wires the two SEED conditions end to end; the
// remaining conditions of the locked semantic map are minted by the emission
// slices (su2+). `ALL` is therefore the compiler's honest inventory of what it
// can currently emit, not a copy of the map, and the registry gate compares in
// that direction only.

use std::fmt;

/// A stable diagnostic code.
///
/// Typed rather than a `String`: a raw string carrier lets a construction site
/// invent `PD9999` or `pd0003`, and the first thing GI-12 owes is that the set
/// of codes this compiler can emit is enumerable from the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticCode {
    /// PD0002 — the initialiser of a top-level `const`/`static` has no value in
    /// the target's arithmetic. One `refuse` closure, `src/typeck/mod.rs`;
    /// the fault (a zero divisor, an overflow, a shift outside `0..63`) is the
    /// PARAMETER of one rule, which is why the six witnesses share one code.
    ConstInitialiserHasNoValue,

    /// PD0003 — a cast among the numeric primitives and `bool`, or between
    /// `char` and `i64`. One predicate over the PAIR (`src/typeck/mod.rs`,
    /// the `legal` match): its middle arm is a symmetric or-pattern, so no
    /// branch anywhere is reached by direction. Direction lives in the
    /// formatted `found` clause, i.e. in the particulars, which are not
    /// identity.
    CastRelation,
}

impl DiagnosticCode {
    /// Every code this compiler can emit today, in allocation order.
    ///
    /// The `--dump-diagnostic-codes` inventory and the registry gate both read
    /// this, so a code that exists in the enum and is missing here is a gate
    /// failure rather than a silent omission (see `every_code_is_in_all`).
    pub const ALL: &'static [DiagnosticCode] = &[
        DiagnosticCode::ConstInitialiserHasNoValue,
        DiagnosticCode::CastRelation,
    ];

    /// The numbers that are RETIRED and must never be allocated again, with the
    /// condition each one named before its merge.
    ///
    /// These six came out of the su0 map review: each was folded into a
    /// surviving code because the two were one rule seen from two positions.
    /// D7 forbids re-pointing them, and forbids closing the holes by
    /// renumbering the survivors.
    pub const TOMBSTONES: &'static [(u16, &'static str)] = &[
        (
            25,
            "nested array inner length, `the field ...` caller spelling",
        ),
        (47, "argument type, const-generic callee spelling"),
        (51, "`macro_rules!`, invocation position spelling"),
        (61, "receiver write-through, `&self` detail spelling"),
        (65, "loop/break value agreement, break-side spelling"),
        (
            73,
            "single-pass expansion, macro-argument position spelling",
        ),
    ];

    /// The allocated number. `PD0002` is 2.
    pub const fn number(self) -> u16 {
        match self {
            DiagnosticCode::ConstInitialiserHasNoValue => 2,
            DiagnosticCode::CastRelation => 3,
        }
    }

    /// The registry's `symbolic_name` column. A name is a convenience for
    /// humans and for grep; the NUMBER is the identity.
    pub const fn symbolic_name(self) -> &'static str {
        match self {
            DiagnosticCode::ConstInitialiserHasNoValue => "const_initialiser_has_no_value",
            DiagnosticCode::CastRelation => "cast_relation",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    /// The wire spelling, and the only one: `PD` then exactly four digits.
    /// The parser anchors on `^error\[PD[0-9]{4}\]: `, so a code that formatted
    /// differently would be invisible to every consumer.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PD{:04}", self.number())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// `ALL` is the inventory the gate trusts, so a code missing from it would
    /// make the registry check pass by not looking. Enumerating the enum from
    /// the outside is impossible in Rust, so this asserts the count against a
    /// literal that a new variant must be edited to change — the edit is the
    /// prompt.
    #[test]
    fn every_code_is_in_all() {
        assert_eq!(
            DiagnosticCode::ALL.len(),
            2,
            "a code was added to the enum without being added to ALL (or this \
             literal was not updated with the new count)"
        );
    }

    #[test]
    fn codes_and_names_are_unique() {
        let numbers: HashSet<u16> = DiagnosticCode::ALL.iter().map(|c| c.number()).collect();
        assert_eq!(numbers.len(), DiagnosticCode::ALL.len(), "duplicate number");
        let names: HashSet<&str> = DiagnosticCode::ALL
            .iter()
            .map(|c| c.symbolic_name())
            .collect();
        assert_eq!(names.len(), DiagnosticCode::ALL.len(), "duplicate name");
    }

    /// D7's no-reuse promise, enforced in the compiler rather than only in the
    /// registry TSV: a TSV can be edited by the same commit that re-points a
    /// code, and then the two agree about something false.
    #[test]
    fn no_active_code_reuses_a_tombstoned_number() {
        let retired: HashSet<u16> = DiagnosticCode::TOMBSTONES.iter().map(|(n, _)| *n).collect();
        for c in DiagnosticCode::ALL {
            assert!(
                !retired.contains(&c.number()),
                "{} reuses the retired number {}",
                c.symbolic_name(),
                c.number()
            );
        }
    }

    #[test]
    fn the_wire_spelling_is_four_digits() {
        assert_eq!(
            DiagnosticCode::ConstInitialiserHasNoValue.to_string(),
            "PD0002"
        );
        assert_eq!(DiagnosticCode::CastRelation.to_string(), "PD0003");
    }
}
