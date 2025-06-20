// Code analysis utilities for LSP
// "Deep insights into your legendary code"

use super::LanguageServer;
use crate::ast::{Expr, Item, Program, Stmt, Type};
use std::collections::{HashMap, HashSet};

/// Code analysis results
pub struct AnalysisResults {
    /// Unused variables
    pub unused_vars: HashSet<String>,
    /// Unreachable code locations
    pub unreachable_code: Vec<crate::errors::Span>,
    /// Type mismatches
    pub type_errors: Vec<TypeError>,
    /// Missing imports
    pub missing_imports: Vec<String>,
}

/// Type error information
pub struct TypeError {
    pub expected: Type,
    pub found: Type,
    pub span: crate::errors::Span,
    pub message: String,
}

impl LanguageServer {
    /// Perform semantic analysis on a program
    pub fn analyze_program(&self, program: &Program) -> AnalysisResults {
        let mut results = AnalysisResults {
            unused_vars: HashSet::new(),
            unreachable_code: Vec::new(),
            type_errors: Vec::new(),
            missing_imports: Vec::new(),
        };

        // Analyze each item
        for item in &program.items {
            self.analyze_item(item, &mut results);
        }

        results
    }

    /// Analyze an item
    fn analyze_item(&self, item: &Item, results: &mut AnalysisResults) {
        match item {
            Item::Function(func) => {
                let mut analyzer = FunctionAnalyzer::new();
                analyzer.analyze_function(func, results);
            }
            Item::Trait(trait_def) => {
                for method in &trait_def.methods {
                    let mut analyzer = FunctionAnalyzer::new();
                    analyzer.analyze_method(method, results);
                }
            }
            Item::Impl(impl_block) => {
                for method in &impl_block.methods {
                    let mut analyzer = FunctionAnalyzer::new();
                    analyzer.analyze_function(method, results);
                }
            }
            _ => {}
        }
    }

    /// Get semantic tokens for syntax highlighting
    pub fn get_semantic_tokens(&self, ast: &Program) -> Vec<SemanticToken> {
        let mut tokens = Vec::new();
        let builder = SemanticTokenBuilder::new();

        for item in &ast.items {
            builder.process_item(item, &mut tokens);
        }

        tokens.sort_by_key(|t| (t.line, t.character));
        tokens
    }
}

/// Semantic token for syntax highlighting
#[derive(Debug, Clone)]
pub struct SemanticToken {
    pub line: u32,
    pub character: u32,
    pub length: u32,
    pub token_type: SemanticTokenType,
    pub modifiers: Vec<SemanticTokenModifier>,
}

/// Semantic token types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticTokenType {
    Type,
    Class,
    Enum,
    Interface,
    Struct,
    TypeParameter,
    Parameter,
    Variable,
    Property,
    EnumMember,
    Function,
    Method,
    Macro,
    Keyword,
    Comment,
    String,
    Number,
    Operator,
}

/// Semantic token modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticTokenModifier {
    Declaration,
    Definition,
    Readonly,
    Static,
    Deprecated,
    Abstract,
    Async,
    Modification,
    Documentation,
    DefaultLibrary,
}

/// Function analyzer
struct FunctionAnalyzer {
    declared_vars: HashMap<String, crate::errors::Span>,
    used_vars: HashSet<String>,
    has_return: bool,
}

impl FunctionAnalyzer {
    fn new() -> Self {
        Self {
            declared_vars: HashMap::new(),
            used_vars: HashSet::new(),
            has_return: false,
        }
    }

    fn analyze_function(&mut self, func: &crate::ast::Function, results: &mut AnalysisResults) {
        // Add parameters to declared vars
        for param in &func.params {
            self.declared_vars.insert(
                param.name.clone(),
                crate::errors::Span {
                    start: 0,
                    end: 0,
                    line: 0,
                    column: 0,
                },
            );
        }

        // Analyze body
        self.analyze_statements(&func.body, results);

        // Check for unused variables
        for var in self.declared_vars.keys() {
            if !self.used_vars.contains(var) && !var.starts_with('_') {
                results.unused_vars.insert(var.clone());
            }
        }

        // Check for missing return
        if func.return_type.is_some() && !self.has_return {
            // TODO: Add error for missing return
        }
    }

    fn analyze_method(&mut self, method: &crate::ast::TraitMethod, results: &mut AnalysisResults) {
        // Add parameters to declared vars
        for param in &method.params {
            self.declared_vars.insert(
                param.name.clone(),
                crate::errors::Span {
                    start: 0,
                    end: 0,
                    line: 0,
                    column: 0,
                },
            );
        }

        // Analyze body
        if let Some(body) = &method.body {
            self.analyze_statements(body, results);
        }
    }

    fn analyze_statements(&mut self, stmts: &[Stmt], results: &mut AnalysisResults) {
        let mut unreachable = false;

        for stmt in stmts {
            if unreachable {
                results.unreachable_code.push(self.get_statement_span(stmt));
            }

            match stmt {
                Stmt::Let {
                    name, value, span, ..
                } => {
                    self.declared_vars.insert(name.clone(), *span);
                    self.analyze_expression(value);
                }
                Stmt::Return(_) => {
                    self.has_return = true;
                    unreachable = true;
                }
                Stmt::Expr(expr) => {
                    self.analyze_expression(expr);
                }
                Stmt::If {
                    condition,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.analyze_expression(condition);
                    self.analyze_statements(then_branch, results);
                    if let Some(else_branch) = else_branch {
                        self.analyze_statements(else_branch, results);
                    }
                }
                Stmt::While {
                    condition, body, ..
                } => {
                    self.analyze_expression(condition);
                    self.analyze_statements(body, results);
                }
                Stmt::For {
                    var,
                    iter,
                    body,
                    span,
                    ..
                } => {
                    self.declared_vars.insert(var.clone(), *span);
                    self.analyze_expression(iter);
                    self.analyze_statements(body, results);
                }
                _ => {}
            }
        }
    }

    fn analyze_expression(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(name) => {
                self.used_vars.insert(name.clone());
            }
            Expr::Call { func, args, .. } => {
                self.analyze_expression(func);
                for arg in args {
                    self.analyze_expression(arg);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.analyze_expression(left);
                self.analyze_expression(right);
            }
            Expr::Unary { operand, .. } => {
                self.analyze_expression(operand);
            }
            Expr::FieldAccess { object, .. } => {
                self.analyze_expression(object);
            }
            Expr::Index { array, index, .. } => {
                self.analyze_expression(array);
                self.analyze_expression(index);
            }
            _ => {}
        }
    }

    fn get_statement_span(&self, stmt: &Stmt) -> crate::errors::Span {
        match stmt {
            Stmt::Let { span, .. } => *span,
            Stmt::Expr(expr) => self.get_expression_span(expr),
            Stmt::Return(expr) => {
                if let Some(expr) = expr {
                    self.get_expression_span(expr)
                } else {
                    crate::errors::Span {
                        start: 0,
                        end: 0,
                        line: 0,
                        column: 0,
                    }
                }
            }
            Stmt::If { span, .. } => *span,
            Stmt::While { span, .. } => *span,
            Stmt::For { span, .. } => *span,
            Stmt::Match { span, .. } => *span,
            _ => crate::errors::Span {
                start: 0,
                end: 0,
                line: 0,
                column: 0,
            },
        }
    }

    fn get_expression_span(&self, expr: &Expr) -> crate::errors::Span {
        match expr {
            Expr::Ident(_) => crate::errors::Span {
                start: 0,
                end: 0,
                line: 0,
                column: 0,
            },
            Expr::Integer(_) => crate::errors::Span {
                start: 0,
                end: 0,
                line: 0,
                column: 0,
            },
            Expr::String(_) => crate::errors::Span {
                start: 0,
                end: 0,
                line: 0,
                column: 0,
            },
            Expr::Bool(_) => crate::errors::Span {
                start: 0,
                end: 0,
                line: 0,
                column: 0,
            },
            Expr::Call { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
            Expr::Unary { span, .. } => *span,
            Expr::FieldAccess { span, .. } => *span,
            Expr::Index { span, .. } => *span,
            Expr::ArrayLiteral { span, .. } => *span,
            Expr::ArrayRepeat { span, .. } => *span,
            Expr::StructLiteral { span, .. } => *span,
            Expr::EnumConstructor { span, .. } => *span,
            _ => crate::errors::Span {
                start: 0,
                end: 0,
                line: 0,
                column: 0,
            },
        }
    }
}

/// Semantic token builder
struct SemanticTokenBuilder;

impl SemanticTokenBuilder {
    fn new() -> Self {
        Self
    }

    fn process_item(&self, item: &Item, tokens: &mut Vec<SemanticToken>) {
        match item {
            Item::Function(func) => {
                // Function name
                tokens.push(SemanticToken {
                    line: func.span.start as u32,
                    character: 0, // TODO: Calculate character offset
                    length: func.name.len() as u32,
                    token_type: SemanticTokenType::Function,
                    modifiers: vec![SemanticTokenModifier::Declaration],
                });

                // Parameters
                for param in &func.params {
                    tokens.push(SemanticToken {
                        line: func.span.start as u32,
                        character: 0, // TODO: Calculate character offset
                        length: param.name.len() as u32,
                        token_type: SemanticTokenType::Parameter,
                        modifiers: vec![],
                    });
                }
            }
            Item::Struct(struct_def) => {
                // Struct name
                tokens.push(SemanticToken {
                    line: struct_def.span.start as u32,
                    character: 0, // TODO: Calculate character offset
                    length: struct_def.name.len() as u32,
                    token_type: SemanticTokenType::Struct,
                    modifiers: vec![SemanticTokenModifier::Declaration],
                });
            }
            Item::Enum(enum_def) => {
                // Enum name
                tokens.push(SemanticToken {
                    line: enum_def.span.start as u32,
                    character: 0, // TODO: Calculate character offset
                    length: enum_def.name.len() as u32,
                    token_type: SemanticTokenType::Enum,
                    modifiers: vec![SemanticTokenModifier::Declaration],
                });
            }
            _ => {}
        }
    }
}
