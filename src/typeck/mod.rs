// Type checker for Palladium
// "Ensuring legends are logically sound"

use crate::ast::{AssignTarget, UnaryOp, *};
use crate::errors::{CompileError, Result};
use std::collections::HashMap;

mod suggestions;
use suggestions::TypeErrorHelper;

/// Type representation for type checker (wraps AST Type)
#[derive(Debug, Clone, PartialEq)]
pub enum CheckerType {
    Unit,
    String,
    Int,
    Bool,
    Array(Box<CheckerType>, usize),
    Function(Vec<CheckerType>, Box<CheckerType>),
    Struct(String),
    TypeParam(String),
    Enum(String),
}

impl From<&crate::ast::Type> for CheckerType {
    fn from(ast_type: &crate::ast::Type) -> Self {
        match ast_type {
            crate::ast::Type::Unit => CheckerType::Unit,
            crate::ast::Type::String => CheckerType::String,
            crate::ast::Type::I32 | crate::ast::Type::I64 => CheckerType::Int,
            crate::ast::Type::Bool => CheckerType::Bool,
            crate::ast::Type::U32 | crate::ast::Type::U64 => CheckerType::Int,
            crate::ast::Type::Array(elem_type, size) => {
                CheckerType::Array(Box::new(CheckerType::from(elem_type.as_ref())), *size)
            }
            crate::ast::Type::Custom(name) => CheckerType::Struct(name.clone()),
            crate::ast::Type::TypeParam(name) => {
                // Type parameters need proper handling through substitution
                // For now, create a placeholder type that can be unified later
                CheckerType::TypeParam(name.clone())
            }
            crate::ast::Type::Generic { name, .. } => {
                // For now, treat generic types as their base type
                CheckerType::Struct(name.clone())
            }
        }
    }
}

impl std::fmt::Display for CheckerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckerType::Unit => write!(f, "()"),
            CheckerType::String => write!(f, "String"),
            CheckerType::Int => write!(f, "Int"),
            CheckerType::Bool => write!(f, "Bool"),
            CheckerType::Array(elem_type, size) => write!(f, "[{}; {}]", elem_type, size),
            CheckerType::Function(params, ret) => {
                write!(f, "fn(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", param)?;
                }
                write!(f, ") -> {}", ret)
            }
            CheckerType::Struct(name) => write!(f, "{}", name),
            CheckerType::TypeParam(name) => write!(f, "{}", name),
            CheckerType::Enum(name) => write!(f, "{}", name),
        }
    }
}

/// Variable information including type and mutability
#[derive(Debug, Clone)]
struct VarInfo {
    ty: CheckerType,
    mutable: bool,
}

/// Symbol table for storing variable types with scope support
#[derive(Debug, Clone)]
struct SymbolTable {
    scopes: Vec<HashMap<String, VarInfo>>,
}

impl SymbolTable {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()], // Start with global scope
        }
    }

    /// Enter a new scope
    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Exit the current scope
    fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Define a variable in the current scope
    fn define(&mut self, name: String, ty: CheckerType, mutable: bool) -> Result<()> {
        if let Some(scope) = self.scopes.last_mut() {
            if scope.contains_key(&name) {
                return Err(CompileError::Generic(format!(
                    "Variable '{}' already defined in this scope",
                    name
                )));
            }
            scope.insert(name, VarInfo { ty, mutable });
            Ok(())
        } else {
            Err(CompileError::Generic("No active scope".to_string()))
        }
    }

    /// Look up a variable (searches all scopes from innermost to outermost)
    fn lookup(&self, name: &str) -> Option<&VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }
}

/// Information about a generic function
#[derive(Debug, Clone)]
pub struct GenericFunction {
    pub type_params: Vec<String>,
    pub params: Vec<(String, crate::ast::Type)>,
    pub return_type: Option<crate::ast::Type>,
    pub body: Vec<crate::ast::Stmt>,
}

/// Enum variant information
#[derive(Debug, Clone)]
struct EnumVariant {
    name: String,
    fields: EnumVariantFields,
}

#[derive(Debug, Clone)]
enum EnumVariantFields {
    Unit,
    Tuple(Vec<CheckerType>),
    Named(Vec<(String, CheckerType)>),
}

/// A concrete instantiation of a generic function
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FunctionInstantiation {
    name: String,
    type_args: Vec<String>, // Concrete types like "i64", "String"
}

pub struct TypeChecker {
    /// Function signatures
    functions: HashMap<String, CheckerType>,
    /// Generic function definitions
    generic_functions: HashMap<String, GenericFunction>,
    /// Instantiated generic functions
    instantiations: HashMap<FunctionInstantiation, CheckerType>,
    /// Struct definitions
    structs: HashMap<String, Vec<(String, CheckerType)>>,
    /// Enum definitions with their variants
    enums: HashMap<String, Vec<EnumVariant>>,
    /// Current function return type (for checking return statements)
    current_function_return: Option<CheckerType>,
    /// Symbol table for variables
    symbols: SymbolTable,
    /// Imported modules and their exported items
    imported_modules: HashMap<String, crate::resolver::ModuleInfo>,
    /// Loop depth counter (for break/continue validation)
    loop_depth: usize,
    /// Error helper for better suggestions
    error_helper: TypeErrorHelper,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut functions = HashMap::new();

        // Built-in functions
        functions.insert(
            "print".to_string(),
            CheckerType::Function(vec![CheckerType::String], Box::new(CheckerType::Unit)),
        );

        // print_int built-in function
        functions.insert(
            "print_int".to_string(),
            CheckerType::Function(vec![CheckerType::Int], Box::new(CheckerType::Unit)),
        );

        // String manipulation functions
        functions.insert(
            "string_len".to_string(),
            CheckerType::Function(vec![CheckerType::String], Box::new(CheckerType::Int)),
        );
        functions.insert(
            "string_concat".to_string(),
            CheckerType::Function(
                vec![CheckerType::String, CheckerType::String],
                Box::new(CheckerType::String),
            ),
        );
        functions.insert(
            "string_eq".to_string(),
            CheckerType::Function(
                vec![CheckerType::String, CheckerType::String],
                Box::new(CheckerType::Bool),
            ),
        );
        functions.insert(
            "string_char_at".to_string(),
            CheckerType::Function(
                vec![CheckerType::String, CheckerType::Int],
                Box::new(CheckerType::Int),
            ),
        );
        functions.insert(
            "string_substring".to_string(),
            CheckerType::Function(
                vec![CheckerType::String, CheckerType::Int, CheckerType::Int],
                Box::new(CheckerType::String),
            ),
        );
        functions.insert(
            "string_from_char".to_string(),
            CheckerType::Function(vec![CheckerType::Int], Box::new(CheckerType::String)),
        );
        functions.insert(
            "char_is_digit".to_string(),
            CheckerType::Function(vec![CheckerType::Int], Box::new(CheckerType::Bool)),
        );
        functions.insert(
            "char_is_alpha".to_string(),
            CheckerType::Function(vec![CheckerType::Int], Box::new(CheckerType::Bool)),
        );
        functions.insert(
            "char_is_whitespace".to_string(),
            CheckerType::Function(vec![CheckerType::Int], Box::new(CheckerType::Bool)),
        );
        functions.insert(
            "string_to_int".to_string(),
            CheckerType::Function(vec![CheckerType::String], Box::new(CheckerType::Int)),
        );

        // File I/O functions
        functions.insert(
            "file_open".to_string(),
            CheckerType::Function(vec![CheckerType::String], Box::new(CheckerType::Int)),
        );
        functions.insert(
            "file_read_all".to_string(),
            CheckerType::Function(vec![CheckerType::Int], Box::new(CheckerType::String)),
        );
        functions.insert(
            "file_read_line".to_string(),
            CheckerType::Function(vec![CheckerType::Int], Box::new(CheckerType::String)),
        );
        functions.insert(
            "file_write".to_string(),
            CheckerType::Function(
                vec![CheckerType::Int, CheckerType::String],
                Box::new(CheckerType::Bool),
            ),
        );
        functions.insert(
            "file_close".to_string(),
            CheckerType::Function(vec![CheckerType::Int], Box::new(CheckerType::Bool)),
        );
        functions.insert(
            "file_exists".to_string(),
            CheckerType::Function(vec![CheckerType::String], Box::new(CheckerType::Bool)),
        );

        // String operations
        functions.insert(
            "string_concat".to_string(),
            CheckerType::Function(
                vec![CheckerType::String, CheckerType::String],
                Box::new(CheckerType::String),
            ),
        );
        functions.insert(
            "int_to_string".to_string(),
            CheckerType::Function(vec![CheckerType::Int], Box::new(CheckerType::String)),
        );

        Self {
            functions,
            generic_functions: HashMap::new(),
            instantiations: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            current_function_return: None,
            symbols: SymbolTable::new(),
            imported_modules: HashMap::new(),
            loop_depth: 0,
            error_helper: TypeErrorHelper::new(),
        }
    }

    /// Set imported modules for type checking
    pub fn set_imported_modules(&mut self, modules: HashMap<String, crate::resolver::ModuleInfo>) {
        self.imported_modules = modules;

        // Process imported functions and add them to our function table
        for (module_name, module_info) in &self.imported_modules {
            // For now, process all exported functions from the module
            for item in &module_info.ast.items {
                match item {
                    crate::ast::Item::Function(func) => {
                        // Only process exported (public) functions
                        if matches!(func.visibility, crate::ast::Visibility::Public) {
                            let qualified_name = format!("{}::{}", module_name, func.name);

                            if !func.type_params.is_empty() {
                                // Generic function
                                let generic_func = GenericFunction {
                                    type_params: func.type_params.clone(),
                                    params: func
                                        .params
                                        .iter()
                                        .map(|p| (p.name.clone(), p.ty.clone()))
                                        .collect(),
                                    return_type: func.return_type.clone(),
                                    body: func.body.clone(),
                                };
                                self.generic_functions
                                    .insert(func.name.clone(), generic_func);
                            } else {
                                // Regular function
                                let param_types: Vec<CheckerType> = func
                                    .params
                                    .iter()
                                    .map(|param| CheckerType::from(&param.ty))
                                    .collect();

                                let return_type = func
                                    .return_type
                                    .as_ref()
                                    .map(CheckerType::from)
                                    .unwrap_or(CheckerType::Unit);

                                let func_type =
                                    CheckerType::Function(param_types, Box::new(return_type));

                                // Add both qualified and unqualified names
                                // Note: In a full implementation, we'd use a proper module resolution system
                                self.functions.insert(func.name.clone(), func_type.clone());
                                self.functions.insert(qualified_name, func_type);
                            }
                        }
                    }
                    crate::ast::Item::Struct(struct_def) => {
                        if matches!(struct_def.visibility, crate::ast::Visibility::Public) {
                            // Convert field types to CheckerType
                            let fields: Vec<(String, CheckerType)> = struct_def
                                .fields
                                .iter()
                                .map(|(name, ty)| (name.clone(), CheckerType::from(ty)))
                                .collect();

                            // Add both qualified and unqualified names
                            self.structs.insert(struct_def.name.clone(), fields.clone());
                            self.structs
                                .insert(format!("{}::{}", module_name, struct_def.name), fields);
                        }
                    }
                    crate::ast::Item::Enum(enum_def) => {
                        // Assume all exported enums are public
                        {
                            // Store enum type information
                            let enum_type = CheckerType::Enum(enum_def.name.clone());

                            // Add variant constructors as functions
                            for variant in &enum_def.variants {
                                let variant_name = format!("{}::{}", enum_def.name, variant.name);
                                let qualified_variant =
                                    format!("{}::{}", module_name, variant_name);

                                // Create constructor function type based on variant fields
                                let func_type = match &variant.data {
                                    crate::ast::EnumVariantData::Unit => {
                                        // Unit variant: no parameters, returns enum type
                                        CheckerType::Function(vec![], Box::new(enum_type.clone()))
                                    }
                                    crate::ast::EnumVariantData::Tuple(types) => {
                                        // Tuple variant: parameters from tuple fields
                                        let param_types: Vec<CheckerType> =
                                            types.iter().map(CheckerType::from).collect();
                                        CheckerType::Function(
                                            param_types,
                                            Box::new(enum_type.clone()),
                                        )
                                    }
                                    crate::ast::EnumVariantData::Struct(fields) => {
                                        // Named variant: parameters from named fields
                                        let param_types: Vec<CheckerType> = fields
                                            .iter()
                                            .map(|(_, ty)| CheckerType::from(ty))
                                            .collect();
                                        CheckerType::Function(
                                            param_types,
                                            Box::new(enum_type.clone()),
                                        )
                                    }
                                };

                                // Register variant constructors
                                self.functions
                                    .insert(variant_name.clone(), func_type.clone());
                                self.functions.insert(qualified_variant, func_type);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Type check a program
    pub fn check(&mut self, program: &Program) -> Result<()> {
        // First pass: collect all function signatures and struct definitions
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    if !func.type_params.is_empty() {
                        // This is a generic function - store it for later instantiation
                        let generic_func = GenericFunction {
                            type_params: func.type_params.clone(),
                            params: func
                                .params
                                .iter()
                                .map(|p| (p.name.clone(), p.ty.clone()))
                                .collect(),
                            return_type: func.return_type.clone(),
                            body: func.body.clone(),
                        };
                        self.generic_functions
                            .insert(func.name.clone(), generic_func);
                    } else {
                        // Regular function - process as before
                        let param_types: Vec<CheckerType> = func
                            .params
                            .iter()
                            .map(|param| CheckerType::from(&param.ty))
                            .collect();

                        let return_type = func
                            .return_type
                            .as_ref()
                            .map(CheckerType::from)
                            .unwrap_or(CheckerType::Unit);

                        let func_type = CheckerType::Function(param_types, Box::new(return_type));
                        self.functions.insert(func.name.clone(), func_type);
                    }
                }
                Item::Struct(struct_def) => {
                    // Convert field types to CheckerType
                    let fields: Vec<(String, CheckerType)> = struct_def
                        .fields
                        .iter()
                        .map(|(name, ty)| (name.clone(), CheckerType::from(ty)))
                        .collect();

                    self.structs.insert(struct_def.name.clone(), fields);
                }
                Item::Enum(enum_def) => {
                    // Store enum variants for type checking
                    let mut variants = Vec::new();

                    for variant in &enum_def.variants {
                        let variant_fields = match &variant.data {
                            crate::ast::EnumVariantData::Unit => EnumVariantFields::Unit,
                            crate::ast::EnumVariantData::Tuple(types) => {
                                let field_types: Vec<CheckerType> =
                                    types.iter().map(CheckerType::from).collect();
                                EnumVariantFields::Tuple(field_types)
                            }
                            crate::ast::EnumVariantData::Struct(fields) => {
                                let named_fields: Vec<(String, CheckerType)> = fields
                                    .iter()
                                    .map(|(name, ty)| (name.clone(), CheckerType::from(ty)))
                                    .collect();
                                EnumVariantFields::Named(named_fields)
                            }
                        };

                        variants.push(EnumVariant {
                            name: variant.name.clone(),
                            fields: variant_fields,
                        });

                        // Also register variant constructors as functions
                        let enum_type = CheckerType::Enum(enum_def.name.clone());
                        let variant_name = format!("{}::{}", enum_def.name, variant.name);

                        let func_type = match &variant.data {
                            crate::ast::EnumVariantData::Unit => {
                                CheckerType::Function(vec![], Box::new(enum_type.clone()))
                            }
                            crate::ast::EnumVariantData::Tuple(types) => {
                                let param_types: Vec<CheckerType> =
                                    types.iter().map(CheckerType::from).collect();
                                CheckerType::Function(param_types, Box::new(enum_type.clone()))
                            }
                            crate::ast::EnumVariantData::Struct(fields) => {
                                let param_types: Vec<CheckerType> =
                                    fields.iter().map(|(_, ty)| CheckerType::from(ty)).collect();
                                CheckerType::Function(param_types, Box::new(enum_type.clone()))
                            }
                        };

                        self.functions.insert(variant_name, func_type);
                    }

                    self.enums.insert(enum_def.name.clone(), variants);
                }
            }
        }

        // Check for main function
        if !self.functions.contains_key("main") {
            return Err(TypeErrorHelper::missing_main());
        }

        // Second pass: type check function bodies
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.check_function(func)?;
                }
                Item::Struct(_) => {
                    // Structs are already processed in the first pass
                }
                Item::Enum(_) => {
                    // Enums are already processed in the first pass
                }
            }
        }

        Ok(())
    }

    /// Type check a function
    fn check_function(&mut self, func: &Function) -> Result<()> {
        // Skip generic functions - they'll be checked when instantiated
        if !func.type_params.is_empty() {
            return Ok(());
        }

        // Enter function scope
        self.symbols.enter_scope();

        // Add function parameters to symbol table
        for param in &func.params {
            let checker_type = CheckerType::from(&param.ty);
            self.symbols
                .define(param.name.clone(), checker_type, param.mutable)?;
        }

        // Set current function return type
        let return_type = func
            .return_type
            .as_ref()
            .map(CheckerType::from)
            .unwrap_or(CheckerType::Unit);
        self.current_function_return = Some(return_type);

        // Type check each statement in the body
        for stmt in &func.body {
            self.check_statement(stmt)?;
        }

        // Exit function scope
        self.symbols.exit_scope();
        self.current_function_return = None;
        Ok(())
    }

    /// Type check a statement
    fn check_statement(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Expr(expr) => {
                self.check_expression(expr)?;
                Ok(())
            }
            Stmt::Return(None) => {
                // Returning nothing is Unit type
                if self.current_function_return != Some(CheckerType::Unit) {
                    return Err(CompileError::TypeMismatch {
                        expected: "()".to_string(),
                        found: "return value".to_string(),
                        span: None,
                    });
                }
                Ok(())
            }
            Stmt::Return(Some(expr)) => {
                let expr_type = self.check_expression(expr)?;
                if let Some(expected) = &self.current_function_return {
                    if expr_type != *expected {
                        return Err(CompileError::TypeMismatch {
                            expected: expected.to_string(),
                            found: expr_type.to_string(),
                            span: None,
                        });
                    }
                }
                Ok(())
            }
            Stmt::Let {
                name,
                ty,
                value,
                mutable,
                ..
            } => {
                // Type check the value expression
                let value_type = self.check_expression(value)?;

                // If type annotation is provided, check that it matches
                if let Some(annotated_type) = ty {
                    let expected_type = CheckerType::from(annotated_type);
                    if value_type != expected_type {
                        return Err(self.error_helper.type_mismatch(
                            &expected_type.to_string(),
                            &value_type.to_string(),
                            None,
                        ));
                    }
                    // Define variable with annotated type
                    self.symbols.define(name.clone(), expected_type, *mutable)?;
                } else {
                    // Define variable with inferred type
                    self.symbols.define(name.clone(), value_type, *mutable)?;
                }

                Ok(())
            }
            Stmt::Assign { target, value, .. } => {
                match target {
                    AssignTarget::Ident(name) => {
                        // Look up the variable and clone necessary info
                        let (var_type, var_mutable) = {
                            match self.symbols.lookup(name) {
                                Some(var_info) => (var_info.ty.clone(), var_info.mutable),
                                None => {
                                    // Update error helper with available variables
                                    let available_vars = self.get_available_variables();
                                    self.error_helper.update_available(
                                        available_vars,
                                        vec![],
                                        vec![],
                                    );
                                    return Err(self.error_helper.undefined_variable(name, None));
                                }
                            }
                        };

                        // Check if variable is mutable
                        if !var_mutable {
                            return Err(self.error_helper.immutable_assignment(name));
                        }

                        // Type check the value expression
                        let value_type = self.check_expression(value)?;

                        // Check that types match
                        if value_type != var_type {
                            return Err(self.error_helper.type_mismatch(
                                &var_type.to_string(),
                                &value_type.to_string(),
                                None,
                            ));
                        }

                        Ok(())
                    }
                    AssignTarget::Index { array, index } => {
                        // Type check the array expression
                        let array_type = self.check_expression(array)?;

                        // Type check the index expression (must be Int)
                        let index_type = self.check_expression(index)?;
                        if index_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "Int".to_string(),
                                found: index_type.to_string(),
                                span: None,
                            });
                        }

                        // Extract element type from array type
                        let elem_type = match array_type {
                            CheckerType::Array(elem_type, _size) => elem_type.as_ref().clone(),
                            _ => {
                                return Err(CompileError::Generic(format!(
                                    "Cannot index into non-array type: {}",
                                    array_type
                                )));
                            }
                        };

                        // Type check the value expression
                        let value_type = self.check_expression(value)?;

                        // Check that types match
                        if value_type != elem_type {
                            return Err(CompileError::TypeMismatch {
                                expected: elem_type.to_string(),
                                found: value_type.to_string(),
                                span: None,
                            });
                        }

                        Ok(())
                    }
                    AssignTarget::FieldAccess { object, field } => {
                        // Type check the object expression
                        let object_type = self.check_expression(object)?;

                        // Get the struct name and check if it exists
                        let struct_name = match object_type {
                            CheckerType::Struct(name) => name,
                            _ => {
                                return Err(CompileError::Generic(format!(
                                    "Cannot access field on non-struct type: {}",
                                    object_type
                                )));
                            }
                        };

                        // Look up the struct fields
                        let fields = self.structs.get(&struct_name).ok_or_else(|| {
                            CompileError::Generic(format!("Unknown struct type: {}", struct_name))
                        })?;

                        // Find the field type
                        let field_type = fields
                            .iter()
                            .find(|(fname, _)| fname == field)
                            .map(|(_, ftype)| ftype.clone())
                            .ok_or_else(|| {
                                CompileError::Generic(format!(
                                    "Struct '{}' has no field '{}'",
                                    struct_name, field
                                ))
                            })?;

                        // Type check the value expression
                        let value_type = self.check_expression(value)?;

                        // Check that types match
                        if value_type != field_type {
                            return Err(CompileError::TypeMismatch {
                                expected: field_type.to_string(),
                                found: value_type.to_string(),
                                span: None,
                            });
                        }

                        Ok(())
                    }
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                // Type check the condition - must be Bool
                let cond_type = self.check_expression(condition)?;
                if cond_type != CheckerType::Bool {
                    return Err(CompileError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: cond_type.to_string(),
                        span: None,
                    });
                }

                // Type check then branch in new scope
                self.symbols.enter_scope();
                for stmt in then_branch {
                    self.check_statement(stmt)?;
                }
                self.symbols.exit_scope();

                // Type check else branch in new scope if it exists
                if let Some(else_stmts) = else_branch {
                    self.symbols.enter_scope();
                    for stmt in else_stmts {
                        self.check_statement(stmt)?;
                    }
                    self.symbols.exit_scope();
                }

                Ok(())
            }
            Stmt::While {
                condition, body, ..
            } => {
                // Type check the condition - must be Bool
                let cond_type = self.check_expression(condition)?;
                if cond_type != CheckerType::Bool {
                    return Err(CompileError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: cond_type.to_string(),
                        span: None,
                    });
                }

                // Type check body in new scope with incremented loop depth
                self.symbols.enter_scope();
                self.loop_depth += 1;
                for stmt in body {
                    self.check_statement(stmt)?;
                }
                self.loop_depth -= 1;
                self.symbols.exit_scope();

                Ok(())
            }
            Stmt::For {
                var, iter, body, ..
            } => {
                // Type check the iterator expression
                let iter_type = self.check_expression(iter)?;

                // Extract element type from array
                let elem_type = match iter_type {
                    CheckerType::Array(elem_type, _size) => elem_type.as_ref().clone(),
                    _ => {
                        return Err(CompileError::Generic(format!(
                            "For loop requires an array, found {}",
                            iter_type
                        )));
                    }
                };

                // Enter new scope for loop body
                self.symbols.enter_scope();
                self.loop_depth += 1;

                // Define loop variable with element type
                self.symbols.define(var.clone(), elem_type, false)?;

                // Type check body
                for stmt in body {
                    self.check_statement(stmt)?;
                }

                self.loop_depth -= 1;
                self.symbols.exit_scope();

                Ok(())
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {
                // Check that we're inside a loop
                if self.loop_depth == 0 {
                    let keyword = if matches!(stmt, Stmt::Break { .. }) {
                        "break"
                    } else {
                        "continue"
                    };
                    return Err(self.error_helper.control_flow_outside_loop(keyword));
                }
                Ok(())
            }
            Stmt::Match { expr, arms, .. } => {
                // Type check the match expression
                let expr_type = self.check_expression(expr)?;

                // For each arm, check the pattern matches the expression type
                // and type check the body
                for arm in arms {
                    // Check pattern compatibility with expression type
                    self.check_pattern(&arm.pattern, &expr_type)?;

                    // Type check arm body in new scope
                    self.symbols.enter_scope();

                    // Bind pattern variables if any
                    self.bind_pattern_variables(&arm.pattern, &expr_type)?;

                    for stmt in &arm.body {
                        self.check_statement(stmt)?;
                    }

                    self.symbols.exit_scope();
                }

                // Pattern exhaustiveness checking
                // Note: Full exhaustiveness checking would require tracking all patterns
                // and ensuring they cover all possible values of the matched type.
                // For now, we rely on the presence of wildcard patterns or complete coverage.

                Ok(())
            }
        }
    }

    /// Type check an expression and return its type
    fn check_expression(&mut self, expr: &Expr) -> Result<CheckerType> {
        match expr {
            Expr::String(_) => Ok(CheckerType::String),
            Expr::Integer(_) => Ok(CheckerType::Int),
            Expr::Bool(_) => Ok(CheckerType::Bool),
            Expr::Ident(name) => {
                // First check if it's a variable
                if let Some(var_info) = self.symbols.lookup(name) {
                    return Ok(var_info.ty.clone());
                }

                // Then check if it's a function
                match self.functions.get(name) {
                    Some(func_type) => Ok(func_type.clone()),
                    None => {
                        // Try to provide helpful suggestions
                        let available_vars = self.get_available_variables();
                        let available_funcs = self.get_available_functions();

                        // Check if it might be a typo for a variable
                        if let Some(suggestion) =
                            crate::errors::suggestions::SuggestionEngine::suggest_similar_name(
                                name,
                                &available_vars,
                            )
                        {
                            return Err(CompileError::Generic(format!(
                                "Undefined variable: '{}'. Did you mean '{}'?",
                                name, suggestion
                            )));
                        }

                        // Check if it might be a typo for a function
                        if let Some(suggestion) =
                            crate::errors::suggestions::SuggestionEngine::suggest_similar_name(
                                name,
                                &available_funcs,
                            )
                        {
                            return Err(CompileError::Generic(format!(
                                "Undefined function: '{}'. Did you mean '{}'?",
                                name, suggestion
                            )));
                        }

                        // No good suggestion found
                        Err(CompileError::Generic(format!(
                            "Undefined variable or function: '{}'",
                            name
                        )))
                    }
                }
            }
            Expr::Call { func, args, .. } => {
                // Get function name (for v0.1, only direct calls)
                let func_name = match func.as_ref() {
                    Expr::Ident(name) => name,
                    _ => {
                        return Err(CompileError::Generic(
                            "Indirect function calls not yet supported".to_string(),
                        ))
                    }
                };

                // First check if it's a generic function that needs instantiation
                if let Some(generic_func) = self.generic_functions.get(func_name).cloned() {
                    // Infer type arguments from the call
                    let type_args = self.infer_type_args(&generic_func, args)?;

                    // Create instantiation key
                    let instantiation = FunctionInstantiation {
                        name: func_name.clone(),
                        type_args: type_args.clone(),
                    };

                    // Check if we've already instantiated this combination
                    if let Some(func_type) = self.instantiations.get(&instantiation) {
                        return self.check_call_with_type(func_name, func_type.clone(), args);
                    }

                    // Need to instantiate the generic function
                    let func_type = self.instantiate_generic_function(&generic_func, &type_args)?;
                    self.instantiations.insert(instantiation, func_type.clone());

                    return self.check_call_with_type(func_name, func_type, args);
                }

                // Look up regular function type
                let func_type = match self.functions.get(func_name) {
                    Some(ft) => ft.clone(),
                    None => {
                        // Update error helper with available functions
                        let available_funcs = self.get_available_functions();
                        self.error_helper
                            .update_available(vec![], available_funcs, vec![]);
                        return Err(self.error_helper.undefined_function(func_name, None));
                    }
                };

                // Check function type
                match func_type {
                    CheckerType::Function(param_types, return_type) => {
                        // Check argument count
                        if args.len() != param_types.len() {
                            return Err(CompileError::ArgumentCountMismatch {
                                name: func_name.clone(),
                                expected: param_types.len(),
                                found: args.len(),
                                span: None,
                            });
                        }

                        // Check argument types
                        for (arg, expected_type) in args.iter().zip(param_types.iter()) {
                            let arg_type = self.check_expression(arg)?;
                            if arg_type != *expected_type {
                                return Err(CompileError::TypeMismatch {
                                    expected: expected_type.to_string(),
                                    found: arg_type.to_string(),
                                    span: None,
                                });
                            }
                        }

                        Ok(return_type.as_ref().clone())
                    }
                    _ => Err(CompileError::Generic(format!(
                        "{} is not a function",
                        func_name
                    ))),
                }
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                let left_type = self.check_expression(left)?;
                let right_type = self.check_expression(right)?;

                match op {
                    BinOp::Add => {
                        // Addition can work for both Int and String (concatenation)
                        match (&left_type, &right_type) {
                            (CheckerType::Int, CheckerType::Int) => Ok(CheckerType::Int),
                            (CheckerType::String, CheckerType::String) => Ok(CheckerType::String),
                            _ => {
                                // For Add, we expect both operands to have the same type
                                if left_type == CheckerType::String {
                                    Err(CompileError::TypeMismatch {
                                        expected: "String".to_string(),
                                        found: right_type.to_string(),
                                        span: None,
                                    })
                                } else if left_type == CheckerType::Int {
                                    Err(CompileError::TypeMismatch {
                                        expected: "Int".to_string(),
                                        found: right_type.to_string(),
                                        span: None,
                                    })
                                } else {
                                    Err(CompileError::TypeMismatch {
                                        expected: "Int or String".to_string(),
                                        found: left_type.to_string(),
                                        span: None,
                                    })
                                }
                            }
                        }
                    }
                    BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                        // Other arithmetic operations require both operands to be Int
                        if left_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "Int".to_string(),
                                found: left_type.to_string(),
                                span: None,
                            });
                        }
                        if right_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "Int".to_string(),
                                found: right_type.to_string(),
                                span: None,
                            });
                        }
                        Ok(CheckerType::Int)
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                        // Comparison operations require same types
                        if left_type != right_type {
                            return Err(CompileError::TypeMismatch {
                                expected: left_type.to_string(),
                                found: right_type.to_string(),
                                span: None,
                            });
                        }
                        // Comparison operations return Bool
                        Ok(CheckerType::Bool)
                    }
                    BinOp::And | BinOp::Or => {
                        // Logical operations require both operands to be Bool
                        if left_type != CheckerType::Bool {
                            return Err(CompileError::TypeMismatch {
                                expected: "Bool".to_string(),
                                found: left_type.to_string(),
                                span: None,
                            });
                        }
                        if right_type != CheckerType::Bool {
                            return Err(CompileError::TypeMismatch {
                                expected: "Bool".to_string(),
                                found: right_type.to_string(),
                                span: None,
                            });
                        }
                        Ok(CheckerType::Bool)
                    }
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                if elements.is_empty() {
                    return Err(CompileError::Generic(
                        "Empty array literals are not supported (cannot infer type)".to_string(),
                    ));
                }

                // Type check first element
                let elem_type = self.check_expression(&elements[0])?;

                // Check that all elements have the same type
                for elem in &elements[1..] {
                    let elem_expr_type = self.check_expression(elem)?;
                    if elem_expr_type != elem_type {
                        return Err(CompileError::TypeMismatch {
                            expected: elem_type.to_string(),
                            found: elem_expr_type.to_string(),
                            span: None,
                        });
                    }
                }

                Ok(CheckerType::Array(Box::new(elem_type), elements.len()))
            }
            Expr::ArrayRepeat { value, count, .. } => {
                // Type check the value
                let elem_type = self.check_expression(value)?;

                // Type check the count - must be an integer literal
                match count.as_ref() {
                    Expr::Integer(n) => {
                        if *n < 0 {
                            return Err(CompileError::Generic(
                                "Array size must be non-negative".to_string(),
                            ));
                        }
                        Ok(CheckerType::Array(Box::new(elem_type), *n as usize))
                    }
                    _ => Err(CompileError::Generic(
                        "Array repeat count must be an integer literal".to_string(),
                    )),
                }
            }
            Expr::Index { array, index, .. } => {
                // Type check the array expression
                let array_type = self.check_expression(array)?;

                // Type check the index expression (must be Int)
                let index_type = self.check_expression(index)?;
                if index_type != CheckerType::Int {
                    return Err(CompileError::TypeMismatch {
                        expected: "Int".to_string(),
                        found: index_type.to_string(),
                        span: None,
                    });
                }

                // Extract element type from array type
                match array_type {
                    CheckerType::Array(elem_type, _size) => Ok(elem_type.as_ref().clone()),
                    _ => Err(CompileError::Generic(format!(
                        "Cannot index into non-array type: {}",
                        array_type
                    ))),
                }
            }
            Expr::StructLiteral { name, fields, .. } => {
                // Look up the struct definition
                let struct_fields = self
                    .structs
                    .get(name)
                    .ok_or_else(|| CompileError::Generic(format!("Unknown struct type: {}", name)))?
                    .clone();

                // Check that all fields are provided and have correct types
                for (field_name, field_type) in &struct_fields {
                    let provided_expr = fields
                        .iter()
                        .find(|(fname, _)| fname == field_name)
                        .map(|(_, expr)| expr)
                        .ok_or_else(|| {
                            CompileError::Generic(format!(
                                "Missing field '{}' in struct literal",
                                field_name
                            ))
                        })?;

                    let provided_type = self.check_expression(provided_expr)?;
                    if provided_type != *field_type {
                        return Err(CompileError::TypeMismatch {
                            expected: field_type.to_string(),
                            found: provided_type.to_string(),
                            span: None,
                        });
                    }
                }

                // Check that no extra fields are provided
                for (provided_name, _) in fields {
                    if !struct_fields
                        .iter()
                        .any(|(fname, _)| fname == provided_name)
                    {
                        return Err(CompileError::Generic(format!(
                            "Unknown field '{}' for struct '{}'",
                            provided_name, name
                        )));
                    }
                }

                Ok(CheckerType::Struct(name.clone()))
            }
            Expr::FieldAccess { object, field, .. } => {
                // Type check the object expression
                let object_type = self.check_expression(object)?;

                // Get the struct name and check if it exists
                let struct_name = match object_type {
                    CheckerType::Struct(name) => name,
                    _ => {
                        return Err(CompileError::Generic(format!(
                            "Cannot access field on non-struct type: {}",
                            object_type
                        )));
                    }
                };

                // Look up the struct fields
                let fields = self.structs.get(&struct_name).ok_or_else(|| {
                    CompileError::Generic(format!("Unknown struct type: {}", struct_name))
                })?;

                // Find the field type
                let field_type = fields
                    .iter()
                    .find(|(fname, _)| fname == field)
                    .map(|(_, ftype)| ftype.clone())
                    .ok_or_else(|| {
                        CompileError::Generic(format!(
                            "Struct '{}' has no field '{}'",
                            struct_name, field
                        ))
                    })?;

                Ok(field_type)
            }
            Expr::EnumConstructor {
                enum_name,
                variant,
                data,
                ..
            } => {
                // Type check enum constructors
                // First check if the enum exists
                if !self.enums.contains_key(enum_name) {
                    return Err(CompileError::Generic(format!(
                        "Undefined enum type: {}",
                        enum_name
                    )));
                }

                // Find the variant
                let variant_info = self.enums[enum_name]
                    .iter()
                    .find(|v| &v.name == variant)
                    .cloned()
                    .ok_or_else(|| {
                        CompileError::Generic(format!("Unknown variant {}::{}", enum_name, variant))
                    })?;

                // Type check the constructor data based on variant fields
                match (&variant_info.fields, data.as_ref()) {
                    (EnumVariantFields::Unit, None) => {
                        // Unit variant with no data - correct
                    }
                    (EnumVariantFields::Unit, Some(_)) => {
                        // Unit variants shouldn't have constructor data
                        return Err(CompileError::Generic(format!(
                            "Unit variant {}::{} cannot have constructor data",
                            enum_name, variant
                        )));
                    }
                    (
                        EnumVariantFields::Tuple(expected_types),
                        Some(crate::ast::EnumConstructorData::Tuple(exprs)),
                    ) => {
                        // Check tuple constructor
                        if expected_types.len() != exprs.len() {
                            return Err(CompileError::Generic(format!(
                                "Wrong number of arguments for {}::{}: expected {}, found {}",
                                enum_name,
                                variant,
                                expected_types.len(),
                                exprs.len()
                            )));
                        }

                        // Type check each expression
                        for (expected, expr) in expected_types.iter().zip(exprs) {
                            let expr_type = self.check_expression(expr)?;
                            if &expr_type != expected {
                                return Err(CompileError::TypeMismatch {
                                    expected: expected.to_string(),
                                    found: expr_type.to_string(),
                                    span: None,
                                });
                            }
                        }
                    }
                    (
                        EnumVariantFields::Named(expected_fields),
                        Some(crate::ast::EnumConstructorData::Struct(field_exprs)),
                    ) => {
                        // Check named constructor
                        if expected_fields.len() != field_exprs.len() {
                            return Err(CompileError::Generic(format!(
                                "Wrong number of fields for {}::{}: expected {}, found {}",
                                enum_name,
                                variant,
                                expected_fields.len(),
                                field_exprs.len()
                            )));
                        }

                        // Type check each field
                        for (field_name, expr) in field_exprs {
                            let expected_type = expected_fields
                                .iter()
                                .find(|(name, _)| name == field_name)
                                .map(|(_, ty)| ty)
                                .ok_or_else(|| {
                                    CompileError::Generic(format!(
                                        "Unknown field {} in {}::{}",
                                        field_name, enum_name, variant
                                    ))
                                })?;

                            let expr_type = self.check_expression(expr)?;
                            if &expr_type != expected_type {
                                return Err(CompileError::TypeMismatch {
                                    expected: expected_type.to_string(),
                                    found: expr_type.to_string(),
                                    span: None,
                                });
                            }
                        }
                    }
                    _ => {
                        return Err(CompileError::Generic(format!(
                            "Mismatched constructor style for {}::{}",
                            enum_name, variant
                        )));
                    }
                }

                Ok(CheckerType::Enum(enum_name.clone()))
            }
            Expr::Range { start, end, .. } => {
                // Type check start and end expressions
                let start_type = self.check_expression(start)?;
                let end_type = self.check_expression(end)?;

                // Both must be integers
                if start_type != CheckerType::Int {
                    return Err(CompileError::TypeMismatch {
                        expected: "Int".to_string(),
                        found: start_type.to_string(),
                        span: None,
                    });
                }
                if end_type != CheckerType::Int {
                    return Err(CompileError::TypeMismatch {
                        expected: "Int".to_string(),
                        found: end_type.to_string(),
                        span: None,
                    });
                }

                // Range expressions have a special internal type
                // For now, we'll treat them as arrays when used in for loops
                Ok(CheckerType::Array(Box::new(CheckerType::Int), 0))
            }
            Expr::Unary { op, operand, .. } => {
                let operand_type = self.check_expression(operand)?;

                match op {
                    UnaryOp::Neg => {
                        // Negation requires operand to be Int
                        if operand_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "Int".to_string(),
                                found: operand_type.to_string(),
                                span: None,
                            });
                        }
                        Ok(CheckerType::Int)
                    }
                    UnaryOp::Not => {
                        // Logical not requires operand to be Bool
                        if operand_type != CheckerType::Bool {
                            return Err(CompileError::TypeMismatch {
                                expected: "Bool".to_string(),
                                found: operand_type.to_string(),
                                span: None,
                            });
                        }
                        Ok(CheckerType::Bool)
                    }
                }
            }
        }
    }

    /// Check that a pattern is compatible with the given type
    fn check_pattern(&self, pattern: &Pattern, expected_type: &CheckerType) -> Result<()> {
        match pattern {
            Pattern::Wildcard => {
                // Wildcard matches any type
                Ok(())
            }
            Pattern::Ident(_) => {
                // Identifier pattern matches any type and binds it
                Ok(())
            }
            Pattern::EnumPattern {
                enum_name,
                variant: _,
                data: _,
            } => {
                // Check that the expected type matches the enum
                match expected_type {
                    CheckerType::Enum(name) if name == enum_name => Ok(()),
                    _ => Err(CompileError::TypeMismatch {
                        expected: format!("enum {}", enum_name),
                        found: expected_type.to_string(),
                        span: None,
                    }),
                }
            }
        }
    }

    /// Bind variables from patterns to the symbol table
    fn bind_pattern_variables(
        &mut self,
        pattern: &Pattern,
        value_type: &CheckerType,
    ) -> Result<()> {
        match pattern {
            Pattern::Wildcard => {
                // No bindings
                Ok(())
            }
            Pattern::Ident(name) => {
                // Bind the identifier to the value type
                self.symbols.define(
                    name.clone(),
                    value_type.clone(),
                    false, // Pattern bindings are immutable by default
                )?;
                Ok(())
            }
            Pattern::EnumPattern {
                enum_name,
                variant,
                data,
                ..
            } => {
                // Bind variables from nested patterns
                if let Some(pattern_data) = data {
                    // Get enum variant info to determine field types
                    if let CheckerType::Enum(expected_enum) = value_type {
                        if expected_enum != enum_name {
                            return Err(CompileError::TypeMismatch {
                                expected: expected_enum.clone(),
                                found: enum_name.clone(),
                                span: None,
                            });
                        }

                        // Find the variant
                        let variants = self.enums.get(enum_name).ok_or_else(|| {
                            CompileError::Generic(format!("Undefined enum type: {}", enum_name))
                        })?;

                        let variant_info = variants
                            .iter()
                            .find(|v| &v.name == variant)
                            .ok_or_else(|| {
                                CompileError::Generic(format!(
                                    "Unknown variant {}::{}",
                                    enum_name, variant
                                ))
                            })?;

                        match (pattern_data, &variant_info.fields) {
                            (
                                PatternData::Tuple(patterns),
                                EnumVariantFields::Tuple(field_types),
                            ) => {
                                // Bind each tuple pattern with its corresponding type
                                if patterns.len() != field_types.len() {
                                    return Err(CompileError::Generic(format!(
                                        "Pattern has wrong number of fields for {}::{}",
                                        enum_name, variant
                                    )));
                                }

                                for (pattern, field_type) in patterns.iter().zip(field_types) {
                                    self.check_pattern(pattern, field_type)?;
                                }
                            }
                            (
                                PatternData::Struct(field_patterns),
                                EnumVariantFields::Named(expected_fields),
                            ) => {
                                // Bind each struct field pattern with its type
                                for (field_name, pattern) in field_patterns {
                                    let field_type = expected_fields
                                        .iter()
                                        .find(|(name, _)| name == field_name)
                                        .map(|(_, ty)| ty)
                                        .ok_or_else(|| {
                                            CompileError::Generic(format!(
                                                "Unknown field {} in {}::{}",
                                                field_name, enum_name, variant
                                            ))
                                        })?;

                                    self.check_pattern(pattern, field_type)?;
                                }
                            }
                            _ => {
                                return Err(CompileError::Generic(format!(
                                    "Pattern structure doesn't match variant {}::{}",
                                    enum_name, variant
                                )));
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }

    /// Infer type arguments for a generic function call
    fn infer_type_args(
        &self,
        generic_func: &GenericFunction,
        args: &[Expr],
    ) -> Result<Vec<String>> {
        let mut type_map: HashMap<String, String> = HashMap::new();

        // Check argument count
        if args.len() != generic_func.params.len() {
            return Err(CompileError::Generic(format!(
                "Function expects {} arguments, got {}",
                generic_func.params.len(),
                args.len()
            )));
        }

        // Infer types from each argument
        for (arg_expr, (_param_name, param_type)) in args.iter().zip(&generic_func.params) {
            self.infer_from_expr_and_type(arg_expr, param_type, &mut type_map)?;
        }

        // Make sure all type parameters were inferred
        let mut type_args = Vec::new();
        for type_param in &generic_func.type_params {
            match type_map.get(type_param) {
                Some(concrete_type) => type_args.push(concrete_type.clone()),
                None => {
                    return Err(CompileError::Generic(format!(
                        "Could not infer type parameter '{}' from function arguments",
                        type_param
                    )));
                }
            }
        }

        Ok(type_args)
    }

    /// Helper to infer type parameters from an expression and expected type
    fn infer_from_expr_and_type(
        &self,
        expr: &Expr,
        expected_type: &crate::ast::Type,
        type_map: &mut HashMap<String, String>,
    ) -> Result<()> {
        match expected_type {
            crate::ast::Type::TypeParam(param_name) => {
                // This is a type parameter - infer its type from the expression
                let expr_type = self.infer_expr_type(expr)?;

                // Check if we already have a mapping for this type parameter
                if let Some(existing_type) = type_map.get(param_name) {
                    if existing_type != &expr_type {
                        return Err(CompileError::Generic(format!(
                            "Type parameter '{}' has conflicting types: '{}' and '{}'",
                            param_name, existing_type, expr_type
                        )));
                    }
                } else {
                    type_map.insert(param_name.clone(), expr_type);
                }
                Ok(())
            }
            crate::ast::Type::Array(elem_type, _size) => {
                // For arrays, we need to infer the element type
                match expr {
                    Expr::ArrayLiteral { elements, .. } => {
                        if !elements.is_empty() {
                            // Use first element to infer the type parameter
                            self.infer_from_expr_and_type(&elements[0], elem_type, type_map)?;
                        }
                    }
                    Expr::ArrayRepeat { value, .. } => {
                        // Use the repeated value to infer the type parameter
                        self.infer_from_expr_and_type(value, elem_type, type_map)?;
                    }
                    Expr::Ident(name) => {
                        // For identifiers, we need to look up their type and extract element type
                        if let Some(var_info) = self.symbols.lookup(name) {
                            if let CheckerType::Array(var_elem_type, _) = &var_info.ty {
                                // If elem_type is a type parameter, map it
                                if let crate::ast::Type::TypeParam(param_name) = elem_type.as_ref()
                                {
                                    let elem_type_str = self.checker_type_to_string(var_elem_type);
                                    type_map.insert(param_name.clone(), elem_type_str);
                                }
                            }
                        }
                    }
                    _ => {
                        // For other expressions, try to infer their type
                        if let crate::ast::Type::TypeParam(_) = elem_type.as_ref() {
                            self.infer_from_expr_and_type(expr, elem_type, type_map)?;
                        }
                    }
                }
                Ok(())
            }
            _ => {
                // Non-generic type - nothing to infer
                Ok(())
            }
        }
    }

    /// Get a string representation of the expression's type for inference
    fn infer_expr_type(&self, expr: &Expr) -> Result<String> {
        match expr {
            Expr::String(_) => Ok("String".to_string()),
            Expr::Integer(_) => Ok("i64".to_string()), // Default to i64
            Expr::Bool(_) => Ok("bool".to_string()),
            Expr::Ident(name) => {
                // Look up variable type
                if let Some(var_info) = self.symbols.lookup(name) {
                    Ok(self.checker_type_to_string(&var_info.ty))
                } else {
                    Err(CompileError::Generic(format!("Unknown variable: {}", name)))
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                // Infer array type from elements
                if elements.is_empty() {
                    return Err(CompileError::Generic(
                        "Cannot infer type from empty array".to_string(),
                    ));
                }

                // Get type of first element (assume all elements have same type)
                let elem_type_str = self.infer_expr_type(&elements[0])?;
                let size = elements.len();
                Ok(format!("[{}; {}]", elem_type_str, size))
            }
            Expr::ArrayRepeat { value, count, .. } => {
                // Infer array type from repeated value
                let elem_type_str = self.infer_expr_type(value)?;

                // Extract count from expression (simplified - assumes integer literal)
                let size = match count.as_ref() {
                    Expr::Integer(n) => *n as usize,
                    _ => {
                        return Err(CompileError::Generic(
                            "Array size must be a constant integer".to_string(),
                        ))
                    }
                };

                Ok(format!("[{}; {}]", elem_type_str, size))
            }
            _ => {
                // For other complex expressions, we'd need full type checking
                Err(CompileError::Generic(
                    "Cannot infer type from complex expression".to_string(),
                ))
            }
        }
    }

    /// Convert CheckerType to string for type arguments
    #[allow(clippy::only_used_in_recursion)]
    fn checker_type_to_string(&self, ty: &CheckerType) -> String {
        match ty {
            CheckerType::Unit => "()".to_string(),
            CheckerType::String => "String".to_string(),
            CheckerType::Int => "i64".to_string(),
            CheckerType::Bool => "bool".to_string(),
            CheckerType::Array(elem, size) => {
                format!("[{}; {}]", self.checker_type_to_string(elem), size)
            }
            CheckerType::Struct(name) => name.clone(),
            CheckerType::TypeParam(name) => name.clone(),
            CheckerType::Enum(name) => name.clone(),
            CheckerType::Function(params, ret) => {
                let param_strs: Vec<String> = params
                    .iter()
                    .map(|p| self.checker_type_to_string(p))
                    .collect();
                format!(
                    "fn({}) -> {}",
                    param_strs.join(", "),
                    self.checker_type_to_string(ret)
                )
            }
        }
    }

    /// Instantiate a generic function with concrete types
    fn instantiate_generic_function(
        &mut self,
        generic_func: &GenericFunction,
        type_args: &[String],
    ) -> Result<CheckerType> {
        // Create a substitution map
        let mut subst_map: HashMap<String, String> = HashMap::new();
        for (type_param, type_arg) in generic_func.type_params.iter().zip(type_args) {
            subst_map.insert(type_param.clone(), type_arg.clone());
        }

        // Substitute types in parameters
        let mut param_types = Vec::new();
        for (_param_name, param_type) in &generic_func.params {
            let substituted_type = self.substitute_type(param_type, &subst_map)?;
            param_types.push(CheckerType::from(&substituted_type));
        }

        // Substitute return type
        let return_type = match &generic_func.return_type {
            Some(ret_type) => {
                let substituted = self.substitute_type(ret_type, &subst_map)?;
                CheckerType::from(&substituted)
            }
            None => CheckerType::Unit,
        };

        Ok(CheckerType::Function(param_types, Box::new(return_type)))
    }

    /// Substitute type parameters in a type
    #[allow(clippy::only_used_in_recursion)]
    fn substitute_type(
        &self,
        ty: &crate::ast::Type,
        subst_map: &HashMap<String, String>,
    ) -> Result<crate::ast::Type> {
        match ty {
            crate::ast::Type::TypeParam(param_name) => {
                match subst_map.get(param_name) {
                    Some(concrete_type) => {
                        // Convert string back to Type
                        match concrete_type.as_str() {
                            "()" => Ok(crate::ast::Type::Unit),
                            "String" => Ok(crate::ast::Type::String),
                            "i64" => Ok(crate::ast::Type::I64),
                            "i32" => Ok(crate::ast::Type::I32),
                            "u64" => Ok(crate::ast::Type::U64),
                            "u32" => Ok(crate::ast::Type::U32),
                            "bool" => Ok(crate::ast::Type::Bool),
                            _ => Ok(crate::ast::Type::Custom(concrete_type.clone())),
                        }
                    }
                    None => Err(CompileError::Generic(format!(
                        "Type parameter '{}' not found in substitution map",
                        param_name
                    ))),
                }
            }
            crate::ast::Type::Array(elem_type, size) => {
                let substituted_elem = self.substitute_type(elem_type, subst_map)?;
                Ok(crate::ast::Type::Array(Box::new(substituted_elem), *size))
            }
            _ => Ok(ty.clone()),
        }
    }

    /// Check a function call with a known function type
    fn check_call_with_type(
        &mut self,
        func_name: &str,
        func_type: CheckerType,
        args: &[Expr],
    ) -> Result<CheckerType> {
        match func_type {
            CheckerType::Function(param_types, return_type) => {
                // Check argument count
                if args.len() != param_types.len() {
                    return Err(CompileError::Generic(format!(
                        "Function '{}' expects {} arguments, got {}",
                        func_name,
                        param_types.len(),
                        args.len()
                    )));
                }

                // Type check each argument
                for (arg, expected_type) in args.iter().zip(&param_types) {
                    let arg_type = self.check_expression(arg)?;
                    if arg_type != *expected_type {
                        return Err(CompileError::TypeMismatch {
                            expected: expected_type.to_string(),
                            found: arg_type.to_string(),
                            span: None,
                        });
                    }
                }

                Ok(*return_type)
            }
            _ => Err(CompileError::Generic(format!(
                "'{}' is not a function",
                func_name
            ))),
        }
    }

    /// Get all generic function instantiations for code generation
    pub fn get_instantiations(&self) -> Vec<(String, Vec<String>, GenericFunction)> {
        let mut result = Vec::new();

        for instantiation in self.instantiations.keys() {
            if let Some(generic_func) = self.generic_functions.get(&instantiation.name) {
                result.push((
                    instantiation.name.clone(),
                    instantiation.type_args.clone(),
                    generic_func.clone(),
                ));
            }
        }

        result
    }

    /// Get all available variable names for suggestions
    fn get_available_variables(&self) -> Vec<String> {
        let mut vars = Vec::new();
        for scope in &self.symbols.scopes {
            for var_name in scope.keys() {
                vars.push(var_name.clone());
            }
        }
        vars
    }

    /// Get all available function names for suggestions
    fn get_available_functions(&self) -> Vec<String> {
        let mut funcs: Vec<String> = self.functions.keys().cloned().collect();
        funcs.extend(self.generic_functions.keys().cloned());
        funcs
    }

    /// Get all available type names for suggestions
    #[allow(dead_code)]
    fn get_available_types(&self) -> Vec<String> {
        let mut types = vec!["String".to_string(), "i64".to_string(), "bool".to_string()];
        types.extend(self.structs.keys().cloned());
        types.extend(self.enums.keys().cloned());
        types
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    #[test]
    fn test_type_check_hello_world() {
        let source = r#"
        fn main() {
            print("Hello, World!");
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_undefined_function() {
        let source = r#"
        fn main() {
            unknown_function();
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
    }

    #[test]
    fn test_let_binding() {
        let source = r#"
        fn main() {
            let x = 42;
            let y: i32 = 10;
            let message = "Hello";
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_variable_usage() {
        let source = r#"
        fn main() {
            let x = 42;
            let y = x;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_undefined_variable() {
        let source = r#"
        fn main() {
            let x = y;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
    }

    #[test]
    fn test_binary_operations() {
        let source = r#"
        fn main() {
            let x = 10 + 20;
            let y = x - 5;
            let z = y * 2;
            let w = z / 3;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_type_mismatch_in_binary() {
        let source = r#"
        fn main() {
            let x = "hello" + 42;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());

        if let Err(CompileError::TypeMismatch {
            expected,
            found,
            span: _,
            ..
        }) = result
        {
            assert_eq!(expected, "String");
            assert_eq!(found, "Int");
        }
    }

    #[test]
    fn test_type_annotation_mismatch() {
        let source = r#"
        fn main() {
            let x: i32 = "not an int";
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());

        if let Err(CompileError::TypeMismatch {
            expected,
            found,
            span: _,
            ..
        }) = result
        {
            assert_eq!(expected, "Int");
            assert_eq!(found, "String");
        }
    }

    #[test]
    fn test_variable_redefinition() {
        let source = r#"
        fn main() {
            let x = 42;
            let x = "redefined";
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
    }

    #[test]
    fn test_for_loop_type_checking() {
        let source = r#"
        fn main() {
            let arr = [1, 2, 3, 4, 5];
            for i in arr {
                print_int(i);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_for_loop_wrong_type() {
        let source = r#"
        fn main() {
            let x = 42;
            for i in x {
                print_int(i);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());

        if let Err(CompileError::Generic(msg)) = result {
            assert!(msg.contains("For loop requires an array"));
        }
    }

    #[test]
    fn test_break_continue_in_loops() {
        let source = r#"
        fn main() {
            let arr = [1, 2, 3, 4, 5];
            
            // Test break and continue in while loop
            let mut i = 0;
            while i < 10 {
                if i == 5 {
                    break;
                }
                if i == 3 {
                    i = i + 1;
                    continue;
                }
                i = i + 1;
            }
            
            // Test break and continue in for loop
            for n in arr {
                if n == 3 {
                    continue;
                }
                if n > 4 {
                    break;
                }
                print_int(n);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_string_len_typecheck() {
        let source = r#"
        fn main() {
            let s = "Hello";
            let len = string_len(s);
            print_int(len);
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_string_concat_typecheck() {
        let source = r#"
        fn main() {
            let s1 = "Hello";
            let s2 = " World";
            let s3 = string_concat(s1, s2);
            print(s3);
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_string_char_predicates() {
        let source = r#"
        fn main() {
            let c = 65;
            let is_alpha = char_is_alpha(c);
            let is_digit = char_is_digit(c);
            let is_space = char_is_whitespace(c);
            if is_alpha {
                print("Is alphabetic");
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_string_type_errors() {
        let source = r#"
        fn main() {
            let n = 42;
            let len = string_len(n); // Error: expects string
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_err());
    }

    #[test]
    fn test_file_io_typecheck() {
        let source = r#"
        fn main() {
            let path = "test.txt";
            let exists = file_exists(path);
            if exists {
                let handle = file_open(path);
                let content = file_read_all(handle);
                file_close(handle);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_file_write_typecheck() {
        let source = r#"
        fn main() {
            let handle = file_open("output.txt");
            let success = file_write(handle, "test content");
            let closed = file_close(handle);
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_file_io_type_errors() {
        let source = r#"
        fn main() {
            let handle = file_open(123); // Error: expects string
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_err());
    }

    #[test]
    fn test_result_enum_definition() {
        let source = r#"
        enum Result {
            Ok(String),
            Err(String),
        }
        
        fn main() {
            let ok = Result::Ok("success");
            let err = Result::Err("failure");
            
            match ok {
                Result::Ok(_) => print("ok"),
                Result::Err(_) => print("err"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        match type_checker.check(&ast) {
            Ok(_) => {}
            Err(e) => panic!("Type check failed: {}", e),
        }
    }

    #[test]
    fn test_result_pattern_matching() {
        let source = r#"
        enum IntResult {
            Ok(i64),
            Err(String),
        }
        
        fn main() {
            let result = IntResult::Ok(42);
            
            match result {
                IntResult::Ok(_) => print("Success"),
                IntResult::Err(_) => print("Error"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_multiple_result_types() {
        let source = r#"
        enum StringResult {
            Ok(String),
            Err(String),
        }
        
        enum FileResult {
            Ok(i64),
            Err(String),
        }
        
        fn main() {
            let s_result = StringResult::Ok("test");
            let f_result = FileResult::Err("not found");
            
            match s_result {
                StringResult::Ok(_) => {
                    print("string ok");
                }
                StringResult::Err(_) => {
                    print("string err");
                }
            }
            
            match f_result {
                FileResult::Ok(_) => {
                    print("file ok");
                }
                FileResult::Err(_) => {
                    print("file err");
                }
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }
}
