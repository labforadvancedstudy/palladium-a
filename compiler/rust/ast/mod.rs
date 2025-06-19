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
    pub type_params: Vec<String>, // Generic type parameters like ["T", "U"]
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
    pub type_params: Vec<String>, // Generic type parameters like ["T", "U"]
    pub const_params: Vec<(String, Type)>, // Const parameters like [("N", Type::U64)]
    pub fields: Vec<(String, Type)>,
    pub span: Span,
}

/// Enum definition
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub lifetime_params: Vec<String>, // Lifetime parameters like ["'a", "'b"]
    pub type_params: Vec<String>, // Generic type parameters like ["T", "U"]
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

/// Type alias definition
#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub visibility: Visibility,
    pub name: String,
    pub lifetime_params: Vec<String>, // Lifetime parameters like ["'a", "'b"]
    pub type_params: Vec<String>, // Generic type parameters like ["T", "U"]
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
}

/// Statements
#[derive(Debug, Clone)]
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
    /// For loop
    For {
        var: String,
        iter: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Break statement
    Break { span: Span },
    /// Continue statement
    Continue { span: Span },
    /// Match statement
    Match {
        expr: Expr,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// Unsafe block
    Unsafe {
        body: Vec<Stmt>,
        span: Span,
    },
}

/// Match arm
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Stmt>,
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
    /// Range expression (start..end)
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        span: Span,
    },
    /// Reference expression (&expr or &mut expr)
    Reference {
        mutable: bool,
        expr: Box<Expr>,
        span: Span,
    },
    /// Dereference expression (*expr)
    Deref {
        expr: Box<Expr>,
        span: Span,
    },
    /// Question mark operator (expr?)
    Question {
        expr: Box<Expr>,
        span: Span,
    },
    /// Macro invocation
    MacroInvocation {
        name: String,
        args: Vec<Token>,  // Token stream for arguments
        span: Span,
    },
    /// Await expression
    Await {
        expr: Box<Expr>,
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
#[derive(Debug, Clone)]
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
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Negation (-)
    Neg,
    /// Logical not (!)
    Not,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::String(_) => Span::dummy(), // TODO: track spans
            Expr::Integer(_) => Span::dummy(),
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
    pub params: Vec<String>,  // Macro parameters
    pub body: Vec<Token>,     // Token stream for the macro body
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
    Paren,    // ()
    Brace,    // {}
    Bracket,  // []
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
            Type::Reference { lifetime, mutable, inner } => {
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
            Stmt::For {
                var, iter, body, ..
            } => {
                write!(f, "for {} in {} {{", var, iter)?;
                for stmt in body {
                    write!(f, " {} ", stmt)?;
                }
                write!(f, "}}")
            }
            Stmt::Break { .. } => write!(f, "break;"),
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
            Expr::Bool(b) => write!(f, "{}", b),
            Expr::Ident(name) => write!(f, "{}", name),
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
            Expr::Range { start, end, .. } => {
                write!(f, "{}..{}", start, end)
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
                    write!(f, "{:?}", token)?;  // TODO: better formatting
                }
                write!(f, ")")
            }
            Expr::Await { expr, .. } => write!(f, "{}.await", expr)
        }
    }
}

impl std::fmt::Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
        }
    }
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "-"),
            UnaryOp::Not => write!(f, "!"),
        }
    }
}
