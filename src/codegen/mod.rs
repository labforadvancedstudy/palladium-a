// Code generation for Palladium
// "Forging legends into machine code"

pub mod c_ident;
pub mod c_literal;
pub mod llvm_backend;
pub mod llvm_backend_improved;
pub mod llvm_native;
pub mod llvm_text_backend;

use crate::ast::{AssignTarget, UnaryOp, *};
use crate::codegen::c_literal::{c_char_constant, c_string_body};
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
    /// parameter (docs/specification/bootstrap-subset.md:152-154).
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
    /// The top-level `const` and `static` names with their C types (N3-09,
    /// N3-10). `variables` is CLEARED at every function boundary, and a
    /// top-level item outlives every boundary — so it is kept here and copied
    /// back in when the map is reset. Without that, a body reading a global was
    /// emitted correctly and then had no type for it, which is only visible
    /// where a type is required: `let t = (X, 1);` could not name its tuple's
    /// shape.
    globals: std::collections::HashMap<String, String>,
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
    /// The C name of the function being generated, for the match trap's message
    /// (N6-11). The generator has spans but no source FILE name, so "where" is
    /// the function plus the match's line — which is what a reader of a crash
    /// needs and all this pass can honestly say.
    current_fn_name: String,
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
    /// `Type::method` -> its declared parameters, RECEIVER INCLUDED.
    ///
    /// `impl_methods` above carries return types only, and `functions` never held
    /// methods at all, so a call site rewritten to `C::get(c)` resolved NO parameter
    /// list and every argument was emitted by value. For `self` that is right; for
    /// `&self` and `&mut self`, which lower to pointers, it emitted `__pd_C_get(c)`
    /// against `const struct C*` and gcc refused the call the front end had approved.
    impl_method_params: std::collections::HashMap<String, Vec<Param>>,
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
    /// Which enum payload slots are stored behind a pointer.
    ///
    /// NOT DERIVED HERE. The type checker owns the definition
    /// (`crate::typeck::RecursiveLayout`) because it also owns the refusal for
    /// the declarations this scheme cannot lay out, and a second derivation in
    /// this file is exactly how the two passes would come to disagree about
    /// which slot is a pointer — the class of defect this repository has closed
    /// twice, in `builtins.rs` and in `local_definition_shadows_import`.
    ///
    /// Filled in `compile_escaped` from the ESCAPED program, because the layout
    /// is keyed by type name and `escape_reserved_names` can rename one.
    recursive_layout: crate::typeck::RecursiveLayout,
    /// Every distinct TUPLE SHAPE this translation unit needs, in the order it
    /// must be defined (N4-12).
    ///
    /// C has no tuple, so each shape becomes a struct. The key is the mangled
    /// name and the value is the element C types, which is also what
    /// `Expr::TupleIndex` reads back to type an element: the mangled name alone
    /// cannot be un-mangled, and guessing is how a `.1` would get the type of
    /// `.0`. Insertion order is definition order, and a nested shape registers
    /// its inner shapes first because its own element types ARE those names.
    tuple_shapes: indexmap_lite::OrderedMap,
    /// Variant constructors, held back until every type definition has been
    /// emitted. They are the only output that needs a payload type COMPLETE,
    /// and for a terminating mutual recursion no ordering of the definitions
    /// can give them that in place.
    enum_constructors: String,
    /// Statements hoisted out of the expression currently being generated, to
    /// be spliced in front of the statement that contains it.
    ///
    /// C HAS NO EXPRESSION WITH A BLOCK IN IT. `let x = if c { 1 } else { 2 };`
    /// has to become a declaration, an `if` statement that assigns it, and a
    /// use of the name — three statements where Palladium wrote one. GNU
    /// statement-expressions (`({ ... })`) would express it in one, and are not
    /// available: the backend is plain `gcc`/`cc` invoked without `-std=gnu*`
    /// guarantees (`src/linker.rs`), and portable C is the contract.
    ///
    /// Emptied by `generate_statement`, which inserts it at the point the
    /// statement started. Nested value expressions work because each
    /// `generate_statement` saves and restores this buffer, so an inner
    /// hoist lands inside the branch it belongs to and not in front of the
    /// outer statement.
    pending_hoists: String,
    /// The C type discovered for a hoisted temporary, keyed by its name.
    ///
    /// A value `match` and a value `loop` are lowered by SYNTHESISING the
    /// statement form with `<temp> = <value>;` written into each arm / each
    /// value-carrying `break`, and generating that with the ordinary emitter.
    /// That reuse is the point: it is what makes an arm's pattern bindings
    /// (`Payload::Num(n) => n * 10`) visible to the arm's value, without a
    /// second copy of the pattern lowering.
    ///
    /// The cost is that the declaration of the temporary has to be written in
    /// FRONT of the construct, by which time those bindings are gone. So the
    /// type is recorded as each assignment is emitted — the one moment the
    /// bindings still exist — and read back afterwards.
    hoist_types: std::collections::HashMap<String, String>,
    /// Temporaries currently being lowered into, so `Stmt::Assign` knows which
    /// assignments to record a type for. Membership, not a name prefix: the
    /// prefix is reserved, but a set says what is true rather than what is
    /// merely conventional.
    open_hoists: std::collections::HashSet<String>,
    /// One frame per enclosing loop, innermost last: the temporary a
    /// value-carrying `break` assigns, or `None` for a loop written for its
    /// effect. Mirrors `BreakTarget` in the type checker — same rule ("a
    /// `break` binds to the innermost loop"), same shape.
    break_temps: Vec<Option<String>>,
    /// Serial number for hoisted temporaries. Function-scoped names would be
    /// enough for C, but a single counter over the whole translation unit costs
    /// nothing and makes every emitted name unique in the file, which is what a
    /// reader diffing generated C wants.
    hoist_counter: usize,
}

/// Where a payload sub-pattern applies: the C lvalue that holds it, and the C
/// type of that lvalue.
///
/// Returned by `payload_subjects` so the condition and the binding emission read
/// ONE description of the layout. They were written separately once, and only
/// one of the two knew that a payload could hold a sub-pattern at all.
struct PayloadSubject {
    subject: String,
    c_type: String,
}

impl CodeGenerator {
    pub fn new(module_name: &str) -> Result<Self> {
        // Pre-allocate string capacity for better performance
        let initial_capacity = 64 * 1024; // 64KB initial capacity
        Ok(Self {
            current_fn_unit_return: None,
            current_fn_name: String::new(),
            module_name: module_name.to_string(),
            output: String::with_capacity(initial_capacity),
            functions: std::collections::HashMap::new(),
            variables: std::collections::HashMap::new(),
            globals: std::collections::HashMap::new(),
            array_bindings: std::collections::HashMap::new(),
            mutable_params: std::collections::HashMap::new(),
            imported_modules: std::collections::HashMap::new(),
            generic_instantiations: Vec::new(),
            generic_struct_instantiations: Vec::new(),
            type_aliases: std::collections::HashMap::new(),
            enums: std::collections::HashMap::new(),
            structs: std::collections::HashMap::new(),
            impl_methods: std::collections::HashMap::new(),
            impl_method_params: std::collections::HashMap::new(),
            generic_struct_instantiation_map: std::collections::HashMap::new(),
            async_functions: std::collections::HashSet::new(),
            defined_structs: std::collections::HashSet::new(),
            recursive_layout: crate::typeck::RecursiveLayout::default(),
            tuple_shapes: indexmap_lite::OrderedMap::default(),
            enum_constructors: String::new(),
            pending_hoists: String::new(),
            hoist_counter: 0,
            hoist_types: std::collections::HashMap::new(),
            open_hoists: std::collections::HashSet::new(),
            break_temps: Vec::new(),
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
    /// Infer the C type of an expression, or `None` when codegen has no rule
    /// for that expression kind.
    fn try_infer_expr_type(&self, expr: &Expr) -> Option<String> {
        self.try_infer_expr_type_in(expr, &std::collections::HashMap::new())
    }

    /// The names an arm's PATTERN binds, at the C types the scrutinee gives
    /// them.
    ///
    /// The mirror of `emit_pattern_bindings`, for inference rather than
    /// emission, and it walks the same three sources of a position's type: the
    /// scrutinee itself for a whole-value binding, the tuple shape registry for
    /// an element (N4-12), and the enum's own payload types for a variant field.
    /// Where a position's type is not derivable the name is simply not recorded
    /// — inference then answers `None` for an expression that reads it, which is
    /// the pre-existing "cannot infer" refusal rather than a guess.
    fn bind_pattern_locals(
        &self,
        pattern: &Pattern,
        subject_type: &str,
        out: &mut std::collections::HashMap<String, String>,
    ) {
        match pattern {
            Pattern::Wildcard | Pattern::Literal(_) | Pattern::Range { .. } => {}
            Pattern::Ident(name) => {
                out.insert(name.clone(), subject_type.to_string());
            }
            Pattern::Binding { name, inner } => {
                out.insert(name.clone(), subject_type.to_string());
                self.bind_pattern_locals(inner, subject_type, out);
            }
            // An alternative may not bind (N6-07), so there is nothing here.
            Pattern::Or(_) => {}
            Pattern::Tuple(elements) => {
                let element_types: Vec<String> = self
                    .tuple_shapes
                    .element_types(subject_type)
                    .map(|types| types.to_vec())
                    .unwrap_or_default();
                for (i, element) in elements.iter().enumerate() {
                    if let Some(element_type) = element_types.get(i) {
                        self.bind_pattern_locals(element, element_type, out);
                    }
                }
            }
            Pattern::EnumPattern {
                enum_name,
                variant,
                data,
            } => {
                let Some(pattern_data) = data else {
                    return;
                };
                let Some(enum_def) = self.enums.get(enum_name) else {
                    return;
                };
                let Some(variant_def) = enum_def.variants.iter().find(|v| &v.name == variant)
                else {
                    return;
                };
                match (&variant_def.data, pattern_data) {
                    (EnumVariantData::Tuple(types), PatternData::Tuple(patterns)) => {
                        for (sub, ty) in patterns.iter().zip(types.iter()) {
                            self.bind_pattern_locals(sub, &self.type_to_c(ty), out);
                        }
                    }
                    (EnumVariantData::Struct(fields), PatternData::Struct(field_patterns)) => {
                        for (field_name, sub) in field_patterns {
                            if let Some((_, ty)) =
                                fields.iter().find(|(fname, _)| fname == field_name)
                            {
                                self.bind_pattern_locals(sub, &self.type_to_c(ty), out);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// The bindings a block's statements add, over the ones already visible.
    ///
    /// Only `let` is collected: it is the only statement that introduces a name
    /// a trailing expression can read.
    fn locals_of(
        &self,
        stmts: &[Stmt],
        outer: &std::collections::HashMap<String, String>,
    ) -> std::collections::HashMap<String, String> {
        let mut locals = outer.clone();
        for stmt in stmts {
            if let Stmt::Let {
                name, ty, value, ..
            } = stmt
            {
                let inferred = match ty {
                    Some(t) => Some(self.type_to_c(t)),
                    None => self.try_infer_expr_type_in(value, &locals),
                };
                if let Some(c_type) = inferred {
                    locals.insert(name.clone(), c_type);
                }
            }
        }
        locals
    }

    fn try_infer_expr_type_in(
        &self,
        expr: &Expr,
        locals: &std::collections::HashMap<String, String>,
    ) -> Option<String> {
        match expr {
            Expr::Integer(_) => Some("long long".to_string()),
            Expr::Float(_) => Some("double".to_string()),
            // N4-12. A tuple's C type is the struct emitted for its SHAPE, and
            // the shape is the elements' own C types — so this is the same
            // mangling `type_to_c` performs on a written `(A, B)`.
            Expr::Tuple { elements, .. } => {
                let mut element_types = Vec::with_capacity(elements.len());
                for element in elements {
                    element_types.push(self.try_infer_expr_type_in(element, locals)?);
                }
                Some(Self::tuple_c_name(&element_types))
            }
            // Read back from the registry, because a mangled name cannot be
            // un-mangled: `__pd_tuple2_long_long_const_char_p` has to be looked
            // up to know that `.1` is a `const char*` and not a `long long`.
            Expr::TupleIndex { expr, index, .. } => {
                let base = self.try_infer_expr_type_in(expr, locals)?;
                self.tuple_shapes.element_types(&base)?.get(*index).cloned()
            }
            // A char literal's TYPE is `char` (N4-04) and its CARRIER is
            // `long long`, which is what this must answer — not C's `char`.
            // Inferring `char` here would narrow `let c = 'a';` to one byte,
            // and one byte cannot hold `'한'` (U+D55C).
            Expr::Char(_) => Some("long long".to_string()),
            Expr::String(_) => Some("const char*".to_string()),
            Expr::Bool(_) => Some("int".to_string()),
            Expr::StructLiteral { name, fields, .. } => {
                // Check if this is a generic struct instantiation
                if let Some(instantiations) = self.generic_struct_instantiation_map.get(name) {
                    // Need to determine which instantiation to use based on field types
                    // For now, we'll infer from the first field's type
                    if let Some((_, field_expr)) = fields.first() {
                        let field_type = match field_expr {
                            Expr::Integer(_) | Expr::Char(_) => "long long",
                            Expr::Float(_) => "double",
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
            Expr::Ident(name) => locals
                .get(name)
                .or_else(|| self.variables.get(name))
                .cloned(),
            Expr::Call { func, args, .. } => {
                // A METHOD CALL IS A CALL (N5-17), and it has to be typed here
                // as well as emitted, because a method call can itself be a
                // receiver: `r.taller().area()` asks this what `r.taller()` is.
                if let Expr::FieldAccess { object, field, .. } = func.as_ref() {
                    let receiver_type = self.try_infer_expr_type_in(object, locals)?;
                    let owner = Self::struct_name_of(&receiver_type)?;
                    let qualified = format!("{}::{}", owner, field);
                    if let Some(ret) = self.impl_methods.get(&qualified) {
                        return self.return_type_to_c(ret.as_ref());
                    }
                    let (_, ret) = self.functions.get(&qualified)?;
                    return self.return_type_to_c(ret.as_ref());
                }

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
                        return self.try_infer_expr_type_in(arg, locals);
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
                            // N4-04: distinct type, same carrier.
                            crate::builtins::BuiltinType::Char => "long long",
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
                    // Bitwise and shift results are the LEFT operand's type
                    // (integers, by the type checker's rule), not a fixed
                    // `long long`: `let m: i32 = a & b;` must not widen.
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                        self.try_infer_expr_type_in(left, locals)
                    }
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                        // The operands decide, because the answer is not always
                        // `long long`: `let d = x / y;` over two `double`s used
                        // to declare `long long d = (x / y);` and TRUNCATE — no
                        // diagnostic, a wrong number, and the type checker
                        // knowing the right answer the whole time.
                        //
                        // The type checker has already refused every mixed pair
                        // (`Int + Float` is a TypeMismatch), so agreement
                        // between the operands is guaranteed by the time this
                        // runs and "either operand is a float" is the same
                        // question as "both are".
                        let left_type = self.infer_expr_type(left);
                        let right_type = self.infer_expr_type(right);
                        if left_type == "const char*" && right_type == "const char*" {
                            Some("const char*".to_string())
                        } else if left_type == "double" || right_type == "double" {
                            Some("double".to_string())
                        } else if left_type == "float" || right_type == "float" {
                            Some("float".to_string())
                        } else {
                            Some("long long".to_string())
                        }
                    }
                    _ => Some("long long".to_string()),
                }
            }
            Expr::Unary { op, operand, .. } => match op {
                UnaryOp::Not => Some("int".to_string()),
                // `~` is an INTEGER operator: its result is the operand's
                // type, not the `int` a truth value would be.
                UnaryOp::BitNot => self.try_infer_expr_type_in(operand, locals),
                UnaryOp::Neg => self.try_infer_expr_type_in(operand, locals),
            },
            Expr::EnumConstructor {
                enum_name, variant, ..
            } => {
                // A path CALL wears this node too (N5-17), and its type is the
                // function's return type, not the "enum" it is not. Same rule
                // as everywhere else: it is a constructor only if the name is
                // an enum's.
                if !self.enums.contains_key(enum_name) {
                    let qualified = format!("{}::{}", enum_name, variant);
                    if let Some(ret) = self.impl_methods.get(&qualified) {
                        return self.return_type_to_c(ret.as_ref());
                    }
                    if let Some((_, ret)) = self.functions.get(&qualified) {
                        return self.return_type_to_c(ret.as_ref());
                    }
                }
                // generate_enum emits `typedef struct <Enum> { ... } <Enum>;`,
                // so `struct <Enum>` names the same type type_to_c() produces
                // for an explicit annotation.
                Some(format!("struct {}", enum_name))
            }
            Expr::Reference { expr, .. } => {
                let inner = self.try_infer_expr_type_in(expr, locals)?;
                // A reference to an array needs C's pointer-to-array declarator
                // (`T (*p)[n]`), which the `let` printer cannot spell; refuse
                // instead of emitting a wrong one.
                if inner.contains('[') {
                    return None;
                }
                Some(format!("{}*", inner))
            }
            Expr::Deref { expr, .. } => {
                let inner = self.try_infer_expr_type_in(expr, locals)?;
                // Strip one pointer level. A non-pointer operand means the
                // operand is a reference *parameter*, which Expr::Ident already
                // auto-dereferences, so the value type is the operand type.
                Some(match inner.strip_suffix('*') {
                    Some(pointee) => pointee.trim_end().to_string(),
                    None => inner,
                })
            }
            Expr::FieldAccess { object, field, .. } => {
                let object_type = self.try_infer_expr_type_in(object, locals)?;
                let struct_name = Self::struct_name_of(&object_type)?;
                let fields = self.structs.get(struct_name)?;
                let (_, field_type) = fields.iter().find(|(name, _)| name == field)?;
                Some(self.type_to_c(field_type))
            }
            Expr::Index { array, .. } => {
                let array_type = self.try_infer_expr_type_in(array, locals)?;
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
                    Some(first) => self.try_infer_expr_type_in(first, locals)?,
                    None => "long long".to_string(),
                };
                Some(Self::array_of(&elem_type, elements.len()))
            }
            Expr::ArrayRepeat { value, count, .. } => {
                let elem_type = self.try_infer_expr_type_in(value, locals)?;
                let size = match count.as_ref() {
                    Expr::Integer(n) => *n as usize,
                    // Non-literal counts are rejected by the type checker.
                    _ => 0,
                };
                Some(Self::array_of(&elem_type, size))
            }
            // The value of an `if`/block expression is its tail, so the tail's
            // type is the temporary's type. Asked of the `then` side first and
            // the `else` side only as a fallback: the type checker has already
            // proved the two agree, so this is about which side this pass
            // happens to have a rule for, not about which one is right.
            //
            // A block's tail may name a local the block itself binds, and those
            // bindings do not exist yet when this is called from outside.
            // `generate_hoisted_block` re-asks the question with the block's
            // scope open; this arm is the answer for the cases that do not need
            // it (a literal, a call, an outer variable).
            Expr::If {
                then_branch,
                then_value,
                else_branch,
                else_value,
                ..
            } => {
                let then_locals = self.locals_of(then_branch, locals);
                then_value
                    .as_ref()
                    .and_then(|v| self.try_infer_expr_type_in(v, &then_locals))
                    .or_else(|| {
                        let else_locals =
                            self.locals_of(else_branch.as_deref().unwrap_or(&[]), locals);
                        else_value
                            .as_ref()
                            .and_then(|v| self.try_infer_expr_type_in(v, &else_locals))
                    })
            }
            // THE TAIL IS TYPED INSIDE THE BLOCK'S OWN BINDINGS.
            // `let x = { let a = 1; a };` was refused as "cannot infer the type
            // of `x`" — a well-typed program the type checker had already
            // accepted — because this asked what `a` was from outside the block
            // that binds it. Nesting made it worse, not better: a block inside
            // an `if` branch inside a block failed the same way at every level.
            Expr::Block { stmts, value, .. } => {
                let inner = self.locals_of(stmts, locals);
                value
                    .as_ref()
                    .and_then(|v| self.try_infer_expr_type_in(v, &inner))
            }
            // A cast's type is the type it names — that is the whole content of
            // the expression.
            Expr::Cast { ty, .. } => Some(self.type_to_c(ty)),
            // Both of these carry their value in places whose bindings are not
            // in scope out here — a `match` arm's payload binding, a `break`
            // inside a loop body. The answer that counts is recorded while the
            // construct is generated (`hoist_types`); this arm only answers the
            // easy cases, and `None` is not a refusal, it is "ask again later".
            // AN ARM'S PATTERN BINDINGS ARE LOCALS OF THAT ARM, and this used
            // to forget them: `let t = match n { other => other };` reported
            // "cannot infer the type of `t`" for a program whose type is written
            // on the scrutinee. `locals_of` collects `let`s and nothing else, so
            // an arm value that IS its binding had no entry to find.
            //
            // THE PATTERN IS BOUND BEFORE THE BODY IS WALKED, and the order is
            // the whole content of this fix. `locals_of` types each `let` from
            // its initialiser, so a body local whose initialiser READS a pattern
            // binding — `P::Num(n) => { let x = n; (x, 1) }` — can only be typed
            // in an environment that already has `n`. Collecting the body first
            // and adding the pattern afterwards left `x` untyped in an otherwise
            // well-typed program.
            Expr::Match { expr, arms, .. } => {
                let scrutinee = self.try_infer_expr_type_in(expr, locals);
                arms.iter().find_map(|arm| {
                    let mut env = locals.clone();
                    if let Some(scrutinee) = scrutinee.as_deref() {
                        self.bind_pattern_locals(&arm.pattern, scrutinee, &mut env);
                    }
                    let arm_locals = self.locals_of(&arm.body, &env);
                    arm.value
                        .as_ref()
                        .and_then(|v| self.try_infer_expr_type_in(v, &arm_locals))
                })
            }
            Expr::Loop { body, .. } => Self::first_break_value(body)
                .and_then(|expr| self.try_infer_expr_type_in(expr, locals)),
            // No rule yet: ranges are only meaningful inside `for`, `?` and
            // macros are lowered elsewhere, and await/async is unimplemented.
            Expr::Range { .. } => Some("__pd_range".to_string()),
            Expr::Question { .. } | Expr::MacroInvocation { .. } | Expr::Await { .. } => None,
        }
    }

    /// Record the return type of every method in an `impl` block under the name
    /// its call sites use (`Type::method`), resolving `Self` to the impl type.
    ///
    /// Takes the map rather than `&mut self` so it can be called while another
    /// field of the generator (the imported-module list) is borrowed.
    fn collect_impl_method_types(
        impl_methods: &mut std::collections::HashMap<String, Option<Type>>,
        impl_method_params: &mut std::collections::HashMap<String, Vec<Param>>,
        impl_block: &ImplBlock,
    ) {
        if !impl_block.type_params.is_empty() {
            // Generic impls are monomorphized elsewhere; their return types are
            // not knowable from the template.
            return;
        }
        let for_type = impl_block.for_type.to_string();
        // `Self` resolved by the ONE function that resolves it
        // (`ImplBlock::methods_with_self_resolved`). This used to substitute
        // the return type here and nowhere else, which is exactly why
        // `fn new(..) -> Self` worked while `fn area(self)` reached gcc as
        // `struct Self self`.
        for method in &impl_block.methods_with_self_resolved() {
            if !method.type_params.is_empty() {
                continue;
            }
            impl_methods.insert(
                format!("{}::{}", for_type, method.name),
                method.return_type.clone(),
            );
            // The PARAMS as declared, so the call site can ask the same question the
            // declaration answers: is this parameter a pointer?
            impl_method_params.insert(
                format!("{}::{}", for_type, method.name),
                method.params.clone(),
            );
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

    /// The shape of a (possibly nested) array type: the element type at the
    /// bottom, and every dimension in C DECLARATOR order — outermost first.
    /// `[[i64; 2]; 3]` -> (`i64`, `[3, 2]`).
    ///
    /// THE ONE DERIVATION for every position that declares an array — locals,
    /// parameters, struct fields — because C puts the brackets after the
    /// identifier and a type STRING cannot be spliced there. `type_to_c` built
    /// `long long[2]` for the element and each declaration site then wrote it
    /// in front of the name, producing `long long[2] grid[3]`: C the author
    /// never wrote, refused by gcc with "brackets are not allowed here; to
    /// declare an array, place the brackets after the identifier". The AST
    /// nests outside-in and C reads left-to-right from the identifier, so
    /// walking down and appending is the order C wants, unreversed.
    fn array_shape(ty: &Type) -> (&Type, Vec<&ArraySize>) {
        let mut dims = Vec::new();
        let mut current = ty;
        while let Type::Array(elem_type, size) = current {
            dims.push(size);
            current = elem_type.as_ref();
        }
        (current, dims)
    }

    /// One declared dimension as a C declaration prints it.
    ///
    /// A const generic prints as its own name, which is not in scope in the
    /// generated C — kept because that is what this pass has always emitted
    /// here, and it reaches gcc as "use of undeclared identifier" rather than
    /// as a wrong number. Positions that cannot afford that (a parameter, an
    /// inner dimension) ask `array_len_of_size` instead and refuse.
    fn c_array_size(size: &ArraySize) -> String {
        match size {
            ArraySize::Literal(n) => n.to_string(),
            ArraySize::ConstParam(name) => name.clone(),
            ArraySize::Expr(_) => "0".to_string(), // TODO: evaluate expression
        }
    }

    /// The bracket suffix of a declarator whose dimensions must ALL be
    /// numbers: `[[i64; 2]; 3]` -> `"[3][2]"`.
    ///
    /// Every dimension after the first is part of the element type in C — it
    /// decides the stride of `g[i]` — so a length this pass cannot resolve has
    /// no honest spelling there. `[0]` is a wrong stride, and the const
    /// generic's own name is not declared in the generated C. The `what` is
    /// how the refusal names the thing being declared, e.g. "the local `grid`".
    fn inner_dims_for_declarator(inner: &[&ArraySize], what: &str) -> Result<String> {
        let mut suffix = String::new();
        for size in inner {
            match Self::array_len_of_size(size) {
                ArrayLen::Proven(n) => suffix.push_str(&format!("[{}]", n)),
                ArrayLen::Unproven(spelling) => {
                    return Err(CompileError::CodegenError {
                        message: format!(
                            "cannot declare {}: the inner array length is written as `{}`, \
                             which this compiler does not resolve (const generic array \
                             lengths are dropped - see \
                             docs/specification/language-spec.md §5). Only the OUTERMOST \
                             length of a nested array may be left open, because every \
                             inner one is what makes a row a row. Give the inner array a \
                             literal length, e.g. `[[i64; 4]; 3]`.",
                            what, spelling
                        ),
                    })
                }
            }
        }
        Ok(suffix)
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

    /// Whether evaluating `expr` can be OBSERVED — N13-03's predicate.
    ///
    /// Written as a whitelist of the forms that provably cannot carry an
    /// effect, so a new `Expr` variant defaults to "effectful". The failure
    /// modes are not symmetric: calling a pure expression effectful costs one
    /// redundant temporary, and calling an effectful one pure loses the
    /// ordering guarantee silently.
    ///
    /// A CALL is the only leaf that makes this false. Nothing else in this
    /// language writes: there are no assignment expressions, and the only
    /// mutable state a callee can reach past its own frame is a `static mut`
    /// (measured: `static mut G` written by one argument and read by the next,
    /// see `test_argument_reads_are_sequenced_left_to_right`) or storage
    /// reached through a `&mut` parameter.
    fn expr_is_pure(expr: &Expr) -> bool {
        match expr {
            Expr::String(_)
            | Expr::Integer(_)
            | Expr::Float(_)
            | Expr::Char(_)
            | Expr::Bool(_)
            | Expr::Ident(_) => true,
            Expr::Index { array, index, .. } => {
                Self::expr_is_pure(array) && Self::expr_is_pure(index)
            }
            Expr::FieldAccess { object, .. } => Self::expr_is_pure(object),
            Expr::Unary { operand, .. } => Self::expr_is_pure(operand),
            Expr::Binary { left, right, .. } => {
                Self::expr_is_pure(left) && Self::expr_is_pure(right)
            }
            Expr::Reference { expr, .. }
            | Expr::Deref { expr, .. }
            | Expr::TupleIndex { expr, .. } => Self::expr_is_pure(expr),
            Expr::Cast { expr, .. } => Self::expr_is_pure(expr),
            Expr::ArrayLiteral { elements, .. } | Expr::Tuple { elements, .. } => {
                elements.iter().all(Self::expr_is_pure)
            }
            Expr::ArrayRepeat { value, .. } => Self::expr_is_pure(value),
            Expr::StructLiteral { fields, .. } => {
                fields.iter().all(|(_, value)| Self::expr_is_pure(value))
            }
            Expr::Range { start, end, .. } => {
                Self::expr_is_pure(start) && Self::expr_is_pure(end)
            }
            // Call, MacroInvocation, Await, Question, If, Loop, Match, Block,
            // EnumConstructor. Every one of these either IS a call or can
            // contain one, and the conservative answer costs only a temporary.
            _ => false,
        }
    }

    /// The declarator for a pointer named `name` to `base` with `dims`.
    ///
    /// `("long long", "")` -> `long long *t`; `("long long", "[2]")` ->
    /// `long long (*t)[2]`. The parentheses are not optional: `long long *t[2]`
    /// is an array of pointers, which is a different type and a different size.
    fn pointer_declarator(base: &str, dims: &str, name: &str) -> String {
        if dims.is_empty() {
            format!("{} *{}", base, name)
        } else {
            format!("{} (*{}){}", base, name, dims)
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
    /// drops it (`src/typeck/mod.rs:4928`, `mutable: _`) and the borrow checker
    /// gives every parameter a plain owned place
    /// (`src/ownership/borrow_checker.rs:641-643`). So `fn f(xs: &[i64; 3])` could
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
            Expr::Tuple { .. } => "tuple",
            Expr::TupleIndex { .. } => "tuple element access",
            Expr::Float(_) => "float literal",
            Expr::Char(_) => "char literal",
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
            Expr::If { .. } => "`if` expression",
            Expr::Block { .. } => "block expression",
            Expr::Cast { .. } => "`as` cast",
            Expr::Loop { .. } => "`loop` expression",
            Expr::Match { .. } => "`match` expression",
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

        // Ask the type checker's analysis which payload slots are pointers,
        // over the same item set it will be emitting: the escaped main program
        // plus every imported module's items, which land in this translation
        // unit too.
        //
        // The SAME set the type checker analysed, built by the same constructor.
        // Two passes deriving one item set independently is how they came to
        // disagree; `LayoutItems::of` owns the visibility filter, the shadowing
        // filter and the module sort, so neither pass can supply a set the other
        // would not have.
        //
        // The sort is inside it because `RandomState` reseeds per process and
        // this analysis feeds `definition_order`, which decides the ORDER TYPE
        // DEFINITIONS ARE EMITTED IN — an unsorted walk put the hash seed into
        // the emitted C whenever two imported modules declare one type name.
        // `make selfhost`'s fixed point cannot see it, because `bootstrap/pdc.pd`
        // imports nothing.
        self.recursive_layout = crate::typeck::RecursiveLayout::analyze(
            &crate::typeck::LayoutItems::of(program, &self.imported_modules),
        );

        self.output.push_str("#include <stdio.h>\n");
        self.output.push_str("#include <string.h>\n");
        self.output.push_str("#include <stdlib.h>\n");
        self.output.push_str("#include <ctype.h>\n");
        self.output.push_str("#include <stdint.h>\n\n");

        // Ranges as VALUES (N5-14). `a..b` used to be refused outside a `for`
        // header because there was nothing for it to BE; this is that thing.
        //
        // The end is kept as written, with a flag, rather than normalised to an
        // exclusive bound: `a..=b` would become `a..b+1`, and `b + 1` is not
        // always a number — `0..=<i64 max>` would wrap to an empty range with
        // no diagnostic.
        //
        // Constructed through a function rather than a compound literal
        // (`(__pd_range){a, b, 0}`), which is C99; the rest of this prelude is
        // C89 and the backend is whatever `cc` the host has.
        self.output.push_str("// Range values (N5-14)\n");
        self.output.push_str("typedef struct {\n");
        self.output.push_str("    long long start;\n");
        self.output.push_str("    long long end;\n");
        self.output.push_str("    int inclusive;\n");
        self.output.push_str("} __pd_range;\n\n");
        self.output.push_str(
            "static __pd_range __pd_range_new(long long start, long long end, int inclusive) {\n",
        );
        self.output.push_str("    __pd_range r;\n");
        self.output.push_str("    r.start = start;\n");
        self.output.push_str("    r.end = end;\n");
        self.output.push_str("    r.inclusive = inclusive;\n");
        self.output.push_str("    return r;\n");
        self.output.push_str("}\n\n");

        // N6-11. THE TRAP. A `match` that takes no arm must stop the program
        // rather than continue: with N6-10 enforced this is unreachable for a
        // well-typed program, which is exactly why it belongs here — it is the
        // defence for the gap between the checker's approximation and what a
        // running program can actually hold (a corrupted tag, a future checker
        // bug), and a defence that only exists where the checker is already
        // right is no defence.
        //
        // The message goes to stderr and the site calls `abort()` ITSELF rather
        // than leaving it to this helper. Two reasons, both measured: gcc's
        // `-Wreturn-type` analysis is not interprocedural, so a call to a static
        // helper that happens to end in `abort()` does not make the end of the
        // caller unreachable — and `-Werror=return-type` is what this trap earns
        // the right to turn on.
        self.output
            .push_str("// The match trap (N6-11)\n");
        self.output
            .push_str("static void __pd_match_trap(const char* where) {\n");
        self.output.push_str(
            "    fprintf(stderr, \"palladium: no match arm was taken in %s\\n\", where);\n",
        );
        self.output.push_str("}\n\n");

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
        // that table (src/ownership/borrow_checker.rs:127). Four of them —
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

        // STORAGE FOR THE PAYLOAD SLOTS THAT BECAME POINTERS.
        //
        // EMITTED ONLY WHEN A SLOT ACTUALLY BECAME ONE. A program with no
        // recursive type must emit the C it emitted before this analysis
        // existed, byte for byte, or the differential over the corpus stops
        // being a measurement of this change.
        //
        // WHAT "FREED AT EXIT" COSTS, said once so it is not only an argument for
        // safety. The arena grows with the number of constructions a run performs,
        // not with the number of nodes alive at any moment, so a loop that builds
        // and discards recursive values retains every one of them until the process
        // ends; and where the string pool merely stops recording past its cap and
        // leaks, this one calls `exit(1)` when `malloc` or `realloc` fails. That is
        // the right failure for a compiler that cannot free, and it is a liveness
        // ceiling rather than a leak that degrades.
        //
        // Freed once at exit rather than per value, which is not a shortcut but
        // the memory model this language already has: there is no drop glue and
        // no per-value free anywhere in it, which is the stated reason `String`
        // is a Copy type (src/ownership/borrow_checker.rs, `is_copy_type`).
        // Under that model a `match` binding may copy a node whose children are
        // shared with the value it came from, because nothing can free a child
        // while the program is running. Introducing per-value frees HERE, for
        // one type constructor, would break that invariant everywhere else.
        //
        // The cap grows instead of being fixed. `MAX_STRINGS` above silently
        // stops recording past 1024 and leaks the rest, which is survivable for
        // strings and is not for a tree, where the count is the program's data
        // size rather than its literal count.
        if self.recursive_layout.cuts_anything() {
            self.output
                .push_str("// Heap cells for recursive enum payload slots\n");
            self.output.push_str("static void** __pd_rec_nodes = 0;\n");
            self.output.push_str("static size_t __pd_rec_count = 0;\n");
            self.output.push_str("static size_t __pd_rec_cap = 0;\n");
            self.output
                .push_str("static void* __pd_rec_alloc(size_t size) {\n");
            self.output.push_str("    void* cell = malloc(size);\n");
            self.output.push_str("    if (!cell) {\n");
            self.output.push_str(
                "        fprintf(stderr, \"palladium: out of memory building a recursive value\\n\");\n",
            );
            self.output.push_str("        exit(1);\n");
            self.output.push_str("    }\n");
            self.output
                .push_str("    if (__pd_rec_count == __pd_rec_cap) {\n");
            self.output
                .push_str("        size_t grown = __pd_rec_cap ? __pd_rec_cap * 2 : 64;\n");
            self.output.push_str(
                "        void** moved = (void**)realloc(__pd_rec_nodes, grown * sizeof(void*));\n",
            );
            self.output.push_str("        if (!moved) {\n");
            self.output.push_str(
                "            fprintf(stderr, \"palladium: out of memory building a recursive value\\n\");\n",
            );
            self.output.push_str("            exit(1);\n");
            self.output.push_str("        }\n");
            self.output.push_str("        __pd_rec_nodes = moved;\n");
            self.output.push_str("        __pd_rec_cap = grown;\n");
            self.output.push_str("    }\n");
            self.output
                .push_str("    __pd_rec_nodes[__pd_rec_count++] = cell;\n");
            self.output.push_str("    return cell;\n");
            self.output.push_str("}\n\n");
            self.output
                .push_str("static void __pd_cleanup_rec_nodes() {\n");
            self.output
                .push_str("    for (size_t i = 0; i < __pd_rec_count; i++) {\n");
            self.output.push_str("        free(__pd_rec_nodes[i]);\n");
            self.output.push_str("    }\n");
            self.output.push_str("    free(__pd_rec_nodes);\n");
            self.output.push_str("    __pd_rec_nodes = 0;\n");
            self.output.push_str("    __pd_rec_count = 0;\n");
            self.output.push_str("    __pd_rec_cap = 0;\n");
            self.output.push_str("}\n\n");
        }

        // Register cleanup with atexit
        self.output
            .push_str("static void __pd_init() __attribute__((constructor));\n");
        self.output.push_str("static void __pd_init() {\n");
        self.output.push_str("    atexit(__pd_cleanup_strings);\n");
        if self.recursive_layout.cuts_anything() {
            self.output
                .push_str("    atexit(__pd_cleanup_rec_nodes);\n");
        }
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

        // Generate panic function wrapper.
        //
        // NOT marked noreturn — the ATTRIBUTE is what this could not have, and
        // the call site carries `abort()` instead. `panic(...)` in a branch that
        // owes
        // a value emits this call and nothing after it, which is correct C and
        // which `-Werror=return-type` rejects unless the compiler knows the call
        // does not come back. `scripts/check-c-returns.py` has known that since
        // it was written (its `NORETURN_RE`); gcc could not, because a plain
        // definition says nothing about control flow to a caller in the same
        // translation unit that the optimiser has not inlined yet. Measured on
        // this edit: without the attribute, `a_branch_that_panics_is_not_refused`
        // fails with "non-void function does not return a value in all control
        // paths" against C that is right.
        //
        // A FUNCTION, and `abort()` is added at the CALL SITE instead (see the
        // `panic` case in the builtin call emission). Two other shapes were
        // tried and rejected, both measured:
        //
        //  * `__attribute__((noreturn))` behind `#if defined(__GNUC__)` — made
        //    `scripts/check-c-returns.py` refuse the whole file, which is the
        //    right call for a reader of generated C ("a preprocessor directive
        //    that can decide which definitions exist") and not something to
        //    weaken for a hint;
        //  * a macro instead of a function — broke the seam gate in
        //    src/builtins.rs, which requires the emitted prelude to DEFINE a
        //    `__pd_<name>` for every registered built-in and reads that
        //    definition's signature.
        self.output.push_str("void __pd_panic(const char* msg) {\n");
        self.output
            .push_str("    fprintf(stderr, \"panic: %s\\n\", msg);\n");
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
        // N4-04. THE ONLY DOOR INTO `char` FROM A NUMBER, so it is where the
        // domain is enforced. A Unicode scalar is 0..=0x10FFFF minus the
        // UTF-16 surrogate range D800..=DFFF, and everything else that reached
        // `char` used to reach `string_from_char`, which writes `(char)c` — one
        // byte. Measured: `55296 as char` printed an empty line and
        // `99999999 as char` printed a garbage byte, both silently.
        self.output
            .push_str("long long __pd_char_from_scalar(long long v) {\n");
        self.output
            .push_str("    if (v < 0 || v > 1114111 || (v >= 55296 && v <= 57343)) {\n");
        self.output.push_str(
            "        fprintf(stderr, \"palladium: %lld is not a Unicode scalar, so it is not \
             a char\\n\", v);\n",
        );
        self.output.push_str("        abort();\n");
        self.output.push_str("    }\n");
        self.output.push_str("    return v;\n");
        self.output.push_str("}\n\n");

        self.output
            .push_str("long long __pd_string_char_at(const char* str, long long index) {\n");
        self.output
            // N14-04. THE `-1` SENTINEL DIED WITH THE TYPE. `string_char_at`
            // returns a `char` now, and -1 is not one — it was an integer
            // smuggled through an integer return, readable only by a caller who
            // knew to look. Nothing ever did: measured across the corpus,
            // `bootstrap/pdc.pd` guards its own bounds (`while i < len`,
            // `if i + 1 < len`) and no program compares a result against -1 or
            // against `< 0`.
            //
            // So the choice was between inventing a `char` to mean "no char"
            // and refusing. This is the answer N6-11 gives a match with no arm
            // to take, for the same reason and in the same shape: say what
            // happened, then `abort()` at the site.
            .push_str("    if (index < 0 || index >= (long long)strlen(str)) {\n");
        self.output.push_str(
            "        fprintf(stderr, \"palladium: string_char_at index %lld is outside a \
             string of length %lld\\n\", index, (long long)strlen(str));\n",
        );
        self.output.push_str("        abort();\n");
        self.output.push_str("    }\n");
        self.output
            .push_str("    return (long long)(unsigned char)str[index];\n");
        self.output.push_str("}\n\n");

        // string_substring
        self.output.push_str("const char* __pd_string_substring(const char* str, long long start, long long end) {\n");
        self.output.push_str("    size_t len = strlen(str);\n");
        self.output.push_str("    if (start < 0) start = 0;\n");
        self.output
            .push_str("    if (end > (long long)len) end = len;\n");
        self.output
            .push_str("    if (start >= end) return __pd_empty_owned();\n");
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
        self.output
            .push_str("extern int pd_path_exists(const char* path, size_t path_len);\n");
        self.output
            .push_str("extern int pd_path_is_file(const char* path, size_t path_len);\n");
        self.output
            .push_str("extern int pd_path_is_dir(const char* path, size_t path_len);\n");
        self.output
            .push_str("extern int pd_create_dir(const char* path, size_t path_len);\n");
        self.output
            .push_str("extern int pd_create_dir_all(const char* path, size_t path_len);\n");
        self.output
            .push_str("extern int pd_remove_file(const char* path, size_t path_len);\n");
        self.output
            .push_str("extern int pd_remove_dir(const char* path, size_t path_len);\n");
        self.output
            .push_str("extern int pd_remove_dir_all(const char* path, size_t path_len);\n");
        self.output.push_str("extern int pd_read_file_to_string(const char* path, size_t path_len, char** out_str, size_t* out_len);\n");
        self.output.push_str("extern int pd_write_string_to_file(const char* path, size_t path_len, const char* data, size_t data_len);\n\n");

        // Wrapper functions that call the external pd_* functions

        // file_seek, over the SAME `long long` handle table as file_write and
        // file_close. `whence` is the Palladium-level 0/1/2 that
        // src/runtime/io.rs::pd_file_seek also uses, mapped here to the C
        // constants rather than passed through: an unrecognised value is -1, not
        // an out-of-range seek. Returns the new absolute position, or -1.
        self.output.push_str(
            "long long __pd_file_seek(long long handle, long long whence, long long offset) {\n",
        );
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
        self.output
            .push_str("int __pd_path_exists(const char* path) {\n");
        self.output
            .push_str("    return pd_path_exists(path, strlen(path));\n");
        self.output.push_str("}\n\n");

        self.output
            .push_str("int __pd_path_is_file(const char* path) {\n");
        self.output
            .push_str("    return pd_path_is_file(path, strlen(path));\n");
        self.output.push_str("}\n\n");

        self.output
            .push_str("int __pd_path_is_dir(const char* path) {\n");
        self.output
            .push_str("    return pd_path_is_dir(path, strlen(path));\n");
        self.output.push_str("}\n\n");

        // Directory operations
        self.output
            .push_str("int __pd_create_dir(const char* path) {\n");
        self.output
            .push_str("    return pd_create_dir(path, strlen(path));\n");
        self.output.push_str("}\n\n");

        self.output
            .push_str("int __pd_create_dir_all(const char* path) {\n");
        self.output
            .push_str("    return pd_create_dir_all(path, strlen(path));\n");
        self.output.push_str("}\n\n");

        self.output
            .push_str("int __pd_remove_file(const char* path) {\n");
        self.output
            .push_str("    return pd_remove_file(path, strlen(path));\n");
        self.output.push_str("}\n\n");

        self.output
            .push_str("int __pd_remove_dir(const char* path) {\n");
        self.output
            .push_str("    return pd_remove_dir(path, strlen(path));\n");
        self.output.push_str("}\n\n");

        self.output
            .push_str("int __pd_remove_dir_all(const char* path) {\n");
        self.output
            .push_str("    return pd_remove_dir_all(path, strlen(path));\n");
        self.output.push_str("}\n\n");

        // Enhanced file operations with string helpers
        self.output
            .push_str("char* __pd_read_file_to_string(const char* path) {\n");
        self.output.push_str("    char* out_str = NULL;\n");
        self.output.push_str("    size_t out_len = 0;\n");
        self.output.push_str(
            "    if (pd_read_file_to_string(path, strlen(path), &out_str, &out_len) == 0) {\n",
        );
        self.output.push_str("        return out_str;\n");
        self.output.push_str("    }\n");
        // Failure returns the empty string, never NULL: a Palladium String is a
        // non-NULL const char* and every string built-in dereferences it at once
        // (string_len -> strlen). Returning NULL here made a missing file a
        // SIGSEGV rather than an error the program could see. This matches
        // __pd_arg_at, which returns "" out of range for the same reason.
        self.output.push_str("    return __pd_empty_owned();\n");
        self.output.push_str("}\n\n");

        self.output
            .push_str("int __pd_write_string_to_file(const char* path, const char* data) {\n");
        self.output.push_str(
            "    return pd_write_string_to_file(path, strlen(path), data, strlen(data));\n",
        );
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
                        Self::collect_impl_method_types(
                        &mut self.impl_methods,
                        &mut self.impl_method_params,
                        impl_block,
                    );
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
                    Self::collect_impl_method_types(
                        &mut self.impl_methods,
                        &mut self.impl_method_params,
                        impl_block,
                    );
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
        //
        // TWO FILTERS, AND BOTH ARE THE TYPE-NAMESPACE HALF OF A RULE THE
        // FUNCTION WALK FURTHER DOWN ALREADY APPLIES.
        //
        // `crate::ast::local_type_shadows_import` is the sibling of the
        // `local_definition_shadows_import` that walk calls, and it is called
        // here for the same reason: a local declaration replaces an imported
        // one, so emitting both puts two definitions of one C tag in the
        // translation unit. Measured, with `pub enum Color` in a module and
        // `struct Color { v: i64 }` in the program:
        // `main.c:280:16: error: redefinition of 'Color'`. The type checker's
        // half of this fix removed the `Type mismatch: expected Color, found
        // Color` in front of it and left this behind it, which is the whole
        // reason both passes ask the shared predicate rather than each pass
        // deciding.
        //
        // The `Public` test on the enum arm matches the struct arm above it. It
        // could not exist before 2026-08-23 — `EnumDef` had no visibility field
        // and the parser dropped the `pub` — so every enum in every imported
        // module was emitted into every program that imported it.
        let mut imported_defs: Vec<(String, Item)> = Vec::new();
        for (_, module_info) in &imported_modules {
            for item in &module_info.ast.items {
                match item {
                    Item::Struct(struct_def) => {
                        if matches!(struct_def.visibility, crate::ast::Visibility::Public)
                            && !crate::ast::local_type_shadows_import(program, &struct_def.name)
                        {
                            // Skip generic structs - they should only be generated when instantiated
                            if struct_def.type_params.is_empty()
                                && struct_def.lifetime_params.is_empty()
                            {
                                imported_defs.push((struct_def.name.clone(), item.clone()));
                            }
                        }
                    }
                    Item::Enum(enum_def) => {
                        if matches!(enum_def.visibility, crate::ast::Visibility::Public)
                            && !crate::ast::local_type_shadows_import(program, &enum_def.name)
                        {
                            // Skip generic enums - they should only be generated when instantiated
                            if enum_def.type_params.is_empty()
                                && enum_def.lifetime_params.is_empty()
                            {
                                imported_defs.push((enum_def.name.clone(), item.clone()));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        self.generate_type_definitions(&imported_defs)?;

        // Generate struct and enum definitions from main program
        //
        // The two phases stay separate, and in this order, because the
        // dependency direction between them is fixed: a module cannot name a
        // type declared in the program that imports it, so every cross-phase
        // edge points backwards into the imports, which are already emitted.
        let mut local_defs: Vec<(String, Item)> = Vec::new();
        for item in &program.items {
            match item {
                Item::Struct(struct_def) => {
                    // Skip generic structs - they should only be generated when instantiated
                    if struct_def.type_params.is_empty() && struct_def.lifetime_params.is_empty() {
                        local_defs.push((struct_def.name.clone(), item.clone()));
                    }
                }
                Item::Enum(enum_def) => {
                    // Skip generic enums - they should only be generated when instantiated
                    if enum_def.type_params.is_empty() && enum_def.lifetime_params.is_empty() {
                        local_defs.push((enum_def.name.clone(), item.clone()));
                    }
                }
                _ => {}
            }
        }
        self.generate_type_definitions(&local_defs)?;

        // Every tuple shape a written TYPE names is registered before the
        // marker is taken, so a function that only RECEIVES a tuple (and never
        // builds one) still has its struct.
        self.register_tuple_types_in(program)?;

        // N4-12. Tuple structs go HERE — after the struct and enum definitions,
        // because an element may be one of those. The shapes are not all known
        // yet (a tuple built inside a function body registers when that body is
        // generated), so the position is recorded and the definitions spliced in
        // at the end.
        let tuple_marker = self.output.len();

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

        // Variant constructors, after EVERY type definition rather than each one
        // straight after its own enum.
        //
        // They are the only emitted code that needs a payload type COMPLETE
        // rather than merely named: an indirect slot takes `sizeof(struct S)`,
        // and every constructor takes its argument by value. Emitted in place,
        // `enum E { Leaf(i64), Node(S) }` over `struct S { e: E }` — a mutual
        // recursion that terminates, so a program CAN build one — put
        // `E_Node__new(struct S arg0)` above the definition of `struct S`, and
        // gcc reported `variable has incomplete type` and `invalid application
        // of sizeof`. No ordering of the two definitions fixes it, because `S`
        // stores an `E` by value and so must come second.
        //
        // This moves the constructors of EVERY enum, not only recursive ones.
        // Deferring conditionally would make the shape of the output depend on
        // a predicate rather than on the language, and the emitted C is the
        // artefact this compiler is judged on: one shape is worth measuring.
        if !self.enum_constructors.is_empty() {
            let constructors = std::mem::take(&mut self.enum_constructors);
            self.output.push_str(&constructors);
        }

        // Top-level `const` and `static` items (N3-09, N3-10), BEFORE every
        // function and in source order among themselves. A body may read an
        // item written below it — order independence is the language's rule —
        // so no position inside the function loop would do.
        self.generate_global_items(program)?;

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
                    // Generate methods from impl blocks, with `Self` resolved to
                    // the impl type — otherwise the receiver is emitted as
                    // `struct Self`, which nothing declares.
                    for method in &impl_block.methods_with_self_resolved() {
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
                Item::Global(_) => {
                    // Emitted before the prototypes; see `generate_global_items`.
                }
            }
        }

        let tuple_defs = self.tuple_definitions();
        if !tuple_defs.is_empty() {
            self.output.insert_str(tuple_marker, &tuple_defs);
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
            Type::F64 => "double".to_string(),
            Type::F32 => "float".to_string(),
            // N4-04. `char` is a DISTINCT TYPE and the SAME CARRIER: a C `char`
            // holds 8 bits and a scalar like U+D55C needs 21, so it rides in the
            // `long long` it always rode in. The split is the checker's, so
            // `c as i64` is a NO-OP IDENTITY CAST — it emits the tokens
            // `(long long)`, and converts nothing. (`n as char` is the one
            // direction that is not free: it emits a call to
            // `__pd_char_from_scalar`, which checks the operand is a Unicode
            // scalar.)
            Type::Char => "long long".to_string(),
            Type::Bool => "int".to_string(),
            Type::String => "const char*".to_string(),
            Type::Unit => "void".to_string(),
            // N4-10. The dimensions come out in DECLARATOR order — outermost
            // first — because that is the order every reader of this string
            // already assumes (`split_array_dims`, `array_of`, and the index
            // rule in `try_infer_expr_type_in`, which drops the FIRST bracket
            // to type `xs[i]`). Composing on the way back up the recursion,
            // `format!("{}[{}]", type_to_c(elem), size)`, printed
            // `[[i64; 2]; 3]` inside-out as `long long[2][3]`, so a reader that
            // dropped the first bracket called a row of length 2 a
            // `long long[3]`.
            Type::Array(_, _) => {
                let (base, dims) = Self::array_shape(ty);
                let mut c_type = self.type_to_c(base);
                for size in dims {
                    c_type.push_str(&format!("[{}]", Self::c_array_size(size)));
                }
                c_type
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
            // N4-12. A tuple is the struct emitted for its SHAPE. This read
            // `void*` with a TODO for as long as no tuple could be constructed,
            // which is why nothing ever noticed: a `void*` that no value can
            // have is indistinguishable from a correct answer.
            Type::Tuple(types) => {
                let element_types: Vec<String> =
                    types.iter().map(|ty| self.type_to_c(ty)).collect();
                Self::tuple_c_name(&element_types)
            }
        }
    }

    /// The C type of one enum payload slot.
    ///
    /// THE ONE DERIVATION, called by both places that declare a slot, so the
    /// tuple form and the named form cannot drift into declaring the same
    /// recursive type two different ways. The three places that USE a slot —
    /// the two constructor writers and the `match` reader — ask
    /// `RecursiveLayout::payload_is_indirect` with the same AST node, so all
    /// five are answering one question about one input.
    fn payload_slot_c_type(&self, enum_name: &str, ty: &Type) -> String {
        let base = self.type_to_c(ty);
        if self.recursive_layout.payload_is_indirect(enum_name, ty) {
            format!("{}*", base)
        } else {
            base
        }
    }

    /// Store one constructor argument into its payload slot.
    ///
    /// THE ONE WRITER, shared by the tuple form and the named form. An indirect
    /// slot takes a cell first and the value into the cell; a direct slot is the
    /// assignment it always was, character for character, so the C emitted for a
    /// program with no recursive type does not move.
    ///
    /// It takes the VARIANT, not an already-derived member name, and derives the
    /// member here. Handing it a string would have put
    /// `c_ident::c_enum_payload_member` at the call sites — which is what
    /// `tests/m1_c_keyword_idents.rs::every_payload_member_emission_uses_the_one_derivation`
    /// caught, and it was right to: a writer that accepts a member name accepts
    /// an underived one.
    fn emit_payload_store(
        &mut self,
        enum_name: &str,
        variant: &str,
        slot: &str,
        source: &str,
        ty: &Type,
    ) {
        if self.recursive_layout.payload_is_indirect(enum_name, ty) {
            let cell = self.type_to_c(ty);
            self.output.push_str(&format!(
                "    result.data.{}.{} = ({}*)__pd_rec_alloc(sizeof({}));
",
                c_ident::c_enum_payload_member(variant),
                slot,
                cell,
                cell
            ));
            self.output.push_str(&format!(
                "    *result.data.{}.{} = {};
",
                c_ident::c_enum_payload_member(variant),
                slot,
                source
            ));
        } else {
            self.output.push_str(&format!(
                "    result.data.{}.{} = {};
",
                c_ident::c_enum_payload_member(variant),
                slot,
                source
            ));
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

        // N4-12. A TUPLE IN A PAYLOAD IS REFUSED, and by name rather than by
        // gcc. Tuple structs are emitted after the enum definitions, because a
        // tuple's element may be an enum; a payload of tuple type needs the
        // reverse order, and satisfying both would take a real dependency sort
        // over generated types. Measured without this: `enum E { P((i64, i64)) }`
        // reached the C compiler as "unknown type name
        // '__pd_tuple2_long_long_long_long'", which is our own C failing on the
        // user's behalf — the failure mode this compiler refuses to ship.
        for variant in &enum_def.variants {
            let payload_types: Vec<&Type> = match &variant.data {
                EnumVariantData::Unit => Vec::new(),
                EnumVariantData::Tuple(types) => types.iter().collect(),
                EnumVariantData::Struct(fields) => fields.iter().map(|(_, ty)| ty).collect(),
            };
            for ty in payload_types {
                if matches!(ty, Type::Tuple(_)) {
                    return Err(CompileError::CodegenError {
                        message: format!(
                            "`{}::{}` carries a TUPLE in its payload, and code generation emits \
                             tuple structs after the enums that would use them, so this type \
                             would be referenced before it is defined. Give the variant its \
                             elements as separate fields (`{}(i64, i64)`), or declare a struct \
                             whose fields ARE those elements and use that — a struct with a \
                             TUPLE field is refused for the mirror reason, so wrapping the tuple \
                             is not a way out",
                            enum_def.name, variant.name, variant.name
                        ),
                    });
                }
            }
        }

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
                            let c_type = self.payload_slot_c_type(&enum_def.name, ty);
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
                        let c_type = self.payload_slot_c_type(&enum_def.name, field_type);
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

        // Generate constructor functions for each variant.
        //
        // Written through `self.output` and then cut off it, rather than into a
        // second sink: `emit_payload_store` and every line below push to
        // `output`, and giving them a destination argument would put the choice
        // of sink at each call site — one more thing that can be got wrong per
        // site. One cut, at the end, cannot be.
        let constructors_start = self.output.len();
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

                    for (i, ty) in types.iter().enumerate() {
                        self.emit_payload_store(
                            &enum_def.name,
                            &variant.name,
                            &format!("field{}", i),
                            &format!("arg{}", i),
                            ty,
                        );
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

                    for (field_name, field_type) in fields {
                        self.emit_payload_store(
                            &enum_def.name,
                            &variant.name,
                            field_name,
                            field_name,
                            field_type,
                        );
                    }

                    self.output.push_str(
                        "    return result;
",
                    );
                    self.output.push_str("}\n\n");
                }
            }
        }
        let constructors = self.output.split_off(constructors_start);
        self.enum_constructors.push_str(&constructors);

        Ok(())
    }

    /// Emit one phase's `struct` and `enum` definitions, DEPENDENCIES FIRST.
    ///
    /// Not source order. C gives a field a size from a complete type, so a
    /// `struct S { e: E }` written above `enum E { A, B }` emitted in the order
    /// it was written produces
    ///
    /// ```text
    /// error: field has incomplete type 'struct E'
    /// ```
    ///
    /// from gcc — and swapping the two declarations, which changes nothing about
    /// the program, makes it compile. A language does not get to have
    /// order-dependent type declarations, and the failure landing in gcc rather
    /// than in a diagnostic is the same shape the recursive-layout refusal exists
    /// to remove.
    ///
    /// The order comes from `RecursiveLayout::definition_order`, over the SAME
    /// cut containment graph the layout analysis built for its refusal. That is
    /// the point of asking it rather than walking the fields here: a payload slot
    /// that became a `struct V*` needs only the tag and constrains nothing, and a
    /// second opinion about which slots those are is a second thing to keep in
    /// step with the four emission sites that already share the first one.
    ///
    /// A cycle among these names is refused rather than emitted. It cannot come
    /// from a program the type checker accepted — `declarations_without_layout`
    /// is exactly that refusal — so reaching here means code generation was
    /// driven directly, and emitting C gcc will reject would be the one outcome
    /// worth avoiding.
    fn generate_type_definitions(&mut self, defs: &[(String, Item)]) -> Result<()> {
        let names: Vec<String> = defs.iter().map(|(name, _)| name.clone()).collect();
        let order = self
            .recursive_layout
            .definition_order(&names)
            .map_err(|cycle| {
                CompileError::Generic(format!(
                    "type definitions cannot be ordered: they store each other by value \
                     ({}), so no emission order gives every field a complete type. This \
                     should have been refused as a recursive type with no layout before \
                     code generation ran",
                    cycle.join(" -> ")
                ))
            })?;

        // By name, and every definition carrying that name, so two declarations
        // sharing one are both emitted (and gcc reports the redefinition) rather
        // than one being silently dropped by a lookup.
        for name in &order {
            for (def_name, item) in defs {
                if def_name != name {
                    continue;
                }
                match item {
                    Item::Struct(struct_def) => self.generate_struct(struct_def)?,
                    Item::Enum(enum_def) => self.generate_enum(enum_def)?,
                    _ => {}
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
                Type::F64 => "double",
                Type::F32 => "float",
                Type::Bool => "int",
                Type::Char => "long long",
                Type::String => "const char*",
                // A field is a declarator too (N4-10): the brackets go after the
                // field name, outermost first, and a nested array field has
                // more than one. Emitting the element's TYPE string put them in
                // front of the name — `long long[2] cells[2];` — which gcc
                // refuses inside a struct for the same reason it refuses it for
                // a local.
                Type::Array(_, _) => {
                    let (base, dims) = Self::array_shape(field_type);
                    let (outer, inner) = dims.split_first().expect("Array has a dimension");
                    let inner_suffix = Self::inner_dims_for_declarator(
                        inner,
                        &format!("the field `{}` of `{}`", field_name, struct_def.name),
                    )?;
                    self.output.push_str(&format!(
                        "{} {}[{}]{};\n",
                        self.type_to_c(base),
                        field_name,
                        Self::c_array_size(outer),
                        inner_suffix
                    ));
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

    /// Emit the C definition of every top-level `const` and `static` (N3-09,
    /// N3-10), and record their C types for the rest of code generation.
    ///
    /// THE STORAGE CLASS IS THE WHOLE DIFFERENCE:
    ///   `const X: i64 = 5;`      ->  `static const long long X = 5;`
    ///   `static Y: i64 = 10;`    ->  `static long long Y = 10;`
    ///   `static mut C: i64 = 0;` ->  `static long long C = 0;`
    ///
    /// `static` on all three is INTERNAL LINKAGE, not the item's own `static`
    /// keyword: the emitted C is one translation unit, and a file-scope name
    /// with external linkage can collide with a libc symbol the program never
    /// mentions (`index`, `time`, `link` are all plausible Palladium item
    /// names). The `const` qualifier is what refuses a write in C as well as in
    /// Palladium, so a code-generation bug that emitted an assignment to a
    /// `const` item would be a gcc error rather than a silent store.
    ///
    /// NOT `#define`. A macro is not scoped, is not typed, and would rewrite
    /// any later use of that spelling anywhere in the file — including inside
    /// an unrelated struct field name — so a `const` item would change the
    /// meaning of code that never read it.
    fn generate_global_items(&mut self, program: &Program) -> Result<()> {
        let globals: Vec<&crate::ast::GlobalDef> = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Global(global) => Some(global),
                _ => None,
            })
            .collect();
        if globals.is_empty() {
            return Ok(());
        }

        self.output
            .push_str("// Top-level const and static items\n");
        for global in globals {
            let c_type = self.type_to_c(&global.ty);
            // THE C `const` FOLLOWS THE LANGUAGE RULE, NOT THE KEYWORD. A
            // `static` without `mut` is read-only in Palladium — the type
            // checker refuses an assignment to it — so emitting it without the
            // qualifier left the rule enforced in exactly one place. With it,
            // gcc is a second enforcer of the same rule, and a code-generation
            // bug that emitted a store to a read-only item becomes a C error
            // rather than a silent write. `static mut` is the one form that
            // stays writable, so it is the one form emitted without `const`.
            let qualifier = match global.kind {
                crate::ast::GlobalKind::Const
                | crate::ast::GlobalKind::Static { is_mut: false } => "static const ",
                crate::ast::GlobalKind::Static { is_mut: true } => "static ",
            };
            let mut definition = String::new();
            definition.push_str(qualifier);
            definition.push_str(&c_type);
            definition.push(' ');
            definition.push_str(&global.name);
            definition.push_str(" = ");
            self.globals.insert(global.name.clone(), c_type);

            // The initialiser is a constant expression by the parser's rule, so
            // it is generated through the ordinary expression path with an empty
            // hoist channel: nothing here can hoist, and if that ever stopped
            // being true the statements would have nowhere to go — which is why
            // this asserts rather than assumes.
            let outer = std::mem::take(&mut self.output);
            let hoists_before = self.pending_hoists.len();
            self.generate_expression(&global.value)?;
            let value = std::mem::replace(&mut self.output, outer);
            if self.pending_hoists.len() != hoists_before {
                return Err(CompileError::Generic(format!(
                    "the initialiser of `{}` needs statements to run before it, \
                     and a top-level item has nowhere to run them",
                    global.name
                )));
            }
            definition.push_str(&value);
            definition.push_str(";\n");
            self.output.push_str(&definition);
        }
        self.output.push('\n');
        Ok(())
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
                    // Same substitution as the definitions above; a prototype
                    // that disagreed with its definition would be a conflicting
                    // C declaration.
                    for method in &impl_block.methods_with_self_resolved() {
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
    ///
    /// N4-10, THE NESTED CASE, AND WHY IT IS SPELLED THIS WAY. A parameter of
    /// type `[[i64; 2]; 3]` is written `long long g[3][2]`, which C reads as
    /// `long long (*g)[2]` — a pointer to a row of 2, exactly the decay of the
    /// caller's object. The alternative spellings were both worse: `long long**`
    /// is a different data layout (an array of pointers) and would read garbage,
    /// and writing the pointer form by hand buys nothing the array form does not
    /// already give, while losing the documented outer length that
    /// `for x in g` and the call site read. So the existing convention — keep
    /// the brackets, let C decay them — goes one level deeper unchanged, and
    /// `g[i][j]` in the body is the same subscript it is on a local.
    fn array_param_declarator(
        elem_type: &Type,
        size: &ArraySize,
        param_name: &str,
        is_const: bool,
    ) -> Result<String> {
        let (base_elem, inner_dims) = Self::array_shape(elem_type);
        let elem_c_type = match base_elem {
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
                    base_elem
                )))
            }
        };
        // The inner dimensions of a nested array parameter are part of the
        // element type C computes strides from, so they may not be left open
        // the way the outermost one below may.
        let inner_suffix = Self::inner_dims_for_declarator(
            &inner_dims,
            &format!("the parameter `{}`", param_name),
        )?;
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
        Ok(format!(
            "{} {}[{}]{}",
            elem_decl, param_name, size_str, inner_suffix
        ))
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
                        elem_type,
                        size,
                        &param.name,
                        false,
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
                            elem_type,
                            size,
                            &param.name,
                            !*mutable,
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
        // ...and the top-level items come straight back: they are in scope in
        // every body, which is the one thing this map's per-function lifetime
        // cannot express on its own.
        for (name, c_type) in &self.globals {
            self.variables.insert(name.clone(), c_type.clone());
        }
        self.array_bindings.clear();
        // BOTH spellings of the unit type, which is the whole point: `None` and
        // `Some(Type::Unit)` are one return type and must generate one shape.
        // And `main` is INSIDE the rule, not an exception to it — it just needs
        // a different replacement, because its C type is `int`.
        self.current_fn_name = name.to_string();
        self.current_fn_unit_return = if matches!(func.return_type, None | Some(Type::Unit)) {
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
                Type::Char => "long long".to_string(),
                Type::Bool => "int".to_string(),
                Type::Custom(name) => name.clone(),
                // Array parameters keep their dimensions, in the same encoding
                // `self.variables` uses for locals ("long long[4]"). Recording
                // the bare element type instead lost the length, which is what
                // forced `for x in arr` to fall back to `sizeof` on a pointer,
                // and left `let e = arr[i];` with no inferable type at all.
                Type::Array(_, _) => self.type_to_c(&param.ty),
                // N4-12. A tuple parameter's type is its shape's struct, and it
                // has to be recorded under that name: `p.0` is typed by looking
                // the name up in the shape registry, and the catch-all below
                // would have called every tuple a `long long`.
                Type::Tuple(_) => self.type_to_c(&param.ty),
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
        let result = self.generate_stmts_in_current_scope(stmts, indent);
        self.close_binding_scope(outer);
        result
    }

    /// The body of [`CodeGenerator::generate_block`] WITHOUT the scope.
    ///
    /// Split out for the value-block lowering, which has to ask
    /// `try_infer_expr_type` about the block's tail expression while the
    /// block's own bindings are still visible — `{ let a = 3; a * 2 }` types its
    /// value as `a`'s type, and `a` stops existing the moment the scope closes.
    fn generate_stmts_in_current_scope(&mut self, stmts: &[Stmt], indent: &str) -> Result<()> {
        for stmt in stmts {
            self.output.push_str(indent);
            self.generate_statement(stmt)?;
        }
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
    fn open_binding_scope(
        &self,
    ) -> (
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

    /// Generate code for a statement.
    ///
    /// Owns the SPLICE POINT for hoisted value expressions: an `if` or a block
    /// used as a value cannot be written inside a C expression, so
    /// `generate_expression` emits its statements into `self.pending_hoists`
    /// and leaves a temporary's name behind. Those statements have to appear
    /// *before* the statement that used the value, and the only place that
    /// knows where the statement began is here.
    ///
    /// `mark` is taken before the body runs and the buffer is inserted at that
    /// offset afterwards, rather than appended, because by then the statement's
    /// own text is already in `self.output`.
    ///
    /// Saving and restoring `pending_hoists` around the body is what makes
    /// nesting work: a value expression inside a branch is generated by a
    /// recursive `generate_statement`, which splices it into the branch and
    /// hands an empty buffer back to the outer level.
    fn generate_statement(&mut self, stmt: &Stmt) -> Result<()> {
        let mark = self.output.len();
        let outer_hoists = std::mem::take(&mut self.pending_hoists);
        let result = self.generate_statement_body(stmt);
        let mine = std::mem::replace(&mut self.pending_hoists, outer_hoists);
        result?;
        if !mine.is_empty() {
            // Splice in front of the statement's INDENTATION, not after it.
            // `generate_block` writes the indent before calling this, so
            // inserting at `mark` would put the first hoisted line behind that
            // indent and leave the statement itself flush against the margin.
            let (line_start, indent) = self.statement_indent(mark);
            let spliced = Self::reindent_to(&mine, &indent);
            self.output.insert_str(line_start, &spliced);
        }
        Ok(())
    }

    /// Where the statement's line begins, and the indentation already written
    /// on it — empty when anything other than spaces precedes `offset`.
    fn statement_indent(&self, offset: usize) -> (usize, String) {
        let line_start = self.output[..offset].rfind('\n').map_or(0, |i| i + 1);
        let prefix = &self.output[line_start..offset];
        if prefix.chars().all(|c| c == ' ') {
            (line_start, prefix.to_string())
        } else {
            (offset, String::new())
        }
    }

    /// Prefix every non-empty line of `text` with `indent`.
    ///
    /// Purely cosmetic, and worth the few lines anyway: generated C is read
    /// here (`build_output/*.c` is what a reviewer diffs when a lowering
    /// changes), and a hoisted `if` left at column 4 inside a block indented to
    /// 12 reads as if it escaped the block it is actually inside.
    fn reindent_to(text: &str, indent: &str) -> String {
        if indent.is_empty() {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len() + indent.len() * 4);
        for line in text.split_inclusive('\n') {
            if !line.trim_start().is_empty() {
                out.push_str(indent);
            }
            out.push_str(line);
        }
        out
    }

    fn generate_statement_body(&mut self, stmt: &Stmt) -> Result<()> {
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
                        // N4-10. Every dimension goes after the NAME, outermost
                        // first, and a nested one has more than one. The
                        // outermost keeps the length rule this position always
                        // had (an unresolved length declares `[0]`); the inner
                        // ones cannot, because an inner length is the stride of
                        // a row rather than a count of them.
                        Type::Array(_, _) => {
                            let (base, dims) = Self::array_shape(t);
                            let (outer, inner) = dims.split_first().expect("Array has a dimension");
                            let outer_val = match Self::array_len_of_size(outer) {
                                ArrayLen::Proven(n) => n,
                                ArrayLen::Unproven(_) => 0, // TODO: resolve const param
                            };
                            let suffix = Self::inner_dims_for_declarator(
                                inner,
                                &format!("the local `{}`", name),
                            )?;
                            (
                                self.type_to_c(base),
                                format!("[{}]{}", outer_val, suffix),
                            )
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
                        // A write to a hoisted temporary is where its C type is
                        // learned: this is a synthesised assignment standing in
                        // for a `match` arm's value, and the arm's pattern
                        // bindings are in scope HERE and nowhere the
                        // declaration can be written. See `hoist_types`.
                        if self.open_hoists.contains(name) && !self.hoist_types.contains_key(name) {
                            if let Some(c_type) = self.try_infer_expr_type(value) {
                                self.hoist_types.insert(name.clone(), c_type);
                            }
                        }
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
                match else_branch {
                    None => self.output.push('\n'),
                    Some(else_stmts) => {
                        // `else if` (N5-06) is emitted as C's `else if`, not as
                        // a nested block. The parser represents the chain as
                        // nesting, and emitting that nesting literally would
                        // indent a five-arm chain five levels deep and make the
                        // generated C read as something the programmer did not
                        // write.
                        //
                        // WITH ONE EXCEPTION, AND IT IS A CORRECTNESS ONE: a
                        // condition containing a value `if`/block hoists
                        // statements in front of itself, and in front of an
                        // `else if` means in front of the WHOLE chain — i.e.
                        // evaluated unconditionally, on a path the programmer
                        // wrote as unreachable. Whether that happens is not
                        // decidable from the condition's shape here, so it is
                        // MEASURED: generate the nested `if`, and fall back to
                        // the block form (where the hoists stay inside the
                        // `else`) if anything came out.
                        let chained = match else_stmts.as_slice() {
                            [nested @ Stmt::If { .. }] => {
                                let saved = std::mem::take(&mut self.pending_hoists);
                                let text =
                                    self.capture_output(|g| g.generate_statement_body(nested))?;
                                let hoisted = std::mem::replace(&mut self.pending_hoists, saved);
                                if hoisted.is_empty() {
                                    Some(text)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };

                        match chained {
                            Some(text) => {
                                self.output.push_str(" else ");
                                self.output.push_str(text.trim_start_matches(' '));
                            }
                            None => {
                                self.output.push_str(" else {\n");
                                self.generate_block(else_stmts, "")?;
                                self.output.push_str("    }\n");
                            }
                        }
                    }
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                // A CONDITION IS RE-EVALUATED EVERY ITERATION, so anything it
                // hoists must be too.
                //
                // MEASURED WRONG CODE, not a hypothetical: `while { i < 3 }`
                // emitted `__pd_val0 = (i < 3); while (__pd_val0)` — the test
                // computed ONCE, before the loop, and the program never
                // terminated. `generate_statement` splices `pending_hoists` in
                // front of the whole statement, which is right for a position
                // that always runs and wrong for one that runs each time round.
                //
                // The repair is a lowering and not a refusal: `while (1) { …the
                // hoisted statements…; if (!(test)) break; …body… }` runs them
                // once per iteration, and `continue` in the body still lands
                // above them.
                let (cond_src, cond_hoists) = self.generate_expr_with_hoists(condition)?;

                if cond_hoists.is_empty() {
                    self.output
                        .push_str(&format!("    while ({}) {{\n", cond_src));
                } else {
                    self.output.push_str("    while (1) {\n");
                    self.output.push_str(&cond_hoists);
                    self.output
                        .push_str(&format!("        if (!({})) break;\n", cond_src));
                }

                // Generate body. The frame is `None`: a `break` written in here
                // belongs to THIS loop, which produces no value, so it must not
                // reach an enclosing value loop's temporary.
                self.break_temps.push(None);
                let result = self.generate_block(body, "");
                self.break_temps.pop();
                result?;

                self.output.push_str("    }\n");
            }
            Stmt::For {
                var, iter, body, ..
            } => {
                self.output.push_str("    {\n"); // Create a new scope

                // Check if iterating over a range
                match iter {
                    Expr::Range {
                        start,
                        end,
                        inclusive,
                        ..
                    } => {
                        // A range written IN the header keeps its fast path:
                        // the bounds go straight into the `for`, with no
                        // `__pd_range` value built and thrown away. `..=` is
                        // the same loop with `<=`.
                        // TWO DEFECTS ARE FIXED HERE, and the old two-line
                        // emission had both.
                        //
                        // THE ENDPOINT WAS RE-EVALUATED EVERY ITERATION
                        // (PRE-EXISTING, older than ranges-as-values):
                        // `for i in 0..f()` put the call in the `for` test, so
                        // `f()` ran once per iteration — measured, four times
                        // for a four-element range. Both ends are now read into
                        // temporaries once, before the loop.
                        //
                        // `..=` INCREMENTED PAST ITS LAST VALUE: `v <= end;
                        // v++` overflows a signed `long long` when `end` is the
                        // maximum, which is undefined behaviour and, in
                        // practice, a loop that never ends. The inclusive form
                        // counts with an UNSIGNED index instead and never
                        // computes `last + 1`. `continue` still works, because
                        // the increment is in the `for` header where C runs it.
                        let n = self.hoist_counter;
                        self.hoist_counter += 1;
                        let (lo, hi) = (format!("__pd_lo{}", n), format!("__pd_hi{}", n));
                        // THE TWO BOUNDS ARE READ IN SOURCE ORDER, and that
                        // needs saying because the obvious emission gets it
                        // backwards. Anything the END hoists is spliced in front
                        // of the WHOLE statement, which is above the start's
                        // read: measured, `for i in lo()..(if c { hi() } else
                        // { 9 })` printed HI before LO. The end's hoisted
                        // statements are therefore emitted HERE, between the two
                        // reads, where the source puts them.
                        let (end_src, end_hoists) = self.generate_expr_with_hoists(end)?;
                        self.output.push_str("        // For loop with range\n");
                        self.output
                            .push_str(&format!("        long long {} = ", lo));
                        self.generate_expression(start)?;
                        self.output.push_str(";\n");
                        self.output.push_str(&end_hoists);
                        self.output
                            .push_str(&format!("        long long {} = {};\n", hi, end_src));

                        // Record the loop variable so expressions in the body
                        // can be typed (see try_infer_expr_type/Expr::Ident).
                        // The scope opens *here*, before the binder is written,
                        // so the binder cannot outlive the loop.
                        let loop_scope = self.open_binding_scope();
                        self.bind_non_array(var, "long long".to_string());
                        if *inclusive {
                            self.output
                                .push_str(&format!("        if ({} <= {}) {{\n", lo, hi));
                            // THE SPAN IS COMPUTED IN UNSIGNED ARITHMETIC,
                            // OPERAND BY OPERAND. `(unsigned long long)(hi - lo)`
                            // does the SUBTRACTION first, in `long long`, and
                            // that overflows for any span wider than the signed
                            // maximum — UBSan on `-1..=<i64 max>`:
                            // "9223372036854775807 - -1 cannot be represented in
                            // type 'long long'". Converting each end first makes
                            // the subtraction modular, which is exactly the
                            // count wanted.
                            //
                            // AND THE LOOP CANNOT USE `k <= n`. When the span is
                            // the whole domain (`<i64 min>..=<i64 max>`) `n` is
                            // `ULLONG_MAX`, `k++` wraps to 0, and `k <= n` is
                            // true forever. The exit test is therefore "was the
                            // one just visited the last one", evaluated in the
                            // increment clause — which is also where `continue`
                            // lands, so it still advances.
                            self.output.push_str(&format!(
                                "        unsigned long long __pd_n{n} = \
                                 (unsigned long long){hi} - (unsigned long long){lo};\n",
                                n = n,
                                hi = hi,
                                lo = lo
                            ));
                            self.output.push_str(&format!(
                                "        for (unsigned long long __pd_k{n} = 0, __pd_done{n} = 0; \
                                 !__pd_done{n}; __pd_done{n} = (__pd_k{n} == __pd_n{n}), \
                                 __pd_k{n}++) {{\n",
                                n = n
                            ));
                            // AND THE VISITED VALUE IS ADDED IN UNSIGNED
                            // ARITHMETIC TOO. `lo + (long long)k` is a SIGNED
                            // addition whose right operand runs up to the span,
                            // so for a span wider than the signed maximum the
                            // last additions overflow — undefined behaviour by
                            // the same rule that made `v++` unusable, and
                            // invisible to any run test because reaching them
                            // takes 2^63 iterations. Adding as `unsigned long
                            // long` wraps by definition; the conversion back is
                            // implementation-defined rather than undefined, and
                            // gcc and clang both define it as modular, which is
                            // the value wanted.
                            self.output.push_str(&format!(
                                "        long long {} = (long long)((unsigned long long){} + \
                                 __pd_k{});\n",
                                var, lo, n
                            ));
                        } else {
                            self.output.push_str(&format!(
                                "        for (long long {v} = {lo}; {v} < {hi}; {v}++) {{\n",
                                v = var,
                                lo = lo,
                                hi = hi
                            ));
                        }

                        // Generate body. The `None` frame says a `break` in
                        // here belongs to THIS loop, which produces no value —
                        // same rule as `while`/`loop` above.
                        self.break_temps.push(None);
                        let generated = self.generate_block(body, "        ");
                        self.break_temps.pop();
                        generated?;
                        self.close_binding_scope(loop_scope);

                        self.output.push_str("        }\n");
                        if *inclusive {
                            self.output.push_str("        }\n");
                        }
                    }
                    // A range that is not written in the header — a `let`
                    // binding, a call's result — is a `__pd_range` value, and
                    // its bound has to be read out of the struct rather than
                    // from the source. The comparison consults `inclusive` on
                    // every step instead of computing `end + 1` once, because
                    // `end + 1` wraps at the maximum.
                    _ if self
                        .try_infer_expr_type(iter)
                        .is_some_and(|t| t == "__pd_range") =>
                    {
                        self.output
                            .push_str("        // For loop over a range value\n");
                        // Same two repairs as the header form above: the ends
                        // are read once, and the inclusive case never computes
                        // `last + 1`. `__pd_r` is numbered because a range-value
                        // loop can contain another one.
                        let n = self.hoist_counter;
                        self.hoist_counter += 1;
                        let r = format!("__pd_r{}", n);
                        self.output
                            .push_str(&format!("        __pd_range {} = ", r));
                        self.generate_expression(iter)?;
                        self.output.push_str(";\n");
                        self.output.push_str(&format!(
                            "        if ({r}.inclusive ? ({r}.start <= {r}.end) \
                             : ({r}.start < {r}.end)) {{\n",
                            r = r
                        ));
                        self.output.push_str(&format!(
                            "        long long __pd_last{n} = {r}.inclusive ? {r}.end \
                             : {r}.end - 1;\n",
                            n = n,
                            r = r
                        ));
                        let loop_scope = self.open_binding_scope();
                        self.bind_non_array(var, "long long".to_string());
                        // Same two properties as the header form above: the
                        // span is subtracted in unsigned arithmetic so it cannot
                        // overflow, and the exit test asks whether the value
                        // just visited was the last, so a full-domain range
                        // terminates instead of wrapping `k` back to 0 forever.
                        self.output.push_str(&format!(
                            "        unsigned long long __pd_n{n} = \
                             (unsigned long long)__pd_last{n} - (unsigned long long){r}.start;\n",
                            n = n,
                            r = r
                        ));
                        self.output.push_str(&format!(
                            "        for (unsigned long long __pd_k{n} = 0, __pd_done{n} = 0; \
                             !__pd_done{n}; __pd_done{n} = (__pd_k{n} == __pd_n{n}), \
                             __pd_k{n}++) {{\n",
                            n = n
                        ));
                        // Same unsigned addition as the header form: the
                        // signed `start + (long long)k` overflows on a span
                        // wider than the signed maximum.
                        self.output.push_str(&format!(
                            "        long long {} = (long long)((unsigned long long){}.start + \
                             __pd_k{});\n",
                            var, r, n
                        ));
                        self.break_temps.push(None);
                        let generated = self.generate_block(body, "        ");
                        self.break_temps.pop();
                        generated?;
                        self.close_binding_scope(loop_scope);
                        self.output.push_str("        }\n");
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
                        let iter_type = self.infer_expr_type(iter);
                        let (elem_type, dims) = Self::split_array_dims(&iter_type);
                        // N4-10. THE ELEMENT OF A NESTED ARRAY IS A ROW, AND C
                        // CANNOT COPY ONE INTO A LOOP VARIABLE: there is no
                        // `long long row[2] = g[_i];` in C. The two ways to
                        // emit something anyway are both wrong — dropping the
                        // inner dimension declares `long long row = g[_i];`,
                        // which gcc refuses as an int/pointer conversion, and
                        // binding a pointer would make `row` ALIAS the grid, so
                        // a write through it reaches the original. `for row in
                        // g` says row is a value. Refused by name instead.
                        if dims.matches('[').count() > 1 {
                            let name = match iter {
                                Expr::Ident(name) => name.as_str(),
                                _ => "<expression>",
                            };
                            let row_dims = &dims[dims[1..].find('[').map(|i| i + 1).unwrap_or(0)..];
                            return Err(CompileError::CodegenError {
                                message: format!(
                                    "cannot iterate `{}`: each step would bind a whole row \
                                     (`{}{}`), and a row is an array, which C cannot copy \
                                     into a loop variable. Loop over the indices and read \
                                     the rows through them: \
                                     `for i in 0..{} {{ let v = {}[i][0]; }}`.",
                                    name,
                                    elem_type,
                                    row_dims,
                                    match self.array_len_of_expr(iter) {
                                        Some(ArrayLen::Proven(n)) => n.to_string(),
                                        _ => "<len>".to_string(),
                                    },
                                    name
                                ),
                            });
                        }
                        let len = self.array_len_of_expr(iter);
                        let storage = match iter {
                            Expr::Ident(name) => self.array_bindings.get(name).map(|b| b.storage),
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

                        // Generate body. The `None` frame says a `break` in
                        // here belongs to THIS loop, which produces no value —
                        // same rule as `while`/`loop` above.
                        self.break_temps.push(None);
                        let generated = self.generate_block(body, "        ");
                        self.break_temps.pop();
                        generated?;
                        self.close_binding_scope(loop_scope);

                        self.output.push_str("        }\n");
                    }
                }
                self.output.push_str("    }\n");
            }
            Stmt::Loop { body, .. } => {
                // `while (1)`, not `for (;;)`. Both are the same loop in C; this
                // is the spelling `test_loop_keyword` in
                // tests/compiler_comprehensive_test.rs asserts, and the emitted
                // C is read by that test rather than only run.
                self.output.push_str("    while (1) {\n");
                self.break_temps.push(None);
                let result = self.generate_block(body, "    ");
                self.break_temps.pop();
                result?;
                self.output.push_str("    }\n");
            }
            Stmt::Break { value: None, .. } => {
                self.output.push_str("    break;\n");
            }
            Stmt::Break {
                value: Some(expr), ..
            } => {
                // The value goes into the temporary of the innermost loop, and
                // the type checker has already proved there is one. Two
                // statements, in this order: assign, then leave.
                let Some(Some(temp)) = self.break_temps.last().cloned() else {
                    return Err(CompileError::CodegenError {
                        message: "a `break` carries a value out of a loop that is not used as a \
                                  value; this should have been refused by the type checker"
                            .to_string(),
                    });
                };
                self.output.push_str("    ");
                self.emit_hoist_assignment(&temp, expr)?;
                self.output.push_str("    break;\n");
            }
            Stmt::Continue { .. } => {
                self.output.push_str("    continue;\n");
            }
            Stmt::Match { expr, arms, span } => {
                // Generate a series of if-else statements for pattern matching
                self.output.push_str("    // Match statement\n");
                self.output.push_str("    {\n");

                // THE TEMPORARY TAKES THE SCRUTINEE'S OWN C TYPE. It used to be
                // `long long` for everything that was not an enum, which was
                // survivable only because no pattern could look at a non-enum
                // value: with N6-02 a `String` scrutinee is matchable, and
                // `long long _match_expr = <const char*>` is not a program.
                let expr_type = self.infer_expr_type(expr);
                let temp_type = if expr_type.is_empty() {
                    "long long".to_string()
                } else {
                    expr_type
                };

                // Store the match expression in a temporary variable
                self.output
                    .push_str("        // Temporary for match expression\n");
                self.output
                    .push_str(&format!("        {} _match_expr = ", temp_type));
                self.generate_expression(expr)?;
                self.output.push_str(";\n");

                // TWO SHAPES, AND THE GUARD IS WHY. Without guards a match is an
                // if/else-if chain, which is what every reader and several tests
                // expect of it. A guard cannot live in that chain: it must see
                // the pattern's bindings (so it needs a statement position after
                // them), it may hoist statements of its own (a value expression
                // in a guard), and a guard that FAILS has to fall through to the
                // next arm — which an `else if` cannot express once the bindings
                // are inside the braces. So a match containing any guard becomes
                // a sequence of `if (!done)` blocks instead, evaluated in arm
                // order, and an unguarded match is emitted exactly as before.
                if arms.iter().any(|arm| arm.guard.is_some()) {
                    let end_label = format!("_match_end{}", self.hoist_counter);
                    self.hoist_counter += 1;

                    for arm in arms {
                        let condition = self.pattern_condition(&arm.pattern, "_match_expr")?;
                        self.output
                            .push_str(&format!("        if ({}) {{\n", condition));

                        let arm_scope = self.open_binding_scope();
                        let emitted = (|| -> Result<()> {
                            self.emit_pattern_bindings(&arm.pattern, "_match_expr", &temp_type)?;

                            match &arm.guard {
                                None => {
                                    self.generate_block(&arm.body, "        ")?;
                                    self.output
                                        .push_str(&format!("            goto {};\n", end_label));
                                }
                                Some(guard) => {
                                    // The guard's own hoisted statements land
                                    // HERE — after the bindings it may read, and
                                    // inside the pattern test, so a guard that
                                    // computes something does not compute it for
                                    // an arm whose pattern did not match.
                                    let (guard_src, guard_hoists) =
                                        self.generate_expr_with_hoists(guard)?;
                                    self.output.push_str(&guard_hoists);
                                    self.output
                                        .push_str(&format!("        if ({}) {{\n", guard_src));
                                    self.generate_block(&arm.body, "        ")?;
                                    self.output
                                        .push_str(&format!("            goto {};\n", end_label));
                                    self.output.push_str("        }\n");
                                }
                            }
                            Ok(())
                        })();
                        self.close_binding_scope(arm_scope);
                        emitted?;

                        self.output.push_str("        }\n");
                    }

                    // N6-11, and the reason this shape uses a LABEL rather than
                    // the `int done` flag it used to: with a flag, the trap sits
                    // behind `if (!done)` and gcc cannot prove the end of a
                    // tail-`match` function is unreachable — measured, and it is
                    // the single thing that kept `-Werror=return-type` out of the
                    // shared gcc invocation. A `goto` past the trap leaves the
                    // fall-through path unconditional, which gcc reads exactly.
                    self.output.push_str(&self.match_trap_body(*span));
                    self.output.push_str(&format!("    {}: ;\n", end_label));

                    self.output.push_str("    }\n");
                    return Ok(());
                }

                // Generate if-else chain for each arm
                for (i, arm) in arms.iter().enumerate() {
                    if i == 0 {
                        self.output.push_str("        if (");
                    } else {
                        self.output.push_str(" else if (");
                    }

                    let condition = self.pattern_condition(&arm.pattern, "_match_expr")?;
                    self.output.push_str(&condition);
                    self.output.push_str(") {\n");

                    let arm_scope = self.open_binding_scope();
                    let emitted = (|| -> Result<()> {
                        self.emit_pattern_bindings(&arm.pattern, "_match_expr", &temp_type)?;
                        self.generate_block(&arm.body, "        ")?;
                        Ok(())
                    })();
                    self.close_binding_scope(arm_scope);
                    emitted?;

                    self.output.push_str("        }");
                }

                // N6-11. THE FINAL ELSE. Every arm having failed, the program
                // stops here instead of walking out of the block with nothing
                // done — which for a value `match` meant reading a temporary
                // that held only its zero-initialiser.
                self.output.push_str(" else {\n");
                self.output.push_str(&self.match_trap_body(*span));
                self.output.push_str("        }");

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
    /// Rewrite `object.field(args)` into `TypeOfObject::field(object, args)`
    /// (N5-17).
    ///
    /// Does a REFERENCE parameter need `&` at the call site?
    ///
    /// Only references reach here; `mut` parameters are decided by their own flag. Two
    /// carve-outs, both measured rather than assumed:
    ///
    ///   * AN ARRAY REFERENT DECAYS. `fn write(xs: &mut [i64; 3])` is emitted as
    ///     `long long xs[3]`, i.e. `long long*`, and `&values` is `long long (*)[3]` -- a
    ///     DIFFERENT pointer type. `test_array_reference_parameters_compile_to_pointers`
    ///     pins `write(values)` for exactly this reason, and taking the address broke it.
    ///   * AN ARGUMENT THAT ALREADY PRODUCES AN ADDRESS. `write(&mut values)` has written
    ///     the `&` in the source; adding another would take the address of a reference.
    ///
    /// Everything else -- a `&self`/`&mut self` receiver, a `&Struct` parameter handed a
    /// place -- is a value where a pointer is wanted, and gets the `&` the declaration
    /// side has always expected.
    /// Can `&` be written in front of this expression in C?
    ///
    /// A PLACE HAS STORAGE; A TEMPORARY DOES NOT. C says so by refusing `&` on an rvalue,
    /// and this is the same question asked one compiler earlier, so the answer arrives as
    /// a Palladium diagnostic instead of a line of generated C the programmer never wrote.
    /// `Expr::Reference` is absent deliberately: it never reaches here, because
    /// `reference_param_needs_address` has already declined to add a second `&`.
    fn expr_is_addressable(arg: &Expr) -> bool {
        matches!(
            arg,
            Expr::Ident(_) | Expr::FieldAccess { .. } | Expr::Index { .. } | Expr::Deref { .. }
        )
    }

    fn reference_param_needs_address(ty: &Type, arg: &Expr) -> bool {
        let Type::Reference { inner, .. } = ty else {
            return false;
        };
        if matches!(inner.as_ref(), Type::Array(_, _)) {
            return false;
        }
        !matches!(arg, Expr::Reference { .. })
    }

    /// The receiver becomes the first argument and appears EXACTLY ONCE, so a
    /// receiver with a side effect happens once however the call is written.
    /// Its position among the arguments is C's business: C does not specify
    /// the order in which a call's arguments are evaluated, and that is already
    /// true of every multi-argument call this compiler emits.
    fn method_call_as_path_call(
        &self,
        object: &Expr,
        field: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Expr> {
        let receiver_type =
            self.try_infer_expr_type(object)
                .ok_or_else(|| CompileError::CodegenError {
                    message: format!(
                        "cannot infer the type of the receiver of `.{}`: no type rule for this \
                         {} expression",
                        field,
                        Self::expr_kind_name(object)
                    ),
                })?;
        let owner =
            Self::struct_name_of(&receiver_type).ok_or_else(|| CompileError::CodegenError {
                message: format!(
                    "the receiver of `.{}` has C type `{}`, which names no type that can carry \
                     an `impl` block; this should have been refused by the type checker",
                    field, receiver_type
                ),
            })?;

        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(object.clone());
        call_args.extend(args.iter().cloned());

        Ok(Expr::Call {
            func: Box::new(Expr::Ident(format!("{}::{}", owner, field))),
            args: call_args,
            span,
        })
    }

    /// A name no source program can collide with, for one hoisted value.
    fn fresh_hoist_name(&mut self) -> String {
        let n = self.hoist_counter;
        self.hoist_counter += 1;
        // `__pd` is reserved to this compiler and the digits make it unique;
        // `c_ident::escape_reserved_names` never produces this shape.
        format!("__pd_val{}", n)
    }

    /// Run `f` with a FRESH output buffer and return what it wrote, leaving
    /// `self.output` exactly as it was.
    ///
    /// Everything in this file writes by appending to one string, so composing
    /// a fragment out of order — a declaration whose type is only known after
    /// the body has been generated — needs the body captured rather than
    /// appended.
    fn capture_output<F>(&mut self, f: F) -> Result<String>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        let saved = std::mem::take(&mut self.output);
        let result = f(self);
        let captured = std::mem::replace(&mut self.output, saved);
        result?;
        Ok(captured)
    }

    /// Generate `expr` and report BOTH its text and the statements it hoisted,
    /// without splicing those statements anywhere.
    ///
    /// The question every conditional position has to ask. `pending_hoists` is
    /// spliced in front of the whole statement by `generate_statement`, which is
    /// right for a position that always runs and WRONG for one that runs
    /// sometimes — a `while` condition, the right operand of `&&`. Those callers
    /// generate first, look, and lower differently when something came out.
    fn generate_expr_with_hoists(&mut self, expr: &Expr) -> Result<(String, String)> {
        let saved = std::mem::take(&mut self.pending_hoists);
        let text = self.capture_output(|g| g.generate_expression(expr));
        let hoists = std::mem::replace(&mut self.pending_hoists, saved);
        Ok((text?, hoists))
    }

    /// Read ONE call argument into a temporary, in source order (N13-03).
    ///
    /// Returns the temporary's name, or `None` for an argument that has no
    /// observable read time and is therefore emitted in place.
    ///
    /// FOUR shapes come back `None` — three claims and one guard, and the
    /// count is stated because it was wrong in language-spec.md's A6.4 for two
    /// review rounds (it said two):
    ///
    /// * an ADDRESS-taken bare name (`mut` parameter). What the call reads is
    ///   the address of a fixed object; no earlier argument can move it.
    /// * a by-value bare name of ARRAY type. An array argument decays to a
    ///   pointer to storage that already exists, so again the read is of an
    ///   address, not of a value.
    /// * a PURE argument whose C type this pass cannot name. There is no honest
    ///   declaration to write, and inventing `long long` is how a `String`
    ///   becomes an integer (see `expr_c_type`). THIS ONE IS A RESIDUAL AND NOT
    ///   A PROOF: emitted inside the call, its read lands after every hoisted
    ///   read, which is its source position only if it was written last. It is
    ///   accepted rather than refused because a pure argument has no effect of
    ///   its own to misplace — it can read stale state, never skip a write —
    ///   and because refusing would reject programs that are fine. The
    ///   effectful half of the same branch IS refused, below.
    /// * an argument whose inferred C type is `void` or empty. This is the
    ///   GUARD, not a case of the rule: no call can pass a `void` argument, so
    ///   nothing reaches it, and it exists so that a future inference returning
    ///   `Some("void")` writes no `void __pd_valN = …;`.
    ///
    /// An EFFECTFUL argument whose type cannot be named is the one case that
    /// REFUSES rather than falling back. Emitting it in place would put a call
    /// back inside the C call expression, where the order is unspecified again
    /// — and it would do so silently, in exactly the position this rule exists
    /// to fix. No source reaches it today (`Expr::Question`,
    /// `Expr::MacroInvocation` and `Expr::Await` are the only always-`None`
    /// arms of `try_infer_expr_type_in`, and each is refused upstream before
    /// codegen), so this is a fail-closed guard on a latent branch and not a
    /// diagnostic users are expected to see.
    ///
    /// Everything else is declared and assigned here. The argument's OWN
    /// hoists — a value `if` written as an argument — are pushed first, so the
    /// order in `pending_hoists` is the order in the source.
    fn hoist_call_argument(
        &mut self,
        arg: &Expr,
        needs_address: bool,
        index: usize,
        callee: &str,
    ) -> Result<Option<String>> {
        if matches!(arg, Expr::Ident(_)) && needs_address {
            return Ok(None);
        }
        let Some(c_type) = self.try_infer_expr_type(arg) else {
            if Self::expr_is_pure(arg) {
                return Ok(None);
            }
            return Err(CompileError::CodegenError {
                message: format!(
                    "cannot sequence argument {} of `{}`: this {} can carry an effect, so \
                     N13-03 requires it to be read at its own position, and its C type \
                     cannot be named for a temporary to read it into. Leaving it inside \
                     the call would put the order back in the C compiler's hands",
                    index + 1,
                    callee,
                    Self::expr_kind_name(arg)
                ),
            });
        };
        let (base, dims) = Self::split_array_dims(&c_type);
        if base == "void" || base.is_empty() {
            return Ok(None);
        }
        if matches!(arg, Expr::Ident(_)) && !dims.is_empty() {
            return Ok(None);
        }

        let (text, arg_hoists) = self.generate_expr_with_hoists(arg)?;
        let temp = self.fresh_hoist_name();
        let decl = if needs_address {
            // `&place`: a pointer to the caller's storage, taken HERE rather
            // than wherever C would have taken it.
            format!(
                "    {} = &({});
",
                Self::pointer_declarator(&base, &dims, &temp),
                text
            )
        } else if dims.is_empty() {
            format!("    {} {} = {};
", base, temp, text)
        } else {
            // An array argument decays to a pointer to its first ELEMENT, so
            // the outermost dimension is the one that goes away:
            // `long long[3][2]` is passed as `long long (*)[2]`.
            let rest = match dims.find(']') {
                Some(i) => &dims[i + 1..],
                None => "",
            };
            format!(
                "    {} = {};
",
                Self::pointer_declarator(&base, rest, &temp),
                text
            )
        };
        self.pending_hoists.push_str(&arg_hoists);
        self.pending_hoists.push_str(&decl);
        Ok(Some(temp))
    }

    /// Emit `{ stmts...; <temp> = value; }` for one branch of a value
    /// expression, and report the C type of `value`.
    ///
    /// The type is asked for INSIDE the block's binding scope, which is the
    /// whole reason this is not `generate_block`: the tail of
    /// `{ let a = 3; a * 2 }` is typed from `a`.
    ///
    /// The value's own hoists (an `if` expression nested in the tail) are
    /// collected separately and emitted inside this block, in front of the
    /// assignment — they are statements of THIS branch, not of the statement
    /// that contains the whole construct.
    fn generate_hoisted_block(
        &mut self,
        stmts: &[Stmt],
        value: Option<&Expr>,
        temp: &str,
        indent: &str,
    ) -> Result<(String, Option<String>)> {
        let Some(value) = value else {
            // The type checker refuses a value block with no tail expression
            // (`check_value_block`), so reaching this means the two passes
            // disagree — say so rather than emitting a C block that assigns
            // nothing and leaves the temporary uninitialised.
            return Err(CompileError::CodegenError {
                message: "a block used as a value has no trailing expression; this should have \
                          been refused by the type checker"
                    .to_string(),
            });
        };

        let outer = self.open_binding_scope();
        let saved_hoists = std::mem::take(&mut self.pending_hoists);

        let generated = (|| -> Result<(String, Option<String>)> {
            let mut text =
                self.capture_output(|g| g.generate_stmts_in_current_scope(stmts, indent))?;
            let c_type = self.try_infer_expr_type(value);
            let value_src = self.capture_output(|g| g.generate_expression(value))?;
            let value_hoists = std::mem::take(&mut self.pending_hoists);
            text.push_str(&Self::reindent_to(&value_hoists, indent));
            text.push_str(&format!("{}{} = {};\n", indent, temp, value_src));
            Ok((text, c_type))
        })();

        self.pending_hoists = saved_hoists;
        self.close_binding_scope(outer);
        generated
    }

    /// The operand of the first value-carrying `break` that binds to this loop.
    ///
    /// Used only as a CHEAP GUESS for the temporary's C type before the body is
    /// generated; the authoritative answer is recorded by
    /// [`CodeGenerator::emit_hoist_assignment`] while the body's bindings are
    /// live. Skips nested loops, because a `break` in one of those binds there.
    fn first_break_value(stmts: &[Stmt]) -> Option<&Expr> {
        for stmt in stmts {
            let found = match stmt {
                Stmt::Break { value, .. } => value.as_ref(),
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => Self::first_break_value(then_branch)
                    .or_else(|| else_branch.as_deref().and_then(Self::first_break_value)),
                Stmt::Match { arms, .. } => arms
                    .iter()
                    .find_map(|arm| Self::first_break_value(&arm.body)),
                Stmt::Unsafe { body, .. } => Self::first_break_value(body),
                // A `break` in here belongs to THAT loop.
                Stmt::Loop { .. } | Stmt::While { .. } | Stmt::For { .. } => None,
                _ => None,
            };
            if found.is_some() {
                return found;
            }
        }
        None
    }

    /// Emit `<temp> = <expr>;` and record the temporary's C type the first time.
    ///
    /// The recording is the whole reason this is not an inline `push_str`: this
    /// is called from inside a `match` arm or a loop body, where the pattern
    /// bindings and locals the value refers to are in scope, and the
    /// declaration that needs the type is written outside them.
    fn emit_hoist_assignment(&mut self, temp: &str, expr: &Expr) -> Result<()> {
        if self.open_hoists.contains(temp) && !self.hoist_types.contains_key(temp) {
            if let Some(c_type) = self.try_infer_expr_type(expr) {
                self.hoist_types.insert(temp.to_string(), c_type);
            }
        }
        self.output.push_str(&format!("{} = ", temp));
        self.generate_expression(expr)?;
        self.output.push_str(";\n");
        Ok(())
    }

    /// Generate a synthesised statement into a fresh buffer, with `temp`
    /// registered so every `<temp> = …;` it writes reports the type it assigned.
    ///
    /// Returns the C text and the type, or a diagnostic naming the temporary
    /// that no assignment could type.
    fn generate_into_hoist_temp<F>(&mut self, temp: &str, f: F) -> Result<(String, Option<String>)>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        self.open_hoists.insert(temp.to_string());
        let saved_hoists = std::mem::take(&mut self.pending_hoists);
        let generated = self.capture_output(f);
        self.pending_hoists = saved_hoists;
        self.open_hoists.remove(temp);

        let text = generated?;
        Ok((text, self.hoist_types.remove(temp)))
    }

    /// The C initialiser that writes a zero of `c_type`.
    ///
    /// `{0}` for anything aggregate, `0` for scalars and pointers. Used only
    /// where a temporary must be defined before a chain that a C compiler
    /// cannot prove writes it.
    fn zero_of(c_type: &str) -> &'static str {
        let scalar = c_type.ends_with('*')
            || matches!(
                c_type,
                "long long"
                    | "long"
                    | "int"
                    | "short"
                    | "char"
                    | "unsigned"
                    | "double"
                    | "float"
                    | "size_t"
            );
        if scalar {
            "0"
        } else {
            "{0}"
        }
    }

    /// The C name of the struct standing for a tuple shape (N4-12).
    ///
    /// PURE, so `type_to_c` can call it from a `&self` context: mangling a shape
    /// and REGISTERING one are different acts, and only the second needs to
    /// mutate. The mangling is of the element C types themselves, so it is
    /// stable across runs (no hash seed, no counter) and a nested shape's name
    /// contains its inner shape's name.
    fn tuple_c_name(element_c_types: &[String]) -> String {
        let mut name = format!("__pd_tuple{}", element_c_types.len());
        for c_type in element_c_types {
            name.push('_');
            let mut last_was_underscore = true;
            for ch in c_type.chars() {
                if ch.is_ascii_alphanumeric() {
                    name.push(ch);
                    last_was_underscore = false;
                } else if ch == '*' {
                    name.push_str("p");
                    last_was_underscore = false;
                } else if !last_was_underscore {
                    name.push('_');
                    last_was_underscore = true;
                }
            }
        }
        name
    }

    /// Register every tuple shape an EXPRESSION builds, innermost first.
    ///
    /// A pure walk: it reads, infers and registers, and writes nothing to the
    /// output. That is the whole point — see the note at the `Expr::Tuple` arm
    /// for what the alternative cost.
    ///
    /// The catch-all covers the expression forms that cannot lexically contain
    /// another expression (literals, identifiers, paths) and the forms whose
    /// contents are refused before code generation (`?`, `.await`, macros).
    fn register_tuple_shapes_in(
        &mut self,
        expr: &Expr,
        locals: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        match expr {
            Expr::Tuple { elements, span } => {
                for element in elements {
                    self.register_tuple_shapes_in(element, locals)?;
                }
                let mut element_types = Vec::with_capacity(elements.len());
                for element in elements {
                    element_types.push(self.expr_c_type(element, locals, *span)?);
                }
                self.register_tuple(&element_types)?;
            }
            Expr::TupleIndex { expr, .. }
            | Expr::Unary { operand: expr, .. }
            | Expr::Cast { expr, .. }
            | Expr::FieldAccess { object: expr, .. }
            | Expr::Deref { expr, .. }
            | Expr::Reference { expr, .. }
            | Expr::Question { expr, .. }
            | Expr::Await { expr, .. } => self.register_tuple_shapes_in(expr, locals)?,
            Expr::Binary { left, right, .. } => {
                self.register_tuple_shapes_in(left, locals)?;
                self.register_tuple_shapes_in(right, locals)?;
            }
            Expr::Index { array, index, .. } => {
                self.register_tuple_shapes_in(array, locals)?;
                self.register_tuple_shapes_in(index, locals)?;
            }
            Expr::Range { start, end, .. } => {
                self.register_tuple_shapes_in(start, locals)?;
                self.register_tuple_shapes_in(end, locals)?;
            }
            Expr::Call { func, args, .. } => {
                self.register_tuple_shapes_in(func, locals)?;
                for arg in args {
                    self.register_tuple_shapes_in(arg, locals)?;
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                for element in elements {
                    self.register_tuple_shapes_in(element, locals)?;
                }
            }
            Expr::ArrayRepeat { value, count, .. } => {
                self.register_tuple_shapes_in(value, locals)?;
                self.register_tuple_shapes_in(count, locals)?;
            }
            Expr::StructLiteral { fields, .. } => {
                for (_, value) in fields {
                    self.register_tuple_shapes_in(value, locals)?;
                }
            }
            Expr::EnumConstructor { data, .. } => {
                if let Some(data) = data {
                    match data {
                        EnumConstructorData::Tuple(args) => {
                            for arg in args {
                                self.register_tuple_shapes_in(arg, locals)?;
                            }
                        }
                        EnumConstructorData::Struct(fields) => {
                            for (_, value) in fields {
                                self.register_tuple_shapes_in(value, locals)?;
                            }
                        }
                    }
                }
            }
            // THE THREE FORMS THAT OPEN A SCOPE extend `locals` exactly as
            // `try_infer_expr_type_in` does, and for the same reason: a tuple
            // built from a name this expression binds has a type, and a walk
            // that looked only at the function's variables could not see it.
            // Measured before this: `(match p { P::Num(n) => (n, 1), … }, 9)`
            // and a tuple built from a branch-local were refused outright.
            Expr::If {
                condition,
                then_branch,
                then_value,
                else_branch,
                else_value,
                ..
            } => {
                self.register_tuple_shapes_in(condition, locals)?;
                if let Some(value) = then_value {
                    let branch_locals = self.locals_of(then_branch, locals);
                    self.register_tuple_shapes_in(value, &branch_locals)?;
                }
                if let Some(value) = else_value {
                    let branch_locals = match else_branch {
                        Some(stmts) => self.locals_of(stmts, locals),
                        None => locals.clone(),
                    };
                    self.register_tuple_shapes_in(value, &branch_locals)?;
                }
            }
            Expr::Block { stmts, value, .. } => {
                if let Some(value) = value {
                    let block_locals = self.locals_of(stmts, locals);
                    self.register_tuple_shapes_in(value, &block_locals)?;
                }
            }
            Expr::Match { expr, arms, .. } => {
                self.register_tuple_shapes_in(expr, locals)?;
                let scrutinee = self.try_infer_expr_type_in(expr, locals);
                for arm in arms {
                    if let Some(value) = &arm.value {
                        // Pattern first, then the body over that environment —
                        // see the note at `try_infer_expr_type_in`'s `Expr::Match`
                        // arm, which had the same inversion and is fixed with it.
                        let mut env = locals.clone();
                        if let Some(scrutinee) = scrutinee.as_deref() {
                            self.bind_pattern_locals(&arm.pattern, scrutinee, &mut env);
                        }
                        let arm_locals = self.locals_of(&arm.body, &env);
                        self.register_tuple_shapes_in(value, &arm_locals)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Record a tuple shape so its struct and constructor are emitted, and
    /// answer with its C name.
    ///
    /// THE MANGLING IS NOT INJECTIVE, and this is where that is caught rather
    /// than where it would be paid. `tuple_c_name` sanitises each element type
    /// and joins with `_`, so a struct named `A_B` beside `C` mangles to the same
    /// name as `A` beside `B_C`. The encoding could be length-framed instead;
    /// refusing is cheaper and, for a collision nobody has hit, more honest than
    /// a scheme whose correctness argument is longer than the bug. What must NOT
    /// happen is the silent version: keeping the first layout and emitting the
    /// second tuple with the first's fields.
    fn register_tuple(&mut self, element_c_types: &[String]) -> Result<String> {
        let name = Self::tuple_c_name(element_c_types);
        if !self
            .tuple_shapes
            .insert(name.clone(), element_c_types.to_vec())
        {
            let existing = self
                .tuple_shapes
                .element_types(&name)
                .map(|t| t.join(", "))
                .unwrap_or_default();
            return Err(CompileError::CodegenError {
                message: format!(
                    "two different tuple shapes mangle to the same C name `{}`: ({}) and ({}). \
                     The name is built by sanitising each element type, so an underscore in a \
                     type name can move the boundary between elements. Rename one of the types \
                     involved; this is a compiler limitation and not a limit of the language",
                    name,
                    existing,
                    element_c_types.join(", ")
                ),
            });
        }
        Ok(name)
    }

    /// Walk every written type in the program and register the tuple shapes.
    ///
    /// TYPES ONLY. A tuple that is CONSTRUCTED registers itself when its
    /// expression is generated; this pass exists for the tuples that are only
    /// ever named — a parameter, a return type, a `let` annotation — which no
    /// expression in this unit may build.
    fn register_tuple_types_in(&mut self, program: &Program) -> Result<()> {
        fn types_in_stmt(stmt: &Stmt, out: &mut Vec<Type>) {
            match stmt {
                Stmt::Let { ty: Some(ty), .. } => out.push(ty.clone()),
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    for s in then_branch {
                        types_in_stmt(s, out);
                    }
                    for s in else_branch.iter().flatten() {
                        types_in_stmt(s, out);
                    }
                }
                Stmt::While { body, .. }
                | Stmt::For { body, .. }
                | Stmt::Loop { body, .. }
                | Stmt::Unsafe { body, .. } => {
                    for s in body {
                        types_in_stmt(s, out);
                    }
                }
                Stmt::Match { arms, .. } => {
                    for arm in arms {
                        for s in &arm.body {
                            types_in_stmt(s, out);
                        }
                    }
                }
                _ => {}
            }
        }

        let mut types: Vec<Type> = Vec::new();
        let mut functions: Vec<&Function> = Vec::new();
        for item in &program.items {
            match item {
                Item::Function(func) => functions.push(func),
                Item::Impl(impl_block) => functions.extend(impl_block.methods.iter()),
                _ => {}
            }
        }
        for func in functions {
            for param in &func.params {
                types.push(param.ty.clone());
            }
            if let Some(ret) = &func.return_type {
                types.push(ret.clone());
            }
            for stmt in &func.body {
                types_in_stmt(stmt, &mut types);
            }
        }
        for ty in &types {
            self.register_tuple_type(ty)?;
        }
        Ok(())
    }

    /// Register every tuple shape a written TYPE names, innermost first.
    fn register_tuple_type(&mut self, ty: &Type) -> Result<()> {
        match ty {
            Type::Tuple(types) => {
                let mut element_types = Vec::with_capacity(types.len());
                for element in types {
                    self.register_tuple_type(element)?;
                    element_types.push(self.type_to_c(element));
                }
                self.register_tuple(&element_types)?;
            }
            Type::Array(element, _) => self.register_tuple_type(element)?,
            Type::Reference { inner, .. } => self.register_tuple_type(inner)?,
            Type::Future { output } => self.register_tuple_type(output)?,
            _ => {}
        }
        Ok(())
    }

    /// The C type of an expression, or a refusal that names the expression.
    ///
    /// Tuple construction needs its elements' types to pick the struct, and an
    /// element whose type this backend cannot work out is a program it must not
    /// emit C for — silently choosing `long long` is how a `String` element
    /// would become an integer.
    fn expr_c_type(
        &self,
        expr: &Expr,
        locals: &std::collections::HashMap<String, String>,
        span: Span,
    ) -> Result<String> {
        self.try_infer_expr_type_in(expr, locals)
            .ok_or_else(|| CompileError::CodegenError {
                // NOT "un-inferable". A name bound by a `match` arm or declared
                // in a branch has a type; what this pass may lack is the SCOPE it
                // was bound in, and saying so points the reader at the right
                // thing. `locals` is threaded from the walk for exactly that
                // reason — the message is what is left when threading it is not
                // enough.
                message: format!(
                    "this {} has no type code generation can see here, so the tuple it is an \
                     element of has no C struct to be built from. If it is a name, it is bound \
                     in a scope this pass did not thread through",
                    Self::expr_kind_name(expr)
                ),
            })
            .map_err(|e| {
                let _ = span;
                e
            })
    }

    /// Emit one C struct and one constructor per tuple shape (N4-12).
    ///
    /// AFTER the struct and enum definitions, because an element may be one of
    /// those; a tuple INSIDE an enum payload or a struct field is refused by
    /// name for the mirror reason — its definition would have to come first.
    fn tuple_definitions(&self) -> String {
        if self.tuple_shapes.is_empty() {
            return String::new();
        }
        let shapes: Vec<(String, Vec<String>)> = self
            .tuple_shapes
            .iter()
            .map(|(name, types)| (name.clone(), types.clone()))
            .collect();
        let mut out = String::from("// Tuple shapes (N4-12)\n");
        // (built into a string and spliced at the marker recorded after the
        // struct and enum definitions: a shape may be registered while a
        // FUNCTION BODY is generated, long after that point in the output)
        for (name, element_types) in shapes {
            out.push_str("typedef struct {\n");
            for (i, c_type) in element_types.iter().enumerate() {
                out.push_str(&format!("    {} f{};\n", c_type, i));
            }
            out.push_str(&format!("}} {};\n", name));

            let params: Vec<String> = element_types
                .iter()
                .enumerate()
                .map(|(i, c_type)| format!("{} f{}", c_type, i))
                .collect();
            out.push_str(&format!(
                "static {} {}_new({}) {{\n",
                name,
                name,
                params.join(", ")
            ));
            out.push_str(&format!("    {} t;\n", name));
            for i in 0..element_types.len() {
                out.push_str(&format!("    t.f{} = f{};\n", i, i));
            }
            out.push_str("    return t;\n");
            out.push_str("}\n\n");
        }
        out
    }

    /// The two statements a failed `match` runs (N6-11).
    ///
    /// `abort()` is written HERE and not inside `__pd_match_trap`, and that is
    /// the whole reason the helper only prints: `-Wreturn-type` is not
    /// interprocedural, so a tail `match` whose arms all `return` would still
    /// look like it can fall off the end of its function if the noreturn call
    /// were one level down. With `abort()` at the site, gcc can see the end is
    /// unreachable — which is what lets `-Werror=return-type` be turned on at
    /// all (src/linker.rs) and what retires the zero-initialiser that stood in
    /// for it.
    fn match_trap_body(&self, span: Span) -> String {
        format!(
            "            __pd_match_trap(\"{} at line {}\");\n            abort();\n",
            self.current_fn_name, span.line
        )
    }

    /// The C expression that decides whether an arm's PATTERN matches `subject`.
    ///
    /// One place, because the two match shapes above (else-if chain and
    /// `if (!done)` sequence) must ask the same question; two spellings of
    /// "does this arm match" is how a guarded and an unguarded match would come
    /// to disagree about the same pattern.
    ///
    /// RECURSIVE OVER THE SUBJECT, and it has to be. A pattern nested in an
    /// enum payload tests a FIELD, not the scrutinee, and the first version of
    /// this stopped at the tag: `P::Num(1)` matched every `Num` and the program
    /// ran with the wrong arm, exit 0. `subject` is the C lvalue this pattern
    /// is being asked about — `_match_expr` at the top, a payload member below.
    fn pattern_condition(&self, pattern: &Pattern, subject: &str) -> Result<String> {
        Ok(match pattern {
            // Both match everything. `1` rather than an omitted test, so the
            // emitted chain keeps one shape.
            Pattern::Wildcard | Pattern::Ident(_) => "1".to_string(),
            // N6-08. The binding is transparent: what decides is its inner.
            Pattern::Binding { inner, .. } => self.pattern_condition(inner, subject)?,
            // N6-05. Element by element, on the members the tuple struct has.
            // The same recursion as an enum payload, so everything that composes
            // there composes here: a literal, a range, an `@`, alternatives, a
            // nested tuple.
            Pattern::Tuple(elements) => {
                let mut parts = Vec::new();
                for (i, element) in elements.iter().enumerate() {
                    let member = format!("{}.f{}", subject, i);
                    let condition = self.pattern_condition(element, &member)?;
                    if condition != "1" {
                        parts.push(format!("({})", condition));
                    }
                }
                if parts.is_empty() {
                    "1".to_string()
                } else {
                    parts.join(" && ")
                }
            }
            // N6-07. The alternatives' own tests, joined. `||` short-circuits,
            // so a later alternative is not evaluated once an earlier one holds
            // — which matters as soon as an alternative's test is a `strcmp`.
            Pattern::Or(alternatives) => {
                let mut parts = Vec::with_capacity(alternatives.len());
                for alternative in alternatives {
                    parts.push(format!("({})", self.pattern_condition(alternative, subject)?));
                }
                parts.join(" || ")
            }
            Pattern::EnumPattern {
                enum_name,
                variant,
                data,
            } => {
                let mut condition = format!("{}.tag == __{}__{}", subject, enum_name, variant);
                for (sub_pattern, member) in self.payload_subjects(enum_name, variant, data.as_ref(), subject)
                {
                    let sub_condition = self.pattern_condition(&sub_pattern, &member.subject)?;
                    if sub_condition != "1" {
                        condition.push_str(&format!(" && ({})", sub_condition));
                    }
                }
                condition
            }
            // N6-02. An integer or a bool is C's own `==`; a STRING is not —
            // `const char* == const char*` compares addresses, so `"be" + "ta"`
            // would fail to match `"beta"` despite being the same text. The
            // runtime already carries `__pd_string_eq` (a `strcmp`), emitted
            // into every output file, and this is the comparison a reader means
            // by a string pattern.
            Pattern::Literal(PatternLiteral::Int(value)) => {
                format!("{} == {}", subject, Self::c_i64_literal(*value))
            }
            Pattern::Literal(PatternLiteral::Bool(value)) => {
                format!("{} == {}", subject, if *value { 1 } else { 0 })
            }
            Pattern::Literal(PatternLiteral::Str(value)) => {
                format!("__pd_string_eq({}, \"{}\")", subject, c_string_body(value))
            }
            // N4-04's representation, not a C character constant: `char` is a
            // `long long` holding the Unicode scalar, so the test is numeric.
            // `c_char_constant` is the one place that spells one, and it carries
            // the source spelling in a trailing comment for a reader of the C.
            Pattern::Literal(PatternLiteral::Char(value)) => {
                format!("{} == {}", subject, c_char_constant(*value))
            }
            // N6-03. Two comparisons on the same subject. Parenthesised because
            // this string is pasted into larger conditions — an `&&` inside an
            // `||` alternative, or beside an enum tag test.
            Pattern::Range { lo, hi, inclusive } => {
                let bound = |literal: &PatternLiteral| match literal {
                    PatternLiteral::Int(value) => Self::c_i64_literal(*value),
                    PatternLiteral::Bool(value) => (if *value { 1 } else { 0 }).to_string(),
                    // Ordered by code point, which is the same `long long` the
                    // equality arm above compares — so a char range is the very
                    // same two comparisons an integer range is.
                    PatternLiteral::Char(value) => c_char_constant(*value),
                    // Refused by the type checker before this point; a range of
                    // strings has no `>=` in C either.
                    PatternLiteral::Str(value) => format!("\"{}\"", c_string_body(value)),
                };
                format!(
                    "({} >= {} && {} {} {})",
                    subject,
                    bound(lo),
                    subject,
                    if *inclusive { "<=" } else { "<" },
                    bound(hi)
                )
            }
        })
    }

    /// An `i64` as a C expression of type `long long`.
    ///
    /// `i64::MIN` IS NOT WRITABLE AS A C LITERAL, and writing it anyway is a
    /// sign inversion rather than a syntax error: `-9223372036854775808` is unary
    /// minus applied to `9223372036854775808`, which does not fit in `long long`,
    /// so the constant takes an UNSIGNED type and `x >= <that>` asks "is x
    /// negative" instead of "is x at least the minimum". Measured before the
    /// repair: `match 5 { -9223372036854775808..=10 => 1, _ => 0 }` printed 0.
    ///
    /// `(-9223372036854775807LL - 1)` is the spelling C guarantees — the operand
    /// fits, the subtraction is exact, and the type is `long long`.
    ///
    /// EVERY OTHER LITERAL CARRIES `LL` TOO, AND THAT SUFFIX IS A BUG FIX RATHER
    /// THAN A STYLE. A C integer literal small enough to fit takes type `int`,
    /// so the ARITHMETIC AROUND IT is 32-bit however wide the destination is —
    /// and this language has one integer width, `i64`. Measured before the
    /// suffix, every one silent, compiled, linked, exit 0:
    ///   `const BIG: i64 = 1 << 62;`            printed -2147483648
    ///   `const BIG: i64 = 2147483647 + 1;`     printed -2147483648
    ///   `const BIG: i64 = 1000000 * 1000000;`  printed -727379968
    ///   `let x: i64 = 1 << 62;`                printed 8261746944
    /// The last one is the reason the fix belongs HERE and not in the constant
    /// evaluator: the same class was already live in ordinary function bodies,
    /// where no evaluator runs. One emission point, both classes.
    ///
    /// `1LL << 62` is the same VALUE as `1 << 62` would be if C had computed it
    /// in 64 bits, so no correct program's output moves — verified by running
    /// the whole conformance corpus and diffing every transcript.
    fn c_i64_literal(value: i64) -> String {
        if value == i64::MIN {
            "(-9223372036854775807LL - 1)".to_string()
        } else {
            format!("{}LL", value)
        }
    }

    /// Every sub-pattern of an enum variant's payload, paired with the C lvalue
    /// and type it applies to.
    ///
    /// ONE PLACE THAT KNOWS THE PAYLOAD LAYOUT, consulted by both the condition
    /// and the binding emission. They used to be written separately, and only
    /// one of them knew about payload sub-patterns at all.
    fn payload_subjects(
        &self,
        enum_name: &str,
        variant: &str,
        data: Option<&PatternData>,
        subject: &str,
    ) -> Vec<(Pattern, PayloadSubject)> {
        let Some(pattern_data) = data else {
            return Vec::new();
        };
        let Some(enum_def) = self.enums.get(enum_name) else {
            return Vec::new();
        };
        let Some(variant_def) = enum_def.variants.iter().find(|v| v.name == variant) else {
            return Vec::new();
        };

        // An indirect slot holds a CELL: the pattern is about the value, so the
        // subject reads through it. Parenthesised because `*x.y` binds the way C
        // says and every caller pastes this into a larger expression.
        let read = |subject: String, ty: &Type| {
            if self.recursive_layout.payload_is_indirect(enum_name, ty) {
                format!("(*{})", subject)
            } else {
                subject
            }
        };

        match (&variant_def.data, pattern_data) {
            (EnumVariantData::Tuple(types), PatternData::Tuple(patterns)) => patterns
                .iter()
                .zip(types.iter())
                .enumerate()
                .map(|(i, (pattern, ty))| {
                    (
                        pattern.clone(),
                        PayloadSubject {
                            subject: read(
                                format!(
                                    "{}.data.{}.field{}",
                                    subject,
                                    c_ident::c_enum_payload_member(variant),
                                    i
                                ),
                                ty,
                            ),
                            c_type: self.type_to_c(ty),
                        },
                    )
                })
                .collect(),
            (EnumVariantData::Struct(fields), PatternData::Struct(field_patterns)) => {
                field_patterns
                    .iter()
                    .filter_map(|(field_name, pattern)| {
                        let (_, field_type) =
                            fields.iter().find(|(fname, _)| fname == field_name)?;
                        Some((
                            pattern.clone(),
                            PayloadSubject {
                                subject: read(
                                    format!(
                                    "{}.data.{}.{}",
                                    subject,
                                    c_ident::c_enum_payload_member(variant),
                                    field_name
                                ),
                                    field_type,
                                ),
                                c_type: self.type_to_c(field_type),
                            },
                        ))
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Declare the variables an arm's pattern binds, at the top of its block.
    ///
    /// The bindings are written before anything else in the arm — before the
    /// guard, which may read them, and before the body.
    fn emit_pattern_bindings(
        &mut self,
        pattern: &Pattern,
        subject: &str,
        subject_type: &str,
    ) -> Result<()> {
        match pattern {
            // None of these bind anything.
            Pattern::Wildcard | Pattern::Literal(_) | Pattern::Range { .. } => Ok(()),
            // N6-07. An alternative may not bind — the type checker refuses that
            // by name, because a single `||` condition has no per-alternative
            // site to assign from.
            Pattern::Or(_) => Ok(()),
            // N6-05. Each element binds against its own member, typed from the
            // shape registry — the same read `Expr::TupleIndex` performs, so a
            // `String` element cannot become a `long long` here either.
            Pattern::Tuple(elements) => {
                // A REGISTRY MISS IS AN ERROR, NOT A `long long`. This binds C
                // variables, and the rule stated at `infer_expr_type` applies:
                // a caller that DECLARES a variable may not guess its type,
                // because guessing emits silently wrong C — a `const char*`
                // element bound to a `long long` is a pointer stored in an
                // integer, which surfaces as a gcc error against code the user
                // never wrote, if it surfaces at all.
                let Some(element_types) = self.tuple_shapes.element_types(subject_type) else {
                    return Err(CompileError::CodegenError {
                        message: format!(
                            "no tuple shape is registered for `{}`, so the elements this \
                             pattern binds have no types to be declared with",
                            subject_type
                        ),
                    });
                };
                let element_types = element_types.to_vec();
                for (i, element) in elements.iter().enumerate() {
                    let member = format!("{}.f{}", subject, i);
                    let Some(member_type) = element_types.get(i).cloned() else {
                        return Err(CompileError::CodegenError {
                            message: format!(
                                "the tuple shape `{}` has {} element(s), and this pattern binds \
                                 element {}",
                                subject_type,
                                element_types.len(),
                                i
                            ),
                        });
                    };
                    self.emit_pattern_bindings(element, &member, &member_type)?;
                }
                Ok(())
            }
            Pattern::Ident(name) => {
                // TYPED AS THE SUBJECT, not as `long long`. The old hardcoded
                // width was invisible while only enums could be matched; a
                // `String` scrutinee bound to a `long long` is a program gcc
                // refuses.
                self.output.push_str(&format!(
                    "            {} {} = {};\n",
                    subject_type, name, subject
                ));
                self.bind_non_array(name, subject_type.to_string());
                Ok(())
            }
            // N6-08. Both: the name for this whole position, and whatever the
            // inner pattern binds under it.
            Pattern::Binding { name, inner } => {
                self.output.push_str(&format!(
                    "            {} {} = {};\n",
                    subject_type, name, subject
                ));
                self.bind_non_array(name, subject_type.to_string());
                self.emit_pattern_bindings(inner, subject, subject_type)
            }
            Pattern::EnumPattern {
                enum_name,
                variant,
                data,
            } => {
                // Collected before anything is emitted: the writes below need
                // `&mut self`, and holding a borrow of `self.enums` across them
                // is what forced the previous version to reach into
                // `self.variables` by hand.
                let payload = self.payload_subjects(enum_name, variant, data.as_ref(), subject);
                for (sub_pattern, member) in payload {
                    self.emit_pattern_bindings(&sub_pattern, &member.subject, &member.c_type)?;
                }
                Ok(())
            }
        }
    }

    /// Lower a `match` in value position (N5-04) to portable C.
    ///
    /// Rewrites each arm's value into `<temp> = <value>;` appended to that
    /// arm's statements, then hands the resulting STATEMENT `match` to the
    /// ordinary emitter. Everything the arms need — the tag test, the payload
    /// extraction, the binding scopes — is already there and is not written
    /// twice.
    fn generate_match_expression(&mut self, expr: &Expr) -> Result<()> {
        let Expr::Match {
            expr: scrutinee,
            arms,
            span,
        } = expr
        else {
            unreachable!("generate_match_expression called on {:?}", expr);
        };

        let temp = self.fresh_hoist_name();

        let mut stmt_arms = Vec::with_capacity(arms.len());
        for arm in arms {
            let Some(value) = arm.value.as_ref() else {
                return Err(CompileError::CodegenError {
                    message: "an arm of a `match` used as a value produces nothing; this should \
                              have been refused by the type checker"
                        .to_string(),
                });
            };
            let mut body = arm.body.clone();
            body.push(Stmt::Assign {
                target: AssignTarget::Ident(temp.clone()),
                value: value.clone(),
                span: *span,
            });
            stmt_arms.push(MatchArm {
                pattern: arm.pattern.clone(),
                guard: arm.guard.clone(),
                body,
            });
        }

        let synthesised = Stmt::Match {
            expr: (**scrutinee).clone(),
            arms: stmt_arms,
            span: *span,
        };

        let (text, c_type) =
            self.generate_into_hoist_temp(&temp, |g| g.generate_statement(&synthesised))?;
        let c_type = self.hoist_temp_type([c_type, None], "`match`")?;

        // DECLARED WITH AN INITIALISER, AND THE REASON IS NOW BELT-AND-BRACES
        // — said plainly, because the measurement that used to be cited here was
        // WRONG.
        //
        // The reason it was added is gone: `match` ends in a trap, so no path
        // leaves this temporary unwritten. A previous revision of this comment
        // claimed the initialiser was still load-bearing and cited five
        // `-Wuninitialized` diagnostics in `tests/06_match_expression`'s C and
        // two in `tests/06_guards`'. RE-MEASURED, stripping ONLY the value-match
        // temporaries' ` = 0` (the earlier pass had also stripped initialisers
        // off unrelated declarations, which is where its warnings came from):
        // Apple clang 21, `-O2 -Wall -Wextra`, 16/9/8 initialisers removed from
        // three fixtures' C — ZERO uninitialized diagnostics in all three.
        //
        // So it is kept as belt-and-braces, not as a load-bearing store: one
        // instruction against a compiler with weaker flow analysis than the one
        // this box has (GNU gcc is not installed here, so that half is unmeasured
        // and is claimed as a possibility rather than a fact). If someone wants
        // it gone, the measurement above is the argument for deleting it.
        self.pending_hoists.push_str(&format!(
            "    {} {} = {};\n",
            c_type,
            temp,
            Self::zero_of(&c_type)
        ));
        self.pending_hoists.push_str(&text);

        self.output.push_str(&temp);
        Ok(())
    }

    /// Lower a `loop` in value position (N5-07) to portable C.
    ///
    /// The body is emitted as the ordinary statement `loop`, with the
    /// temporary pushed as this loop's break target so that
    /// `break <value>;` becomes `<temp> = <value>; break;` — and so that a
    /// `break` inside a NESTED loop, which pushes its own `None` frame, cannot
    /// reach it.
    fn generate_loop_expression(&mut self, expr: &Expr) -> Result<()> {
        let Expr::Loop { body, .. } = expr else {
            unreachable!("generate_loop_expression called on {:?}", expr);
        };

        let temp = self.fresh_hoist_name();
        let temp_for_body = temp.clone();

        // The `while (1)` wrapper is written HERE rather than by synthesising a
        // `Stmt::Loop` and reusing its arm. That arm pushes a `None` break
        // frame — correct for a loop written for its effect, and exactly wrong
        // for this one, which must be the target its `break`s assign. Measured:
        // synthesising it made every `break <value>;` in a value loop report
        // "carries a value out of a loop that is not used as a value".
        let generated = self.generate_into_hoist_temp(&temp, |g| {
            g.output.push_str("    while (1) {\n");
            g.break_temps.push(Some(temp_for_body.clone()));
            let result = g.generate_block(body, "    ");
            g.break_temps.pop();
            result?;
            g.output.push_str("    }\n");
            Ok(())
        });
        let (text, c_type) = generated?;

        let c_type = self.hoist_temp_type(
            [
                c_type,
                Self::first_break_value(body).and_then(|e| self.try_infer_expr_type(e)),
            ],
            "`loop`",
        )?;

        // No initialiser here: the only way out of a `loop` is a `break`, and
        // the type checker has proved every one of them carries a value, so
        // control cannot reach the use without having written it.
        self.pending_hoists
            .push_str(&format!("    {} {};\n", c_type, temp));
        self.pending_hoists.push_str(&text);

        self.output.push_str(&temp);
        Ok(())
    }

    /// The C declaration type for a hoisted temporary, or a diagnostic.
    fn hoist_temp_type(&self, candidates: [Option<String>; 2], kind: &str) -> Result<String> {
        let inferred =
            candidates
                .into_iter()
                .flatten()
                .next()
                .ok_or_else(|| CompileError::CodegenError {
                    message: format!(
                        "cannot infer the type of this {} expression's value, so the temporary \
                         that holds it cannot be declared. Bind the branches to annotated \
                         `let`s instead.",
                        kind
                    ),
                })?;
        if inferred.contains('[') {
            // `T x[n]` is not assignable in C, so a hoisted array temporary
            // would emit code gcc refuses. Refused here, by name.
            return Err(CompileError::CodegenError {
                message: format!(
                    "an array cannot be the value of a {} expression: C has no array assignment, \
                     so the hoisted temporary could not be written to",
                    kind
                ),
            });
        }
        Ok(inferred)
    }

    /// Lower an `if` in value position (N5-03) to portable C.
    ///
    /// `let x = if c { 1 } else { 2 };` becomes
    ///
    /// ```text
    /// long long __pd_val0;
    /// if (c) { __pd_val0 = 1; } else { __pd_val0 = 2; }
    /// long long x = __pd_val0;
    /// ```
    ///
    /// A conditional expression (`c ? 1 : 2`) would be shorter and is wrong:
    /// a branch may run statements before its value, and C's `?:` has nowhere
    /// to put them.
    fn generate_if_expression(&mut self, expr: &Expr) -> Result<()> {
        let Expr::If {
            condition,
            then_branch,
            then_value,
            else_branch,
            else_value,
            ..
        } = expr
        else {
            unreachable!("generate_if_expression called on {:?}", expr);
        };
        let Some(else_branch) = else_branch else {
            return Err(CompileError::CodegenError {
                message: "an `if` used as a value has no `else` branch; this should have been \
                          refused by the type checker"
                    .to_string(),
            });
        };

        let temp = self.fresh_hoist_name();

        // The condition is evaluated BEFORE the `if`, so anything it hoists
        // belongs in the enclosing statement's prelude - which is where
        // `pending_hoists` already points.
        let condition_src = self.capture_output(|g| g.generate_expression(condition))?;

        let (then_text, then_type) =
            self.generate_hoisted_block(then_branch, then_value.as_deref(), &temp, "        ")?;
        let (else_text, else_type) =
            self.generate_hoisted_block(else_branch, else_value.as_deref(), &temp, "        ")?;

        let c_type = self.hoist_temp_type([then_type, else_type], "`if`")?;

        self.pending_hoists
            .push_str(&format!("    {} {};\n", c_type, temp));
        self.pending_hoists
            .push_str(&format!("    if ({}) {{\n", condition_src));
        self.pending_hoists.push_str(&then_text);
        self.pending_hoists.push_str("    } else {\n");
        self.pending_hoists.push_str(&else_text);
        self.pending_hoists.push_str("    }\n");

        self.output.push_str(&temp);
        Ok(())
    }

    /// Lower a block in value position (N5-05) to portable C.
    ///
    /// The braces are kept in the emitted C rather than flattened, so a local
    /// the block binds cannot collide with a name in the enclosing function -
    /// `{ let a = 3; a }` beside an outer `a` is two variables in Palladium and
    /// has to stay two in C.
    fn generate_block_expression(&mut self, expr: &Expr) -> Result<()> {
        let Expr::Block { stmts, value, .. } = expr else {
            unreachable!("generate_block_expression called on {:?}", expr);
        };

        let temp = self.fresh_hoist_name();
        let (body, c_type) =
            self.generate_hoisted_block(stmts, value.as_deref(), &temp, "        ")?;
        let c_type = self.hoist_temp_type([c_type, None], "block")?;

        self.pending_hoists
            .push_str(&format!("    {} {};\n", c_type, temp));
        self.pending_hoists.push_str("    {\n");
        self.pending_hoists.push_str(&body);
        self.pending_hoists.push_str("    }\n");

        self.output.push_str(&temp);
        Ok(())
    }

    fn generate_expression(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            // N4-12. Built through a CONSTRUCTOR FUNCTION and not a compound
            // literal `(T){a, b}`: compound literals are C99 and the generated
            // prelude is C89 against whatever `cc` the host has — the same
            // reason `__pd_range_new` exists.
            Expr::Tuple { elements, span } => {
                // EVERY NESTED SHAPE IS REGISTERED BEFORE THIS ONE, by a walk
                // that EMITS NOTHING. Registration order is definition order in
                // the emitted C, so an outer shape recorded first would be
                // defined before the inner struct its field has the type of:
                // measured, `let n = ((1, 2), 3);` in a program with no other
                // tuple reached gcc as "unknown type name
                // '__pd_tuple2_long_long_long_long'". The shipped fixtures hid it
                // because a function signature had already named the inner shape.
                //
                // THE FIRST REPAIR SPECULATIVELY GENERATED each nested element
                // into a discarded buffer, which registered the shapes as a side
                // effect. That worked, and it was the wrong shape of fix: it ran
                // real emission for its side effects, so its correctness depended
                // on `capture_output` snapshotting every mutable channel the
                // generation could touch — `pending_hoists`, `hoist_counter`,
                // `open_hoists`, `break_temps`, `variables`, `array_bindings`,
                // `tuple_shapes` — and on that list staying complete as the
                // generator grows. `register_tuple_shapes_in` asks for none of
                // that: it walks the expression, computes C types, and registers.
                // The generator's own variable map is the outer scope here: a
                // tuple written in statement position sees the function's
                // locals, and the walk extends that as it descends.
                let outer: std::collections::HashMap<String, String> = self.variables.clone();
                self.register_tuple_shapes_in(expr, &outer)?;
                let mut element_types = Vec::with_capacity(elements.len());
                for element in elements {
                    element_types.push(self.expr_c_type(element, &outer, *span)?);
                }
                let name = self.register_tuple(&element_types)?;
                self.output.push_str(&format!("{}_new(", name));
                for (i, element) in elements.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.generate_expression(element)?;
                }
                self.output.push(')');
            }
            Expr::TupleIndex { expr, index, .. } => {
                // Parenthesised: the base may be any expression, and `f().f0`
                // parses differently from `(f()).f0` only by luck.
                self.output.push('(');
                self.generate_expression(expr)?;
                self.output.push_str(&format!(").f{}", index));
            }
            Expr::String(s) => {
                self.output.push_str(&format!("\"{}\"", c_string_body(s)));
            }
            Expr::Integer(n) => {
                // Through the same helper the patterns use: `i64::MIN` has no C
                // literal spelling, and writing it as one silently changes the
                // constant's TYPE to unsigned. No expression reaches it by
                // parsing (the lexer's `-?[0-9]+` cannot produce it either, since
                // the digits alone overflow), but the optimizer folds
                // `-9223372036854775807 - 1` into exactly this node — and with
                // `-Werror=return-type` and friends on the command line, an
                // unsigned constant in a signed comparison is a diagnostic away
                // from being a build failure.
                self.output.push_str(&Self::c_i64_literal(*n));
            }
            Expr::Float(x) => {
                // `{:?}` on an f64 always writes a `.`, so `3.0` cannot come out
                // as `3` — which C would read as an int and, in a `double`
                // context, silently change the type of a division.
                self.output.push_str(&format!("{:?}", x));
            }
            Expr::Char(c) => {
                self.output.push_str(&c_char_constant(*c));
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
                        let is_array = self.variables.get(name).is_some_and(|ty| ty.contains('['));

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
            Expr::Call { func, args, span } => {
                // METHOD CALL SYNTAX (N5-17). Rewritten to the path call it
                // means — `x.f(a)` is `TypeOfX::f(x, a)` — and then emitted by
                // the ordinary call path below, which already mangles a `::`
                // name to `__pd_Type_f`. Rewriting rather than emitting here
                // keeps ONE call emitter: the array-capability check, the
                // built-in mapping and the generic mangling all still run.
                //
                // The type checker performed the same rewrite and already
                // proved the method exists; this arm re-derives it from the C
                // type because code generation does not receive the checker's
                // conclusions.
                if let Expr::FieldAccess { object, field, .. } = func.as_ref() {
                    let rewritten = self.method_call_as_path_call(object, field, args, *span)?;
                    return self.generate_expression(&rewritten);
                }

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
                            // N6-11's neighbour. `panic(...)` never comes back,
                            // and gcc has no way to know that from a call to a
                            // function defined in this file whose body it has
                            // not inlined — so `-Werror=return-type` rejected
                            // `if c { v } else { panic("...") }`, C that is
                            // right and that `scripts/check-c-returns.py` has
                            // always accepted (its `NORETURN_RE`). The comma
                            // operator puts `abort()` at the call site, where
                            // gcc reads it, and keeps the whole thing an
                            // EXPRESSION — of type `void`, like the call it
                            // replaces, so it works in the STATEMENT positions a
                            // `void` call worked in. It does not make `panic`
                            // usable where a value is wanted, and nothing in this
                            // language puts it there.
                            "panic" => {
                                self.output.push_str("(__pd_panic");
                            }
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
                    // `impl_method_params` is consulted when `functions` misses, because a
                    // method call has already been rewritten to `Type::method(recv, ..)` by
                    // `method_call_as_path_call`, and methods were never registered in
                    // `functions`. Without this the receiver of every `&self`/`&mut self`
                    // method was emitted by value against a pointer parameter.
                    Expr::Ident(name) => self
                        .functions
                        .get(name)
                        .map(|(params, _)| params.clone())
                        .or_else(|| self.impl_method_params.get(name).cloned()),
                    _ => None,
                };

                // N13-03. ARGUMENTS ARE EVALUATED LEFT TO RIGHT, and C does
                // not evaluate them in any order the standard names — so the
                // guarantee cannot rest on the compiler that reads this file.
                // It is STRUCTURAL: every argument READ happens at that
                // argument's own position — value arguments into temporaries
                // declared in source order, place arguments as a pointer taken
                // there — and the call names the temporaries. The two shapes
                // that get no temporary, and what each rests on, are on
                // `hoist_call_argument`.
                //
                // MEASURED, and the reason a pure argument is hoisted too:
                // `static mut G = 10; add2(bump(), G)` where `bump()` writes
                // `G` printed 100 on this host — `G` read AFTER the write. That
                // is the answer the source asks for, and nothing in the emitted
                // C required it; a gcc that reads `G` first would have printed
                // 11. So the rule is not "sequence the effectful arguments":
                // an effectful argument has to be ordered against the LATER
                // PURE ones as well, and only reading every argument that has
                // a read at its own position says that.
                //
                // It fires on calls of TWO OR MORE arguments with at least one
                // effectful argument, because that is exactly when the order is
                // observable. A one-argument call has nothing to be ordered
                // against — which is also what keeps `panic`'s comma-operator
                // shape below composing, and what keeps the emitted C for the
                // vast majority of calls byte-identical.
                //
                // NOT IN SCOPE: struct-literal field order and binary-operator
                // operand order are their own rows.
                let sequenced = args.len() >= 2 && !args.iter().all(Self::expr_is_pure);
                let callee = match func.as_ref() {
                    Expr::Ident(name) => name.clone(),
                    _ => "<indirect>".to_string(),
                };

                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }

                    // Does this parameter arrive as a POINTER? The call side used to ask a
                    // NARROWER question than the declaration side: `param.mutable` alone,
                    // while parameters are emitted by `is_pointer`. A `&self`/`&mut self`
                    // receiver is recorded by the parser as `mutable: false` with a
                    // `Type::Reference` type, so it fell through the gap: declared
                    // `const struct C* self`, called `__pd_C_get(c)`.
                    //
                    // NOT THE SAME FUNCTION AS THE DECLARATION SIDE'S, and the difference is
                    // load-bearing rather than incidental: `is_pointer` is a question about a
                    // TYPE, and this one is also about the ARGUMENT -- an array referent and
                    // an argument that already produces an address both want the `&` left
                    // off. The claim here is the weaker and true one: the two sides now agree
                    // about which parameters are pointers, having disagreed before.
                    let needs_address = if let Some(params) = &func_params {
                        i < params.len()
                            && (params[i].mutable
                                || Self::reference_param_needs_address(&params[i].ty, arg))
                    } else {
                        false
                    };

                    // TAKING THE ADDRESS OF A TEMPORARY IS NOT C, AND THIS IS WHERE IT WAS
                    // EMITTED. `sink(make())` against `fn sink(c: &C)` passed the whole front
                    // end and died in gcc with `cannot take the address of an rvalue of type
                    // 'struct C'` -- front-end approval plus a C-compiler rejection, which is
                    // the one class this compiler does not ship.
                    //
                    // NOT FIXED BY HOISTING. The hoist path below emits `T* t = &(expr);`, so
                    // it takes the address of the same rvalue one line earlier; and a single
                    // argument is never `sequenced` anyway. Materialising `T t = expr;` and
                    // passing `&t` would be new lowering rather than something that falls out
                    // of what is here, so the refusal is by NAME and the temp is not built.
                    if needs_address && !Self::expr_is_addressable(arg) {
                        return Err(CompileError::CodegenError {
                            message: format!(
                                "cannot pass a temporary as argument {} of `{}`: the parameter \
                                 is a reference, so the call site takes its address, and this \
                                 {} has no storage to point at. Bind it to a local first and \
                                 pass that",
                                i + 1,
                                callee,
                                Self::expr_kind_name(arg)
                            ),
                        });
                    }

                    if sequenced {
                        if let Some(temp) = self.hoist_call_argument(arg, needs_address, i, &callee)?
                        {
                            self.output.push_str(&temp);
                            continue;
                        }
                    }

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
                if matches!(func.as_ref(), Expr::Ident(name) if name == "panic") {
                    self.output.push_str(", abort())");
                }
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                // `&&` AND `||` DO NOT EVALUATE THEIR RIGHT OPERAND WHEN THE
                // LEFT DECIDES, and a hoisted right operand did.
                //
                // MEASURED: `let x = flag && { print("leaked"); true };` with
                // `flag` false PRINTED "leaked". The block's statements were
                // spliced in front of the whole `let`, so they ran before the
                // `&&` was even reached — short-circuiting silently gone.
                //
                // Lowered rather than refused, and only when the right operand
                // actually hoists, so an ordinary `a && b` keeps emitting `a &&
                // b`. The left operand never needs this: it always runs.
                if matches!(op, BinOp::And | BinOp::Or) {
                    let (rhs_src, rhs_hoists) = self.generate_expr_with_hoists(right)?;
                    let c_op = if matches!(op, BinOp::And) {
                        " && "
                    } else {
                        " || "
                    };

                    if rhs_hoists.is_empty() {
                        self.output.push('(');
                        self.generate_expression(left)?;
                        self.output.push_str(c_op);
                        self.output.push_str(&rhs_src);
                        self.output.push(')');
                        return Ok(());
                    }

                    let temp = self.fresh_hoist_name();
                    // The left operand is generated into the enclosing prelude,
                    // where anything IT hoists belongs — it is unconditional.
                    let lhs_src = self.capture_output(|g| g.generate_expression(left))?;
                    self.pending_hoists
                        .push_str(&format!("    int {};\n", temp));
                    self.pending_hoists
                        .push_str(&format!("    {} = ({});\n", temp, lhs_src));
                    self.pending_hoists.push_str(&format!(
                        "    if ({}{}) {{\n",
                        if matches!(op, BinOp::And) { "" } else { "!" },
                        temp
                    ));
                    self.pending_hoists.push_str(&rhs_hoists);
                    self.pending_hoists
                        .push_str(&format!("        {} = ({});\n", temp, rhs_src));
                    self.pending_hoists.push_str("    }\n");
                    self.output.push_str(&temp);
                    return Ok(());
                }

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
                        // N5-12. Every binary operand is already wrapped in
                        // parentheses by the emitter around this match, so C's
                        // own precedence for these — which puts `&`/`^`/`|`
                        // LOOSER than the comparisons, unlike Palladium and
                        // unlike Rust — cannot change what the program means.
                        BinOp::BitAnd => " & ",
                        BinOp::BitOr => " | ",
                        BinOp::BitXor => " ^ ",
                        BinOp::Shl => " << ",
                        BinOp::Shr => " >> ",
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
                span,
            } => {
                // `Type::method(args)` arrives here as an enum constructor
                // because the parser had no types to tell the two apart
                // (N5-17). THE RULE IS THE TYPE CHECKER'S, restated where it is
                // applied: it is a constructor if and only if the name is an
                // enum's. See `TypeChecker::path_names_an_enum`.
                //
                // THE TWO PREDICATES ARE NOT IDENTICAL, AND THE ASYMMETRY IS
                // SAFE. `path_names_an_enum` consults `enums` AND
                // `generic_enums`; this consults `enums` alone. A generic enum
                // is therefore "not an enum" here — and it still cannot be
                // mis-emitted, because the diversion below requires a POSITIVE
                // hit in `functions` or `impl_methods`, and a generic-enum
                // constructor is in neither. Falling through the `if` is the
                // constructor path, which is the right answer.
                //
                // That is an argument about a code path, so it is pinned by a
                // test rather than left as one: `a_generic_enum_constructor_is
                // _not_emitted_as_a_call` in tests/m2_value_form_lowering.rs,
                // with `the_path_call_diversion_needs_a_positive_function_hit`
                // as its other half. Unifying the predicates would be a change
                // with no measured defect behind it.
                if !self.enums.contains_key(enum_name) {
                    if let Some(EnumConstructorData::Tuple(exprs)) = data {
                        let qualified = format!("{}::{}", enum_name, variant);
                        if self.functions.contains_key(&qualified)
                            || self.impl_methods.contains_key(&qualified)
                        {
                            return self.generate_expression(&Expr::Call {
                                func: Box::new(Expr::Ident(qualified)),
                                args: exprs.clone(),
                                span: *span,
                            });
                        }
                    }
                }

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
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                // N5-14. This arm used to be the refusal "Range expressions can
                // only be used in for loops", which was true of the CODE and
                // not of the language: the parser built a range anywhere an
                // expression could go, and the program died three passes later.
                self.output.push_str("__pd_range_new(");
                self.generate_expression(start)?;
                self.output.push_str(", ");
                self.generate_expression(end)?;
                self.output
                    .push_str(&format!(", {})", if *inclusive { 1 } else { 0 }));
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
                    UnaryOp::BitNot => {
                        self.output.push_str("(~(");
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
                // that WAS generated was the free function `<name>_poll`.
                return Err(CompileError::await_unimplemented(*span));
            }
            // Value positions. Both leave a temporary's NAME here and their
            // statements in `pending_hoists`; see `generate_statement` for
            // where those get spliced back in.
            Expr::If { .. } => self.generate_if_expression(expr)?,
            Expr::Block { .. } => self.generate_block_expression(expr)?,
            Expr::Cast { expr, ty, .. } => {
                // `(long long)5`, with no parentheses added around the operand:
                // `generate_expression` already parenthesises every compound
                // form, and the reviewed test `test_as_cast` reads the emitted
                // C for exactly this spelling.
                //
                // THE ONE CONVERSION C DOES NOT DO FOR US is to `bool`.
                // Palladium's `bool` is C's `int`, so `(int)5` would be 5 —
                // truthy, but not `true`, and `5 as bool as i64` would print 5
                // instead of 1. A cast to `bool` is a comparison against zero,
                // which is what the conversion MEANS.
                //
                // AND A CAST TO `char` IS CHECKED. `as char` is the only way to
                // build a character from a number, so it is the only place a
                // value that is not a Unicode scalar can enter the type.
                // MEASURED before this check: `55296 as char` (a UTF-16
                // surrogate) and `99999999 as char` (past U+10FFFF) both
                // compiled, and `string_from_char` then wrote the low byte —
                // an empty line and a garbage byte respectively, with no
                // diagnostic. That is the silent-wrong class, so it traps at
                // the conversion, the way `string_char_at` traps an index with
                // no character behind it.
                if matches!(ty, Type::Char) {
                    self.output.push_str("__pd_char_from_scalar(");
                    self.generate_expression(expr)?;
                    self.output.push(')');
                } else if matches!(ty, Type::Bool) {
                    self.output.push_str("((");
                    self.generate_expression(expr)?;
                    self.output.push_str(") != 0)");
                } else {
                    self.output.push_str(&format!("({})", self.type_to_c(ty)));
                    self.generate_expression(expr)?;
                }
            }
            Expr::Match { .. } => self.generate_match_expression(expr)?,
            Expr::Loop { .. } => self.generate_loop_expression(expr)?,
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
        // `LL` ON EVERY INTEGER LITERAL is the repair for C computing the
        // arithmetic around a small literal in 32 bits (see `c_i64_literal`),
        // so these assertions moved with the emission rather than around it.
        assert!(codegen.output.contains("int x = 42LL;"));
        assert!(codegen.output.contains("long long y = 100LL;"));
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
        assert!(c.contains("struct N n = N_A__new(5LL);"), "{}", c);
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
        assert!(c.contains("xs[0LL] = 9LL;"), "{}", c);
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
    // `impl` methods were the case the review named, and THE HOLE IS LIVE NOW,
    // not latent. This comment used to end "the refusal covers it if `::` calls
    // are ever routed here" — N5-17 routed them here. Both `Type::method(xs)`
    // and `x.method(xs)` are rewritten into an ordinary `Expr::Call` whose
    // callee is `Type::method` (see `method_call_as_path_call` and the
    // `EnumConstructor` arm), so they reach this guard, and `impl` methods are
    // still deliberately kept out of `functions`. Measured on the shape the
    // review named: passing an array to `h.mutate(v)` is refused with the
    // message below naming `Holder::mutate`.
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
        assert!(
            msg.contains("cannot pass an array to `print_int`"),
            "{}",
            msg
        );
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
        assert!(
            msg.contains("cannot establish where the array came from"),
            "{}",
            msg
        );
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
        assert!(
            msg.contains("cannot establish where the array came from"),
            "{}",
            msg
        );
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
        assert!(c.contains("__pd_print_int(arr[0LL])"), "{}", c);
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
        assert_eq!(
            c.matches("long long head(long long xs[]").count(),
            2,
            "{}",
            c
        );
    }

    // The declarator recurses through `array_shape` now, so a nested array
    // parameter is EMITTED rather than refused: the dimensions go after the
    // name, outermost first (N4-10, pinned end to end by
    // tests/02_types_nested_arrays.pd).
    //
    // What is still refused BY NAME is an UNRESOLVED INNER length. Every
    // dimension after the outermost is the stride C computes a row from, and a
    // length this pass cannot resolve has no honest spelling there - `[0]`
    // would compute wrong addresses silently
    // (tests/reject/nested_array_param_inner_length.pd). Function types do not
    // reach here at all - the parser refuses them ("expected type, found 'fn'",
    // language-spec.md §5).
    #[test]
    fn test_nested_array_parameter_declarator_and_inner_length_refusal() {
        let c = generate(
            r#"
        fn f(g: [[i64; 2]; 3]) -> i64 { return 0; }
        fn main() { print("ok"); }
        "#,
        )
        .unwrap();
        assert!(c.contains("long long f(long long g[3][2])"), "{}", c);

        let err = generate(
            r#"
        fn f(g: [[i64; N]; 3]) -> i64 { return g[0][0]; }
        fn main() { print("ok"); }
        "#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(
                "cannot declare the parameter `g`: the inner array length is written as `N`"
            ),
            "{}",
            msg
        );
    }

    // N13-03. THE GUARANTEE IS THE EMITTED SHAPE, not what this host's gcc
    // happens to do. Measured: `add3(a(), b(), c())` ran left to right here
    // BEFORE any of this existed, so the conformance fixture
    // (tests/03_arg_evaluation_order.pd) cannot fail on a host that agrees with
    // us by accident. This test is the one that can: it reads the C.
    #[test]
    fn test_argument_reads_are_sequenced_left_to_right() {
        let c = generate(
            r#"
        static mut G: i64 = 0;
        fn bump() -> i64 { G = 99; return 1; }
        fn add2(x: i64, y: i64) -> i64 { return x + y; }
        fn main() { print_int(add2(bump(), G)); }
        "#,
        )
        .unwrap();

        // Both arguments are read, in source order, BEFORE the call.
        let first = c.find("long long __pd_val0 = bump();").unwrap_or_else(|| panic!("{}", c));
        let second = c.find("long long __pd_val1 = G;").unwrap_or_else(|| panic!("{}", c));
        let call = c
            .find("add2(__pd_val0, __pd_val1)")
            .unwrap_or_else(|| panic!("{}", c));
        assert!(first < second && second < call, "{}", c);

        // And the effectful argument is not left inside the call, where C
        // would have been free to run it after `G` was read.
        assert!(!c.contains("add2(bump()"), "{}", c);
    }

    // The rule fires on the position where the order is OBSERVABLE, and not
    // anywhere else. A one-argument call has nothing to be ordered against, so
    // it keeps its shape - which is also what keeps `panic`'s comma-operator
    // form composing, and what keeps this change off the emitted C of almost
    // every call in the corpus.
    #[test]
    fn test_a_single_argument_and_an_all_pure_call_are_not_sequenced() {
        let c = generate(
            r#"
        fn f() -> i64 { return 1; }
        fn add2(x: i64, y: i64) -> i64 { return x + y; }
        fn main() {
            let n: i64 = 2;
            print_int(f());
            print_int(add2(n, 3));
            if n > 100 { panic("no"); }
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("__pd_print_int(f())"), "{}", c);
        assert!(c.contains("__pd_print_int(add2(n, 3LL))"), "{}", c);
        assert!(c.contains("(__pd_panic(\"no\"), abort())"), "{}", c);
        assert!(!c.contains("__pd_val"), "{}", c);
    }

    // A `mut` parameter takes the ADDRESS of the caller's storage, and an
    // ARRAY argument decays to a pointer. Neither is a value to copy, so both
    // are hoisted as POINTERS - which is what sequences an effectful
    // subscript, the one shape that could otherwise smuggle a call past the
    // rule (measured before the fix: `take(&xs[idx()], eff())`, one C
    // expression, order unspecified).
    #[test]
    fn test_place_arguments_are_sequenced_as_pointers() {
        let c = generate(
            r#"
        fn idx() -> i64 { return 1; }
        fn eff() -> i64 { return 2; }
        fn take(mut x: i64, y: i64) -> i64 { x = 1; return y; }
        fn row_sum(r: [i64; 2], k: i64) -> i64 { return r[0] + k; }
        fn main() {
            let mut xs: [i64; 3] = [10, 20, 30];
            let grid: [[i64; 2]; 3] = [[1, 2], [3, 4], [5, 6]];
            print_int(take(xs[idx()], eff()));
            print_int(row_sum(grid[idx()], eff()));
        }
        "#,
        )
        .unwrap();
        assert!(c.contains("long long *__pd_val0 = &(xs[idx()]);"), "{}", c);
        assert!(c.contains("take(__pd_val0, __pd_val1)"), "{}", c);
        assert!(c.contains("long long *__pd_val2 = grid[idx()];"), "{}", c);
        assert!(c.contains("row_sum(__pd_val2, __pd_val3)"), "{}", c);
    }

    // THE FAIL-CLOSED HALF OF N13-03, on a branch NO SOURCE REACHES TODAY.
    //
    // `try_infer_expr_type_in` has exactly three arms that always answer
    // `None` — `Expr::Question`, `Expr::MacroInvocation` and `Expr::Await` —
    // and every one of them is an effectful form. Each is refused before
    // codegen runs, MEASURED at the source level rather than assumed:
    //
    //   add2(g()?, 2)        "the `?` operator is not implemented"
    //   add2(g().await, 2)   "a `return` with a value inside an `async fn`
    //                         is not implemented"
    //   add2(vec!(1), 2)     "Type mismatch: expected Int, found [Int; 1]"
    //   add2(nope(), 2)      "Undefined function: nope"
    //
    // So the argument is SYNTHETIC, deliberately, for the same reason
    // `tuple_shape_tests::the_mangling_can_collide` builds its inputs by hand:
    // the guard exists for a future spelling, and a test that could not run is
    // worse than one that is honest about being constructed. What it pins is
    // that the branch REFUSES instead of quietly emitting the call in place —
    // which would put an effectful argument back inside the C call expression,
    // in exactly the position this rule exists to fix.
    #[test]
    fn test_an_effectful_argument_with_no_nameable_type_is_refused() {
        let mut codegen = CodeGenerator::new("test").unwrap();
        let span = Span::new(0, 0, 1, 1);
        let unnameable = Expr::Await {
            expr: Box::new(Expr::Integer(1)),
            span,
        };
        assert!(!CodeGenerator::expr_is_pure(&unnameable));
        assert!(codegen.try_infer_expr_type(&unnameable).is_none());

        let err = codegen
            .hoist_call_argument(&unnameable, false, 1, "add2")
            .unwrap_err()
            .to_string();
        assert!(err.contains("cannot sequence argument 2 of `add2`"), "{}", err);

        // A PURE argument keeps the fallback: its read time is not observable,
        // so emitting it in place loses nothing and refusing it would reject
        // programs that are fine.
        let pure = Expr::Ident("x".to_string());
        assert!(CodeGenerator::expr_is_pure(&pure));
        assert!(codegen.try_infer_expr_type(&pure).is_none());
        assert_eq!(
            codegen.hoist_call_argument(&pure, false, 0, "add2").unwrap(),
            None
        );
    }

    // SEQUENCING MUST NOT UNDO SHORT-CIRCUITING. `&&` does not evaluate its
    // right operand when the left decides, and argument temporaries are
    // statements — put them above the whole call and the right operand runs
    // unconditionally, which is the defect the `Expr::Binary` arm was written
    // to fix for hoisted blocks.
    //
    // It composes because the `&&` lowering captures the right operand's
    // hoists and emits them INSIDE the guard it generates. This test pins that
    // composition: the inner call's argument temporaries appear after the
    // `if (__pd_val...)` line, not before it.
    //
    // ANCHORED PAST `int main`, AND THAT IS THE WHOLE POINT OF THE FIRST
    // VERSION'S FAILURE. It searched the WHOLE translation unit for `if (`,
    // which the runtime preamble contains at byte 869 (inside
    // `__pd_alloc_string`) while `int main` does not start until byte 8564 —
    // measured, both operators. So `guard_at < first_temp` was true of the
    // preamble and not of the lowering, and the test would have stayed green
    // with the temps hoisted anywhere in `main`. It now anchors on the guard
    // this lowering actually emits (`if (__pd_val` / `if (!__pd_val`, byte
    // 8730) inside `main`, and states the property in the form a regression
    // breaks: NO argument temporary appears between `main` and the guard.
    #[test]
    fn test_sequenced_arguments_stay_inside_a_short_circuit_guard() {
        for (op, guard) in [("&&", "if (__pd_val"), ("||", "if (!__pd_val")] {
            let source = format!(
                r#"
        fn a() -> i64 {{ return 1; }}
        fn b() -> i64 {{ return 2; }}
        fn inner(x: i64, y: i64) -> bool {{ return x < y; }}
        fn use2(c: bool, k: i64) -> i64 {{ if c {{ return k; }} return 0; }}
        fn main() {{
            let flag: bool = false;
            let n: i64 = 7;
            print_int(use2(flag {} inner(a(), b()), n));
        }}
        "#,
                op
            );
            let c = generate(&source).unwrap();
            let main_at = c.find("int main").unwrap_or_else(|| panic!("{}", c));
            let body = &c[main_at..];
            let guard_at = body.find(guard).unwrap_or_else(|| panic!("{}", c));
            let first_temp = body.find("= a();").unwrap_or_else(|| panic!("{}", c));
            let second_temp = body.find("= b();").unwrap_or_else(|| panic!("{}", c));
            assert!(
                guard_at < first_temp && first_temp < second_temp,
                "{} lowering put the argument temps outside the guard:\n{}",
                op,
                c
            );

            // The displacement a regression causes, stated directly: hoisting
            // the argument temps to the enclosing statement puts them HERE.
            let before_guard = &body[..guard_at];
            assert!(
                !before_guard.contains("= a();") && !before_guard.contains("= b();"),
                "{} lowering ran an argument temp unconditionally:\n{}",
                op,
                c
            );
        }
    }

    // A METHOD CALL'S RECEIVER IS THE FIRST ARGUMENT (N5-17 rewrites `r.f(a)`
    // to `Type::f(r, a)`), so under N13-03 it is read at its own position like
    // any other argument — before the arguments written after it, and exactly
    // once. This is the shape language-spec.md §A method calls now describes.
    #[test]
    fn test_an_effectful_method_receiver_is_read_first_and_once() {
        let c = generate(
            r#"
        static mut G: i64 = 0;
        struct Counter { n: i64 }
        impl Counter {
            fn plus(self, k: i64) -> i64 { return self.n + k; }
        }
        fn make() -> Counter { G = 5; return Counter { n: 1 }; }
        fn main() { print_int(make().plus(G)); }
        "#,
        )
        .unwrap();
        assert!(c.contains("struct Counter __pd_val0 = make();"), "{}", c);
        assert!(c.contains("long long __pd_val1 = G;"), "{}", c);
        assert!(
            c.contains("__pd_Counter_plus(__pd_val0, __pd_val1)"),
            "{}",
            c
        );
        assert_eq!(c.matches("__pd_val0 = make();").count(), 1, "{}", c);
        assert!(!c.contains("__pd_Counter_plus(make()"), "{}", c);
    }

    #[test]
    fn test_declared_mutable_array_parameters_may_be_written() {
        // `&mut [T; N]`, and `mut xs: [T; N]` - the spelling the bootstrap
        // subset mandates (bootstrap-subset.md:104) - both stay legal.
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

/// A tiny insertion-ordered map, because tuple shapes must be EMITTED in the
/// order they were registered and LOOKED UP by name.
///
/// A `HashMap` loses the order, a `BTreeMap` imposes an alphabetical one that
/// puts a nested shape before the shapes it is built from, and pulling in a
/// dependency for eleven lines would be the larger change. Order is definition
/// order: `((i64, i64), i64)` registers its inner shape while computing its own
/// element types, so the inner name is already present when the outer arrives.
mod indexmap_lite {
    #[derive(Default)]
    pub struct OrderedMap {
        entries: Vec<(String, Vec<String>)>,
    }

    impl OrderedMap {
        /// Record a shape under `name` if it is new.
        ///
        /// Answers `false` when the name is already taken by a DIFFERENT layout,
        /// which the caller turns into an error. Keeping the first silently is
        /// what the mangling's non-injectivity would otherwise cost: two shapes
        /// would share one struct and one of them would be emitted with the
        /// other's fields.
        pub fn insert(&mut self, name: String, element_types: Vec<String>) -> bool {
            match self.entries.iter().find(|(existing, _)| existing == &name) {
                Some((_, existing_types)) => existing_types == &element_types,
                None => {
                    self.entries.push((name, element_types));
                    true
                }
            }
        }

        pub fn element_types(&self, name: &str) -> Option<&[String]> {
            self.entries
                .iter()
                .find(|(existing, _)| existing == name)
                .map(|(_, types)| types.as_slice())
        }

        pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
            self.entries.iter().map(|(name, types)| (name, types))
        }

        pub fn is_empty(&self) -> bool {
            self.entries.is_empty()
        }
    }
}

#[cfg(test)]
mod tuple_shape_tests {
    use super::*;

    /// THE MANGLING IS NOT INJECTIVE, and this test says so in one line.
    ///
    /// `tuple_c_name` sanitises each element's C type and joins with `_`, so an
    /// underscore inside a type name is indistinguishable from the separator
    /// between two elements. It is why `register_tuple` refuses a second layout
    /// under a name it has already given out.
    ///
    /// SYNTHETIC ELEMENT TYPES, DELIBERATELY — and this is the part worth
    /// reading. No Palladium program known to this author can reach the refusal,
    /// because every element spelling `type_to_c` produces carries a delimiter no
    /// other spelling can fake: a named type becomes `struct X` (the space
    /// sanitises to `_`, so `A_B` beside `C` is `struct_A_B_struct_C` while `A`
    /// beside `B_C` is `struct_A_struct_B_C`), a primitive is one of a closed set
    /// (`long_long`, `const_char_p`, `int`, `double`), and a nested tuple's name
    /// begins `__pd_tupleN_`, whose arity digit fixes how many elements follow.
    /// Tried and refused for other reasons: type aliases (`type A_B = i64;` is
    /// not transparent to the type checker here).
    ///
    /// So the refusal is a guard against a FUTURE spelling, and the only way to
    /// test it today is to hand `register_tuple` the strings such a spelling
    /// would produce. A test that could not run is worse than one whose inputs
    /// are honest about being constructed.
    #[test]
    fn the_mangling_can_collide() {
        assert_eq!(
            CodeGenerator::tuple_c_name(&["A_B".to_string(), "C".to_string()]),
            CodeGenerator::tuple_c_name(&["A".to_string(), "B_C".to_string()]),
            "if these ever differ the mangling became injective — delete the refusal below \
             and say so, do not leave a check nobody can reach"
        );
    }

    /// And the collision is REFUSED rather than silently resolved in favour of
    /// whichever shape arrived first.
    #[test]
    fn a_colliding_layout_is_refused_by_name() {
        let mut codegen = CodeGenerator::new("collision_probe").expect("a generator");
        let first = codegen
            .register_tuple(&["A_B".to_string(), "C".to_string()])
            .expect("the first shape registers");
        let second = codegen.register_tuple(&["A".to_string(), "B_C".to_string()]);
        let err = second.expect_err(
            "the second shape mangles to the same name with different fields; accepting it \
             would emit one struct and use it for both",
        );
        let text = format!("{}", err);
        assert!(
            text.contains("mangle to the same C name") && text.contains(&first),
            "the refusal must name the collision and the name involved: {}",
            text
        );
    }

    /// Registering the SAME layout twice is not a collision — it is the ordinary
    /// case, and it must stay idempotent.
    #[test]
    fn the_same_layout_registers_twice() {
        let mut codegen = CodeGenerator::new("collision_probe").expect("a generator");
        let a = codegen
            .register_tuple(&["long long".to_string(), "const char*".to_string()])
            .expect("first");
        let b = codegen
            .register_tuple(&["long long".to_string(), "const char*".to_string()])
            .expect("second registration of the same shape is not a collision");
        assert_eq!(a, b);
    }
}
