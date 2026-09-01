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
    /// PD0001 — a `match` covers every value its scrutinee can take.
    /// FOUR POSITIONS, one rule, all under `src/typeck/`: the bool split and
    /// the enum-variant sweep in `exhaustiveness.rs`, its non-enum wildcard
    /// arm, and the arm in `mod.rs` for a scrutinee whose values this checker
    /// cannot enumerate. What is MISSING is the parameter — a variant name, a
    /// `bool` value, "a `_` or binding arm" — and the ten corpus witnesses are
    /// told apart by it, not by ten codes.
    MatchIsExhaustive,
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

    /// PD0004 — a local binding may not shadow a top-level item. One `Err` in
    /// `refuse_global_shadow` (`src/typeck/mod.rs`); the binder kind it names —
    /// a local, a parameter, a loop variable, a pattern binding, an `@` binding
    /// — is passed IN by the caller, so it is the parameter of one rule and the
    /// four witnesses share this code. Kept apart from
    /// `TopLevelNamesShareOneNamespace` because that one is about two TOP-LEVEL
    /// declarations and the repair differs: a local can be renamed freely.
    LocalBindingMayNotShadowATopLevelItem,

    /// PD0005 — a top-level name is declared once: this language has ONE
    /// namespace for values, functions and types. FOUR POSITIONS state it,
    /// all `src/typeck/mod.rs` — a global against a function, a global against
    /// a type, a global against another global, and `refuse_global_collision`
    /// for a function or type declared against an existing global — and all
    /// four carry this code for the reason PD0049 is one code in two positions.
    /// The source calls the pair "ONE NAMESPACE, CHECKED IN BOTH DIRECTIONS";
    /// which direction was written is the particular.
    TopLevelNamesShareOneNamespace,

    /// PD0007 — `pub` on a top-level `const`/`static` is not implemented,
    /// because nothing exports one and nothing emits a definition for an
    /// imported one. One predicate over `Visibility::Public` in
    /// `register_global` (`src/typeck/mod.rs`); `const` and `static` are the
    /// noun, so the three witnesses share this code.
    PubOnATopLevelItemIsNotImplemented,

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

    /// PD0009 — a pattern has the TYPE of the scrutinee the `match` is on.
    /// THREE PATTERN FORMS state it in `check_pattern` (`src/typeck/mod.rs`):
    /// a tuple pattern against a non-tuple, a range pattern whose endpoints are
    /// a different type, and a literal pattern. All three carry this code —
    /// one rule, three positions — and only the literal position has corpus
    /// witnesses; the other two are reached by programs written in the test.
    PatternHasTheScrutineeType,

    /// PD0010 — a range pattern must be able to match something: `'z'..='a'`
    /// and `3..3` are empty. One predicate over the two endpoints
    /// (`src/typeck/mod.rs`), AFTER the kind and agreement checks have passed;
    /// the computed bounds in the payload are fixture data, which is why three
    /// witnesses share this code.
    RangePatternMatchesSomething,

    /// PD0011 — an alternative of an `|` pattern binds nothing. One `Err`
    /// over `first_binder` (`src/typeck/mod.rs`): every alternative would have
    /// to bind the same names at the same types for the arm's body to be
    /// checkable, and this compiler does not do that, so the binding form is
    /// refused rather than half-supported.
    OrPatternAlternativeBindsNothing,

    /// PD0013 — a top-level item is one of the declaration forms this language
    /// has. The `_` arm of `parse_item` (`src/parser/mod.rs`), which is where a
    /// file that is not Palladium source at all arrives. TWO witnesses with
    /// CHARACTER-IDENTICAL payloads and no discriminator between them, on
    /// purpose: one is a `package.pd` manifest and the other an `extern` block,
    /// and the rule refuses them for the same reason — neither is a declaration
    /// — so a fragment telling them apart would be pinning the fixture rather
    /// than the rule.
    TopLevelItemIsADeclarationForm,

    /// PD0014 — a variant pattern is written in the SHAPE the variant
    /// declares: a tuple payload takes a tuple pattern, a named payload takes
    /// braces. The `_` arm of the payload match in `bind_pattern_variables`
    /// (`src/typeck/mod.rs`). Kept apart from `VariantPatternFieldIsDeclared`:
    /// that one is about a field NAME inside a shape that already fits.
    PatternShapeMatchesTheVariant,

    /// PD0015 — a named field in a variant PATTERN must be one the variant
    /// declares. The lookup in `enum_pattern_field_types`
    /// (`src/typeck/mod.rs`). The CONSTRUCTION position prints the identical
    /// sentence and stays uncoded: the locked map keeps a pattern rule and a
    /// value rule apart even when they read the same, as it does for the tuple
    /// arity pair PD0037/PD0038.
    VariantPatternFieldIsDeclared,

    /// PD0016 — every arm of a `match` is reachable. One `Err` in
    /// `src/typeck/exhaustiveness.rs`, raised after the walk that promotes a
    /// completed `bool` split, so a later arm on a finished variant meets this
    /// check like any other. The message names no pattern, so the two
    /// witnesses have character-identical payloads and take no discriminator.
    MatchArmIsReachable,

    /// PD0017 — a generic method is not implemented: code generation emits no
    /// symbol for one, so the call would fail at link time. TWO POSITIONS in
    /// `src/typeck/mod.rs` — the path spelling `Rect::id(r)` and the method
    /// spelling `r.id()` — which the source itself calls "the same call", and
    /// each has its own corpus witness.
    GenericMethodIsNotImplemented,

    /// PD0018 — an `async fn` is not implemented (N7-18): there is no runtime
    /// to drive the future, and code generation would emit a `<name>_Future`
    /// and a `<name>_poll` that nothing calls. THREE POSITIONS in
    /// `src/typeck/mod.rs` — the entry point, the general `is_async`
    /// predicate, and the deferred check for an `async fn main` that an
    /// imported module might have shadowed — and the source says of the first
    /// that it is a "named sub-case" of the second, kept only for its wording.
    /// The spelling (`async fn` / `async fn main`) is the parameter.
    AsyncFnIsNotImplemented,

    /// PD0019 — a program may not define or shadow a built-in name (N14-02).
    /// One `Err` in `refuse_builtin_definition` (`src/typeck/mod.rs`); the
    /// `reason` varies by whether the name is CALLABLE — a definition nothing
    /// can reach, or one namespace with two meanings — which is the parameter,
    /// the same "one `Err`, reason varies" shape that retired PD0061 into
    /// PD0060. `LocalBinderMayNotShadowABuiltIn` is the sibling one scope in
    /// and is a different rule, as the locked map records.
    ProgramMayNotDefineABuiltInName,

    /// PD0020 — a top-level initialiser has to be a constant expression. One
    /// `refuse` closure in `validate_global_initializer` (`src/parser/mod.rs`);
    /// the form it saw (a call, a name that reads another item, an array
    /// literal, an `if`) is the PARAMETER, because the reason is the same for
    /// every one of them: the item becomes a C file-scope definition and
    /// nothing runs before `main`.
    TopLevelInitialiserMustBeConstant,

    /// PD0021 — a method taking `&mut self` may not be CALLED through a
    /// receiver that is not `&mut self`. One `Err` on the method-call path
    /// (`src/typeck/mod.rs`), reached when the caller's receiver is not
    /// `&mut self` and the callee's is; the `detail` is chosen by the CALLER's
    /// receiver form — a `&self` shared borrow, or a by-value `self` copy the
    /// caller would never observe — which is the parameter of one rule, and is
    /// why the three witnesses share this code. The locked map carried the
    /// by-value spelling as PD0064; it named this same single `Err`, so su3
    /// merged it here and RETIRED 64 to `TOMBSTONES` rather than dropping it.
    /// It was never emitted, and that is not the test: the su0 six were never
    /// emitted either, and a number the map allocated is a number a reader can
    /// cite. `ReceiverWriteThroughNeedsMutSelf` is the neighbour and guards
    /// `Stmt::Assign`; this rule guards the CALL that reaches the same field
    /// through a callee, which that predicate cannot see.
    MutMethodCallNeedsAMutReceiver,

    /// PD0022 — a `let` with a type annotation requires its initialiser to
    /// have exactly that type; there is no implicit conversion. The annotated
    /// arm of `Stmt::Let` (`src/typeck/mod.rs`), which reaches
    /// `TypeErrorHelper::type_mismatch`. That helper is called from the
    /// ASSIGNMENT arm too, enforcing a different rule, so the code is attached
    /// at the call site and never inside the helper — the same reason
    /// `consume_coded` exists on the parser side.
    LetAnnotationAndInitialiserAgree,

    /// PD0029 — a macro invocation writes its arguments in PARENTHESES:
    /// `name!(...)` is the one call shape. The `consume` after `!` in
    /// `parse_postfix` (`src/parser/mod.rs`), so `vec![7]` — the spelling every
    /// Rust program uses, and the one `src/macros/mod.rs` registers a builtin
    /// for — does not parse.
    MacroInvocationIsParenthesised,

    /// PD0030 — a function's parameter list is `name: type` items closed by
    /// `)`. TWO POSITIONS, one rule, both `src/parser/mod.rs`: `parse_function`,
    /// which is the parser for a free function AND for an `impl` method
    /// (`parse_impl` calls it rather than parsing a parameter list of its own),
    /// and the trait-method parser inside `parse_trait`, which does have one.
    /// The call-argument `)`s elsewhere in the file are a DIFFERENT rule and
    /// stay uncoded — a shared token is not a shared condition.
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

    /// PD0042 — an argument must have the parameter's declared type. One
    /// `Err` in the `Expr::Call` argument loop (`src/typeck/mod.rs`). The
    /// const-generic callee spelling reaches the same `Err` — an arity probe
    /// in the su0 map review showed it is not on a separate path — which is
    /// what retired PD0047 into this code; `Int` vs `[Int; N]` is the expected
    /// type, i.e. the particular.
    ArgumentHasTheParameterType,

    /// PD0043 — the left operand of `+` is `Int`, `Float` or `String`. One
    /// arm of the binary-operator check (`src/typeck/mod.rs`). Kept apart from
    /// PD0022 and PD0042 although all three can print the word `Char`: the su0
    /// map review rejected that merge by site, because nothing is shared but a
    /// word in the payload, and a payload is fixture data.
    AdditionOperandIsIntFloatOrString,

    /// PD0044 — a program must contain a `main` function. One predicate over
    /// `self.functions` in `check` (`src/typeck/mod.rs`); the message comes
    /// from `TypeErrorHelper::missing_main`, and the code is attached at the
    /// predicate rather than in the helper for the reason PD0022's is. Its
    /// witness is a `skip` row — a module of a package, which is not a program
    /// — and the refusal is its non-program proof (spec R7).
    ProgramHasAMainFunction,

    /// PD0045 — a name used in a match arm's body must be BOUND by that arm's
    /// pattern; an omitted field is not bound. The suggestion-carrying arm of
    /// the undefined-name path (`src/typeck/mod.rs`). The no-suggestion
    /// spelling of the same rule a few lines down has no fixture and stays
    /// uncoded, which the locked map records as this code's corpus gap.
    MatchArmBodyUsesNamesItsPatternBinds,

    /// PD0046 — a struct type named in the program must be one the program
    /// declares; `try { }` is read as one, because `try` is not a keyword.
    /// THREE POSITIONS in `src/typeck/mod.rs` — a struct literal, an
    /// assignment target's base, and a field read — which the locked map names
    /// as "the identical refusal", so all three carry this code. The
    /// assignment-base position is witnessed by an `xfail` row, the field read
    /// by a program written in the test.
    StructTypeIsDeclared,

    /// PD0048 — constructing a variant of a GENERIC enum is not implemented:
    /// code generation emits no type, no tag and no constructor for one. One
    /// predicate over `generic_enums` in the enum-construction path
    /// (`src/typeck/mod.rs`).
    GenericEnumIsNotImplemented,

    /// PD0049 — `macro_rules!` is not this language's macro system: there is ONE
    /// and no procedural/declarative split (N3-14). Stated in TWO POSITIONS —
    /// the item position in `parse_item` (`src/parser/mod.rs`) and the
    /// invocation position in `unknown_macro` (`src/macros/mod.rs`) — and one
    /// sentence said in two places is one rule, which is why PD0051 was retired
    /// into this code rather than kept as the invocation spelling.
    MacroRulesIsNotThisMacroSystem,

    /// PD0050 — `*self` is not a place: a reference receiver is already
    /// dereferenced by field access, so `*self` asked for a second
    /// indirection. A predicate over the literal token `self` under a `Deref`
    /// (`src/typeck/mod.rs`). The su0 map review kept it apart from the
    /// receiver rules: this one is about the SPELLING, not about what the
    /// receiver form permits.
    DerefSelfIsNotAPlace,

    /// PD0052 — a `mut` parameter on a method is not implemented. One `find`
    /// over the method's parameters in the impl walk (`src/typeck/mod.rs`).
    /// The source records that this is now CONSERVATIVE rather than forced —
    /// the call side takes an address for pointer parameters since su2 — and
    /// that lifting it is its own row; a code names the rule the compiler
    /// enforces today.
    MutParameterOnAMethodIsNotImplemented,

    /// PD0053 — an integer literal cast to `char` must denote a Unicode
    /// scalar. ONE `Err` behind a two-disjunct predicate (`src/typeck/mod.rs`):
    /// outside `0..=0x10FFFF`, or inside the UTF-16 surrogate range. Only the
    /// surrogate disjunct has a corpus fixture; the other is reached by a
    /// program written in the test, and both are this one rule. Refused at
    /// compile time only when the operand is WRITTEN DOWN — a computed value
    /// still traps at run time, which is what `char_from_non_scalar` pins.
    IntegerCastToCharIsAUnicodeScalar,

    /// PD0054 — a `return` with a value inside an `async fn` is not
    /// implemented: the poll function the body is emitted into returns an
    /// `int` readiness flag, so there is nowhere to put the value. Its own
    /// number rather than `AsyncFnIsNotImplemented`'s because the locked map
    /// allocated two; the source calls this arm a "named sub-case" of the
    /// general `is_async` refusal, which is recorded as a tension in the unit's
    /// notes rather than settled by this slice.
    AsyncValueReturnIsNotImplemented,

    /// PD0055 — a loop and its `break`s must agree about whether there is a
    /// value. TWO POSITIONS, both `src/typeck/mod.rs`: a bare `break` out of a
    /// `loop` used as a value, and a valued `break` out of a loop that is not.
    /// The source calls the second "the mirror of `record_break_value`'s
    /// refusal … both directions of the same rule", which is what retired
    /// PD0065 into this code — a symmetric relation whose direction is a
    /// formatted particular, like PD0003's cast matrix.
    LoopAndItsBreaksAgreeAboutAValue,

    /// PD0056 — a top-level `const`/`static` may only have a numeric or
    /// `bool` type, because every other type is built by code that runs and
    /// nothing runs before `main`. One `matches!` over the declared type in
    /// `register_global` (`src/typeck/mod.rs`); `noun` is `const` or `static`
    /// and only `const` has a fixture, which the locked map records as this
    /// code's corpus gap.
    TopLevelItemTypeIsNumericOrBool,

    /// PD0057 — an `if` used as a value must have an `else`, so every path
    /// produces one. The `else_branch.is_none()` predicate
    /// (`src/typeck/mod.rs`). Separate from `ValueIfBranchesAgree`, which is
    /// only REACHABLE once this one is satisfied: two independent obligations
    /// with different groundings, which is why the su0 review rejected the
    /// merge.
    ValueIfHasAnElse,

    /// PD0058 — the two branches of a value `if` must have one type, because
    /// the value lands in one hoisted C temporary. The second predicate of the
    /// same arm (`src/typeck/mod.rs`), reached only after `ValueIfHasAnElse`
    /// has passed.
    ValueIfBranchesAgree,

    /// PD0059 — the two endpoints of a range pattern must be the same kind of
    /// literal: there is no order between a `char` and an `Int`. A separate
    /// predicate from `RangePatternEndpointKind`, and reached only after it
    /// passes (`src/typeck/mod.rs`).
    RangePatternEndpointsAgree,

    /// PD0060 — a receiver that is not `&mut self` may not be written
    /// through. ONE `Err` guarding `Stmt::Assign` (`src/typeck/mod.rs`); the
    /// `detail` is chosen by the `SelfReceiver` kind — a shared borrow, or a
    /// by-value copy the caller would not observe — which is the PARAMETER of
    /// one rule, and is what retired PD0061 into this code.
    ReceiverWriteThroughNeedsMutSelf,

    /// PD0062 — a top-level item is read-only unless it is declared
    /// `static mut`. One arm inside the immutable-assignment path, guarded by
    /// `global_items` (`src/typeck/mod.rs`): it exists because the stock
    /// advice is "declare it with `let mut`", and there is no `let` here to add
    /// `mut` to.
    TopLevelItemIsReadOnlyUnlessStaticMut,

    /// PD0063 — the `self` binding is not reassignable, whatever form the
    /// receiver took. It fires BEFORE the receiver-form test
    /// (`src/typeck/mod.rs`), so it refuses `&mut self` receivers too, which is
    /// what makes it a different rule from
    /// `ReceiverWriteThroughNeedsMutSelf`. Without it `self = C { … }` reached
    /// the stock immutable-binding message, which advises `let mut self` — a
    /// spelling the parser refuses.
    SelfIsNotReassignable,

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

    /// PD0070 — a type that stores itself by value has no layout. One `Err`
    /// at the end of the cycle walk (`src/typeck/mod.rs`); the message has two
    /// branches — a cycle through a zero-length array, which is a deliberate
    /// exclusion (N4-23), and one with nothing on it that can stop — and only
    /// the zero-length branch has a fixture. Both are this rule; the branch is
    /// the reason, not the identity.
    RecursiveTypeHasNoLayout,

    /// PD0071 — the `?` operator is not implemented: code generation has no
    /// lowering of it onto the enum representation it emits. The `Expr::Question`
    /// arm of `check_expression` (`src/typeck/mod.rs`). Code generation refuses
    /// it a second time from its own arm, which nothing reaches while the type
    /// checker runs first, and that arm is a codegen-slice question rather than
    /// this one's.
    QuestionOperatorIsNotImplemented,

    /// PD0072 — a range pattern's endpoints are integer or `char` literals,
    /// the two kinds this language orders. One loop over the low and high
    /// positions (`src/typeck/mod.rs`), whose `ordered` predicate refuses `Str`
    /// and `Bool`; only `Str` in the low position has a fixture, which the
    /// locked map records as this code's corpus gap.
    RangePatternEndpointKind,

    /// PD0074 — a multi-character operator may not appear in a macro body or
    /// argument, because `AstToken::Punct` holds one `char` and `= =` is not
    /// `==`. One arm over ten operator tokens in `token_to_ast_token`
    /// (`src/parser/mod.rs`). It SHARES the `refuse` closure with
    /// `NonIntegerLiteralInMacroTokenStream` and is a different rule: the
    /// literal refusals are about a lost KIND, this one about a
    /// representation that does not exist.
    MultiCharacterOperatorInMacroTokenStream,

    /// PD0075 — a local binder may not have a built-in's name (N14-02). One
    /// `Err` in `refuse_builtin_shadow` (`src/typeck/mod.rs`); `what` names
    /// five binder kinds and only the parameter has a `.pd` fixture. The
    /// sibling one scope out is `ProgramMayNotDefineABuiltInName`, and the map
    /// keeps the two apart for the reason it keeps PD0004 apart from PD0005:
    /// shadowing and redeclaring are different rules with different repairs.
    LocalBinderMayNotShadowABuiltIn,

    /// PD0076 — a block used as a value must end in an expression. One
    /// predicate in `check_value_block` (`src/typeck/mod.rs`): a block whose
    /// last statement ends in `;` has no value to hoist into the C temporary
    /// the value position needs.
    ValueBlockEndsInAnExpression,

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
        DiagnosticCode::MatchIsExhaustive,
        DiagnosticCode::ConstInitialiserHasNoValue,
        DiagnosticCode::CastRelation,
        DiagnosticCode::LocalBindingMayNotShadowATopLevelItem,
        DiagnosticCode::TopLevelNamesShareOneNamespace,
        DiagnosticCode::PubOnATopLevelItemIsNotImplemented,
        DiagnosticCode::NonIntegerLiteralInMacroTokenStream,
        DiagnosticCode::PatternHasTheScrutineeType,
        DiagnosticCode::RangePatternMatchesSomething,
        DiagnosticCode::OrPatternAlternativeBindsNothing,
        DiagnosticCode::TopLevelItemIsADeclarationForm,
        DiagnosticCode::PatternShapeMatchesTheVariant,
        DiagnosticCode::VariantPatternFieldIsDeclared,
        DiagnosticCode::MatchArmIsReachable,
        DiagnosticCode::GenericMethodIsNotImplemented,
        DiagnosticCode::AsyncFnIsNotImplemented,
        DiagnosticCode::ProgramMayNotDefineABuiltInName,
        DiagnosticCode::TopLevelInitialiserMustBeConstant,
        DiagnosticCode::MutMethodCallNeedsAMutReceiver,
        DiagnosticCode::LetAnnotationAndInitialiserAgree,
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
        DiagnosticCode::ArgumentHasTheParameterType,
        DiagnosticCode::AdditionOperandIsIntFloatOrString,
        DiagnosticCode::ProgramHasAMainFunction,
        DiagnosticCode::MatchArmBodyUsesNamesItsPatternBinds,
        DiagnosticCode::StructTypeIsDeclared,
        DiagnosticCode::GenericEnumIsNotImplemented,
        DiagnosticCode::MacroRulesIsNotThisMacroSystem,
        DiagnosticCode::DerefSelfIsNotAPlace,
        DiagnosticCode::MutParameterOnAMethodIsNotImplemented,
        DiagnosticCode::IntegerCastToCharIsAUnicodeScalar,
        DiagnosticCode::AsyncValueReturnIsNotImplemented,
        DiagnosticCode::LoopAndItsBreaksAgreeAboutAValue,
        DiagnosticCode::TopLevelItemTypeIsNumericOrBool,
        DiagnosticCode::ValueIfHasAnElse,
        DiagnosticCode::ValueIfBranchesAgree,
        DiagnosticCode::RangePatternEndpointsAgree,
        DiagnosticCode::ReceiverWriteThroughNeedsMutSelf,
        DiagnosticCode::TopLevelItemIsReadOnlyUnlessStaticMut,
        DiagnosticCode::SelfIsNotReassignable,
        DiagnosticCode::ReturnValueOnEveryPath,
        DiagnosticCode::SinglePassExpansion,
        DiagnosticCode::MacroBodySubstitutesOwnParameters,
        DiagnosticCode::MacroParameterNeedsDollar,
        DiagnosticCode::RecursiveTypeHasNoLayout,
        DiagnosticCode::QuestionOperatorIsNotImplemented,
        DiagnosticCode::RangePatternEndpointKind,
        DiagnosticCode::MultiCharacterOperatorInMacroTokenStream,
        DiagnosticCode::LocalBinderMayNotShadowABuiltIn,
        DiagnosticCode::ValueBlockEndsInAnExpression,
        DiagnosticCode::UnknownEscapeSpelling,
        DiagnosticCode::UnterminatedBlockComment,
    ];

    /// The numbers that are RETIRED and must never be allocated again, with the
    /// condition each one named before its merge.
    ///
    /// Six came out of the su0 map review and the seventh out of su3's
    /// attachment: each was folded into a surviving code because the two were
    /// one rule seen from two positions. D7 forbids re-pointing them, and
    /// forbids closing the holes by renumbering the survivors.
    ///
    /// WHAT MAKES A NUMBER OWE A TOMBSTONE is ALLOCATION IN THE MAP, not first
    /// emission. None of these seven was ever emitted by this compiler — the
    /// su0 six were retired before su1 wired anything, and 64 was retired by
    /// the slice that would have minted it. Retiring on first emission instead
    /// would leave every map-allocated number that a slice merges free for
    /// reuse, and the map is a durable artifact that other readers cite.
    pub const TOMBSTONES: &'static [(u16, &'static str)] = &[
        (
            25,
            "nested array inner length, `the field ...` caller spelling",
        ),
        (47, "argument type, const-generic callee spelling"),
        (51, "`macro_rules!`, invocation position spelling"),
        (61, "receiver write-through, `&self` detail spelling"),
        (
            64,
            "mut-method call through a non-mut receiver, by-value caller spelling",
        ),
        (65, "loop/break value agreement, break-side spelling"),
        (
            73,
            "single-pass expansion, macro-argument position spelling",
        ),
    ];

    /// The allocated number. `PD0002` is 2.
    pub const fn number(self) -> u16 {
        match self {
            DiagnosticCode::MatchIsExhaustive => 1,
            DiagnosticCode::ConstInitialiserHasNoValue => 2,
            DiagnosticCode::CastRelation => 3,
            DiagnosticCode::LocalBindingMayNotShadowATopLevelItem => 4,
            DiagnosticCode::TopLevelNamesShareOneNamespace => 5,
            DiagnosticCode::PubOnATopLevelItemIsNotImplemented => 7,
            DiagnosticCode::NonIntegerLiteralInMacroTokenStream => 8,
            DiagnosticCode::PatternHasTheScrutineeType => 9,
            DiagnosticCode::RangePatternMatchesSomething => 10,
            DiagnosticCode::OrPatternAlternativeBindsNothing => 11,
            DiagnosticCode::TopLevelItemIsADeclarationForm => 13,
            DiagnosticCode::PatternShapeMatchesTheVariant => 14,
            DiagnosticCode::VariantPatternFieldIsDeclared => 15,
            DiagnosticCode::MatchArmIsReachable => 16,
            DiagnosticCode::GenericMethodIsNotImplemented => 17,
            DiagnosticCode::AsyncFnIsNotImplemented => 18,
            DiagnosticCode::ProgramMayNotDefineABuiltInName => 19,
            DiagnosticCode::TopLevelInitialiserMustBeConstant => 20,
            DiagnosticCode::MutMethodCallNeedsAMutReceiver => 21,
            DiagnosticCode::LetAnnotationAndInitialiserAgree => 22,
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
            DiagnosticCode::ArgumentHasTheParameterType => 42,
            DiagnosticCode::AdditionOperandIsIntFloatOrString => 43,
            DiagnosticCode::ProgramHasAMainFunction => 44,
            DiagnosticCode::MatchArmBodyUsesNamesItsPatternBinds => 45,
            DiagnosticCode::StructTypeIsDeclared => 46,
            DiagnosticCode::GenericEnumIsNotImplemented => 48,
            DiagnosticCode::MacroRulesIsNotThisMacroSystem => 49,
            DiagnosticCode::DerefSelfIsNotAPlace => 50,
            DiagnosticCode::MutParameterOnAMethodIsNotImplemented => 52,
            DiagnosticCode::IntegerCastToCharIsAUnicodeScalar => 53,
            DiagnosticCode::AsyncValueReturnIsNotImplemented => 54,
            DiagnosticCode::LoopAndItsBreaksAgreeAboutAValue => 55,
            DiagnosticCode::TopLevelItemTypeIsNumericOrBool => 56,
            DiagnosticCode::ValueIfHasAnElse => 57,
            DiagnosticCode::ValueIfBranchesAgree => 58,
            DiagnosticCode::RangePatternEndpointsAgree => 59,
            DiagnosticCode::ReceiverWriteThroughNeedsMutSelf => 60,
            DiagnosticCode::TopLevelItemIsReadOnlyUnlessStaticMut => 62,
            DiagnosticCode::SelfIsNotReassignable => 63,
            DiagnosticCode::ReturnValueOnEveryPath => 66,
            DiagnosticCode::SinglePassExpansion => 67,
            DiagnosticCode::MacroBodySubstitutesOwnParameters => 68,
            DiagnosticCode::MacroParameterNeedsDollar => 69,
            DiagnosticCode::RecursiveTypeHasNoLayout => 70,
            DiagnosticCode::QuestionOperatorIsNotImplemented => 71,
            DiagnosticCode::RangePatternEndpointKind => 72,
            DiagnosticCode::MultiCharacterOperatorInMacroTokenStream => 74,
            DiagnosticCode::LocalBinderMayNotShadowABuiltIn => 75,
            DiagnosticCode::ValueBlockEndsInAnExpression => 76,
            DiagnosticCode::UnknownEscapeSpelling => 77,
            DiagnosticCode::UnterminatedBlockComment => 78,
        }
    }

    /// The registry's `symbolic_name` column. A name is a convenience for
    /// humans and for grep; the NUMBER is the identity.
    pub const fn symbolic_name(self) -> &'static str {
        match self {
            DiagnosticCode::MatchIsExhaustive => "match_is_exhaustive",
            DiagnosticCode::ConstInitialiserHasNoValue => "const_initialiser_has_no_value",
            DiagnosticCode::CastRelation => "cast_relation",
            DiagnosticCode::LocalBindingMayNotShadowATopLevelItem => {
                "local_binding_may_not_shadow_a_top_level_item"
            }
            DiagnosticCode::TopLevelNamesShareOneNamespace => "top_level_names_share_one_namespace",
            DiagnosticCode::PubOnATopLevelItemIsNotImplemented => {
                "pub_on_a_top_level_item_is_not_implemented"
            }
            DiagnosticCode::NonIntegerLiteralInMacroTokenStream => {
                "non_integer_literal_in_macro_token_stream"
            }
            DiagnosticCode::PatternHasTheScrutineeType => "pattern_has_the_scrutinee_type",
            DiagnosticCode::RangePatternMatchesSomething => "range_pattern_matches_something",
            DiagnosticCode::OrPatternAlternativeBindsNothing => {
                "or_pattern_alternative_binds_nothing"
            }
            DiagnosticCode::TopLevelItemIsADeclarationForm => {
                "top_level_item_is_a_declaration_form"
            }
            DiagnosticCode::PatternShapeMatchesTheVariant => "pattern_shape_matches_the_variant",
            DiagnosticCode::VariantPatternFieldIsDeclared => "variant_pattern_field_is_declared",
            DiagnosticCode::MatchArmIsReachable => "match_arm_is_reachable",
            DiagnosticCode::GenericMethodIsNotImplemented => "generic_method_is_not_implemented",
            DiagnosticCode::AsyncFnIsNotImplemented => "async_fn_is_not_implemented",
            DiagnosticCode::ProgramMayNotDefineABuiltInName => {
                "program_may_not_define_a_built_in_name"
            }
            DiagnosticCode::TopLevelInitialiserMustBeConstant => {
                "top_level_initialiser_must_be_constant"
            }
            DiagnosticCode::MutMethodCallNeedsAMutReceiver => {
                "mut_method_call_needs_a_mut_receiver"
            }
            DiagnosticCode::LetAnnotationAndInitialiserAgree => {
                "let_annotation_and_initialiser_agree"
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
            DiagnosticCode::ArgumentHasTheParameterType => "argument_has_the_parameter_type",
            DiagnosticCode::AdditionOperandIsIntFloatOrString => {
                "addition_operand_is_int_float_or_string"
            }
            DiagnosticCode::ProgramHasAMainFunction => "program_has_a_main_function",
            DiagnosticCode::MatchArmBodyUsesNamesItsPatternBinds => {
                "match_arm_body_uses_names_its_pattern_binds"
            }
            DiagnosticCode::StructTypeIsDeclared => "struct_type_is_declared",
            DiagnosticCode::GenericEnumIsNotImplemented => "generic_enum_is_not_implemented",
            DiagnosticCode::MacroRulesIsNotThisMacroSystem => {
                "macro_rules_is_not_this_macro_system"
            }
            DiagnosticCode::DerefSelfIsNotAPlace => "deref_self_is_not_a_place",
            DiagnosticCode::MutParameterOnAMethodIsNotImplemented => {
                "mut_parameter_on_a_method_is_not_implemented"
            }
            DiagnosticCode::IntegerCastToCharIsAUnicodeScalar => {
                "integer_cast_to_char_is_a_unicode_scalar"
            }
            DiagnosticCode::AsyncValueReturnIsNotImplemented => {
                "async_value_return_is_not_implemented"
            }
            DiagnosticCode::LoopAndItsBreaksAgreeAboutAValue => {
                "loop_and_its_breaks_agree_about_a_value"
            }
            DiagnosticCode::TopLevelItemTypeIsNumericOrBool => {
                "top_level_item_type_is_numeric_or_bool"
            }
            DiagnosticCode::ValueIfHasAnElse => "value_if_has_an_else",
            DiagnosticCode::ValueIfBranchesAgree => "value_if_branches_agree",
            DiagnosticCode::RangePatternEndpointsAgree => "range_pattern_endpoints_agree",
            DiagnosticCode::ReceiverWriteThroughNeedsMutSelf => {
                "receiver_write_through_needs_mut_self"
            }
            DiagnosticCode::TopLevelItemIsReadOnlyUnlessStaticMut => {
                "top_level_item_is_read_only_unless_static_mut"
            }
            DiagnosticCode::SelfIsNotReassignable => "self_is_not_reassignable",
            DiagnosticCode::ReturnValueOnEveryPath => "return_value_on_every_path",
            DiagnosticCode::SinglePassExpansion => "single_pass_expansion",
            DiagnosticCode::MacroBodySubstitutesOwnParameters => {
                "macro_body_substitutes_own_parameters"
            }
            DiagnosticCode::MacroParameterNeedsDollar => "macro_parameter_needs_dollar",
            DiagnosticCode::RecursiveTypeHasNoLayout => "recursive_type_has_no_layout",
            DiagnosticCode::QuestionOperatorIsNotImplemented => {
                "question_operator_is_not_implemented"
            }
            DiagnosticCode::RangePatternEndpointKind => "range_pattern_endpoint_kind",
            DiagnosticCode::MultiCharacterOperatorInMacroTokenStream => {
                "multi_character_operator_in_macro_token_stream"
            }
            DiagnosticCode::LocalBinderMayNotShadowABuiltIn => {
                "local_binder_may_not_shadow_a_built_in"
            }
            DiagnosticCode::ValueBlockEndsInAnExpression => "value_block_ends_in_an_expression",
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
            64,
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
        // The count is asserted for the reason `every_code_is_in_all` asserts
        // its own: a tombstone is permanent, so ADDING one has to be an edit a
        // reviewer sees rather than a line that slid in with a merge.
        assert_eq!(
            DiagnosticCode::TOMBSTONES.len(),
            7,
            "a number was retired (or revived) without this literal being updated"
        );
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
