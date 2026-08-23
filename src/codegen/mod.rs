// Code generation for Palladium
// "Forging legends into machine code"

pub mod c_ident;
pub mod llvm_backend;
pub mod llvm_backend_improved;
pub mod llvm_native;
pub mod llvm_text_backend;

use crate::ast::{AssignTarget, UnaryOp, *};
use crate::errors::{CompileError, Result, Span};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// What code generation can *prove* about an array's outermost length.
///
/// The C type strings in `variables` cannot carry this: `[T; N]` with a const
/// generic `N` and `[T; <expr>]` both print as `[0]` (`type_to_c`), which is
/// indistinguishable from a genuine `[T; 0]`. Deciding a loop bound from that
/// string is how a length that was never resolved silently became a wrong
/// number, so lengths travel as this type instead.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ArrayLen {
    /// A literal length from the declaration. Zero is a real length here, not
    /// a missing one.
    Proven(usize),
    /// The declaration named a length this pass cannot evaluate: a const
    /// generic parameter, or an unevaluated size expression. The string is how
    /// the source spelled it, so a diagnostic can quote it.
    Unproven(String),
}

/// The three spellings an array parameter can have. C decays all of them to a
/// pointer into the caller's storage, so this is the only surviving record of
/// what the author declared, and therefore of what may be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArrayParamForm {
    /// `xs: [T; N]` - no declared intent to mutate anything.
    ByValue,
    /// `mut xs: [T; N]` - the bootstrap subset's spelling for a mutable array
    /// parameter (docs/specification/bootstrap-subset.md:96).
    MutByValue,
    /// `xs: &[T; N]`.
    Shared,
    /// `xs: &mut [T; N]`.
    Mutable,
}

/// Where an array binding's storage lives, which decides whether `sizeof` can
/// count its elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArrayStorage {
    /// A local array object: `sizeof(x)/sizeof(x[0])` really is its length.
    Object,
    /// A parameter: it decayed to a pointer before the callee saw it, so
    /// `sizeof` measures the pointer and the length must come from the type.
    Parameter(ArrayParamForm),
}

/// An in-scope array binding, with the two facts C throws away.
#[derive(Clone, Debug)]
struct ArrayBinding {
    len: ArrayLen,
    storage: ArrayStorage,
}

/// WHERE THIS PASS DECIDES THE THINGS REVIEW KEEPS ASKING ABOUT
/// ------------------------------------------------------------
/// Same problem as the type checker: ~4,300 lines, past three reviewers' read
/// limits. The decisions recent review turns on:
///
///   `current_fn_unit_return`    the field, and its per-function reset in
///                               `generate_function_with_name` — BOTH spellings
///                               of the unit type, and `main`'s `return 0;`
///                               because its C type is `int`.
///   `generate_statement`,       the two `Stmt::Return` arms that consume it.
///   `generate_function_with_name`, `generate_block`
///                               the ONLY two callers of `generate_statement`;
///                               the first sets the reset, the second inherits
///                               it. There were THREE:
///                               `generate_async_function_with_name` was the
///                               other, and it is gone — it was the Future/poll
///                               emitter N7 forbids (N7-18), and `is_async` is
///                               now a refusal at the top of
///                               `generate_function_with_name`.
///   `function_signature`        the `actual_return_type` special case that
///                               rewrites `main` to `int`.
///
/// EVERY SITE THAT DECIDES WHAT THE IMPORTED PROGRAM CONTAINS — derived by
/// listing the readers of `self.imported_modules` and of `self.generic_instantiations`,
/// not by recalling which ones felt relevant. The first version of this map
/// named one of them, and the omitted one was a release blocker: it is a claim
/// about what matters, so it inherits its author's blind spot, and the way out
/// is to enumerate from the code.
///
///   `compile`, signature pass   registers imported public signatures into
///                               `functions` (later OVERWRITTEN by the local
///                               pass) and into `async_functions` (a HashSet
///                               that is only ever inserted into — see the
///                               declared residual on `async_functions`).
///                               Asks NO shadowing question.
///   `compile`, type pass        imported public structs and enums are emitted
///                               unconditionally beside the local ones. The
///                               shared predicate is function-only, so this
///                               question is not merely unasked, it is
///                               unanswerable here today.
///   the imported-function loop  which imported BODIES are emitted: public,
///                               `type_params` empty, and not shadowed — the
///                               last via
///                               `crate::ast::local_definition_shadows_import`,
///                               the same function the type checker calls.
///   the imported-prototype loop in `generate_function_prototypes`, the ONLY
///                               `seen` set in this file. Must ask the SAME
///                               three questions as the body loop, because
///                               `seen` is first-wins and imports are visited
///                               first: a shadowed import that gets a prototype
///                               takes the name and suppresses the local one.
///                               It did not ask, and gcc refused the result.
///   `generic_instantiations`    monomorphised bodies, imported or local. The
///                               list is built by the TYPE CHECKER
///                               (`TypeChecker::get_instantiations`) from
///                               `generic_functions`, so which body an
///                               instantiation carries is decided there and
///                               only executed here — including for an imported
///                               generic, which this pass emits like any other.
pub struct CodeGenerator {
    module_name: String,
    output: String,
    /// Map of function names to their signatures (params and return type)
    functions: std::collections::HashMap<String, (Vec<Param>, Option<Type>)>,
    /// Map of variable names to their C types (for type inference)
    variables: std::collections::HashMap<String, String>,
    /// Array bindings in the function being generated, with the length and the
    /// storage class that the C type string cannot express. Cleared with
    /// `variables` at every function boundary.
    array_bindings: std::collections::HashMap<String, ArrayBinding>,
    /// Map of parameter names to their mutability (for current function)
    mutable_params: std::collections::HashMap<String, bool>,
    /// When the function being generated returns UNIT in Palladium, the C
    /// return statement that must replace a value-bearing one — and `None`
    /// when it genuinely returns a value.
    ///
    /// It is the statement rather than a bool because C `main` is the exception
    /// and a bool made it a NAME-KEYED one. `Type::Unit` maps to C `void`
    /// (`type_to_c`), but `main` is then rewritten to `int`
    /// (`function_signature`, the `actual_return_type` special case), so the
    /// right replacement differs: `return;` for an ordinary unit function,
    /// `return 0;` for `main`. The first version of this field was
    /// `current_fn_is_void: bool` set with `&& name != "main"`, which excluded
    /// the one function every program has — and because C `main` returns `int`,
    /// gcc does NOT extend the courtesy it extends to `void`:
    ///
    /// ```text
    /// fn main() -> () { print_int(7) }
    /// error: returning 'void' from a function with incompatible result
    ///        type 'int'
    /// ```
    ///
    /// A hard failure, in the defect that had just been closed, preserved by a
    /// name-keyed exception. Set per function alongside `mutable_params`, in
    /// the ONE path that feeds `generate_statement` with a `Function`. There
    /// was a second — `generate_async_function_with_name` set it too — and it
    /// is gone with the rest of the Future/poll emitter (N7-18).
    current_fn_unit_return: Option<&'static str>,
    /// Imported modules
    imported_modules: std::collections::HashMap<String, crate::resolver::ModuleInfo>,
    /// Generic function instantiations to generate
    generic_instantiations: Vec<(String, Vec<String>, crate::typeck::GenericFunction)>,
    /// Generic struct instantiations to generate
    generic_struct_instantiations: Vec<(String, Vec<String>, crate::typeck::GenericStruct)>,
    /// Type aliases for resolving custom types
    type_aliases: std::collections::HashMap<String, Type>,
    /// Map of enum names to their definitions
    enums: std::collections::HashMap<String, EnumDef>,
    /// Map of struct names to their declared fields, for typing field access.
    /// Filled by generate_struct, so it covers imported, local and
    /// monomorphized structs alike.
    structs: std::collections::HashMap<String, Vec<(String, Type)>>,
    /// Return types of `impl` block methods, keyed by their call syntax
    /// (`Type::method`). Kept out of `functions` so that only type inference
    /// sees them and argument passing is unaffected.
    impl_methods: std::collections::HashMap<String, Option<Type>>,
    /// Map from original generic struct name to list of instantiations
    /// e.g., "Box" -> [("i64", "Box_i64"), ("bool", "Box_bool")]
    generic_struct_instantiation_map: std::collections::HashMap<String, Vec<(Vec<String>, String)>>,
    /// Set of async function names.
    ///
    /// DECLARED RESIDUAL, not fixed here: unlike `functions`, which the
    /// main-program pass OVERWRITES entry by entry, this is a set that is only
    /// ever inserted into. An imported `pub async fn f` shadowed by a local
    /// ordinary `fn f` therefore leaves `f` in here, and `try_infer_expr_type`
    /// types a call to the LOCAL `f` as `f_Future`. MEASURED: the emitted C
    /// carried `f_Future v = f();` beside `long long f()` and gcc reported
    /// `use of undeclared identifier 'f_Future'`. It is the same class as the
    /// prototype-loop defect — a decision about the imported program made
    /// without asking `local_definition_shadows_import` — one container over,
    /// and it belongs with the module-system defects owed to M4 rather than to
    /// this branch's scope.
    async_functions: std::collections::HashSet<String>,
    /// Names of struct tags actually emitted (structs + enums, both are
    /// `typedef struct Name {...} Name;`). Used to keep forward declarations
    /// from naming an incomplete tag, which would make the tag local to the
    /// prototype's parameter list and conflict with the definition.
    defined_structs: std::collections::HashSet<String>,
}

impl CodeGenerator {
    pub fn new(module_name: &str) -> Result<Self> {
        // Pre-allocate string capacity for better performance
        let initial_capacity = 64 * 1024; // 64KB initial capacity
        Ok(Self {
            current_fn_unit_return: None,
            module_name: module_name.to_string(),
            output: String::with_capacity(initial_capacity),
            functions: std::collections::HashMap::new(),
            variables: std::collections::HashMap::new(),
            array_bindings: std::collections::HashMap::new(),
            mutable_params: std::collections::HashMap::new(),
            imported_modules: std::collections::HashMap::new(),
            generic_instantiations: Vec::new(),
            generic_struct_instantiations: Vec::new(),
            type_aliases: std::collections::HashMap::new(),
            enums: std::collections::HashMap::new(),
            structs: std::collections::HashMap::new(),
            impl_methods: std::collections::HashMap::new(),
            generic_struct_instantiation_map: std::collections::HashMap::new(),
            async_functions: std::collections::HashSet::new(),
            defined_structs: std::collections::HashSet::new(),
        })
    }

    /// Set imported modules for code generation
    pub fn set_imported_modules(
        &mut self,
        modules: std::collections::HashMap<String, crate::resolver::ModuleInfo>,
    ) {
        // Imported bodies are emitted into the same translation unit as the
        // main program, so they need the same escape. Escaping the main program
        // alone would rename a local call and leave the imported definition it
        // resolves to spelled the old way.
        self.imported_modules = modules
            .into_iter()
            .map(|(name, mut info)| {
                info.ast = c_ident::escape_reserved_names(&info.ast);
                (name, info)
            })
            .collect();
    }

    /// Set generic function instantiations for code generation
    ///
    /// ESCAPED HERE, like the imported ASTs above, and for a sharper reason:
    /// these templates do not come from the `Program` that `compile` escapes.
    /// They come from the TYPE CHECKER, which is handed the unescaped AST
    /// (src/driver/mod.rs:109), and `monomorphize_function` clones their names,
    /// parameters and bodies straight into `generate_function`. Measured before
    /// this line existed: `fn pick<T>(register: T) -> T { let static: T = …; }`
    /// emitted `long long pick__i64(long long register)` and gcc refused it.
    ///
    /// The mangled name `mangle_generic_name` builds from the escaped one
    /// (`pick__i64`) needs no escape of its own: it always contains `__`, and
    /// nothing in `RESERVED` survives the trailing-underscore strip with an
    /// embedded one.
    pub fn set_generic_instantiations(
        &mut self,
        instantiations: Vec<(String, Vec<String>, crate::typeck::GenericFunction)>,
    ) {
        self.generic_instantiations = instantiations
            .into_iter()
            .map(|(name, type_args, f)| {
                let (name, f) = c_ident::escape_generic_function(&name, &f);
                (name, type_args, f)
            })
            .collect();
    }

    /// Set generic struct instantiations for code generation
    ///
    /// See `set_generic_instantiations` — same source, same bypass, same fix.
    pub fn set_generic_struct_instantiations(
        &mut self,
        instantiations: Vec<(String, Vec<String>, crate::typeck::GenericStruct)>,
    ) {
        self.generic_struct_instantiations = instantiations
            .into_iter()
            .map(|(name, type_args, s)| {
                let (name, s) = c_ident::escape_generic_struct(&name, &s);
                (name, type_args, s)
            })
            .collect();
    }

    /// Infer the C type of an expression, defaulting to `long long` when this
    /// pass has no rule for it.
    ///
    /// Only callers for which a wrong guess is harmless (picking between the
    /// string-concat and the arithmetic form of `+`, deciding whether a match
    /// scrutinee is an enum, …) may use this. A caller that *declares* a C
    /// variable with the result must use [`CodeGenerator::try_infer_expr_type`]
    /// and turn `None` into a diagnostic: defaulting a declaration to
    /// `long long` emits silently wrong C (a pointer or a struct stored in an
    /// integer), which only surfaces as a gcc error against generated code the
    /// user never wrote.
    fn infer_expr_type(&self, expr: &Expr) -> String {
        self.try_infer_expr_type(expr)
            .unwrap_or_else(|| "long long".to_string())
    }

    /// Infer the C type of an expression, or `None` when codegen has no rule
    /// for that expression kind.
    ///
    /// The returned string may carry array dimensions (`"long long[3]"`), which
    /// is the same encoding `self.variables` uses; split it with
    /// [`CodeGenerator::split_array_dims`] before emitting a declaration.
    fn try_infer_expr_type(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Integer(_) => Some("long long".to_string()),
            Expr::String(_) => Some("const char*".to_string()),
            Expr::Bool(_) => Some("int".to_string()),
            Expr::StructLiteral { name, fields, .. } => {
                // Check if this is a generic struct instantiation
                if let Some(instantiations) = self.generic_struct_instantiation_map.get(name) {
                    // Need to determine which instantiation to use based on field types
                    // For now, we'll infer from the first field's type
                    if let Some((_, field_expr)) = fields.first() {
                        let field_type = match field_expr {
                            Expr::Integer(_) => "long long",
                            Expr::String(_) => "const char*",
                            Expr::Bool(_) => "int",
                            _ => "long long",
                        };

                        // Find the matching instantiation
                        for (type_args, mangled_name) in instantiations {
                            // Simple heuristic: check if any type arg matches the field type
                            for type_arg in type_args {
                                if (type_arg == "i64" && field_type.contains("long long"))
                                    || (type_arg == "bool" && field_type == "int")
                                    || (type_arg == "String" && field_type.contains("char*"))
                                {
                                    return Some(format!("struct {}", mangled_name));
                                }
                            }
                        }
                    }
                    Some(format!("struct {}", name))
                } else {
                    Some(format!("struct {}", name))
                }
            }
            // Every binder (let, parameter, for-loop variable, match binding)
            // records its C type in `self.variables`, so a name we cannot find
            // means codegen genuinely does not know the type.
            Expr::Ident(name) => self.variables.get(name).cloned(),
            Expr::Call { func, args, .. } => {
                let Expr::Ident(func_name) = func.as_ref() else {
                    // Indirect calls are rejected by generate_expression anyway.
                    return None;
                };

                // The program's own signatures are the most specific answer, so
                // they are consulted before the built-in table.
                if let Some((params, ret_type)) = self.functions.get(func_name) {
                    // Check if this is an async function
                    if self.async_functions.contains(func_name) {
                        return Some(format!("{}_Future", func_name));
                    }
                    // `fn id<T>(x: T) -> T` returns whatever it was handed:
                    // monomorphization has not happened yet at this point, so
                    // recover the type argument from the matching argument.
                    if let Some(Type::TypeParam(param_name)) = ret_type.as_ref() {
                        let position = params
                            .iter()
                            .position(|p| matches!(&p.ty, Type::TypeParam(n) if n == param_name));
                        let arg = position.and_then(|i| args.get(i))?;
                        return self.try_infer_expr_type(arg);
                    }
                    return self.return_type_to_c(ret_type.as_ref());
                }

                // Methods declared in `impl` blocks are called as `Type::method`.
                if let Some(ret_type) = self.impl_methods.get(func_name) {
                    return self.return_type_to_c(ret_type.as_ref());
                }

                // Built-ins come from the single source of truth in crate::builtins
                // so this cannot drift from the type checker.
                if let Some(builtin) = crate::builtins::lookup(func_name) {
                    return Some(
                        match builtin.ret {
                            crate::builtins::BuiltinType::I64 => "long long",
                            crate::builtins::BuiltinType::Str => "const char*",
                            crate::builtins::BuiltinType::Bool => "int",
                            crate::builtins::BuiltinType::Unit => "void",
                        }
                        .to_string(),
                    );
                }

                None
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                match op {
                    // Comparisons and the logical connectives produce a bool,
                    // which is `int` in C - never the operand type.
                    BinOp::Eq
                    | BinOp::Ne
                    | BinOp::Lt
                    | BinOp::Gt
                    | BinOp::Le
                    | BinOp::Ge
                    | BinOp::And
                    | BinOp::Or => Some("int".to_string()),
                    BinOp::Add => {
                        // String concatenation returns a string
                        let left_type = self.infer_expr_type(left);
                        let right_type = self.infer_expr_type(right);
                        if left_type == "const char*" && right_type == "const char*" {
                            Some("const char*".to_string())
                        } else {
                            Some("long long".to_string())
                        }
                    }
                    _ => Some("long long".to_string()),
                }
            }
            Expr::Unary { op, operand, .. } => match op {
                UnaryOp::Not => Some("int".to_string()),
                UnaryOp::Neg => self.try_infer_expr_type(operand),
            },
            Expr::EnumConstructor { enum_name, .. } => {
                // generate_enum emits `typedef struct <Enum> { ... } <Enum>;`,
                // so `struct <Enum>` names the same type type_to_c() produces
                // for an explicit annotation.
                Some(format!("struct {}", enum_name))
            }
            Expr::Reference { expr, .. } => {
                let inner = self.try_infer_expr_type(expr)?;
                // A reference to an array needs C's pointer-to-array declarator
                // (`T (*p)[n]`), which the `let` printer cannot spell; refuse
                // instead of emitting a wrong one.
                if inner.contains('[') {
                    return None;
                }
                Some(format!("{}*", inner))
            }
            Expr::Deref { expr, .. } => {
                let inner = self.try_infer_expr_type(expr)?;
                // Strip one pointer level. A non-pointer operand means the
                // operand is a reference *parameter*, which Expr::Ident already
                // auto-dereferences, so the value type is the operand type.
                Some(match inner.strip_suffix('*') {
                    Some(pointee) => pointee.trim_end().to_string(),
                    None => inner,
                })
            }
            Expr::FieldAccess { object, field, .. } => {
                let object_type = self.try_infer_expr_type(object)?;
                let struct_name = Self::struct_name_of(&object_type)?;
                let fields = self.structs.get(struct_name)?;
                let (_, field_type) = fields.iter().find(|(name, _)| name == field)?;
                Some(self.type_to_c(field_type))
            }
            Expr::Index { array, .. } => {
                let array_type = self.try_infer_expr_type(array)?;
                // `T[n]` -> `T`, `T*` -> `T`, `const char*` -> `char`.
                if let Some(open) = array_type.find('[') {
                    let base = array_type[..open].trim_end();
                    let rest = &array_type[open..];
                    // Drop the outermost dimension, keeping any inner ones.
                    let close = rest.find(']')?;
                    return Some(format!("{}{}", base, &rest[close + 1..]));
                }
                if array_type == "const char*" {
                    return Some("char".to_string());
                }
                let pointee = array_type.strip_suffix('*')?;
                Some(pointee.trim_end().to_string())
            }
            Expr::ArrayLiteral { elements, .. } => {
                let elem_type = match elements.first() {
                    Some(first) => self.try_infer_expr_type(first)?,
                    None => "long long".to_string(),
                };
                Some(Self::array_of(&elem_type, elements.len()))
            }
            Expr::ArrayRepeat { value, count, .. } => {
                let elem_type = self.try_infer_expr_type(value)?;
                let size = match count.as_ref() {
                    Expr::Integer(n) => *n as usize,
                    // Non-literal counts are rejected by the type checker.
                    _ => 0,
                };
                Some(Self::array_of(&elem_type, size))
            }
            // No rule yet: ranges are only meaningful inside `for`, `?` and
            // macros are lowered elsewhere, and await/async is unimplemented.
            Expr::Range { .. }
            | Expr::Question { .. }
            | Expr::MacroInvocation { .. }
            | Expr::Await { .. } => None,
        }
    }

    /// Record the return type of every method in an `impl` block under the name
    /// its call sites use (`Type::method`), resolving `Self` to the impl type.
    ///
    /// Takes the map rather than `&mut self` so it can be called while another
    /// field of the generator (the imported-module list) is borrowed.
    fn collect_impl_method_types(
        impl_methods: &mut std::collections::HashMap<String, Option<Type>>,
        impl_block: &ImplBlock,
    ) {
        if !impl_block.type_params.is_empty() {
            // Generic impls are monomorphized elsewhere; their return types are
            // not knowable from the template.
            return;
        }
        let for_type = impl_block.for_type.to_string();
        for method in &impl_block.methods {
            if !method.type_params.is_empty() {
                continue;
            }
            let ret = method.return_type.clone().map(|ty| match ty {
                Type::Custom(name) if name == "Self" => impl_block.for_type.clone(),
                other => other,
            });
            impl_methods.insert(format!("{}::{}", for_type, method.name), ret);
        }
    }

    /// C type of a function's declared return type, `void` for no return type.
    fn return_type_to_c(&self, ret_type: Option<&Type>) -> Option<String> {
        match ret_type {
            None | Some(Type::Unit) => Some("void".to_string()),
            // A generic return type is only known after monomorphization, which
            // this pass does not track per call site.
            Some(Type::TypeParam(_)) | Some(Type::Generic { .. }) => None,
            Some(ty) => Some(self.type_to_c(ty)),
        }
    }

    /// The struct/enum tag named by a C type string, if it names one:
    /// `"struct Point"` / `"struct Point*"` / `"Point"` -> `"Point"`.
    fn struct_name_of(c_type: &str) -> Option<&str> {
        let name = c_type.trim_end_matches(['*', ' ']);
        let name = name.strip_prefix("struct ").unwrap_or(name);
        if name.is_empty() || name.contains('[') {
            None
        } else {
            Some(name)
        }
    }

    /// Add an outermost array dimension to a (possibly already array) type:
    /// `("long long", 3)` -> `"long long[3]"`, `("long long[2]", 3)` ->
    /// `"long long[3][2]"`.
    fn array_of(elem_type: &str, size: usize) -> String {
        let (base, inner_dims) = Self::split_array_dims(elem_type);
        format!("{}[{}]{}", base, size, inner_dims)
    }

    /// Split an inferred C type into its base type and its array dimensions:
    /// `"long long[3][2]"` -> `("long long", "[3][2]")`.
    fn split_array_dims(c_type: &str) -> (String, String) {
        match c_type.find('[') {
            Some(i) => (
                c_type[..i].trim_end().to_string(),
                c_type[i..].replace(' ', ""),
            ),
            None => (c_type.to_string(), String::new()),
        }
    }

    /// The length a declared array size states, keeping the distinction between
    /// "the author wrote 0" and "this pass could not work the length out".
    fn array_len_of_size(size: &ArraySize) -> ArrayLen {
        match size {
            ArraySize::Literal(n) => ArrayLen::Proven(*n),
            // Const generics parse but are dropped (language-spec.md §5), so
            // the name is all that is left of the length.
            ArraySize::ConstParam(name) => ArrayLen::Unproven(name.clone()),
            ArraySize::Expr(_) => ArrayLen::Unproven("<size expression>".to_string()),
        }
    }

    /// The variable an assignment ultimately writes into: `xs[i].f` -> `xs`.
    fn assign_target_root(target: &AssignTarget) -> Option<&str> {
        match target {
            AssignTarget::Ident(name) => Some(name),
            AssignTarget::Index { array, .. } => Self::expr_root_ident(array),
            AssignTarget::FieldAccess { object, .. } => Self::expr_root_ident(object),
            AssignTarget::Deref { expr } => Self::expr_root_ident(expr),
        }
    }

    /// The variable at the base of a place expression, if it has one.
    fn expr_root_ident(expr: &Expr) -> Option<&str> {
        match expr {
            Expr::Ident(name) => Some(name),
            Expr::Index { array, .. } => Self::expr_root_ident(array),
            Expr::FieldAccess { object, .. } => Self::expr_root_ident(object),
            Expr::Deref { expr, .. } => Self::expr_root_ident(expr),
            _ => None,
        }
    }

    /// Reject a write into an array parameter that did not declare that it may
    /// be written.
    ///
    /// Every array parameter decays to a pointer into the *caller's* array, so
    /// a write is visible to the caller whatever the parameter was spelled.
    /// `&mut [T; N]` and `mut xs: [T; N]` say that is intended. A bare
    /// `[T; N]` does not, and the language has not decided whether it copies or
    /// aliases: docs/specification/language-spec.md §9 defines the memory model
    /// without mentioning array parameters, and §5 records that the typechecker
    /// cannot tell `&T` from `T` at all. Emitting the aliasing write would
    /// silently pick one of the two answers, so it is refused until the
    /// specification picks.
    fn check_array_write(&self, name: &str) -> Result<()> {
        let Some(binding) = self.array_bindings.get(name) else {
            return Ok(());
        };
        let ArrayStorage::Parameter(form) = binding.storage else {
            return Ok(());
        };
        match form {
            ArrayParamForm::Mutable | ArrayParamForm::MutByValue => Ok(()),
            ArrayParamForm::Shared => Err(CompileError::CodegenError {
                message: format!(
                    "cannot write to `{}`: it is a shared reference parameter, \
                     `&[T; N]`, and a shared reference does not permit mutation. \
                     Declare it `&mut [T; N]` if this function is meant to modify \
                     the caller's array.",
                    name
                ),
            }),
            ArrayParamForm::ByValue => Err(CompileError::CodegenError {
                message: format!(
                    "cannot write to `{}`: it is a by-value array parameter, but C \
                     decays every array parameter to a pointer, so the write would \
                     reach the caller's array rather than a copy. Whether `[T; N]` \
                     parameters copy or alias is not decided by the language \
                     specification, so this is refused instead of guessing. Declare \
                     `{}: &mut [T; N]` (or `mut {}: [T; N]`) to modify the caller's \
                     array.",
                    name, name, name
                ),
            }),
        }
    }

    /// Whether an argument denotes an array, as far as this pass can tell.
    ///
    /// Deliberately generous: a name with an array binding, or any expression
    /// whose inferred C type carries dimensions (a struct field of array type,
    /// for instance). Over-answering `true` costs a refusal, which is
    /// recoverable; under-answering it costs a silent capability leak.
    fn is_array_argument(&self, expr: &Expr) -> bool {
        let place = match expr {
            Expr::Reference { expr, .. } => expr.as_ref(),
            other => other,
        };
        // Only the name itself is the array. Its *root* is not a usable test:
        // `arr[0]` roots to `arr` but is an element, and treating it as an
        // array refused `print_int(arr[0])` across the corpus.
        if let Expr::Ident(name) = place {
            if self.array_bindings.contains_key(name.as_str()) {
                return true;
            }
        }
        self.try_infer_expr_type(place)
            .is_some_and(|ty| ty.contains('['))
    }

    /// Whether a parameter may write into the caller's array through the
    /// pointer it receives: `&mut [T; N]`, or `mut xs: [T; N]`.
    fn param_grants_array_write(param: &Param) -> bool {
        match &param.ty {
            Type::Array(_, _) => param.mutable,
            Type::Reference { inner, mutable, .. } => {
                *mutable && matches!(inner.as_ref(), Type::Array(_, _))
            }
            _ => false,
        }
    }

    /// Reject a call that hands a callee more write capability over an array
    /// than the caller itself holds.
    ///
    /// Refusing the *assignment* is not enough on its own: nothing between the
    /// front end and here re-checks a reference's mutability - the typechecker
    /// drops it (`src/typeck/mod.rs:2794-2794`, `mutable: _`) and the borrow checker
    /// gives every parameter a plain owned place
    /// (`src/ownership/borrow_checker.rs:548`). So `fn f(xs: &[i64; 3])` could
    /// call `fn mutate(xs: &mut [i64; 3])` and have the write performed under
    /// the callee's mutable binding, where it is legitimate. Measured, before
    /// this check: the caller's `v[0]` came back 99 through both a shared and a
    /// bare array parameter. Capability has to be checked where it is passed,
    /// not only where it is used.
    fn check_call_array_capabilities(&self, func_name: &str, args: &[Expr]) -> Result<()> {
        let Some((params, _)) = self.functions.get(func_name) else {
            // No signature for this callee, so there is no way to tell whether
            // it writes through an array it is handed. `impl` methods are the
            // reachable case: they are called as `Type::method` and are kept
            // out of `functions` deliberately, so the guard used to skip them
            // entirely and a shared array could be lent to a method that
            // mutates it. Unknown capability is refused rather than allowed.
            for arg in args {
                if self.is_array_argument(arg) {
                    return Err(CompileError::CodegenError {
                        message: format!(
                            "cannot pass an array to `{}`: this compiler does not know that \
                             callee's parameter list, so it cannot tell whether `{}` writes \
                             through the array - and every array is passed as a pointer into \
                             the caller's storage. Allowing it would make the array write rule \
                             (docs/specification/language-spec.md §9.2) unenforceable for this \
                             call. Call a plain `fn` with a declared array parameter instead; \
                             `impl` methods and imported callees cannot take arrays yet.",
                            func_name, func_name
                        ),
                    });
                }
            }
            return Ok(());
        };
        for (i, param) in params.iter().enumerate() {
            let Some(arg) = args.get(i) else { break };
            if !Self::param_grants_array_write(param) {
                continue;
            }
            // `&mut xs` and a bare `xs` reach the callee identically, so both
            // are judged by what the referent is.
            let place = match arg {
                Expr::Reference { expr, .. } => expr.as_ref(),
                other => other,
            };
            // The argument's own provenance decides, and only a *name* has one
            // this pass can state. Anything else - a struct field, an element,
            // a call result - does not, and "cannot tell" must not read as
            // "may". `mutate(s.array)` used to root to `s`, find no binding,
            // and be waved through even when `s` was a shared parameter.
            //
            // Deliberately not `expr_root_ident`: rooting an *element* at its
            // array (`grid[0]` -> `grid`) would let the element inherit the
            // array's capability, which is a different, more permissive rule
            // than the one §9.2 states. Inheriting is defensible, but it is not
            // what the specification promises a reader, and the specification
            // is the contract.
            let root = match place {
                Expr::Ident(name) => Some(name.as_str()),
                _ => None,
            };
            let binding = root.and_then(|r| self.array_bindings.get(r));
            let Some(binding) = binding else {
                if !self.is_array_argument(arg) {
                    // Not an array at all: nothing to lend.
                    continue;
                }
                return Err(CompileError::CodegenError {
                    message: format!(
                        "cannot pass this argument to `{}`: the parameter `{}` may write to \
                         the caller's array, and this compiler cannot establish where the \
                         array came from ({}), so it cannot tell whether the write is \
                         permitted. Every array is passed as a pointer into the caller's \
                         storage, so guessing would silently break the array write rule \
                         (docs/specification/language-spec.md §9.2). Bind the array to a \
                         local or a parameter first and pass that name.",
                        func_name,
                        param.name,
                        Self::expr_kind_name(place)
                    ),
                });
            };
            let ArrayStorage::Parameter(form) = binding.storage else {
                continue;
            };
            let held = match form {
                ArrayParamForm::Mutable | ArrayParamForm::MutByValue => continue,
                ArrayParamForm::Shared => "a shared reference parameter, `&[T; N]`",
                ArrayParamForm::ByValue => "a by-value array parameter, `[T; N]`",
            };
            // A binding was found, so the argument had a root name.
            let root = root.unwrap_or("this argument");
            return Err(CompileError::CodegenError {
                message: format!(
                    "cannot pass `{}` to `{}`: the parameter `{}` may write to the \
                     caller's array, but `{}` is {} here, which does not carry that \
                     permission. Passing it on would let `{}` perform a write that \
                     `{}` is not allowed to perform itself. Declare `{}: &mut [T; N]`.",
                    root, func_name, param.name, root, held, func_name, root, root
                ),
            });
        }
        Ok(())
    }

    /// The length of the array an expression denotes, or `None` when the
    /// expression is not a known array binding.
    fn array_len_of_expr(&self, expr: &Expr) -> Option<ArrayLen> {
        match expr {
            Expr::Ident(name) => self.array_bindings.get(name).map(|b| b.len.clone()),
            Expr::ArrayLiteral { elements, .. } => Some(ArrayLen::Proven(elements.len())),
            Expr::ArrayRepeat { count, .. } => Some(match count.as_ref() {
                Expr::Integer(n) if *n >= 0 => ArrayLen::Proven(*n as usize),
                other => ArrayLen::Unproven(Self::expr_kind_name(other).to_string()),
            }),
            _ => None,
        }
    }

    /// Human-readable name of an expression kind, for diagnostics.
    fn expr_kind_name(expr: &Expr) -> &'static str {
        match expr {
            Expr::String(_) => "string literal",
            Expr::Integer(_) => "integer literal",
            Expr::Bool(_) => "boolean literal",
            Expr::Ident(_) => "variable reference",
            Expr::ArrayLiteral { .. } => "array literal",
            Expr::ArrayRepeat { .. } => "array repeat",
            Expr::Index { .. } => "array index",
            Expr::Call { .. } => "function call",
            Expr::Binary { .. } => "binary operation",
            Expr::Unary { .. } => "unary operation",
            Expr::StructLiteral { .. } => "struct literal",
            Expr::FieldAccess { .. } => "field access",
            Expr::EnumConstructor { .. } => "enum constructor",
            Expr::Range { .. } => "range",
            Expr::Reference { .. } => "reference",
            Expr::Deref { .. } => "dereference",
            Expr::Question { .. } => "`?` operator",
            Expr::MacroInvocation { .. } => "macro invocation",
            Expr::Await { .. } => "await",
        }
    }

    /// Compile an AST to machine code
    ///
    /// The escape runs HERE rather than in the driver so it cannot be bypassed:
    /// every route into code generation goes through this method, and a caller
    /// that built a `CodeGenerator` directly would otherwise emit C in which
    /// `fn double(x: i64)` is `long long double(long long x)`. See
    /// `c_ident::escape_reserved_names` for what it renames and what it leaves.
    pub fn compile(&mut self, program: &Program) -> Result<()> {
        let program = c_ident::escape_reserved_names(program);
        self.compile_escaped(&program)
    }

    fn compile_escaped(&mut self, program: &Program) -> Result<()> {
        // For v0.1, we'll generate a simple C file that we can compile with gcc
        // This is a temporary solution until LLVM integration is complete

        self.output.push_str("#include <stdio.h>\n");
        self.output.push_str("#include <string.h>\n");
        self.output.push_str("#include <stdlib.h>\n");
        self.output.push_str("#include <ctype.h>\n");
        self.output.push_str("#include <stdint.h>\n\n");

        // Memory management for strings
        self.output
            .push_str("// String memory pool to prevent leaks\n");
        self.output.push_str("#define STRING_POOL_SIZE 65536\n");
        self.output.push_str("#define MAX_STRINGS 1024\n");
        self.output
            .push_str("static char __pd_string_pool[STRING_POOL_SIZE];\n");
        self.output
            .push_str("static size_t __pd_string_pool_offset = 0;\n");
        self.output
            .push_str("static char* __pd_allocated_strings[MAX_STRINGS];\n");
        self.output.push_str("static int __pd_num_strings = 0;\n\n");

        // String allocation function
        self.output
            .push_str("static char* __pd_alloc_string(size_t size) {\n");
        self.output
            .push_str("    if (__pd_string_pool_offset + size > STRING_POOL_SIZE) {\n");
        self.output
            .push_str("        // Pool exhausted, fall back to malloc\n");
        self.output
            .push_str("        char* ptr = (char*)malloc(size);\n");
        self.output
            .push_str("        if (__pd_num_strings < MAX_STRINGS) {\n");
        self.output
            .push_str("            __pd_allocated_strings[__pd_num_strings++] = ptr;\n");
        self.output.push_str("        }\n");
        self.output.push_str("        return ptr;\n");
        self.output.push_str("    }\n");
        self.output
            .push_str("    char* ptr = &__pd_string_pool[__pd_string_pool_offset];\n");
        self.output
            .push_str("    __pd_string_pool_offset += size;\n");
        self.output.push_str("    return ptr;\n");
        self.output.push_str("}\n\n");

        // AN OWNED EMPTY STRING. `src/builtins.rs` declares seven builtins
        // `ReturnMode::Owned`, and the ownership pass derives its signatures from
        // that table (src/ownership/borrow_checker.rs:112). Four of them —
        // string_substring, file_read_all, file_read_line, read_file_to_string —
        // had REACHABLE branches returning the literal `""`, which is static
        // storage the builtin did not allocate. The declaration was false against
        // the code on a live path, and the guard that should have caught it
        // compared `Owned` against the declared `Effect::Memory`: two metadata
        // fields in the same table agreeing with each other.
        //
        // Fixed by making the declaration TRUE rather than by weakening it to a
        // conditional-ownership model the language has no way to express.
        // MEASURED, because allocating on a failure path is not free: this goes
        // through __pd_alloc_string like every other owned string, so it takes one
        // byte from the 64KB bump pool and introduces NO failure class that the
        // other owned returns do not already have — a pool-exhausted malloc that
        // fails returns NULL there too. `strdup` would have added libc's own
        // failure mode for nothing.
        self.output
            .push_str("static const char* __pd_empty_owned() {\n");
        self.output
            .push_str("    char* s = __pd_alloc_string(1);\n");
        self.output.push_str("    if (s) s[0] = '\\0';\n");
        self.output.push_str("    return s;\n");
        self.output.push_str("}\n\n");

        // Cleanup function
        self.output
            .push_str("static void __pd_cleanup_strings() {\n");
        self.output
            .push_str("    for (int i = 0; i < __pd_num_strings; i++) {\n");
        self.output
            .push_str("        free(__pd_allocated_strings[i]);\n");
        self.output.push_str("    }\n");
        self.output.push_str("    __pd_num_strings = 0;\n");
        self.output.push_str("    __pd_string_pool_offset = 0;\n");
        self.output.push_str("}\n\n");

        // Register cleanup with atexit
        self.output
            .push_str("static void __pd_init() __attribute__((constructor));\n");
        self.output.push_str("static void __pd_init() {\n");
        self.output.push_str("    atexit(__pd_cleanup_strings);\n");
        self.output.push_str("}\n\n");

        // Command-line arguments, captured by main() on entry
        self.output.push_str("static int __pd_argc = 0;\n");
        self.output.push_str("static char** __pd_argv = 0;\n\n");

        // arg_count
        self.output.push_str("long long __pd_arg_count(void) {\n");
        self.output.push_str("    return (long long)__pd_argc;\n");
        self.output.push_str("}\n\n");

        // arg_at
        self.output
            .push_str("const char* __pd_arg_at(long long i) {\n");
        self.output
            .push_str("    if (i < 0 || i >= (long long)__pd_argc) return \"\";\n");
        self.output.push_str("    return __pd_argv[i];\n");
        self.output.push_str("}\n\n");

        // Generate print function wrapper
        self.output.push_str("void __pd_print(const char* str) {\n");
        self.output.push_str("    printf(\"%s\\n\", str);\n");
        self.output.push_str("}\n\n");

        // Generate print_int function wrapper
        self.output
            .push_str("void __pd_print_int(long long value) {\n");
        self.output.push_str("    printf(\"%lld\\n\", value);\n");
        self.output.push_str("}\n\n");

        // Generate panic function wrapper
        self.output.push_str("void __pd_panic(const char* msg) {\n");
        self.output.push_str("    fprintf(stderr, \"panic: %s\\n\", msg);\n");
        self.output.push_str("    abort();\n");
        self.output.push_str("}\n\n");

        // Generate string manipulation functions

        // string_len
        self.output
            .push_str("long long __pd_string_len(const char* str) {\n");
        self.output.push_str("    return strlen(str);\n");
        self.output.push_str("}\n\n");

        // string_concat
        self.output
            .push_str("const char* __pd_string_concat(const char* s1, const char* s2) {\n");
        self.output.push_str("    size_t len1 = strlen(s1);\n");
        self.output.push_str("    size_t len2 = strlen(s2);\n");
        self.output
            .push_str("    char* result = __pd_alloc_string(len1 + len2 + 1);\n");
        self.output.push_str("    strcpy(result, s1);\n");
        self.output.push_str("    strcat(result, s2);\n");
        self.output.push_str("    return result;\n");
        self.output.push_str("}\n\n");

        // string_eq
        self.output
            .push_str("int __pd_string_eq(const char* s1, const char* s2) {\n");
        self.output.push_str("    return strcmp(s1, s2) == 0;\n");
        self.output.push_str("}\n\n");

        // string_char_at
        self.output
            .push_str("long long __pd_string_char_at(const char* str, long long index) {\n");
        self.output
            .push_str("    if (index < 0 || index >= (long long)strlen(str)) return -1;\n");
        self.output
            .push_str("    return (long long)(unsigned char)str[index];\n");
        self.output.push_str("}\n\n");

        // string_substring
        self.output.push_str("const char* __pd_string_substring(const char* str, long long start, long long end) {\n");
        self.output.push_str("    size_t len = strlen(str);\n");
        self.output.push_str("    if (start < 0) start = 0;\n");
        self.output
            .push_str("    if (end > (long long)len) end = len;\n");
        self.output.push_str("    if (start >= end) return __pd_empty_owned();\n");
        self.output.push_str("    size_t sub_len = end - start;\n");
        self.output
            .push_str("    char* result = __pd_alloc_string(sub_len + 1);\n");
        self.output
            .push_str("    strncpy(result, str + start, sub_len);\n");
        self.output.push_str("    result[sub_len] = '\\0';\n");
        self.output.push_str("    return result;\n");
        self.output.push_str("}\n\n");

        // string_from_char
        self.output
            .push_str("const char* __pd_string_from_char(long long c) {\n");
        self.output
            .push_str("    char* result = __pd_alloc_string(2);\n");
        self.output.push_str("    result[0] = (char)c;\n");
        self.output.push_str("    result[1] = '\\0';\n");
        self.output.push_str("    return result;\n");
        self.output.push_str("}\n\n");

        // char_is_digit
        self.output
            .push_str("int __pd_char_is_digit(long long c) {\n");
        self.output.push_str("    return isdigit((int)c);\n");
        self.output.push_str("}\n\n");

        // char_is_alpha
        self.output
            .push_str("int __pd_char_is_alpha(long long c) {\n");
        self.output.push_str("    return isalpha((int)c);\n");
        self.output.push_str("}\n\n");

        // char_is_whitespace
        self.output
            .push_str("int __pd_char_is_whitespace(long long c) {\n");
        self.output.push_str("    return isspace((int)c);\n");
        self.output.push_str("}\n\n");

        // string_to_int
        self.output
            .push_str("long long __pd_string_to_int(const char* str) {\n");
        self.output.push_str("    return atoll(str);\n");
        self.output.push_str("}\n\n");

        // int_to_string
        self.output
            .push_str("const char* __pd_int_to_string(long long n) {\n");
        self.output
            .push_str("    char* buffer = __pd_alloc_string(32);\n");
        self.output
            .push_str("    snprintf(buffer, 32, \"%lld\", n);\n");
        self.output.push_str("    return buffer;\n");
        self.output.push_str("}\n\n");

        // File I/O functions
        self.output.push_str("// File I/O support\n");
        self.output.push_str("#define MAX_FILES 256\n");
        self.output
            .push_str("static FILE* __pd_file_handles[MAX_FILES] = {0};\n");
        self.output.push_str("static int __pd_next_handle = 1;\n\n");

        // file_open
        self.output
            .push_str("long long __pd_file_open(const char* path) {\n");
        self.output
            .push_str("    if (__pd_next_handle >= MAX_FILES) return -1;\n");
        self.output.push_str("    FILE* f = fopen(path, \"r+\");\n");
        self.output
            .push_str("    if (!f) f = fopen(path, \"w+\");\n");
        self.output.push_str("    if (!f) return -1;\n");
        self.output
            .push_str("    int handle = __pd_next_handle++;\n");
        self.output.push_str("    __pd_file_handles[handle] = f;\n");
        self.output.push_str("    return handle;\n");
        self.output.push_str("}\n\n");

        // file_read_all
        self.output
            .push_str("const char* __pd_file_read_all(long long handle) {\n");
        self.output.push_str("    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return __pd_empty_owned();\n");
        self.output
            .push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output.push_str("    fseek(f, 0, SEEK_END);\n");
        self.output.push_str("    long size = ftell(f);\n");
        self.output.push_str("    fseek(f, 0, SEEK_SET);\n");
        self.output
            .push_str("    char* buffer = __pd_alloc_string(size + 1);\n");
        self.output.push_str("    fread(buffer, 1, size, f);\n");
        self.output.push_str("    buffer[size] = '\\0';\n");
        self.output.push_str("    return buffer;\n");
        self.output.push_str("}\n\n");

        // file_read_line
        self.output
            .push_str("const char* __pd_file_read_line(long long handle) {\n");
        self.output.push_str("    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return __pd_empty_owned();\n");
        self.output.push_str("    static char line_buffer[4096];\n");
        self.output
            .push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output
            .push_str("    if (fgets(line_buffer, sizeof(line_buffer), f)) {\n");
        self.output
            .push_str("        size_t len = strlen(line_buffer);\n");
        self.output.push_str(
            "        if (len > 0 && line_buffer[len-1] == '\\n') line_buffer[len-1] = '\\0';\n",
        );
        self.output
            .push_str("        char* result = __pd_alloc_string(len + 1);\n");
        self.output
            .push_str("        strcpy(result, line_buffer);\n");
        self.output.push_str("        return result;\n");
        self.output.push_str("    }\n");
        self.output.push_str("    return __pd_empty_owned();\n");
        self.output.push_str("}\n\n");

        // file_write
        self.output
            .push_str("int __pd_file_write(long long handle, const char* content) {\n");
        self.output.push_str(
            "    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return 0;\n",
        );
        self.output
            .push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output.push_str("    return fputs(content, f) >= 0;\n");
        self.output.push_str("}\n\n");

        // file_close
        self.output
            .push_str("int __pd_file_close(long long handle) {\n");
        self.output.push_str(
            "    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return 0;\n",
        );
        self.output
            .push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output
            .push_str("    __pd_file_handles[handle] = NULL;\n");
        self.output.push_str("    return fclose(f) == 0;\n");
        self.output.push_str("}\n\n");

        // file_exists
        self.output
            .push_str("int __pd_file_exists(const char* path) {\n");
        self.output.push_str("    FILE* f = fopen(path, \"r\");\n");
        self.output.push_str("    if (f) {\n");
        self.output.push_str("        fclose(f);\n");
        self.output.push_str("        return 1;\n");
        self.output.push_str("    }\n");
        self.output.push_str("    return 0;\n");
        self.output.push_str("}\n\n");

        // Enhanced I/O Runtime Function Declarations
        // External function declarations for runtime I/O
        //
        // THE `FileHandle` (typedef void*) API IS GONE FROM THIS PRELUDE, 2026-08-23.
        // It existed to back six builtins that could not be called: this compiler
        // types every file handle as `i64` (`src/builtins.rs`), an `i64` cannot hold
        // an opaque pointer, and so every one of the six was refused at typecheck.
        // Four of them — `file_open_ex`, `file_close_ex`, `file_read_ex`,
        // `file_write_ex` — are not in N14 and left the registry; their wrappers are
        // deleted here, and with them the last reference to `FileHandle`, the
        // `FileMode` enum and the `pd_file_open/close/read/write/seek/flush` externs.
        // The other two are normative and are LOWERED ONTO `__pd_file_handles`
        // below, beside `__pd_file_write` and `__pd_file_close`, which is the handle
        // representation the language actually has.
        self.output.push_str("// External runtime I/O functions\n");
        self.output.push_str("extern int pd_path_exists(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_path_is_file(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_path_is_dir(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_create_dir(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_create_dir_all(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_remove_file(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_remove_dir(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_remove_dir_all(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_read_file_to_string(const char* path, size_t path_len, char** out_str, size_t* out_len);\n");
        self.output.push_str("extern int pd_write_string_to_file(const char* path, size_t path_len, const char* data, size_t data_len);\n\n");
        
        // Wrapper functions that call the external pd_* functions

        // file_seek, over the SAME `long long` handle table as file_write and
        // file_close. `whence` is the Palladium-level 0/1/2 that
        // src/runtime/io.rs::pd_file_seek also uses, mapped here to the C
        // constants rather than passed through: an unrecognised value is -1, not
        // an out-of-range seek. Returns the new absolute position, or -1.
        self.output
            .push_str("long long __pd_file_seek(long long handle, long long whence, long long offset) {\n");
        self.output.push_str(
            "    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return -1;\n",
        );
        // Written as brace-free statements each ending in `;`, like every other
        // wrapper in this prelude. That is not a style preference:
        // scripts/gate_probe.py's structural reader parses the emitted C on that
        // invariant, and a single-line `if (…) { … }` here made it report
        // MALFUNCTION on all seven stdlib drivers — a checker that cannot account
        // for a shape stops analysing, which it says rather than guessing.
        self.output
            .push_str("    if (whence != 0 && whence != 1 && whence != 2) return -1;\n");
        self.output.push_str(
            "    int w = whence == 0 ? SEEK_SET : (whence == 1 ? SEEK_CUR : SEEK_END);\n",
        );
        self.output
            .push_str("    FILE* f = __pd_file_handles[handle];\n");
        self.output
            .push_str("    if (fseek(f, (long)offset, w) != 0) return -1;\n");
        self.output.push_str("    return (long long)ftell(f);\n");
        self.output.push_str("}\n\n");

        // file_flush. 1 on success, 0 on failure — the convention its siblings
        // __pd_file_write and __pd_file_close already use, NOT the 0-ok/-1-fail
        // of the deleted `pd_file_flush`. N14 spells both of these as
        // `Result<…, IoError>`; `Result` is not built in yet, and this divergence
        // is recorded in the specification's A8 with the rest of the family.
        self.output
            .push_str("long long __pd_file_flush(long long handle) {\n");
        self.output.push_str(
            "    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return 0;\n",
        );
        self.output
            .push_str("    return fflush(__pd_file_handles[handle]) == 0;\n");
        self.output.push_str("}\n\n");

        // Path manipulation functions
        self.output.push_str("int __pd_path_exists(const char* path) {\n");
        self.output.push_str("    return pd_path_exists(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_path_is_file(const char* path) {\n");
        self.output.push_str("    return pd_path_is_file(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_path_is_dir(const char* path) {\n");
        self.output.push_str("    return pd_path_is_dir(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        // Directory operations
        self.output.push_str("int __pd_create_dir(const char* path) {\n");
        self.output.push_str("    return pd_create_dir(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_create_dir_all(const char* path) {\n");
        self.output.push_str("    return pd_create_dir_all(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_remove_file(const char* path) {\n");
        self.output.push_str("    return pd_remove_file(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_remove_dir(const char* path) {\n");
        self.output.push_str("    return pd_remove_dir(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_remove_dir_all(const char* path) {\n");
        self.output.push_str("    return pd_remove_dir_all(path, strlen(path));\n");
        self.output.push_str("}\n\n");
        
        // Enhanced file operations with string helpers
        self.output.push_str("char* __pd_read_file_to_string(const char* path) {\n");
        self.output.push_str("    char* out_str = NULL;\n");
        self.output.push_str("    size_t out_len = 0;\n");
        self.output.push_str("    if (pd_read_file_to_string(path, strlen(path), &out_str, &out_len) == 0) {\n");
        self.output.push_str("        return out_str;\n");
        self.output.push_str("    }\n");
        // Failure returns the empty string, never NULL: a Palladium String is a
        // non-NULL const char* and every string built-in dereferences it at once
        // (string_len -> strlen). Returning NULL here made a missing file a
        // SIGSEGV rather than an error the program could see. This matches
        // __pd_arg_at, which returns "" out of range for the same reason.
        self.output.push_str("    return __pd_empty_owned();\n");
        self.output.push_str("}\n\n");
        
        self.output.push_str("int __pd_write_string_to_file(const char* path, const char* data) {\n");
        self.output.push_str("    return pd_write_string_to_file(path, strlen(path), data, strlen(data));\n");
        self.output.push_str("}\n\n");

        // First pass: collect function signatures, type aliases, and enum definitions from imported modules
        //
        // Sorted by module name, not `HashMap` order. `RandomState` reseeds per
        // process, so iterating the map directly made the emitted C depend on the
        // hash seed: eight compiles of one unchanged two-module program produced
        // two distinct outputs, four each, differing only in the order of the
        // imported declarations. `make selfhost`'s fixed point is a claim about
        // byte identity, and it holds today only because `bootstrap/pdc.pd`
        // imports nothing — so this must be ordered before modules can appear in
        // the self-hosting compiler at all.
        let mut sorted_modules: Vec<_> = self.imported_modules.iter().collect();
        sorted_modules.sort_by_key(|(name, _)| *name);
        for (_, module_info) in sorted_modules {
            for item in &module_info.ast.items {
                match item {
                    Item::Function(func) => {
                        if matches!(func.visibility, crate::ast::Visibility::Public) {
                            self.functions.insert(
                                func.name.clone(),
                                (func.params.clone(), func.return_type.clone()),
                            );
                            if func.is_async {
                                self.async_functions.insert(func.name.clone());
                            }
                        }
                    }
                    Item::TypeAlias(type_alias) => {
                        if matches!(type_alias.visibility, crate::ast::Visibility::Public) {
                            // Skip generic type aliases for now
                            if type_alias.type_params.is_empty()
                                && type_alias.lifetime_params.is_empty()
                            {
                                self.type_aliases
                                    .insert(type_alias.name.clone(), type_alias.ty.clone());
                            }
                        }
                    }
                    Item::Enum(enum_def) => {
                        // Skip generic enums for now
                        if enum_def.type_params.is_empty() && enum_def.lifetime_params.is_empty() {
                            self.enums.insert(enum_def.name.clone(), enum_def.clone());
                        }
                    }
                    Item::Impl(impl_block) => {
                        Self::collect_impl_method_types(&mut self.impl_methods, impl_block);
                    }
                    Item::Macro(_) => {
                        // Macros are expanded before codegen, skip here
                    }
                    _ => {}
                }
            }
        }

        // Then collect function signatures, type aliases, and enum definitions from main program
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.functions.insert(
                        func.name.clone(),
                        (func.params.clone(), func.return_type.clone()),
                    );
                    if func.is_async {
                        self.async_functions.insert(func.name.clone());
                    }
                }
                Item::TypeAlias(type_alias) => {
                    // Skip generic type aliases for now
                    if type_alias.type_params.is_empty() && type_alias.lifetime_params.is_empty() {
                        self.type_aliases
                            .insert(type_alias.name.clone(), type_alias.ty.clone());
                    }
                }
                Item::Enum(enum_def) => {
                    // Skip generic enums for now
                    if enum_def.type_params.is_empty() && enum_def.lifetime_params.is_empty() {
                        self.enums.insert(enum_def.name.clone(), enum_def.clone());
                    }
                }
                Item::Impl(impl_block) => {
                    Self::collect_impl_method_types(&mut self.impl_methods, impl_block);
                }
                Item::Macro(_) => {
                    // Macros are expanded before codegen, skip here
                }
                _ => {}
            }
        }

        // Generate struct definitions from imported modules first.
        //
        // Sorted by module name for the same reason as the collection pass above:
        // this local drives BOTH the imported struct definitions here and the
        // imported function bodies further down, and `HashMap` order would put
        // the hash seed into the emitted C.
        let mut imported_modules: Vec<_> = self.imported_modules.clone().into_iter().collect();
        imported_modules.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (_, module_info) in &imported_modules {
            for item in &module_info.ast.items {
                match item {
                    Item::Struct(struct_def) => {
                        if matches!(struct_def.visibility, crate::ast::Visibility::Public) {
                            // Skip generic structs - they should only be generated when instantiated
                            if struct_def.type_params.is_empty()
                                && struct_def.lifetime_params.is_empty()
                            {
                                self.generate_struct(struct_def)?;
                            }
                        }
                    }
                    Item::Enum(enum_def) => {
                        // Skip generic enums - they should only be generated when instantiated
                        if enum_def.type_params.is_empty() && enum_def.lifetime_params.is_empty() {
                            self.generate_enum(enum_def)?;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Generate struct and enum definitions from main program
        for item in &program.items {
            match item {
                Item::Struct(struct_def) => {
                    // Skip generic structs - they should only be generated when instantiated
                    if struct_def.type_params.is_empty() && struct_def.lifetime_params.is_empty() {
                        self.generate_struct(struct_def)?;
                    }
                }
                Item::Enum(enum_def) => {
                    // Skip generic enums - they should only be generated when instantiated
                    if enum_def.type_params.is_empty() && enum_def.lifetime_params.is_empty() {
                        self.generate_enum(enum_def)?;
                    }
                }
                _ => {}
            }
        }

        // Generate monomorphized versions of generic structs FIRST
        if !self.generic_struct_instantiations.is_empty() {
            self.output.push_str("// Monomorphized generic structs\n");

            for (struct_name, type_args, generic_struct) in
                &self.generic_struct_instantiations.clone()
            {
                // Create a concrete struct from the generic template
                let concrete_struct =
                    self.monomorphize_struct(struct_name, type_args, generic_struct)?;

                // Track the instantiation mapping for struct literal generation
                let instantiations = self
                    .generic_struct_instantiation_map
                    .entry(struct_name.clone())
                    .or_default();
                instantiations.push((type_args.clone(), concrete_struct.name.clone()));

                self.generate_struct(&concrete_struct)?;
            }
            self.output.push('\n');
        }

        // Forward-declare every user function before any body is emitted, so that
        // a call to a function defined later in the file (and mutual recursion,
        // which no ordering can satisfy) compiles under C99.
        self.generate_function_prototypes(program)?;

        // Generate monomorphized versions of generic functions AFTER structs
        if !self.generic_instantiations.is_empty() {
            self.output.push_str("// Monomorphized generic functions\n");

            for (func_name, type_args, generic_func) in &self.generic_instantiations.clone() {
                // Create a concrete function from the generic template
                let concrete_func =
                    self.monomorphize_function(func_name, type_args, generic_func)?;
                self.generate_function(&concrete_func)?;
            }
            self.output.push('\n');
        }

        // Generate functions from imported modules, off the SAME SORTED LOCAL as
        // above — `imported_modules` here is a sorted Vec, not the HashMap this
        // loop used to walk with `.values()`, because HashMap order put the hash
        // seed into the emitted C.
        //
        // A LOCAL DEFINITION SHADOWS AN IMPORTED ONE, and this loop emitted both.
        // The type checker has always resolved the name to the local function —
        // imported signatures are registered first and the local pass overwrites
        // them — so emitting the imported body as well produces two C definitions
        // of one name. Measured with a module declaring `pub async fn main` and a
        // program declaring its own `fn main`: the C carried `main_Future main()`
        // AND `int main(int, char**)`, and gcc refused it. It only became visible
        // once typeck stopped rejecting that program outright, which it should
        // never have been doing.
        // THROUGH THE SHARED DEFINITION. This loop used to build its own name
        // set, which counted a local GENERIC as shadowing — so an imported body
        // was suppressed while the type checker went on resolving calls to it,
        // leaving a name typeck accepted with no definition emitted. The one
        // definition lives in src/ast/mod.rs and both passes call it, which is
        // what makes "both passes ask one question" a fact about the call graph
        // rather than a claim in a comment.
        for (_, module_info) in &imported_modules {
            for item in &module_info.ast.items {
                if let Item::Function(func) = item {
                    // Only generate public, non-generic, non-shadowed functions
                    if matches!(func.visibility, crate::ast::Visibility::Public)
                        && func.type_params.is_empty()
                        && !crate::ast::local_definition_shadows_import(program, &func.name)
                    {
                        self.generate_function(func)?;
                    }
                }
            }
        }

        // Generate functions from main program
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    // Skip generic functions - they should only be generated when instantiated
                    if !func.type_params.is_empty() {
                        continue;
                    }
                    self.generate_function(func)?;
                }
                Item::Struct(_) => {
                    // Already generated above
                }
                Item::Enum(_) => {
                    // Enum definitions already generated above
                }
                Item::Trait(_) => {
                    // Traits don't generate C code directly
                    // They are used for type checking only
                }
                Item::TypeAlias(_) => {
                    // Type aliases don't generate C code
                    // They are resolved during type checking
                }
                Item::Impl(impl_block) => {
                    // Generate methods from impl blocks
                    for method in &impl_block.methods {
                        if !method.type_params.is_empty() {
                            continue;
                        }
                        // Create a mangled method name
                        let mangled_name = format!(
                            "__pd_{}_{}",
                            impl_block.for_type.to_string().replace("::", "_"),
                            method.name
                        );
                        self.generate_function_with_name(method, &mangled_name)?;
                    }
                }
                Item::Macro(_) => {
                    // Macros are expanded before codegen, skip here
                }
            }
        }

        Ok(())
    }

    /// Convert Type to C type string, resolving type aliases
    fn type_to_c(&self, ty: &Type) -> String {
        match ty {
            Type::I32 => "int".to_string(),
            Type::I64 => "long long".to_string(),
            Type::U32 => "unsigned int".to_string(),
            Type::U64 => "unsigned long long".to_string(),
            Type::Bool => "int".to_string(),
            Type::String => "const char*".to_string(),
            Type::Unit => "void".to_string(),
            Type::Array(elem_type, size) => {
                let size_str = match size {
                    ArraySize::Literal(n) => n.to_string(),
                    ArraySize::ConstParam(name) => name.clone(),
                    ArraySize::Expr(_) => "0".to_string(), // TODO: evaluate expression
                };
                format!("{}[{}]", self.type_to_c(elem_type), size_str)
            }
            Type::Custom(name) => {
                // First check if it's a type alias
                if let Some(aliased_type) = self.type_aliases.get(name) {
                    // Recursively resolve the aliased type
                    self.type_to_c(aliased_type)
                } else {
                    // Otherwise it's a struct or enum name
                    // In C, structs need the "struct" prefix
                    format!("struct {}", name)
                }
            }
            Type::TypeParam(_) | Type::Generic { .. } => {
                // TODO: Proper generic handling
                "void*".to_string() // Placeholder
            }
            Type::Reference { inner, .. } => {
                // References compile to pointers in C
                format!("{}*", self.type_to_c(inner))
            }
            Type::Future { output } => {
                // Futures compile to a struct with state and result
                format!("Future_{}", self.type_to_c(output))
            }
            Type::Tuple(_) => {
                // Tuples not yet supported in C codegen
                "void*".to_string() // TODO: Generate struct for tuple
            }
        }
    }

    /// Generate code for an enum definition
    fn generate_enum(&mut self, enum_def: &EnumDef) -> Result<()> {
        self.defined_structs.insert(enum_def.name.clone());
        // Generate a tagged union for the enum
        self.output.push_str(&format!(
            "// Enum {}
",
            enum_def.name
        ));

        // First, generate the tag enum
        self.output.push_str("typedef enum {\n");
        for variant in &enum_def.variants {
            self.output.push_str(&format!(
                "    __{}__{},
",
                enum_def.name, variant.name
            ));
        }
        self.output.push_str(&format!(
            "}} {}Tag;
\n",
            enum_def.name
        ));

        // Generate data structs for variants with data
        for variant in &enum_def.variants {
            match &variant.data {
                EnumVariantData::Unit => {
                    // Unit variants don't need data structs
                }
                EnumVariantData::Tuple(types) => {
                    if !types.is_empty() {
                        self.output.push_str("typedef struct {\n");
                        for (i, ty) in types.iter().enumerate() {
                            let c_type = self.type_to_c(ty);
                            self.output.push_str(&format!(
                                "    {} field{};
",
                                c_type, i
                            ));
                        }
                        self.output.push_str(&format!(
                            "}} {}__{}_Data;
\n",
                            enum_def.name, variant.name
                        ));
                    }
                }
                EnumVariantData::Struct(fields) => {
                    self.output.push_str("typedef struct {\n");
                    for (field_name, field_type) in fields {
                        let c_type = self.type_to_c(field_type);
                        self.output.push_str(&format!(
                            "    {} {};
",
                            c_type, field_name
                        ));
                    }
                    self.output.push_str(&format!(
                        "}} {}__{}_Data;
\n",
                        enum_def.name, variant.name
                    ));
                }
            }
        }

        // Generate the enum struct with tag and union
        self.output.push_str(&format!(
            "typedef struct {} {{
",
            enum_def.name
        ));
        self.output.push_str(&format!(
            "    {}Tag tag;
",
            enum_def.name
        ));

        // Only generate union if there are variants with data
        let has_data_variants = enum_def
            .variants
            .iter()
            .any(|v| !matches!(v.data, EnumVariantData::Unit));
        if has_data_variants {
            self.output.push_str(
                "    union {
",
            );
            for variant in &enum_def.variants {
                match &variant.data {
                    EnumVariantData::Unit => {
                        // Unit variants don't have data in the union
                    }
                    EnumVariantData::Tuple(types) => {
                        if !types.is_empty() {
                            self.output.push_str(&format!(
                                "        {}__{}_Data {};
",
                                enum_def.name,
                                variant.name,
                                c_ident::c_enum_payload_member(&variant.name)
                            ));
                        }
                    }
                    EnumVariantData::Struct(_) => {
                        self.output.push_str(&format!(
                            "        {}__{}_Data {};
",
                            enum_def.name,
                            variant.name,
                            c_ident::c_enum_payload_member(&variant.name)
                        ));
                    }
                }
            }
            self.output.push_str(
                "    } data;
",
            );
        }

        self.output.push_str(&format!(
            "}} {};
\n",
            enum_def.name
        ));

        // Generate constructor functions for each variant
        for variant in &enum_def.variants {
            match &variant.data {
                EnumVariantData::Unit => {
                    // Unit variant constructor
                    self.output.push_str(&format!(
                        "static inline {} {}_{}() {{
",
                        enum_def.name, enum_def.name, variant.name
                    ));
                    self.output.push_str(&format!(
                        "    {} result = {{.tag = __{}__{}}};
",
                        enum_def.name, enum_def.name, variant.name
                    ));
                    self.output.push_str(
                        "    return result;
",
                    );
                    self.output.push_str("}\n\n");
                }
                EnumVariantData::Tuple(types) => {
                    // Tuple variant constructor
                    self.output.push_str(&format!(
                        "static inline {} {}_{}__new(",
                        enum_def.name, enum_def.name, variant.name
                    ));

                    for (i, ty) in types.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        let c_type = self.type_to_c(ty);
                        self.output.push_str(&format!("{} arg{}", c_type, i));
                    }

                    self.output.push_str(") {\n");
                    self.output.push_str(&format!(
                        "    {} result = {{.tag = __{}__{}}};
",
                        enum_def.name, enum_def.name, variant.name
                    ));

                    if !types.is_empty() {
                        for i in 0..types.len() {
                            self.output.push_str(&format!(
                                "    result.data.{}.field{} = arg{};
",
                                c_ident::c_enum_payload_member(&variant.name),
                                i,
                                i
                            ));
                        }
                    }

                    self.output.push_str(
                        "    return result;
",
                    );
                    self.output.push_str("}\n\n");
                }
                EnumVariantData::Struct(fields) => {
                    // Struct variant constructor
                    self.output.push_str(&format!(
                        "static inline {} {}_{}__new(",
                        enum_def.name, enum_def.name, variant.name
                    ));

                    for (i, (field_name, field_type)) in fields.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        let c_type = self.type_to_c(field_type);
                        self.output.push_str(&format!("{} {}", c_type, field_name));
                    }

                    self.output.push_str(") {\n");
                    self.output.push_str(&format!(
                        "    {} result = {{.tag = __{}__{}}};
",
                        enum_def.name, enum_def.name, variant.name
                    ));

                    for (field_name, _) in fields {
                        self.output.push_str(&format!(
                            "    result.data.{}.{} = {};
",
                            c_ident::c_enum_payload_member(&variant.name),
                            field_name,
                            field_name
                        ));
                    }

                    self.output.push_str(
                        "    return result;
",
                    );
                    self.output.push_str("}\n\n");
                }
            }
        }

        Ok(())
    }

    /// Generate code for a struct definition
    fn generate_struct(&mut self, struct_def: &StructDef) -> Result<()> {
        self.defined_structs.insert(struct_def.name.clone());
        // Remember the layout so field access can be typed without re-deriving
        // it from the AST (the borrow checker keeps the same map).
        self.structs
            .insert(struct_def.name.clone(), struct_def.fields.clone());
        self.output
            .push_str(&format!("typedef struct {} {{\n", struct_def.name));

        for (field_name, field_type) in &struct_def.fields {
            self.output.push_str("    ");

            let c_type = match field_type {
                Type::I32 => "int",
                Type::I64 => "long long",
                Type::U32 => "unsigned int",
                Type::U64 => "unsigned long long",
                Type::Bool => "int",
                Type::String => "const char*",
                Type::Array(elem_type, size) => {
                    // For arrays in structs, we need to handle them specially
                    let elem_c_type = self.type_to_c(elem_type.as_ref());
                    let size_str = match size {
                        ArraySize::Literal(n) => n.to_string(),
                        ArraySize::ConstParam(name) => name.clone(),
                        ArraySize::Expr(_) => "0".to_string(), // TODO: evaluate expression
                    };
                    self.output
                        .push_str(&format!("{} {}[{}];\n", elem_c_type, field_name, size_str));
                    continue;
                }
                Type::Unit => "void",
                Type::Custom(_name) => {
                    // Use type_to_c to resolve type aliases
                    let resolved_type = self.type_to_c(field_type);
                    self.output
                        .push_str(&format!("{} {};\n", resolved_type, field_name));
                    continue;
                }
                Type::TypeParam(_) | Type::Generic { .. } => {
                    return Err(CompileError::Generic(
                        "Generic types in structs not yet supported".to_string(),
                    ));
                }
                Type::Reference { .. } => {
                    return Err(CompileError::Generic(
                        "Reference types in structs not yet supported".to_string(),
                    ));
                }
                Type::Future { .. } => {
                    return Err(CompileError::Generic(
                        "Future types in structs not yet supported".to_string(),
                    ));
                }
                Type::Tuple(_) => {
                    return Err(CompileError::Generic(
                        "Tuple types in structs not yet supported".to_string(),
                    ));
                }
            };

            self.output
                .push_str(&format!("{} {};\n", c_type, field_name));
        }

        self.output
            .push_str(&format!("}} {};\n\n", struct_def.name));
        Ok(())
    }

    /// Generate code for a function
    fn generate_function(&mut self, func: &Function) -> Result<()> {
        self.generate_function_with_name(func, &func.name)
    }

    /// True when every `struct X` named in a C signature refers to a tag we emitted.
    fn signature_tags_are_defined(
        sig: &str,
        defined_structs: &std::collections::HashSet<String>,
    ) -> bool {
        for after in sig.split("struct ").skip(1) {
            let tag: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !defined_structs.contains(&tag) {
                return false;
            }
        }
        true
    }

    /// Emit a C forward declaration for every user-defined function.
    ///
    /// Placed after all type definitions and before the first function body, so
    /// call sites never depend on definition order. `main` is skipped: it is
    /// emitted as the C entry point (`int main(int argc, char** argv)`) and is
    /// never called from Palladium code. `__pd_*` builtins are already defined
    /// in the runtime prelude above.
    fn generate_function_prototypes(&mut self, program: &Program) -> Result<()> {
        let mut prototypes: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let defined_structs = self.defined_structs.clone();

        let mut push = |prototypes: &mut Vec<String>, name: &str, sig: String| {
            // A prototype that names a struct tag we never defined would declare
            // that tag inside the parameter list, making it a distinct type from
            // the one in the definition ("conflicting types"). Such a program is
            // already broken without us; stay silent instead of making it worse.
            if !Self::signature_tags_are_defined(&sig, &defined_structs) {
                return;
            }
            if seen.insert(name.to_string()) {
                prototypes.push(format!("{};\n", sig));
            }
        };

        // Imported modules: the ones whose BODY is emitted below, asked through
        // the same shared definition the body loop uses.
        //
        // This loop used to omit the shadowing test, and it is the loop that
        // decides the name — `seen` is first-wins and imports are visited before
        // the main program, so a shadowed import took the slot and the LOCAL
        // prototype was then dropped as a duplicate. MEASURED: an imported
        // `pub fn f(x: i64) -> i64` with a local `fn f() -> i64` emitted
        // `long long f(long long x);` next to `long long f() { … }` and gcc
        // refused it ("conflicting types for 'f'", "too few arguments"). The
        // condition list here read like the body loop's but was one term short,
        // which is exactly what a shared predicate cannot protect against if a
        // call site does not call it.
        //
        // The `seen` ordering is no longer load-bearing between these two
        // sources: with the shadowing test, an imported name and a local name
        // cannot both be pushed. A local TYPE-PARAMETERISED function does not
        // shadow (see `local_definition_shadows_import`) but emits no prototype
        // under its own name either — its instantiations are mangled — so that
        // case is not a collision.
        //
        // SORTED BY MODULE NAME, like the three sites that fill the definitions.
        // This is the fourth and last place the imported modules are iterated, and
        // `HashMap` order here alone kept the emitted C unstable: with the other
        // three ordered, twenty-four compiles of one unchanged six-module program
        // still produced twenty-four distinct files, differing only in this block.
        // Ordered too, `test_the_whole_emitted_c_is_byte_stable`'s 8 are identical.
        let mut imported_modules: Vec<_> = self.imported_modules.clone().into_iter().collect();
        imported_modules.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (_, module_info) in &imported_modules {
            for item in &module_info.ast.items {
                if let Item::Function(func) = item {
                    if matches!(func.visibility, crate::ast::Visibility::Public)
                        && func.type_params.is_empty()
                        && !crate::ast::local_definition_shadows_import(program, &func.name)
                        && !func.is_async
                        && func.name != "main"
                    {
                        let sig = self.function_signature(func, &func.name)?;
                        push(&mut prototypes, &func.name, sig);
                    }
                }
            }
        }

        // Monomorphized generic instantiations are emitted as ordinary functions.
        for (func_name, type_args, generic_func) in &self.generic_instantiations.clone() {
            let concrete_func = self.monomorphize_function(func_name, type_args, generic_func)?;
            if concrete_func.is_async {
                continue;
            }
            let sig = self.function_signature(&concrete_func, &concrete_func.name)?;
            let name = concrete_func.name.clone();
            push(&mut prototypes, &name, sig);
        }

        // Main program: free functions and impl methods.
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    if !func.type_params.is_empty() || func.is_async || func.name == "main" {
                        continue;
                    }
                    let sig = self.function_signature(func, &func.name)?;
                    push(&mut prototypes, &func.name, sig);
                }
                Item::Impl(impl_block) => {
                    for method in &impl_block.methods {
                        if !method.type_params.is_empty() || method.is_async {
                            continue;
                        }
                        let mangled_name = format!(
                            "__pd_{}_{}",
                            impl_block.for_type.to_string().replace("::", "_"),
                            method.name
                        );
                        let sig = self.function_signature(method, &mangled_name)?;
                        push(&mut prototypes, &mangled_name, sig);
                    }
                }
                _ => {}
            }
        }

        if !prototypes.is_empty() {
            self.output.push_str("// Forward declarations\n");
            for prototype in prototypes {
                self.output.push_str(&prototype);
            }
            self.output.push('\n');
        }

        Ok(())
    }

    /// The C declarator for an array parameter: `long long xs[5]`.
    ///
    /// C decays every array parameter to a pointer, so `[T; N]`, `&[T; N]` and
    /// `&mut [T; N]` are all passed the same way; `is_const` is the only thing
    /// that distinguishes a shared reference from a mutable one. Keeping the
    /// `[N]` suffix (rather than writing `T*`) documents the length at the call
    /// site and lets indexing and element assignment read naturally.
    ///
    /// `is_const` has to qualify the *element slot*, which for a pointer
    /// element is not the same place as qualifying what it points at:
    /// `String` is `const char*`, so a shared `&[String; N]` is
    /// `const char* const xs[N]` (the slot cannot be reassigned) and not
    /// `const char* xs[N]` (only the characters are read-only, `xs[i] = other`
    /// still compiles).
    fn array_param_declarator(
        elem_type: &Type,
        size: &ArraySize,
        param_name: &str,
        is_const: bool,
    ) -> Result<String> {
        let elem_c_type = match elem_type {
            Type::I32 => "int",
            Type::I64 => "long long",
            Type::U32 => "unsigned int",
            Type::U64 => "unsigned long long",
            Type::Bool => "int",
            // Same spelling `type_to_c` gives a `String` local, so a caller's
            // `const char* xs[3]` and this parameter are the same pointer type.
            // Writing `char*` here made `&mut [String; N]` a `char**` against
            // the caller's `const char**`: an incompatible pointer type that
            // also discards a qualifier.
            Type::String => "const char*",
            Type::Custom(name) => name.as_str(), // Support struct arrays
            _ => {
                return Err(CompileError::Generic(format!(
                    "Unsupported array element type in function parameter: {:?}",
                    elem_type
                )))
            }
        };
        // A trailing `*` means the qualifier belongs after it: `const char*
        // const`, not `const const char*`.
        let elem_decl = match (is_const, elem_c_type.ends_with('*')) {
            (false, _) => elem_c_type.to_string(),
            (true, true) => format!("{} const", elem_c_type),
            (true, false) => format!("const {}", elem_c_type),
        };
        // Only a proven length may be printed. A const generic prints as its
        // own name, which is not in scope in the generated C - `[i64; N]` used
        // to emit `long long xs[N]` and gcc rejected it with "use of undeclared
        // identifier 'N'". An unproven length decays to `[]`, which is exactly
        // what C does to the parameter anyway.
        let size_str = match Self::array_len_of_size(size) {
            ArrayLen::Proven(n) => n.to_string(),
            ArrayLen::Unproven(_) => String::new(),
        };
        Ok(format!("{} {}[{}]", elem_decl, param_name, size_str))
    }

    /// Build the C signature line for a function, without a trailing `{` or `;`.
    ///
    /// Single source of truth: both the definition (`generate_function_with_name`)
    /// and the forward declaration (`generate_function_prototypes`) call this, so
    /// the two can never disagree.
    fn function_signature(&self, func: &Function, name: &str) -> Result<String> {
        // Function signature with return type
        let return_type_string = match &func.return_type {
            Some(Type::Array(_, _)) => {
                // Arrays cannot be returned by value in C, would need to return pointer
                return Err(CompileError::Generic(
                    "Returning arrays from functions is not yet supported".to_string(),
                ));
            }
            Some(t) => self.type_to_c(t),
            None => "void".to_string(),
        };

        let return_type = return_type_string.as_str();

        // Special case: main always returns int in C
        let actual_return_type = if name == "main" && return_type == "void" {
            "int"
        } else {
            return_type
        };

        // The C entry point takes (argc, argv) so that arg_count()/arg_at() can
        // reach the command line. Only applies to a parameterless Palladium main.
        let is_c_entry = name == "main" && func.params.is_empty();

        // Generate function parameters
        let mut sig = String::new();
        sig.push_str(&format!("{} {}(", actual_return_type, name));

        if is_c_entry {
            sig.push_str("int argc, char** argv");
        }

        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                sig.push_str(", ");
            }

            match &param.ty {
                Type::Array(elem_type, size) => {
                    // In C, array parameters are passed as pointers.
                    // We'll generate: type name[size] for clarity, though it decays to pointer
                    sig.push_str(&Self::array_param_declarator(
                        elem_type, size, &param.name, false,
                    )?);
                }
                Type::Custom(_) => {
                    // Use type_to_c to resolve type aliases
                    let c_type = self.type_to_c(&param.ty);
                    if param.mutable {
                        // Pass by pointer for mutable parameters
                        sig.push_str(&format!("{}* {}", c_type, param.name));
                    } else {
                        // Pass by value for immutable parameters
                        sig.push_str(&format!("{} {}", c_type, param.name));
                    }
                }
                Type::Reference { inner, mutable, .. } => {
                    // A reference to an array is passed exactly like the array
                    // itself - C decays both to a pointer to the first element -
                    // so `&[T; N]` differs from `&mut [T; N]` only by const.
                    // Without this case the whole parameter was rejected, which
                    // is why examples/practical/simple_sort.pd did not compile.
                    if let Type::Array(elem_type, size) = inner.as_ref() {
                        sig.push_str(&Self::array_param_declarator(
                            elem_type, size, &param.name, !*mutable,
                        )?);
                        continue;
                    }
                    // Handle reference parameters
                    match inner.as_ref() {
                        Type::I32 => {
                            sig.push_str(if *mutable { "int* " } else { "const int* " });
                        }
                        Type::I64 => {
                            sig.push_str(if *mutable {
                                "long long* "
                            } else {
                                "const long long* "
                            });
                        }
                        Type::U32 => {
                            sig.push_str(if *mutable {
                                "unsigned int* "
                            } else {
                                "const unsigned int* "
                            });
                        }
                        Type::U64 => {
                            sig.push_str(if *mutable {
                                "unsigned long long* "
                            } else {
                                "const unsigned long long* "
                            });
                        }
                        Type::Bool => {
                            sig.push_str(if *mutable { "int* " } else { "const int* " });
                        }
                        Type::String => {
                            sig.push_str(if *mutable { "char** " } else { "const char** " });
                        }
                        Type::Custom(name) => {
                            if *mutable {
                                sig.push_str(&format!("struct {}* ", name));
                            } else {
                                sig.push_str(&format!("const struct {}* ", name));
                            }
                        }
                        _ => {
                            return Err(CompileError::Generic(
                                "Unsupported type in reference parameter".to_string(),
                            ));
                        }
                    }
                    sig.push_str(&param.name);
                }
                _ => {
                    // For other types
                    let c_type = self.type_to_c(&param.ty);

                    if param.mutable {
                        // Pass by pointer for mutable parameters
                        sig.push_str(&format!("{}* {}", c_type, param.name));
                    } else {
                        // Pass by value for immutable parameters
                        sig.push_str(&format!("{} {}", c_type, param.name));
                    }
                }
            }
        }

        sig.push(')');

        Ok(sig)
    }

    fn generate_function_with_name(&mut self, func: &Function, name: &str) -> Result<()> {
        // N7-18. This dispatched into `generate_async_function_with_name`, which
        // emitted a `<name>_Future` struct and a `<name>_poll` routine. Deleted;
        // see `CompileError::async_fn_unimplemented`. Same shape as `?`/`.await`.
        if func.is_async {
            return Err(CompileError::async_fn_unimplemented(func.span));
        }

        // The C entry point takes (argc, argv) so that arg_count()/arg_at() can
        // reach the command line. Only applies to a parameterless Palladium main.
        let is_c_entry = name == "main" && func.params.is_empty();

        let signature = self.function_signature(func, name)?;
        self.output.push_str(&signature);
        self.output.push_str(" {\n");

        if is_c_entry {
            self.output.push_str("    __pd_argc = argc;\n");
            self.output.push_str("    __pd_argv = argv;\n");
        }

        // Clear mutable_params from previous function and populate with current function's params
        self.mutable_params.clear();
        self.variables.clear(); // Clear variables from previous function
        self.array_bindings.clear();
        // BOTH spellings of the unit type, which is the whole point: `None` and
        // `Some(Type::Unit)` are one return type and must generate one shape.
        // And `main` is INSIDE the rule, not an exception to it — it just needs
        // a different replacement, because its C type is `int`.
        self.current_fn_unit_return = if matches!(func.return_type, None | Some(Type::Unit))
        {
            if name == "main" {
                Some("    return 0;\n")
            } else {
                Some("    return;\n")
            }
        } else {
            None
        };

        for param in &func.params {
            // Track if parameter is a pointer (either mutable or reference)
            let is_pointer = param.mutable || matches!(&param.ty, Type::Reference { .. });
            self.mutable_params.insert(param.name.clone(), is_pointer);

            // Record the length and the declared form of array parameters. C
            // keeps neither: every one of them arrives as a bare pointer.
            let array_param = match &param.ty {
                Type::Array(_, size) => Some((
                    size,
                    if param.mutable {
                        ArrayParamForm::MutByValue
                    } else {
                        ArrayParamForm::ByValue
                    },
                )),
                Type::Reference { inner, mutable, .. } => match inner.as_ref() {
                    Type::Array(_, size) => Some((
                        size,
                        if *mutable {
                            ArrayParamForm::Mutable
                        } else {
                            ArrayParamForm::Shared
                        },
                    )),
                    _ => None,
                },
                _ => None,
            };
            if let Some((size, form)) = array_param {
                self.array_bindings.insert(
                    param.name.clone(),
                    ArrayBinding {
                        len: Self::array_len_of_size(size),
                        storage: ArrayStorage::Parameter(form),
                    },
                );
            }

            // Also track parameter types for type inference
            let c_type = match &param.ty {
                Type::String => "const char*".to_string(),
                Type::I32 => "int".to_string(),
                Type::I64 => "long long".to_string(),
                Type::Bool => "int".to_string(),
                Type::Custom(name) => name.clone(),
                // Array parameters keep their dimensions, in the same encoding
                // `self.variables` uses for locals ("long long[4]"). Recording
                // the bare element type instead lost the length, which is what
                // forced `for x in arr` to fall back to `sizeof` on a pointer,
                // and left `let e = arr[i];` with no inferable type at all.
                Type::Array(_, _) => self.type_to_c(&param.ty),
                Type::Reference { inner, .. } => {
                    // For references, we track the base type
                    match inner.as_ref() {
                        Type::Custom(name) => name.clone(),
                        Type::I32 => "int".to_string(),
                        Type::I64 => "long long".to_string(),
                        Type::Array(_, _) => self.type_to_c(inner),
                        _ => "long long".to_string(),
                    }
                }
                _ => "long long".to_string(),
            };
            self.variables.insert(param.name.clone(), c_type);
        }

        // Function body
        for stmt in &func.body {
            self.generate_statement(stmt)?;
        }

        // Close function
        // Only add default return for void main or if no explicit return
        if func.name == "main" && func.return_type.is_none() {
            self.output.push_str("    return 0;\n");
        }
        self.output.push_str("}\n\n");

        // Clear parameter tracking after function
        self.mutable_params.clear();

        Ok(())
    }

    /// Generate the statements of a nested block, with their own array-binding
    /// scope. `indent` is prepended to each statement.
    ///
    /// The scope is the point. `array_bindings` records what may be written and
    /// how long each array is, and a flat function-wide map let an inner
    /// binding overwrite an outer one and never give it back:
    ///
    /// ```text
    /// fn f(xs: [i64; 3]) {
    ///     if true { let xs: [i64; 2] = [1, 2]; }
    ///     xs[0] = 99;          // guard saw the *shadow*, an owned local
    /// }
    /// ```
    ///
    /// which compiled, ran, and left 99 in the caller's array. The same leak
    /// gave a following `for x in xs` the shadow's length: a `[i64; 4]`
    /// parameter iterated twice.
    fn generate_block(&mut self, stmts: &[Stmt], indent: &str) -> Result<()> {
        let outer = self.open_binding_scope();
        for stmt in stmts {
            self.output.push_str(indent);
            self.generate_statement(stmt)?;
        }
        self.close_binding_scope(outer);
        Ok(())
    }

    /// Record a binding that is *not* an array, shadowing any array of the same
    /// name.
    ///
    /// Inserting into `variables` alone is not shadowing: `array_bindings` kept
    /// the outer entry, so a loop variable named after an outer array was still
    /// treated as that array - `for v in ys { print_int(v); }` under an outer
    /// `let mut v = [1, 2, 3];` was refused as "cannot pass an array to
    /// print_int". A name means one thing at a time, in every map.
    fn bind_non_array(&mut self, name: &str, c_type: String) {
        self.variables.insert(name.to_string(), c_type);
        self.array_bindings.remove(name);
    }

    /// Snapshot the bindings a scope may shadow.
    ///
    /// Take this **before the scope's first write**, not before its body. A
    /// `for` variable and a match binding are written into these maps *before*
    /// the block is generated, so snapshotting inside `generate_block` captured
    /// the already-overwritten map and the binder outlived its own scope: after
    /// `for v in xs { }`, an outer `v` still had the loop variable's type.
    fn open_binding_scope(&self) -> (
        std::collections::HashMap<String, ArrayBinding>,
        std::collections::HashMap<String, String>,
    ) {
        (self.array_bindings.clone(), self.variables.clone())
    }

    fn close_binding_scope(
        &mut self,
        saved: (
            std::collections::HashMap<String, ArrayBinding>,
            std::collections::HashMap<String, String>,
        ),
    ) {
        let (arrays, variables) = saved;
        self.array_bindings = arrays;
        self.variables = variables;
    }

    /// Generate code for a statement
    fn generate_statement(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Expr(expr) => {
                self.output.push_str("    ");
                self.generate_expression(expr)?;
                self.output.push_str(";\n");
            }
            Stmt::Return(None) => {
                // `return;` from C `main` is the same constraint violation one
                // step down: measured, `fn main() { return; }` emitted `return;`
                // from `int main`. The unit replacement knows which is right.
                self.output
                    .push_str(self.current_fn_unit_return.unwrap_or("    return;\n"));
            }
            Stmt::Return(Some(expr)) => {
                if let Some(unit_return) = self.current_fn_unit_return {
                    // The expression is still EVALUATED — it is there for its
                    // effect, and dropping it would change what the program
                    // does — but its value is not returned, because this
                    // function returns nothing in Palladium.
                    self.output.push_str("    ");
                    self.generate_expression(expr)?;
                    self.output.push_str(";\n");
                    self.output.push_str(unit_return);
                } else {
                    self.output.push_str("    return ");
                    self.generate_expression(expr)?;
                    self.output.push_str(";\n");
                }
            }
            Stmt::Let {
                name, ty, value, ..
            } => {
                self.output.push_str("    ");

                // Determine C type. `array_dims` is the (possibly empty)
                // bracket suffix of the declarator, e.g. "[3]".
                let (c_type, array_dims) = match ty {
                    Some(t) => match t {
                        Type::Array(elem_type, size) => {
                            let size_val = match size {
                                ArraySize::Literal(n) => *n,
                                ArraySize::ConstParam(_) => 0, // TODO: resolve const param
                                ArraySize::Expr(_) => 0,       // TODO: evaluate expression
                            };
                            (self.type_to_c(elem_type), format!("[{}]", size_val))
                        }
                        _ => (self.type_to_c(t), String::new()),
                    },
                    None => {
                        // No annotation: the inferred type IS the declared type,
                        // so a guess here is a silently miscompiled program.
                        // Refuse instead of defaulting to an integer.
                        let inferred_type = self.try_infer_expr_type(value).ok_or_else(|| {
                            CompileError::CodegenError {
                                message: format!(
                                    "cannot infer the type of `{}`: no type rule for this {} \
                                     expression. Add an explicit type annotation, \
                                     e.g. `let {}: i64 = ...;`",
                                    name,
                                    Self::expr_kind_name(value),
                                    name
                                ),
                            }
                        })?;
                        Self::split_array_dims(&inferred_type)
                    }
                };
                // Track variable type for future inference. Arrays keep their
                // dimensions in the recorded type.
                self.variables
                    .insert(name.clone(), format!("{}{}", c_type, array_dims));

                // A local array is a real object: its length comes from the
                // annotation when there is one, otherwise from the initializer.
                // A non-array `let` must *remove* any array of the same name,
                // or the outer array keeps answering for the inner binding.
                if array_dims.is_empty() {
                    self.array_bindings.remove(name.as_str());
                } else {
                    let len = match ty {
                        Some(Type::Array(_, size)) => Self::array_len_of_size(size),
                        _ => self
                            .array_len_of_expr(value)
                            .unwrap_or_else(|| ArrayLen::Unproven("initializer".to_string())),
                    };
                    self.array_bindings.insert(
                        name.clone(),
                        ArrayBinding {
                            len,
                            storage: ArrayStorage::Object,
                        },
                    );
                }

                self.output
                    .push_str(&format!("{} {}{} = ", c_type, name, array_dims));
                self.generate_expression(value)?;
                self.output.push_str(";\n");
            }
            Stmt::Assign { target, value, .. } => {
                if let Some(root) = Self::assign_target_root(target) {
                    self.check_array_write(root)?;
                }
                self.output.push_str("    ");
                match target {
                    AssignTarget::Ident(name) => {
                        // Check if this is a mutable parameter
                        if let Some(&is_mutable) = self.mutable_params.get(name) {
                            if is_mutable {
                                // Dereference mutable parameters
                                self.output.push_str(&format!("(*{}) = ", name));
                            } else {
                                self.output.push_str(&format!("{} = ", name));
                            }
                        } else {
                            self.output.push_str(&format!("{} = ", name));
                        }
                    }
                    AssignTarget::Index { array, index } => {
                        self.generate_expression(array)?;
                        self.output.push('[');
                        self.generate_expression(index)?;
                        self.output.push_str("] = ");
                    }
                    AssignTarget::FieldAccess { object, field } => {
                        // Check if object is a mutable parameter (pointer)
                        let use_arrow = match object.as_ref() {
                            Expr::Ident(name) => {
                                self.mutable_params.get(name).copied().unwrap_or(false)
                            }
                            _ => false,
                        };

                        if use_arrow {
                            // For mutable params, we need special handling
                            if let Expr::Ident(name) = object.as_ref() {
                                self.output.push_str(&format!("{}->{} = ", name, field));
                            } else {
                                self.generate_expression(object)?;
                                self.output.push_str(&format!("->{} = ", field));
                            }
                        } else {
                            self.generate_expression(object)?;
                            self.output.push_str(&format!(".{} = ", field));
                        }
                    }
                    AssignTarget::Deref { expr } => {
                        // Generate dereference assignment: *expr = value
                        self.output.push_str("*(");
                        self.generate_expression(expr)?;
                        self.output.push_str(") = ");
                    }
                }
                self.generate_expression(value)?;
                self.output.push_str(";\n");
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.output.push_str("    if (");
                self.generate_expression(condition)?;
                self.output.push_str(") {\n");

                // Generate then branch
                self.generate_block(then_branch, "")?;

                self.output.push_str("    }");

                // Generate else branch if present
                if let Some(else_stmts) = else_branch {
                    self.output.push_str(" else {\n");
                    self.generate_block(else_stmts, "")?;
                    self.output.push_str("    }");
                }

                self.output.push('\n');
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.output.push_str("    while (");
                self.generate_expression(condition)?;
                self.output.push_str(") {\n");

                // Generate body
                self.generate_block(body, "")?;

                self.output.push_str("    }\n");
            }
            Stmt::For {
                var, iter, body, ..
            } => {
                self.output.push_str("    {\n"); // Create a new scope

                // Check if iterating over a range
                match iter {
                    Expr::Range { start, end, .. } => {
                        // Generate C-style for loop for range
                        self.output.push_str("        // For loop with range\n");
                        self.output
                            .push_str(&format!("        for (long long {} = ", var));
                        // Record the loop variable so expressions in the body
                        // can be typed (see try_infer_expr_type/Expr::Ident).
                        // The scope opens *here*, before the binder is written,
                        // so the binder cannot outlive the loop.
                        let loop_scope = self.open_binding_scope();
                        self.bind_non_array(var, "long long".to_string());
                        self.generate_expression(start)?;
                        self.output.push_str(&format!("; {} < ", var));
                        self.generate_expression(end)?;
                        self.output.push_str(&format!("; {}++) {{\n", var));

                        // Generate body
                        self.generate_block(body, "        ")?;
                        self.close_binding_scope(loop_scope);

                        self.output.push_str("        }\n");
                    }
                    _ => {
                        // For arrays and other iterables
                        self.output.push_str("        // For-in loop\n");

                        // The bound is decided from typed provenance, never
                        // from the printed C type: `[T; N]` with an unresolved
                        // `N` also prints as `[0]`, so a string cannot tell a
                        // real zero from a length nobody worked out.
                        //
                        // - a proven length is emitted, `0` included;
                        // - an unproven length on a *parameter* is a hard error:
                        //   the parameter decayed to a pointer, so
                        //   `sizeof(x)/sizeof(x[0])` is a pointer-size ratio
                        //   (8/8 = 1 for `i64`), which is how this loop used to
                        //   visit one element and stop without saying anything;
                        // - `sizeof` survives only where it is actually
                        //   correct: a local array object, whose size C knows.
                        let (elem_type, _) = Self::split_array_dims(&self.infer_expr_type(iter));
                        let len = self.array_len_of_expr(iter);
                        let storage = match iter {
                            Expr::Ident(name) => {
                                self.array_bindings.get(name).map(|b| b.storage)
                            }
                            _ => None,
                        };

                        self.output.push_str("        for (long long _i = 0; _i < ");
                        match (&len, storage) {
                            (Some(ArrayLen::Proven(n)), _) => {
                                self.output.push_str(&n.to_string());
                            }
                            (
                                Some(ArrayLen::Unproven(spelling)),
                                Some(ArrayStorage::Parameter(_)),
                            ) => {
                                let name = match iter {
                                    Expr::Ident(name) => name.as_str(),
                                    _ => "<expression>",
                                };
                                return Err(CompileError::CodegenError {
                                    message: format!(
                                        "cannot iterate `{}`: its length is declared as `{}`, \
                                         which this compiler does not resolve (const generic \
                                         array lengths are dropped - see \
                                         docs/specification/language-spec.md §5). `{}` is a \
                                         parameter, so it has decayed to a pointer and its \
                                         length cannot be recovered at run time either. Give \
                                         the parameter a literal length, e.g. \
                                         `{}: [T; 4]`, or iterate an explicit range.",
                                        name, spelling, name, name
                                    ),
                                });
                            }
                            _ => {
                                self.output.push_str("sizeof(");
                                self.generate_expression(iter)?;
                                self.output.push_str(")/sizeof(");
                                self.generate_expression(iter)?;
                                self.output.push_str("[0])");
                            }
                        }
                        self.output.push_str("; _i++) {\n");

                        // Declare loop variable and assign current element.
                        // Its type is the array's element type, not always an
                        // integer, and the body needs to know it.
                        self.output
                            .push_str(&format!("            {} {} = ", elem_type, var));
                        let loop_scope = self.open_binding_scope();
                        self.bind_non_array(var, elem_type);
                        self.generate_expression(iter)?;
                        self.output.push_str("[_i];\n");

                        // Generate body
                        self.generate_block(body, "        ")?;
                        self.close_binding_scope(loop_scope);

                        self.output.push_str("        }\n");
                    }
                }
                self.output.push_str("    }\n");
            }
            Stmt::Break { .. } => {
                self.output.push_str("    break;\n");
            }
            Stmt::Continue { .. } => {
                self.output.push_str("    continue;\n");
            }
            Stmt::Match { expr, arms, .. } => {
                // Generate a series of if-else statements for pattern matching
                self.output.push_str("    // Match statement\n");
                self.output.push_str("    {\n");

                // Determine the type of the match expression
                let expr_type = self.infer_expr_type(expr);
                let is_enum =
                    expr_type != "long long" && expr_type != "const char*" && expr_type != "int";

                // Store the match expression in a temporary variable
                self.output
                    .push_str("        // Temporary for match expression\n");
                if is_enum {
                    self.output
                        .push_str(&format!("        {} _match_expr = ", expr_type));
                } else {
                    self.output.push_str("        long long _match_expr = ");
                }
                self.generate_expression(expr)?;
                self.output.push_str(";\n");

                // Generate if-else chain for each arm
                for (i, arm) in arms.iter().enumerate() {
                    if i == 0 {
                        self.output.push_str("        if (");
                    } else {
                        self.output.push_str(" else if (");
                    }

                    // Generate pattern matching condition
                    match &arm.pattern {
                        Pattern::Wildcard => {
                            // Wildcard always matches
                            self.output.push('1');
                        }
                        Pattern::Ident(name) => {
                            // Identifier pattern always matches and binds
                            self.output.push_str("1) {\n");
                            self.output.push_str(&format!(
                                "            long long {} = _match_expr;\n",
                                name
                            ));
                            let arm_scope = self.open_binding_scope();
                            self.bind_non_array(name, "long long".to_string());
                            // Continue with body generation below
                            self.generate_block(&arm.body, "        ")?;
                            self.close_binding_scope(arm_scope);
                            self.output.push_str("        }");
                            continue;
                        }
                        Pattern::EnumPattern {
                            enum_name,
                            variant,
                            data,
                        } => {
                            // Opened before the variant's data bindings are
                            // written, so they die with the arm.
                            let arm_scope = self.open_binding_scope();
                            // Generate enum tag check
                            self.output.push_str(&format!(
                                "_match_expr.tag == __{}__{})",
                                enum_name, variant
                            ));
                            self.output.push_str(" {\n");

                            // Extract data if present
                            if let Some(pattern_data) = data {
                                // Look up the enum definition to get field types
                                if let Some(enum_def) = self.enums.get(enum_name) {
                                    // Find the variant
                                    if let Some(variant_def) =
                                        enum_def.variants.iter().find(|v| &v.name == variant)
                                    {
                                        match (&variant_def.data, pattern_data) {
                                            (
                                                EnumVariantData::Tuple(types),
                                                PatternData::Tuple(patterns),
                                            ) => {
                                                // Extract tuple fields with proper types
                                                for (i, (pattern, ty)) in
                                                    patterns.iter().zip(types.iter()).enumerate()
                                                {
                                                    if let Pattern::Ident(name) = pattern {
                                                        let c_type = self.type_to_c(ty);
                                                        self.output.push_str(&format!(
                                                            "            {} {} = _match_expr.data.{}.field{};\n",
                                                            c_type, name, c_ident::c_enum_payload_member(variant), i
                                                        ));
                                                        // The binding is a real
                                                        // variable; type it for
                                                        // the arm body.
                                                        // Field-level writes:
                                                        // `self.enums` is
                                                        // borrowed here, so
                                                        // bind_non_array's
                                                        // `&mut self` would
                                                        // conflict. Same two
                                                        // operations.
                                                        self.variables
                                                            .insert(name.clone(), c_type);
                                                        self.array_bindings
                                                            .remove(name.as_str());
                                                    }
                                                }
                                            }
                                            (
                                                EnumVariantData::Struct(fields),
                                                PatternData::Struct(field_patterns),
                                            ) => {
                                                // Extract struct fields with proper types
                                                for (field_name, pattern) in field_patterns {
                                                    if let Pattern::Ident(name) = pattern {
                                                        // Find the field type
                                                        if let Some((_, field_type)) = fields
                                                            .iter()
                                                            .find(|(fname, _)| fname == field_name)
                                                        {
                                                            let c_type = self.type_to_c(field_type);
                                                            self.output.push_str(&format!(
                                                                "            {} {} = _match_expr.data.{}.{};\n",
                                                                c_type, name, c_ident::c_enum_payload_member(variant), field_name
                                                            ));
                                                            self.variables
                                                                .insert(name.clone(), c_type);
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {
                                                // Fallback for mismatched patterns (shouldn't happen with proper type checking)
                                                return Err(CompileError::Generic(
                                                    "Pattern type mismatch in enum variant"
                                                        .to_string(),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }

                            // Continue with body generation below
                            self.generate_block(&arm.body, "        ")?;
                            self.close_binding_scope(arm_scope);
                            self.output.push_str("        }");
                            continue;
                        }
                    }

                    self.output.push_str(") {\n");

                    // Generate arm body
                    self.generate_block(&arm.body, "        ")?;

                    self.output.push_str("        }");
                }

                // If no wildcard pattern, we might need a default case
                // TODO: Add exhaustiveness checking

                self.output.push_str("\n    }\n");
            }
            Stmt::Unsafe { body, .. } => {
                // Unsafe blocks in C are just regular blocks
                // The safety checks are done at compile time
                self.output.push_str("    // unsafe block\n");
                self.output.push_str("    {\n");

                // Generate body
                self.generate_block(body, "    ")?;

                self.output.push_str("    }\n");
            }
        }
        Ok(())
    }

    /// Generate code for an expression
    fn generate_expression(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::String(s) => {
                // Escape the string properly
                let escaped = s
                    .replace("\\", "\\\\")
                    .replace("\"", "\\\"")
                    .replace("\n", "\\n")
                    .replace("\t", "\\t")
                    .replace("\r", "\\r");
                self.output.push_str(&format!("\"{}\"", escaped));
            }
            Expr::Integer(n) => {
                self.output.push_str(&format!("{}", n));
            }
            Expr::Bool(b) => {
                // C represents bool as 1 or 0
                self.output.push_str(if *b { "1" } else { "0" });
            }
            Expr::Ident(name) => {
                // Check if this is a mutable parameter
                if let Some(&is_mutable) = self.mutable_params.get(name) {
                    if is_mutable {
                        // For arrays, don't dereference as they're already pointers.
                        // The recorded type of the binding answers this for the
                        // function being generated; searching every function's
                        // parameter list by name (the previous approach) both
                        // missed `&[T; N]` parameters and could answer with an
                        // unrelated function's parameter of the same name.
                        let is_array = self
                            .variables
                            .get(name)
                            .is_some_and(|ty| ty.contains('['));

                        if is_array {
                            // Arrays are already pointers, don't dereference
                            self.output.push_str(name);
                        } else {
                            // Dereference mutable parameters
                            self.output.push_str(&format!("(*{})", name));
                        }
                    } else {
                        self.output.push_str(name);
                    }
                } else {
                    // Regular variable
                    self.output.push_str(name);
                }
            }
            Expr::Call { func, args, .. } => {
                // A call is the other way to write into an array parameter, so
                // it is checked before anything is emitted.
                if let Expr::Ident(name) = func.as_ref() {
                    self.check_call_array_capabilities(name, args)?;
                }

                // Generate function name
                match func.as_ref() {
                    Expr::Ident(name) => {
                        // Map built-in functions. The C symbol is mechanically
                        // __pd_<name> for every built-in, so this derives from the
                        // registry (crate::builtins) instead of restating the table:
                        // a 38-arm match here was a hand-maintained copy that would
                        // silently miss a newly registered built-in, emitting a bare
                        // call to an undeclared C function.
                        match name.as_str() {
                            name if crate::builtins::is_builtin(name) => {
                                self.output.push_str(&format!("__pd_{}", name));
                            }
                            _ => {
                                // Check if this is a method call (contains ::)
                                if name.contains("::") {
                                    // Convert Type::method to __pd_Type_method
                                    let mangled = format!("__pd_{}", name.replace("::", "_"));
                                    self.output.push_str(&mangled);
                                } else if let Some(mangled_name) =
                                    self.get_mangled_name_for_call(name, args)
                                {
                                    // Check if this is a generic function that needs name mangling
                                    self.output.push_str(&mangled_name);
                                } else {
                                    self.output.push_str(name);
                                }
                            }
                        }
                    }
                    _ => {
                        return Err(CompileError::Generic(
                            "Indirect calls not yet supported".to_string(),
                        ));
                    }
                }

                // Generate arguments
                self.output.push('(');

                // Get function signature to check parameter mutability
                let func_params = match func.as_ref() {
                    Expr::Ident(name) => self.functions.get(name).map(|(params, _)| params.clone()),
                    _ => None,
                };

                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }

                    // Check if this parameter is mutable
                    let needs_address = if let Some(params) = &func_params {
                        if i < params.len() && params[i].mutable {
                            // Need to pass address for mutable parameters
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if needs_address {
                        // Check if argument is already a pointer (mutable param) or array
                        if let Expr::Ident(name) = arg {
                            if self.mutable_params.get(name).copied().unwrap_or(false) {
                                // Already a pointer, just pass it
                                self.output.push_str(name);
                            } else {
                                // Check if it's an array variable - arrays are already pointers
                                let var_type = self.variables.get(name).map(|s| s.as_str());
                                if var_type.is_some_and(|t| t.contains("[")) {
                                    // It's an array, don't take address
                                    self.generate_expression(arg)?;
                                } else {
                                    // Need to take address
                                    self.output.push('&');
                                    self.generate_expression(arg)?;
                                }
                            }
                        } else {
                            // Need to take address
                            self.output.push('&');
                            self.generate_expression(arg)?;
                        }
                    } else {
                        self.generate_expression(arg)?;
                    }
                }
                self.output.push(')');
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                // Check if this is string concatenation
                let left_type = self.infer_expr_type(left);
                let right_type = self.infer_expr_type(right);

                if matches!(op, BinOp::Add)
                    && left_type == "const char*"
                    && right_type == "const char*"
                {
                    // String concatenation - use helper function
                    self.output.push_str("__pd_string_concat(");
                    self.generate_expression(left)?;
                    self.output.push_str(", ");
                    self.generate_expression(right)?;
                    self.output.push(')');
                } else {
                    // Regular binary operation
                    self.output.push('(');

                    // Generate left operand
                    self.generate_expression(left)?;

                    // Generate operator
                    let op_str = match op {
                        BinOp::Add => " + ",
                        BinOp::Sub => " - ",
                        BinOp::Mul => " * ",
                        BinOp::Div => " / ",
                        BinOp::Mod => " % ",
                        BinOp::Eq => " == ",
                        BinOp::Ne => " != ",
                        BinOp::Lt => " < ",
                        BinOp::Gt => " > ",
                        BinOp::Le => " <= ",
                        BinOp::Ge => " >= ",
                        BinOp::And => " && ",
                        BinOp::Or => " || ",
                    };
                    self.output.push_str(op_str);

                    // Generate right operand
                    self.generate_expression(right)?;

                    self.output.push(')');
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                // Generate array literal: {1, 2, 3}
                self.output.push('{');
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.generate_expression(elem)?;
                }
                self.output.push('}');
            }
            Expr::ArrayRepeat { value, count, .. } => {
                // Generate array repeat initialization
                // For [0; 10], generate: {0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
                self.output.push('{');
                if let Expr::Integer(n) = count.as_ref() {
                    for i in 0..*n {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.generate_expression(value)?;
                    }
                }
                self.output.push('}');
            }
            Expr::Index { array, index, .. } => {
                // Generate array indexing: arr[i]
                self.generate_expression(array)?;
                self.output.push('[');
                self.generate_expression(index)?;
                self.output.push(']');
            }
            Expr::StructLiteral { name, fields, .. } => {
                // Generate struct literal: (StructName){.field1 = value1, .field2 = value2}
                // Check if this is a generic struct instantiation
                let struct_name =
                    if let Some(instantiations) = self.generic_struct_instantiation_map.get(name) {
                        // Need to determine which instantiation to use based on field types
                        // For now, we'll infer from the first field's type
                        if let Some((_field_name, field_expr)) = fields.first() {
                            let field_type = self.infer_expr_type(field_expr);

                            // Find the matching instantiation
                            let mut found_name = None;
                            for (type_args, mangled_name) in instantiations {
                                // Simple heuristic: check if any type arg matches the field type
                                for type_arg in type_args {
                                    if (type_arg == "i64" && field_type.contains("long long"))
                                        || (type_arg == "bool" && field_type == "int")
                                        || (type_arg == "String" && field_type.contains("char*"))
                                    {
                                        found_name = Some(mangled_name.as_str());
                                        break;
                                    }
                                }
                                if found_name.is_some() {
                                    break;
                                }
                            }
                            found_name.unwrap_or(name.as_str())
                        } else {
                            name.as_str()
                        }
                    } else {
                        // Use the original name for non-generic structs
                        name.as_str()
                    };

                self.output.push_str(&format!("(struct {})", struct_name));
                self.output.push('{');
                for (i, (field_name, field_expr)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&format!(".{} = ", field_name));
                    self.generate_expression(field_expr)?;
                }
                self.output.push('}');
            }
            Expr::FieldAccess { object, field, .. } => {
                // Check if object is a mutable parameter (pointer)
                let use_arrow = match object.as_ref() {
                    Expr::Ident(name) => self.mutable_params.get(name).copied().unwrap_or(false),
                    _ => false,
                };

                // Generate field access: obj.field or obj->field
                // Note: Don't generate expression for object if it's a mutable param
                // because we already handle the dereference in Expr::Ident
                if use_arrow {
                    // For mutable params, we need special handling
                    if let Expr::Ident(name) = object.as_ref() {
                        self.output.push_str(&format!("{}->{}", name, field));
                    } else {
                        self.generate_expression(object)?;
                        self.output.push_str(&format!("->{}", field));
                    }
                } else {
                    self.generate_expression(object)?;
                    self.output.push_str(&format!(".{}", field));
                }
            }
            Expr::EnumConstructor {
                enum_name,
                variant,
                data,
                ..
            } => {
                // Generate enum constructor call
                match data {
                    None => {
                        // Unit variant
                        self.output.push_str(&format!("{}_{}", enum_name, variant));
                        self.output.push_str("()");
                    }
                    Some(EnumConstructorData::Tuple(exprs)) => {
                        // Tuple variant
                        self.output
                            .push_str(&format!("{}_{}__new", enum_name, variant));
                        self.output.push('(');
                        for (i, expr) in exprs.iter().enumerate() {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            self.generate_expression(expr)?;
                        }
                        self.output.push(')');
                    }
                    Some(EnumConstructorData::Struct(fields)) => {
                        // Struct variant
                        self.output
                            .push_str(&format!("{}_{}__new", enum_name, variant));
                        self.output.push('(');
                        for (i, (_, expr)) in fields.iter().enumerate() {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            self.generate_expression(expr)?;
                        }
                        self.output.push(')');
                    }
                }
            }
            Expr::Range { .. } => {
                // Range expressions are not directly translatable to C
                // They should only appear in for loops which handle them specially
                return Err(CompileError::Generic(
                    "Range expressions can only be used in for loops".to_string(),
                ));
            }
            Expr::Unary { op, operand, .. } => {
                // Generate unary expression
                match op {
                    UnaryOp::Neg => {
                        self.output.push_str("(-(");
                        self.generate_expression(operand)?;
                        self.output.push_str("))");
                    }
                    UnaryOp::Not => {
                        self.output.push_str("(!(");
                        self.generate_expression(operand)?;
                        self.output.push_str("))");
                    }
                }
            }
            Expr::Reference { expr, .. } => {
                // Generate reference (address-of) expression.
                // C doesn't distinguish between & and &mut.
                //
                // A reference to an array is the exception: `&[T; N]` parameters
                // are received as `T*` (see `array_param_declarator`), so the
                // operand must *decay* rather than have its address taken —
                // `&arr` would be a `T (*)[N]`, a different pointer type.
                let is_array = self
                    .try_infer_expr_type(expr)
                    .is_some_and(|ty| ty.contains('['));
                if is_array {
                    self.generate_expression(expr)?;
                } else {
                    self.output.push_str("(&(");
                    self.generate_expression(expr)?;
                    self.output.push_str("))");
                }
            }
            Expr::Deref { expr, .. } => {
                // Generate dereference expression
                self.output.push_str("(*(");
                self.generate_expression(expr)?;
                self.output.push_str("))");
            }
            Expr::Question { expr: _, span } => {
                // D5. The type checker refuses this first, but this backend is
                // what emitted the undefined `struct Result`, and it is callable
                // on its own, so the refusal also lives at the defect.
                return Err(CompileError::question_unimplemented(*span));
            }
            Expr::MacroInvocation { .. } => {
                // Macros should have been expanded before codegen
                return Err(CompileError::Generic(
                    "Unexpected macro invocation in code generation - macros should be expanded before this phase".to_string()
                ));
            }
            Expr::Await { expr: _, span } => {
                // D5. Same shape as `?` above: this arm emitted
                // `future.poll(&future)`, which is not C, while the poll routine
                // that is generated is the free function `<name>_poll`.
                return Err(CompileError::await_unimplemented(*span));
            }
        }
        Ok(())
    }

    /// Create a monomorphized version of a generic struct
    fn monomorphize_struct(
        &self,
        struct_name: &str,
        type_args: &[String],
        generic_struct: &crate::typeck::GenericStruct,
    ) -> Result<StructDef> {
        // Generate a mangled name for the concrete struct
        let mangled_name = format!("{}_{}", struct_name, type_args.join("_"));

        // Create a mapping from type parameters to concrete types
        let mut type_map = std::collections::HashMap::new();
        for (i, type_param) in generic_struct.type_params.iter().enumerate() {
            if i < type_args.len() {
                type_map.insert(type_param.clone(), type_args[i].clone());
            }
        }

        // Substitute types in fields
        let concrete_fields = generic_struct
            .fields
            .iter()
            .map(|(name, ty)| {
                let concrete_type = self.substitute_type(ty, &type_map);
                (name.clone(), concrete_type)
            })
            .collect();

        // Create the concrete struct
        Ok(StructDef {
            name: mangled_name,
            lifetime_params: vec![], // No longer generic
            type_params: vec![],     // No longer generic
            const_params: vec![],    // No longer generic
            fields: concrete_fields,
            visibility: crate::ast::Visibility::Private, // Monomorphized structs are internal
            span: Span {
                start: 0,
                end: 0,
                line: 0,
                column: 0,
            }, // Synthetic span for generated struct
        })
    }

    /// Create a monomorphized version of a generic function
    fn monomorphize_function(
        &self,
        func_name: &str,
        type_args: &[String],
        generic_func: &crate::typeck::GenericFunction,
    ) -> Result<Function> {
        // Generate a mangled name for the concrete function
        let mangled_name = format!("{}__{}", func_name, type_args.join("_"));

        // Create a mapping from type parameters to concrete types
        let mut type_map = std::collections::HashMap::new();
        for (i, type_param) in generic_func.type_params.iter().enumerate() {
            if i < type_args.len() {
                type_map.insert(type_param.clone(), type_args[i].clone());
            }
        }

        // Substitute types in parameters
        let concrete_params = generic_func
            .params
            .iter()
            .map(|(name, ty)| {
                let concrete_type = self.substitute_type(ty, &type_map);
                Param {
                    name: name.clone(),
                    ty: concrete_type,
                    mutable: false, // TODO: Preserve mutability from original
                }
            })
            .collect();

        // Substitute type in return type
        let concrete_return_type = generic_func
            .return_type
            .as_ref()
            .map(|ty| self.substitute_type(ty, &type_map));

        // Substitute types in the function body
        let concrete_body = self.substitute_types_in_body(&generic_func.body, &type_map);

        // Create the concrete function
        //
        // `is_async` AND `span` COME FROM THE TEMPLATE. This read
        // `is_async: false, // Monomorphized functions are not async`, which
        // made the claim true BY ERASING IT: an `async fn g<T>` that was
        // instantiated arrived here async and left synchronous, so it emitted
        // an ordinary `g__i64`, the keyword was silently dropped, and every
        // `is_async` guard downstream — including this file's own
        // `if concrete_func.is_async { continue; }` in the prototype loop — was
        // dead code that read as coverage. Monomorphisation substitutes TYPES;
        // it is not the place that decides an effect question. The span travels
        // with it so the N7-18 refusal points at the declaration the programmer
        // wrote rather than at the synthetic `Span { 0, 0, 0, 0 }` this invented.
        Ok(Function {
            name: mangled_name,
            is_async: generic_func.is_async,
            lifetime_params: vec![], // No longer generic
            type_params: vec![],     // No longer generic
            const_params: vec![],    // No longer generic
            params: concrete_params,
            return_type: concrete_return_type,
            body: concrete_body,
            visibility: crate::ast::Visibility::Private, // Monomorphized functions are internal
            span: generic_func.span,
            effects: None, // Effects are not tracked for monomorphized functions yet
        })
    }

    /// Create a mangled name for a generic function
    fn mangle_generic_name(&self, func_name: &str, type_args: &[String]) -> String {
        format!("{}__{}", func_name, type_args.join("_"))
    }

    /// Get the mangled name for a generic function call
    fn get_mangled_name_for_call(&self, func_name: &str, args: &[Expr]) -> Option<String> {
        // Check if this function has generic instantiations
        let mut instantiations_for_func = Vec::new();
        for (name, type_args, _) in &self.generic_instantiations {
            if name == func_name {
                instantiations_for_func.push(type_args.clone());
            }
        }

        if instantiations_for_func.is_empty() {
            return None;
        }

        // If there's only one instantiation, use it
        if instantiations_for_func.len() == 1 {
            return Some(self.mangle_generic_name(func_name, &instantiations_for_func[0]));
        }

        // Try to infer which instantiation based on the first argument type
        if let Some(first_arg) = args.first() {
            let arg_type_str = match first_arg {
                Expr::ArrayLiteral { elements, .. } => {
                    if !elements.is_empty() {
                        // Infer from first element
                        self.infer_expr_type(&elements[0])
                    } else {
                        return None;
                    }
                }
                Expr::Ident(name) => {
                    // Look up variable type
                    self.variables
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| self.infer_expr_type(first_arg))
                }
                _ => self.infer_expr_type(first_arg),
            };

            // Find best matching instantiation
            for type_args in &instantiations_for_func {
                // Check if any type arg matches our inferred type
                for type_arg in type_args {
                    if type_arg == "i64" && arg_type_str.contains("long long") {
                        return Some(self.mangle_generic_name(func_name, type_args));
                    }
                    if type_arg == "bool"
                        && arg_type_str.contains("int")
                        && !arg_type_str.contains("long")
                    {
                        return Some(self.mangle_generic_name(func_name, type_args));
                    }
                    if type_arg == "String" && arg_type_str.contains("char*") {
                        return Some(self.mangle_generic_name(func_name, type_args));
                    }
                    if type_arg == &arg_type_str {
                        return Some(self.mangle_generic_name(func_name, type_args));
                    }
                }
            }
        }

        // Default to first instantiation if we can't determine
        Some(self.mangle_generic_name(func_name, &instantiations_for_func[0]))
    }

    /// Substitute type parameters with concrete types in a type
    #[allow(clippy::only_used_in_recursion)]
    fn substitute_type(
        &self,
        ty: &Type,
        type_map: &std::collections::HashMap<String, String>,
    ) -> Type {
        match ty {
            Type::TypeParam(name) => {
                // Replace type parameter with concrete type
                if let Some(concrete_name) = type_map.get(name) {
                    // Parse the concrete type name
                    match concrete_name.as_str() {
                        "i32" | "I32" => Type::I32,
                        "i64" | "I64" => Type::I64,
                        "u32" | "U32" => Type::U32,
                        "u64" | "U64" => Type::U64,
                        "bool" | "Bool" => Type::Bool,
                        "string" | "String" => Type::String,
                        _ => Type::Custom(concrete_name.clone()),
                    }
                } else {
                    ty.clone()
                }
            }
            Type::Array(elem_type, size) => Type::Array(
                Box::new(self.substitute_type(elem_type, type_map)),
                size.clone(),
            ),
            Type::Generic { name, args } => {
                // Substitute in generic type arguments
                let substituted_args = args
                    .iter()
                    .map(|arg| match arg {
                        GenericArg::Type(t) => GenericArg::Type(self.substitute_type(t, type_map)),
                        GenericArg::Const(c) => GenericArg::Const(c.clone()), // TODO: substitute const params
                    })
                    .collect();
                Type::Generic {
                    name: name.clone(),
                    args: substituted_args,
                }
            }
            Type::Custom(name) => {
                // Check if this custom type is actually a type parameter
                if let Some(concrete_name) = type_map.get(name) {
                    // Parse the concrete type name
                    match concrete_name.as_str() {
                        "i32" | "I32" => Type::I32,
                        "i64" | "I64" => Type::I64,
                        "u32" | "U32" => Type::U32,
                        "u64" | "U64" => Type::U64,
                        "bool" | "Bool" => Type::Bool,
                        "string" | "String" => Type::String,
                        _ => Type::Custom(concrete_name.clone()),
                    }
                } else {
                    ty.clone()
                }
            }
            _ => ty.clone(),
        }
    }

    /// Substitute types in a statement body
    fn substitute_types_in_body(
        &self,
        stmts: &[Stmt],
        _type_map: &std::collections::HashMap<String, String>,
    ) -> Vec<Stmt> {
        // For now, we'll just clone the body
        // In a full implementation, we'd need to walk the AST and substitute types
        // This is a simplified version that works for basic cases
        stmts.to_vec()
    }

    /// The C source generated so far.
    ///
    /// Exists so that the registry-to-runtime seam test in `crate::builtins` can
    /// read the C prelude this compiler actually emits, rather than a copy of it.
    #[allow(dead_code)] // used by the drift tests in crate::builtins
    pub(crate) fn generated_c(&self) -> &str {
        &self.output
    }

    /// Write the generated code to a file
    pub fn write_output(&self) -> Result<PathBuf> {
        // Create build_output directory if it doesn't exist
        let build_dir = PathBuf::from("build_output");
        if !build_dir.exists() {
            fs::create_dir_all(&build_dir)?;
        }

        // Clean module name (remove .pd extension if present)
        let base_name = Path::new(&self.module_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&self.module_name);

        let output_path = build_dir.join(format!("{}.c", base_name));
        let mut file = File::create(&output_path)?;
        file.write_all(self.output.as_bytes())?;

        println!("   Generated C code: {}", output_path.display());

        Ok(output_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    #[test]
    fn test_codegen_hello_world() {
        let source = r#"
        fn main() {
            print("Hello, World!");
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());

        // Check generated code contains expected elements
        assert!(codegen.output.contains("int main(int argc, char** argv)"));
        assert!(codegen.output.contains("__pd_argc = argc;"));
        assert!(codegen.output.contains("__pd_print(\"Hello, World!\")"));
    }

    #[test]
    fn test_codegen_let_binding() {
        let source = r#"
        fn main() {
            let x: i32 = 42;
            let y = 100;
            print_int(x);
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());

        // Check generated code contains expected elements
        assert!(codegen.output.contains("int x = 42;"));
        assert!(codegen.output.contains("long long y = 100;"));
        assert!(codegen.output.contains("__pd_print_int(x)"));
    }

    #[test]
    fn test_codegen_binary_operations() {
        let source = r#"
        fn main() {
            let x = 10;
            let y = 20;
            let sum = x + y;
            let product = x * y;
            print_int(sum);
            print_int(product);
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());

        // Check generated code contains expected elements
        assert!(codegen.output.contains("long long sum = (x + y);"));
        assert!(codegen.output.contains("long long product = (x * y);"));
        assert!(codegen.output.contains("__pd_print_int(sum)"));
        assert!(codegen.output.contains("__pd_print_int(product)"));
    }

    #[test]
    fn test_codegen_comparison_operations() {
        let source = r#"
        fn main() -> i32 {
            let a = 5;
            let b = 10;
            let result = a < b;
            return result;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());

        // Check generated code contains expected elements
        assert!(codegen.output.contains("int main(int argc, char** argv)"));
        // A comparison yields a bool, which is `int` in C - not the operand type.
        assert!(codegen.output.contains("int result = (a < b);"));
        assert!(codegen.output.contains("return result;"));
    }

    #[test]
    fn test_codegen_for_loop() {
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

        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());

        // Check generated code contains for loop
        assert!(codegen.output.contains("for (long long _i = 0;"));
        assert!(codegen.output.contains("long long i = arr[_i];"));
        assert!(codegen.output.contains("__pd_print_int(i)"));
    }

    #[test]
    fn test_codegen_break_continue() {
        let source = r#"
        fn main() {
            while true {
                if x > 10 {
                    break;
                }
                if x == 5 {
                    continue;
                }
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut codegen = CodeGenerator::new("test").unwrap();
        assert!(codegen.compile(&ast).is_ok());

        // Check generated code contains break and continue
        assert!(codegen.output.contains("break;"));
        assert!(codegen.output.contains("continue;"));
    }

    /// Compile a program and return the generated C, for the inference tests.
    fn generate(source: &str) -> Result<String> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut codegen = CodeGenerator::new("test").unwrap();
        codegen.compile(&ast)?;
        Ok(codegen.output)
    }

    // An un-annotated `let` used to run its own ad-hoc inference and fall back
    // to `long long` for every expression kind it did not enumerate, so a
    // reference, an enum value or a string was declared as an integer and the
    // program only failed later, inside gcc, against C the user never wrote.

    #[test]
    fn test_infer_let_reference() {
        let c = generate(
            r#"
        fn main() {
            let x = 42;
            let y = &x;
            print_int(*y);
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("long long* y ="), "{}", c);
    }

    #[test]
    fn test_infer_let_enum_constructor() {
        let c = generate(
            r#"
        enum N { A(i64), B }
        fn main() {
            let n = N::A(5);
            match n {
                N::A(v) => { print_int(v); }
                N::B => { print("b"); }
            }
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("struct N n = N_A__new(5);"), "{}", c);
    }

    #[test]
    fn test_infer_let_string_copy() {
        let c = generate(
            r#"
        fn main() {
            let a: String = "hello";
            let b = a;
            print(b);
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("const char* b = a;"), "{}", c);
    }

    #[test]
    fn test_infer_let_field_access_and_index() {
        let c = generate(
            r#"
        struct P { name: String, age: i64 }
        fn main() {
            let p = P { name: "z", age: 3 };
            let n = p.name;
            let arr = ["a", "b"];
            let first = arr[0];
            print(n);
            print(first);
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("const char* n = "), "{}", c);
        assert!(c.contains("const char* arr[2] = "), "{}", c);
        assert!(c.contains("const char* first = "), "{}", c);
    }

    #[test]
    fn test_infer_let_comparison_is_bool() {
        let c = generate(
            r#"
        fn main() {
            let a = 1;
            let b = 2;
            let c = a < b;
            let d = !c;
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("int c = (a < b);"), "{}", c);
        assert!(c.contains("int d = "), "{}", c);
    }

    #[test]
    fn test_infer_let_call_return_types() {
        let c = generate(
            r#"
        struct P { age: i64 }
        fn make() -> P { return P { age: 1 }; }
        fn main() {
            let p = make();
            let n = string_len("hi");
            let s = int_to_string(7);
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("struct P p = make();"), "{}", c);
        assert!(c.contains("long long n = "), "{}", c);
        assert!(c.contains("const char* s = "), "{}", c);
    }

    #[test]
    fn test_uninferable_let_is_a_compile_error_not_bad_c() {
        // `await` has no code generation rule; the old catch-all declared the
        // binding as `long long` and emitted C that gcc rejected.
        //
        // `work` USED TO BE DECLARED `async fn`, which was incidental to this
        // test and is now refused first (N7-18), so the assertions below were
        // measuring the wrong refusal. `.await` is what this test is about, and
        // it is written on an ordinary call — the operand is not inspected
        // (`CompileError::await_unimplemented`), and `try_infer_expr_type`
        // answers `None` for `Expr::Await` whatever the callee is.
        let err = generate(
            r#"
        fn work() -> i64 { return 1; }
        fn main() {
            let v = work().await;
        }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot infer the type of `v`"), "{}", msg);
        assert!(msg.contains("await"), "{}", msg);
        assert!(msg.contains("explicit type annotation"), "{}", msg);
    }

    // D5. The type checker now refuses `?` and `.await` before code generation
    // runs, but this backend is reachable on its own (this very harness skips
    // the type checker) and it is what emitted the bad C. The refusal is
    // therefore asserted at both ends. `generate` returns the emitted C on
    // success, so `unwrap_err` is the whole contract here: an Ok of any kind,
    // empty or not, fails these tests. (An earlier version asserted that the
    // output did *not* contain "struct Result" via `unwrap_or_default`, which
    // an error satisfies vacuously — it could not tell refusal from success.)

    #[test]
    fn test_question_codegen_refuses_instead_of_fabricating_struct_result() {
        let err = generate(
            r#"
        fn helper(x: i64) -> Result<i64, i64> {
            let v: i64 = might_fail(x)?;
            return might_fail(v);
        }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("`?` operator"), "{}", msg);
        assert!(msg.contains("not implemented"), "{}", msg);
    }

    #[test]
    fn test_await_codegen_refuses_instead_of_calling_a_poll_member() {
        let err = generate(
            r#"
        fn main() {
            let v: i64 = work(3).await;
        }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("`.await`"), "{}", msg);
        assert!(msg.contains("not implemented"), "{}", msg);
    }

    // A reference to an array had no case in the parameter arm at all, so
    // `fn f(xs: &mut [i64; 3])` was rejected with "Unsupported type in
    // reference parameter" and examples/practical/simple_sort.pd could not be
    // compiled. C decays array parameters to pointers, so the referent is
    // passed like the array itself and `&` vs `&mut` is a const difference.
    #[test]
    fn test_array_reference_parameters_compile_to_pointers() {
        let c = generate(
            r#"
        fn read(xs: &[i64; 3]) -> i64 { return xs[0]; }
        fn write(xs: &mut [i64; 3]) { xs[0] = 9; }
        fn main() {
            let mut values = [1, 2, 3];
            write(&mut values);
            print_int(read(&values));
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("long long read(const long long xs[3])"), "{}", c);
        assert!(c.contains("void write(long long xs[3])"), "{}", c);
        // The body indexes the pointer directly - `(*xs)[0]` would not compile.
        assert!(c.contains("xs[0] = 9;"), "{}", c);
        // `&values` must decay: `&(values)` is a `long long (*)[3]`, a
        // different pointer type from the parameter's `long long*`.
        assert!(c.contains("write(values)"), "{}", c);
        assert!(c.contains("read(values)"), "{}", c);
    }

    // `sizeof(xs)/sizeof(xs[0])` counts elements only for an array *object*.
    // A parameter has already decayed to a pointer, so for `[i64; N]` that is
    // 8/8 = 1 and the loop silently ran exactly once.
    #[test]
    fn test_for_over_array_parameter_uses_the_declared_length() {
        let c = generate(
            r#"
        fn total(xs: [i64; 4]) {
            for x in xs {
                print_int(x);
            }
        }
        fn main() {
            let nums = [1, 2, 3, 4];
            total(nums);
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("for (long long _i = 0; _i < 4; _i++)"), "{}", c);
        assert!(!c.contains("sizeof(xs)"), "{}", c);
    }

    // Zero is a length, not a missing length. Routing it through the printed C
    // type collapsed it onto "unknown" and fell back to the sizeof ratio, so
    // the loop ran once and read element 0 of an empty array.
    #[test]
    fn test_for_over_zero_length_parameter_does_not_iterate() {
        let c = generate(
            r#"
        fn nothing(xs: [i64; 0]) {
            for x in xs {
                print_int(x);
            }
        }
        fn main() { print("ok"); }
        "#,
        )
        .unwrap();
        assert!(c.contains("for (long long _i = 0; _i < 0; _i++)"), "{}", c);
        assert!(!c.contains("sizeof(xs)"), "{}", c);
    }

    // A length the compiler never resolved must not become a number: const
    // generic lengths are dropped (language-spec.md §5), and the parameter has
    // decayed, so neither the type nor `sizeof` can supply the count.
    #[test]
    fn test_unresolved_parameter_length_is_a_diagnostic_not_a_sizeof() {
        let err = generate(
            r#"
        fn total(xs: [i64; N]) {
            for x in xs {
                print_int(x);
            }
        }
        fn main() { print("ok"); }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot iterate `xs`"), "{}", msg);
        assert!(msg.contains("`N`"), "{}", msg);
    }

    // An unresolved length must not be *printed* either: `long long xs[N]` put
    // a Palladium const generic into C, where gcc reported "use of undeclared
    // identifier 'N'" against code the user never wrote.
    #[test]
    fn test_unresolved_parameter_length_decays_instead_of_naming_it() {
        let c = generate(
            r#"
        fn head(xs: [i64; N]) -> i64 { return xs[0]; }
        fn main() { print("ok"); }
        "#,
        )
        .unwrap();
        assert!(c.contains("long long head(long long xs[])"), "{}", c);
        assert!(!c.contains("xs[N]"), "{}", c);
    }

    // A `String` element is itself `const char*`, so const-qualifying the
    // element type only froze the characters: `const char* xs[N]` still allows
    // `xs[i] = other`, and the mutable form came out as `char**`, which is an
    // incompatible pointer type against the caller's `const char**`.
    #[test]
    fn test_string_array_reference_qualifies_the_slot_not_the_characters() {
        let c = generate(
            r#"
        fn read(names: &[String; 2]) -> String { return names[0]; }
        fn write(names: &mut [String; 2]) { names[0] = "x"; }
        fn main() {
            let mut names: [String; 2] = ["a", "b"];
            write(&mut names);
            print(read(&names));
        }
        "#,
        )
        .unwrap();
        assert!(
            c.contains("const char* read(const char* const names[2])"),
            "{}",
            c
        );
        assert!(c.contains("void write(const char* names[2])"), "{}", c);
        // The pre-fix spelling: an unqualified `char*` element, which is a
        // `char**` parameter against the caller's `const char**`.
        assert!(!c.contains("(char* names"), "{}", c);
    }

    // Every array parameter decays to a pointer into the caller's array, so a
    // write through one is visible to the caller no matter how it was spelled.
    // Only the two spellings that declare the intent may do it.
    #[test]
    fn test_write_through_shared_array_reference_is_rejected() {
        let err = generate(
            r#"
        fn f(xs: &[i64; 3]) { xs[0] = 99; }
        fn main() { let mut v = [1, 2, 3]; f(&v); }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot write to `xs`"), "{}", msg);
        assert!(msg.contains("shared reference"), "{}", msg);
    }

    #[test]
    fn test_write_to_by_value_array_parameter_is_rejected() {
        let err = generate(
            r#"
        fn f(xs: [i64; 3]) { xs[0] = 99; }
        fn main() { let mut v = [1, 2, 3]; f(v); print_int(v[0]); }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot write to `xs`"), "{}", msg);
        // The caller used to observe 99 through a parameter that never said it
        // could be written; the diagnostic has to name the undecided semantics.
        assert!(msg.contains("not decided by the language"), "{}", msg);
    }

    // Refusing the assignment is not enough while a call can hand the write on:
    // nothing re-checks reference mutability after the parser, so the callee's
    // `&mut` binding made the write legitimate at the point it happened. Both
    // forwardings put 99 in the caller's array before this check existed.
    #[test]
    fn test_shared_array_parameter_cannot_be_forwarded_as_mutable() {
        let err = generate(
            r#"
        fn mutate(xs: &mut [i64; 3]) { xs[0] = 99; }
        fn f(xs: &[i64; 3]) { mutate(xs); }
        fn main() { let mut v = [1, 2, 3]; f(&v); print_int(v[0]); }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot pass `xs` to `mutate`"), "{}", msg);
        assert!(msg.contains("shared reference parameter"), "{}", msg);
    }

    #[test]
    fn test_by_value_array_parameter_cannot_be_forwarded_as_mutable() {
        let err = generate(
            r#"
        fn mutate(xs: &mut [i64; 3]) { xs[0] = 99; }
        fn g(xs: [i64; 3]) { mutate(xs); }
        fn main() { let mut v = [1, 2, 3]; g(v); print_int(v[0]); }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot pass `xs` to `mutate`"), "{}", msg);
        assert!(msg.contains("by-value array parameter"), "{}", msg);
    }

    #[test]
    fn test_forwarding_a_mutable_array_parameter_is_still_allowed() {
        // The permission exists here, so passing it on is not laundering.
        let c = generate(
            r#"
        fn mutate(xs: &mut [i64; 3]) { xs[0] = 99; }
        fn f(xs: &mut [i64; 3]) { mutate(xs); }
        fn main() { let mut v = [1, 2, 3]; f(&mut v); print_int(v[0]); }
        "#,
        )
        .unwrap();
        assert!(c.contains("void f(long long xs[3])"), "{}", c);
        assert!(c.contains("mutate(xs)"), "{}", c);
    }

    // A flat, function-wide binding map let a block-local `xs` stand in for the
    // parameter after the block closed: the write guard saw an owned local and
    // allowed a write straight into the caller's array.
    #[test]
    fn test_shadowing_does_not_launder_write_permission() {
        let err = generate(
            r#"
        fn f(xs: [i64; 3]) {
            if true {
                let xs: [i64; 2] = [1, 2];
                print_int(xs[0]);
            }
            xs[0] = 99;
        }
        fn main() { let mut v = [1, 2, 3]; f(v); print_int(v[0]); }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot write to `xs`"), "{}", msg);
        assert!(msg.contains("by-value array parameter"), "{}", msg);
    }

    // Unknown capability must not read as permitted: with no signature for the
    // callee there is no way to tell whether it writes through the array, and
    // every array is passed as a pointer into the caller's storage.
    //
    // A builtin is the reachable callee that `functions` does not contain.
    // `impl` methods were the case the review named, but `Type::method(x)` does
    // not reach this guard at all: the parser builds an `EnumConstructor`, not
    // a `Call` (it emits `Holder_mutate__new(xs)` and the type checker rejects
    // it with "Undefined enum type: Holder"), so that hole is latent rather
    // than live. The refusal covers it if `::` calls are ever routed here.
    #[test]
    fn test_array_passed_to_an_unknown_callee_is_refused() {
        let err = generate(
            r#"
        fn main() {
            let arr = [1, 2, 3];
            print_int(arr);
        }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot pass an array to `print_int`"), "{}", msg);
        assert!(msg.contains("does not know that callee"), "{}", msg);
    }

    // Same rule for an argument whose provenance is unknown: `b.items` roots to
    // `b`, which has no array binding, so the guard used to wave it through
    // even when `b` was a shared parameter.
    #[test]
    fn test_array_argument_with_unknown_provenance_is_refused() {
        let err = generate(
            r#"
        struct Bag { items: [i64; 3] }
        fn mutate(xs: &mut [i64; 3]) { xs[0] = 99; }
        fn f(mut b: Bag) { mutate(b.items); }
        fn main() { let mut bag = Bag { items: [1, 2, 3] }; f(bag); }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot establish where the array came from"), "{}", msg);
        assert!(msg.contains("field access"), "{}", msg);
    }

    // The scope must open before the binder is written, not before the block
    // body: a `for` variable is recorded first, so snapshotting inside
    // `generate_block` captured the already-overwritten map and the binder
    // outlived its loop. And a non-array binder has to *shadow* an outer array
    // of the same name in every map, or the outer array keeps answering for it.
    #[test]
    fn test_for_binder_does_not_outlive_its_loop_in_codegen() {
        let c = generate(
            r#"
        fn main() {
            let v: [String; 2] = ["a", "b"];
            let ys = [7, 8];
            for v in ys {
                print_int(v);
            }
            for s in v {
                print(s);
            }
        }
        "#,
        )
        .unwrap();
        // The second loop iterates the outer `[String; 2]`, so its element
        // declaration must be `const char*` and its bound 2 - both of which
        // come from bindings the loop variable had overwritten.
        assert!(c.contains("const char* s = v[_i];"), "{}", c);
        assert!(c.contains("for (long long _i = 0; _i < 2; _i++)"), "{}", c);
    }

    #[test]
    fn test_non_array_binder_shadows_an_outer_array() {
        // `v` names an array outside the loop and an integer inside it; the
        // inner one must not be refused as "an array" by the capability guard.
        let c = generate(
            r#"
        fn main() {
            let v = [1, 2, 3];
            let ys = [7, 8];
            for v in ys {
                print_int(v);
            }
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("__pd_print_int(v)"), "{}", c);
    }

    // An *array-valued* element - a row of a nested array - is refused as
    // unknown provenance, which is what §9.2 promises. Rooting it at its array
    // (`grid[0]` -> `grid`) would let it inherit that array's capability: a
    // defensible rule, but not the documented one, and the code used to do it
    // while the specification said otherwise.
    #[test]
    fn test_array_valued_element_is_refused_as_unknown_provenance() {
        let err = generate(
            r#"
        fn mutate(row: &mut [i64; 2]) { row[0] = 99; }
        fn main() {
            let mut grid: [[i64; 2]; 2] = [[1, 2], [3, 4]];
            mutate(&mut grid[0]);
        }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot establish where the array came from"), "{}", msg);
        assert!(msg.contains("array index"), "{}", msg);
    }

    // The refusal must not swallow ordinary code: an array *element* is not an
    // array, and reading `arr[0]` into a builtin is the commonest line there
    // is. Rooting the test at the array name refused five corpus files.
    #[test]
    fn test_passing_an_array_element_to_a_builtin_is_not_refused() {
        let c = generate(
            r#"
        fn main() {
            let arr = [1, 2, 3];
            print_int(arr[0]);
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("__pd_print_int(arr[0])"), "{}", c);
    }

    // `generate_block` restored the array bindings but not `variables`, from
    // which the loop's element declaration is taken, so a shadow of a different
    // element type made the *outer* array iterate with the wrong C type.
    #[test]
    fn test_shadowing_does_not_leak_its_element_type_to_the_parameter() {
        let c = generate(
            r#"
        fn show(names: &[String; 3]) {
            if true {
                let names: [i64; 2] = [1, 2];
                print_int(names[0]);
            }
            for n in names {
                print(n);
            }
        }
        fn main() {
            let ns: [String; 3] = ["a", "b", "c"];
            show(&ns);
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("const char* n = names[_i];"), "{}", c);
        assert!(!c.contains("long long n = names[_i];"), "{}", c);
    }

    #[test]
    fn test_shadowing_does_not_leak_its_length_to_the_parameter() {
        let c = generate(
            r#"
        fn f(xs: [i64; 4]) {
            if true {
                let xs: [i64; 2] = [9, 9];
                print_int(xs[0]);
            }
            for x in xs {
                print_int(x);
            }
        }
        fn main() { let v = [1, 2, 3, 4]; f(v); }
        "#,
        )
        .unwrap();
        // The inner loop-free block keeps its own length; the parameter keeps 4.
        assert!(c.contains("for (long long _i = 0; _i < 4; _i++)"), "{}", c);
        assert!(!c.contains("_i < 2;"), "{}", c);
    }

    // The prototype and the definition are built by one helper, so they cannot
    // disagree - but "cannot" is worth one assertion that checks both places.
    #[test]
    fn test_unresolved_length_decays_in_prototype_and_definition_alike() {
        let c = generate(
            r#"
        fn head(xs: [i64; N]) -> i64 { return xs[0]; }
        fn other() -> i64 { return 1; }
        fn main() { print_int(other()); }
        "#,
        )
        .unwrap();
        assert!(c.contains("long long head(long long xs[]);"), "{}", c);
        assert!(c.contains("long long head(long long xs[]) {"), "{}", c);
        assert_eq!(c.matches("long long head(long long xs[]").count(), 2, "{}", c);
    }

    // The supported element types are exactly what the declarator enumerates.
    // A nested array is rejected by name rather than emitted as invalid C, and
    // function types do not reach here at all - the parser refuses them
    // ("expected type, found 'fn'", language-spec.md §5).
    #[test]
    fn test_nested_array_parameter_is_rejected_by_name() {
        let err = generate(
            r#"
        fn f(g: [[i64; 2]; 3]) -> i64 { return 0; }
        fn main() { print("ok"); }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Unsupported array element type in function parameter"),
            "{}",
            msg
        );
    }

    #[test]
    fn test_declared_mutable_array_parameters_may_be_written() {
        // `&mut [T; N]`, and `mut xs: [T; N]` - the spelling the bootstrap
        // subset mandates (bootstrap-subset.md:95) - both stay legal.
        let c = generate(
            r#"
        fn a(xs: &mut [i64; 3]) { xs[0] = 1; }
        fn b(mut xs: [i64; 3]) { xs[0] = 2; }
        fn main() { let mut v = [1, 2, 3]; a(&mut v); b(v); }
        "#,
        )
        .unwrap();
        assert!(c.contains("void a(long long xs[3])"), "{}", c);
        assert!(c.contains("void b(long long xs[3])"), "{}", c);
    }
}
