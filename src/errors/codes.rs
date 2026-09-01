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

    /// PD0008 — a non-integer literal is not representable in the macro token
    /// stream. One `refuse` closure in `token_to_ast_token`
    /// (`src/parser/mod.rs`), whose `what` names the KIND that was written:
    /// `AstToken::Literal` is a `String` carrying no kind, so a string, a float,
    /// a char and a boolean all come back as text. The kind is the PARAMETER of
    /// one rule, which is why the three corpus witnesses share this code.
    ///
    /// The closure is SHARED with `MultiCharacterOperatorInMacroTokenStream`, so
    /// the code is passed to it per arm rather than attached inside it — one
    /// closure is not one rule, and a code that lived in the closure would say
    /// the two were.
    NonIntegerLiteralInMacroTokenStream,

    /// PD0013 — a top-level item is one of the declaration forms this language
    /// has. The `_` arm of `parse_item` (`src/parser/mod.rs`), which is where a
    /// file that is not Palladium source at all arrives. TWO witnesses with
    /// CHARACTER-IDENTICAL payloads and no discriminator between them, on
    /// purpose: one is a `package.pd` manifest and the other an `extern` block,
    /// and the rule refuses them for the same reason — neither is a declaration
    /// — so a fragment telling them apart would be pinning the fixture rather
    /// than the rule.
    TopLevelItemIsADeclarationForm,

    /// PD0020 — a top-level initialiser has to be a constant expression. One
    /// `refuse` closure in `validate_global_initializer` (`src/parser/mod.rs`);
    /// the form it saw (a call, a name that reads another item, an array
    /// literal, an `if`) is the PARAMETER, because the reason is the same for
    /// every one of them: the item becomes a C file-scope definition and
    /// nothing runs before `main`.
    TopLevelInitialiserMustBeConstant,

    /// PD0029 — a macro invocation writes its arguments in PARENTHESES:
    /// `name!(...)` is the one call shape. The `consume` after `!` in
    /// `parse_postfix` (`src/parser/mod.rs`), so `vec![7]` — the spelling every
    /// Rust program uses, and the one `src/macros/mod.rs` registers a builtin
    /// for — does not parse.
    MacroInvocationIsParenthesised,

    /// PD0030 — a function's parameter list is `name: type` items closed by
    /// `)`. TWO POSITIONS, one rule: the free function (`parse_function`) and
    /// the method (`parse_impl`'s method loop), both `src/parser/mod.rs`. The
    /// call-argument `)`s elsewhere in the file are a DIFFERENT rule and stay
    /// uncoded — a shared token is not a shared condition.
    ParameterListIsClosedByParen,

    /// PD0031 — a `let` initialiser is mandatory: `=` follows the binding.
    /// `parse_let` (`src/parser/mod.rs`). `grammar.ebnf`'s `let_stmt` has no
    /// optional-initialiser branch, and this is the half of that production
    /// refused by the `=` position; `LetBindsOneBareIdentifier` is the other
    /// half, refused a few lines earlier by a different branch.
    LetInitialiserIsMandatory,

    /// PD0032 — a generic parameter list holds BARE NAMES and is closed by `>`;
    /// the `:` that would introduce a trait bound ends the list. FIVE POSITIONS
    /// declare generic parameters (function, trait, trait method, impl, type
    /// alias — all `src/parser/mod.rs`) and all five carry this code, for the
    /// reason PD0049 is one code in two positions. The generic ARGUMENT list at
    /// a use site is a different rule and is not coded.
    GenericParameterListIsBareNames,

    /// PD0033 — a CHAINED tuple index has to be parenthesised. `.0.1` matches
    /// the float rule (digits, dot, digits), so it arrives as ONE `Float` token
    /// and the two indices are gone: `p.0.10` and `p.0.1` both hold 0.1, and a
    /// compiler that recovered the digits would be guessing with a wrong answer
    /// available. The `Token::Float` arm of the postfix `.` loop.
    ChainedTupleIndexIsParenthesised,

    /// PD0034 — the high end of a range pattern is a literal, because
    /// open-ended range patterns are not in the language. `parse_range_pattern`
    /// (`src/parser/mod.rs`), the arm reached after `..`/`..=`.
    RangePatternHighEndIsALiteral,

    /// PD0035 — a range pattern has BOTH endpoints. The `DotDot | DotDotEq` arm
    /// of `parse_pattern_primary`: `..5` has no low end. A separate condition
    /// from `RangePatternHighEndIsALiteral` because the locked map keeps them
    /// apart — the missing end is a different position in the production, not a
    /// parameter of one refusal.
    RangePatternHasBothEndpoints,

    /// PD0036 — a tuple index is written WITHOUT leading zeros. The lexer hands
    /// the parser an `i64`, so the spelling is gone by then and the check is on
    /// the SPAN: an index spelled wider than its value has digits. Without it
    /// `p.01` compiled silently as `.1`, which is the shape of a typo that
    /// changes which element a program reads. The neighbouring refusal in the
    /// same arm (a NEGATIVE index) is a third rule with no corpus witness and
    /// stays uncoded.
    TupleIndexHasNoLeadingZeros,

    /// PD0037 — a tuple PATTERN has at least two elements: grouping is not a
    /// pattern form, so `(p)` is not a way to write `p`. `parse_pattern_primary`.
    TuplePatternHasAtLeastTwoElements,

    /// PD0038 — a tuple has at least two elements: `(e)` is grouping and the
    /// one-element tuple `(e,)` is not a form this language has. The EXPRESSION
    /// position, `parse_primary`. Kept apart from
    /// `TuplePatternHasAtLeastTwoElements` by the locked map: one arity rule is
    /// stated over values and the other over patterns, and the two productions
    /// can move independently.
    TupleHasAtLeastTwoElements,

    /// PD0039 — an expression position holds one of the expression forms this
    /// language has. The catch-all of `parse_primary` (`src/parser/mod.rs`), and
    /// its corpus witness is a closure literal: there is no `Closure` node in
    /// `src/ast/`, so `|` in expression position is not the start of anything.
    ExpressionPositionHoldsAnExpressionForm,

    /// PD0040 — a pattern position holds one of the pattern forms this language
    /// has. The catch-all of `parse_pattern_primary`. Its witness is a bare
    /// `{ x }`: field shorthand is a spelling INSIDE a variant path, so a brace
    /// does not begin pattern content anywhere else.
    PatternPositionHoldsAPatternForm,

    /// PD0041 — `let` binds ONE BARE IDENTIFIER; there are no `let` patterns.
    /// `parse_let` (`src/parser/mod.rs`). The `for` loop's variable position a
    /// few hundred lines down prints the SAME SENTENCE and is a different rule,
    /// so it is not coded with this number — which is the whole reason the code
    /// is attached at a site and not derived from a message.
    LetBindsOneBareIdentifier,

    /// PD0049 — `macro_rules!` is not this language's macro system: there is ONE
    /// and no procedural/declarative split (N3-14). Stated in TWO POSITIONS —
    /// the item position in `parse_item` (`src/parser/mod.rs`) and the
    /// invocation position in `unknown_macro` (`src/macros/mod.rs`) — and one
    /// sentence said in two places is one rule, which is why PD0051 was retired
    /// into this code rather than kept as the invocation spelling.
    MacroRulesIsNotThisMacroSystem,

    /// PD0066 — a function that declares a return type must produce a value on
    /// every path. ONE predicate, `returns_on_every_path` in
    /// `src/parser/mod.rs`: its two refusals differ only in what the author
    /// wrote (a value in tail position on some paths, or no value anywhere),
    /// not in the rule, so both carry this code. A non-void C function that
    /// falls off its end returns the register's contents.
    ReturnValueOnEveryPath,

    /// PD0067 — macro expansion is a single pass, so an invocation PRODUCED by
    /// an expansion is never expanded. Refused where the program can be edited,
    /// in a macro BODY (`register_macro`) and in a macro ARGUMENT
    /// (`refuse_nested_invocation`), both `src/macros/mod.rs`: the source calls
    /// the second "the same one-pass fact seen from the other side", which is
    /// why PD0073 was retired into this code.
    SinglePassExpansion,

    /// PD0068 — a macro body may substitute only its own parameters.
    /// `register_macro` (`src/macros/mod.rs`): an unmatched `$name` was left in
    /// place and re-parsed, so the diagnostic came from the CALL site about a
    /// character the caller never wrote.
    MacroBodySubstitutesOwnParameters,

    /// PD0069 — a parameter named in a macro body must be written `$name`,
    /// because a bare name resolves at the CALL site. Two arms of one match in
    /// `register_macro` (`src/macros/mod.rs`) — position 0 and everywhere else —
    /// which the source itself calls "the same defect with a cheaper test", so
    /// they are one condition and carry one code.
    MacroParameterNeedsDollar,

    /// PD0074 — a multi-character operator may not appear in a macro body or
    /// argument, because `AstToken::Punct` holds one `char` and `= =` is not
    /// `==`. One arm over ten operator tokens in `token_to_ast_token`
    /// (`src/parser/mod.rs`). It SHARES the `refuse` closure with
    /// `NonIntegerLiteralInMacroTokenStream` and is a different rule: the
    /// literal refusals are about a lost KIND, this one about a
    /// representation that does not exist.
    MultiCharacterOperatorInMacroTokenStream,

    /// PD0077 — a `\` in a literal must be followed by a spelling the escape
    /// table names. The lexer raises `LexError`, which carries no code, so the
    /// code is attached where a lexical refusal BECOMES a `CompileError`
    /// (`src/lexer/scanner.rs`) — the same place for the string position and the
    /// char position, which are the one rule.
    UnknownEscapeSpelling,

    /// PD0078 — a `/*` must be closed. One nesting-aware site in the lexer,
    /// coded at the `LexError` -> `CompileError` conversion for the reason
    /// `UnknownEscapeSpelling` is.
    UnterminatedBlockComment,
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
        DiagnosticCode::NonIntegerLiteralInMacroTokenStream,
        DiagnosticCode::TopLevelItemIsADeclarationForm,
        DiagnosticCode::TopLevelInitialiserMustBeConstant,
        DiagnosticCode::MacroInvocationIsParenthesised,
        DiagnosticCode::ParameterListIsClosedByParen,
        DiagnosticCode::LetInitialiserIsMandatory,
        DiagnosticCode::GenericParameterListIsBareNames,
        DiagnosticCode::ChainedTupleIndexIsParenthesised,
        DiagnosticCode::RangePatternHighEndIsALiteral,
        DiagnosticCode::RangePatternHasBothEndpoints,
        DiagnosticCode::TupleIndexHasNoLeadingZeros,
        DiagnosticCode::TuplePatternHasAtLeastTwoElements,
        DiagnosticCode::TupleHasAtLeastTwoElements,
        DiagnosticCode::ExpressionPositionHoldsAnExpressionForm,
        DiagnosticCode::PatternPositionHoldsAPatternForm,
        DiagnosticCode::LetBindsOneBareIdentifier,
        DiagnosticCode::MacroRulesIsNotThisMacroSystem,
        DiagnosticCode::ReturnValueOnEveryPath,
        DiagnosticCode::SinglePassExpansion,
        DiagnosticCode::MacroBodySubstitutesOwnParameters,
        DiagnosticCode::MacroParameterNeedsDollar,
        DiagnosticCode::MultiCharacterOperatorInMacroTokenStream,
        DiagnosticCode::UnknownEscapeSpelling,
        DiagnosticCode::UnterminatedBlockComment,
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
            DiagnosticCode::NonIntegerLiteralInMacroTokenStream => 8,
            DiagnosticCode::TopLevelItemIsADeclarationForm => 13,
            DiagnosticCode::TopLevelInitialiserMustBeConstant => 20,
            DiagnosticCode::MacroInvocationIsParenthesised => 29,
            DiagnosticCode::ParameterListIsClosedByParen => 30,
            DiagnosticCode::LetInitialiserIsMandatory => 31,
            DiagnosticCode::GenericParameterListIsBareNames => 32,
            DiagnosticCode::ChainedTupleIndexIsParenthesised => 33,
            DiagnosticCode::RangePatternHighEndIsALiteral => 34,
            DiagnosticCode::RangePatternHasBothEndpoints => 35,
            DiagnosticCode::TupleIndexHasNoLeadingZeros => 36,
            DiagnosticCode::TuplePatternHasAtLeastTwoElements => 37,
            DiagnosticCode::TupleHasAtLeastTwoElements => 38,
            DiagnosticCode::ExpressionPositionHoldsAnExpressionForm => 39,
            DiagnosticCode::PatternPositionHoldsAPatternForm => 40,
            DiagnosticCode::LetBindsOneBareIdentifier => 41,
            DiagnosticCode::MacroRulesIsNotThisMacroSystem => 49,
            DiagnosticCode::ReturnValueOnEveryPath => 66,
            DiagnosticCode::SinglePassExpansion => 67,
            DiagnosticCode::MacroBodySubstitutesOwnParameters => 68,
            DiagnosticCode::MacroParameterNeedsDollar => 69,
            DiagnosticCode::MultiCharacterOperatorInMacroTokenStream => 74,
            DiagnosticCode::UnknownEscapeSpelling => 77,
            DiagnosticCode::UnterminatedBlockComment => 78,
        }
    }

    /// The registry's `symbolic_name` column. A name is a convenience for
    /// humans and for grep; the NUMBER is the identity.
    pub const fn symbolic_name(self) -> &'static str {
        match self {
            DiagnosticCode::ConstInitialiserHasNoValue => "const_initialiser_has_no_value",
            DiagnosticCode::CastRelation => "cast_relation",
            DiagnosticCode::NonIntegerLiteralInMacroTokenStream => {
                "non_integer_literal_in_macro_token_stream"
            }
            DiagnosticCode::TopLevelItemIsADeclarationForm => {
                "top_level_item_is_a_declaration_form"
            }
            DiagnosticCode::TopLevelInitialiserMustBeConstant => {
                "top_level_initialiser_must_be_constant"
            }
            DiagnosticCode::MacroInvocationIsParenthesised => "macro_invocation_is_parenthesised",
            DiagnosticCode::ParameterListIsClosedByParen => "parameter_list_is_closed_by_paren",
            DiagnosticCode::LetInitialiserIsMandatory => "let_initialiser_is_mandatory",
            DiagnosticCode::GenericParameterListIsBareNames => {
                "generic_parameter_list_is_bare_names"
            }
            DiagnosticCode::ChainedTupleIndexIsParenthesised => {
                "chained_tuple_index_is_parenthesised"
            }
            DiagnosticCode::RangePatternHighEndIsALiteral => "range_pattern_high_end_is_a_literal",
            DiagnosticCode::RangePatternHasBothEndpoints => "range_pattern_has_both_endpoints",
            DiagnosticCode::TupleIndexHasNoLeadingZeros => "tuple_index_has_no_leading_zeros",
            DiagnosticCode::TuplePatternHasAtLeastTwoElements => {
                "tuple_pattern_has_at_least_two_elements"
            }
            DiagnosticCode::TupleHasAtLeastTwoElements => "tuple_has_at_least_two_elements",
            DiagnosticCode::ExpressionPositionHoldsAnExpressionForm => {
                "expression_position_holds_an_expression_form"
            }
            DiagnosticCode::PatternPositionHoldsAPatternForm => {
                "pattern_position_holds_a_pattern_form"
            }
            DiagnosticCode::LetBindsOneBareIdentifier => "let_binds_one_bare_identifier",
            DiagnosticCode::MacroRulesIsNotThisMacroSystem => {
                "macro_rules_is_not_this_macro_system"
            }
            DiagnosticCode::ReturnValueOnEveryPath => "return_value_on_every_path",
            DiagnosticCode::SinglePassExpansion => "single_pass_expansion",
            DiagnosticCode::MacroBodySubstitutesOwnParameters => {
                "macro_body_substitutes_own_parameters"
            }
            DiagnosticCode::MacroParameterNeedsDollar => "macro_parameter_needs_dollar",
            DiagnosticCode::MultiCharacterOperatorInMacroTokenStream => {
                "multi_character_operator_in_macro_token_stream"
            }
            DiagnosticCode::UnknownEscapeSpelling => "unknown_escape_spelling",
            DiagnosticCode::UnterminatedBlockComment => "unterminated_block_comment",
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
            26,
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
        assert_eq!(
            DiagnosticCode::NonIntegerLiteralInMacroTokenStream.to_string(),
            "PD0008"
        );
        assert_eq!(
            DiagnosticCode::UnterminatedBlockComment.to_string(),
            "PD0078"
        );
    }
}
