// Type checker error suggestions for Palladium
// "Helping developers fix type errors with intelligent suggestions"

use crate::errors::{suggestions::SuggestionEngine, CompileError, Span};

/// Enhanced type error creation with suggestions
pub struct TypeErrorHelper {
    available_variables: Vec<String>,
    available_functions: Vec<String>,
    available_types: Vec<String>,
}

impl TypeErrorHelper {
    pub fn new() -> Self {
        Self {
            available_variables: Vec::new(),
            available_functions: Vec::new(),
            available_types: Vec::new(),
        }
    }

    /// Update available identifiers for suggestions
    pub fn update_available(&mut self, vars: Vec<String>, funcs: Vec<String>, types: Vec<String>) {
        self.available_variables = vars;
        self.available_functions = funcs;
        self.available_types = types;
    }

    /// Create undefined variable error with suggestions
    pub fn undefined_variable(&self, name: &str, span: Option<Span>) -> CompileError {
        let mut error = CompileError::UndefinedVariable {
            name: name.to_string(),
            span,
        };

        // Try to find similar variable names
        if let Some(suggestion) =
            SuggestionEngine::suggest_similar_name(name, &self.available_variables)
        {
            // We'll enhance the error message in the diagnostic conversion
            error = CompileError::Generic(format!(
                "Undefined variable: '{}'. Did you mean '{}'?",
                name, suggestion
            ));
        }

        error
    }

    /// Create undefined function error with suggestions
    pub fn undefined_function(&self, name: &str, span: Option<Span>) -> CompileError {
        // First check if it's a common function that needs an import
        if let Some(import_suggestion) = SuggestionEngine::suggest_import_for_function(name) {
            return CompileError::Generic(format!(
                "Undefined function: '{}'. Try adding: {}",
                name, import_suggestion
            ));
        }

        // Then check for similar function names
        if let Some(suggestion) =
            SuggestionEngine::suggest_similar_name(name, &self.available_functions)
        {
            return CompileError::Generic(format!(
                "Undefined function: '{}'. Did you mean '{}'?",
                name, suggestion
            ));
        }

        CompileError::UndefinedFunction {
            name: name.to_string(),
            span,
        }
    }

    /// Create type mismatch error with conversion suggestions
    pub fn type_mismatch(&self, expected: &str, found: &str, span: Option<Span>) -> CompileError {
        // Check if there's a suggested conversion
        if let Some(conversion) = SuggestionEngine::suggest_type_conversion(found, expected) {
            return CompileError::Generic(format!(
                "Type mismatch: expected {}, found {}. {}",
                expected, found, conversion
            ));
        }

        CompileError::TypeMismatch {
            expected: expected.to_string(),
            found: found.to_string(),
            span,
        }
    }

    /// Create immutable variable assignment error with suggestions
    pub fn immutable_assignment(&self, name: &str) -> CompileError {
        CompileError::Generic(
            format!(
                "Cannot assign to immutable variable '{}'. To make it mutable, declare it with 'let mut {} = ...'", 
                name, name
            )
        )
    }

    /// Create missing main function error with example
    pub fn missing_main() -> CompileError {
        CompileError::Generic(
            "No main function found. Add a main function:\n\nfn main() {\n    // Your code here\n}"
                .to_string(),
        )
    }

    /// Create invalid array index type error
    #[allow(dead_code)]
    pub fn invalid_array_index(&self, found_type: &str) -> CompileError {
        CompileError::Generic(format!(
            "Array indices must be integers. Found '{}'. Convert to int or use an integer literal.",
            found_type
        ))
    }

    /// Create non-boolean condition error
    #[allow(dead_code)]
    pub fn non_boolean_condition(&self, context: &str, found_type: &str) -> CompileError {
        CompileError::Generic(
            format!(
                "{} condition must be a boolean expression. Found '{}'. Use comparison operators (==, !=, <, >, <=, >=) or boolean values (true, false).",
                context, found_type
            )
        )
    }

    /// Create break/continue outside loop error
    pub fn control_flow_outside_loop(&self, keyword: &str) -> CompileError {
        CompileError::Generic(
            format!(
                "'{}' can only be used inside a loop (while or for). Wrap your code in a loop or remove the '{}' statement.",
                keyword, keyword
            )
        )
    }

    /// Create for loop non-array error
    #[allow(dead_code)]
    pub fn for_loop_non_array(&self, found_type: &str) -> CompileError {
        CompileError::Generic(
            format!(
                "For loops require an array or range to iterate over. Found '{}'. Use an array literal [1, 2, 3] or a range (1..10).",
                found_type
            )
        )
    }

    /// Create struct field access error
    #[allow(dead_code)]
    pub fn invalid_field_access(
        &self,
        struct_name: &str,
        field_name: &str,
        available_fields: &[String],
    ) -> CompileError {
        if let Some(suggestion) =
            SuggestionEngine::suggest_similar_name(field_name, available_fields)
        {
            CompileError::Generic(format!(
                "Struct '{}' has no field '{}'. Did you mean '{}'?",
                struct_name, field_name, suggestion
            ))
        } else {
            let fields_list = if available_fields.is_empty() {
                "no fields".to_string()
            } else {
                format!("fields: {}", available_fields.join(", "))
            };

            CompileError::Generic(format!(
                "Struct '{}' has no field '{}'. Available {}",
                struct_name, field_name, fields_list
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every ordering of `items`. Each of the three lists below is built by the
    /// checker from a `HashMap` — `Checker::functions` for functions,
    /// `Checker::structs`/`enums` for types, `SymbolTable::scopes`
    /// (`Vec<HashMap<String, VarInfo>>`) for variables — and `RandomState` is
    /// seeded per process, so the order they reach this helper in is a fresh
    /// draw on every run. Enumerating the orderings is a claim about all of
    /// them; running the compiler N times is only a claim about those N runs.
    fn permutations(items: &[&str]) -> Vec<Vec<String>> {
        if items.is_empty() {
            return vec![Vec::new()];
        }

        let mut out = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let mut rest: Vec<&str> = items.to_vec();
            rest.remove(i);
            for mut tail in permutations(&rest) {
                let mut perm = vec![item.to_string()];
                perm.append(&mut tail);
                out.push(perm);
            }
        }
        out
    }

    /// The live call site: `Checker::get_available_functions` feeds
    /// `self.functions.keys()` straight in. `file_read` ties at edit-distance 3
    /// against both `file_read_ex` and `file_seek`, which is what made
    /// `pdc compile bootstrap/v1_archive/archive/compiler_combined.pd` print a
    /// different suggestion run to run (7/5 over 12 processes on 2b43176).
    #[test]
    fn undefined_function_tie_does_not_depend_on_hashmap_order() {
        for perm in permutations(&["file_read_ex", "file_seek", "file_read_all"]) {
            let mut helper = TypeErrorHelper::new();
            helper.update_available(vec![], perm.clone(), vec![]);

            assert_eq!(
                helper.undefined_function("file_read", None).to_string(),
                "Undefined function: 'file_read'. Did you mean 'file_read_ex'?",
                "function suggestion changed with candidate order {:?}",
                perm
            );
        }
    }

    /// The same shape at a call site that no fixture triggers today:
    /// `Checker::get_available_variables` walks `Vec<HashMap<String, VarInfo>>`
    /// and pushes `scope.keys()`, so it is order-unstable for exactly the same
    /// reason. `count` is distance 1 from both `counts` and `mount`.
    #[test]
    fn undefined_variable_tie_does_not_depend_on_scope_hashmap_order() {
        for perm in permutations(&["counts", "mount", "index"]) {
            let mut helper = TypeErrorHelper::new();
            helper.update_available(perm.clone(), vec![], vec![]);

            assert_eq!(
                helper.undefined_variable("count", None).to_string(),
                "Undefined variable: 'count'. Did you mean 'counts'?",
                "variable suggestion changed with candidate order {:?}",
                perm
            );
        }
    }

    /// The third untriggered call site, reaching the
    /// `Struct '{}' has no field '{}'. Did you mean '{}'?` message. The field
    /// list comes from `Checker::structs`, another `HashMap`.
    ///
    /// The expected answer here is `next`, not `texts`: both are edit-distance
    /// 1 from `text` and the tie-break is lexicographic. That is the honest
    /// contract — it guarantees the *same* suggestion every run, not the
    /// subjectively better one, and there is no ordering in which the checker
    /// could have known which of two equidistant names was meant.
    #[test]
    fn invalid_field_access_tie_does_not_depend_on_struct_hashmap_order() {
        let helper = TypeErrorHelper::new();

        for perm in permutations(&["texts", "next", "span"]) {
            assert_eq!(
                helper
                    .invalid_field_access("Token", "text", &perm)
                    .to_string(),
                "Struct 'Token' has no field 'text'. Did you mean 'next'?",
                "field suggestion changed with candidate order {:?}",
                perm
            );
        }
    }

    /// The import hint still short-circuits ahead of the similar-name search,
    /// in every order — determinism must not have reordered the two branches.
    #[test]
    fn undefined_function_import_hint_still_precedes_the_suggestion() {
        for perm in permutations(&["printline", "print_ln"]) {
            let mut helper = TypeErrorHelper::new();
            helper.update_available(vec![], perm.clone(), vec![]);

            assert_eq!(
                helper.undefined_function("println", None).to_string(),
                "Undefined function: 'println'. Try adding: import std.io;",
                "import hint lost with candidate order {:?}",
                perm
            );
        }
    }

    /// With no candidate inside the threshold the helper must fall through to
    /// the structured variant rather than invent a suggestion to be stable.
    #[test]
    fn undefined_function_without_a_near_candidate_stays_structured() {
        let mut helper = TypeErrorHelper::new();
        helper.update_available(
            vec![],
            vec!["alpha".to_string(), "beta".to_string()],
            vec![],
        );

        let err = helper.undefined_function("completely_unrelated", None);
        assert!(
            matches!(err, CompileError::UndefinedFunction { .. }),
            "expected UndefinedFunction, got {:?}",
            err
        );
    }
}
