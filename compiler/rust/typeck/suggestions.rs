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
