// Abstract Syntax Tree for Palladium
// "The blueprint of legends"

use crate::errors::Span;

/// The root of a Palladium program
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

/// Top-level items in a program
#[derive(Debug, Clone)]
pub enum Item {
    Function(Function),
}

/// Function definition
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub return_type: Option<Type>,
    pub body: Vec<Stmt>,
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
    /// Custom type
    Custom(String),
}

/// Statements
#[derive(Debug, Clone)]
pub enum Stmt {
    /// Expression statement
    Expr(Expr),
    /// Return statement
    Return(Option<Expr>),
    /// Let binding (for future use)
    Let {
        name: String,
        value: Expr,
        span: Span,
    },
}

/// Expressions
#[derive(Debug, Clone)]
pub enum Expr {
    /// String literal
    String(String),
    /// Integer literal (for future use)
    Integer(i64),
    /// Identifier
    Ident(String),
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
}

/// Binary operators (for future use)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::String(_) => Span::dummy(), // TODO: track spans
            Expr::Integer(_) => Span::dummy(),
            Expr::Ident(_) => Span::dummy(),
            Expr::Call { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
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
            write!(f, "{}", param)?;
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
            Type::Custom(name) => write!(f, "{}", name),
        }
    }
}

impl std::fmt::Display for Stmt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stmt::Expr(expr) => write!(f, "{};", expr),
            Stmt::Return(None) => write!(f, "return;"),
            Stmt::Return(Some(expr)) => write!(f, "return {};", expr),
            Stmt::Let { name, value, .. } => write!(f, "let {} = {};", name, value),
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::String(s) => write!(f, "\"{}\"", s),
            Expr::Integer(n) => write!(f, "{}", n),
            Expr::Ident(name) => write!(f, "{}", name),
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
            Expr::Binary { left, op, right, .. } => {
                write!(f, "({} {} {})", left, op, right)
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
            BinOp::Eq => write!(f, "=="),
            BinOp::Ne => write!(f, "!="),
            BinOp::Lt => write!(f, "<"),
            BinOp::Gt => write!(f, ">"),
            BinOp::Le => write!(f, "<="),
            BinOp::Ge => write!(f, ">="),
        }
    }
}