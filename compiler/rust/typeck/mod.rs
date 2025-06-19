// Type checker for Palladium
// "Ensuring legends are logically sound"

use crate::ast::{AssignTarget, UnaryOp, *};
use crate::errors::{CompileError, Result};
use std::collections::HashMap;

mod suggestions;
use suggestions::TypeErrorHelper;

mod exhaustiveness;
use exhaustiveness::{ExhaustivenessChecker, EnumInfo, VariantInfo};

/// Type representation for type checker (wraps AST Type)
#[derive(Debug, Clone, PartialEq)]
pub enum CheckerType {
    Unit,
    String,
    Int,
    Bool,
    Array(Box<CheckerType>, ArraySizeValue),
    Function(Vec<CheckerType>, Box<CheckerType>),
    Struct(String),
    TypeParam(String),
    Enum(String),
    Generic {
        name: String,
        args: Vec<GenericArgValue>,
    },
}

/// Array size value for type checking
#[derive(Debug, Clone, PartialEq)]
pub enum ArraySizeValue {
    Literal(usize),
    ConstParam(String),
}

impl std::fmt::Display for ArraySizeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArraySizeValue::Literal(n) => write!(f, "{}", n),
            ArraySizeValue::ConstParam(name) => write!(f, "{}", name),
        }
    }
}

/// Generic argument value for type checking
#[derive(Debug, Clone, PartialEq)]
pub enum GenericArgValue {
    Type(CheckerType),
    Const(ConstValueResolved),
}

/// Resolved const value
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValueResolved {
    Integer(i64),
    ConstParam(String),
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
                let size_value = match size {
                    ArraySize::Literal(n) => ArraySizeValue::Literal(*n),
                    ArraySize::ConstParam(name) => ArraySizeValue::ConstParam(name.clone()),
                    ArraySize::Expr(_) => {
                        // For now, we don't support expressions
                        ArraySizeValue::Literal(0) // Placeholder
                    }
                };
                CheckerType::Array(Box::new(CheckerType::from(elem_type.as_ref())), size_value)
            }
            crate::ast::Type::Custom(name) => CheckerType::Struct(name.clone()),
            crate::ast::Type::TypeParam(name) => {
                // Type parameters need proper handling through substitution
                // For now, create a placeholder type that can be unified later
                CheckerType::TypeParam(name.clone())
            }
            crate::ast::Type::Generic { name, args } => {
                // Convert generic arguments properly
                let checker_args: Vec<GenericArgValue> = args.iter().map(|arg| {
                    match arg {
                        GenericArg::Type(t) => GenericArgValue::Type(CheckerType::from(t)),
                        GenericArg::Const(c) => GenericArgValue::Const(match c {
                            ConstValue::Integer(n) => ConstValueResolved::Integer(*n),
                            ConstValue::ConstParam(name) => ConstValueResolved::ConstParam(name.clone()),
                        }),
                    }
                }).collect();
                CheckerType::Generic {
                    name: name.clone(),
                    args: checker_args,
                }
            }
            crate::ast::Type::Reference { inner, .. } => {
                // For now, treat references as the inner type
                // TODO: Proper reference type handling
                CheckerType::from(inner.as_ref())
            }
            crate::ast::Type::Future { output } => {
                // Create a Future generic type
                CheckerType::Generic {
                    name: "Future".to_string(),
                    args: vec![GenericArgValue::Type(CheckerType::from(output.as_ref()))],
                }
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
            CheckerType::Array(elem_type, size) => {
                match size {
                    ArraySizeValue::Literal(n) => write!(f, "[{}; {}]", elem_type, n),
                    ArraySizeValue::ConstParam(name) => write!(f, "[{}; {}]", elem_type, name),
                }
            }
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
            CheckerType::Generic { name, args } => {
                write!(f, "{}<", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    match arg {
                        GenericArgValue::Type(t) => write!(f, "{}", t)?,
                        GenericArgValue::Const(c) => match c {
                            ConstValueResolved::Integer(n) => write!(f, "{}", n)?,
                            ConstValueResolved::ConstParam(name) => write!(f, "{}", name)?,
                        },
                    }
                }
                write!(f, ">")
            }
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
    pub lifetime_params: Vec<String>,
    pub type_params: Vec<String>,
    pub params: Vec<(String, crate::ast::Type)>,
    pub return_type: Option<crate::ast::Type>,
    pub body: Vec<crate::ast::Stmt>,
}

/// Generic enum definition
#[derive(Debug, Clone)]
pub struct GenericEnum {
    pub lifetime_params: Vec<String>,
    pub type_params: Vec<String>,
    pub variants: Vec<(String, crate::ast::EnumVariantData)>,
}

/// Generic struct definition
#[derive(Debug, Clone)]
pub struct GenericStruct {
    pub lifetime_params: Vec<String>,
    pub type_params: Vec<String>,
    pub fields: Vec<(String, crate::ast::Type)>,
}

/// Generic type alias definition
#[derive(Debug, Clone)]
pub struct GenericTypeAlias {
    pub lifetime_params: Vec<String>,
    pub type_params: Vec<String>,
    pub ty: crate::ast::Type,
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

/// A concrete instantiation of a generic struct
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructInstantiation {
    pub name: String,
    pub type_args: Vec<String>, // Concrete types like "i64", "String"
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
    /// Generic struct definitions
    generic_structs: HashMap<String, GenericStruct>,
    /// Instantiated generic structs
    struct_instantiations: HashMap<StructInstantiation, CheckerType>,
    /// Enum definitions with their variants
    enums: HashMap<String, Vec<EnumVariant>>,
    /// Generic enum definitions
    generic_enums: HashMap<String, GenericEnum>,
    /// Type alias definitions
    type_aliases: HashMap<String, crate::ast::Type>,
    /// Generic type alias definitions
    generic_type_aliases: HashMap<String, GenericTypeAlias>,
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
    /// Unsafe block depth counter (for tracking unsafe context)
    unsafe_depth: usize,
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
            generic_structs: HashMap::new(),
            struct_instantiations: HashMap::new(),
            enums: HashMap::new(),
            generic_enums: HashMap::new(),
            type_aliases: HashMap::new(),
            generic_type_aliases: HashMap::new(),
            current_function_return: None,
            symbols: SymbolTable::new(),
            imported_modules: HashMap::new(),
            loop_depth: 0,
            error_helper: TypeErrorHelper::new(),
            unsafe_depth: 0,
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
                                    lifetime_params: func.lifetime_params.clone(),
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
                    crate::ast::Item::Trait(trait_def) => {
                        if matches!(trait_def.visibility, crate::ast::Visibility::Public) {
                            // Store trait information
                            // TODO: Implement trait tracking
                        }
                    }
                    crate::ast::Item::Impl(_) => {
                        // Impl blocks are processed separately
                    }
                    crate::ast::Item::TypeAlias(type_alias) => {
                        if matches!(type_alias.visibility, crate::ast::Visibility::Public) {
                            // Store type alias information
                            let qualified_name = format!("{}::{}", module_name, type_alias.name);
                            
                            if !type_alias.type_params.is_empty() {
                                // Generic type alias
                                let generic_alias = GenericTypeAlias {
                                    lifetime_params: type_alias.lifetime_params.clone(),
                                    type_params: type_alias.type_params.clone(),
                                    ty: type_alias.ty.clone(),
                                };
                                self.generic_type_aliases
                                    .insert(type_alias.name.clone(), generic_alias.clone());
                                self.generic_type_aliases
                                    .insert(qualified_name, generic_alias);
                            } else {
                                // Regular type alias
                                self.type_aliases
                                    .insert(type_alias.name.clone(), type_alias.ty.clone());
                                self.type_aliases
                                    .insert(qualified_name, type_alias.ty.clone());
                            }
                        }
                    }
                    crate::ast::Item::Macro(_) => {
                        // Macros are handled during expansion phase, skip here
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
                            lifetime_params: func.lifetime_params.clone(),
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
                            .map(|param| self.ast_type_to_checker_type(&param.ty))
                            .collect();

                        let return_type = func
                            .return_type
                            .as_ref()
                            .map(|t| self.ast_type_to_checker_type(t))
                            .unwrap_or(CheckerType::Unit);

                        let func_type = CheckerType::Function(param_types, Box::new(return_type));
                        self.functions.insert(func.name.clone(), func_type);
                    }
                }
                Item::Struct(struct_def) => {
                    // Check if this is a generic struct
                    if !struct_def.type_params.is_empty() || !struct_def.lifetime_params.is_empty() {
                        // Store as generic struct
                        let generic_struct = GenericStruct {
                            lifetime_params: struct_def.lifetime_params.clone(),
                            type_params: struct_def.type_params.clone(),
                            fields: struct_def.fields.clone(),
                        };
                        self.generic_structs.insert(struct_def.name.clone(), generic_struct);
                    } else {
                        // Convert field types to CheckerType for non-generic structs
                        let fields: Vec<(String, CheckerType)> = struct_def
                            .fields
                            .iter()
                            .map(|(name, ty)| (name.clone(), CheckerType::from(ty)))
                            .collect();

                        self.structs.insert(struct_def.name.clone(), fields);
                    }
                }
                Item::Enum(enum_def) => {
                    // Check if this is a generic enum
                    if !enum_def.type_params.is_empty() || !enum_def.lifetime_params.is_empty() {
                        // Store as generic enum
                        let generic_enum = GenericEnum {
                            lifetime_params: enum_def.lifetime_params.clone(),
                            type_params: enum_def.type_params.clone(),
                            variants: enum_def.variants.iter()
                                .map(|v| (v.name.clone(), v.data.clone()))
                                .collect(),
                        };
                        self.generic_enums.insert(enum_def.name.clone(), generic_enum);
                    } else {
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
                Item::Trait(_trait_def) => {
                    // Store trait information
                    // TODO: Implement trait tracking
                }
                Item::TypeAlias(type_alias) => {
                    // Check if this is a generic type alias
                    if !type_alias.type_params.is_empty() || !type_alias.lifetime_params.is_empty() {
                        // Store as generic type alias
                        let generic_alias = GenericTypeAlias {
                            lifetime_params: type_alias.lifetime_params.clone(),
                            type_params: type_alias.type_params.clone(),
                            ty: type_alias.ty.clone(),
                        };
                        self.generic_type_aliases.insert(type_alias.name.clone(), generic_alias);
                    } else {
                        // Store regular type alias
                        self.type_aliases.insert(type_alias.name.clone(), type_alias.ty.clone());
                    }
                }
                Item::Impl(impl_block) => {
                    // Register methods from impl blocks
                    for method in &impl_block.methods {
                        // Create qualified method name
                        let method_name = if let Some(_trait_type) = &impl_block.trait_type {
                            // Trait implementation method
                            format!("{}::{}", impl_block.for_type, method.name)
                        } else {
                            // Inherent method
                            format!("{}::{}", impl_block.for_type, method.name)
                        };
                        
                        if !method.type_params.is_empty() {
                            // Generic method - store for later instantiation
                            let generic_func = GenericFunction {
                                lifetime_params: method.lifetime_params.clone(),
                                type_params: method.type_params.clone(),
                                params: method
                                    .params
                                    .iter()
                                    .map(|p| (p.name.clone(), p.ty.clone()))
                                    .collect(),
                                return_type: method.return_type.clone(),
                                body: method.body.clone(),
                            };
                            self.generic_functions.insert(method_name, generic_func);
                        } else {
                            // Regular method
                            let param_types: Vec<CheckerType> = method
                                .params
                                .iter()
                                .map(|param| CheckerType::from(&param.ty))
                                .collect();

                            let return_type = method
                                .return_type
                                .as_ref()
                                .map(CheckerType::from)
                                .unwrap_or(CheckerType::Unit);

                            let func_type = CheckerType::Function(param_types, Box::new(return_type));
                            self.functions.insert(method_name, func_type);
                        }
                    }
                }
                Item::Macro(_) => {
                    // Macros are handled during expansion phase, skip here
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
                Item::Trait(_) => {
                    // Traits are already processed in the first pass
                    // TODO: Type check trait methods with bodies
                }
                Item::TypeAlias(_) => {
                    // Type aliases are already processed in the first pass
                    // No body to check
                }
                Item::Impl(impl_block) => {
                    // Type check impl block methods
                    for method in &impl_block.methods {
                        self.check_function(method)?;
                    }
                }
                Item::Macro(_) => {
                    // Macros are handled during expansion phase, skip here
                }
            }
        }

        Ok(())
    }

    /// Convert AST type to CheckerType considering context (struct vs enum)
    fn ast_type_to_checker_type(&self, ast_type: &crate::ast::Type) -> CheckerType {
        match ast_type {
            crate::ast::Type::Custom(name) => {
                // First check if it's a type alias
                if let Some(aliased_type) = self.type_aliases.get(name) {
                    // Recursively resolve the aliased type
                    return self.ast_type_to_checker_type(aliased_type);
                }
                
                // Check if it's an enum
                if self.enums.contains_key(name) {
                    CheckerType::Enum(name.clone())
                } else {
                    CheckerType::Struct(name.clone())
                }
            }
            crate::ast::Type::Generic { name, args } => {
                // First check if it's a generic type alias
                if let Some(generic_alias) = self.generic_type_aliases.get(name) {
                    // We have a generic type alias, substitute the type parameters
                    if args.len() != generic_alias.type_params.len() {
                        // For now, just return the generic type without substitution
                        // TODO: Proper error handling for wrong number of type arguments
                        let checker_args: Vec<GenericArgValue> = args.iter()
                            .map(|arg| match arg {
                                GenericArg::Type(t) => GenericArgValue::Type(self.ast_type_to_checker_type(t)),
                                GenericArg::Const(c) => GenericArgValue::Const(match c {
                                    ConstValue::Integer(n) => ConstValueResolved::Integer(*n),
                                    ConstValue::ConstParam(name) => ConstValueResolved::ConstParam(name.clone()),
                                }),
                            })
                            .collect();
                        return CheckerType::Generic {
                            name: name.clone(),
                            args: checker_args,
                        };
                    }
                    
                    // Create a substitution map for type parameters only
                    let mut substitutions = std::collections::HashMap::new();
                    let type_args: Vec<crate::ast::Type> = args.iter()
                        .filter_map(|arg| match arg {
                            GenericArg::Type(t) => Some(t.clone()),
                            GenericArg::Const(_) => None, // TODO: handle const generics in aliases
                        })
                        .collect();
                    
                    for (param, arg) in generic_alias.type_params.iter().zip(type_args.iter()) {
                        substitutions.insert(param.clone(), arg.clone());
                    }
                    
                    // Substitute type parameters in the aliased type
                    let substituted_type = self.substitute_type_params_map(&generic_alias.ty, &substitutions);
                    return self.ast_type_to_checker_type(&substituted_type);
                }
                
                // Not a type alias, convert generic types normally
                let checker_args: Vec<GenericArgValue> = args.iter()
                    .map(|arg| match arg {
                        GenericArg::Type(t) => GenericArgValue::Type(self.ast_type_to_checker_type(t)),
                        GenericArg::Const(c) => GenericArgValue::Const(match c {
                            ConstValue::Integer(n) => ConstValueResolved::Integer(*n),
                            ConstValue::ConstParam(name) => ConstValueResolved::ConstParam(name.clone()),
                        }),
                    })
                    .collect();
                    
                CheckerType::Generic {
                    name: name.clone(),
                    args: checker_args,
                }
            }
            _ => CheckerType::from(ast_type)
        }
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
            let checker_type = self.ast_type_to_checker_type(&param.ty);
            self.symbols
                .define(param.name.clone(), checker_type, param.mutable)?;
        }

        // Set current function return type
        let base_return_type = func
            .return_type
            .as_ref()
            .map(|t| self.ast_type_to_checker_type(t))
            .unwrap_or(CheckerType::Unit);
        
        // If function is async, wrap return type in Future
        let return_type = if func.is_async {
            CheckerType::Generic {
                name: "Future".to_string(),
                args: vec![GenericArgValue::Type(base_return_type)],
            }
        } else {
            base_return_type
        };
        
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
                    let expected_type = self.ast_type_to_checker_type(annotated_type);
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

                        let field_type = match &object_type {
                            // Handle non-generic structs
                            CheckerType::Struct(name) => {
                                // Look up the struct fields
                                let fields = self.structs.get(name).ok_or_else(|| {
                                    CompileError::Generic(format!("Unknown struct type: {}", name))
                                })?;

                                // Find the field type
                                fields
                                    .iter()
                                    .find(|(fname, _)| fname == field)
                                    .map(|(_, ftype)| ftype.clone())
                                    .ok_or_else(|| {
                                        CompileError::Generic(format!(
                                            "Struct '{}' has no field '{}'",
                                            name, field
                                        ))
                                    })?
                            }
                            // Handle generic struct instances
                            CheckerType::Generic { name, args } => {
                                // Look up the generic struct definition
                                let generic_struct = self.generic_structs.get(name).ok_or_else(|| {
                                    CompileError::Generic(format!("Unknown generic struct type: {}", name))
                                })?;

                                // Find the field's declared type
                                let field_type = generic_struct.fields
                                    .iter()
                                    .find(|(fname, _)| fname == field)
                                    .map(|(_, ftype)| ftype)
                                    .ok_or_else(|| {
                                        CompileError::Generic(format!(
                                            "Struct '{}' has no field '{}'",
                                            name, field
                                        ))
                                    })?;

                                // Extract type arguments only
                                let type_args: Vec<CheckerType> = args.iter()
                                    .filter_map(|arg| match arg {
                                        GenericArgValue::Type(t) => Some(t.clone()),
                                        GenericArgValue::Const(_) => None, // TODO: handle const generics
                                    })
                                    .collect();
                                
                                // Substitute type parameters in the field type
                                let concrete_field_type = self.substitute_type_params(
                                    field_type, 
                                    &generic_struct.type_params, 
                                    &type_args
                                )?;

                                concrete_field_type
                            }
                            _ => {
                                return Err(CompileError::Generic(format!(
                                    "Cannot access field on non-struct type: {}",
                                    object_type
                                )));
                            }
                        };

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
                    AssignTarget::Deref { expr } => {
                        // Type check the expression being dereferenced
                        let _ptr_type = self.check_expression(expr)?;
                        // For now, we don't have proper reference types, so just check the value
                        let _value_type = self.check_expression(value)?;
                        // TODO: Check that ptr_type is actually a reference to value_type
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
            Stmt::Match { expr, arms, span, .. } => {
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
                if let CheckerType::Enum(enum_name) = &expr_type {
                    // Build enum info for exhaustiveness checker
                    let mut enum_infos = HashMap::new();
                    for (name, variants) in &self.enums {
                        let variant_infos: Vec<VariantInfo> = variants.iter().map(|v| {
                            let arity = match &v.fields {
                                EnumVariantFields::Unit => 0,
                                EnumVariantFields::Tuple(types) => types.len(),
                                EnumVariantFields::Named(fields) => fields.len(),
                            };
                            VariantInfo {
                                name: v.name.clone(),
                                arity,
                            }
                        }).collect();
                        
                        enum_infos.insert(name.clone(), EnumInfo {
                            name: name.clone(),
                            variants: variant_infos,
                        });
                    }
                    
                    let exhaustiveness_checker = ExhaustivenessChecker::new(enum_infos);
                    let patterns: Vec<Pattern> = arms.iter().map(|arm| arm.pattern.clone()).collect();
                    exhaustiveness_checker.check_match(enum_name, &patterns, *span)?;
                }

                Ok(())
            }
            Stmt::Unsafe { body, .. } => {
                // Enter unsafe context
                self.unsafe_depth += 1;
                
                // Type check body in new scope
                self.symbols.enter_scope();
                for stmt in body {
                    self.check_statement(stmt)?;
                }
                self.symbols.exit_scope();
                
                // Exit unsafe context
                self.unsafe_depth -= 1;
                
                Ok(())
            }
        }
    }
    
    /// Substitute type parameters in a type with concrete types
    fn substitute_type_params(
        &self,
        ty: &crate::ast::Type,
        type_params: &[String],
        concrete_types: &[CheckerType],
    ) -> Result<CheckerType> {
        match ty {
            crate::ast::Type::TypeParam(name) => {
                // Find the index of this type parameter
                if let Some(idx) = type_params.iter().position(|p| p == name) {
                    if idx < concrete_types.len() {
                        Ok(concrete_types[idx].clone())
                    } else {
                        Err(CompileError::Generic(
                            format!("Type parameter {} not found in substitution", name)
                        ))
                    }
                } else {
                    Err(CompileError::Generic(
                        format!("Unknown type parameter: {}", name)
                    ))
                }
            }
            crate::ast::Type::Custom(name) => {
                // Check if this custom type is actually a type parameter
                if let Some(idx) = type_params.iter().position(|p| p == name) {
                    if idx < concrete_types.len() {
                        Ok(concrete_types[idx].clone())
                    } else {
                        Err(CompileError::Generic(
                            format!("Type parameter {} not found in substitution", name)
                        ))
                    }
                } else {
                    // Not a type parameter, just a regular custom type
                    Ok(CheckerType::from(ty))
                }
            }
            // For other types, just convert normally
            _ => Ok(CheckerType::from(ty)),
        }
    }

    /// Substitute type parameters in a type using a substitution map
    fn substitute_type_params_map(
        &self,
        ty: &crate::ast::Type,
        substitutions: &std::collections::HashMap<String, crate::ast::Type>,
    ) -> crate::ast::Type {
        match ty {
            crate::ast::Type::TypeParam(name) | crate::ast::Type::Custom(name) => {
                // Check if this is a type parameter that should be substituted
                if let Some(replacement) = substitutions.get(name) {
                    replacement.clone()
                } else {
                    ty.clone()
                }
            }
            crate::ast::Type::Generic { name, args } => {
                // Recursively substitute in generic type arguments
                let new_args: Vec<GenericArg> = args.iter()
                    .map(|arg| match arg {
                        GenericArg::Type(t) => GenericArg::Type(self.substitute_type_params_map(t, substitutions)),
                        GenericArg::Const(c) => GenericArg::Const(c.clone()), // TODO: substitute const params
                    })
                    .collect();
                crate::ast::Type::Generic {
                    name: name.clone(),
                    args: new_args,
                }
            }
            crate::ast::Type::Array(elem_type, size) => {
                crate::ast::Type::Array(
                    Box::new(self.substitute_type_params_map(elem_type, substitutions)),
                    size.clone(),
                )
            }
            crate::ast::Type::Reference { lifetime, inner, mutable } => {
                crate::ast::Type::Reference {
                    lifetime: lifetime.clone(),
                    inner: Box::new(self.substitute_type_params_map(inner, substitutions)),
                    mutable: *mutable,
                }
            }
            // Other types don't contain type parameters
            _ => ty.clone(),
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

                Ok(CheckerType::Array(Box::new(elem_type), ArraySizeValue::Literal(elements.len())))
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
                        Ok(CheckerType::Array(Box::new(elem_type), ArraySizeValue::Literal(*n as usize)))
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
                // First check if this is a generic struct
                if let Some(generic_struct) = self.generic_structs.get(name).cloned() {
                    // For generic structs, we need to infer type parameters from field values
                    let mut type_substitutions: HashMap<String, CheckerType> = HashMap::new();
                    
                    // First pass: check that all provided fields are valid
                    for (field_name, _) in fields {
                        // Find the field's declared type in the generic struct
                        generic_struct.fields.iter()
                            .find(|(fname, _)| fname == field_name)
                            .ok_or_else(|| {
                                CompileError::Generic(format!(
                                    "Unknown field '{}' for struct '{}'",
                                    field_name, name
                                ))
                            })?;
                    }
                    
                    // Second pass: collect type constraints from field values
                    for (field_name, field_expr) in fields {
                        // Find the field's declared type in the generic struct
                        let field_type = generic_struct.fields.iter()
                            .find(|(fname, _)| fname == field_name)
                            .map(|(_, ftype)| ftype)
                            .unwrap(); // Safe because we checked in first pass
                        
                        // Type check the field expression
                        let provided_type = self.check_expression(field_expr)?;
                        
                        // If the field type is a type parameter, record the constraint
                        if let crate::ast::Type::TypeParam(param_name) = field_type {
                            if generic_struct.type_params.contains(param_name) {
                                // Check if we already have a constraint for this type parameter
                                if let Some(existing_type) = type_substitutions.get(param_name) {
                                    if *existing_type != provided_type {
                                        return Err(CompileError::Generic(format!(
                                            "Conflicting type constraints for type parameter '{}': {} vs {}",
                                            param_name, existing_type, provided_type
                                        )));
                                    }
                                } else {
                                    type_substitutions.insert(param_name.clone(), provided_type);
                                }
                            }
                        } else if let crate::ast::Type::Custom(type_name) = field_type {
                            // Check if it's a type parameter referenced as Custom type
                            if generic_struct.type_params.contains(type_name) {
                                if let Some(existing_type) = type_substitutions.get(type_name) {
                                    if *existing_type != provided_type {
                                        return Err(CompileError::Generic(format!(
                                            "Conflicting type constraints for type parameter '{}': {} vs {}",
                                            type_name, existing_type, provided_type
                                        )));
                                    }
                                } else {
                                    type_substitutions.insert(type_name.clone(), provided_type);
                                }
                            }
                        }
                        // TODO: Handle nested generic types like Box<T> where T needs to be inferred
                    }
                    
                    // Check that all type parameters have been inferred
                    let mut inferred_args = Vec::new();
                    for type_param in &generic_struct.type_params {
                        match type_substitutions.get(type_param) {
                            Some(inferred_type) => {
                                inferred_args.push(inferred_type.clone());
                            }
                            None => {
                                return Err(CompileError::Generic(format!(
                                    "Could not infer type parameter '{}' for struct '{}'",
                                    type_param, name
                                )));
                            }
                        }
                    }
                    
                    // Check that all required fields are provided
                    for (field_name, field_type) in &generic_struct.fields {
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
                        
                        // Substitute type parameters in the field type
                        let concrete_checker_type = self.substitute_type_params(field_type, &generic_struct.type_params, &inferred_args)?;
                        
                        // Type check the field with the concrete type
                        let provided_type = self.check_expression(provided_expr)?;
                        if provided_type != concrete_checker_type {
                            return Err(CompileError::TypeMismatch {
                                expected: concrete_checker_type.to_string(),
                                found: provided_type.to_string(),
                                span: None,
                            });
                        }
                    }
                    
                    // Track this instantiation for code generation
                    let type_arg_strings: Vec<String> = inferred_args.iter().map(|ct| {
                        match ct {
                            CheckerType::Int => "i64".to_string(),
                            CheckerType::Bool => "bool".to_string(),
                            CheckerType::String => "String".to_string(),
                            CheckerType::Struct(name) => name.clone(),
                            CheckerType::Generic { name, args } => {
                                // Handle nested generics like Box<Box<Int>>
                                let arg_strs: Vec<String> = args.iter().map(|a| match a {
                                    GenericArgValue::Type(t) => match t {
                                        CheckerType::Int => "i64".to_string(),
                                        CheckerType::Bool => "bool".to_string(),
                                        CheckerType::String => "String".to_string(),
                                        CheckerType::Struct(n) => n.clone(),
                                        _ => "Unknown".to_string(),
                                    },
                                    GenericArgValue::Const(c) => match c {
                                        ConstValueResolved::Integer(n) => n.to_string(),
                                        ConstValueResolved::ConstParam(name) => name.clone(),
                                    },
                                }).collect();
                                format!("{}<{}>", name, arg_strs.join(", "))
                            }
                            _ => "Unknown".to_string(),
                        }
                    }).collect();
                    
                    let instantiation = StructInstantiation {
                        name: name.clone(),
                        type_args: type_arg_strings,
                    };
                    
                    let instantiated_type = CheckerType::Generic {
                        name: name.clone(),
                        args: inferred_args.iter().map(|t| GenericArgValue::Type(t.clone())).collect(),
                    };
                    
                    self.struct_instantiations.insert(instantiation, instantiated_type.clone());
                    
                    // Return the generic struct type with inferred type arguments
                    return Ok(instantiated_type);
                }
                
                // Look up the non-generic struct definition
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

                match &object_type {
                    // Handle non-generic structs
                    CheckerType::Struct(name) => {
                        // Look up the struct fields
                        let fields = self.structs.get(name).ok_or_else(|| {
                            CompileError::Generic(format!("Unknown struct type: {}", name))
                        })?;

                        // Find the field type
                        let field_type = fields
                            .iter()
                            .find(|(fname, _)| fname == field)
                            .map(|(_, ftype)| ftype.clone())
                            .ok_or_else(|| {
                                CompileError::Generic(format!(
                                    "Struct '{}' has no field '{}'",
                                    name, field
                                ))
                            })?;

                        Ok(field_type)
                    }
                    // Handle generic struct instances
                    CheckerType::Generic { name, args } => {
                        // Look up the generic struct definition
                        let generic_struct = self.generic_structs.get(name).ok_or_else(|| {
                            CompileError::Generic(format!("Unknown generic struct type: {}", name))
                        })?;

                        // Find the field's declared type
                        let field_type = generic_struct.fields
                            .iter()
                            .find(|(fname, _)| fname == field)
                            .map(|(_, ftype)| ftype)
                            .ok_or_else(|| {
                                CompileError::Generic(format!(
                                    "Struct '{}' has no field '{}'",
                                    name, field
                                ))
                            })?;

                        // Extract types from generic args
                        let concrete_types: Vec<CheckerType> = args.iter()
                            .filter_map(|arg| match arg {
                                GenericArgValue::Type(t) => Some(t.clone()),
                                _ => None,
                            })
                            .collect();
                        
                        // Substitute type parameters in the field type
                        let concrete_field_type = self.substitute_type_params(
                            field_type, 
                            &generic_struct.type_params, 
                            &concrete_types
                        )?;

                        Ok(concrete_field_type)
                    }
                    _ => {
                        Err(CompileError::Generic(format!(
                            "Cannot access field on non-struct type: {}",
                            object_type
                        )))
                    }
                }
            }
            Expr::EnumConstructor {
                enum_name,
                variant,
                data,
                ..
            } => {
                // Type check enum constructors
                // First check if the enum exists (could be generic or regular)
                if let Some(generic_enum) = self.generic_enums.get(enum_name).cloned() {
                    // Handle generic enum - infer type parameters from constructor arguments
                    let mut inferred_types = Vec::new();
                    
                    // Find the variant in the generic enum definition
                    let variant_data = generic_enum.variants.iter()
                        .find(|(v_name, _)| v_name == variant)
                        .map(|(_, data)| data)
                        .ok_or_else(|| {
                            CompileError::Generic(format!("Unknown variant {}::{}", enum_name, variant))
                        })?;
                    
                    // Infer type parameters from constructor arguments
                    match (variant_data, data.as_ref()) {
                        (crate::ast::EnumVariantData::Tuple(param_types), Some(crate::ast::EnumConstructorData::Tuple(arg_exprs))) => {
                            // For each type parameter in the variant, infer from arguments
                            for (param_type, arg_expr) in param_types.iter().zip(arg_exprs) {
                                let arg_type = self.check_expression(arg_expr)?;
                                
                                // If the parameter type is a type parameter, record the inferred type
                                let is_type_param = match param_type {
                                    crate::ast::Type::TypeParam(param_name) => Some(param_name),
                                    crate::ast::Type::Custom(param_name) if generic_enum.type_params.contains(param_name) => Some(param_name),
                                    _ => None,
                                };
                                
                                if let Some(param_name) = is_type_param {
                                    // Find the index of this type parameter
                                    if let Some(idx) = generic_enum.type_params.iter().position(|p| p == param_name) {
                                        // Ensure we have enough slots
                                        while inferred_types.len() <= idx {
                                            inferred_types.push(CheckerType::Unit); // placeholder
                                        }
                                        inferred_types[idx] = arg_type;
                                    }
                                }
                            }
                        }
                        _ => {
                            // For other cases, we can't infer yet
                            // Return a basic enum type for now
                        }
                    }
                    
                    // If we inferred any types, return a generic type
                    if !inferred_types.is_empty() {
                        return Ok(CheckerType::Generic {
                            name: enum_name.clone(),
                            args: inferred_types.iter().map(|t| GenericArgValue::Type(t.clone())).collect(),
                        });
                    } else {
                        // No type parameters inferred, return basic enum
                        return Ok(CheckerType::Enum(enum_name.clone()));
                    }
                }
                
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
                Ok(CheckerType::Array(Box::new(CheckerType::Int), ArraySizeValue::Literal(0)))
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
            Expr::Reference { mutable: _, expr, .. } => {
                // Type check the inner expression
                let inner_type = self.check_expression(expr)?;
                
                // For now, references have the same type as their inner value
                // TODO: Proper reference type handling
                Ok(inner_type)
            }
            Expr::Deref { expr, .. } => {
                // Type check the expression being dereferenced
                let expr_type = self.check_expression(expr)?;
                
                // For now, assume dereference returns the same type
                // TODO: Proper reference type handling - should check that expr_type is a reference
                Ok(expr_type)
            }
            Expr::Question { expr, .. } => {
                // Type check the expression
                let expr_type = self.check_expression(expr)?;
                
                // The expression must be a Result<T, E> type
                match &expr_type {
                    CheckerType::Generic { name, args } if name == "Result" && args.len() == 2 => {
                        // Extract the Ok type (T) and Error type (E) from Result<T, E>
                        let ok_type = &args[0];
                        let err_type = &args[1];
                        
                        // Check that the current function returns a Result type
                        if let Some(return_type) = &self.current_function_return {
                            match return_type {
                                CheckerType::Generic { name: ret_name, args: ret_args } 
                                    if ret_name == "Result" && ret_args.len() == 2 => {
                                    // Check that error types match
                                    let ret_err_type = &ret_args[1];
                                    if err_type != ret_err_type {
                                        let ret_err_str = match ret_err_type {
                                            GenericArgValue::Type(t) => self.checker_type_to_string(t),
                                            GenericArgValue::Const(c) => match c {
                                                ConstValueResolved::Integer(n) => n.to_string(),
                                                ConstValueResolved::ConstParam(name) => name.clone(),
                                            }
                                        };
                                        let err_str = match err_type {
                                            GenericArgValue::Type(t) => self.checker_type_to_string(t),
                                            GenericArgValue::Const(c) => match c {
                                                ConstValueResolved::Integer(n) => n.to_string(),
                                                ConstValueResolved::ConstParam(name) => name.clone(),
                                            }
                                        };
                                        let ok_str = match ok_type {
                                            GenericArgValue::Type(t) => self.checker_type_to_string(t),
                                            GenericArgValue::Const(c) => match c {
                                                ConstValueResolved::Integer(n) => n.to_string(),
                                                ConstValueResolved::ConstParam(name) => name.clone(),
                                            }
                                        };
                                        return Err(CompileError::TypeMismatch {
                                            expected: format!("Result<_, {}>", ret_err_str),
                                            found: format!("Result<{}, {}>", ok_str, err_str),
                                            span: None,
                                        });
                                    }
                                    
                                    // Return the Ok type
                                    match ok_type {
                                        GenericArgValue::Type(t) => Ok(t.clone()),
                                        _ => Err(CompileError::Generic("Expected type in Result".to_string()))
                                    }
                                }
                                _ => Err(CompileError::Generic(
                                    "The ? operator can only be used in functions that return Result".to_string()
                                ))
                            }
                        } else {
                            Err(CompileError::Generic(
                                "The ? operator can only be used inside a function".to_string()
                            ))
                        }
                    }
                    CheckerType::Enum(name) if name == "Result" => {
                        // Handle non-generic Result (shouldn't happen in practice)
                        Err(CompileError::Generic(
                            "Result type must have generic parameters".to_string()
                        ))
                    }
                    _ => Err(CompileError::TypeMismatch {
                        expected: "Result<T, E>".to_string(),
                        found: expr_type.to_string(),
                        span: None,
                    })
                }
            }
            Expr::MacroInvocation { .. } => {
                // Macros should have been expanded before type checking
                Err(CompileError::Generic(
                    "Unexpected macro invocation in type checking - macros should be expanded before this phase".to_string()
                ))
            }
            Expr::Await { expr, .. } => {
                // Check that the expression is a Future type
                let expr_type = self.check_expression(expr)?;
                match &expr_type {
                    CheckerType::Generic { name, args } if name == "Future" && args.len() == 1 => {
                        // Extract the output type from Future<T>
                        if let GenericArgValue::Type(output_type) = &args[0] {
                            Ok(output_type.clone())
                        } else {
                            Err(CompileError::Generic(
                                "Invalid Future type".to_string()
                            ))
                        }
                    }
                    _ => Err(CompileError::TypeMismatch {
                        expected: "Future<T>".to_string(),
                        found: self.checker_type_to_string(&expr_type),
                        span: None,
                    })
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
                    CheckerType::Generic { name, .. } if name == enum_name => Ok(()),
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
                    
                    // Handle both regular and generic enums
                    match value_type {
                        CheckerType::Enum(expected_enum) if expected_enum == enum_name => {
                            // Regular enum - check if it's actually a generic enum
                            if self.generic_enums.contains_key(enum_name) {
                                // This shouldn't happen - generic enums should have Generic type
                                return Err(CompileError::Generic(
                                    format!("Generic enum {} used without type parameters", enum_name)
                                ));
                            }
                            
                            // Handle regular enum
                            let variants = self.enums.get(enum_name).ok_or_else(|| {
                                CompileError::Generic(format!("Undefined enum type: {}", enum_name))
                            })?.clone();

                            let variant_info = variants
                                .iter()
                                .find(|v| &v.name == variant)
                                .ok_or_else(|| {
                                    CompileError::Generic(format!(
                                        "Unknown variant {}::{}",
                                        enum_name, variant
                                    ))
                                })?.clone();

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
                                        self.bind_pattern_variables(pattern, field_type)?;
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

                                        self.bind_pattern_variables(pattern, field_type)?;
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
                        CheckerType::Generic { name, args } if name == enum_name => {
                            // Generic enum with type arguments
                            if let Some(generic_enum) = self.generic_enums.get(enum_name).cloned() {
                                // Find the variant
                                let variant_data = generic_enum.variants.iter()
                                    .find(|(v_name, _)| v_name == variant)
                                    .map(|(_, data)| data)
                                    .ok_or_else(|| {
                                        CompileError::Generic(format!("Unknown variant {}::{}", enum_name, variant))
                                    })?;
                                
                                // Bind pattern variables based on variant data
                                match (pattern_data, variant_data) {
                                    (PatternData::Tuple(patterns), crate::ast::EnumVariantData::Tuple(param_types)) => {
                                        if patterns.len() != param_types.len() {
                                            return Err(CompileError::Generic(format!(
                                                "Pattern has wrong number of fields for {}::{}",
                                                enum_name, variant
                                            )));
                                        }
                                        
                                        // For each pattern, determine its type by substituting type parameters
                                        let type_params = generic_enum.type_params.clone();
                                        for (pattern, param_type) in patterns.iter().zip(param_types) {
                                            // Extract types from generic args
                                            let concrete_types: Vec<CheckerType> = args.iter()
                                                .filter_map(|arg| match arg {
                                                    GenericArgValue::Type(t) => Some(t.clone()),
                                                    _ => None,
                                                })
                                                .collect();
                                            let concrete_type = self.substitute_type_params(param_type, &type_params, &concrete_types)?;
                                            self.bind_pattern_variables(pattern, &concrete_type)?;
                                        }
                                        return Ok(());
                                    }
                                    _ => {
                                        // TODO: Handle other pattern types
                                        return Ok(());
                                    }
                                }
                            } else {
                                return Err(CompileError::Generic(
                                    format!("Generic enum {} not found in definitions", enum_name)
                                ));
                            }
                        }
                        _ => {
                            return Err(CompileError::TypeMismatch {
                                expected: format!("enum {}", enum_name),
                                found: value_type.to_string(),
                                span: None,
                            });
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
            CheckerType::Generic { name, args } => {
                let arg_strs: Vec<String> = args
                    .iter()
                    .map(|a| match a {
                        GenericArgValue::Type(t) => self.checker_type_to_string(t),
                        GenericArgValue::Const(c) => match c {
                            ConstValueResolved::Integer(n) => n.to_string(),
                            ConstValueResolved::ConstParam(name) => name.clone(),
                        }
                    })
                    .collect();
                format!("{}<{}>", name, arg_strs.join(", "))
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
                Ok(crate::ast::Type::Array(Box::new(substituted_elem), size.clone()))
            }
            crate::ast::Type::Reference { lifetime, mutable, inner } => {
                let substituted_inner = self.substitute_type(inner, subst_map)?;
                Ok(crate::ast::Type::Reference {
                    lifetime: lifetime.clone(),
                    mutable: *mutable,
                    inner: Box::new(substituted_inner),
                })
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
    
    /// Get all generic struct instantiations for code generation
    pub fn get_struct_instantiations(&self) -> Vec<(String, Vec<String>, GenericStruct)> {
        let mut result = Vec::new();

        for instantiation in self.struct_instantiations.keys() {
            if let Some(generic_struct) = self.generic_structs.get(&instantiation.name) {
                result.push((
                    instantiation.name.clone(),
                    instantiation.type_args.clone(),
                    generic_struct.clone(),
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

    /// Check if we're currently in an unsafe context
    pub fn in_unsafe_context(&self) -> bool {
        self.unsafe_depth > 0
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

    #[test]
    fn test_exhaustive_enum_match() {
        let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }
        
        fn main() {
            let c = Color::Red;
            
            match c {
                Color::Red => print("red"),
                Color::Green => print("green"),
                Color::Blue => print("blue"),
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
    fn test_non_exhaustive_enum_match() {
        let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }
        
        fn main() {
            let c = Color::Red;
            
            match c {
                Color::Red => print("red"),
                Color::Green => print("green"),
                // Missing Blue!
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
        
        if let Err(CompileError::NonExhaustiveMatch { missing_patterns, .. }) = result {
            assert!(missing_patterns.contains(&"Color::Blue".to_string()));
        } else {
            panic!("Expected NonExhaustiveMatch error");
        }
    }

    #[test]
    fn test_wildcard_makes_match_exhaustive() {
        let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }
        
        fn main() {
            let c = Color::Red;
            
            match c {
                Color::Red => print("red"),
                _ => print("other"),
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
    fn test_unreachable_pattern_after_wildcard() {
        let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }
        
        fn main() {
            let c = Color::Red;
            
            match c {
                Color::Red => print("red"),
                _ => print("any"),
                Color::Blue => print("blue"), // Unreachable!
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
        
        if let Err(CompileError::UnreachablePattern { .. }) = result {
            // Expected
        } else {
            panic!("Expected UnreachablePattern error");
        }
    }
}
