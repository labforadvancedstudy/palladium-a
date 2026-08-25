// Borrow checker for Palladium
// "Ensuring memory safety through static analysis"

use crate::ast::{AssignTarget, Expr, Function, Item, Pattern, Program, Stmt, Type};
use crate::errors::{CompileError, Result};
use crate::ownership::{expr_to_place, Lifetime, OwnershipContext, Place, RefKind};
use std::collections::HashMap;

/// The borrow checker analyzes the program to ensure memory safety
pub struct BorrowChecker {
    /// Current ownership context
    context: OwnershipContext,
    /// Function signatures for ownership analysis
    functions: HashMap<String, FunctionSig>,
    /// Local variable types for Copy checking
    local_types: HashMap<String, Type>,
    /// Track if we're in an unsafe context
    unsafe_depth: usize,
    /// Field types of every struct in the program, so that the type of a
    /// projection like `v.data[0]` can be resolved when deciding Copy vs move.
    struct_fields: HashMap<String, Vec<(String, Type)>>,
    /// Which in-scope bindings were declared mutable (`let mut x`, `mut x: T`,
    /// or a `&mut T` parameter). Every binder in the grammar registers here -
    /// parameters, `let`, the `for` variable and pattern bindings - and a name
    /// that is *absent* is refused a mutable borrow rather than allowed one:
    /// this map is the invariant, so an unregistered binder must fail loudly
    /// instead of silently granting write permission.
    mutable_bindings: HashMap<String, bool>,
    /// Lifetime of the call expression whose arguments are being checked, if any.
    /// Every borrow created while evaluating those arguments — including the
    /// temporary `&x` / `&mut x` references written in argument position — gets
    /// this lifetime and is released when the call completes.
    call_lifetime: Option<Lifetime>,
    /// Modules the resolver loaded for this compilation, keyed by the name the
    /// importing program refers to them by (the alias, if there was one).
    ///
    /// This pass used to have no channel at all by which an imported signature
    /// could arrive: it was handed the *pre-resolution* AST, and `Program.imports`
    /// carries only paths, never the loaded module. So every call to an imported
    /// function died as "Use of uninitialized value", because the callee was not
    /// in `functions` and was then looked up as a variable. The type checker has
    /// had this channel since it was written (`TypeChecker::set_imported_modules`);
    /// this is the same channel, and the driver fills both from one resolver run.
    imported_modules: HashMap<String, crate::resolver::ModuleInfo>,
    /// For every generic name this compilation instantiates, WHERE the template
    /// codegen monomorphizes came from: `None` local, `Some(module)` imported.
    ///
    /// A SET OF NAMES IS NOT ENOUGH, and that was the second bug on this line.
    /// `TypeChecker::generic_functions` is keyed by bare name and is
    /// last-writer-wins, so "the name `pick` is instantiated" says nothing about
    /// WHICH `pick<T>` is the one emitted. Keyed on the name, this pass checked
    /// every same-named imported template and refused a build over a body
    /// nothing emits. Keyed on the origin, it checks the one template that
    /// becomes C.
    instantiated_generic_origins: HashMap<String, Option<String>>,
}

/// Function signature for ownership analysis
#[derive(Debug, Clone)]
struct FunctionSig {
    /// Parameter ownership requirements
    params: Vec<ParamOwnership>,
    /// Return value ownership
    #[allow(dead_code)]
    returns: ReturnOwnership,
    /// The DECLARED return type, when the callee declares one.
    ///
    /// `returns` above answers "who owns what comes back"; this answers "what
    /// comes back", and nothing had the second answer. Without it a receiver
    /// that is itself a call — `make().consume(x)`, `s.dup().take()` — typed as
    /// `i64` by default, no `Type::method` signature was found for it, and the
    /// method's parameters went unenforced: measured, `let a = make().consume(x);
    /// let b = x.v;` COMPILED with `x` moved.
    ret_ty: Option<Type>,
}

/// How a callee takes each parameter.
///
/// The borrowing modes carry no lifetime: the caller-side borrow always lasts
/// exactly for the call expression, so `check_call_args` supplies that lifetime.
#[derive(Debug, Clone)]
enum ParamOwnership {
    /// Parameter takes ownership (moves the value)
    Move,
    /// Parameter borrows immutably
    Borrow,
    /// Parameter borrows mutably
    BorrowMut,
    /// Parameter is Copy (no ownership transfer)
    Copy,
}

#[derive(Debug, Clone)]
enum ReturnOwnership {
    /// Returns owned value
    Owned,
    /// Returns borrowed value with lifetime
    #[allow(dead_code)]
    Borrowed(Lifetime),
    /// No return value
    Unit,
    /// Returns copy value (primitives)
    Copy,
}

impl FunctionSig {
    /// Build the ownership signature of a built-in from the canonical table.
    fn from_builtin(b: &crate::builtins::Builtin) -> Self {
        use crate::builtins::{ParamMode, ReturnMode};

        let params = b
            .params
            .iter()
            .map(|param| match param.mode {
                ParamMode::Copy => ParamOwnership::Copy,
                ParamMode::Borrow => ParamOwnership::Borrow,
                ParamMode::Move => ParamOwnership::Move,
            })
            .collect();

        let returns = match b.ret_mode {
            ReturnMode::Owned => ReturnOwnership::Owned,
            // Storage the built-in never allocated and that outlives the program —
            // `arg_at` handing back a pointer into `argv`. Borrowed for 'static is
            // the honest signature; Owned would tell this pass that the caller is
            // free to release memory belonging to the process.
            ReturnMode::BorrowedStatic => ReturnOwnership::Borrowed(Lifetime::Static),
            ReturnMode::Copy => ReturnOwnership::Copy,
            ReturnMode::Unit => ReturnOwnership::Unit,
        };

        // A built-in's return type is not modelled here: no built-in returns a
        // struct or an enum, so no built-in call can be the receiver of a
        // method. Left `None` deliberately rather than mapped through a second
        // spelling of the builtin type table.
        FunctionSig {
            params,
            returns,
            ret_ty: None,
        }
    }
}

impl Default for BorrowChecker {
    fn default() -> Self {
        // Register built-in functions from the single source of truth
        // (src/builtins.rs). Deriving them here — instead of keeping a
        // hand-written copy — is what keeps this pass in sync with the type
        // checker; a built-in known to only one pass used to type-check and
        // then die here with "Use of uninitialized value".
        let functions = crate::builtins::BUILTINS
            .iter()
            .map(|b| (b.name.to_string(), FunctionSig::from_builtin(b)))
            .collect();

        Self {
            context: OwnershipContext::new(),
            functions,
            local_types: HashMap::new(),
            mutable_bindings: HashMap::new(),
            unsafe_depth: 0,
            struct_fields: HashMap::new(),
            call_lifetime: None,
            imported_modules: HashMap::new(),
            instantiated_generic_origins: HashMap::new(),
        }
    }
}

impl BorrowChecker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Names of every function signature this pass knows.
    ///
    /// On a freshly constructed checker this is exactly the built-in set, which is
    /// what the drift test in `crate::builtins` asserts against. Returning *all*
    /// names (not just the ones that happen to be in the canonical table) is
    /// deliberate: a hand-added registration here must show up as a diff.
    #[allow(dead_code)] // used by the drift tests in crate::builtins
    pub(crate) fn registered_function_names(&self) -> std::collections::BTreeSet<String> {
        self.functions.keys().cloned().collect()
    }

    /// Hand this pass the modules the resolver loaded, so that a call to an
    /// imported function can be checked at all.
    ///
    /// Takes exactly what `TypeChecker::set_imported_modules` takes
    /// (`src/typeck/mod.rs:1378-1378`), because the driver has one resolver result and
    /// two passes that need it; a second shape here would be a second thing to
    /// keep in sync. Registration itself is deferred to `check_program`, which is
    /// where the ordering against local definitions is decided.
    pub fn set_imported_modules(&mut self, modules: HashMap<String, crate::resolver::ModuleInfo>) {
        self.imported_modules = modules;
    }

    /// Tell this pass which generic templates the compilation instantiates, and
    /// where each one came from.
    ///
    /// Takes `TypeChecker::get_instantiated_generic_origins`, which the driver
    /// computes between type checking and this pass. The ORIGIN is the load-bearing
    /// half: it is what distinguishes the imported `pick<T>` that gets
    /// monomorphized from the imported `pick<T>` that a local definition of the
    /// same name displaced. Passing names alone made those two indistinguishable
    /// and turned an error in the displaced body into a build failure.
    ///
    /// Not supplying it is safe in the only direction that matters: the map is then
    /// empty, generic imported bodies are skipped, and this pass is back to where it
    /// was rather than checking bodies against a map it does not have.
    pub fn set_instantiated_generic_origins(&mut self, origins: HashMap<String, Option<String>>) {
        self.instantiated_generic_origins = origins;
    }

    /// Register the public functions *and struct layouts* of every imported module.
    ///
    /// LAYOUTS ARE NOT OPTIONAL. Field Copy classification reads `struct_fields`
    /// (`place_type`, `is_expr_copy`), and an unresolvable projection falls into
    /// the conservative "not Copy" default. So a body walked without its struct
    /// layouts does not merely lose precision — it MOVES on every field read, and
    /// the second read of an `i64` field is reported as "Use of moved value". Once
    /// the third pass below started walking imported bodies, that turned the
    /// missing layouts into a FALSE REJECT of a valid program: byte-identical
    /// source compiled when the struct was declared locally and was refused when
    /// it came from a module. Over-approximating a refusal fails closed onto
    /// correct programs, which is the worse polarity of the two.
    ///
    /// Public-only, and imports-before-locals, for the same reasons as functions
    /// below; the type checker registers imported layouts under exactly the same
    /// filter (`src/typeck/mod.rs:1558-1559`), so the two passes agree on which
    /// `P` is meant.
    ///
    /// THE REMAINING WINDOW USED TO BE UNREACHABLE, AND IS NOW REACHABLE — the
    /// second belt has been removed on purpose, so the ordering above is load
    /// bearing on its own. This paragraph used to argue that if an imported
    /// layout ever leaked where a local should win, the program could not be
    /// built at all: code generation emitted every public imported struct and then
    /// every local struct with no shadowing check between them, so the C held two
    /// definitions of `P` and gcc said "redefinition of 'P'". That was measured
    /// and it was true.
    ///
    /// It stopped being true on 2026-08-23. The emission walk now asks
    /// `crate::ast::local_type_shadows_import` (`src/codegen/mod.rs:1811-1839`)
    /// and skips the imported definition, because the same window in the TYPE
    /// CHECKER was producing `Type mismatch: expected Color, found Color` for an
    /// ordinary program — a local `struct Color` over an imported `pub enum
    /// Color` — and closing it there without closing it here only moved the
    /// failure into gcc. Measured now: that program compiles and prints the local
    /// answer, where at `5fbd1fe` it was `gcc compilation failed`.
    ///
    /// So the ordering above no longer has a backstop, and the reason it is still
    /// correct is the one stated in SHADOWING below rather than anything the
    /// linker does.
    ///
    /// SHADOWING: a local definition wins, and it wins by *order* — this runs
    /// before the walk over `program.items`, so a local `fn helper` overwrites an
    /// imported `helper` in `functions`. That direction is the only one that can
    /// be right here: the local definition is the one whose body this pass will
    /// go on to check, and whose signature codegen will emit, so registering the
    /// import over it would make the borrow checker reason about a *different*
    /// function from the one that runs. It also matches what the type checker
    /// already does — it fills its table from imports in `set_imported_modules`
    /// and then overwrites from local items during `check` — so the two passes
    /// agree about which `helper` is meant.
    ///
    /// Modules are visited in sorted key order rather than `HashMap` order: when
    /// two modules export the same name, the winner must not depend on the hash
    /// seed. Which of the two wins is still arbitrary and is a real ambiguity the
    /// language does not yet diagnose (M3), but it is at least the same one twice.
    ///
    /// Both the bare name and `module::name` are registered, matching the type
    /// checker, so a qualified call is not rejected by a pass the unqualified one
    /// passes.
    fn register_imported_functions(&mut self) {
        let modules = std::mem::take(&mut self.imported_modules);
        let mut names: Vec<&String> = modules.keys().collect();
        names.sort();

        for module_name in names {
            let module_info = &modules[module_name];
            for item in &module_info.ast.items {
                match item {
                    Item::Struct(struct_def) => {
                        if !matches!(struct_def.visibility, crate::ast::Visibility::Public) {
                            continue;
                        }
                        // GENERIC LAYOUTS ARE REGISTERED TOO, and the guard that
                        // used to skip them was the same mistake as the one over
                        // function bodies, one item kind across.
                        //
                        // Its reason was that codegen emits only NON-generic
                        // imported structs (`src/codegen/mod.rs:1815-1820`), so a
                        // generic `P<T>` would be "a layout for a type this
                        // compilation never produces". Structs have a
                        // monomorphization path too
                        // (`generic_struct_instantiations`), so that was false in
                        // exactly the way the function version was — and once the
                        // walk below started checking instantiated imported bodies,
                        // the missing layout became a FALSE REJECT rather than a
                        // harmless omission: field Copy classification falls back to
                        // "not Copy" for a layout it cannot resolve, so the second
                        // read of an `i64` field is reported as a use of a moved
                        // value. Measured, the only difference being `<T>` on the
                        // struct:
                        //
                        //     pub struct P<T> { a: i64 }   -> Use of moved value: p.a
                        //     pub struct Q    { a: i64 }   -> compiles
                        //
                        // The local walk has never had this guard
                        // (`Item::Struct` below registers every local struct,
                        // generic or not), so registering here is what makes the two
                        // sides agree rather than a new rule. The collision the old
                        // comment worried about is handled by ORDER, as everywhere
                        // else here: imports first, locals overwrite, so a local
                        // `struct P` wins the name — which is the layout codegen
                        // emits.
                        self.struct_fields
                            .insert(struct_def.name.clone(), struct_def.fields.clone());
                    }
                    Item::Function(func) => {
                        // Private items of a module are not callable from outside it,
                        // so registering them would let this pass accept a program the
                        // type checker rejects — the two passes must refuse the same
                        // programs, not merely overlap.
                        if !matches!(func.visibility, crate::ast::Visibility::Public) {
                            continue;
                        }
                        self.collect_function_sig(func);
                        let qualified_name = format!("{}::{}", module_name, func.name);
                        self.collect_function_sig_with_name(func, &qualified_name);
                    }
                    _ => {}
                }
            }
        }

        self.imported_modules = modules;
    }

    /// Check if we're currently in an unsafe context
    #[allow(dead_code)]
    fn in_unsafe_context(&self) -> bool {
        self.unsafe_depth > 0
    }

    /// Check a program for ownership violations
    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        // Imported signatures first, so that the walk below - the local
        // definitions - overwrites them. See `register_imported_functions`.
        self.register_imported_functions();

        // First pass: collect function signatures and struct layouts
        for item in &program.items {
            match item {
                Item::Struct(struct_def) => {
                    self.struct_fields
                        .insert(struct_def.name.clone(), struct_def.fields.clone());
                }
                Item::Function(func) => {
                    self.collect_function_sig(func);
                }
                Item::Impl(impl_block) => {
                    // Collect method signatures from impl blocks
                    for method in &impl_block.methods {
                        // Create qualified method name
                        let qualified_name = format!("{}::{}", impl_block.for_type, method.name);
                        self.collect_function_sig_with_name(method, &qualified_name);
                        // `fn dup(self) -> Self` returns the impl's type, and
                        // `Self` is a name no `impl` block is registered under.
                        // Resolved through the one substitution point the type
                        // checker and code generation also call, so a third
                        // reading of `Self` does not appear here.
                        if let Some(sig) = self.functions.get_mut(&qualified_name) {
                            sig.ret_ty = sig
                                .ret_ty
                                .as_ref()
                                .map(|ty| crate::ast::substitute_self(ty, &impl_block.for_type));
                        }
                    }
                }
                _ => {}
            }
        }

        // Second pass: check function bodies
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.check_function(func)?;
                }
                Item::Impl(impl_block) => {
                    // Check method bodies from impl blocks
                    for method in &impl_block.methods {
                        self.check_function(method)?;
                    }
                }
                _ => {}
            }
        }

        // Third pass: check the bodies of the imported modules.
        //
        // WITHOUT THIS, REGISTERING THE SIGNATURES IS A SAFETY HOLE RATHER THAN A
        // FEATURE. `register_imported_functions` makes a call to an imported
        // function pass this pass; the walk above visits `program.items` only, so
        // the body behind that signature was never visited by anything. The
        // compiler would then be ACCEPTING code it has never checked, and
        // `pub fn bad(mut x: i64) { x = 42; }` in a module compiled, linked and
        // printed 42 while the byte-identical program written locally was refused
        // with "cannot borrow `n` as mutable". A pass that refuses a program in one
        // file and accepts it in two is not checking the language.
        //
        // ONLY `Item::Function` IS WALKED, and an imported `impl` method is not a
        // gap in that. Codegen's imported walk matches `Item::Struct` and
        // `Item::Enum` (`src/codegen/mod.rs:1811-1839`) and, separately,
        // `Item::Function` (`src/codegen/mod.rs:1953-1954`) — there is no `Item::Impl`
        // arm anywhere in it. So an imported impl method is not merely uncallable:
        // IT DOES NOT EXIST IN THE OUTPUT. Measured — a module exporting
        // `pub struct P { a: i64 }` with `impl P { fn get(self) -> i64 { … } }`
        // compiles, and the emitted C contains no trace of `get`. Checking a body
        // that produces no code would be checking something that cannot run, and
        // the fail-open this pass exists to close is specifically "accepted code
        // that DOES run unchecked". That argument holds whatever the parser later
        // does with method-call syntax, which a runtime measurement of a call would
        // not.
        //
        // Private items are skipped for the same reason they are skipped in
        // registration: they are not callable from here, so their signatures are
        // not in `functions` and checking their bodies would report names this
        // program cannot reach. That leaves a real gap — a module has no private
        // scope, so a PUBLIC imported function that calls a private sibling is
        // reported as an undefined name — and it is declared as such in
        // `tests/m3_imported_calls.rs`
        // (`test_a_module_can_use_its_own_private_items`), not papered over.
        //
        // Shadowing: `functions` at this point holds the LOCAL definitions, because
        // the first pass overwrote the imported ones. So an imported body that calls
        // a name the importing program also defines is checked against the local
        // signature. That is wrong, and it is the same unresolved ambiguity that
        // `test_a_local_definition_shadows_an_imported_one` declares; it is recorded
        // here rather than silently fixed one pass at a time.
        let modules = std::mem::take(&mut self.imported_modules);
        let mut module_names: Vec<&String> = modules.keys().collect();
        module_names.sort();
        for module_name in module_names {
            for item in &modules[module_name].ast.items {
                if let Item::Function(func) = item {
                    if !matches!(func.visibility, crate::ast::Visibility::Public) {
                        continue;
                    }
                    // A GENERIC BODY IS CHECKED EXACTLY WHEN IT IS INSTANTIATED,
                    // because that is exactly when codegen emits one.
                    //
                    // THE PREVIOUS VERSION OF THIS GUARD SKIPPED EVERY GENERIC BODY
                    // AND WAS A FAIL-OPEN. Its stated reason was that a skipped body
                    // "produces no C, because the codegen guard is the same
                    // predicate". That is true of the DIRECT imported-emission path
                    // (`src/codegen/mod.rs:1954-1957`, public and non-generic) and
                    // false of MONOMORPHIZATION, which is a different path and emits
                    // `name__T` from the same template. Measured on the guard:
                    //
                    //     lib: pub fn bad<T>(x: T) -> i64 {
                    //              let a: S = S{v:7}; let b: S = a; let c: S = a;
                    //              return c.v; }
                    //     main: import lib; fn main() { print_int(bad(1)); }
                    //
                    // compiled, emitted `bad__i64`, linked and PRINTED 7 — a plain
                    // use-after-move that runs. The byte-identical LOCAL generic is
                    // refused, by this same pass, which is the "refuses it in one
                    // file, accepts it in two" defect this third pass exists to
                    // close, reopened for generics.
                    //
                    // Reading a guarantee off the stated reason instead of off the
                    // mechanism is the recurrence family here: ONE COMPILER PASS
                    // SKIPS CODE ANOTHER PASS EMITS. So the predicate is now the
                    // emission set itself — `instantiated_generics` comes from
                    // `TypeChecker::get_instantiations`, which the driver already
                    // computes before this pass runs and which is the same list
                    // codegen monomorphizes from.
                    //
                    // The uninstantiated case still has to be skipped, and that is
                    // not this defect wearing a hat: imported ASTs are never
                    // macro-expanded (the driver expands the top-level AST before it
                    // resolves modules), so walking `pub fn gen<T>(..) { vec!(7) }`
                    // that nothing calls turned a compilation `main` COMPLETES into
                    // "Unexpected macro invocation in borrow checking". Nothing emits
                    // that body either, so skipping it is the emission rule, not an
                    // exception to it. When the same body IS instantiated the
                    // compilation already fails in codegen with the macro error, so
                    // checking it here moves the phase and not the verdict. The
                    // underlying asymmetry is declared in
                    // `tests/m3_imported_calls.rs`
                    // (`test_a_macro_in_an_imported_body_is_never_expanded`); the
                    // honest fix is expanding module ASTs.
                    // The ORIGIN, not just the name. `generic_functions` in the
                    // type checker is keyed by bare name and last-writer-wins,
                    // with locals walked after imports, so a local `pick<T>`
                    // DISPLACES an imported one and codegen monomorphizes the
                    // local. Testing only "is the name `pick` instantiated" made
                    // this pass check the displaced import too. Measured:
                    //
                    //     lib.pd:  pub fn pick<T>(x: T) -> i64 { ...use-after-move... }
                    //     main.pd: import lib;
                    //              fn pick<T>(x: T) -> i64 { return 3; }
                    //              fn main() { print_int(pick(1)); }
                    //
                    //     -> error: Use of moved value: a
                    //
                    // over a body the emitted C contains no trace of; renaming the
                    // imported function, changing nothing else, compiled and ran.
                    // The same shape with no local definition at all is two modules
                    // exporting the name, where the loser is displaced identically.
                    if !func.type_params.is_empty()
                        && self.instantiated_generic_origins.get(&func.name)
                            != Some(&Some(module_name.clone()))
                    {
                        continue;
                    }
                    self.check_function(func)?;
                }
            }
        }
        self.imported_modules = modules;

        Ok(())
    }

    /// Collect function signature for ownership analysis
    fn collect_function_sig(&mut self, func: &Function) {
        self.collect_function_sig_with_name(func, &func.name);
    }

    /// Collect function signature with a custom name
    fn collect_function_sig_with_name(&mut self, func: &Function, name: &str) {
        let mut params = Vec::new();

        for param in &func.params {
            // `mut` is decided before the type, because codegen emits *every*
            // mutable parameter as a pointer to the caller's storage
            // (src/codegen/mod.rs, "Pass by pointer for mutable parameters"),
            // whatever the type is. Classifying `mut x: i64` or
            // `mut s: String` as Copy - which the type-first order did, via the
            // `String` and primitive arms below - meant the write permission
            // was never checked for them: `fn bump(mut x: i64) { x = 42; }`
            // called with an immutable `let n = 1;` compiled and printed 42.
            if param.mutable {
                params.push(ParamOwnership::BorrowMut);
                continue;
            }
            let ownership = match &param.ty {
                Type::Array(_, _) => {
                    // An array parameter is a *reference*, not a move. The C
                    // backend emits `T name[N]`, which decays to a pointer, so
                    // the callee writes into the caller's storage and the caller
                    // keeps using the array afterwards — see the parameter
                    // emission in src/codegen/mod.rs ("In C, array parameters
                    // are passed as pointers"). Codegen is the ground truth
                    // here, so this pass models it as a borrow either way and
                    // only the mutability of the binding picks the kind.
                    ParamOwnership::Borrow
                }
                // `String` is Copy (see `is_copy_type`): it lowers to
                // `const char*` and nothing frees it per value, so passing one
                // neither transfers nor invalidates anything.
                Type::String => ParamOwnership::Copy,
                // Non-copy types
                Type::Custom(_) => ParamOwnership::Move,
                Type::Reference { mutable, .. } => {
                    if *mutable {
                        ParamOwnership::BorrowMut
                    } else {
                        ParamOwnership::Borrow
                    }
                }
                _ => ParamOwnership::Copy, // Primitives are Copy
            };
            params.push(ownership);
        }

        let returns = match &func.return_type {
            Some(Type::Reference { .. }) => {
                ReturnOwnership::Borrowed(Lifetime::Named("fn".to_string()))
            }
            Some(_) => ReturnOwnership::Owned,
            None => ReturnOwnership::Unit,
        };

        self.functions.insert(
            name.to_string(),
            FunctionSig {
                params,
                returns,
                ret_ty: func.return_type.clone(),
            },
        );
    }

    /// Check a function for ownership violations.
    ///
    /// The function scope opened here is what makes the per-function state
    /// per-function. `local_types` and `mutable_bindings` are cleared outright
    /// just below; `self.context` cannot be, because it carries the counters
    /// that keep temporaries and lifetimes distinct, so it is unwound by
    /// `exit_scope` instead — which is only truthful now that `exit_scope`
    /// actually retires bindings and borrows rather than dropping a lifetime
    /// variant nothing constructs.
    fn check_function(&mut self, func: &Function) -> Result<()> {
        self.context.enter_scope();
        self.local_types.clear();
        self.mutable_bindings.clear();

        // Initialize parameters and their types
        for param in &func.params {
            let place = Place::Local(param.name.clone());
            self.context.declare(&place);
            self.context.init_owned(place);
            self.local_types
                .insert(param.name.clone(), param.ty.clone());
            // `mut x: T` and `x: &mut T` are the two ways a parameter arrives
            // writable; anything else may not be mutably re-borrowed.
            let writable =
                param.mutable || matches!(&param.ty, Type::Reference { mutable: true, .. });
            self.mutable_bindings.insert(param.name.clone(), writable);
        }

        // Check function body
        for stmt in &func.body {
            self.check_stmt(stmt)?;
        }

        self.context.exit_scope();
        Ok(())
    }

    /// Check a statement for ownership violations
    fn check_stmt(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Let {
                name,
                value,
                ty,
                mutable,
                ..
            } => {
                // Check the value expression
                self.check_expr(value)?;

                // Remember whether this binding may be mutated, so that
                // `&mut name` can be rejected when it may not.
                self.mutable_bindings.insert(name.clone(), *mutable);

                // Store the type if provided
                if let Some(ty) = ty {
                    self.local_types.insert(name.clone(), ty.clone());
                } else {
                    // Infer type from expression
                    let inferred_type = self.expr_type(value);
                    self.local_types.insert(name.clone(), inferred_type);
                }

                // Initialize the new variable
                let place = Place::Local(name.clone());

                // THE ORDER OF THESE THREE STEPS IS THE WHOLE POINT, because a
                // `Place` is a NAME and `let s: S = s;` gives the source and the
                // destination the same one.
                //
                // It used to be declare -> move_value, and `move_value` writes
                // `Moved` to the source and then `Owned` to the destination. With
                // one key those are the same slot, so the second write cancelled
                // the first and the move never happened; `declare` had already
                // snapshotted the outer `Owned`, so scope exit restored it and an
                // outer binding survived being moved out of. Measured: the
                // shadowing form was ACCEPTED and the identical program using a
                // different inner name was refused, the name being the only
                // difference between them.
                //
                //   1. move out of the SOURCE, naming no destination. This is also
                //      where an already-moved or borrowed source is refused, so it
                //      must run before anything writes to the name.
                //   2. record what this binder shadows — now the POST-move state,
                //      so scope exit restores `Moved` rather than resurrecting the
                //      outer value.
                //   3. give the new binding its own ownership.
                //
                // A Copy source skips step 1 and keeps both bindings usable, which
                // is what Copy means.
                if let Some(from_place) = expr_to_place(value) {
                    if !self.is_expr_copy(value) {
                        self.context.move_out_of(from_place, value.span())?;
                    }
                }
                self.context.declare(&place);
                self.context.init_owned(place);
            }

            Stmt::Assign {
                target,
                value,
                span,
            } => {
                // Check the value expression
                self.check_expr(value)?;

                // Get target place
                let target_place = match target {
                    AssignTarget::Ident(name) => Place::Local(name.clone()),
                    AssignTarget::Index { array, index } => {
                        self.check_expr(array)?;
                        self.check_expr(index)?;
                        if let Some(base) = expr_to_place(array) {
                            Place::Index {
                                base: Box::new(base),
                                index: "dynamic".to_string(),
                            }
                        } else {
                            return Err(CompileError::BorrowChecker {
                                message: "Cannot assign to temporary value".to_string(),
                                span: Some(*span),
                            });
                        }
                    }
                    AssignTarget::FieldAccess { object, field } => {
                        self.check_expr(object)?;
                        if let Some(base) = expr_to_place(object) {
                            Place::Field {
                                base: Box::new(base),
                                field: field.clone(),
                            }
                        } else {
                            return Err(CompileError::BorrowChecker {
                                message: "Cannot assign to temporary value".to_string(),
                                span: Some(*span),
                            });
                        }
                    }
                    AssignTarget::Deref { expr } => {
                        self.check_expr(expr)?;
                        // For dereference assignment, we need the place that the reference points to
                        if let Some(place) = expr_to_place(expr) {
                            // The dereferenced place is what we're assigning to
                            place
                        } else {
                            return Err(CompileError::BorrowChecker {
                                message: "Cannot dereference temporary value".to_string(),
                                span: Some(*span),
                            });
                        }
                    }
                };

                // Check if assignment is allowed
                if let Some(from_place) = expr_to_place(value) {
                    if !self.is_expr_copy(value) {
                        // Move ownership
                        self.context.move_value(from_place, target_place, *span)?;
                    }
                }
            }

            Stmt::Expr(expr) => {
                self.check_expr(expr)?;
            }

            Stmt::Return(Some(expr)) => {
                self.check_expr(expr)?;
                // TODO: Check return value ownership matches function signature
            }

            Stmt::Return(None) => {}

            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.check_expr(condition)?;

                self.context.enter_scope();
                self.check_block_stmts(then_branch)?;
                self.context.exit_scope();

                if let Some(else_stmts) = else_branch {
                    self.context.enter_scope();
                    self.check_block_stmts(else_stmts)?;
                    self.context.exit_scope();
                }
            }

            Stmt::While {
                condition, body, ..
            } => {
                self.check_expr(condition)?;

                self.context.enter_scope();
                self.check_block_stmts(body)?;
                self.context.exit_scope();
            }

            Stmt::Loop { body, .. } => {
                self.context.enter_scope();
                self.check_block_stmts(body)?;
                self.context.exit_scope();
            }

            Stmt::For {
                var, iter, body, ..
            } => {
                self.check_expr(iter)?;

                // The scope opens before the binder is registered, so the
                // loop variable cannot outlive the loop.
                let loop_scope = self.open_mutability_scope();
                self.context.enter_scope();
                // Initialize loop variable. `for x in xs` binds `x`
                // immutably - there is no `for mut x` in the grammar - and the
                // binding has to be recorded, because an unregistered name is
                // now a refusal, not a free pass.
                let place = Place::Local(var.clone());
                self.context.declare(&place);
                self.context.init_owned(place);
                self.mutable_bindings.insert(var.clone(), false);

                self.check_block_stmts(body)?;
                self.context.exit_scope();
                self.close_mutability_scope(loop_scope);
            }

            Stmt::Match { expr, arms, .. } => {
                self.check_expr(expr)?;

                for arm in arms {
                    // Opened before the pattern binds anything, so the arm's
                    // bindings do not survive the arm.
                    let arm_scope = self.open_mutability_scope();
                    self.context.enter_scope();

                    // Bind pattern variables
                    self.bind_pattern(&arm.pattern)?;

                    self.check_block_stmts(&arm.body)?;

                    self.context.exit_scope();
                    self.close_mutability_scope(arm_scope);
                }
            }

            // The value a `break` carries is an ordinary expression and can
            // move out of a binding like any other.
            Stmt::Break { value, .. } => {
                if let Some(expr) = value {
                    self.check_expr(expr)?;
                }
            }

            Stmt::Continue { .. } => {}

            Stmt::Unsafe { body, .. } => {
                // In unsafe blocks, we still perform ownership checks
                // but allow certain operations that would normally be forbidden
                self.unsafe_depth += 1;
                self.context.enter_scope();
                self.check_block_stmts(body)?;
                self.context.exit_scope();
                self.unsafe_depth -= 1;
            }
        }

        Ok(())
    }

    /// Check an expression for ownership violations
    fn check_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Ident(name) => {
                let place = Place::Local(name.clone());
                let ownership = self.context.get_ownership(&place).cloned();

                // A BINDING IN SCOPE WINS OVER A FUNCTION OF THE SAME NAME.
                //
                // This used to ask `functions` first and return early on a hit, so
                // a local whose name collided with any registered function skipped
                // the move and initialization checks entirely. Measured, in return
                // position, with `struct S { v: i64 }`:
                //
                //     let helper: S = S { v: 1 };  let b = helper;  return helper;
                //
                // is refused with "Use of moved value: helper" on its own, and
                // ACCEPTED — compiled, linked, printed 1 — as soon as a
                // `fn helper()` exists for the name to collide with. The hole was
                // already there for a local `fn helper`; registering imported
                // signatures widened it to every name any imported module exports,
                // which is a much larger surface for a program to collide with by
                // accident. The two controls are
                // `test_a_local_binding_is_not_laundered_by_a_local_function` and
                // `..._by_an_imported_function` in tests/m3_imported_calls.rs.
                //
                // The order is decided rather than merely swapped: a name that IS a
                // place in this scope is a variable, whatever else shares its
                // spelling, so the function table is consulted only for a name the
                // ownership context does not know. `let`-RHS and call-argument
                // positions never reached here — they have their own move handling —
                // which is why this survived four constructions before one in return
                // position found it.
                if ownership.is_none() && self.functions.contains_key(name) {
                    // It's a function - no ownership check needed
                    return Ok(());
                }

                // Check if the value is initialized and not moved
                match ownership.as_ref() {
                    Some(crate::ownership::Ownership::Owned) => {
                        // Value is accessible
                    }
                    Some(crate::ownership::Ownership::Borrowed { .. }) => {
                        // Value is borrowed but still accessible
                    }
                    Some(crate::ownership::Ownership::BorrowedMut { .. }) => {
                        // Value is mutably borrowed but still accessible
                    }
                    Some(crate::ownership::Ownership::Moved) => {
                        return Err(CompileError::UseOfMovedValue {
                            name: name.clone(),
                            span: Some(expr.span()),
                        });
                    }
                    None => {
                        return Err(CompileError::UseOfUninitializedValue {
                            name: name.clone(),
                            span: Some(expr.span()),
                        });
                    }
                }
            }

            Expr::Call { func, args, span } => {
                // Check function expression — EXCEPT a method callee, whose
                // receiver `check_call_args` checks itself as argument 0.
                // Checking it here as well visits the receiver TWICE, which is
                // harmless for a name and wrong for anything that moves:
                // measured, `a.dup().take()` reported "Use of moved value: a"
                // for a program that uses `a` exactly once, because the inner
                // call ran its own argument moves on both visits.
                if !matches!(func.as_ref(), Expr::FieldAccess { .. }) {
                    self.check_expr(func)?;
                }

                // A borrow taken to pass an argument lasts exactly as long as
                // the call expression. Give this call its own lifetime, tag
                // every borrow created while checking its arguments with it,
                // and end them all once the call is done — otherwise the value
                // stays borrowed forever and the next use of it is rejected.
                let call_lifetime = self.context.new_lifetime();
                let outer_lifetime = self.call_lifetime.replace(call_lifetime.clone());

                let result = self.check_call_args(func, args, &call_lifetime, *span);

                self.call_lifetime = outer_lifetime;
                self.context.end_borrows(&call_lifetime);
                result?;
            }

            Expr::Binary { left, right, .. } => {
                self.check_expr(left)?;
                self.check_expr(right)?;
            }

            Expr::Unary { operand, .. } => {
                self.check_expr(operand)?;
            }

            Expr::ArrayLiteral { elements, .. } => {
                for elem in elements {
                    self.check_expr(elem)?;
                }
            }

            Expr::ArrayRepeat { value, count, .. } => {
                self.check_expr(value)?;
                self.check_expr(count)?;
            }

            Expr::Index { array, index, .. } => {
                self.check_expr(array)?;
                self.check_expr(index)?;
            }

            Expr::StructLiteral { fields, .. } => {
                for (_, expr) in fields {
                    self.check_expr(expr)?;
                }
            }

            Expr::FieldAccess { object, .. } => {
                self.check_expr(object)?;
            }

            Expr::EnumConstructor {
                enum_name,
                variant,
                data,
                span,
            } => {
                // `Type::method(receiver, …)` WEARS THIS NODE, because the
                // parser builds every `A::b(...)` as an enum constructor and has
                // no types to tell the two apart. It therefore never reached
                // `check_call_args`, and the ownership rules of a method called
                // this way were not applied AT ALL — measured: `S::take(s);
                // S::take(s);` was accepted with `s` annotated or not, while
                // `s.take()` twice and the free-function spelling were both
                // refused.
                //
                // Keyed on a POSITIVE hit in the signature table, which is the
                // same rule code generation uses to decide the same question.
                let qualified = format!("{}::{}", enum_name, variant);
                if self.functions.contains_key(&qualified) {
                    if let Some(crate::ast::EnumConstructorData::Tuple(exprs)) = data {
                        let call_lifetime = self.context.new_lifetime();
                        return self.check_call_args(
                            &Expr::Ident(qualified),
                            exprs,
                            &call_lifetime,
                            *span,
                        );
                    }
                }
                match data {
                    Some(crate::ast::EnumConstructorData::Tuple(exprs)) => {
                        for expr in exprs {
                            self.check_expr(expr)?;
                        }
                    }
                    Some(crate::ast::EnumConstructorData::Struct(fields)) => {
                        for (_, expr) in fields {
                            self.check_expr(expr)?;
                        }
                    }
                    None => {}
                }
            }

            Expr::Range { start, end, .. } => {
                self.check_expr(start)?;
                self.check_expr(end)?;
            }

            Expr::Reference {
                mutable,
                expr,
                span,
            } => {
                // Taking a reference to an expression
                self.check_expr(expr)?;

                // If we can get a place for the expression, create a borrow
                if let Some(place) = expr_to_place(expr) {
                    // A mutable reference is the permission to write, so the
                    // binding underneath has to be one that may be written.
                    // This was unchecked: `let v = [1, 2, 3]; set(&mut v);`
                    // compiled, ran, and left 42 in an immutable binding.
                    if *mutable {
                        self.check_mutable_borrow_allowed(&place, *span)?;
                    }

                    // In argument position (`f(&mut v)`) the reference is a
                    // temporary that dies with the call, so it joins that
                    // call's lifetime and is released with it. Anywhere else
                    // (`let r = &mut v;`) the reference outlives the expression
                    // and keeps the conservative never-released lifetime.
                    let lifetime = match &self.call_lifetime {
                        Some(call_lifetime) => call_lifetime.clone(),
                        None => self.context.new_lifetime(),
                    };
                    let kind = if *mutable {
                        RefKind::Mutable
                    } else {
                        RefKind::Shared
                    };
                    self.context.borrow(place, kind, lifetime, *span)?;
                } else {
                    // Can't take reference to temporary
                    return Err(CompileError::BorrowChecker {
                        message: "Cannot take reference to temporary value".to_string(),
                        span: Some(*span),
                    });
                }
            }

            Expr::Deref { expr, .. } => {
                // Dereferencing an expression
                self.check_expr(expr)?;
                // TODO: Check that the expression is actually a reference type
            }

            Expr::Question { expr, .. } => {
                // Question operator checks the inner expression
                self.check_expr(expr)?;
                // TODO: Handle ownership implications of early return
            }

            // Literals don't need ownership checking
            Expr::String(_) | Expr::Integer(_) | Expr::Float(_) | Expr::Char(_) | Expr::Bool(_) => {
            }
            Expr::MacroInvocation { .. } => {
                // Macros should have been expanded before borrow checking
                return Err(CompileError::Generic(
                    "Unexpected macro invocation in borrow checking - macros should be expanded before this phase".to_string()
                ));
            }
            Expr::Await { expr, .. } => {
                self.check_expr(expr)?;
            }
            // A branch is a SCOPE: a local bound inside it dies at its `}`,
            // exactly like the `unsafe` block above. Checking the statements
            // without entering a scope would let a branch-local binding
            // outlive the branch and answer for an outer name of the same
            // spelling.
            Expr::If {
                condition,
                then_branch,
                then_value,
                else_branch,
                else_value,
                ..
            } => {
                self.check_expr(condition)?;
                self.check_value_block(then_branch, then_value.as_deref())?;
                if let Some(stmts) = else_branch {
                    self.check_value_block(stmts, else_value.as_deref())?;
                }
            }
            Expr::Block { stmts, value, .. } => {
                self.check_value_block(stmts, value.as_deref())?;
            }
            Expr::Cast { expr, .. } => {
                self.check_expr(expr)?;
            }
            Expr::Loop { body, .. } => {
                self.check_value_block(body, None)?;
            }
            Expr::Match { expr, arms, .. } => {
                self.check_expr(expr)?;
                for arm in arms {
                    // Same shape as `Stmt::Match` above, including
                    // `bind_pattern`: without it a payload binding is a name
                    // this pass has never seen, and `Payload::Num(n) => n * 10`
                    // is refused as "Use of uninitialized value: n" — measured.
                    let arm_scope = self.open_mutability_scope();
                    self.context.enter_scope();

                    let checked = (|| -> Result<()> {
                        self.bind_pattern(&arm.pattern)?;
                        self.check_block_stmts(&arm.body)?;
                        if let Some(value) = &arm.value {
                            self.check_expr(value)?;
                        }
                        Ok(())
                    })();

                    self.context.exit_scope();
                    self.close_mutability_scope(arm_scope);
                    checked?;
                }
            }
        }

        Ok(())
    }

    /// Check `{ stmts...; value }` in value position, in its own scope.
    fn check_value_block(&mut self, stmts: &[Stmt], value: Option<&Expr>) -> Result<()> {
        self.context.enter_scope();
        let result = (|| {
            self.check_block_stmts(stmts)?;
            match value {
                Some(expr) => self.check_expr(expr),
                None => Ok(()),
            }
        })();
        self.context.exit_scope();
        result
    }

    /// Check the statements of a nested block, with their own mutability scope.
    ///
    /// `mutable_bindings` is the record of what may be written, and a flat
    /// function-wide map leaks in both directions: an inner `let mut x` leaves
    /// an outer immutable `x` marked writable for the rest of the function, and
    /// an inner immutable `x` makes a legitimate outer `&mut x` be rejected.
    /// This is the same defect that was fixed for codegen's array bindings -
    /// and it was reintroduced here by the fix for that one.
    fn check_block_stmts(&mut self, stmts: &[Stmt]) -> Result<()> {
        let outer = self.open_mutability_scope();
        for stmt in stmts {
            self.check_stmt(stmt)?;
        }
        self.close_mutability_scope(outer);
        Ok(())
    }

    /// Decide a `mut`-parameter argument that `expr_to_place` cannot model.
    ///
    /// `expr_to_place` returns `None` both for genuine rvalues and for real
    /// storage it simply cannot describe, and the two need opposite answers:
    ///
    /// * `bump(1)`, `retitle(make())`, `bump(a + 1)` have no storage at all.
    ///   Codegen takes their address anyway - it emitted `bump(&1)` - and gcc
    ///   rejected the compiler's own output with "cannot take the address of an
    ///   rvalue". Refused here instead, in the language, because the
    ///   alternative is inventing semantics for a write nobody can observe.
    /// * `bump(xs[i])` with a non-literal index *is* caller storage, and its C
    ///   (`bump(&xs[i])`) is correct. It must not be refused - but it was also
    ///   never checked, so `let xs = [1, 2, 3]; bump(xs[i]);` wrote 9 into an
    ///   immutable binding. The root binding decides, exactly as it does for a
    ///   place this pass can model.
    ///
    /// `&mut x` arguments are not judged here: `check_expr` has already run the
    /// same permission check on them through the `Expr::Reference` arm.
    fn check_unmodellable_mutable_argument(
        &self,
        arg: &Expr,
        span: crate::errors::Span,
    ) -> Result<()> {
        if matches!(arg, Expr::Reference { .. }) {
            return Ok(());
        }
        match Self::place_root_ident(arg) {
            Some(root) => self.check_mutable_borrow_allowed(&Place::Local(root.to_string()), span),
            None => Err(CompileError::BorrowChecker {
                message: format!(
                    "cannot pass this {} to a `mut` parameter: a `mut` parameter receives a \
                     pointer to the caller's storage (language-spec.md §9.2), and this \
                     argument has no storage to point at - the write would have nowhere to \
                     land. Bind it to a variable first and pass that.",
                    Self::expr_kind(arg)
                ),
                span: Some(span),
            }),
        }
    }

    /// The variable at the base of a place expression, if the expression
    /// denotes storage at all. Unlike `expr_to_place` this does not need to
    /// model the projection, only to find what it is rooted in.
    fn place_root_ident(expr: &Expr) -> Option<&str> {
        match expr {
            Expr::Ident(name) => Some(name),
            Expr::Index { array, .. } => Self::place_root_ident(array),
            Expr::FieldAccess { object, .. } => Self::place_root_ident(object),
            Expr::Deref { expr, .. } => Self::place_root_ident(expr),
            _ => None,
        }
    }

    /// How to name an expression in a diagnostic.
    fn expr_kind(expr: &Expr) -> &'static str {
        match expr {
            Expr::Integer(_) => "integer literal",
            Expr::Float(_) => "float literal",
            Expr::Char(_) => "char literal",
            Expr::String(_) => "string literal",
            Expr::Bool(_) => "boolean literal",
            Expr::Call { .. } => "call result",
            Expr::Binary { .. } => "computed value",
            Expr::Unary { .. } => "computed value",
            Expr::ArrayLiteral { .. } | Expr::ArrayRepeat { .. } => "array literal",
            Expr::StructLiteral { .. } => "struct literal",
            Expr::EnumConstructor { .. } => "enum value",
            _ => "temporary value",
        }
    }

    /// Snapshot the mutability record a scope may shadow.
    ///
    /// Take this **before the scope's first write**. A `for` variable and a
    /// match binding are registered *before* the block body is checked, so
    /// snapshotting inside `check_block_stmts` captured the already-overwritten
    /// map: the binder outlived its own scope, and an outer `let mut v` stayed
    /// marked immutable after `for v in xs { }`, rejecting a later `&mut v`.
    fn open_mutability_scope(&self) -> HashMap<String, bool> {
        self.mutable_bindings.clone()
    }

    fn close_mutability_scope(&mut self, saved: HashMap<String, bool>) {
        self.mutable_bindings = saved;
    }

    /// Reject `&mut place` when the binding underneath was not declared mutable.
    ///
    /// An unregistered name is **refused**, not permitted. While the map held
    /// only `let` bindings, treating unknown as allowed was a reasonable
    /// default; now that the map is the invariant, that default is a hole that
    /// grows every time a binder is added and forgotten - `for` variables and
    /// match bindings were exactly that, and passed straight through. Every
    /// binder in the grammar registers here: parameters, `let`, the `for`
    /// variable, and pattern bindings. Anything else that reaches this point
    /// is a binder nobody taught the invariant about, and failing loudly is
    /// the whole point of the rule.
    fn check_mutable_borrow_allowed(&self, place: &Place, span: crate::errors::Span) -> Result<()> {
        let mut root = place;
        loop {
            match root {
                Place::Local(name) => {
                    if self.mutable_bindings.get(name) != Some(&true) {
                        return Err(CompileError::BorrowChecker {
                            message: format!(
                                "cannot borrow `{}` as mutable: it is not declared mutable. \
                                 Declare it `let mut {}` (or take the parameter as \
                                 `&mut`) if it is meant to be modified.",
                                name, name
                            ),
                            span: Some(span),
                        });
                    }
                    return Ok(());
                }
                Place::Field { base, .. } | Place::Index { base, .. } => root = base,
                Place::Temp(_) => return Ok(()),
            }
        }
    }

    /// Check a call's arguments and apply the callee's ownership requirements.
    ///
    /// `call_lifetime` is the lifetime of the enclosing call expression; every
    /// borrow taken here uses it so that `end_borrows` can release the whole set
    /// when the call returns. The lifetime recorded in the signature is only the
    /// *mode* marker — the caller-side borrow lives for the call, not forever.
    fn check_call_args(
        &mut self,
        func: &Expr,
        args: &[Expr],
        call_lifetime: &Lifetime,
        span: crate::errors::Span,
    ) -> Result<()> {
        // A METHOD CALL HAS A SIGNATURE TOO, and this used to consult one only
        // for a bare identifier. `impl` methods are registered under
        // `Type::method` in `check_program`'s first pass, so the signature was
        // there the whole time and nothing looked it up: `s.take(); s.take();`
        // was ACCEPTED where the identical free-function spelling was refused
        // as a use after move.
        //
        // The receiver becomes ARGUMENT 0, which is what makes `self` an
        // ordinary by-value parameter here — the same rewrite the type checker
        // and code generation both perform.
        let mut receiver_first: Vec<&Expr> = Vec::with_capacity(args.len() + 1);
        let sig_opt = match func {
            Expr::Ident(func_name) => {
                receiver_first.extend(args.iter());
                self.functions.get(func_name).cloned()
            }
            Expr::FieldAccess { object, field, .. } => {
                let owner = self.method_owner_name(object);
                receiver_first.push(object);
                receiver_first.extend(args.iter());
                owner.and_then(|owner| {
                    self.functions
                        .get(&format!("{}::{}", owner, field))
                        .cloned()
                })
            }
            _ => {
                receiver_first.extend(args.iter());
                None
            }
        };

        let Some(sig) = sig_opt else {
            // Unknown callee: still check the arguments themselves.
            for arg in receiver_first {
                self.check_expr(arg)?;
            }
            return Ok(());
        };

        let args = &receiver_first;
        for (i, arg) in args.iter().enumerate() {
            let arg: &Expr = arg;
            self.check_expr(arg)?;

            // Handle ownership based on parameter type
            let Some(param_ownership) = sig.params.get(i) else {
                continue;
            };
            let Some(place) = expr_to_place(arg) else {
                // No modellable place. That covers two very different things,
                // and only one of them is fine to ignore.
                if matches!(param_ownership, ParamOwnership::BorrowMut) {
                    self.check_unmodellable_mutable_argument(arg, span)?;
                }
                continue;
            };

            match param_ownership {
                ParamOwnership::Move => {
                    // Move the argument
                    let temp = self.context.new_temp();
                    self.context.move_value(place, temp, span)?;
                }
                ParamOwnership::Borrow => {
                    // Borrow immutably for the duration of the call
                    self.context
                        .borrow(place, RefKind::Shared, call_lifetime.clone(), span)?;
                }
                ParamOwnership::BorrowMut => {
                    // A `mut x: T` parameter writes through the pointer it is
                    // given, exactly as `&mut x` does, so it needs the same
                    // permission. Without this the explicit-reference check was
                    // one spelling away from being bypassed: passing an
                    // immutable local to a `mut` parameter still mutated it.
                    self.check_mutable_borrow_allowed(&place, span)?;
                    // Borrow mutably for the duration of the call
                    self.context
                        .borrow(place, RefKind::Mutable, call_lifetime.clone(), span)?;
                }
                ParamOwnership::Copy => {
                    // No ownership transfer
                }
            }
        }

        Ok(())
    }

    /// Bind variables in a pattern
    fn bind_pattern(&mut self, pattern: &Pattern) -> Result<()> {
        match pattern {
            Pattern::Ident(name) => {
                let place = Place::Local(name.clone());
                self.context.declare(&place);
                self.context.init_owned(place);
                // A match binding is immutable: the grammar has no `mut`
                // pattern. Recording it keeps the map total over binders.
                self.mutable_bindings.insert(name.clone(), false);
            }
            Pattern::EnumPattern { data, .. } => {
                if let Some(pattern_data) = data {
                    match pattern_data {
                        crate::ast::PatternData::Tuple(patterns) => {
                            for pattern in patterns {
                                self.bind_pattern(pattern)?;
                            }
                        }
                        crate::ast::PatternData::Struct(fields) => {
                            for (_, pattern) in fields {
                                self.bind_pattern(pattern)?;
                            }
                        }
                    }
                }
            }
            Pattern::Wildcard => {}
        }
        Ok(())
    }

    /// Check if a type is Copy (doesn't move on assignment)
    #[allow(clippy::only_used_in_recursion)]
    fn is_copy_type(&self, ty: &Type) -> bool {
        match ty {
            Type::I32 | Type::I64 | Type::U32 | Type::U64 | Type::F64 | Type::F32 | Type::Bool => {
                true
            }
            // `String` is Copy. It lowers to `const char*`, there is no drop
            // glue and no per-value free anywhere in the language (strings live
            // in a static arena released once by `__pd_cleanup_strings` via
            // `atexit`), so a copy duplicates nothing and invalidates nothing.
            //
            // Treating it as a move was also *unsatisfiable* rather than merely
            // strict: there is no `clone`, and `&T` is erased by the type
            // checker (no reference type exists there), so no syntax could read
            // a String twice out of an aggregate — `let x = s.t[0];` moved out
            // of the slot and the slot could never be read again.
            Type::String => true,
            Type::Array(_, _) | Type::Custom(_) => false,
            Type::Reference { .. } => true, // References are Copy
            Type::Unit => true,
            Type::TypeParam(_) => false, // Conservative: assume not Copy
            Type::Generic { .. } => false, // Conservative: assume not Copy
            Type::Future { .. } => false, // Futures are not Copy
            Type::Tuple(types) => {
                // Tuple is Copy if all its elements are Copy
                types.iter().all(|t| self.is_copy_type(t))
            }
        }
    }

    /// Type of a place expression, following field and index projections.
    ///
    /// This exists so `is_expr_copy` can tell `v.data[0]` (an `i64`, Copy) from
    /// `s.name` (a `String`, not Copy). Without it every projection fell into the
    /// conservative "not Copy" default and `let max = v.data[0];` *moved* the
    /// element, so the second read of the same element was rejected.
    fn place_type(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Ident(name) => self.local_types.get(name).cloned(),
            Expr::FieldAccess { object, field, .. } => {
                let base = self.place_type(object)?;
                let name = match Self::strip_reference(&base) {
                    Type::Custom(name) => name.clone(),
                    _ => return None,
                };
                self.struct_fields
                    .get(&name)?
                    .iter()
                    .find(|(field_name, _)| field_name == field)
                    .map(|(_, ty)| ty.clone())
            }
            Expr::Index { array, .. } => {
                let base = self.place_type(array)?;
                match Self::strip_reference(&base) {
                    Type::Array(elem, _) => Some(elem.as_ref().clone()),
                    _ => None,
                }
            }
            Expr::Deref { expr, .. } => self
                .place_type(expr)
                .map(|ty| Self::strip_reference(&ty).clone()),
            _ => None,
        }
    }

    /// A `&T` / `&mut T` projects to `T` for the purposes above.
    fn strip_reference(ty: &Type) -> &Type {
        match ty {
            Type::Reference { inner, .. } => Self::strip_reference(inner),
            other => other,
        }
    }

    /// Check if an expression type is Copy
    fn is_expr_copy(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Integer(_) | Expr::Float(_) | Expr::Char(_) | Expr::Bool(_) | Expr::String(_) => {
                true
            }
            // Idents and projections are Copy exactly when their type is; an
            // unresolvable type stays conservatively non-Copy.
            Expr::Ident(_) | Expr::FieldAccess { .. } | Expr::Index { .. } | Expr::Deref { .. } => {
                self.place_type(expr)
                    .map(|ty| self.is_copy_type(&ty))
                    .unwrap_or(false)
            }
            _ => false, // Conservative default
        }
    }

    /// The type name a method call on `object` dispatches on.
    ///
    /// Only NAMED types carry an `impl`, so anything else has no method to look
    /// up and this says so rather than producing a qualified name that cannot
    /// resolve.
    fn method_owner_name(&self, object: &Expr) -> Option<String> {
        match self.expr_type(object) {
            Type::Custom(name) => Some(name),
            Type::Generic { name, .. } => Some(name),
            _ => None,
        }
    }

    /// The type of an expression — A SIMPLIFIED MODEL, AND SAID SO HERE.
    ///
    /// This pass needs types for two decisions: whether a value is Copy, and
    /// which `impl` a method call dispatches on. It answers them from types
    /// somebody WROTE — a literal, a `let` annotation, a struct or enum
    /// construction, a callee's declared return type — and never infers one.
    ///
    /// Where no written type reaches an expression it answers `I64` and the
    /// caller carries on: for a receiver that means no `Type::method` signature
    /// is found and that call's parameters go unenforced, so the failure mode is
    /// FAIL-OPEN — a program that should be refused compiles. The shapes that
    /// currently land there are listed at the fallback arm below. The real type
    /// model is in `src/typeck`; the repair is to read it, not to grow a second
    /// one here.
    fn expr_type(&self, expr: &Expr) -> Type {
        match expr {
            Expr::Integer(_) => Type::I64,
            Expr::Float(_) => Type::F64,
            // The scalar, as an integer — the same decision the type checker
            // records at `Expr::Char`. N4-04 moves both or neither.
            Expr::Char(_) => Type::I64,
            Expr::String(_) => Type::String,
            Expr::Bool(_) => Type::Bool,
            // A BINDING'S TYPE COMES FROM `local_types`, which is the map this
            // very function fills. It used to answer `I64` for every name,
            // which made the map it populates a map of lies for anything that
            // was not an integer.
            Expr::Ident(name) => self.local_types.get(name).cloned().unwrap_or(Type::I64),
            // The two constructions that NAME their own type. Not a type
            // inferencer — there is one of those in `src/typeck`, and a second
            // would be the drift this repository has closed twice. These are the
            // cases where the type is written in the expression itself.
            Expr::StructLiteral { name, .. } => Type::Custom(name.clone()),
            Expr::EnumConstructor { enum_name, .. } => Type::Custom(enum_name.clone()),
            // A CALL IS TYPED BY THE CALLEE'S OWN DECLARATION, which is the only
            // way a chained receiver can be resolved: `make().consume(x)` needs
            // `make`'s return type before `T::consume` can be looked up at all.
            // Still not an inferencer — every answer here is a type somebody
            // WROTE in a signature.
            Expr::Call { func, .. } => self.call_return_type(func).unwrap_or(Type::I64),
            // The value forms produce the type of the value they carry out. The
            // branches are required to agree by the type checker, so reading one
            // is reading all of them.
            Expr::If {
                then_value,
                else_value,
                ..
            } => then_value
                .as_ref()
                .or(else_value.as_ref())
                .map(|v| self.expr_type(v))
                .unwrap_or(Type::I64),
            Expr::Block { value, .. } => value
                .as_ref()
                .map(|v| self.expr_type(v))
                .unwrap_or(Type::I64),
            Expr::Match { arms, .. } => arms
                .iter()
                .find_map(|arm| arm.value.as_ref())
                .map(|v| self.expr_type(v))
                .unwrap_or(Type::I64),
            // FAIL-OPEN, AND NAMED. Everything else answers `I64`, which for a
            // receiver means "no method signature found" and so no enforcement
            // of that call's parameters. The shapes this currently covers, all
            // of them receivers this pass cannot type:
            //   * a field access — `p.inner.take()`; struct_fields holds the
            //     layout, but a field's type is not read back here;
            //   * an index — `xs[0].take()`;
            //   * a unary/binary/cast expression, a range, a reference or a
            //     dereference;
            //   * a `loop` used as a value, whose type lives in its `break`s.
            // Widening this is a type model, not a patch, and the model already
            // exists in src/typeck; the honest state is that this pass has a
            // simplified one. Declared here and in the debt inventory rather
            // than left for a reader to discover from a program that compiles.
            _ => Type::I64,
        }
    }

    /// The declared return type of a call, by callee shape.
    ///
    /// The two shapes are the two spellings of a call this language has: a bare
    /// name, and a method reached through `.`, which is registered under
    /// `Type::method` exactly as `check_call_args` looks it up.
    fn call_return_type(&self, callee: &Expr) -> Option<Type> {
        match callee {
            Expr::Ident(name) => self.functions.get(name).and_then(|s| s.ret_ty.clone()),
            Expr::FieldAccess { object, field, .. } => {
                let owner = self.method_owner_name(object)?;
                self.functions
                    .get(&format!("{}::{}", owner, field))
                    .and_then(|s| s.ret_ty.clone())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn dummy_span() -> crate::errors::Span {
        crate::errors::Span::new(0, 0, 0, 0)
    }

    /// Run the real front end up to and including borrow checking.
    ///
    /// Driving these from source instead of a hand-built AST is deliberate: the
    /// defects below are about how *call expressions* thread borrows, and a
    /// hand-built AST is exactly where a wrong assumption about the shape the
    /// parser produces would hide.
    fn borrow_check(source: &str) -> Result<()> {
        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.collect_tokens()?;
        let mut parser = crate::parser::Parser::new(tokens);
        let mut program = parser.parse()?;
        crate::macros::MacroExpander::new().expand_program(&mut program)?;
        BorrowChecker::new().check_program(&program)
    }

    /// REGRESSION: a borrow taken to pass an argument was never released, so the
    /// *second* function call on the same array was rejected with "Conflicting
    /// borrows". A self-hosting compiler passes its token/AST buffers to one
    /// function after another, so this made the bootstrap program unwritable.
    #[test]
    fn test_array_argument_is_usable_after_the_call() {
        let result = borrow_check(
            r#"
            fn fill(mut a: [i64; 4]) { a[0] = 99; }
            fn peek(mut a: [i64; 4]) -> i64 { return a[0]; }
            fn main() {
                let mut xs = [0; 4];
                fill(xs);
                print_int(peek(xs));
                print_int(xs[0]);
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "call-argument borrow outlived the call: {:?}",
            result.err()
        );
    }

    /// REGRESSION: a non-`mut` array parameter was classified as a Move, so the
    /// caller could never touch the array again. The C backend emits `T a[N]`,
    /// which decays to a pointer — reference semantics — so a move is wrong on
    /// codegen's own terms.
    #[test]
    fn test_non_mut_array_parameter_is_a_borrow_not_a_move() {
        let result = borrow_check(
            r#"
            fn fill(a: [i64; 4]) { a[0] = 7; }
            fn main() {
                let mut xs = [0; 4];
                fill(xs);
                print_int(xs[0]);
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "array parameter still treated as a move: {:?}",
            result.err()
        );
    }

    /// A `&mut x` written in argument position is a temporary that dies with the
    /// call, so repeated `f(&mut v)` is fine. This is the `tests/misc/test_vec_i64.pd`
    /// shape, which failed with "already mutably borrowed" on the second push.
    #[test]
    fn test_reference_arguments_are_released_after_the_call() {
        let result = borrow_check(
            r#"
            struct S { x: i64 }
            fn bump(s: &mut S) { s.x = s.x + 1; }
            fn get(s: &S) -> i64 { return s.x; }
            fn main() {
                let mut v = S { x: 0 };
                bump(&mut v);
                bump(&mut v);
                print_int(get(&v));
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "reference argument borrow outlived the call: {:?}",
            result.err()
        );
    }

    /// REGRESSION: passing a struct *field* to a borrowing built-in died with
    /// "Use of uninitialized value: s.a" even though the field was assigned on
    /// the line before. Only locals are ever registered in the ownership map, so
    /// the projection `s.a` had no entry and was read as uninitialized. This is
    /// the exact program from the bootstrap compiler that first hit it.
    #[test]
    fn test_struct_field_argument_is_not_uninitialized() {
        let result = borrow_check(
            r#"
            struct S { a: String }
            fn go(mut s: S) {
                s.a = "q";
                if string_len(s.a) == 0 { print("e"); }
                print(s.a);
            }
            fn main() { let mut x: S = S { a: "" }; go(x); }
            "#,
        );
        assert!(
            result.is_ok(),
            "struct field argument reported uninitialized: {:?}",
            result.err()
        );
    }

    /// The same defect, with no assignment and no `if` block anywhere: the
    /// trigger is the projection itself, not the assignment or the block scope.
    #[test]
    fn test_unassigned_struct_field_argument_is_not_uninitialized() {
        let result = borrow_check(
            r#"
            struct S { a: String }
            fn go(s: S) -> i64 { return string_len(s.a); }
            fn main() { let x: S = S { a: "q" }; print_int(go(x)); }
            "#,
        );
        assert!(
            result.is_ok(),
            "unassigned struct field argument reported uninitialized: {:?}",
            result.err()
        );
    }

    /// Array elements are projections too, and had the same defect.
    #[test]
    fn test_array_element_argument_is_not_uninitialized() {
        let result = borrow_check(
            r#"
            fn main() {
                let parts: [String; 2] = ["ab", "cd"];
                print_int(string_len(parts[0]));
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "array element argument reported uninitialized: {:?}",
            result.err()
        );
    }

    /// GENUINE ERROR, still rejected: resolving a projection to its base must not
    /// lose *partial* moves — a non-Copy field moved out of a struct stays moved.
    /// The field type is a struct, since `String` is Copy.
    #[test]
    fn test_moved_struct_field_is_still_rejected() {
        let result = borrow_check(
            r#"
            struct Inner { v: i64 }
            struct S { a: Inner }
            fn main() {
                let x: S = S { a: Inner { v: 1 } };
                let p = x.a;
                let q = x.a;
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::UseOfMovedValue { .. })),
            "reuse of a moved struct field was accepted: {:?}",
            result
        );
    }

    /// GENUINE ERROR, still rejected: a real move of a non-Copy value.
    ///
    /// This used to be spelled with a `String` (`let a = "x"; let b = a;`), but
    /// `String` is now Copy — it lowers to `const char*`, nothing frees it per
    /// value, and with no `clone` and no usable reference type the move rule was
    /// unsatisfiable. Struct values are still non-Copy, so the use-after-move
    /// path is exercised with one of those instead.
    #[test]
    fn test_use_after_move_is_still_rejected() {
        let result = borrow_check(
            r#"
            struct S { v: i64 }
            fn main() {
                let a: S = S { v: 1 };
                let b = a;
                let c = a;
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::UseOfMovedValue { .. })),
            "use after move was accepted: {:?}",
            result
        );
    }

    /// A CALL AS A RECEIVER IS TYPED BY THE CALLEE'S SIGNATURE.
    ///
    /// Measured before this: `let a = make().consume(x); let b = x.v;` COMPILED.
    /// The receiver `make()` had no case in `expr_type`, so it answered `I64`,
    /// no `T::consume` signature was found for an `i64`, and the method's
    /// by-value parameter `x: S` was never enforced — the identical free-function
    /// spelling was refused the whole time.
    #[test]
    fn test_a_call_receiver_enforces_the_methods_parameters() {
        let result = borrow_check(
            r#"
            struct S { v: i64 }
            struct T { w: i64 }
            impl T { fn consume(self, x: S) -> i64 { return x.v + self.w; } }
            fn make() -> T { return T { w: 1 }; }
            fn main() {
                let x = S { v: 5 };
                let a = make().consume(x);
                let b = x.v;
                print_int(a + b);
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::UseOfMovedValue { .. })),
            "a moved argument to a method on a call receiver was accepted: {:?}",
            result
        );
    }

    /// The same, one link further along: the receiver is itself a METHOD call,
    /// so the chain resolves only if a method's return type is available too —
    /// and `-> Self` has to resolve to the impl type before the lookup can hit.
    #[test]
    fn test_a_method_chain_receiver_enforces_the_next_calls_parameters() {
        let result = borrow_check(
            r#"
            struct S { v: i64 }
            impl S {
                fn dup(self) -> Self { return S { v: self.v }; }
                fn eat(self, o: S) -> i64 { return self.v + o.v; }
            }
            fn main() {
                let a = S { v: 1 };
                let b = S { v: 2 };
                let n = a.dup().eat(b);
                let m = b.v;
                print_int(n + m);
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::UseOfMovedValue { .. })),
            "a moved argument to a method on a method-chain receiver was accepted: {:?}",
            result
        );
    }

    /// NO FALSE POSITIVE: one use is one use, however the receiver is spelled.
    ///
    /// The contract on the repair above — enforcing a chained receiver's
    /// signature must not invent a second move. A fresh binding per by-value
    /// receiver, a chain used once, and a method taking an argument by copy all
    /// stay accepted. (`a.dup()` CONSUMES `a`, so a later `a.take()` is a real
    /// second use and belongs in the refusal tests above; it was written here
    /// first, and the pass was right to refuse it.)
    #[test]
    fn test_single_use_chains_are_still_accepted() {
        let result = borrow_check(
            r#"
            struct S { v: i64 }
            impl S {
                fn dup(self) -> Self { return S { v: self.v }; }
                fn take(self) -> i64 { return self.v; }
                fn add(self, n: i64) -> i64 { return self.v + n; }
            }
            fn make() -> S { return S { v: 7 }; }
            fn main() {
                let a = S { v: 1 };
                let b = S { v: 2 };
                print_int(a.dup().take());
                print_int(b.add(3));
                print_int(make().take());
            }
            "#,
        );
        assert!(result.is_ok(), "a single-use chain was refused: {:?}", result);
    }

    /// THE DECLARED FAIL-OPEN, pinned so the declaration cannot rot into a lie.
    ///
    /// A FIELD-ACCESS receiver (`p.inner.take()`) is still typed `I64`, so the
    /// method's parameters are not enforced and this program is ACCEPTED. It is
    /// the shape named in `expr_type`'s fail-open comment and in the debt
    /// inventory. If this test ever fails because the program is refused, the
    /// repair landed and both declarations should be narrowed to match.
    #[test]
    fn test_a_field_access_receiver_is_the_declared_fail_open() {
        let result = borrow_check(
            r#"
            struct S { v: i64 }
            struct P { inner: S }
            impl S { fn eat(self, o: S) -> i64 { return self.v + o.v; } }
            fn main() {
                let p = P { inner: S { v: 1 } };
                let b = S { v: 2 };
                let n = p.inner.eat(b);
                let m = b.v;
                print_int(n + m);
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "a field-access receiver now enforces its signature — narrow the fail-open \
             declaration in expr_type and in the debt inventory: {:?}",
            result
        );
    }

    /// GENUINE ERROR, still rejected: two mutable borrows of the same array that
    /// really are live at the same time, because they are arguments to the *same*
    /// call. Releasing borrows per call must not release them per argument.
    #[test]
    fn test_aliased_mutable_arguments_in_one_call_are_rejected() {
        let result = borrow_check(
            r#"
            fn f(mut a: [i64; 4], mut b: [i64; 4]) { a[0] = 1; b[0] = 2; }
            fn main() {
                let mut xs = [0; 4];
                f(xs, xs);
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::ConflictingBorrows { .. })),
            "aliased mutable arguments were accepted: {:?}",
            result
        );
    }

    /// GENUINE ERROR, still rejected: a shared and a mutable borrow of the same
    /// place, live simultaneously as arguments to one call.
    #[test]
    fn test_shared_and_mutable_arguments_in_one_call_are_rejected() {
        let result = borrow_check(
            r#"
            struct S { x: i64 }
            fn g(a: &S, b: &mut S) -> i64 { return a.x; }
            fn main() {
                let mut v = S { x: 1 };
                print_int(g(&v, &mut v));
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::ConflictingBorrows { .. })),
            "shared+mutable arguments were accepted: {:?}",
            result
        );
    }

    /// A non-Copy value moves on `let`. Uses a struct type: `String` is Copy
    /// now (see `is_copy_type`), so it no longer exercises this path.
    #[test]
    fn test_basic_move() {
        let mut checker = BorrowChecker::new();

        let func = Function {
            visibility: Visibility::Private,
            is_async: false,
            name: "test".to_string(),
            lifetime_params: vec![],
            type_params: vec![],
            const_params: vec![],
            params: vec![],
            return_type: None,
            effects: None,
            body: vec![
                Stmt::Let {
                    name: "x".to_string(),
                    ty: Some(Type::Custom("S".to_string())),
                    value: Expr::StructLiteral {
                        name: "S".to_string(),
                        fields: vec![("v".to_string(), Expr::Integer(1))],
                        span: dummy_span(),
                    },
                    mutable: false,
                    span: dummy_span(),
                },
                Stmt::Let {
                    name: "y".to_string(),
                    ty: Some(Type::Custom("S".to_string())),
                    value: Expr::Ident("x".to_string()),
                    mutable: false,
                    span: dummy_span(),
                },
                // This should fail - x was moved
                Stmt::Expr(Expr::Ident("x".to_string())),
            ],
            span: dummy_span(),
        };

        let result = checker.check_function(&func);
        assert!(result.is_err());
    }

    #[test]
    fn test_copy_type_no_move() {
        let mut checker = BorrowChecker::new();

        let func = Function {
            visibility: Visibility::Private,
            is_async: false,
            name: "test".to_string(),
            lifetime_params: vec![],
            type_params: vec![],
            const_params: vec![],
            params: vec![],
            return_type: None,
            effects: None,
            body: vec![
                Stmt::Let {
                    name: "x".to_string(),
                    ty: Some(Type::I32),
                    value: Expr::Integer(42),
                    mutable: false,
                    span: dummy_span(),
                },
                Stmt::Let {
                    name: "y".to_string(),
                    ty: Some(Type::I32),
                    value: Expr::Ident("x".to_string()),
                    mutable: false,
                    span: dummy_span(),
                },
                // This should succeed - i32 is Copy
                Stmt::Expr(Expr::Ident("x".to_string())),
            ],
            span: dummy_span(),
        };

        let result = checker.check_function(&func);
        if let Err(ref e) = result {
            eprintln!("Error in test_copy_type_no_move: {:?}", e);
        }
        assert!(result.is_ok());
    }

    /// GENUINE ERROR, newly rejected: a mutable reference to a binding that was
    /// never declared mutable.
    ///
    /// Creating the reference was unchecked, so `&mut v` on an immutable `let`
    /// compiled, linked and wrote through: the program below printed 42 for a
    /// binding the author did not mark `mut`. The write itself is legal C - the
    /// parameter decays to a pointer - so nothing downstream caught it either.
    #[test]
    fn test_mutable_borrow_of_immutable_binding_is_rejected() {
        let result = borrow_check(
            r#"
            fn set(xs: &mut [i64; 3]) { xs[0] = 42; }
            fn main() {
                let v = [1, 2, 3];
                set(&mut v);
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::BorrowChecker { .. })),
            "a mutable borrow of an immutable binding was accepted: {:?}",
            result
        );
    }

    /// The same shape on a scalar, which has no array decay to blame.
    #[test]
    fn test_mutable_borrow_of_immutable_scalar_is_rejected() {
        let result = borrow_check(
            r#"
            fn bump(x: &mut i64) { }
            fn main() {
                let n = 1;
                bump(&mut n);
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::BorrowChecker { .. })),
            "a mutable borrow of an immutable scalar was accepted: {:?}",
            result
        );
    }

    /// NOT AN ERROR: the same program with the binding declared mutable. The
    /// check must cost nothing to code that says what it means.
    #[test]
    fn test_mutable_borrow_of_mutable_binding_is_accepted() {
        let result = borrow_check(
            r#"
            fn set(xs: &mut [i64; 3]) { xs[0] = 42; }
            fn main() {
                let mut v = [1, 2, 3];
                set(&mut v);
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "a mutable borrow of a `let mut` binding was rejected: {:?}",
            result
        );
    }

    /// NOT AN ERROR: a `&mut` *parameter* re-borrowed mutably. The permission
    /// arrived with the parameter, so it is not invented here.
    ///
    /// The element type matters: this used to pass `&mut xs[0]` (a `&mut i64`)
    /// to a `&mut [i64; 3]` parameter, which is a type error, and `borrow_check`
    /// never runs the type checker - so the test proved only that the borrow
    /// checker accepted a program nothing else would. The callee now takes the
    /// element type the argument actually has, and
    /// test_reborrow_program_compiles_end_to_end puts the same source through
    /// the whole pipeline.
    #[test]
    fn test_mutable_reborrow_of_mutable_parameter_is_accepted() {
        let result = borrow_check(REBORROW_PROGRAM);
        assert!(
            result.is_ok(),
            "a mutable re-borrow of a `&mut` parameter was rejected: {:?}",
            result
        );
    }

    /// The program above, through lexer, parser, macros, type checker, borrow
    /// checker and code generation - so "accepted" means accepted by the
    /// compiler, not by one pass of it.
    const REBORROW_PROGRAM: &str = r#"
            fn bump(x: &mut i64) { }
            fn outer(xs: &mut [i64; 3]) { bump(&mut xs[0]); }
            fn main() {
                let mut v = [1, 2, 3];
                outer(&mut v);
                print_int(v[0]);
            }
            "#;

    #[test]
    fn test_reborrow_program_compiles_end_to_end() {
        let mut lexer = crate::lexer::Lexer::new(REBORROW_PROGRAM);
        let tokens = lexer.collect_tokens().expect("lexing");
        let mut program = crate::parser::Parser::new(tokens).parse().expect("parsing");
        crate::macros::MacroExpander::new()
            .expand_program(&mut program)
            .expect("macro expansion");
        crate::typeck::TypeChecker::new()
            .check(&program)
            .expect("type checking");
        BorrowChecker::new()
            .check_program(&program)
            .expect("borrow checking");
        let mut codegen = crate::codegen::CodeGenerator::new("reborrow").expect("codegen setup");
        codegen.compile(&program).expect("code generation");
    }

    /// GENUINE ERROR, newly rejected: an immutable local passed to a `mut`
    /// parameter. `mut xs: T` writes through the pointer it is handed exactly
    /// as `&mut xs` does, so the explicit-reference check was one spelling away
    /// from being bypassed - this program used to compile and write 42 into an
    /// immutable binding.
    #[test]
    fn test_immutable_local_passed_to_a_mut_parameter_is_rejected() {
        let result = borrow_check(
            r#"
            fn bump(mut xs: [i64; 3]) { xs[0] = 42; }
            fn main() {
                let v = [1, 2, 3];
                bump(v);
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::BorrowChecker { .. })),
            "an immutable local passed to a `mut` parameter was accepted: {:?}",
            result
        );
    }

    /// The mutability record is per scope. A flat map leaked forward: an inner
    /// `let mut v` left the *outer*, immutable `v` marked writable for the rest
    /// of the function. This is the defect that was fixed for codegen's array
    /// bindings and then reintroduced here.
    #[test]
    fn test_inner_mutable_shadow_does_not_make_the_outer_binding_writable() {
        let result = borrow_check(
            r#"
            fn set(xs: &mut [i64; 3]) { xs[0] = 9; }
            fn main() {
                let v = [1, 2, 3];
                if true {
                    let mut v = [7, 7, 7];
                    print_int(v[0]);
                }
                set(&mut v);
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::BorrowChecker { .. })),
            "an inner `let mut` shadow made an immutable outer binding writable: {:?}",
            result
        );
    }

    /// And backward: an inner immutable shadow must not make a legitimate outer
    /// `let mut` be rejected. A scope that only ever over-rejects is not scoped,
    /// it is broken in the other direction.
    #[test]
    fn test_inner_immutable_shadow_does_not_reject_the_outer_mutable_binding() {
        let result = borrow_check(
            r#"
            fn set(xs: &mut [i64; 3]) { xs[0] = 9; }
            fn main() {
                let mut v = [1, 2, 3];
                if true {
                    let v = [7, 7, 7];
                    print_int(v[0]);
                }
                set(&mut v);
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "an inner immutable shadow rejected a legitimate outer `let mut`: {:?}",
            result
        );
    }

    /// GENUINE ERROR, newly rejected: an immutable *scalar* passed to a `mut`
    /// parameter. Codegen emits every `mut` parameter as a pointer into the
    /// caller's storage, but the signature collector checked the type before
    /// the mutability, so a primitive was classified Copy and never reached the
    /// permission check. Measured before the fix: this program printed 42.
    #[test]
    fn test_immutable_scalar_passed_to_a_mut_parameter_is_rejected() {
        let result = borrow_check(
            r#"
            fn bump(mut x: i64) { x = 42; }
            fn main() {
                let n = 1;
                bump(n);
                print_int(n);
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::BorrowChecker { .. })),
            "an immutable scalar passed to a `mut` parameter was accepted: {:?}",
            result
        );
    }

    /// The same for `String`, which had its own Copy arm ahead of the
    /// mutability test. Measured before the fix: the caller's string printed as
    /// "changed".
    #[test]
    fn test_immutable_string_passed_to_a_mut_parameter_is_rejected() {
        let result = borrow_check(
            r#"
            fn retitle(mut s: String) { s = "changed"; }
            fn main() {
                let t: String = "orig";
                retitle(t);
                print(t);
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::BorrowChecker { .. })),
            "an immutable String passed to a `mut` parameter was accepted: {:?}",
            result
        );
    }

    /// A `mut` parameter of primitive type is still usable - the fix must
    /// reject the immutable *argument*, not the declaration.
    #[test]
    fn test_mutable_local_passed_to_a_mut_scalar_parameter_is_accepted() {
        let result = borrow_check(
            r#"
            fn bump(mut x: i64) { x = 42; }
            fn main() {
                let mut n = 1;
                bump(n);
                print_int(n);
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "a `let mut` scalar passed to a `mut` parameter was rejected: {:?}",
            result
        );
    }

    /// GENUINE ERROR, newly rejected: a `for` variable borrowed mutably. Loop
    /// variables entered the ownership context but never the mutability map,
    /// and an unregistered name used to be permitted, so they bypassed the
    /// invariant entirely.
    #[test]
    fn test_mutable_borrow_of_a_for_variable_is_rejected() {
        let result = borrow_check(
            r#"
            fn bump(x: &mut i64) { }
            fn main() {
                let xs = [1, 2, 3];
                for v in xs {
                    bump(&mut v);
                }
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::BorrowChecker { .. })),
            "a mutable borrow of a `for` variable was accepted: {:?}",
            result
        );
    }

    /// And a match binding, which `bind_pattern` also left unregistered.
    #[test]
    fn test_mutable_borrow_of_a_match_binding_is_rejected() {
        let result = borrow_check(
            r#"
            fn bump(x: &mut i64) { }
            fn main() {
                let n = 5;
                match n {
                    other => { bump(&mut other); }
                }
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::BorrowChecker { .. })),
            "a mutable borrow of a match binding was accepted: {:?}",
            result
        );
    }

    /// REGRESSION, this branch's own: registering the `for` binder before
    /// `check_block_stmts` meant the helper snapshotted the *already
    /// overwritten* map, so the loop variable outlived its loop and an outer
    /// `let mut v` stayed marked immutable afterwards.
    #[test]
    fn test_for_binder_does_not_outlive_its_loop() {
        let result = borrow_check(
            r#"
            fn set(xs: &mut [i64; 3]) { xs[0] = 9; }
            fn main() {
                let mut v = [1, 2, 3];
                let ys = [7, 8];
                for v in ys { print_int(v); }
                set(&mut v);
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "a `for` binder left the outer `let mut` marked immutable: {:?}",
            result
        );
    }

    /// The other direction: inside the loop the binder really is immutable.
    #[test]
    fn test_for_binder_is_immutable_inside_its_loop() {
        let result = borrow_check(
            r#"
            fn bump(x: &mut i64) { }
            fn main() {
                let mut v = 1;
                let ys = [7, 8];
                for v in ys { bump(&mut v); }
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::BorrowChecker { .. })),
            "a mutable borrow of the `for` binder was accepted: {:?}",
            result
        );
    }

    /// Same pair for match arms.
    #[test]
    fn test_match_binder_does_not_outlive_its_arm() {
        let result = borrow_check(
            r#"
            fn bump(x: &mut i64) { }
            fn main() {
                let mut n = 1;
                let k = 5;
                match k { n => { print_int(n); } }
                bump(&mut n);
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "a match binder left the outer `let mut` marked immutable: {:?}",
            result
        );
    }

    #[test]
    fn test_match_binder_is_immutable_inside_its_arm() {
        let result = borrow_check(
            r#"
            fn bump(x: &mut i64) { }
            fn main() {
                let mut n = 1;
                let k = 5;
                match k { other => { bump(&mut other); } }
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::BorrowChecker { .. })),
            "a mutable borrow of the match binder was accepted: {:?}",
            result
        );
    }

    /// GENUINE ERROR, newly rejected: an rvalue passed to a `mut` parameter.
    /// There is no storage for the pointer the callee receives; codegen emitted
    /// `bump(&1)` and gcc rejected it with "cannot take the address of an
    /// rvalue". Refused in the language instead.
    #[test]
    fn test_rvalue_passed_to_a_mut_parameter_is_rejected() {
        for (arg, program) in [
            (
                "integer literal",
                r#"
            fn bump(mut x: i64) { x = 42; }
            fn main() { bump(1); }
            "#,
            ),
            (
                "call result",
                r#"
            fn make() -> i64 { return 1; }
            fn bump(mut x: i64) { x = 42; }
            fn main() { bump(make()); }
            "#,
            ),
            (
                "computed value",
                r#"
            fn bump(mut x: i64) { x = 42; }
            fn main() { let a = 1; bump(a + 1); }
            "#,
            ),
        ] {
            let result = borrow_check(program);
            assert!(
                matches!(result, Err(CompileError::BorrowChecker { .. })),
                "an rvalue ({}) passed to a `mut` parameter was accepted: {:?}",
                arg,
                result
            );
        }
    }

    /// NOT AN ERROR: `xs[i]` with a non-literal index is real caller storage,
    /// and its C (`bump(&xs[i])`) is correct. `expr_to_place` cannot model it,
    /// which must not be read as "no storage".
    #[test]
    fn test_unmodellable_place_passed_to_a_mut_parameter_is_accepted() {
        let result = borrow_check(
            r#"
            fn bump(mut x: i64) { x = 9; }
            fn main() {
                let mut xs = [1, 2, 3];
                let i = 1;
                bump(xs[i]);
            }
            "#,
        );
        assert!(
            result.is_ok(),
            "a variable-indexed element passed to a `mut` parameter was rejected: {:?}",
            result
        );
    }

    /// But it is still checked: the same shape on an immutable binding used to
    /// go unexamined and wrote 9 into it.
    #[test]
    fn test_unmodellable_place_on_an_immutable_binding_is_rejected() {
        let result = borrow_check(
            r#"
            fn bump(mut x: i64) { x = 9; }
            fn main() {
                let xs = [1, 2, 3];
                let i = 1;
                bump(xs[i]);
            }
            "#,
        );
        assert!(
            matches!(result, Err(CompileError::BorrowChecker { .. })),
            "a write through an immutable binding's element was accepted: {:?}",
            result
        );
    }
}
