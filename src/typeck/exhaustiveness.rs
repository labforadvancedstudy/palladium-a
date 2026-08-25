// Pattern exhaustiveness checking for Palladium
// "Ensuring all possibilities are covered"

use crate::ast::{Pattern, PatternData};
use crate::errors::{CompileError, Result, Span};
use std::collections::{HashMap, HashSet};

/// Represents a pattern in exhaustiveness checking
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PatternKind {
    /// Wildcard pattern (_) - matches anything
    Wildcard,
    /// Variable binding - matches anything and binds it
    Binding(String),
    /// Enum constructor pattern
    Constructor {
        enum_name: String,
        variant: String,
        arity: usize,
    },
    /// Or-pattern (N6-07) — matches if any alternative does.
    Or(Vec<PatternKind>),
    /// Tuple pattern (N6-05) — matches element-wise.
    Tuple(Vec<PatternKind>),
    /// Range pattern (N6-03) — matches an interval, and contributes nothing to
    /// completeness for the reason given at the walk that ignores it.
    Range {
        lo: crate::ast::PatternLiteral,
        hi: crate::ast::PatternLiteral,
        inclusive: bool,
    },
    /// Literal pattern (N6-02) — matches one value of its type.
    ///
    /// CONTRIBUTES TO COMPLETENESS OVER `bool` AND NOTHING ELSE. `true` and
    /// `false` are the whole domain of a `bool`; no finite set of integer or
    /// string literals is the whole domain of an `i64` or a `String`, so over
    /// those a literal arm covers a point and leaves the rest to a catch-all.
    Literal(crate::ast::PatternLiteral),
}

/// Information about enum variants for exhaustiveness checking
#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub name: String,
    pub variants: Vec<VariantInfo>,
}

#[derive(Debug, Clone)]
pub struct VariantInfo {
    pub name: String,
    #[allow(dead_code)]
    pub arity: usize, // Number of fields (0 for unit variants)
}

/// Pattern exhaustiveness checker
pub struct ExhaustivenessChecker {
    /// Information about all enums in the program
    enums: HashMap<String, EnumInfo>,
}

impl ExhaustivenessChecker {
    pub fn new(enums: HashMap<String, EnumInfo>) -> Self {
        Self { enums }
    }

    /// The patterns a completeness count should actually see.
    ///
    /// TWO NORMALISATIONS, both of them required for a right answer rather than
    /// a convenience:
    ///
    ///  * an OR-PATTERN contributes ALL of its alternatives (N6-07), so
    ///    `Circle | Square` covers two variants and a match carrying it plus
    ///    `Triangle` needs no wildcard;
    ///  * a BINDING PATTERN contributes exactly what its inner contributes
    ///    (N6-08) — `all @ Circle` is a `Circle` arm that also names the value,
    ///    and reading it as a bare binder would make it a catch-all that swallows
    ///    every later arm as unreachable.
    ///
    /// Done once, here, so the enum walk and the redundancy walk cannot disagree
    /// about what an arm covers.
    fn normalize(patterns: &[Pattern]) -> Vec<Pattern> {
        fn push(pattern: &Pattern, out: &mut Vec<Pattern>) {
            match pattern {
                Pattern::Or(alternatives) => {
                    for alternative in alternatives {
                        push(alternative, out);
                    }
                }
                Pattern::Binding { inner, .. } => push(inner, out),
                other => out.push(other.clone()),
            }
        }
        let mut out = Vec::with_capacity(patterns.len());
        for pattern in patterns {
            push(pattern, &mut out);
        }
        out
    }

    /// Does this pattern match EVERY value of its type?
    ///
    /// N6-10's whole question for a type whose values cannot be enumerated. The
    /// four irrefutable shapes, and no others:
    ///
    ///  * `_` and a bare binding, which is what irrefutable means;
    ///  * `name @ inner`, exactly when `inner` is — the binding tests nothing;
    ///  * `a | b`, when EITHER alternative is — one of them always fires;
    ///  * `(a, b)`, when EVERY element is — the conjunction always holds.
    ///
    /// A literal and a range are refutable however wide they are. `0..=<i64 max>`
    /// beside `<i64 min>..=-1` covers the domain and is still refused: proving
    /// that is interval arithmetic, and this checker does not do arithmetic. The
    /// diagnostic says so rather than leaving a reader to infer it.
    pub fn is_irrefutable(pattern: &Pattern) -> bool {
        match pattern {
            Pattern::Wildcard | Pattern::Ident(_) => true,
            Pattern::Binding { inner, .. } => Self::is_irrefutable(inner),
            Pattern::Or(alternatives) => alternatives.iter().any(Self::is_irrefutable),
            Pattern::Tuple(elements) => elements.iter().all(Self::is_irrefutable),
            Pattern::Literal(_) | Pattern::Range { .. } | Pattern::EnumPattern { .. } => false,
        }
    }

    /// Check if a match expression is exhaustive
    pub fn check_match(&self, matched_type: &str, patterns: &[Pattern], span: Span) -> Result<()> {
        let patterns = &Self::normalize(patterns);
        // If the matched type is an enum, check exhaustiveness
        if let Some(enum_info) = self.enums.get(matched_type) {
            self.check_enum_exhaustiveness(enum_info, patterns, span)
        } else {
            // For non-enum types, we need at least one wildcard or binding pattern
            let has_catchall = patterns
                .iter()
                .any(|p| matches!(p, Pattern::Wildcard | Pattern::Ident(_)));
            if !has_catchall {
                Err(CompileError::NonExhaustiveMatch {
                    missing_patterns: vec!["_ (wildcard pattern)".to_string()],
                    span: Some(span),
                })
            } else {
                Ok(())
            }
        }
    }

    /// The one payload shape whose values can be UNIONED across arms: a single
    /// bool literal in one position, with every other position irrefutable.
    ///
    /// Answers the position's key and the literal's value. Any other shape — two
    /// refutable positions, a non-bool literal, a range — answers `None`, because
    /// unioning it would be a claim about a space this checker does not model.
    fn bool_split_slot(data: Option<&PatternData>) -> Option<(String, bool)> {
        fn peel(pattern: &Pattern) -> &Pattern {
            match pattern {
                Pattern::Binding { inner, .. } => peel(inner),
                other => other,
            }
        }
        let slots: Vec<(String, &Pattern)> = match data? {
            PatternData::Tuple(patterns) => patterns
                .iter()
                .enumerate()
                .map(|(i, p)| (i.to_string(), p))
                .collect(),
            PatternData::Struct(fields) => fields
                .iter()
                .map(|(name, p)| (name.clone(), p))
                .collect(),
        };
        let mut found: Option<(String, bool)> = None;
        for (key, pattern) in slots {
            match peel(pattern) {
                Pattern::Literal(crate::ast::PatternLiteral::Bool(value)) => {
                    if found.is_some() {
                        // Two bool positions is a product space again.
                        return None;
                    }
                    found = Some((key, *value));
                }
                other if Self::is_irrefutable(other) => {}
                _ => return None,
            }
        }
        found
    }

    /// Does an enum variant's payload accept every value the variant can hold?
    ///
    /// The question `check_enum_exhaustiveness` needs before it may count a
    /// variant as covered. No payload at all is irrefutable (a unit variant
    /// matches whenever its tag does); otherwise every sub-pattern must be.
    fn payload_is_irrefutable(data: Option<&PatternData>) -> bool {
        match data {
            None => true,
            Some(PatternData::Tuple(patterns)) => patterns.iter().all(Self::is_irrefutable),
            Some(PatternData::Struct(fields)) => {
                fields.iter().all(|(_, pattern)| Self::is_irrefutable(pattern))
            }
        }
    }

    /// Check that a `bool` scrutinee is covered.
    ///
    /// THE ONLY TYPE A LITERAL ARM CAN EXHAUST. `true` and `false` are two
    /// values and there is no third, so a match carrying both is complete with
    /// no catch-all — which is what makes `match flag { true => …, false => … }`
    /// a legal program rather than one that needs a `_` arm nobody can reach.
    ///
    /// Every other scrutinee type keeps the pre-existing position (unchecked
    /// unless it is an enum): N6-10, "a non-exhaustive match is a compile error
    /// for EVERY scrutinee type", is a separate owed row, and enforcing it here
    /// for integers would refuse programs this compiler accepts today without
    /// the trap N6-11 requires to make that refusal honest.
    pub fn check_bool_match(&self, patterns: &[Pattern], span: Span) -> Result<()> {
        let patterns = &Self::normalize(patterns);
        let mut has_true = false;
        let mut has_false = false;
        for pattern in patterns {
            match pattern {
                Pattern::Wildcard | Pattern::Ident(_) => return Ok(()),
                Pattern::Literal(crate::ast::PatternLiteral::Bool(true)) => has_true = true,
                Pattern::Literal(crate::ast::PatternLiteral::Bool(false)) => has_false = true,
                _ => {}
            }
        }
        if has_true && has_false {
            return Ok(());
        }
        let missing = match (has_true, has_false) {
            (true, false) => vec!["false".to_string()],
            (false, true) => vec!["true".to_string()],
            _ => vec!["true".to_string(), "false".to_string()],
        };
        Err(CompileError::NonExhaustiveMatch {
            missing_patterns: missing,
            span: Some(span),
        })
    }

    /// Check if patterns are exhaustive for an enum
    fn check_enum_exhaustiveness(
        &self,
        enum_info: &EnumInfo,
        patterns: &[Pattern],
        span: Span,
    ) -> Result<()> {
        // Track which variants are covered
        let mut covered_variants = HashSet::new();
        let mut has_wildcard = false;
        let mut unreachable_patterns = Vec::new();
        // Variants a pattern named but did not cover, and the bool literals those
        // patterns pinned. See `bool_split_slot` for the one shape that adds up.
        let mut seen_refutably: HashSet<String> = HashSet::new();
        let mut bool_splits: HashMap<(String, String), HashSet<bool>> = HashMap::new();

        for (i, pattern) in patterns.iter().enumerate() {
            match pattern {
                Pattern::Wildcard | Pattern::Ident(_) => {
                    // Wildcard or binding matches all remaining variants
                    if has_wildcard || covered_variants.len() == enum_info.variants.len() {
                        unreachable_patterns.push((i, pattern.to_string()));
                    }
                    has_wildcard = true;
                }
                // A literal is not a variant, and `check_pattern` in the type
                // checker has already refused it against an enum scrutinee.
                // Counted as covering nothing rather than assumed unreachable.
                // N6-03. A RANGE CONTRIBUTES NOTHING, and `normalize` leaves it
                // alone for that reason: `i64` is not enumerable by any finite
                // set of ranges this checker counts, so `0..=59` plus `60..=100`
                // is not "complete" here even when a reader can see that it is.
                // Full product/interval analysis is N6-10's problem (4e), not a
                // half-measure smuggled in as a normalisation.
                Pattern::Literal(_) | Pattern::Range { .. } => {}
                // N6-05. A tuple covers something only when EVERY element is
                // irrefutable, and then it covers everything — which for an enum
                // scrutinee is not reachable anyway (a tuple is not a variant).
                // Counting `(0, b)` would need the product space: `(0, _)` plus
                // `(1, _)` plus … is not a completeness question this checker can
                // answer without one. N6-10 (4e) is where that lives.
                Pattern::Tuple(_) => {}
                // `normalize` removed these before the walk began; the arms
                // exist so a future caller that forgets to normalise fails to
                // compile rather than silently counting nothing.
                Pattern::Or(_) | Pattern::Binding { .. } => {}
                Pattern::EnumPattern {
                    enum_name,
                    variant,
                    data,
                } => {
                    if enum_name != &enum_info.name {
                        return Err(CompileError::TypeMismatch {
                            expected: enum_info.name.clone(),
                            found: enum_name.clone(),
                            span: Some(span),
                        });
                    }

                    // Check if this variant exists
                    if !enum_info.variants.iter().any(|v| &v.name == variant) {
                        return Err(CompileError::Generic(format!(
                            "Unknown variant '{}::{}' in match pattern",
                            enum_name, variant
                        )));
                    }

                    // A VARIANT IS COVERED ONLY IF ITS PAYLOAD IS IRREFUTABLE.
                    // `P::Num(1)` matches SOME `Num`s, and counting it as the
                    // variant made `match P::Num(9) { P::Num(1) => 10 }` a
                    // complete match: it compiled, took no arm at run time and
                    // trapped — which is the trap doing its job for a program the
                    // checker should never have accepted. Same rule as everywhere
                    // else in this file, and the same predicate (`is_irrefutable`).
                    //
                    // A refutable arm cannot make a LATER arm unreachable:
                    // `P::Num(1)` then `P::Num(2)` are two different questions
                    // about the same variant, and neither answers the other. But
                    // an arm reached after the variant is ALREADY COVERED is dead
                    // whatever its own payload asks — measured: `P::Num(_)` then
                    // `P::Num(1)` compiled with the second arm unreachable,
                    // because the check asked whether the LATER arm covers rather
                    // than whether the variant already was.
                    let covers = Self::payload_is_irrefutable(data.as_ref());
                    if has_wildcard || covered_variants.contains(variant) {
                        unreachable_patterns.push((i, pattern.to_string()));
                    } else if covers {
                        covered_variants.insert(variant.clone());
                    } else {
                        // Not covered by this arm. Two things are still worth
                        // recording: that the variant was SEEN refutably (so the
                        // diagnostic can say what is actually wrong), and any
                        // bool literal it pins in a payload position (so
                        // `B(true)` + `B(false)` can add up).
                        seen_refutably.insert(variant.clone());
                        if let Some((slot, value)) = Self::bool_split_slot(data.as_ref()) {
                            let values = bool_splits.entry((variant.clone(), slot)).or_default();
                            values.insert(value);
                            // PROMOTED THE MOMENT THE SPLIT COMPLETES, not after
                            // the walk. `bool` has two values, so the arm that
                            // supplies the second one finishes the variant — and
                            // anything written after it is dead. Folding this in
                            // at the end left `On(true)`, `On(false)`, `On(_)`
                            // compiling with a third arm nothing could reach,
                            // which is the defect H1 fixed for irrefutable
                            // payloads showing up again one rule over.
                            if values.contains(&true) && values.contains(&false) {
                                covered_variants.insert(variant.clone());
                            }
                        }
                    }
                }
            }
        }

        // Report unreachable patterns
        if !unreachable_patterns.is_empty() {
            return Err(CompileError::UnreachablePattern {
                patterns: unreachable_patterns.into_iter().map(|(_, p)| p).collect(),
                span: Some(span),
            });
        }

        // (The bool split that completes a variant is promoted INSIDE the walk
        // above, so a later arm on that variant meets the dead-arm check like any
        // other. `bool` is the one payload space that adds up: two values and no
        // third, which is 4a's rule for a `bool` SCRUTINEE reaching one level
        // down. Enum, integer and string payload spaces are NOT unioned, because
        // two arms pinning different values of such a position leave the rest of
        // it open.)

        // Check if all variants are covered
        if !has_wildcard && covered_variants.len() < enum_info.variants.len() {
            let missing_variants: Vec<String> = enum_info
                .variants
                .iter()
                .filter(|v| !covered_variants.contains(&v.name))
                .map(|v| {
                    // NAMING A VARIANT AS "MISSING" WHEN IT IS WRITTEN THREE
                    // TIMES reads as a compiler that cannot see the program. If
                    // the variant appears with payloads none of which accept
                    // every value, say THAT, and say what would fix it.
                    if seen_refutably.contains(&v.name) {
                        format!(
                            "{}::{} — it is matched only with refutable payloads, so no arm \
                             accepts every {}::{}; add `{}::{}(_)` or a `_` arm",
                            enum_info.name,
                            v.name,
                            enum_info.name,
                            v.name,
                            enum_info.name,
                            v.name
                        )
                    } else {
                        format!("{}::{}", enum_info.name, v.name)
                    }
                })
                .collect();

            return Err(CompileError::NonExhaustiveMatch {
                missing_patterns: missing_variants,
                span: Some(span),
            });
        }

        Ok(())
    }

    /// Check for redundant patterns (patterns that can never match)
    #[allow(dead_code)]
    pub fn check_redundancy(patterns: &[Pattern]) -> Vec<(usize, String)> {
        let mut redundant = Vec::new();
        let mut seen_wildcard = false;
        let mut seen_variants = HashSet::new();

        for (i, pattern) in patterns.iter().enumerate() {
            match pattern {
                Pattern::Wildcard | Pattern::Ident(_) => {
                    if seen_wildcard {
                        redundant.push((i, "This pattern is unreachable".to_string()));
                    }
                    seen_wildcard = true;
                }
                Pattern::Literal(_)
                | Pattern::Range { .. }
                | Pattern::Or(_)
                | Pattern::Tuple(_)
                | Pattern::Binding { .. } => {}
                Pattern::EnumPattern {
                    enum_name, variant, ..
                } => {
                    let variant_key = format!("{}::{}", enum_name, variant);
                    if seen_wildcard {
                        redundant.push((i, "This pattern is unreachable (previous wildcard pattern covers all cases)".to_string()));
                    } else if seen_variants.contains(&variant_key) {
                        redundant.push((i, format!("Variant '{}' already covered", variant_key)));
                    } else {
                        seen_variants.insert(variant_key);
                    }
                }
            }
        }

        redundant
    }
}

/// Helper to extract pattern information from AST patterns
impl Pattern {
    /// Convert AST pattern to exhaustiveness checker pattern kind
    pub fn to_pattern_kind(&self) -> PatternKind {
        match self {
            Pattern::Wildcard => PatternKind::Wildcard,
            Pattern::Ident(name) => PatternKind::Binding(name.clone()),
            Pattern::Literal(literal) => PatternKind::Literal(literal.clone()),
            Pattern::Range {
                lo,
                hi,
                inclusive,
            } => PatternKind::Range {
                lo: lo.clone(),
                hi: hi.clone(),
                inclusive: *inclusive,
            },
            Pattern::Or(alternatives) => {
                PatternKind::Or(alternatives.iter().map(|p| p.to_pattern_kind()).collect())
            }
            Pattern::Tuple(elements) => {
                PatternKind::Tuple(elements.iter().map(|p| p.to_pattern_kind()).collect())
            }
            // Transparent, exactly as it is to completeness counting.
            Pattern::Binding { inner, .. } => inner.to_pattern_kind(),
            Pattern::EnumPattern {
                enum_name,
                variant,
                data,
            } => {
                let arity = match data {
                    None => 0,
                    Some(PatternData::Tuple(patterns)) => patterns.len(),
                    Some(PatternData::Struct(fields)) => fields.len(),
                };
                PatternKind::Constructor {
                    enum_name: enum_name.clone(),
                    variant: variant.clone(),
                    arity,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_option_enum() -> EnumInfo {
        EnumInfo {
            name: "Option".to_string(),
            variants: vec![
                VariantInfo {
                    name: "Some".to_string(),
                    arity: 1,
                },
                VariantInfo {
                    name: "None".to_string(),
                    arity: 0,
                },
            ],
        }
    }

    #[allow(dead_code)]
    fn create_result_enum() -> EnumInfo {
        EnumInfo {
            name: "Result".to_string(),
            variants: vec![
                VariantInfo {
                    name: "Ok".to_string(),
                    arity: 1,
                },
                VariantInfo {
                    name: "Err".to_string(),
                    arity: 1,
                },
            ],
        }
    }

    #[test]
    fn test_exhaustive_enum_match() {
        let mut enums = HashMap::new();
        enums.insert("Option".to_string(), create_option_enum());

        let checker = ExhaustivenessChecker::new(enums);

        let patterns = vec![
            Pattern::EnumPattern {
                enum_name: "Option".to_string(),
                variant: "Some".to_string(),
                data: Some(PatternData::Tuple(vec![Pattern::Ident("x".to_string())])),
            },
            Pattern::EnumPattern {
                enum_name: "Option".to_string(),
                variant: "None".to_string(),
                data: None,
            },
        ];

        assert!(checker
            .check_match("Option", &patterns, Span::dummy())
            .is_ok());
    }

    #[test]
    fn test_non_exhaustive_enum_match() {
        let mut enums = HashMap::new();
        enums.insert("Option".to_string(), create_option_enum());

        let checker = ExhaustivenessChecker::new(enums);

        let patterns = vec![Pattern::EnumPattern {
            enum_name: "Option".to_string(),
            variant: "Some".to_string(),
            data: Some(PatternData::Tuple(vec![Pattern::Ident("x".to_string())])),
        }];

        let result = checker.check_match("Option", &patterns, Span::dummy());
        assert!(result.is_err());

        if let Err(CompileError::NonExhaustiveMatch {
            missing_patterns, ..
        }) = result
        {
            assert_eq!(missing_patterns, vec!["Option::None"]);
        } else {
            panic!("Expected NonExhaustiveMatch error");
        }
    }

    #[test]
    fn test_wildcard_makes_exhaustive() {
        let mut enums = HashMap::new();
        enums.insert("Option".to_string(), create_option_enum());

        let checker = ExhaustivenessChecker::new(enums);

        let patterns = vec![
            Pattern::EnumPattern {
                enum_name: "Option".to_string(),
                variant: "Some".to_string(),
                data: Some(PatternData::Tuple(vec![Pattern::Ident("x".to_string())])),
            },
            Pattern::Wildcard,
        ];

        assert!(checker
            .check_match("Option", &patterns, Span::dummy())
            .is_ok());
    }

    #[test]
    fn test_unreachable_pattern_after_wildcard() {
        let mut enums = HashMap::new();
        enums.insert("Option".to_string(), create_option_enum());

        let checker = ExhaustivenessChecker::new(enums);

        let patterns = vec![
            Pattern::Wildcard,
            Pattern::EnumPattern {
                enum_name: "Option".to_string(),
                variant: "None".to_string(),
                data: None,
            },
        ];

        let result = checker.check_match("Option", &patterns, Span::dummy());
        assert!(result.is_err());

        if let Err(CompileError::UnreachablePattern { .. }) = result {
            // Expected
        } else {
            panic!("Expected UnreachablePattern error");
        }
    }

    #[test]
    fn test_duplicate_variant_pattern() {
        let mut enums = HashMap::new();
        enums.insert("Option".to_string(), create_option_enum());

        let checker = ExhaustivenessChecker::new(enums);

        let patterns = vec![
            Pattern::EnumPattern {
                enum_name: "Option".to_string(),
                variant: "None".to_string(),
                data: None,
            },
            Pattern::EnumPattern {
                enum_name: "Option".to_string(),
                variant: "None".to_string(),
                data: None,
            },
            Pattern::EnumPattern {
                enum_name: "Option".to_string(),
                variant: "Some".to_string(),
                data: Some(PatternData::Tuple(vec![Pattern::Wildcard])),
            },
        ];

        let result = checker.check_match("Option", &patterns, Span::dummy());
        assert!(result.is_err());

        if let Err(CompileError::UnreachablePattern { .. }) = result {
            // Expected
        } else {
            panic!("Expected UnreachablePattern error");
        }
    }
}
