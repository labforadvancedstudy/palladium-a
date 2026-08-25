// Abstract Syntax Tree for Palladium
// "The blueprint of legends"

use crate::errors::Span;

/// The root of a Palladium program
#[derive(Debug, Clone)]
pub struct Program {
    pub imports: Vec<Import>,
    pub items: Vec<Item>,
}

/// Import statement
#[derive(Debug, Clone)]
pub struct Import {
    pub path: Vec<String>,          // e.g., ["std", "math"] for std::math
    pub items: Option<Vec<String>>, // e.g., Some(["pd_abs", "pd_sin"]) for specific imports, None for wildcard
    pub alias: Option<String>,      // e.g., Some("m") for std::math as m
    pub span: Span,
}

/// Top-level items in a program
#[derive(Debug, Clone)]
pub enum Item {
    Function(Function),
    Struct(StructDef),
    Enum(EnumDef),
    Trait(TraitDef),
    Impl(ImplBlock),
    TypeAlias(TypeAlias),
    Macro(MacroDef),
}

/// Visibility modifier
#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

/// Generic parameter (type or const)
#[derive(Debug, Clone)]
pub enum GenericParam {
    /// Type parameter: T
    Type(String),
    /// Const parameter: const N: usize
    Const { name: String, ty: Type },
}

/// Array size (can be a literal or const generic)
#[derive(Debug, Clone, PartialEq)]
pub enum ArraySize {
    /// Literal size: [T; 5]
    Literal(usize),
    /// Const generic parameter: [T; N]
    ConstParam(String),
    /// Expression (for future use): [T; N + 1]
    Expr(Box<Expr>),
}

impl std::fmt::Display for ArraySize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArraySize::Literal(n) => write!(f, "{}", n),
            ArraySize::ConstParam(name) => write!(f, "{}", name),
            ArraySize::Expr(_) => write!(f, "<expr>"), // Placeholder
        }
    }
}

/// Generic argument (can be a type or const value)
#[derive(Debug, Clone, PartialEq)]
pub enum GenericArg {
    /// Type argument: Vec<T>
    Type(Type),
    /// Const argument: Array<T, 5>
    Const(ConstValue),
}

impl std::fmt::Display for GenericArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenericArg::Type(t) => write!(f, "{}", t),
            GenericArg::Const(c) => match c {
                ConstValue::Integer(n) => write!(f, "{}", n),
                ConstValue::ConstParam(name) => write!(f, "{}", name),
            },
        }
    }
}

/// Const value for const generics
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    /// Integer literal
    Integer(i64),
    /// Const parameter reference
    ConstParam(String),
}

/// Function parameter
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

/// Function definition
#[derive(Debug, Clone)]
pub struct Function {
    pub visibility: Visibility,
    pub is_async: bool,
    pub name: String,
    pub lifetime_params: Vec<String>, // Lifetime parameters like ["'a", "'b"]
    pub type_params: Vec<String>,     // Generic type parameters like ["T", "U"]
    pub const_params: Vec<(String, Type)>, // Const parameters like [("N", Type::U64)]
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Vec<Stmt>,
    pub span: Span,
    pub effects: Option<Vec<String>>, // Effect annotations like ["io", "async"]
}

/// Struct definition
#[derive(Debug, Clone)]
pub struct StructDef {
    pub visibility: Visibility,
    pub name: String,
    pub lifetime_params: Vec<String>, // Lifetime parameters like ["'a", "'b"]
    pub type_params: Vec<String>,     // Generic type parameters like ["T", "U"]
    pub const_params: Vec<(String, Type)>, // Const parameters like [("N", Type::U64)]
    pub fields: Vec<(String, Type)>,
    pub span: Span,
}

/// Enum definition
#[derive(Debug, Clone)]
pub struct EnumDef {
    /// Whether `pub` was written. Present since 2026-08-23; before that an
    /// `EnumDef` had no visibility at all, `src/parser/mod.rs` dropped the `pub`
    /// it had just parsed on the floor for this one item kind, and both readers
    /// recorded the gap in prose instead of a field: the resolver exported every
    /// enum with "EnumDef doesn't have a visibility field in the current AST",
    /// and the type checker's imported registration said "Assume all exported
    /// enums are public". A keyword the parser accepts and discards is worse
    /// than one it rejects.
    pub visibility: Visibility,
    pub name: String,
    pub lifetime_params: Vec<String>, // Lifetime parameters like ["'a", "'b"]
    pub type_params: Vec<String>,     // Generic type parameters like ["T", "U"]
    pub const_params: Vec<(String, Type)>, // Const parameters like [("N", Type::U64)]
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

/// Enum variant
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub data: EnumVariantData,
}

/// Enum variant data
#[derive(Debug, Clone)]
pub enum EnumVariantData {
    /// Unit variant (no data)
    Unit,
    /// Tuple variant with types
    Tuple(Vec<Type>),
    /// Struct variant with named fields
    Struct(Vec<(String, Type)>),
}

/// Trait definition
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub visibility: Visibility,
    pub name: String,
    pub lifetime_params: Vec<String>,
    pub type_params: Vec<String>,
    pub methods: Vec<TraitMethod>,
    pub span: Span,
}

/// Trait method
#[derive(Debug, Clone)]
pub struct TraitMethod {
    pub name: String,
    pub lifetime_params: Vec<String>,
    pub type_params: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub has_body: bool,
    pub body: Option<Vec<Stmt>>,
    pub span: Span,
}

/// Implementation block
#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub lifetime_params: Vec<String>,
    pub type_params: Vec<String>,
    pub trait_type: Option<Type>, // None for inherent impl, Some for trait impl
    pub for_type: Type,
    pub methods: Vec<Function>,
    pub span: Span,
}

impl ImplBlock {
    /// This block's methods with every `Self` replaced by the type the block is
    /// for (N5-17).
    ///
    /// WHY THIS EXISTS AT ALL. `fn area(self) -> i64` parses into a parameter
    /// whose type is `Custom("Self")`, and nothing resolved it: the type
    /// checker refused the method with "Unknown struct type: Self", and where
    /// it got past the checker code generation emitted `struct Self self` —
    /// a type nothing declares, so gcc refused C that the front end had
    /// approved. That is the one outcome the conformance runner never accepts.
    ///
    /// WHY IT IS ONE FUNCTION AND NOT A SUBSTITUTION IN EACH PASS. The type
    /// checker and the code generator both walk these methods, and if they
    /// resolved `Self` separately they could resolve it differently — which is
    /// the exact defect class this repository has closed twice already
    /// (`builtins.rs`, `RecursiveLayout`). Both call this.
    ///
    /// The RETURN type was already being substituted, in codegen's
    /// `collect_impl_method_types` and nowhere else, which is why
    /// `fn new(..) -> Self` worked and `fn area(self)` did not.
    pub fn methods_with_self_resolved(&self) -> Vec<Function> {
        self.methods
            .iter()
            .map(|method| {
                let mut method = method.clone();
                for param in &mut method.params {
                    param.ty = substitute_self(&param.ty, &self.for_type);
                }
                method.return_type = method
                    .return_type
                    .as_ref()
                    .map(|ty| substitute_self(ty, &self.for_type));
                method
            })
            .collect()
    }
}

/// Replace `Self` with `for_type`, everywhere inside `ty`.
///
/// Recursive rather than a top-level match, because `Self` can be nested:
/// `&Self`, `[Self; 3]` and `Option<Self>` are all reachable from an impl
/// block's signature, and a shallow replacement would leave the inner one to
/// reach the backend.
pub fn substitute_self(ty: &Type, for_type: &Type) -> Type {
    match ty {
        Type::Custom(name) if name == "Self" => for_type.clone(),
        Type::Reference {
            lifetime,
            mutable,
            inner,
        } => Type::Reference {
            lifetime: lifetime.clone(),
            mutable: *mutable,
            inner: Box::new(substitute_self(inner, for_type)),
        },
        Type::Array(elem, size) => {
            Type::Array(Box::new(substitute_self(elem, for_type)), size.clone())
        }
        Type::Future { output } => Type::Future {
            output: Box::new(substitute_self(output, for_type)),
        },
        Type::Tuple(items) => {
            Type::Tuple(items.iter().map(|t| substitute_self(t, for_type)).collect())
        }
        Type::Generic { name, args } => Type::Generic {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| match arg {
                    GenericArg::Type(t) => GenericArg::Type(substitute_self(t, for_type)),
                    other => other.clone(),
                })
                .collect(),
        },
        other => other.clone(),
    }
}

/// Type alias definition
#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub visibility: Visibility,
    pub name: String,
    pub lifetime_params: Vec<String>, // Lifetime parameters like ["'a", "'b"]
    pub type_params: Vec<String>,     // Generic type parameters like ["T", "U"]
    pub ty: Type,
    pub span: Span,
}

/// Type representation
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Primitive types
    I32,
    I64,
    U32,
    U64,
    /// IEEE-754 binary64 — N4's `f64`, C's `double`.
    F64,
    /// IEEE-754 binary32 — N4's `f32`, C's `float`.
    F32,
    Bool,
    String,
    /// Unit type (void)
    Unit,
    /// Array type: element type and size
    Array(Box<Type>, ArraySize),
    /// Custom type
    Custom(String),
    /// Generic type parameter (e.g., T, U)
    TypeParam(String),
    /// Generic type with concrete arguments (e.g., Vec<i32>, Array<i32, 5>)
    Generic {
        name: String,
        args: Vec<GenericArg>,
    },
    /// Reference type (&T or &mut T)
    Reference {
        lifetime: Option<String>,
        mutable: bool,
        inner: Box<Type>,
    },
    /// Future type for async functions
    Future {
        output: Box<Type>,
    },
    /// Tuple type (T1, T2, ...)
    Tuple(Vec<Type>),
}

/// Does a LOCAL definition of `name` replace an imported one?
///
/// THE ONE DEFINITION OF THAT QUESTION, called by the type checker and by code
/// generation, because they were asking two.
///
/// Typeck said a local TYPE-PARAMETERISED function does not shadow an ordinary
/// import (it is registered in a separate table and only materialises when
/// instantiated). Codegen's own list said it does, and therefore suppressed the
/// imported body — leaving a call resolved by typeck to the imported function
/// with no definition emitted for it. Typeck's rule is the correct one, because
/// it describes what actually replaces the imported body: a function with type
/// parameters emits nothing under its own name, so it replaces nothing.
///
/// THE TEST IS `type_params`, AND ONLY `type_params`. An earlier version of this
/// comment said "a local GENERIC", which is wider than the mechanism: `Function`
/// also carries `lifetime_params` and `const_params` (:111-123), and a function
/// generic in only those axes is NOT deferred by anybody. Both passes route it
/// through the ordinary path — typeck's first pass and `check_function` branch
/// on `type_params.is_empty()` alone (src/typeck/mod.rs), and codegen's
/// main-program loop skips on `!func.type_params.is_empty()` alone
/// (src/codegen/mod.rs) — so it IS emitted under its own name and therefore DOES
/// replace the import.
///
/// MEASURED, both directions, `fn f<'a>()` and `fn f<const N: u64>()` over an
/// imported `pub fn f()`: as written, one `long long f()` is emitted and the
/// program prints the local answer. Widening the predicate to
/// `type_params.is_empty() && const_params.is_empty() && lifetime_params.is_empty()`
/// — i.e. making the code say what the old comment said — emits the imported
/// body as well and gcc reports `redefinition of 'f'`. The claim was wrong, not
/// the mechanism; a control for that direction is
/// `a_lifetime_generic_local_still_shadows_an_import` in tests/d3b_tail_if.rs.
///
/// It lives here rather than in either pass so that "both passes ask one
/// question" is a fact about the call graph and not a claim in a comment.
pub fn local_definition_shadows_import(program: &Program, name: &str) -> bool {
    program
        .items
        .iter()
        .any(|item| matches!(item, Item::Function(f) if f.name == name && f.type_params.is_empty()))
}

/// Does a LOCAL TYPE declaration of `name` replace an imported one?
///
/// The TYPE-namespace counterpart of `local_definition_shadows_import`, and it
/// lives beside it deliberately: "a local declaration wins over an import" is
/// one rule, and a reader who finds one half must find the other half in the
/// same place rather than discovering that the other namespace re-invented it.
///
/// THE DEFECT THAT PRODUCED IT. The type checker decided whether a named type
/// was an `enum` from a bare-name set that was a pure UNION of local and
/// imported enum names — no visibility, no masking. So
///
/// ```text
/// lib.pd:   pub enum Color { Red, Green }
/// main.pd:  import lib;
///           struct Color { v: i64 }
///           fn main() { let c: Color = Color { v: 7 }; print_int(c.v); }
/// ```
///
/// classified the LOCAL `struct Color` as `Enum("Color")` because an imported
/// enum somewhere had claimed the bare name, and was refused with
/// `Type mismatch: expected Color, found Color` — the same diagnostic naming
/// one type on both sides that the recursive-data-types branch was written to
/// remove, reopened through the import path.
///
/// EVERY TYPE-ITEM KIND COUNTS, not just `enum`. The question is "has a local
/// declaration taken this name", and a local `struct`, `enum` or `type` alias
/// all take it. Testing only for a local `enum` would have left the reported
/// program broken, since the local declaration there is a `struct`.
///
/// GENERIC LOCALS COUNT TOO, and this is where it differs from its function
/// sibling. A type-parameterised FUNCTION emits nothing under its own name, so
/// it replaces nothing; a type-parameterised TYPE still occupies the name for
/// every lookup that asks what kind `Color` is, because that question is asked
/// of the name and not of an instantiation.
pub fn local_type_shadows_import(program: &Program, name: &str) -> bool {
    program.items.iter().any(|item| match item {
        Item::Struct(s) => s.name == name,
        Item::Enum(e) => e.name == name,
        Item::TypeAlias(t) => t.name == name,
        _ => false,
    })
}

/// Statements
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Expression statement
    Expr(Expr),
    /// Return statement
    Return(Option<Expr>),
    /// Let binding
    Let {
        name: String,
        ty: Option<Type>,
        value: Expr,
        mutable: bool,
        span: Span,
    },
    /// Assignment statement
    Assign {
        target: AssignTarget,
        value: Expr,
        span: Span,
    },
    /// If statement
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
        span: Span,
    },
    /// While loop
    While {
        condition: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Unconditional loop: `loop { … }` (N5-07).
    ///
    /// NOT sugar for `while true`, even though it lowers to one. `while true`
    /// is an expression the checker has to evaluate and the reader has to
    /// verify; `loop` says "no exit but a `break`" in the grammar, which is
    /// what makes `break <value>` well defined — there is exactly one way out,
    /// so there is exactly one place the value can come from.
    Loop { body: Vec<Stmt>, span: Span },
    /// For loop
    For {
        var: String,
        iter: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Break statement, optionally carrying the value of the `loop` it exits.
    ///
    /// `value` is `Some` only for `break <expr>;`. Which loop it belongs to is
    /// NOT recorded here: there are no loop labels, so a `break` binds to the
    /// innermost enclosing loop and every pass that needs the target keeps its
    /// own stack while it walks (`src/typeck/mod.rs`, `src/codegen/mod.rs`).
    /// Recording it in the node would be a second answer to the same question.
    Break { value: Option<Expr>, span: Span },
    /// Continue statement
    Continue { span: Span },
    /// Match statement
    Match {
        expr: Expr,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// Unsafe block
    Unsafe { body: Vec<Stmt>, span: Span },
}

/// Match arm
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    /// `pattern if cond =>` — the arm's second test (N6-09).
    ///
    /// `None` is an UNGUARDED arm, which is not the same as a guard that is
    /// always true: an unguarded arm counts toward exhaustiveness and a guarded
    /// one never can, because whether it is taken is not decidable from the
    /// pattern.
    pub guard: Option<Expr>,
    pub body: Vec<Stmt>,
}

/// One arm of a `match` in VALUE position: what it matches, what it runs, and
/// what it produces.
///
/// A struct rather than a `Vec<Option<Expr>>` running alongside `Vec<MatchArm>`.
/// The parser already keeps arm tails in a parallel vector (`BlockTail::Match`)
/// and has to check the two lengths agree every time it reads them; carrying
/// that shape into the AST would spread the same check over every consumer.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArmValue {
    pub pattern: Pattern,
    /// The same slot as `MatchArm::guard`, carried across the statement→value
    /// reinterpretation with the pattern and the body. The two structs are kept
    /// field-for-field parallel on purpose (see the type's own note).
    pub guard: Option<Expr>,
    pub body: Vec<Stmt>,
    /// `None` when the arm ends in a statement. Refused in value position by
    /// the type checker, which is where "this arm had to produce something" is
    /// known.
    pub value: Option<Expr>,
}

/// Pattern for matching
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// Wildcard pattern (_)
    Wildcard,
    /// Identifier pattern (binds value)
    Ident(String),
    /// Enum pattern
    EnumPattern {
        enum_name: String,
        variant: String,
        data: Option<PatternData>,
    },
    /// Literal pattern — `0`, `-1`, `"beta"`, `true` (N6-02).
    Literal(PatternLiteral),
    /// Or-pattern — `A | B | C` (N6-07).
    ///
    /// FLAT, NOT NESTED PAIRS. `A | B | C` is one arm accepting three shapes,
    /// and every consumer wants exactly that list: exhaustiveness expands it
    /// into its alternatives, and code generation joins their tests with `||`.
    /// A right-leaning pair tree would make both walk a shape that carries no
    /// information — `|` has no associativity anyone can observe.
    Or(Vec<Pattern>),
    /// Binding pattern — `name @ pattern` (N6-08).
    ///
    /// Names the value AND keeps testing it. Transparent to exhaustiveness: it
    /// covers exactly what `inner` covers.
    Binding {
        name: String,
        inner: Box<Pattern>,
    },
    /// Tuple pattern — `(p1, p2)` (N6-05).
    ///
    /// ARITY IS TWO OR MORE, matching the values of N4-12: `(p)` is refused as
    /// grouping rather than read as a one-element tuple, so the parentheses mean
    /// one thing in pattern position too.
    Tuple(Vec<Pattern>),
    /// Range pattern — `lo .. hi` and `lo ..= hi` (N6-03).
    ///
    /// BOTH ENDPOINTS, ALWAYS. The normative production
    /// (`docs/specification/grammar.ebnf`) gives exactly two range forms and
    /// both are closed; open-ended ranges are named nowhere, so the parser
    /// refuses them rather than this type carrying an `Option` for a form the
    /// language does not have.
    ///
    /// The endpoints are `PatternLiteral` and not `Expr` for N6-02's reason —
    /// one carrier for "a literal in pattern position", and no expression forms
    /// smuggled into a place nothing can evaluate them. Which literals are
    /// ACCEPTABLE is a type question (integers today), so the type checker
    /// answers it and names the type it found.
    Range {
        lo: PatternLiteral,
        hi: PatternLiteral,
        inclusive: bool,
    },
}

/// The literals a pattern may be, and no others.
///
/// A CLOSED THREE-VARIANT ENUM RATHER THAN AN `Expr`. Reusing `Expr` here would
/// put every expression form into pattern position and leave each consumer to
/// re-refuse `match x { f() => … }` on its own; it would also cost `Pattern` its
/// `Eq`/`Hash`, which the exhaustiveness checker's `PatternKind` derives, because
/// `Expr` carries an `f64`. N6-02 names exactly three literal kinds and this is
/// exactly those three. A float pattern is deliberately absent: equality on
/// `f64` is not the relation a reader assumes a pattern means.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PatternLiteral {
    Int(i64),
    Str(String),
    Bool(bool),
}

/// Pattern data for enum variants
#[derive(Debug, Clone, PartialEq)]
pub enum PatternData {
    /// Tuple pattern: Some(x)
    Tuple(Vec<Pattern>),
    /// Struct pattern: Rectangle { width: w, height: h }
    Struct(Vec<(String, Pattern)>),
}

/// Expressions
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// String literal
    String(String),
    /// Integer literal (for future use)
    Integer(i64),
    /// Float literal — `3.5`. Always `f64` (N4 has no literal suffixes and no
    /// context in which `f32` could be inferred from the spelling alone).
    Float(f64),
    /// Char literal — `'a'`, holding the Unicode scalar it denotes.
    ///
    /// KEPT AS A `char` RATHER THAN FOLDED TO ITS CODE POINT AT PARSE TIME.
    /// The value is the same either way; the program text is not, and every
    /// consumer that reads the AST rather than the source (the LSP, the
    /// optimizer's constant folder, any future formatter) has no way back from
    /// `Integer(97)` to `'a'`.
    ///
    /// Its TYPE, however, is `i64` today, not a distinct `char` — see
    /// `src/typeck/mod.rs`. N4-04 (`char` as a primitive type) is a separate,
    /// still-owed row, and it cannot land before N14-04 changes
    /// `string_char_at` to return one.
    Char(char),
    /// Boolean literal
    Bool(bool),
    /// Identifier
    Ident(String),
    /// Array literal
    ArrayLiteral { elements: Vec<Expr>, span: Span },
    /// Array repeat literal [value; count]
    ArrayRepeat {
        value: Box<Expr>,
        count: Box<Expr>,
        span: Span,
    },
    /// Array indexing
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// Function call
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    /// Binary operation (for future use)
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
        span: Span,
    },
    /// Unary operation
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    /// Struct literal
    StructLiteral {
        name: String,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    /// Field access
    FieldAccess {
        object: Box<Expr>,
        field: String,
        span: Span,
    },
    /// Enum constructor
    EnumConstructor {
        enum_name: String,
        variant: String,
        data: Option<EnumConstructorData>,
        span: Span,
    },
    /// Range expression — `start..end` and `start..=end` (N5-14).
    ///
    /// `inclusive` is a flag rather than a second variant because the two
    /// forms differ in exactly one bit of behaviour (whether `end` is visited)
    /// and agree in everything else — their type, their storage, and every
    /// pass that walks them. Two variants would duplicate all of that to
    /// express one boolean.
    ///
    /// Kept as an unnormalised flag rather than lowered to `start..end+1`,
    /// because `end + 1` is not always a number: `0..=i64::MAX` would wrap to
    /// an empty range, quietly.
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
        span: Span,
    },
    /// Reference expression (&expr or &mut expr)
    Reference {
        mutable: bool,
        expr: Box<Expr>,
        span: Span,
    },
    /// Dereference expression (*expr)
    Deref { expr: Box<Expr>, span: Span },
    /// Question mark operator (expr?)
    Question { expr: Box<Expr>, span: Span },
    /// Macro invocation
    MacroInvocation {
        name: String,
        args: Vec<Token>, // Token stream for arguments
        span: Span,
    },
    /// Await expression
    Await { expr: Box<Expr>, span: Span },
    /// `if` in VALUE position — `let x = if c { 1 } else { 2 };` (N5-03).
    ///
    /// Distinct from [`Stmt::If`] rather than a flag on it, because the two
    /// carry different obligations: a statement `if` may have no `else` and
    /// neither branch has to produce anything, while this one must have both
    /// and both must agree on a type. Folding them together would have made
    /// every consumer of `Stmt::If` ask "but is this one the value kind?".
    ///
    /// WHY THE BRANCH VALUE IS SEPARATE FROM THE BRANCH STATEMENTS: a branch
    /// is `{ stmts... ; value }`, and once the tail is inside the `Vec<Stmt>`
    /// nothing downstream can tell it from a `;`-terminated statement — the
    /// same fact that forced `BlockTail` to exist in the parser.
    ///
    /// WHY THE VALUES ARE `Option`: the parser accepts `if c { print("x"); }`
    /// here and the TYPE CHECKER refuses it, so the diagnostic can say
    /// "an `if` used as a value needs an `else`" instead of the parser's
    /// "expected ...". `None` never reaches codegen.
    If {
        condition: Box<Expr>,
        then_branch: Vec<Stmt>,
        then_value: Option<Box<Expr>>,
        else_branch: Option<Vec<Stmt>>,
        else_value: Option<Box<Expr>>,
        span: Span,
    },
    /// `as` cast — `x as i64` (N5-15).
    ///
    /// The target is a `Type` and not a string, because it is parsed by the
    /// ordinary type parser: `as` takes a type, per grammar.ebnf
    /// (`cast_expr = expression "as" type`), and a second spelling of types
    /// here would be a second place for `int` to mean something.
    Cast {
        expr: Box<Expr>,
        ty: Type,
        span: Span,
    },
    /// `loop` in VALUE position — `let x = loop { …; break v; };` (N5-07).
    ///
    /// Its value comes from the `break`s inside `body`, not from a tail
    /// expression, which is why there is no `value` field here and why the type
    /// checker has to walk the body to find one.
    Loop { body: Vec<Stmt>, span: Span },
    /// `match` in VALUE position — `let x = match e { … };` (N5-04).
    ///
    /// Separate from [`Stmt::Match`] for the reason `Expr::If` is separate from
    /// `Stmt::If`: an arm of a statement `match` produces nothing and is under
    /// no obligation to, while every arm of this one must produce a value and
    /// all of them must agree on its type.
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArmValue>,
        span: Span,
    },
    /// A block in VALUE position — `let x = { let a = 1; a + 1 };` (N5-05).
    ///
    /// `value` is the trailing `;`-less expression; `None` is a block that ends
    /// in a statement, which the type checker refuses in value position.
    Block {
        stmts: Vec<Stmt>,
        value: Option<Box<Expr>>,
        span: Span,
    },
    /// Tuple construction — `(a, b)` (N4-12).
    ///
    /// ARITY IS TWO OR MORE. `(e)` is grouping, which every program in this
    /// corpus already relies on, so a one-element tuple would have to be `(e,)`
    /// — a spelling whose only job is to disambiguate against grouping. The
    /// parser refuses it by name rather than giving the same parentheses two
    /// meanings decided by a trailing comma.
    Tuple {
        elements: Vec<Expr>,
        span: Span,
    },
    /// Tuple element access — `p.0` (N4-12).
    ///
    /// The index is a `usize` and not an expression: `.0` is SYNTAX. A tuple's
    /// elements may have different types, so an index the compiler cannot read
    /// at compile time has no type to be.
    TupleIndex {
        expr: Box<Expr>,
        index: usize,
        span: Span,
    },
}

/// Enum constructor data
#[derive(Debug, Clone, PartialEq)]
pub enum EnumConstructorData {
    /// Tuple constructor: Color::Red(255)
    Tuple(Vec<Expr>),
    /// Struct constructor: Shape::Rectangle { width: 10, height: 20 }
    Struct(Vec<(String, Expr)>),
}

/// Assignment targets
#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    /// Simple variable assignment
    Ident(String),
    /// Array element assignment
    Index { array: Box<Expr>, index: Box<Expr> },
    /// Field assignment
    FieldAccess { object: Box<Expr>, field: String },
    /// Dereference assignment (*ptr = value)
    Deref { expr: Box<Expr> },
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    /// Bitwise `&`, `|`, `^` and the shifts (N5-12). Integer-only; the type
    /// checker refuses every other operand type by name.
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Negation (-)
    Neg,
    /// Logical not (!)
    Not,
    /// Bitwise complement (~) — N5-12. Distinct from `Not`: `!` is a
    /// truth-value operator over `bool` and `~` flips the bits of an integer,
    /// and folding them together would make `!0` and `~0` the same expression
    /// with two different answers.
    BitNot,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::String(_) => Span::dummy(), // TODO: track spans
            Expr::Integer(_) => Span::dummy(),
            Expr::Float(_) => Span::dummy(),
            Expr::Char(_) => Span::dummy(),
            Expr::Bool(_) => Span::dummy(),
            Expr::Ident(_) => Span::dummy(),
            Expr::ArrayLiteral { span, .. } => *span,
            Expr::ArrayRepeat { span, .. } => *span,
            Expr::Index { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
            Expr::Unary { span, .. } => *span,
            Expr::StructLiteral { span, .. } => *span,
            Expr::FieldAccess { span, .. } => *span,
            Expr::EnumConstructor { span, .. } => *span,
            Expr::Range { span, .. } => *span,
            Expr::Reference { span, .. } => *span,
            Expr::Deref { span, .. } => *span,
            Expr::Question { span, .. } => *span,
            Expr::MacroInvocation { span, .. } => *span,
            Expr::Await { span, .. } => *span,
            Expr::If { span, .. } => *span,
            Expr::Block { span, .. } => *span,
            Expr::Tuple { span, .. } => *span,
            Expr::TupleIndex { span, .. } => *span,
            Expr::Cast { span, .. } => *span,
            Expr::Loop { span, .. } => *span,
            Expr::Match { span, .. } => *span,
        }
    }
}

/// AST visitor trait for traversing the tree
pub trait Visitor<T> {
    fn visit_program(&mut self, program: &Program) -> T;
    fn visit_function(&mut self, func: &Function) -> T;
    fn visit_stmt(&mut self, stmt: &Stmt) -> T;
    fn visit_expr(&mut self, expr: &Expr) -> T;
}

/// Pretty printing for AST nodes
impl std::fmt::Display for Program {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in &self.items {
            writeln!(f, "{}", item)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::Function(func) => write!(f, "{}", func),
            Item::Struct(struct_def) => write!(f, "{}", struct_def),
            Item::Enum(enum_def) => write!(f, "{}", enum_def),
            Item::Trait(trait_def) => write!(f, "{}", trait_def),
            Item::Impl(impl_block) => write!(f, "{}", impl_block),
            Item::TypeAlias(type_alias) => write!(f, "{}", type_alias),
            Item::Macro(macro_def) => write!(f, "{}", macro_def),
        }
    }
}

impl std::fmt::Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fn {}(", self.name)?;
        for (i, param) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            if param.mutable {
                write!(f, "mut ")?;
            }
            write!(f, "{}: {}", param.name, param.ty)?;
        }
        write!(f, ")")?;
        if let Some(ret_type) = &self.return_type {
            write!(f, " -> {}", ret_type)?;
        }
        writeln!(f, " {{")?;
        for stmt in &self.body {
            writeln!(f, "    {}", stmt)?;
        }
        write!(f, "}}")
    }
}

impl std::fmt::Display for StructDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "struct {} {{", self.name)?;
        for (i, (field_name, field_type)) in self.fields.iter().enumerate() {
            if i == 0 {
                writeln!(f)?;
            }
            writeln!(f, "    {}: {},", field_name, field_type)?;
        }
        write!(f, "}}")
    }
}

impl std::fmt::Display for EnumDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "enum {} {{", self.name)?;
        for (i, variant) in self.variants.iter().enumerate() {
            if i == 0 {
                writeln!(f)?;
            }
            write!(f, "    {}", variant.name)?;
            match &variant.data {
                EnumVariantData::Unit => {}
                EnumVariantData::Tuple(types) => {
                    write!(f, "(")?;
                    for (j, ty) in types.iter().enumerate() {
                        if j > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", ty)?;
                    }
                    write!(f, ")")?;
                }
                EnumVariantData::Struct(fields) => {
                    write!(f, " {{ ")?;
                    for (j, (fname, ftype)) in fields.iter().enumerate() {
                        if j > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}: {}", fname, ftype)?;
                    }
                    write!(f, " }}")?;
                }
            }
            writeln!(f, ",")?;
        }
        write!(f, "}}")
    }
}

/// Macro definition
#[derive(Debug, Clone)]
pub struct MacroDef {
    pub name: String,
    pub params: Vec<String>, // Macro parameters
    pub body: Vec<Token>,    // Token stream for the macro body
    pub span: Span,
}

/// Token for macro expansion (simplified token representation)
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    Literal(String),
    Punct(char),
    Group(Delimiter, Vec<Token>),
}

/// Delimiter for grouped tokens
#[derive(Debug, Clone, PartialEq)]
pub enum Delimiter {
    Paren,   // ()
    Brace,   // {}
    Bracket, // []
}

impl std::fmt::Display for MacroDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "macro {}! {{ ... }}", self.name)
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::I32 => write!(f, "i32"),
            Type::I64 => write!(f, "i64"),
            Type::U32 => write!(f, "u32"),
            Type::U64 => write!(f, "u64"),
            Type::F64 => write!(f, "f64"),
            Type::F32 => write!(f, "f32"),
            Type::Bool => write!(f, "bool"),
            Type::String => write!(f, "String"),
            Type::Unit => write!(f, "()"),
            Type::Array(elem_type, size) => write!(f, "[{}; {}]", elem_type, size),
            Type::Custom(name) => write!(f, "{}", name),
            Type::TypeParam(name) => write!(f, "{}", name),
            Type::Generic { name, args } => {
                write!(f, "{}<", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ">")
            }
            Type::Reference {
                lifetime,
                mutable,
                inner,
            } => {
                write!(f, "&")?;
                if let Some(lt) = lifetime {
                    write!(f, "'{} ", lt)?;
                }
                if *mutable {
                    write!(f, "mut ")?;
                }
                write!(f, "{}", inner)
            }
            Type::Future { output } => write!(f, "Future<{}>", output),
            Type::Tuple(types) => {
                write!(f, "(")?;
                for (i, ty) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", ty)?;
                }
                write!(f, ")")
            }
        }
    }
}

impl std::fmt::Display for TraitDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vis = match self.visibility {
            Visibility::Public => "pub ",
            Visibility::Private => "",
        };
        write!(f, "{}trait {}", vis, self.name)?;

        // Generic parameters
        if !self.lifetime_params.is_empty() || !self.type_params.is_empty() {
            write!(f, "<")?;
            let mut first = true;
            for lt in &self.lifetime_params {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}", lt)?;
                first = false;
            }
            for tp in &self.type_params {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}", tp)?;
                first = false;
            }
            write!(f, ">")?;
        }

        writeln!(f, " {{")?;
        for method in &self.methods {
            write!(f, "    fn {}", method.name)?;

            // Method generic parameters
            if !method.lifetime_params.is_empty() || !method.type_params.is_empty() {
                write!(f, "<")?;
                let mut first = true;
                for lt in &method.lifetime_params {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", lt)?;
                    first = false;
                }
                for tp in &method.type_params {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", tp)?;
                    first = false;
                }
                write!(f, ">")?;
            }

            write!(f, "(")?;
            for (i, param) in method.params.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                if param.mutable {
                    write!(f, "mut ")?;
                }
                write!(f, "{}: {}", param.name, param.ty)?;
            }
            write!(f, ")")?;

            if let Some(ret) = &method.return_type {
                write!(f, " -> {}", ret)?;
            }

            if method.has_body {
                writeln!(f, " {{ ... }}")?;
            } else {
                writeln!(f, ";")?;
            }
        }
        write!(f, "}}")
    }
}

impl std::fmt::Display for ImplBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "impl")?;

        // Generic parameters
        if !self.lifetime_params.is_empty() || !self.type_params.is_empty() {
            write!(f, "<")?;
            let mut first = true;
            for lt in &self.lifetime_params {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}", lt)?;
                first = false;
            }
            for tp in &self.type_params {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}", tp)?;
                first = false;
            }
            write!(f, ">")?;
        }

        if let Some(trait_type) = &self.trait_type {
            write!(f, " {} for", trait_type)?;
        }

        write!(f, " {} {{", self.for_type)?;

        for method in &self.methods {
            writeln!(f)?;
            write!(f, "    {}", method)?;
        }

        write!(f, "\n}}")
    }
}

impl std::fmt::Display for TypeAlias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vis = match self.visibility {
            Visibility::Public => "pub ",
            Visibility::Private => "",
        };
        write!(f, "{}type {}", vis, self.name)?;

        // Generic parameters
        if !self.lifetime_params.is_empty() || !self.type_params.is_empty() {
            write!(f, "<")?;
            let mut first = true;
            for lt in &self.lifetime_params {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}", lt)?;
                first = false;
            }
            for tp in &self.type_params {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}", tp)?;
                first = false;
            }
            write!(f, ">")?;
        }

        write!(f, " = {};", self.ty)
    }
}

impl std::fmt::Display for Stmt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stmt::Expr(expr) => write!(f, "{};", expr),
            Stmt::Return(None) => write!(f, "return;"),
            Stmt::Return(Some(expr)) => write!(f, "return {};", expr),
            Stmt::Let {
                name,
                ty,
                value,
                mutable,
                ..
            } => {
                let mut_str = if *mutable { "mut " } else { "" };
                if let Some(ty) = ty {
                    write!(f, "let {}{}: {} = {};", mut_str, name, ty, value)
                } else {
                    write!(f, "let {}{} = {};", mut_str, name, value)
                }
            }
            Stmt::Assign { target, value, .. } => match target {
                AssignTarget::Ident(name) => write!(f, "{} = {};", name, value),
                AssignTarget::Index { array, index } => {
                    write!(f, "{}[{}] = {};", array, index, value)
                }
                AssignTarget::FieldAccess { object, field } => {
                    write!(f, "{}.{} = {};", object, field, value)
                }
                AssignTarget::Deref { expr } => {
                    write!(f, "*{} = {};", expr, value)
                }
            },
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                write!(f, "if {} {{", condition)?;
                for stmt in then_branch {
                    write!(f, " {} ", stmt)?;
                }
                write!(f, "}}")?;
                if let Some(else_stmts) = else_branch {
                    write!(f, " else {{")?;
                    for stmt in else_stmts {
                        write!(f, " {} ", stmt)?;
                    }
                    write!(f, "}}")?;
                }
                Ok(())
            }
            Stmt::While {
                condition, body, ..
            } => {
                write!(f, "while {} {{", condition)?;
                for stmt in body {
                    write!(f, " {} ", stmt)?;
                }
                write!(f, "}}")
            }
            Stmt::Loop { body, .. } => {
                write!(f, "loop {{")?;
                for stmt in body {
                    write!(f, " {} ", stmt)?;
                }
                write!(f, "}}")
            }
            Stmt::For {
                var, iter, body, ..
            } => {
                write!(f, "for {} in {} {{", var, iter)?;
                for stmt in body {
                    write!(f, " {} ", stmt)?;
                }
                write!(f, "}}")
            }
            Stmt::Break { value: Some(v), .. } => write!(f, "break {};", v),
            Stmt::Break { value: None, .. } => write!(f, "break;"),
            Stmt::Continue { .. } => write!(f, "continue;"),
            Stmt::Match { expr, arms, .. } => {
                writeln!(f, "match {} {{", expr)?;
                for arm in arms {
                    write!(f, "    {} => ", arm.pattern)?;
                    if arm.body.len() == 1 {
                        if let Stmt::Expr(e) = &arm.body[0] {
                            writeln!(f, "{},", e)?;
                        } else {
                            writeln!(f, "{{")?;
                            for stmt in &arm.body {
                                writeln!(f, "        {}", stmt)?;
                            }
                            writeln!(f, "    }}")?;
                        }
                    } else {
                        writeln!(f, "{{")?;
                        for stmt in &arm.body {
                            writeln!(f, "        {}", stmt)?;
                        }
                        writeln!(f, "    }}")?;
                    }
                }
                write!(f, "}}")
            }
            Stmt::Unsafe { body, .. } => {
                writeln!(f, "unsafe {{")?;
                for stmt in body {
                    writeln!(f, "    {}", stmt)?;
                }
                write!(f, "}}")
            }
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::String(s) => write!(f, "\"{}\"", s),
            Expr::Integer(n) => write!(f, "{}", n),
            // `{}` on an f64 prints `3` for 3.0, which is an integer literal in
            // every language this project's readers know. `{:?}` keeps the dot.
            Expr::Float(x) => write!(f, "{:?}", x),
            Expr::Char(c) => write!(f, "'{}'", c.escape_debug()),
            Expr::Bool(b) => write!(f, "{}", b),
            Expr::Ident(name) => write!(f, "{}", name),
            Expr::Tuple { elements, .. } => {
                let parts: Vec<String> = elements.iter().map(|e| e.to_string()).collect();
                write!(f, "({})", parts.join(", "))
            }
            Expr::TupleIndex { expr, index, .. } => write!(f, "{}.{}", expr, index),
            Expr::ArrayLiteral { elements, .. } => {
                write!(f, "[")?;
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, "]")
            }
            Expr::ArrayRepeat { value, count, .. } => {
                write!(f, "[{}; {}]", value, count)
            }
            Expr::Index { array, index, .. } => {
                write!(f, "{}[{}]", array, index)
            }
            Expr::Call { func, args, .. } => {
                write!(f, "{}(", func)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                write!(f, "({} {} {})", left, op, right)
            }
            Expr::Unary { op, operand, .. } => {
                write!(f, "({}{})", op, operand)
            }
            Expr::StructLiteral { name, fields, .. } => {
                write!(f, "{} {{ ", name)?;
                for (i, (field_name, field_expr)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", field_name, field_expr)?;
                }
                write!(f, " }}")
            }
            Expr::FieldAccess { object, field, .. } => {
                write!(f, "{}.{}", object, field)
            }
            Expr::EnumConstructor {
                enum_name,
                variant,
                data,
                ..
            } => {
                write!(f, "{}::{}", enum_name, variant)?;
                match data {
                    Some(EnumConstructorData::Tuple(args)) => {
                        write!(f, "(")?;
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", arg)?;
                        }
                        write!(f, ")")
                    }
                    Some(EnumConstructorData::Struct(fields)) => {
                        write!(f, " {{ ")?;
                        for (i, (fname, fexpr)) in fields.iter().enumerate() {
                            if i > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}: {}", fname, fexpr)?;
                        }
                        write!(f, " }}")
                    }
                    None => Ok(()),
                }
            }
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                write!(f, "{}..{}{}", start, if *inclusive { "=" } else { "" }, end)
            }
            Expr::Reference { mutable, expr, .. } => {
                if *mutable {
                    write!(f, "&mut {}", expr)
                } else {
                    write!(f, "&{}", expr)
                }
            }
            Expr::Deref { expr, .. } => {
                write!(f, "*{}", expr)
            }
            Expr::Question { expr, .. } => {
                write!(f, "{}?", expr)
            }
            Expr::MacroInvocation { name, args, .. } => {
                write!(f, "{}!(", name)?;
                for (i, token) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{:?}", token)?; // TODO: better formatting
                }
                write!(f, ")")
            }
            Expr::Await { expr, .. } => write!(f, "{}.await", expr),
            // The STATEMENTS of a value block are not printed. This `Display`
            // is used in diagnostics, where the interesting part is the value
            // and a multi-statement body would run the message off the line;
            // `...` says something was elided rather than pretending the block
            // was empty.
            Expr::If {
                condition,
                then_branch,
                then_value,
                else_branch,
                else_value,
                ..
            } => {
                write!(f, "if {} {{ ", condition)?;
                if !then_branch.is_empty() {
                    write!(f, "... ")?;
                }
                match then_value {
                    Some(v) => write!(f, "{} }}", v)?,
                    None => write!(f, "}}")?,
                }
                match else_branch {
                    Some(stmts) => {
                        write!(f, " else {{ ")?;
                        if !stmts.is_empty() {
                            write!(f, "... ")?;
                        }
                        match else_value {
                            Some(v) => write!(f, "{} }}", v),
                            None => write!(f, "}}"),
                        }
                    }
                    None => Ok(()),
                }
            }
            Expr::Cast { expr, ty, .. } => write!(f, "{} as {}", expr, ty),
            Expr::Loop { body, .. } => {
                write!(f, "loop {{")?;
                if !body.is_empty() {
                    write!(f, " ... ")?;
                }
                write!(f, "}}")
            }
            Expr::Match { expr, arms, .. } => {
                write!(f, "match {} {{ ", expr)?;
                for arm in arms {
                    write!(f, "{} => ", arm.pattern)?;
                    match &arm.value {
                        Some(v) => write!(f, "{}, ", v)?,
                        None => write!(f, "..., ")?,
                    }
                }
                write!(f, "}}")
            }
            Expr::Block { stmts, value, .. } => {
                write!(f, "{{ ")?;
                if !stmts.is_empty() {
                    write!(f, "... ")?;
                }
                match value {
                    Some(v) => write!(f, "{} }}", v),
                    None => write!(f, "}}"),
                }
            }
        }
    }
}

impl std::fmt::Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Pattern::Or(alternatives) => {
                let parts: Vec<String> = alternatives.iter().map(|p| p.to_string()).collect();
                write!(f, "{}", parts.join(" | "))
            }
            Pattern::Binding { name, inner } => write!(f, "{} @ {}", name, inner),
            Pattern::Tuple(elements) => {
                let parts: Vec<String> = elements.iter().map(|p| p.to_string()).collect();
                write!(f, "({})", parts.join(", "))
            }
            Pattern::Range { lo, hi, inclusive } => write!(
                f,
                "{}{}{}",
                Pattern::Literal(lo.clone()),
                if *inclusive { "..=" } else { ".." },
                Pattern::Literal(hi.clone())
            ),
            Pattern::Literal(PatternLiteral::Int(v)) => write!(f, "{}", v),
            Pattern::Literal(PatternLiteral::Str(v)) => write!(f, "{:?}", v),
            Pattern::Literal(PatternLiteral::Bool(v)) => write!(f, "{}", v),
            Pattern::Wildcard => write!(f, "_"),
            Pattern::Ident(name) => write!(f, "{}", name),
            Pattern::EnumPattern {
                enum_name,
                variant,
                data,
            } => {
                write!(f, "{}::{}", enum_name, variant)?;
                match data {
                    Some(PatternData::Tuple(patterns)) => {
                        write!(f, "(")?;
                        for (i, pattern) in patterns.iter().enumerate() {
                            if i > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", pattern)?;
                        }
                        write!(f, ")")
                    }
                    Some(PatternData::Struct(field_patterns)) => {
                        write!(f, " {{ ")?;
                        for (i, (field_name, pattern)) in field_patterns.iter().enumerate() {
                            if i > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}: {}", field_name, pattern)?;
                        }
                        write!(f, " }}")
                    }
                    None => Ok(()),
                }
            }
        }
    }
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Mod => write!(f, "%"),
            BinOp::Eq => write!(f, "=="),
            BinOp::Ne => write!(f, "!="),
            BinOp::Lt => write!(f, "<"),
            BinOp::Gt => write!(f, ">"),
            BinOp::Le => write!(f, "<="),
            BinOp::Ge => write!(f, ">="),
            BinOp::And => write!(f, "&&"),
            BinOp::Or => write!(f, "||"),
            BinOp::BitAnd => write!(f, "&"),
            BinOp::BitOr => write!(f, "|"),
            BinOp::BitXor => write!(f, "^"),
            BinOp::Shl => write!(f, "<<"),
            BinOp::Shr => write!(f, ">>"),
        }
    }
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "-"),
            UnaryOp::Not => write!(f, "!"),
            UnaryOp::BitNot => write!(f, "~"),
        }
    }
}
