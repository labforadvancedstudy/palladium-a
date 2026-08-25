//! Palladium identifiers that C would read as something other than a name.
//!
//! THE DEFECT THIS EXISTS FOR
//! Code generation used to write every user identifier into the C verbatim.
//! `fn double(x: i64) -> i64` emitted
//!
//! ```text
//! long long double(long long x);
//! ```
//!
//! which is not a declaration of anything — `double` is a C type specifier, so
//! gcc read `long long double` and said "'long long double' is invalid". The
//! compiler had already printed "✅ Compilation successful" by then, because
//! the failure happens in the linker step and only when `-o` is passed; a
//! `pdc compile` with no `-o` never calls gcc at all and reports success over C
//! that cannot be compiled. Every identifier position was affected, not only
//! function names — measured on `main`:
//!
//! ```text
//! typedef struct register { long long signed; ... } register;
//! long long static = x;
//! __pd_print_int(double(short));
//! ```
//!
//! WHY MANGLING AND NOT A REFUSAL
//! `double` is an ordinary word. Refusing it would export an implementation
//! detail of the backend into the language, and the declared expectation is
//! `test_c_keyword_identifier_still_links` — the program must still LINK, not
//! be diagnosed. The C name is not part of Palladium's interface, so renaming
//! it costs the user nothing.
//!
//! THE ESCAPE IS INJECTIVE, WHICH IS THE WHOLE POINT
//! A rename that can map two source names onto one C name trades a loud gcc
//! error for a silent one — two functions sharing a definition. So a trailing
//! `_` is appended not only to reserved words but to anything that BECOMES one
//! when its trailing underscores are stripped:
//!
//! ```text
//! double   -> double_
//! double_  -> double__
//! doubled  -> doubled     (unchanged: `doubled` is not a reserved word)
//! ```
//!
//! Distinct inputs therefore stay distinct. A trailing underscore is also the
//! one decoration C does not reserve for the implementation: leading `_` at
//! file scope and any `__` anywhere are reserved (C11 7.1.3), and `__pd_` is
//! already the runtime's own prefix.
//!
//! THE THING PROTECTED MUST BE THE THING EMITTED
//! The first version of this module escaped the AST and stopped there, on the
//! reasoning that every name codegen prints comes from the AST. That is false
//! for a DERIVED name — one codegen computes from an AST name — and it was
//! false in exactly one place, found by review and reproduced:
//!
//! ```text
//! enum E { Register(i64), Plain }
//!   ->  E__Register_Data register;          // the union member
//!       gcc: error: expected identifier
//! ```
//!
//! The member was `variant.name.to_lowercase()`. The escape had compared the
//! SOURCE spelling `Register` against the reserved list, found no match, and
//! passed it through; the derivation then produced `register`, which is
//! reserved. A derivation also has to be INJECTIVE for the same reason the
//! escape does, and case folding is not: `Register` and `register` as two
//! variants of one enum both fold to `register`, which is a silent duplicate
//! union member — the loud-to-silent trade the escape exists to prevent.
//!
//! So every derived name goes through a NAMED function here, each of which
//! applies `c_ident` to its own result rather than trusting its input, and each
//! of which is tested for both properties. `c_enum_payload_member` is the one
//! that exists today.
//!
//! TWO ENUMERATIONS, AND NEITHER IS LOAD-BEARING AS PROSE. A hand-written
//! navigation map that omits one site is the same disease one level up, so the
//! lists below are DERIVED FROM `src/codegen/mod.rs` AT TEST TIME by
//! `every_payload_member_emission_uses_the_one_derivation`,
//! `code_generation_never_case_folds_an_identifier` and
//! `every_codegen_ingress_escapes_what_it_is_given`
//! (tests/m1_c_keyword_idents.rs). What follows is their current output, for a
//! reader who cannot run them.
//!
//! EVERY ENTRY INTO CODE GENERATION — an AST or a type-checker template can
//! arrive only through these four, so a fifth is a visible omission:
//!
//! ```text
//! CodeGenerator::set_imported_modules              -> escape_reserved_names
//! CodeGenerator::set_generic_instantiations        -> escape_generic_function
//! CodeGenerator::set_generic_struct_instantiations -> escape_generic_struct
//! CodeGenerator::compile                           -> escape_reserved_names
//! ```
//!
//! EVERY SITE THAT SPELLS THE ENUM PAYLOAD MEMBER — six, and there is no
//! seventh; the test finds them by the C they emit, not by this list:
//!
//! ```text
//! union member declaration, tuple variant   result.data.<m>.field<i>  (write)
//! union member declaration, struct variant  result.data.<m>.<field>   (write)
//! match read, _match_expr.data.<m>.field<i> _match_expr.data.<m>.<f>  (read)
//! ```
//!
//! Code generation now case-folds NOTHING: there is no `to_lowercase`,
//! `to_uppercase` or `to_ascii_*` anywhere in `src/codegen/mod.rs`, which is
//! the cheapest statement that the enum-member defect cannot come back, and it
//! is the one the second test above makes.
//!
//! The generic setters above are the same defect as the enum member, one level
//! up:
//! monomorphisation templates come from the TYPE CHECKER, which is handed the
//! unescaped AST (`src/driver/mod.rs:109`, before `compile` escapes anything),
//! and `monomorphize_function` clones their names, parameters and bodies
//! straight into `generate_function`. Measured before the fix:
//!
//! ```text
//! fn pick<T>(register: T) -> T { … }
//!   ->  long long pick__i64(long long register) { void* static = register; }
//!       gcc: error: expected identifier or '('
//! ```

use crate::ast::{
    ArraySize, AssignTarget, ConstValue, EnumConstructorData, EnumVariantData, Expr, Function,
    GenericArg, Item, Pattern, PatternData, Program, Stmt, Type,
};
use std::borrow::Cow;

/// Spellings a supported C compiler will not accept as an identifier.
///
/// THE MEMBERSHIP RULE, so that "is X missing?" has an answer that is not
/// recall: **an entry belongs here iff some C compiler this project links with
/// refuses to use the name as an identifier.** Nothing weaker — a name that
/// merely *looks* reserved is not reserved (see the `__label` case below) — and
/// nothing stronger, because a rename nobody needs is still a rename.
///
/// WHERE EACH GROUP CAME FROM
///
///   1. C89/C99/C11/C23 keywords, including the `_`-prefixed spellings.
///      A program compiled as C23 (gcc 15 defaults to `-std=gnu23`) rejects
///      `bool` and `constexpr` as names while a C99 build accepts them, so the
///      union is taken and the emitted C's validity stops depending on which
///      `-std` the linker happens to pick.
///   2. The GNU alternate keywords and GCC/clang extension keywords —
///      `__asm__`, `__inline__`, `__const__`, `__restrict__`, `__attribute__`,
///      `__typeof__`, … — from the GCC manual's *Alternate Keywords* and
///      *C Extensions* sections. **Every one of them was then MEASURED**: the
///      candidate corpus is compiled one name at a time by the real `cc` in
///      `the_reserved_list_covers_every_keyword_this_toolchain_has`
///      (tests/m1_c_keyword_idents.rs), which fails if this list is short.
///      That test is the derivation; this list is its record.
///      The gap was real and was found by review: the list already reached for
///      GNU by carrying `asm`, `typeof`, `inline` and `restrict`, and stopped
///      before their `__`-wrapped alternates, which gcc rejects just as hard.
///   3. `_FloatN` / `_FloatNx` and the Embedded-C fixed-point keywords
///      (`_Accum`, `_Fract`, `_Sat`). **Documentation, not measurement**: the
///      GCC manual makes them keywords and this checkout's clang accepts them
///      as identifiers, so the measuring test above cannot see them. They are
///      here because the emitted C is compiled by whatever `cc` the user has,
///      and a program that links on macOS and fails on Linux is the portability
///      form of the same silent defect.
///
/// NOT IN HERE, DELIBERATELY, AND THE BOUNDARY IS MEASURED BOTH WAYS:
///
///   * Library and builtin identifiers — `printf`, `malloc`, `strlen`, and
///     `__builtin_va_list`, which this toolchain also refuses as a function
///     name. Those fail LOUDLY (gcc: "conflicting types for 'strlen'"), so
///     none of them is the silent class M1 exists to remove, and reserving them
///     would mean tracking every header the prelude includes.
///     `a_library_name_is_still_rejected_and_rejected_loudly` pins that.
///   * Names that merely look like implementation names. `__label__` is a
///     keyword; `__label` is NOT, and neither are `__extension`, `__func`,
///     `__foo__` or `__bar` — all measured. C11 §7.1.3 does reserve every
///     `__`-prefixed identifier to the implementation, so escaping them all
///     would be defensible, but it would rename names no compiler objects to.
///     `is_escaped_or_reserved` is written to draw exactly this line.
///
/// Sorted, and `sorted_for_binary_search` below is what keeps it that way.
/// `pub` so the derivation test can read the list it is checking.
pub const RESERVED: &[&str] = &[
    "_Accum",
    "_Alignas",
    "_Alignof",
    "_Atomic",
    "_BitInt",
    "_Bool",
    "_Complex",
    "_Decimal128",
    "_Decimal32",
    "_Decimal64",
    "_Float128",
    "_Float128x",
    "_Float16",
    "_Float16x",
    "_Float32",
    "_Float32x",
    "_Float64",
    "_Float64x",
    "_Fract",
    "_Generic",
    "_Imaginary",
    "_Nonnull",
    "_Noreturn",
    "_Null_unspecified",
    "_Nullable",
    "_Sat",
    "_Static_assert",
    "_Thread_local",
    "__FUNCTION__",
    "__PRETTY_FUNCTION__",
    "__alignof",
    "__alignof__",
    "__asm",
    "__asm__",
    "__attribute",
    "__attribute__",
    "__auto_type",
    "__bf16",
    "__cdecl",
    "__complex",
    "__complex__",
    "__const",
    "__const__",
    "__extension__",
    "__fastcall",
    "__float128",
    "__fp16",
    "__func__",
    "__has_include",
    "__ibm128",
    "__imag",
    "__imag__",
    "__inline",
    "__inline__",
    "__int128",
    "__label__",
    "__nullable",
    "__real",
    "__real__",
    "__restrict",
    "__restrict__",
    "__signed",
    "__signed__",
    "__stdcall",
    "__thiscall",
    "__thread",
    "__typeof",
    "__typeof__",
    "__vectorcall",
    "__volatile",
    "__volatile__",
    "alignas",
    "alignof",
    "asm",
    "auto",
    "bool",
    "break",
    "case",
    "char",
    "const",
    "constexpr",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "false",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "nullptr",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "struct",
    "switch",
    "thread_local",
    "true",
    "typedef",
    "typeof",
    "typeof_unqual",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
];

/// The C spelling of a Palladium identifier.
///
/// Returns the name unchanged in the overwhelmingly common case, so this can be
/// called at every emission site without allocating at any of them.
pub fn c_ident(name: &str) -> Cow<'_, str> {
    if is_escaped_or_reserved(name) {
        Cow::Owned(format!("{}_", name))
    } else {
        Cow::Borrowed(name)
    }
}

/// The C name of the union member that carries an enum variant's payload.
///
/// This is a DERIVED name — the only one code generation computes rather than
/// copies — and it is written at four sites: the member's declaration inside
/// `union { … } data`, the two constructor bodies (`result.data.<m>.field0`,
/// `result.data.<m>.<field>`), and match-arm destructuring
/// (`_match_expr.data.<m>.…`). All four must agree, which is why it is one
/// function rather than four expressions.
///
/// IT WAS `variant.name.to_lowercase()`, and both halves of that were wrong:
///
///   * not reserved-safe — `Register` folds to `register`, so
///     `enum E { Register(i64) }` emitted `E__Register_Data register;` and gcc
///     said "expected identifier". The AST escape could not see it: it had
///     already passed `Register`, which is not a reserved word.
///   * not injective — `Register` and `register` as two variants of one enum
///     both fold to `register`, a duplicate union member. That is the
///     loud-to-silent trade `c_ident`'s own injectivity exists to prevent, and
///     it is worse here because a duplicate member is not even a gcc error in
///     every position.
///
/// The derivation is now the identity followed by the escape. The identity is
/// injective over the variants of one enum (two variants of the same name emit
/// two `__E__X` tag constants and gcc refuses the program), and the escape is
/// applied to THIS function's own result rather than assumed from its input —
/// so the guarantee belongs to this function and does not depend on having been
/// called with an already-escaped name.
pub fn c_enum_payload_member(variant: &str) -> Cow<'_, str> {
    c_ident(variant)
}

/// A monomorphisation template for a generic function, with every name it
/// carries in its C spelling.
///
/// The NAME is escaped too, not only the body: `get_mangled_name_for_call`
/// resolves a call by comparing the call's function name — which comes from the
/// escaped AST — against these entries, so an unescaped key would stop matching
/// a keyword-named generic instead of merely misspelling it.
///
/// `type_params` are left alone for the reason `escape_reserved_names` gives:
/// `type_to_c` erases them to `void*` and the name never reaches the C.
pub fn escape_generic_function(
    name: &str,
    f: &crate::typeck::GenericFunction,
) -> (String, crate::typeck::GenericFunction) {
    let mut out = f.clone();
    for (param, ty) in &mut out.params {
        escape_in_place(param);
        escape_type(ty);
    }
    if let Some(ty) = &mut out.return_type {
        escape_type(ty);
    }
    escape_block(&mut out.body);
    (c_ident(name).into_owned(), out)
}

/// The same for a generic struct template. See `escape_generic_function`.
pub fn escape_generic_struct(
    name: &str,
    s: &crate::typeck::GenericStruct,
) -> (String, crate::typeck::GenericStruct) {
    let mut out = s.clone();
    for (field, ty) in &mut out.fields {
        escape_in_place(field);
        escape_type(ty);
    }
    (c_ident(name).into_owned(), out)
}

/// Is `name` a reserved word, or already in the image of the escape?
///
/// The second half is what makes `c_ident` injective. Without it `double` and
/// `double_` would both emit `double_`.
///
/// ONE UNDERSCORE AT A TIME, CHECKING AT EVERY STEP — not
/// `trim_end_matches('_')` once, which is what this was and which OVER-RESERVED.
/// Measured with the real `cc`: `__label__`, `__extension__` and `__func__` are
/// keywords while `__label`, `__extension` and `__func` are ordinary
/// identifiers. Stripping every trailing underscore in one go asks about
/// `__label` when the entry is `__label__`, so it either misses the keyword (if
/// the list stores the `__`-form) or renames the non-keyword (if it stores the
/// stem). Stepping means the list can store exactly what a compiler rejects.
///
/// Injectivity survives the change. `f(x) = x + "_"` when this returns true and
/// `x` otherwise; suppose `f(a) = f(b)` with `a != b`. If both escaped or
/// neither did, `a = b`. Otherwise, say `a` escaped and `b` did not, so
/// `b = a + "_"` — but then `b` ends in `_` and this function's own recursion
/// reaches `a`, which is reserved, so `b` escapes too. Contradiction.
///
/// It also stays a bounded walk: each step removes one byte.
fn is_escaped_or_reserved(name: &str) -> bool {
    if RESERVED.binary_search(&name).is_ok() {
        return true;
    }
    match name.strip_suffix('_') {
        Some(shorter) => is_escaped_or_reserved(shorter),
        None => false,
    }
}

/// Rewrite every name in `program` to its C spelling.
///
/// WHY A PASS OVER THE AST AND NOT `c_ident(…)` AT EACH EMISSION SITE
/// Code generation does not only *print* names, it KEYS ON THEM: `variables`,
/// `mutable_params`, `array_bindings`, `structs`, `type_aliases` and
/// `defined_structs` are maps from a name to something the emitted C depends
/// on, and `defined_structs` is compared against tags parsed back out of a
/// signature string (`signature_tags_are_defined`). Escaping at the print sites
/// leaves every one of those maps holding the *source* spelling, so each is one
/// more place that has to be found and changed in step — and a miss there is
/// not a compile error in this compiler, it is a wrong lookup: a struct tag
/// declared "not defined", a prototype silently dropped, `long long register
/// _match_expr` in the middle of a `match`. Renaming once, before any of those
/// maps is built, makes them all agree by construction.
///
/// It runs immediately before code generation and NOWHERE EARLIER, so every
/// diagnostic the user sees — type errors, borrow errors, the parser's own
/// refusals — still names the identifier they wrote.
///
/// WHAT IS DELIBERATELY NOT RENAMED
///   * `Import` paths and module names: module resolution already happened, and
///     these are file names, not C identifiers.
///   * `Type::TypeParam` and `type_params`: a type parameter is erased to
///     `void*` by `type_to_c` and its name never reaches the C.
///   * `MacroDef` / `Expr::MacroInvocation` names: macros are expanded before
///     this point, so nothing here survives to be emitted.
///
/// A name in any of those positions that collided with a C keyword would be a
/// separate defect; none of them is a silent one, because the emitted C would
/// name a symbol nothing defines.
pub fn escape_reserved_names(program: &Program) -> Program {
    let mut out = program.clone();
    for item in &mut out.items {
        escape_item(item);
    }
    out
}

fn escape_item(item: &mut Item) {
    match item {
        Item::Function(f) => escape_function(f),
        Item::Struct(s) => {
            escape_in_place(&mut s.name);
            for (field, ty) in &mut s.fields {
                escape_in_place(field);
                escape_type(ty);
            }
            escape_const_params(&mut s.const_params);
        }
        Item::Enum(e) => {
            escape_in_place(&mut e.name);
            for variant in &mut e.variants {
                escape_in_place(&mut variant.name);
                match &mut variant.data {
                    EnumVariantData::Unit => {}
                    EnumVariantData::Tuple(types) => types.iter_mut().for_each(escape_type),
                    EnumVariantData::Struct(fields) => {
                        for (field, ty) in fields {
                            escape_in_place(field);
                            escape_type(ty);
                        }
                    }
                }
            }
            escape_const_params(&mut e.const_params);
        }
        Item::Trait(t) => {
            escape_in_place(&mut t.name);
            for m in &mut t.methods {
                escape_in_place(&mut m.name);
                for p in &mut m.params {
                    escape_in_place(&mut p.name);
                    escape_type(&mut p.ty);
                }
                if let Some(ty) = &mut m.return_type {
                    escape_type(ty);
                }
                if let Some(body) = &mut m.body {
                    escape_block(body);
                }
            }
        }
        Item::Impl(b) => {
            if let Some(ty) = &mut b.trait_type {
                escape_type(ty);
            }
            escape_type(&mut b.for_type);
            for m in &mut b.methods {
                escape_function(m);
            }
        }
        Item::TypeAlias(a) => {
            escape_in_place(&mut a.name);
            escape_type(&mut a.ty);
        }
        // Expanded before code generation; see the doc comment.
        Item::Macro(_) => {}
    }
}

fn escape_function(f: &mut Function) {
    escape_in_place(&mut f.name);
    for p in &mut f.params {
        escape_in_place(&mut p.name);
        escape_type(&mut p.ty);
    }
    if let Some(ty) = &mut f.return_type {
        escape_type(ty);
    }
    escape_const_params(&mut f.const_params);
    escape_block(&mut f.body);
}

/// Const generic parameter names ARE emitted: `type_to_c` prints
/// `ArraySize::ConstParam(name)` straight into the array's bracket suffix.
fn escape_const_params(params: &mut [(String, Type)]) {
    for (name, ty) in params {
        escape_in_place(name);
        escape_type(ty);
    }
}

fn escape_type(ty: &mut Type) {
    match ty {
        Type::Custom(name) => escape_in_place(name),
        Type::Generic { name, args } => {
            escape_in_place(name);
            for arg in args {
                match arg {
                    GenericArg::Type(t) => escape_type(t),
                    GenericArg::Const(ConstValue::ConstParam(n)) => escape_in_place(n),
                    GenericArg::Const(ConstValue::Integer(_)) => {}
                }
            }
        }
        Type::Array(inner, size) => {
            escape_type(inner);
            match size {
                ArraySize::ConstParam(n) => escape_in_place(n),
                ArraySize::Expr(e) => escape_expr(e),
                ArraySize::Literal(_) => {}
            }
        }
        Type::Reference { inner, .. } | Type::Future { output: inner } => escape_type(inner),
        Type::Tuple(types) => types.iter_mut().for_each(escape_type),
        Type::I32
        | Type::I64
        | Type::U32
        | Type::U64
        | Type::F64
        | Type::F32
        | Type::Bool
        | Type::String
        | Type::Unit
        // Erased to `void*`; the name never reaches the C.
        | Type::TypeParam(_) => {}
    }
}

fn escape_block(stmts: &mut [Stmt]) {
    for stmt in stmts {
        escape_stmt(stmt);
    }
}

fn escape_stmt(stmt: &mut Stmt) {
    match stmt {
        Stmt::Expr(e) => escape_expr(e),
        Stmt::Return(e) => {
            if let Some(e) = e {
                escape_expr(e);
            }
        }
        Stmt::Let {
            name, ty, value, ..
        } => {
            escape_in_place(name);
            if let Some(ty) = ty {
                escape_type(ty);
            }
            escape_expr(value);
        }
        Stmt::Assign { target, value, .. } => {
            escape_assign_target(target);
            escape_expr(value);
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            escape_expr(condition);
            escape_block(then_branch);
            if let Some(eb) = else_branch {
                escape_block(eb);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            escape_expr(condition);
            escape_block(body);
        }
        Stmt::Loop { body, .. } => escape_block(body),
        Stmt::For {
            var, iter, body, ..
        } => {
            escape_in_place(var);
            escape_expr(iter);
            escape_block(body);
        }
        Stmt::Match { expr, arms, .. } => {
            escape_expr(expr);
            for arm in arms {
                escape_pattern(&mut arm.pattern);
                escape_block(&mut arm.body);
            }
        }
        Stmt::Unsafe { body, .. } => escape_block(body),
        // A `break` may carry a value, and that value can name a binding
        // whose spelling collides with a C keyword like any other.
        Stmt::Break { value, .. } => {
            if let Some(v) = value {
                escape_expr(v);
            }
        }
        Stmt::Continue { .. } => {}
    }
}

fn escape_pattern(pattern: &mut Pattern) {
    match pattern {
        Pattern::Wildcard => {}
        // A literal pattern holds no identifier, so there is nothing here that
        // could collide with a C keyword.
        Pattern::Literal(_) => {}
        Pattern::Ident(name) => escape_in_place(name),
        Pattern::EnumPattern {
            enum_name,
            variant,
            data,
        } => {
            escape_in_place(enum_name);
            escape_in_place(variant);
            match data {
                None => {}
                Some(PatternData::Tuple(ps)) => ps.iter_mut().for_each(escape_pattern),
                Some(PatternData::Struct(fields)) => {
                    for (field, p) in fields {
                        escape_in_place(field);
                        escape_pattern(p);
                    }
                }
            }
        }
    }
}

fn escape_assign_target(target: &mut AssignTarget) {
    match target {
        AssignTarget::Ident(name) => escape_in_place(name),
        AssignTarget::Index { array, index } => {
            escape_expr(array);
            escape_expr(index);
        }
        AssignTarget::FieldAccess { object, field } => {
            escape_expr(object);
            escape_in_place(field);
        }
        AssignTarget::Deref { expr } => escape_expr(expr),
    }
}

fn escape_expr(expr: &mut Expr) {
    match expr {
        Expr::Ident(name) => escape_in_place(name),
        Expr::ArrayLiteral { elements, .. } => elements.iter_mut().for_each(escape_expr),
        Expr::ArrayRepeat { value, count, .. } => {
            escape_expr(value);
            escape_expr(count);
        }
        Expr::Index { array, index, .. } => {
            escape_expr(array);
            escape_expr(index);
        }
        Expr::Call { func, args, .. } => {
            escape_expr(func);
            args.iter_mut().for_each(escape_expr);
        }
        Expr::Binary { left, right, .. } => {
            escape_expr(left);
            escape_expr(right);
        }
        Expr::Unary { operand, .. } => escape_expr(operand),
        Expr::StructLiteral { name, fields, .. } => {
            escape_in_place(name);
            for (field, value) in fields {
                escape_in_place(field);
                escape_expr(value);
            }
        }
        Expr::FieldAccess { object, field, .. } => {
            escape_expr(object);
            escape_in_place(field);
        }
        Expr::EnumConstructor {
            enum_name,
            variant,
            data,
            ..
        } => {
            escape_in_place(enum_name);
            escape_in_place(variant);
            match data {
                None => {}
                Some(EnumConstructorData::Tuple(es)) => es.iter_mut().for_each(escape_expr),
                Some(EnumConstructorData::Struct(fields)) => {
                    for (field, value) in fields {
                        escape_in_place(field);
                        escape_expr(value);
                    }
                }
            }
        }
        Expr::Range { start, end, .. } => {
            escape_expr(start);
            escape_expr(end);
        }
        Expr::Reference { expr, .. }
        | Expr::Deref { expr, .. }
        | Expr::Question { expr, .. }
        | Expr::Await { expr, .. } => escape_expr(expr),
        // A value `if`/block carries STATEMENTS as well as expressions, and
        // both halves bind names that can collide with a C keyword; walking
        // only the values would leave `let double = 1;` inside a branch
        // unescaped while the same `let` one line out was renamed.
        Expr::If {
            condition,
            then_branch,
            then_value,
            else_branch,
            else_value,
            ..
        } => {
            escape_expr(condition);
            escape_block(then_branch);
            if let Some(v) = then_value {
                escape_expr(v);
            }
            if let Some(eb) = else_branch {
                escape_block(eb);
            }
            if let Some(v) = else_value {
                escape_expr(v);
            }
        }
        Expr::Block { stmts, value, .. } => {
            escape_block(stmts);
            if let Some(v) = value {
                escape_expr(v);
            }
        }
        Expr::Cast { expr, ty, .. } => {
            escape_expr(expr);
            escape_type(ty);
        }
        Expr::Loop { body, .. } => escape_block(body),
        Expr::Match { expr, arms, .. } => {
            escape_expr(expr);
            for arm in arms {
                escape_pattern(&mut arm.pattern);
                escape_block(&mut arm.body);
                if let Some(v) = &mut arm.value {
                    escape_expr(v);
                }
            }
        }
        // Expanded before code generation; see the doc comment.
        Expr::MacroInvocation { .. } => {}
        Expr::String(_) | Expr::Integer(_) | Expr::Float(_) | Expr::Char(_) | Expr::Bool(_) => {}
    }
}

fn escape_in_place(name: &mut String) {
    if let Cow::Owned(escaped) = c_ident(name) {
        *name = escaped;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_for_binary_search() {
        let mut sorted = RESERVED.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            RESERVED,
            &sorted[..],
            "RESERVED is searched with binary_search, so it has to stay sorted; \
             an out-of-order entry makes that lookup MISS and the keyword is \
             emitted verbatim again"
        );
        let mut deduped = sorted.clone();
        deduped.dedup();
        assert_eq!(deduped.len(), RESERVED.len(), "duplicate entry in RESERVED");
    }

    #[test]
    fn a_reserved_word_is_escaped() {
        assert_eq!(c_ident("double"), "double_");
        assert_eq!(c_ident("register"), "register_");
        assert_eq!(c_ident("_Atomic"), "_Atomic_");
    }

    #[test]
    fn an_ordinary_name_is_untouched() {
        // Borrowed, not just equal: every call site is on the hot path of code
        // generation and the common case must not allocate.
        assert!(matches!(c_ident("fibonacci"), Cow::Borrowed("fibonacci")));
        assert!(matches!(c_ident("doubled"), Cow::Borrowed("doubled")));
        assert!(matches!(c_ident("main"), Cow::Borrowed("main")));
        assert!(matches!(c_ident("print_int"), Cow::Borrowed("print_int")));
        // The runtime's own prefixes must not be caught by the underscore rule.
        assert!(matches!(c_ident("__pd_print"), Cow::Borrowed("__pd_print")));
        assert!(matches!(
            c_ident("pd_string_len"),
            Cow::Borrowed("pd_string_len")
        ));
    }

    /// The property that makes this safe: no two source names may become one C
    /// name. A rename that collides turns gcc's loud error into a silent one.
    #[test]
    fn the_escape_is_injective() {
        assert_eq!(c_ident("double_"), "double__");
        assert_eq!(c_ident("double__"), "double___");
        assert_ne!(c_ident("double"), c_ident("double_"));

        // Exhaustively, over every reserved word and three escape depths.
        //
        // The SOURCES are de-duplicated first, because the list deliberately
        // carries both a GNU alternate and its base (`__asm` and `__asm__`),
        // and `__asm` + "__" is `__asm__` — one source reached two ways, not a
        // collision. Feeding it twice would make this test fail on a list that
        // is correct.
        let mut sources = std::collections::BTreeSet::new();
        for word in RESERVED {
            for depth in 0..4 {
                sources.insert(format!("{}{}", word, "_".repeat(depth)));
            }
        }
        let mut seen = std::collections::HashMap::new();
        for src in &sources {
            let out = c_ident(src).into_owned();
            if let Some(other) = seen.insert(out.clone(), src.clone()) {
                panic!("`{}` and `{}` both emit `{}`", other, src, out);
            }
        }
    }

    /// THE OTHER DIRECTION, and the reason `is_escaped_or_reserved` steps one
    /// underscore at a time instead of stripping them all at once.
    ///
    /// Measured with the real `cc`: `__label__` is a keyword and `__label` is
    /// not; the same for `__extension__`/`__extension` and `__func__`/`__func`.
    /// A widened list must not start renaming the ones no compiler objects to.
    #[test]
    fn a_name_that_only_looks_reserved_is_not_escaped() {
        for keyword in ["__label__", "__extension__", "__func__", "__asm__"] {
            assert_ne!(
                c_ident(keyword),
                keyword,
                "`{}` is a keyword and must be escaped",
                keyword
            );
        }
        for ordinary in [
            "__label",
            "__extension",
            "__func",
            "__foo__",
            "__bar",
            "_leading",
            "__pd_print",
        ] {
            assert!(
                matches!(c_ident(ordinary), Cow::Borrowed(_)),
                "`{}` is not a keyword on any toolchain measured, so it must \
                 reach the C exactly as written; got `{}`",
                ordinary,
                c_ident(ordinary)
            );
        }
    }

    /// A bare `_` is not a reserved word once its underscores are stripped, and
    /// the stripping must not panic on it.
    #[test]
    fn underscores_alone_are_not_reserved() {
        assert!(matches!(c_ident("_"), Cow::Borrowed("_")));
        assert!(matches!(c_ident("___"), Cow::Borrowed("___")));
    }

    /// The derived name must carry BOTH of `c_ident`'s properties on its own,
    /// because the AST escape cannot see it: it inspects the source spelling,
    /// and what reaches the C is a function of that spelling.
    #[test]
    fn the_enum_payload_member_is_reserved_safe() {
        // The measured repro: `Register` is not reserved, `register` is, and
        // the old derivation turned one into the other.
        assert_ne!(c_enum_payload_member("Register"), "register");
        // Over the WHOLE list, not a sample: no reserved word survives as a
        // member, and no member is itself reserved.
        for word in RESERVED {
            let m = c_enum_payload_member(word);
            assert_ne!(&*m, *word, "a reserved word must not survive as a member");
            assert!(
                RESERVED.binary_search(&&*m).is_err(),
                "`{}` derived the reserved member `{}`",
                word,
                m
            );
        }
        // A variant whose SOURCE spelling FOLDS onto a reserved word is the
        // whole class, not just the one instance review found.
        for word in ["Register", "Double", "STATIC", "Union", "Short", "Asm"] {
            let m = c_enum_payload_member(word);
            assert!(
                RESERVED.binary_search(&&*m).is_err(),
                "`{}` still derives the reserved member `{}`",
                word,
                m
            );
            assert_ne!(
                &*m,
                word.to_lowercase().as_str(),
                "`{}` must not be case-folded at all",
                word
            );
        }
    }

    /// Case folding is not injective, and a duplicate union member is a SILENT
    /// wrong answer where a distinct one is at worst a loud gcc error.
    #[test]
    fn the_enum_payload_member_is_injective() {
        assert_ne!(
            c_enum_payload_member("Register"),
            c_enum_payload_member("register"),
            "two variants of one enum must not share a union member"
        );
        let names = [
            "Register", "register", "REGISTER", "Plain", "plain", "double", "double_",
        ];
        let mut seen = std::collections::HashSet::new();
        for n in names {
            assert!(
                seen.insert(c_enum_payload_member(n).into_owned()),
                "`{}` collides with an earlier variant",
                n
            );
        }
    }
}
