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

    /// Check if a match expression is exhaustive
    pub fn check_match(&self, matched_type: &str, patterns: &[Pattern], span: Span) -> Result<()> {
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

        for (i, pattern) in patterns.iter().enumerate() {
            match pattern {
                Pattern::Wildcard | Pattern::Ident(_) => {
                    // Wildcard or binding matches all remaining variants
                    if has_wildcard || covered_variants.len() == enum_info.variants.len() {
                        unreachable_patterns.push((i, pattern.to_string()));
                    }
                    has_wildcard = true;
                }
                Pattern::EnumPattern {
                    enum_name, variant, ..
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

                    // Check if already covered by wildcard
                    if has_wildcard || covered_variants.contains(variant) {
                        unreachable_patterns.push((i, pattern.to_string()));
                    } else {
                        covered_variants.insert(variant.clone());
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

        // Check if all variants are covered
        if !has_wildcard && covered_variants.len() < enum_info.variants.len() {
            let missing_variants: Vec<String> = enum_info
                .variants
                .iter()
                .filter(|v| !covered_variants.contains(&v.name))
                .map(|v| format!("{}::{}", enum_info.name, v.name))
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
