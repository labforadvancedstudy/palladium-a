// Parser for Palladium
// "Constructing legends from tokens"

use crate::ast::{AssignTarget, Param, UnaryOp, *};
use crate::errors::{CompileError, Result, Span};
use crate::lexer::Token;

/// The attributes this compiler implements. **EMPTY, and that is the answer.**
///
/// N2-10 makes `#` lex; N2-11 makes an unknown attribute a compile error. Read
/// together with an empty table they say: **every attribute is a compile error
/// today.** That is deliberate and it is why the two rows ship in one commit.
/// The alternative — lex `#[total]`, accept it, generate no totality check — is
/// the defect M1 was spent removing, one construct along: a source that claims
/// a property and a binary that does not have it.
///
/// So the table is empty rather than absent, and empty rather than containing
/// `total`. `#[total]` is N8, owned by M6; until M6 discharges the obligation
/// there is nothing for the compiler to honour, and a compiler that accepts an
/// obligation it cannot discharge is worse than one that says no.
/// `tests/reject/total_attribute.pd` pins that: a reject before this change (at
/// `Unexpected character '#'`, one level below the parser) and a reject after
/// it, for a reason that now names the attribute.
///
/// `the_known_attribute_set_is_empty_on_purpose` fails when a name is added, so
/// M6 cannot add one without reading this paragraph.
pub const KNOWN_ATTRIBUTES: &[&str] = &[];

/// One `#[name]`, `#[name(args)]` or `#![name(args)]` as written.
///
/// Nothing consumes this beyond the refusal, because there is nothing to
/// consume: the known set is empty. It exists as a named value anyway so the
/// refusal can report WHICH attribute in WHAT shape, which is what makes "the
/// three shapes of N2-10 lex" a checkable claim rather than an assertion about
/// code nobody can observe.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    /// The attribute's name — `total` in `#[total(2)]`.
    pub name: String,
    /// The argument tokens as source text, one entry per token; empty when the
    /// attribute has no `( … )` at all. Not parsed into anything structured: an
    /// argument grammar for a set of zero known attributes is a grammar for
    /// nothing.
    pub args: Vec<String>,
    /// `#!` rather than `#` — an inner attribute, applying to the compilation
    /// unit, and legal only at the top of it.
    pub inner: bool,
    pub span: Span,
}

pub struct Parser {
    tokens: Vec<(Token, Span)>,
    current: usize,
    /// Type parameters currently in scope (for parsing generic functions)
    type_params_in_scope: Vec<String>,
    /// Cache for current token to avoid repeated bounds checking
    current_token_cache: Option<(Token, Span)>,
}

/// The value-producing shape of a block's final statement, as the parser saw it.
///
/// WHY THIS LIVES IN THE PARSER AND NOT IN A LATER PASS
/// ----------------------------------------------------
/// The property being lowered is "the block ends in an expression written with
/// **no** `;`". The AST cannot express it: `foo();` and a trailing `foo()` are
/// both `Stmt::Expr(..)`, and a branch of an `if` is a bare `Vec<Stmt>` with no
/// record of which of its statements was in tail position. So the distinction
/// exists only between `parse_block_with_implicit_return` reading the token and
/// the `Vec<Stmt>` it returns. A pass over the AST would have to *guess*, and a
/// guess here turns `if c { print_int(1); } else { print_int(2); }` into
/// `return print_int(1);` — returning void from a non-void function.
///
/// The alternative would be to enrich the AST with a tail marker and lower
/// later. That is a strictly larger change (every `Stmt::Expr` construction and
/// match site) which buys nothing: `parse_function` is the only consumer,
/// because only there is the return type known.
#[derive(Debug, Clone)]
enum BlockTail {
    /// Nothing to lower: the block is empty, or ends in a `;` statement, a
    /// `return`, a loop, a `let`, … Its value is not a tail expression.
    NoValue,
    /// The last statement is a `Stmt::Expr` written without a `;`.
    Expr,
    /// The last statement is an `if`. Each branch carries its own tail;
    /// `else_tail` is `None` when the `if` has no `else` at all.
    If {
        then_tail: Box<BlockTail>,
        else_tail: Option<Box<BlockTail>>,
        span: Span,
    },
    /// The last statement is a `match`, with one tail per arm in arm order.
    Match {
        arm_tails: Vec<BlockTail>,
        span: Span,
    },
}

impl BlockTail {
    /// Did the programmer write a value in tail position anywhere in here?
    ///
    /// This is the trigger for the refusal below, and it is deliberately about
    /// *what was written* rather than about control flow. A tail expression in
    /// a branch is unambiguous evidence that the branch was meant to be the
    /// function's value; if some other path of the same construct has no value,
    /// the program as written cannot be compiled honestly.
    fn writes_a_value(&self) -> bool {
        match self {
            BlockTail::NoValue => false,
            BlockTail::Expr => true,
            BlockTail::If {
                then_tail,
                else_tail,
                ..
            } => {
                then_tail.writes_a_value() || else_tail.as_ref().is_some_and(|e| e.writes_a_value())
            }
            BlockTail::Match { arm_tails, .. } => arm_tails.iter().any(|t| t.writes_a_value()),
        }
    }

    /// The span to blame when this tail cannot be lowered.
    fn span(&self) -> Option<Span> {
        match self {
            BlockTail::NoValue | BlockTail::Expr => None,
            BlockTail::If { span, .. } | BlockTail::Match { span, .. } => Some(*span),
        }
    }

    /// `if` or `match` — the keyword named in the refusal.
    fn keyword(&self) -> &'static str {
        match self {
            BlockTail::Match { .. } => "match",
            _ => "if",
        }
    }
}

/// Can every path out of `stmts` be made to end in a `return`?
///
/// This walks the statements and the tail together, because the two carry
/// different halves of the answer: the tail says which leaves are `;`-less
/// expressions that the lowering below can rewrite, and the statements say
/// which leaves cannot reach their closing brace at all (a `return`, a call
/// that does not come back, a loop with no exit) and so need no rewriting.
///
/// That second half is not a nicety. `if n > 0 { return n * 10; } else { n - 1 }`
/// has a perfectly good value on both paths — one written as a `return`, one in
/// tail position — and a rule that only looked at the tail would refuse it.
///
/// This is the same invariant `scripts/check-c-returns.py` enforces over the
/// emitted C ("returns on every path"), stated over the source instead. It is
/// deliberately shallow: it decides the *tail* construct of a function body,
/// not arbitrary control flow, so it is not a general flow analysis and does
/// not pretend to be one.
fn returns_on_every_path(stmts: &[Stmt], tail: &BlockTail) -> bool {
    match tail {
        // The lowering will turn this leaf into a `return`.
        BlockTail::Expr => true,
        // Nothing to lower here, so this path is only good if it already ends
        // in a terminator.
        BlockTail::NoValue => already_terminates(stmts),
        BlockTail::If {
            then_tail,
            else_tail,
            ..
        } => {
            let Some(Stmt::If {
                then_branch,
                else_branch,
                ..
            }) = stmts.last()
            else {
                return false;
            };
            // No `else` means the false path reaches the closing brace with
            // nothing to return, whatever the `if` branch does.
            match (else_branch, else_tail) {
                (Some(eb), Some(et)) => {
                    returns_on_every_path(then_branch, then_tail) && returns_on_every_path(eb, et)
                }
                _ => false,
            }
        }
        BlockTail::Match { arm_tails, .. } => {
            let Some(Stmt::Match { arms, .. }) = stmts.last() else {
                return false;
            };
            // `all` over an empty iterator is `true`, so emptiness is explicit.
            // THE RESIDUAL THIS NOTE USED TO RECORD IS GONE (N6-11): the C
            // emitted for a `match` ends in an `else` that traps, so a value
            // matching no arm stops the program instead of falling off the end
            // of its function. That is what let `-Werror=return-type` be armed
            // in src/linker.rs, and the generated-C invariant
            // (scripts/check-c-returns.py) now finds nothing here either.
            !arms.is_empty()
                && arms.len() == arm_tails.len()
                && arms
                    .iter()
                    .zip(arm_tails.iter())
                    .all(|(arm, t)| returns_on_every_path(&arm.body, t))
        }
    }
}

/// Does this statement list have a path to its closing brace at all?
///
/// WHERE THE WHOLE TERMINATION ANALYSIS LIVES, for a reader who cannot open a
/// 4,000-line file: it is five functions and one call site, and nothing else in
/// this file participates.
///
///   `src/parser/mod.rs:78-136`    `BlockTail` — the shape of the block's final
///                                statement as the parser SAW it, plus
///                                `writes_a_value()`, which decides whether a
///                                refusal is owed at all
///   `src/parser/mod.rs:155-203`  `returns_on_every_path` — the decision. The
///                                NOTE inside it used to record one declared
///                                residual (a `match` with no final `else`);
///                                N6-11's trap discharged it
///   `src/parser/mod.rs:278-280`  `already_terminates` — `any`, not "the last
///                                statement", because anything after an
///                                unconditional terminator is unreachable
///   `src/parser/mod.rs:283-311`  `stmt_terminates` — the four cases, each
///                                paired with its counterpart in
///                                scripts/check-c-returns.py by the table above
///   `src/parser/mod.rs:339-371`  `contains_escaping_break` +
///                                `stmt_contains_escaping_break` — reachable
///                                breaks only, mirroring `contains_break`
///   `src/parser/mod.rs:1188-1212`  the only caller: the refusal and the lowering
///
/// The agreement between this side and the C-side reader is not asserted by
/// this comment — it is executed by `assert_net_a` in tests/d3b_tail_if.rs,
/// which runs scripts/check-c-returns.py over the C emitted for every program
/// those tests accept.
///
/// WHERE THIS AND `scripts/check-c-returns.py` MUST AGREE
/// -----------------------------------------------------
/// The two analyses answer the same question on either side of code
/// generation — this one over Palladium statements, `terminates()` over the C
/// that those statements become — so a shape one accepts and the other does not
/// is a program that compiles here and is then flagged by the generated-C
/// invariant, or (the silent direction) the reverse. They are kept in step case
/// by case:
///
///   `Stmt::Return`            <-> `RETURN_RE`
///   a trailing `panic("…")`   <-> `NORETURN_RE` (`__pd_panic` calls `abort()`,
///                                 src/codegen/mod.rs `__pd_panic` wrapper)
///   `while true { … }` with   <-> the `while\s*\(\s*1\s*\)` case plus
///   no escaping `break`           `contains_break`. `Expr::Bool(true)` is
///                                 emitted as the literal `1`
///                                 (src/codegen/mod.rs, `Expr::Bool`), which is
///                                 the exact spelling that case recognises.
///                                 ONE KNOWN, MEASURED DIVERGENCE: `while
///                                 1 == 1` is folded to `Expr::Bool(true)` by
///                                 src/optimizer/constant_folding.rs:154
///                                 (`BinOp::Eq`) and so ALSO emits `while (1)`,
///                                 while
///                                 this analysis reads the UNFOLDED ast and
///                                 does not call it infinite. The divergence is
///                                 in the safe direction — this side refuses a
///                                 program the C-side reader would have
///                                 accepted — and it is pinned by
///                                 `codegen_spellings_the_generated_c_invariant_
///                                 depends_on` in tests/d3b_tail_if.rs.
///   `if`/`else`, both arms    <-> the `h.startswith("if")` case
///
/// `any` rather than "the last statement": anything written after a statement
/// that cannot fall through is unreachable, so the list cannot fall through
/// either. `scripts/check-c-returns.py` scans its item list the same way, and
/// it has to — a branch of `if c { return 1; print_int(2); } else { 3 }` is
/// entered by this side and read by that one.
///
/// The residual, stated so it is not mistaken for a general analysis: this only
/// sees statements that terminate UNCONDITIONALLY. Unreachability that depends
/// on evaluating a condition (`if false { … }`, a `while` whose guard is
/// provably always true but not written `true`) is not modelled by either side,
/// and the conservative answer there is "falls through", which refuses rather
/// than miscompiles.
fn already_terminates(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_terminates)
}

/// Does this single statement never fall through to the statement after it?
fn stmt_terminates(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) => true,
        Stmt::Expr(Expr::Call { func, .. }) => {
            matches!(func.as_ref(), Expr::Ident(name) if name == "panic")
        }
        // An infinite loop has no exit edge — unless a `break` binds to it.
        Stmt::While {
            condition: Expr::Bool(true),
            body,
            ..
        } => !contains_escaping_break(body),
        // `loop` is the same statement with the condition removed, so it is the
        // same rule. Written as its own arm rather than folded into the one
        // above because the `while` arm is keyed on the literal `true`, and a
        // `loop` has no condition to be literal.
        Stmt::Loop { body, .. } => !contains_escaping_break(body),
        // Needs BOTH arms. An `if` with no `else` does not match this pattern
        // and so falls through to `false` below, which is the right answer: its
        // false path reaches the next statement.
        Stmt::If {
            then_branch,
            else_branch: Some(eb),
            ..
        } => already_terminates(then_branch) && already_terminates(eb),
        Stmt::Unsafe { body, .. } => already_terminates(body),
        _ => false,
    }
}

/// Is there a REACHABLE `break` that escapes the loop whose body is `stmts`?
///
/// Mirrors `contains_break` in `scripts/check-c-returns.py`: a `break` written
/// inside a nested loop binds to that loop, so it does not let control out of
/// ours. Palladium has no labelled break (`Stmt::Break` carries only a span),
/// so nesting is the whole rule.
///
/// REACHABILITY, AND WHY IT HAD TO COME DOWN HERE TOO
/// The scan stops at the first statement that cannot fall through, because a
/// `break` written after one is dead text:
///
/// ```text
/// if c { 1 } else { while true { return 2; break; } }
/// ```
///
/// The `return` leaves the function, so the `break` never runs, so the loop has
/// no exit edge and the `else` branch cannot fall through — the program is
/// correct. Counting every SYNTACTICALLY PRESENT break called that loop
/// escapable and refused it (measured before this fix). `already_terminates`
/// above was already reachability-aware after the first round; this is the same
/// rule one level down, and the two have to move together or a program is
/// accepted by one and refused by the other.
///
/// The mutual recursion (`stmt_terminates` asks about breaks in a loop body,
/// this asks whether a statement terminates) descends into strictly smaller
/// statement trees on every step, so it is well founded.
fn contains_escaping_break(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        if stmt_contains_escaping_break(stmt) {
            return true;
        }
        if stmt_terminates(stmt) {
            // Everything after this point is unreachable, breaks included.
            return false;
        }
    }
    false
}

fn stmt_contains_escaping_break(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Break { .. } => true,
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            contains_escaping_break(then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(|eb| contains_escaping_break(eb))
        }
        Stmt::Match { arms, .. } => arms.iter().any(|a| contains_escaping_break(&a.body)),
        Stmt::Unsafe { body, .. } => contains_escaping_break(body),
        // A `break` in here belongs to THAT loop, not to ours.
        Stmt::While { .. } | Stmt::For { .. } => false,
        _ => false,
    }
}

/// Does this declared return type oblige the body to produce a value?
///
/// BOTH SPELLINGS OF UNIT ARE ONE TYPE, and this is the third place on this
/// branch that has had to say so: `None` and `Some(Type::Unit)` mean the same
/// thing, and a rule that tests `return_type.is_some()` treats `fn f() -> ()`
/// as value-returning. For the refusal below that would mean rejecting
/// `fn f() -> () { print_int(1); }`, which is a correct program.
fn returns_a_value(return_type: Option<&Type>) -> bool {
    !matches!(return_type, None | Some(Type::Unit))
}

/// The same phrase as `missing_path`, but for a whole function body rather than
/// a branch of one.
///
/// `missing_path` says "this branch" for a `NoValue` tail, which is the right
/// words one level in and the wrong words at the top: a body that simply ends
/// has no branch to point at.
fn missing_path_from_body(stmts: &[Stmt], tail: &BlockTail) -> String {
    match tail {
        BlockTail::NoValue => {
            if stmts.is_empty() {
                "the body is empty, so it returns whatever is in the return register".to_string()
            } else {
                "the body can reach its closing brace without executing a `return`".to_string()
            }
        }
        _ => missing_path(stmts, tail),
    }
}

/// Which path has no value, phrased for the diagnostic. Names the *first*
/// offending path so the message points at one concrete thing.
fn missing_path(stmts: &[Stmt], tail: &BlockTail) -> String {
    match tail {
        BlockTail::Expr => "this branch".to_string(),
        BlockTail::NoValue => "this branch".to_string(),
        BlockTail::If {
            then_tail,
            else_tail,
            ..
        } => {
            let Some(Stmt::If {
                then_branch,
                else_branch,
                ..
            }) = stmts.last()
            else {
                return "some path".to_string();
            };
            match (else_branch, else_tail) {
                (None, _) | (_, None) => "there is no `else` branch, so the false path".to_string(),
                (Some(eb), Some(et)) => {
                    if !returns_on_every_path(then_branch, then_tail) {
                        "the `if` branch".to_string()
                    } else if !returns_on_every_path(eb, et) {
                        "the `else` branch".to_string()
                    } else {
                        "some path".to_string()
                    }
                }
            }
        }
        BlockTail::Match { arm_tails, .. } => {
            let Some(Stmt::Match { arms, .. }) = stmts.last() else {
                return "some path".to_string();
            };
            match arms
                .iter()
                .zip(arm_tails.iter())
                .position(|(arm, t)| !returns_on_every_path(&arm.body, t))
            {
                Some(i) => format!("match arm {}", i + 1),
                None => "an unmatched value".to_string(),
            }
        }
    }
}

/// Rewrite every tail expression described by `tail` into a `return`.
///
/// Only ever called after `returns_on_every_path`, so every leaf it walks to is
/// either a `Stmt::Expr` in tail position (rewritten) or a branch that already
/// terminates (left alone).
fn lower_tail_to_return(stmts: &mut [Stmt], tail: &BlockTail) {
    match tail {
        BlockTail::NoValue => {}
        BlockTail::Expr => {
            if let Some(last @ Stmt::Expr(_)) = stmts.last_mut() {
                // Move the expression out through a placeholder rather than
                // cloning it: an arm's tail can be an arbitrarily large call
                // tree and this runs once per function.
                let taken = std::mem::replace(last, Stmt::Return(None));
                if let Stmt::Expr(expr) = taken {
                    *last = Stmt::Return(Some(expr));
                }
            }
        }
        BlockTail::If {
            then_tail,
            else_tail,
            ..
        } => {
            if let Some(Stmt::If {
                then_branch,
                else_branch,
                ..
            }) = stmts.last_mut()
            {
                lower_tail_to_return(then_branch, then_tail);
                if let (Some(eb), Some(et)) = (else_branch.as_mut(), else_tail.as_ref()) {
                    lower_tail_to_return(eb, et);
                }
            }
        }
        BlockTail::Match { arm_tails, .. } => {
            if let Some(Stmt::Match { arms, .. }) = stmts.last_mut() {
                for (arm, arm_tail) in arms.iter_mut().zip(arm_tails.iter()) {
                    lower_tail_to_return(&mut arm.body, arm_tail);
                }
            }
        }
    }
}

impl Parser {
    pub fn new(tokens: Vec<(Token, Span)>) -> Self {
        let current_token_cache = if !tokens.is_empty() {
            Some(tokens[0].clone())
        } else {
            None
        };

        Self {
            tokens,
            current: 0,
            type_params_in_scope: Vec::new(),
            current_token_cache,
        }
    }

    /// Parse generic parameters (<'a, T, const N: usize>)
    #[allow(clippy::type_complexity)]
    fn parse_generic_params(&mut self) -> Result<(Vec<String>, Vec<String>, Vec<(String, Type)>)> {
        let mut lifetime_params = Vec::new();
        let mut type_params = Vec::new();
        let mut const_params = Vec::new();

        if self.check(&Token::Lt) {
            self.advance()?; // consume '<'

            loop {
                // Check if it's a lifetime parameter
                if self.check(&Token::SingleQuote) {
                    self.advance()?; // consume single quote
                    let lifetime_name = match self.advance()? {
                        (Token::Identifier(name), _) => format!("'{}", name),
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "lifetime name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    lifetime_params.push(lifetime_name);
                } else if self.check(&Token::Const) {
                    // It's a const parameter
                    self.advance()?; // consume 'const'
                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "const parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    self.consume(Token::Colon, "Expected ':' after const parameter name")?;
                    let param_type = self.parse_type()?;
                    const_params.push((param_name, param_type));
                } else {
                    // It's a type parameter
                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "type parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    type_params.push(param_name.clone());
                }

                if !self.check(&Token::Comma) {
                    break;
                }
                self.advance()?; // consume ','
            }

            self.consume(Token::Gt, "Expected '>' after generic parameters")?;
        }

        Ok((lifetime_params, type_params, const_params))
    }

    /// Get current token
    pub fn current_token(&self) -> &Token {
        if let Some((ref token, _)) = self.current_token_cache {
            token
        } else {
            &Token::Eof
        }
    }

    /// Update cache when current position changes
    fn update_cache(&mut self) {
        self.current_token_cache = if self.current < self.tokens.len() {
            Some(self.tokens[self.current].clone())
        } else {
            None
        };
    }

    /// Get the current span for error reporting
    fn current_span(&self) -> Option<crate::errors::Span> {
        if self.current < self.tokens.len() {
            let span = &self.tokens[self.current].1;
            Some(crate::errors::Span::new(
                span.start,
                span.end,
                span.line,
                span.column,
            ))
        } else if self.current > 0 && self.current - 1 < self.tokens.len() {
            // Use previous token's span if at end
            let span = &self.tokens[self.current - 1].1;
            Some(crate::errors::Span::new(
                span.start,
                span.end,
                span.line,
                span.column,
            ))
        } else {
            None
        }
    }

    /// Get span from an expression
    fn expr_span(expr: &Expr) -> Span {
        match expr {
            // Expressions without span field return dummy span for now
            Expr::Integer(_) => Span::dummy(),
            Expr::Float(_) => Span::dummy(),
            Expr::Char(_) => Span::dummy(),
            Expr::String(_) => Span::dummy(),
            Expr::Bool(_) => Span::dummy(),
            Expr::Ident(_) => Span::dummy(),
            // Expressions with span field
            Expr::Tuple { span, .. } => *span,
            Expr::TupleIndex { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
            Expr::Unary { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::Index { span, .. } => *span,
            Expr::ArrayLiteral { span, .. } => *span,
            Expr::ArrayRepeat { span, .. } => *span,
            Expr::StructLiteral { span, .. } => *span,
            Expr::FieldAccess { span, .. } => *span,
            Expr::EnumConstructor { span, .. } => *span,
            Expr::Range { span, .. } => *span,
            Expr::Reference { span, .. } => *span,
            Expr::Deref { span, .. } => *span,
            Expr::Question { span, .. } => *span,
            Expr::MacroInvocation { span, .. } => *span,
            Expr::If { span, .. } => *span,
            Expr::Block { span, .. } => *span,
            Expr::Cast { span, .. } => *span,
            Expr::Loop { span, .. } => *span,
            Expr::Match { span, .. } => *span,
            Expr::Await { span, .. } => *span,
        }
    }

    /// Parse a complete program
    pub fn parse(&mut self) -> Result<Program> {
        let mut imports = Vec::new();
        let mut items = Vec::new();

        // Inner attributes (`#![name(args)]`) apply to the compilation unit and
        // N2 puts them "at the top of" it, so they are read before anything
        // else. Each is refused as it is read (N2-11), so the loop cannot
        // actually run twice today; it is a loop because the shape is a list
        // and writing it as one `if` would make the second `#!` fail with a
        // different, wrong message once an attribute is ever implemented.
        while self.check(&Token::HashBang) {
            self.parse_attribute()?;
        }

        // Parse imports first
        while self.check(&Token::Import) {
            imports.push(self.parse_import()?);
        }

        // Then parse items
        while !self.is_at_end() {
            items.push(self.parse_item()?);
        }

        Ok(Program { imports, items })
    }

    /// Parse one attribute and refuse it (N2-10 lexes it, N2-11 refuses it).
    ///
    /// Returns the attribute for the benefit of a future caller that has a
    /// non-empty `KNOWN_ATTRIBUTES` to check it against; today the `Ok` path is
    /// unreachable from source, which is exactly what an empty known set means
    /// and is asserted by `every_attribute_shape_is_refused_by_name`.
    ///
    /// The refusal happens HERE, after the whole attribute has been read,
    /// rather than at the `#`. Reading it first is what lets the diagnostic
    /// name the attribute — `unknown attribute \`frobnicate\`` — and naming it
    /// is the difference between a message a reader can act on and one that
    /// says the syntax exists.
    fn parse_attribute(&mut self) -> Result<Attribute> {
        let (open, open_span) = self.advance()?; // `#` or `#!`
        let inner = matches!(open, Token::HashBang);

        self.consume(
            Token::LeftBracket,
            if inner {
                "Expected '[' after '#!'"
            } else {
                "Expected '[' after '#'"
            },
        )?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "attribute name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // `( … )`, kept as raw token spellings. Balanced on parentheses rather
        // than scanned to the first `)`, so `#[a(b(c))]` reads as one attribute
        // and not as one that ends early with a stray `)` after it.
        let mut args = Vec::new();
        if self.check(&Token::LeftParen) {
            self.advance()?;
            let mut depth = 1usize;
            loop {
                if self.is_at_end() {
                    return Err(CompileError::UnexpectedToken {
                        expected: "')' to close the attribute's arguments".to_string(),
                        found: "end of file".to_string(),
                        span: self.current_span(),
                    });
                }
                let (tok, _) = self.advance()?;
                match tok {
                    Token::LeftParen => {
                        depth += 1;
                        args.push("(".to_string());
                    }
                    Token::RightParen => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                        args.push(")".to_string());
                    }
                    other => args.push(other.to_string()),
                }
            }
        }

        let close_span =
            self.consume(Token::RightBracket, "Expected ']' to close the attribute")?;
        let span = Span::new(
            open_span.start,
            close_span.end,
            open_span.line,
            open_span.column,
        );

        if !KNOWN_ATTRIBUTES.contains(&name.as_str()) {
            return Err(CompileError::unknown_attribute(
                &name,
                KNOWN_ATTRIBUTES,
                span,
            ));
        }

        Ok(Attribute {
            name,
            args,
            inner,
            span,
        })
    }

    /// Parse an import statement
    fn parse_import(&mut self) -> Result<crate::ast::Import> {
        let start_span = self.consume(Token::Import, "Expected 'import'")?;

        let mut path = Vec::new();
        let mut items = None;

        // Parse first part of path
        let first = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "module name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };
        path.push(first);

        // Parse remaining path segments
        while self.check(&Token::DoubleColon) {
            self.advance()?; // consume '::'

            // Check if this might be a specific item import
            if matches!(self.peek(), Ok(Token::Identifier(_))) {
                // Look ahead to see if this is the last segment (followed by ; or ,)
                let next_is_terminator = self
                    .tokens
                    .get(self.current + 1)
                    .map(|(t, _)| matches!(t, Token::Semicolon | Token::Comma | Token::LeftBrace))
                    .unwrap_or(false);

                if next_is_terminator {
                    // This is a specific item import
                    items = Some(self.parse_import_items()?);
                    break;
                } else {
                    // This is another module in the path
                    let segment = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "module name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    path.push(segment);
                }
            } else if self.check(&Token::Star) {
                // Wildcard import
                self.advance()?; // consume '*'
                items = None; // Explicitly None for wildcard
                break;
            } else if self.check(&Token::LeftBrace) {
                // Multiple item import: import std::math::{pd_abs, pd_sin}
                items = Some(self.parse_import_items()?);
                break;
            } else {
                return Err(CompileError::UnexpectedToken {
                    expected: "module name, item name, or '*'".to_string(),
                    found: self.peek()?.to_string(),
                    span: self.current_span(),
                });
            }
        }

        // Parse optional alias
        let mut alias = None;
        if self.check(&Token::As) {
            self.advance()?; // consume 'as'
            match self.advance()? {
                (Token::Identifier(name), _) => {
                    alias = Some(name);
                }
                (token, _) => {
                    return Err(CompileError::UnexpectedToken {
                        expected: "alias name".to_string(),
                        found: token.to_string(),
                        span: self.current_span(),
                    });
                }
            }
        }

        let end_span = self.consume(Token::Semicolon, "Expected ';' after import")?;

        Ok(crate::ast::Import {
            path,
            items,
            alias,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse import items - either single item or multiple items in braces
    fn parse_import_items(&mut self) -> Result<Vec<String>> {
        let mut items = Vec::new();

        if self.check(&Token::LeftBrace) {
            // Multiple items: {pd_abs, pd_sin, pd_cos}
            self.advance()?; // consume '{'

            loop {
                match self.advance()? {
                    (Token::Identifier(name), _) => {
                        items.push(name);

                        if self.check(&Token::RightBrace) {
                            self.advance()?; // consume '}'
                            break;
                        } else {
                            self.consume(Token::Comma, "Expected ',' or '}' after import item")?;
                            // Allow trailing comma
                            if self.check(&Token::RightBrace) {
                                self.advance()?; // consume '}'
                                break;
                            }
                        }
                    }
                    (token, _) => {
                        return Err(CompileError::UnexpectedToken {
                            expected: "item name".to_string(),
                            found: token.to_string(),
                            span: self.current_span(),
                        });
                    }
                }
            }
        } else {
            // Single item
            match self.advance()? {
                (Token::Identifier(name), _) => {
                    items.push(name);
                }
                (token, _) => {
                    return Err(CompileError::UnexpectedToken {
                        expected: "item name".to_string(),
                        found: token.to_string(),
                        span: self.current_span(),
                    });
                }
            }
        }

        Ok(items)
    }

    /// Parse a top-level item
    fn parse_item(&mut self) -> Result<Item> {
        // Outer attributes precede everything else on an item, including `pub`.
        // Each is refused as it is read (N2-11).
        while self.check(&Token::Hash) {
            self.parse_attribute()?;
        }

        // `#!` here rather than at the top of the file: say so, instead of
        // letting it fall through to "Expected function, struct, …", which
        // names the wrong problem.
        if self.check(&Token::HashBang) {
            return Err(CompileError::SyntaxError {
                message: "an inner attribute `#![…]` may only appear at the top of the file, \
                          before any import or item; write `#[…]` to annotate this item"
                    .to_string(),
                span: self.current_span(),
            });
        }

        // Check for visibility modifier
        let visibility = if self.check(&Token::Pub) {
            self.advance()?; // consume 'pub'
            crate::ast::Visibility::Public
        } else {
            crate::ast::Visibility::Private
        };

        // Check for async modifier
        let is_async = if self.check(&Token::Async) {
            self.advance()?; // consume 'async'
            true
        } else {
            false
        };

        match self.peek()? {
            Token::Fn => {
                let mut func = self.parse_function()?;
                func.visibility = visibility;
                func.is_async = is_async;
                Ok(Item::Function(func))
            }
            Token::Struct => {
                if is_async {
                    return Err(CompileError::SyntaxError {
                        message: "async can only be used with functions".to_string(),
                        span: self.current_span(),
                    });
                }
                let mut struct_def = self.parse_struct()?;
                struct_def.visibility = visibility;
                Ok(Item::Struct(struct_def))
            }
            Token::Enum => {
                let mut enum_def = self.parse_enum()?;
                enum_def.visibility = visibility;
                Ok(Item::Enum(enum_def))
            }
            Token::Trait => {
                let mut trait_def = self.parse_trait()?;
                trait_def.visibility = visibility;
                Ok(Item::Trait(trait_def))
            }
            Token::Impl => Ok(Item::Impl(self.parse_impl()?)),
            Token::Type => {
                let mut type_alias = self.parse_type_alias()?;
                type_alias.visibility = visibility;
                Ok(Item::TypeAlias(type_alias))
            }
            Token::Macro => Ok(Item::Macro(self.parse_macro()?)),
            Token::Const | Token::Static => {
                if is_async {
                    return Err(CompileError::SyntaxError {
                        message: "async can only be used with functions".to_string(),
                        span: self.current_span(),
                    });
                }
                let mut global = self.parse_global()?;
                global.visibility = visibility;
                Ok(Item::Global(global))
            }
            // N3-14. `macro_rules! name { … }` is not this language's macro
            // syntax, and it used to fall through to "Expected function,
            // struct, enum, trait, type, impl, or macro declaration" — which
            // names seven things a reader coming from Rust would read as "no
            // macros here", when the truth is the opposite: there IS a macro
            // system and there is exactly ONE, so `macro_rules!` is refused
            // because the language HAS a macro form rather than because it
            // lacks one.
            Token::Identifier(name)
                if name == "macro_rules" && self.check_at(1, &Token::Not) =>
            {
                Err(CompileError::SyntaxError {
                    message: "`macro_rules!` is not a declaration in this language: there is ONE \
                              macro system and no procedural/declarative split. Write \
                              `macro name!(params) { body }`"
                        .to_string(),
                    span: self.current_span(),
                })
            }
            _ => {
                if is_async {
                    Err(CompileError::SyntaxError {
                        message: "async can only be used with function declarations".to_string(),
                        span: self.current_span(),
                    })
                } else {
                    Err(CompileError::SyntaxError {
                        message: "Expected function, struct, enum, trait, type, impl, or macro declaration".to_string(),
                        span: self.current_span(),
                    })
                }
            }
        }
    }

    /// Parse a function declaration
    fn parse_function(&mut self) -> Result<Function> {
        let start_span = self.consume(Token::Fn, "Expected 'fn'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "function name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Parse generic parameters (lifetimes, types, and consts) if present
        let (lifetime_params, type_params, const_params) = self.parse_generic_params()?;

        // Set type parameters in scope for parsing function signature and body
        self.type_params_in_scope = type_params.clone();

        self.consume(Token::LeftParen, "Expected '('")?;

        // Parse function parameters
        let mut params = Vec::new();

        if !self.check(&Token::RightParen) {
            loop {
                // Check for self parameter variants
                if self.check(&Token::SelfParam) {
                    self.advance()?; // consume 'self'
                    params.push(Param {
                        name: "self".to_string(),
                        ty: Type::Custom("Self".to_string()),
                        mutable: false,
                    });
                } else if self.check(&Token::Ampersand) {
                    self.advance()?; // consume '&'

                    // Check for optional 'mut' after '&'
                    let mutable = if self.check(&Token::Mut) {
                        self.advance()?; // consume 'mut'
                        true
                    } else {
                        false
                    };

                    // Now must be 'self'
                    if self.check(&Token::SelfParam) {
                        self.advance()?; // consume 'self'
                        params.push(Param {
                            name: "self".to_string(),
                            ty: Type::Reference {
                                lifetime: None,
                                mutable,
                                inner: Box::new(Type::Custom("Self".to_string())),
                            },
                            mutable: false,
                        });
                    } else {
                        return Err(CompileError::UnexpectedToken {
                            expected: "self".to_string(),
                            found: self.peek()?.to_string(),
                            span: self.current_span(),
                        });
                    }
                } else {
                    // Regular parameter parsing
                    // Check for optional 'mut' keyword
                    let mutable = if self.check(&Token::Mut) {
                        self.advance()?; // consume 'mut'
                        true
                    } else {
                        false
                    };

                    // Parse parameter name
                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };

                    // Parse parameter type
                    self.consume(Token::Colon, "Expected ':' after parameter name")?;
                    let param_type = self.parse_type()?;

                    params.push(Param {
                        name: param_name,
                        ty: param_type,
                        mutable,
                    });
                }

                if !self.check(&Token::Comma) {
                    break;
                }
                self.advance()?; // consume ','
            }
        }

        self.consume(Token::RightParen, "Expected ')'")?;

        // Parse return type if present
        let return_type = if self.check(&Token::Arrow) {
            self.advance()?; // consume '->'
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(Token::LeftBrace, "Expected '{'")?;

        let (mut body, body_tail) = self.parse_block_with_implicit_return()?;

        // Grammar: `block = '{' { statement } [ expression ] '}'` — a trailing
        // expression in a function body is the function's value. Lower it to an
        // explicit return so codegen emits `return <expr>;` instead of dropping
        // it as a bare expression statement (which returned garbage).
        //
        // D3 lowered only `BlockTail::Expr`. D3b is the same defect one level
        // in: `fn fib(n: i64) -> i64 { if n <= 1 { n } else { … } }` ends in a
        // `Stmt::If`, never entered that path, and `fib(10)` returned
        // 8261746944 instead of 55. So the lowering now recurses through the
        // branches of a tail `if` and the arms of a tail `match`.
        //
        // Only the *function body* tail is lowered, and only the tails at the
        // end of it: an `if` in the middle of a body still has ordinary
        // expression statements in its branches.
        //
        // A UNIT FUNCTION KEEPS THE PLAIN EXPRESSION STATEMENT, and "unit" means
        // BOTH SPELLINGS OF IT. This comment used to say `return_type == None`
        // while the condition below tested `is_some()`, and the two spellings
        // therefore emitted different C:
        //
        //     fn f() { print_int(7) }        ->  void f() { __pd_print_int(7); }
        //     fn f() -> () { print_int(7) }  ->  void f() { return __pd_print_int(7); }
        //
        // `parse_type` returns `Type::Unit` for `()`, `is_some()` is true for
        // it, and the lowering ran — producing `return <void expression>;` from
        // a `void` function, which is a C constraint violation that gcc and
        // clang happen to accept as an extension. Two spellings of one type
        // must not produce different output, and neither of them should produce
        // that.
        //
        // WHERE THE FIX IS, AND WHY IT IS NOT HERE.
        // The obvious repair is to exclude `Some(Type::Unit)` from this
        // condition. MEASURED, that silently deletes a diagnostic:
        //
        //     fn f() -> () { 5 }
        //       lowered    -> `return 5;` -> typeck: "expected (), found Int"
        //       not lowered-> `5;`        -> compiles clean, value discarded
        //
        // The type checker only sees the mismatch through the `Stmt::Return`
        // this lowering creates; a bare expression statement of the wrong type
        // in a unit function is not a rule it has. Trading a real diagnostic
        // for identical output would be the wrong way round for a branch about
        // a compiler that must not accept what it cannot honour.
        //
        // So the lowering still runs for `-> ()`, and CODE GENERATION handles
        // the void case: `src/codegen/mod.rs`, `Stmt::Return(Some(expr))` under
        // `current_fn_unit_return`, emits the expression as a statement followed
        // by that function's unit return (`return;`, or `return 0;` for `main`,
        // whose C type is `int`) rather than `return <void expression>;`. The
        // constraint violation is gone, the diagnostic is kept, and the two
        // spellings differ only by that inert return.
        //
        // (This comment named `current_fn_is_void` for two rounds after that
        // field was replaced — the second stale mechanism reference on this
        // branch. Both were found by review, not by a check; a name that no
        // longer exists is greppable, and if a third appears it is worth
        // mechanising rather than fixing by hand again.)
        if return_type.is_some() {
            if returns_on_every_path(&body, &body_tail) {
                lower_tail_to_return(&mut body, &body_tail);
            } else if body_tail.writes_a_value() {
                // A value was written in tail position but some path of the same
                // construct has none. Emitting C for this is how the compiler
                // starts lying — the function falls off its end and returns
                // whatever is in the return register. Refuse instead.
                //
                // Nothing correct is lost by refusing: to reach here a program
                // must already contain a tail expression inside a branch, and
                // every such program miscompiles today.
                return Err(CompileError::tail_value_not_on_every_path(
                    body_tail.keyword(),
                    &missing_path(&body, &body_tail),
                    body_tail.span().unwrap_or(start_span),
                ));
            } else if returns_a_value(return_type.as_ref()) {
                // NOTHING was written in tail position anywhere, and the body
                // still has a path to its closing brace: `fn f() -> i64 { }`,
                // `if c { return 1; }` as the last statement, a `while` that
                // may not be entered. This used to be left alone, on the
                // reasoning that the parser had no evidence of what the author
                // meant. The declared return type IS that evidence, and it is
                // the stronger kind: whatever was meant, a non-void C function
                // that falls off its end returns the register's contents.
                //
                // WHY THE PREDICATE IS THE SAME ONE, AND NOT A SECOND ANALYSIS.
                // `returns_on_every_path` has already answered this question in
                // the line above — the two arms differ only in which refusal
                // they raise. Asking again elsewhere (a pass over the AST in
                // typeck, say) would be a THIRD copy of the terminator rules
                // after this one and `scripts/check-c-returns.py`, and the
                // hazard those two already carry is drifting apart; the C-side
                // reader is kept in step case-by-case by the table on
                // `already_terminates` and executed by `assert_net_a`. A third
                // copy doubles that surface to buy nothing. It also could not
                // be as precise: tail position is not in the AST at all (see
                // `BlockTail`'s own comment), so a later pass would be reading
                // whatever this lowering happened to leave behind.
                //
                // POLARITY. This refuses, so its errors fall on VALID programs,
                // and `returns_on_every_path` is deliberately conservative —
                // unreachability that depends on evaluating a condition is not
                // modelled. Everything it does model is receipted from the
                // accept side in `tests/m1_missing_return.rs`.
                return Err(CompileError::missing_return(
                    &name,
                    &missing_path_from_body(&body, &body_tail),
                    body_tail.span().unwrap_or(start_span),
                ));
            }
            // Otherwise the function returns `()` — under either spelling — and
            // reaching the closing brace with no value is what it is for.
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}'")?;

        // Clear type parameters from scope
        self.type_params_in_scope.clear();

        Ok(Function {
            visibility: crate::ast::Visibility::Private, // TODO: parse pub keyword
            is_async: false,                             // Will be set by parse_item
            name,
            lifetime_params,
            type_params,
            const_params,
            params,
            return_type,
            body,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
            effects: None, // Effects will be inferred during analysis
        })
    }

    /// Parse a struct definition
    fn parse_struct(&mut self) -> Result<StructDef> {
        let start_span = self.consume(Token::Struct, "Expected 'struct'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "struct name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Parse generic parameters (lifetimes, types, and consts) if present
        let (lifetime_params, type_params, const_params) = self.parse_generic_params()?;

        self.consume(Token::LeftBrace, "Expected '{' after struct name")?;

        let mut fields = Vec::new();

        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            // Parse field name
            let field_name = match self.advance()? {
                (Token::Identifier(name), _) => name,
                (token, _) => {
                    return Err(CompileError::UnexpectedToken {
                        expected: "field name".to_string(),
                        found: token.to_string(),
                        span: self.current_span(),
                    });
                }
            };

            self.consume(Token::Colon, "Expected ':' after field name")?;
            let field_type = self.parse_type()?;

            fields.push((field_name, field_type));

            // Fields are separated by commas
            if !self.check(&Token::RightBrace) {
                self.consume(Token::Comma, "Expected ',' after field")?;
            }
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after struct fields")?;

        Ok(StructDef {
            visibility: crate::ast::Visibility::Private, // TODO: parse pub keyword
            name,
            lifetime_params,
            type_params,
            const_params,
            fields,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse an enum definition
    fn parse_enum(&mut self) -> Result<EnumDef> {
        let start_span = self.consume(Token::Enum, "Expected 'enum'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "enum name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Parse generic parameters (lifetimes, types, and consts) if present
        let (lifetime_params, type_params, const_params) = self.parse_generic_params()?;

        self.consume(Token::LeftBrace, "Expected '{' after enum name")?;

        let mut variants = Vec::new();

        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            // Parse variant name
            let variant_name = match self.advance()? {
                (Token::Identifier(name), _) => name,
                (token, _) => {
                    return Err(CompileError::UnexpectedToken {
                        expected: "variant name".to_string(),
                        found: token.to_string(),
                        span: self.current_span(),
                    });
                }
            };

            // Parse variant data
            let data = if self.check(&Token::LeftParen) {
                // Tuple variant
                self.advance()?; // consume '('
                let mut types = Vec::new();

                if !self.check(&Token::RightParen) {
                    loop {
                        types.push(self.parse_type()?);
                        if !self.check(&Token::Comma) {
                            break;
                        }
                        self.advance()?; // consume ','
                    }
                }

                self.consume(Token::RightParen, "Expected ')' after tuple variant types")?;
                EnumVariantData::Tuple(types)
            } else if self.check(&Token::LeftBrace) {
                // Struct variant
                self.advance()?; // consume '{'
                let mut fields = Vec::new();

                while !self.check(&Token::RightBrace) && !self.is_at_end() {
                    let field_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "field name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };

                    self.consume(Token::Colon, "Expected ':' after field name")?;
                    let field_type = self.parse_type()?;

                    fields.push((field_name, field_type));

                    if !self.check(&Token::RightBrace) {
                        self.consume(Token::Comma, "Expected ',' after field")?;
                    }
                }

                self.consume(
                    Token::RightBrace,
                    "Expected '}' after struct variant fields",
                )?;
                EnumVariantData::Struct(fields)
            } else {
                // Unit variant
                EnumVariantData::Unit
            };

            variants.push(EnumVariant {
                name: variant_name,
                data,
            });

            // Variants are separated by commas
            if !self.check(&Token::RightBrace) {
                self.consume(Token::Comma, "Expected ',' after variant")?;
            }
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after enum variants")?;

        Ok(EnumDef {
            visibility: crate::ast::Visibility::Private,
            name,
            lifetime_params,
            type_params,
            const_params,
            variants,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a trait definition
    fn parse_trait(&mut self) -> Result<TraitDef> {
        let start_span = self.consume(Token::Trait, "Expected 'trait'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "trait name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Parse generic parameters (lifetimes and types) if present
        let mut lifetime_params = Vec::new();
        let mut type_params = Vec::new();

        if self.check(&Token::Lt) {
            self.advance()?; // consume '<'

            loop {
                // Check if it's a lifetime parameter
                if self.check(&Token::SingleQuote) {
                    self.advance()?; // consume single quote
                    let lifetime_name = match self.advance()? {
                        (Token::Identifier(name), _) => format!("'{}", name),
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "lifetime name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    lifetime_params.push(lifetime_name);
                } else {
                    // It's a type parameter
                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "type parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    type_params.push(param_name);
                }

                if !self.check(&Token::Comma) {
                    break;
                }
                self.advance()?; // consume ','
            }

            self.consume(Token::Gt, "Expected '>' after generic parameters")?;
        }

        self.consume(Token::LeftBrace, "Expected '{' after trait name")?;

        let mut methods = Vec::new();

        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            // Parse method
            let method_start = self.consume(Token::Fn, "Expected 'fn' for trait method")?;

            let method_name = match self.advance()? {
                (Token::Identifier(name), _) => name,
                (token, _) => {
                    return Err(CompileError::UnexpectedToken {
                        expected: "method name".to_string(),
                        found: token.to_string(),
                        span: self.current_span(),
                    });
                }
            };

            // Parse method generic parameters
            let mut method_lifetime_params = Vec::new();
            let mut method_type_params = Vec::new();

            if self.check(&Token::Lt) {
                self.advance()?; // consume '<'

                loop {
                    if self.check(&Token::SingleQuote) {
                        self.advance()?;
                        let lifetime_name = match self.advance()? {
                            (Token::Identifier(name), _) => format!("'{}", name),
                            (token, _) => {
                                return Err(CompileError::UnexpectedToken {
                                    expected: "lifetime name".to_string(),
                                    found: token.to_string(),
                                    span: self.current_span(),
                                });
                            }
                        };
                        method_lifetime_params.push(lifetime_name);
                    } else {
                        let param_name = match self.advance()? {
                            (Token::Identifier(name), _) => name,
                            (token, _) => {
                                return Err(CompileError::UnexpectedToken {
                                    expected: "type parameter name".to_string(),
                                    found: token.to_string(),
                                    span: self.current_span(),
                                });
                            }
                        };
                        method_type_params.push(param_name);
                    }

                    if !self.check(&Token::Comma) {
                        break;
                    }
                    self.advance()?;
                }

                self.consume(Token::Gt, "Expected '>' after generic parameters")?;
            }

            // Parse parameters
            self.consume(Token::LeftParen, "Expected '(' after method name")?;
            let mut params = Vec::new();

            if !self.check(&Token::RightParen) {
                loop {
                    let mutable = if self.check(&Token::Mut) {
                        self.advance()?;
                        true
                    } else {
                        false
                    };

                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };

                    self.consume(Token::Colon, "Expected ':' after parameter name")?;
                    let param_type = self.parse_type()?;

                    params.push(Param {
                        name: param_name,
                        ty: param_type,
                        mutable,
                    });

                    if !self.check(&Token::Comma) {
                        break;
                    }
                    self.advance()?;
                }
            }

            self.consume(Token::RightParen, "Expected ')'")?;

            // Parse return type
            let return_type = if self.check(&Token::Arrow) {
                self.advance()?;
                Some(self.parse_type()?)
            } else {
                None
            };

            // Check if method has body
            let (has_body, body) = if self.check(&Token::LeftBrace) {
                self.advance()?; // consume '{'
                let mut stmts = Vec::new();
                while !self.check(&Token::RightBrace) && !self.is_at_end() {
                    stmts.push(self.parse_statement()?);
                }
                let _method_end = self.consume(Token::RightBrace, "Expected '}'")?;
                (true, Some(stmts))
            } else {
                self.consume(
                    Token::Semicolon,
                    "Expected ';' after trait method signature",
                )?;
                (false, None)
            };

            methods.push(TraitMethod {
                name: method_name,
                lifetime_params: method_lifetime_params,
                type_params: method_type_params,
                params,
                return_type,
                has_body,
                body,
                span: Span::new(
                    method_start.start,
                    self.current_span()
                        .map(|s| s.end)
                        .unwrap_or(method_start.end),
                    method_start.line,
                    method_start.column,
                ),
            });
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after trait methods")?;

        Ok(TraitDef {
            visibility: crate::ast::Visibility::Private, // Will be set by caller
            name,
            lifetime_params,
            type_params,
            methods,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse an impl block
    fn parse_impl(&mut self) -> Result<ImplBlock> {
        let start_span = self.consume(Token::Impl, "Expected 'impl'")?;

        // Parse generic parameters
        let mut lifetime_params = Vec::new();
        let mut type_params = Vec::new();

        if self.check(&Token::Lt) {
            self.advance()?; // consume '<'

            loop {
                if self.check(&Token::SingleQuote) {
                    self.advance()?;
                    let lifetime_name = match self.advance()? {
                        (Token::Identifier(name), _) => format!("'{}", name),
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "lifetime name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    lifetime_params.push(lifetime_name);
                } else {
                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "type parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    type_params.push(param_name);
                }

                if !self.check(&Token::Comma) {
                    break;
                }
                self.advance()?;
            }

            self.consume(Token::Gt, "Expected '>' after generic parameters")?;
        }

        // First, try to parse a type
        let first_type = self.parse_type()?;

        // Check if this is a trait impl (has 'for' keyword)
        let (trait_type, for_type) = if self.check(&Token::For) {
            self.advance()?; // consume 'for'
            let impl_type = self.parse_type()?;
            (Some(first_type), impl_type)
        } else {
            // This is an inherent impl
            (None, first_type)
        };

        self.consume(Token::LeftBrace, "Expected '{' after impl type")?;

        let mut methods = Vec::new();

        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            // For now, only support fn methods in impl blocks
            if !self.check(&Token::Fn) {
                return Err(CompileError::UnexpectedToken {
                    expected: "'fn' for method".to_string(),
                    found: self.peek()?.to_string(),
                    span: self.current_span(),
                });
            }
            let method = self.parse_function()?;
            methods.push(method);
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after impl methods")?;

        Ok(ImplBlock {
            lifetime_params,
            type_params,
            trait_type,
            for_type,
            methods,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a type alias definition
    fn parse_type_alias(&mut self) -> Result<TypeAlias> {
        let start_span = self.consume(Token::Type, "Expected 'type'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "type alias name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Parse generic parameters (lifetimes and types) if present
        let mut lifetime_params = Vec::new();
        let mut type_params = Vec::new();

        if self.check(&Token::Lt) {
            self.advance()?; // consume '<'

            loop {
                // Check if it's a lifetime parameter
                if self.check(&Token::SingleQuote) {
                    self.advance()?; // consume single quote
                    let lifetime_name = match self.advance()? {
                        (Token::Identifier(name), _) => format!("'{}", name),
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "lifetime name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    lifetime_params.push(lifetime_name);
                } else {
                    // It's a type parameter
                    let param_name = match self.advance()? {
                        (Token::Identifier(name), _) => name,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "type parameter name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                    type_params.push(param_name);
                }

                if !self.check(&Token::Comma) {
                    break;
                }
                self.advance()?; // consume ','
            }

            self.consume(Token::Gt, "Expected '>' after generic parameters")?;
        }

        self.consume(Token::Eq, "Expected '=' after type alias name")?;

        let ty = self.parse_type()?;

        let end_span = self.consume(Token::Semicolon, "Expected ';' after type alias")?;

        Ok(TypeAlias {
            visibility: Visibility::Private, // Will be set in parse_item
            name,
            lifetime_params,
            type_params,
            ty,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a top-level `const` or `static` item (N3-09, N3-10).
    ///
    /// ```text
    /// const_item  = [ "pub" ] "const" identifier ":" type "=" expression ";"
    /// static_item = [ "pub" ] "static" [ "mut" ] identifier ":" type "=" expression ";"
    /// ```
    ///
    /// THE TYPE IS MANDATORY, unlike a `let`. A top-level item is read by every
    /// function in the file, so inferring its type would make the meaning of a
    /// name at the top of the program depend on an expression the reader has to
    /// find; and code generation needs a C type for the definition before it has
    /// walked any body.
    fn parse_global(&mut self) -> Result<GlobalDef> {
        let (keyword, start_span) = self.advance()?;
        let kind = match keyword {
            Token::Const => GlobalKind::Const,
            Token::Static => GlobalKind::Static {
                is_mut: if self.check(&Token::Mut) {
                    self.advance()?;
                    true
                } else {
                    false
                },
            },
            token => {
                return Err(CompileError::UnexpectedToken {
                    expected: "'const' or 'static'".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };
        let noun = match kind {
            GlobalKind::Const => "const",
            GlobalKind::Static { .. } => "static",
        };

        // `const mut` is not a spelling: a name with one value has nothing to
        // make mutable. Saying so beats "Expected const item name, found 'mut'".
        if matches!(kind, GlobalKind::Const) && self.check(&Token::Mut) {
            return Err(CompileError::SyntaxError {
                message: "a `const` may not be `mut`: it names a value, not a place. \
                          Write `static mut` for a variable that lives for the whole program"
                    .to_string(),
                span: self.current_span(),
            });
        }

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: format!("{} item name", noun),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        self.consume(
            Token::Colon,
            &format!("Expected ':' and a type after the {} name", noun),
        )?;
        let ty = self.parse_type()?;

        self.consume(
            Token::Eq,
            &format!("Expected '=' and an initialiser after the {} type", noun),
        )?;
        let value = self.parse_expression()?;
        Self::validate_global_initializer(&value, noun)?;

        let end_span = self.consume(
            Token::Semicolon,
            &format!("Expected ';' after the {} item", noun),
        )?;

        Ok(GlobalDef {
            visibility: Visibility::Private, // Will be set in parse_item
            kind,
            name,
            ty,
            value,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// The initialiser forms a top-level item may use, and the refusal by name
    /// for every other one.
    ///
    /// WHY A CLOSED LIST RATHER THAN "ANY EXPRESSION": the item becomes a C
    /// file-scope definition, and C requires a file-scope initialiser to be a
    /// constant expression. Everything permitted here is one after lowering —
    /// literals and operators over literals fold in the translation unit — so
    /// the emitted C is valid by construction rather than by hope. Anything
    /// else (a call, a name, an array or struct literal, an `if`) would either
    /// need running code before `main` or would read another item, and
    /// `initializer element is not constant` from the C compiler is a
    /// diagnostic about generated code that names nothing the author wrote.
    ///
    /// The refusal names the form it saw, so the reader learns the rule from
    /// one attempt rather than from this list.
    fn validate_global_initializer(expr: &Expr, noun: &str) -> Result<()> {
        let refuse = |form: &str, span: Span| -> Result<()> {
            Err(CompileError::SyntaxError {
                message: format!(
                    "a top-level `{}` initialiser has to be a constant expression, \
                     and {} is not one: write a literal, or arithmetic over literals",
                    noun, form
                ),
                span: Some(span),
            })
        };
        match expr {
            Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Char(_) => Ok(()),
            Expr::Binary { left, right, .. } => {
                Self::validate_global_initializer(left, noun)?;
                Self::validate_global_initializer(right, noun)
            }
            Expr::Unary { operand, .. } => Self::validate_global_initializer(operand, noun),
            // A string is a pointer into the arena, produced by a runtime call,
            // so it is refused HERE rather than by the type rule below — the
            // reason is the initialiser, not the type.
            Expr::String(_) => refuse("a string literal", expr.span()),
            Expr::Ident(name) => refuse(
                &format!("the name `{}` (one item may not read another)", name),
                expr.span(),
            ),
            Expr::Call { .. } | Expr::MacroInvocation { .. } => {
                refuse("a call (nothing runs before `main`)", expr.span())
            }
            Expr::ArrayLiteral { .. } | Expr::ArrayRepeat { .. } => {
                refuse("an array literal", expr.span())
            }
            Expr::StructLiteral { .. } => refuse("a struct literal", expr.span()),
            Expr::EnumConstructor { .. } => refuse("an enum constructor", expr.span()),
            Expr::If { .. } => refuse("an `if`", expr.span()),
            Expr::Match { .. } => refuse("a `match`", expr.span()),
            Expr::Block { .. } => refuse("a block", expr.span()),
            Expr::Loop { .. } => refuse("a `loop`", expr.span()),
            Expr::Cast { .. } => refuse("a cast", expr.span()),
            Expr::FieldAccess { .. } => refuse("a field access", expr.span()),
            Expr::Index { .. } => refuse("an index", expr.span()),
            Expr::Range { .. } => refuse("a range", expr.span()),
            Expr::Reference { .. } | Expr::Deref { .. } => refuse("a reference", expr.span()),
            Expr::Question { .. } => refuse("`?`", expr.span()),
            Expr::Await { .. } => refuse("`await`", expr.span()),
            Expr::Tuple { .. } => refuse("a tuple literal", expr.span()),
            Expr::TupleIndex { .. } => refuse("a tuple index", expr.span()),
        }
    }

    /// Parse a macro definition
    fn parse_macro(&mut self) -> Result<MacroDef> {
        let start_span = self.consume(Token::Macro, "Expected 'macro'")?;

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "macro name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Expect '!' after macro name
        self.consume(Token::Not, "Expected '!' after macro name")?;

        // Parse parameter list in parentheses (optional for now)
        let params = if self.check(&Token::LeftParen) {
            self.advance()?; // consume '('
            let mut params = Vec::new();

            while !self.check(&Token::RightParen) && !self.is_at_end() {
                match self.advance()? {
                    (Token::Identifier(param), _) => {
                        params.push(param);

                        if self.check(&Token::Comma) {
                            self.advance()?; // consume ','
                        } else if !self.check(&Token::RightParen) {
                            return Err(CompileError::SyntaxError {
                                message: "Expected ',' or ')' in macro parameters".to_string(),
                                span: self.current_span(),
                            });
                        }
                    }
                    (token, _) => {
                        return Err(CompileError::UnexpectedToken {
                            expected: "parameter name".to_string(),
                            found: token.to_string(),
                            span: self.current_span(),
                        });
                    }
                }
            }

            self.consume(Token::RightParen, "Expected ')' after macro parameters")?;
            params
        } else {
            Vec::new()
        };

        // Parse macro body (for now, just collect tokens between braces)
        self.consume(Token::LeftBrace, "Expected '{' to start macro body")?;

        let mut body = Vec::new();
        let mut brace_depth = 1;

        while brace_depth > 0 && !self.is_at_end() {
            let (token, _) = self.advance()?;

            match &token {
                Token::LeftBrace => {
                    brace_depth += 1;
                    body.push(self.token_to_ast_token(token)?);
                }
                Token::RightBrace => {
                    brace_depth -= 1;
                    if brace_depth > 0 {
                        body.push(self.token_to_ast_token(token)?);
                    }
                }
                _ => {
                    body.push(self.token_to_ast_token(token)?);
                }
            }
        }

        let end_span = self.current_span().unwrap_or(start_span);

        Ok(MacroDef {
            name,
            params,
            body,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Convert lexer token to AST token for macro body
    /// One lexer token as the AST token a macro body or argument list stores.
    ///
    /// EVERY REFUSAL HERE REPLACES A LOSS. The `_` arm used to be
    /// `AstToken::Ident(format!("{:?}", token))`, so a token this table did not
    /// list became an IDENTIFIER SPELLED LIKE ITS RUST DEBUG NAME: `==` in a
    /// macro body reached the type checker as `EqEq` and was reported as
    /// "Undefined variable or function: 'EqEq'", and `let` came back as `Let`.
    /// Neither names anything the author wrote. Measured, both, before this.
    ///
    /// THE LITERAL REFUSALS ARE THE SERIOUS HALF, because that class was
    /// SILENT. `AstToken::Literal` is a `String` and carries no kind, and the
    /// reverse conversion in `src/macros/mod.rs` guesses with `parse::<i64>()`
    /// — so a non-integer literal came back as a `Token::String`. Measured at
    /// e8eb1a9, all three compiling and running:
    ///   `macro pi!() { 3.5 }`   -> `print(pi!())` printed `3.5` as a STRING
    ///   `macro yes!() { true }` -> `print(yes!())` printed `true`, a String
    ///   `macro s!() { "hi" }`   -> `print(s!())` printed NOTHING, because the
    ///                              stored text keeps its quotes, is re-quoted
    ///                              on the way out, and re-lexes as `""` `hi` `""`
    /// A wrong value that compiles is the one outcome a refusal must replace.
    fn token_to_ast_token(&self, token: Token) -> Result<crate::ast::Token> {
        use crate::ast::Token as AstToken;

        let refuse = |what: &str, why: &str| -> Result<crate::ast::Token> {
            Err(CompileError::SyntaxError {
                message: format!(
                    "{} may not appear in a macro body or in a macro argument: {}",
                    what, why
                ),
                span: self.current_span(),
            })
        };

        Ok(match token {
            Token::Identifier(s) => AstToken::Ident(s),
            Token::Integer(n) => AstToken::Literal(n.to_string()),
            Token::String(_) => {
                return refuse(
                    "a string literal",
                    "the token stream stores a literal as text with no kind, so it comes back \
                     re-quoted and re-lexes as two empty strings around a bare identifier",
                )
            }
            Token::Float(_) => {
                return refuse(
                    "a float literal",
                    "the token stream stores a literal as text with no kind, so it comes back \
                     as a string and `3.5` expands to the four characters, not the number",
                )
            }
            Token::Char(_) => {
                return refuse(
                    "a character literal",
                    "the token stream stores a literal as text with no kind, so it comes back \
                     as a string",
                )
            }
            Token::True | Token::False => {
                return refuse(
                    "a boolean literal",
                    "the token stream stores a literal as text with no kind, so `true` comes \
                     back as the string \"true\"",
                )
            }
            Token::LeftParen => AstToken::Punct('('),
            Token::RightParen => AstToken::Punct(')'),
            Token::LeftBrace => AstToken::Punct('{'),
            Token::RightBrace => AstToken::Punct('}'),
            Token::LeftBracket => AstToken::Punct('['),
            Token::RightBracket => AstToken::Punct(']'),
            Token::Semicolon => AstToken::Punct(';'),
            Token::Comma => AstToken::Punct(','),
            Token::Dot => AstToken::Punct('.'),
            Token::Plus => AstToken::Punct('+'),
            Token::Minus => AstToken::Punct('-'),
            Token::Star => AstToken::Punct('*'),
            Token::Slash => AstToken::Punct('/'),
            Token::Not => AstToken::Punct('!'),
            Token::Eq => AstToken::Punct('='),
            // THE REVERSE TABLE IN `src/macros/mod.rs` ALREADY ACCEPTED THESE
            // EIGHT, and this one did not list them, so each was lost on the way
            // IN and could never be lost on the way back. `$` is the one that
            // matters: a macro parameter is substituted by
            // `substitute_template` on seeing `Token::Dollar` followed by a
            // name, and `$x` in a body degraded to the identifier `Dollar`, so
            // NO PARAMETER OF ANY MACRO HAS EVER BEEN SUBSTITUTED. Completing
            // the table is not a redesign of the macro system; it is the row
            // that was missing from it.
            Token::Percent => AstToken::Punct('%'),
            Token::Lt => AstToken::Punct('<'),
            Token::Gt => AstToken::Punct('>'),
            Token::Ampersand => AstToken::Punct('&'),
            Token::Pipe => AstToken::Punct('|'),
            Token::Question => AstToken::Punct('?'),
            Token::Dollar => AstToken::Punct('$'),
            Token::Colon => AstToken::Punct(':'),
            // A MULTI-CHARACTER OPERATOR HAS NO REPRESENTATION HERE.
            // `AstToken::Punct` is ONE `char`, so `==`, `<=`, `&&`, `->`, `::`
            // and `=>` cannot be stored, and storing them as two puncts would
            // be a different program: `= =` is not `==`. Refused by name.
            Token::EqEq
            | Token::Ne
            | Token::Le
            | Token::Ge
            | Token::AndAnd
            | Token::OrOr
            | Token::Arrow
            | Token::FatArrow
            | Token::DoubleColon
            | Token::DotDot => {
                return refuse(
                    &format!("the operator {}", token),
                    "a macro body stores one character per punctuation token, so a \
                     two-character operator cannot be written down; put it in a function and \
                     call that",
                )
            }
            other => {
                return refuse(
                    &format!("{}", other),
                    "the macro token stream can carry identifiers, integer literals and \
                     single-character punctuation, and nothing else",
                )
            }
        })
    }

    /// Parse a block of statements that may have an implicit return
    ///
    /// Returns the statements plus the [`BlockTail`] describing the block's
    /// final statement. The tail is what lets `parse_function` lower a
    /// function-body tail into `Stmt::Return`; without it a tail expression is
    /// indistinguishable from a plain expression statement and silently
    /// generates no `return` in C.
    ///
    /// `if` and `match` are dispatched here rather than through
    /// `parse_statement` for one reason only: their branch tails have to travel
    /// out with them. `parse_statement` returns a bare `Stmt`, and by the time
    /// a `Stmt::If` exists the fact that a branch ended in a `;`-less
    /// expression is gone. The parse itself is unchanged — the same
    /// `parse_if` / `parse_match` bodies run.
    fn parse_block_with_implicit_return(&mut self) -> Result<(Vec<Stmt>, BlockTail)> {
        let mut stmts = Vec::new();
        // The tail of the statement parsed most recently. Overwritten on every
        // iteration, so when the loop exits at `}` it describes the last
        // statement — which is the only one in tail position.
        let mut tail = BlockTail::NoValue;

        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            if self.check(&Token::If) {
                let (stmt, if_tail) = self.parse_if_with_tail()?;
                stmts.push(stmt);
                tail = if_tail;
                continue;
            }
            if self.check(&Token::Match) {
                let (stmt, match_tail) = self.parse_match_with_tail()?;
                stmts.push(stmt);
                tail = match_tail;
                continue;
            }
            // `loop` in STATEMENT position stays a statement. Without this it
            // would reach the expression attempt below and parse as
            // `Expr::Loop`, which is a construct that must produce a value —
            // so `loop { … }` written for its effect would be asked for one.
            // A `loop` in tail position is not a value either: `BlockTail` has
            // no `Loop` shape, and inventing one would mean a function body
            // could return through a `break`, which is a larger claim than
            // N5-07 makes.
            if self.check(&Token::Loop) {
                stmts.push(self.parse_loop()?);
                tail = BlockTail::NoValue;
                continue;
            }

            // Check if this could be the last expression (implicit return)
            let checkpoint = self.current;

            // Try to parse as expression first
            if let Ok(expr) = self.parse_expression() {
                // Check if this is followed by a closing brace (implicit return)
                if self.check(&Token::RightBrace) {
                    // This is an implicit return
                    stmts.push(Stmt::Expr(expr));
                    tail = BlockTail::Expr;
                    break;
                } else if self.check(&Token::Semicolon) {
                    // Normal expression statement
                    self.advance()?; // consume ';'
                    stmts.push(Stmt::Expr(expr));
                    tail = BlockTail::NoValue;
                } else {
                    // Rewind and parse as statement
                    self.current = checkpoint;
                    stmts.push(self.parse_statement()?);
                    tail = BlockTail::NoValue;
                }
            } else {
                // Rewind and parse as statement
                self.current = checkpoint;
                stmts.push(self.parse_statement()?);
                tail = BlockTail::NoValue;
            }
        }

        Ok((stmts, tail))
    }

    /// Parse a statement
    pub fn parse_statement(&mut self) -> Result<Stmt> {
        match self.peek()? {
            Token::Let => self.parse_let(),
            Token::Return => self.parse_return(),
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::Loop => self.parse_loop(),
            Token::For => self.parse_for(),
            Token::Break => self.parse_break(),
            Token::Continue => self.parse_continue(),
            Token::Match => self.parse_match(),
            Token::Unsafe => self.parse_unsafe(),
            Token::Identifier(_) | Token::Star => {
                // Could be assignment or expression statement
                // Parse the left-hand side as an expression first
                let checkpoint = self.current;
                let expr = self.parse_expression()?; // Parse full expression including dereference

                // Check if this is an assignment, plain or compound.
                //
                // COMPOUND ASSIGNMENT IS DESUGARED HERE (N5-13):
                // `t op= v` becomes `t = t op v`. Two reasons, and neither is
                // taste. grammar.ebnf gives the normative form as a STATEMENT
                // (`place ( "+=" | … ) expression ";"`) rather than as an
                // operator, so there is nothing to put in `BinOp`; and the
                // reviewed test `test_compound_assignment_operators` reads the
                // emitted C for `x = x + 1;`, not for C's own `x += 1`.
                //
                // THE RESIDUAL THIS CHOICE BUYS, STATED RATHER THAN SOLVED:
                // the target appears twice in the desugaring, so it is
                // EVALUATED twice. `x`, `s.field` and `*p` cannot tell the
                // difference. `a[next()] += 1` can — it calls `next()` twice.
                // Fixing it needs a place-expression lowering that binds the
                // subscript to a temporary first, which is a change to how
                // every assignment target is emitted and is not this row.
                let compound_op = match self.peek() {
                    Ok(Token::PlusEq) => Some(BinOp::Add),
                    Ok(Token::MinusEq) => Some(BinOp::Sub),
                    Ok(Token::StarEq) => Some(BinOp::Mul),
                    Ok(Token::SlashEq) => Some(BinOp::Div),
                    Ok(Token::PercentEq) => Some(BinOp::Mod),
                    _ => None,
                };
                if compound_op.is_some() || (self.check(&Token::Eq) && !self.check_at(1, &Token::Eq))
                {
                    // This is an assignment
                    let start_span = expr.span();
                    self.advance()?; // consume '=' or the compound operator
                    let rhs = self.parse_expression()?;
                    let end_span =
                        self.consume(Token::Semicolon, "Expected ';' after assignment")?;

                    let value = match compound_op {
                        None => rhs,
                        Some(op) => {
                            let span = Span::new(
                                start_span.start,
                                end_span.end,
                                start_span.line,
                                start_span.column,
                            );
                            Expr::Binary {
                                left: Box::new(expr.clone()),
                                op,
                                right: Box::new(rhs),
                                span,
                            }
                        }
                    };

                    // Convert expression to assignment target
                    let target = match expr {
                        Expr::Ident(name) => AssignTarget::Ident(name),
                        Expr::Index { array, index, .. } => AssignTarget::Index { array, index },
                        Expr::FieldAccess { object, field, .. } => {
                            AssignTarget::FieldAccess { object, field }
                        }
                        Expr::Deref { expr, .. } => AssignTarget::Deref { expr },
                        _ => {
                            return Err(CompileError::SyntaxError {
                                message: "Invalid assignment target".to_string(),
                                span: self.current_span(),
                            });
                        }
                    };

                    return Ok(Stmt::Assign {
                        target,
                        value,
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    });
                }

                // Not an assignment, continue parsing as expression
                self.current = checkpoint;
                let expr = self.parse_expression()?;
                self.consume(Token::Semicolon, "Expected ';' after expression")?;
                Ok(Stmt::Expr(expr))
            }
            _ => {
                // Expression statement
                let expr = self.parse_expression()?;
                self.consume(Token::Semicolon, "Expected ';' after expression")?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    /// Parse a return statement
    fn parse_return(&mut self) -> Result<Stmt> {
        self.consume(Token::Return, "Expected 'return'")?;

        if self.check(&Token::Semicolon) {
            self.advance()?;
            Ok(Stmt::Return(None))
        } else {
            let expr = self.parse_expression()?;
            self.consume(Token::Semicolon, "Expected ';' after return value")?;
            Ok(Stmt::Return(Some(expr)))
        }
    }

    /// Parse a let statement
    fn parse_let(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::Let, "Expected 'let'")?;

        // Check for optional 'mut' keyword
        let mutable = if self.check(&Token::Mut) {
            self.advance()?; // consume 'mut'
            true
        } else {
            false
        };

        let name = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "variable name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        // Optional type annotation
        let ty = if self.check(&Token::Colon) {
            self.advance()?; // consume ':'
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(Token::Eq, "Expected '=' after variable name")?;
        let value = self.parse_expression()?;
        let end_span = self.consume(Token::Semicolon, "Expected ';' after let statement")?;

        Ok(Stmt::Let {
            name,
            ty,
            value,
            mutable,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse an if statement
    fn parse_if(&mut self) -> Result<Stmt> {
        Ok(self.parse_if_with_tail()?.0)
    }

    /// Parse an if statement, keeping the tail of each branch.
    ///
    /// Tail expressions inside if/else branches stay plain expression
    /// statements here — whether they become returns is decided in
    /// `parse_function`, which is the only place that knows the return type.
    /// What this function must not do is *discard* the branch tails, because
    /// nothing downstream can reconstruct them (see [`BlockTail`]).
    fn parse_if_with_tail(&mut self) -> Result<(Stmt, BlockTail)> {
        let start_span = self.consume(Token::If, "Expected 'if'")?;

        let condition = self.parse_expression()?;

        self.consume(Token::LeftBrace, "Expected '{' after if condition")?;

        let (then_branch, then_tail) = self.parse_block_with_implicit_return()?;

        self.consume(Token::RightBrace, "Expected '}' after if body")?;

        let mut else_tail = None;
        let else_branch = if self.check(&Token::Else) {
            self.advance()?; // consume 'else'

            if self.check(&Token::If) {
                // `else if` (N5-06). There is no `ElseIf` node and there does
                // not need to be one: the chain IS the nesting the corpus used
                // to have to write by hand, so the rest of the compiler sees
                // exactly the shape it already handles.
                //
                // The tail travels with it. An `else if` arm is a branch like
                // any other, and dropping its `BlockTail` here would silently
                // un-return the arms of every tail-position chain — the D3
                // defect one level further in (see `BlockTail`).
                let (nested, nested_tail) = self.parse_if_with_tail()?;
                else_tail = Some(Box::new(nested_tail));
                Some(vec![nested])
            } else {
                self.consume(Token::LeftBrace, "Expected '{' after else")?;

                let (else_stmts, tail) = self.parse_block_with_implicit_return()?;
                else_tail = Some(Box::new(tail));

                let _end_span = self.consume(Token::RightBrace, "Expected '}' after else body")?;
                Some(else_stmts)
            }
        } else {
            None
        };

        let end_span = self.tokens[self.current - 1].1;
        let span = Span::new(
            start_span.start,
            end_span.end,
            start_span.line,
            start_span.column,
        );

        Ok((
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            },
            BlockTail::If {
                then_tail: Box::new(then_tail),
                else_tail,
                span,
            },
        ))
    }

    /// Parse an `if` in VALUE position — `let x = if c { 1 } else { 2 };`.
    ///
    /// The parse is the STATEMENT parse: `parse_if_with_tail` already reads
    /// every branch and already reports which statement of each branch was in
    /// tail position, and those two facts are exactly what a value `if` needs.
    /// Writing a second `if` parser here would be a second grammar that could
    /// drift from the first, and `else if` chains would have to be implemented
    /// twice.
    ///
    /// What this adds is the REINTERPRETATION: statements plus a `BlockTail`
    /// become statements plus a value expression (see [`Parser::split_value_block`]).
    fn parse_if_expression(&mut self) -> Result<Expr> {
        let (stmt, tail) = self.parse_if_with_tail()?;
        Ok(Self::if_stmt_into_expr(stmt, &tail))
    }

    /// Parse a `match` in VALUE position — `let x = match e { … };` (N5-04).
    ///
    /// Same reuse as `parse_if_expression`: `parse_match_with_tail` already
    /// reads the arms and already reports which statement of each arm was in
    /// tail position, and those are exactly the two facts a value `match`
    /// needs. One grammar, one place for arm syntax to change.
    fn parse_match_expression(&mut self) -> Result<Expr> {
        let (stmt, tail) = self.parse_match_with_tail()?;
        Ok(Self::match_stmt_into_expr(stmt, &tail))
    }

    /// Reinterpret a parsed `Stmt::Match` + its `BlockTail` as an `Expr::Match`.
    fn match_stmt_into_expr(stmt: Stmt, tail: &BlockTail) -> Expr {
        let Stmt::Match { expr, arms, span } = stmt else {
            unreachable!("parse_match_with_tail returned a statement that is not a `match`");
        };
        let arm_tails = match tail {
            BlockTail::Match { arm_tails, .. } => arm_tails.as_slice(),
            _ => unreachable!("parse_match_with_tail returned a tail that is not a `match` tail"),
        };

        let arms = arms
            .into_iter()
            .enumerate()
            .map(|(i, arm)| {
                // The two vectors are built together by `parse_match_with_tail`
                // and cannot disagree; a missing tail is read as "this arm has
                // no value", which the type checker refuses by name.
                let arm_tail = arm_tails.get(i).unwrap_or(&BlockTail::NoValue);
                let (body, value) = Self::split_value_block(arm.body, arm_tail);
                MatchArmValue {
                    pattern: arm.pattern,
                    guard: arm.guard,
                    body,
                    value: value.map(|v| *v),
                }
            })
            .collect();

        Expr::Match {
            expr: Box::new(expr),
            arms,
            span,
        }
    }

    /// Parse a block in VALUE position — `let x = { let a = 1; a + 1 };`.
    ///
    /// The opening `{` has already been consumed by `parse_primary`.
    fn parse_block_expression(&mut self, start_span: Span) -> Result<Expr> {
        let (stmts, tail) = self.parse_block_with_implicit_return()?;
        let end_span = self.consume(Token::RightBrace, "Expected '}' after block expression")?;
        let (stmts, value) = Self::split_value_block(stmts, &tail);
        Ok(Expr::Block {
            stmts,
            value,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Reinterpret a parsed `Stmt::If` + its `BlockTail` as an `Expr::If`.
    ///
    /// Total by construction: anything that is not the `if` shape the tail
    /// describes yields a branch with no value, which the type checker refuses
    /// by name. This function never invents a diagnostic of its own, because
    /// the shape it would be complaining about is one it built itself.
    fn if_stmt_into_expr(stmt: Stmt, tail: &BlockTail) -> Expr {
        let Stmt::If {
            condition,
            then_branch,
            else_branch,
            span,
        } = stmt
        else {
            // `parse_if_with_tail` returns nothing else.
            unreachable!("parse_if_with_tail returned a statement that is not an `if`");
        };
        let (then_tail, else_tail) = match tail {
            BlockTail::If {
                then_tail,
                else_tail,
                ..
            } => (then_tail.as_ref(), else_tail.as_ref()),
            _ => unreachable!("parse_if_with_tail returned a tail that is not an `if` tail"),
        };

        let (then_branch, then_value) = Self::split_value_block(then_branch, then_tail);
        let (else_branch, else_value) = match (else_branch, else_tail) {
            (Some(stmts), Some(t)) => {
                let (stmts, value) = Self::split_value_block(stmts, t);
                (Some(stmts), value)
            }
            // No `else` at all: the type checker names the missing branch.
            (eb, _) => (eb, None),
        };

        Expr::If {
            condition: Box::new(condition),
            then_branch,
            then_value,
            else_branch,
            else_value,
            span,
        }
    }

    /// Split a parsed block into "the statements it runs" and "the value it
    /// produces", using the tail the block parser recorded.
    ///
    /// The tail is the only witness that the last statement was written
    /// without a `;`; by the time there is a `Vec<Stmt>` the two spellings are
    /// the same node. That is the whole reason `BlockTail` exists, and it is
    /// why this cannot be a pass over the AST.
    ///
    /// A block whose tail is itself an `if` produces an `Expr::If` value — so
    /// `let x = if a { if b { 1 } else { 2 } } else { 3 };` nests without the
    /// inner `if` ever being parsed a second time.
    fn split_value_block(mut stmts: Vec<Stmt>, tail: &BlockTail) -> (Vec<Stmt>, Option<Box<Expr>>) {
        match tail {
            BlockTail::Expr => match stmts.pop() {
                Some(Stmt::Expr(expr)) => (stmts, Some(Box::new(expr))),
                // Put it back: the tail said `Expr` and the statement is not
                // one, so this block has no value the checker can use.
                Some(other) => {
                    stmts.push(other);
                    (stmts, None)
                }
                None => (stmts, None),
            },
            BlockTail::If { .. } => match stmts.pop() {
                Some(stmt @ Stmt::If { .. }) => {
                    let expr = Self::if_stmt_into_expr(stmt, tail);
                    (stmts, Some(Box::new(expr)))
                }
                Some(other) => {
                    stmts.push(other);
                    (stmts, None)
                }
                None => (stmts, None),
            },
            BlockTail::Match { .. } => match stmts.pop() {
                Some(stmt @ Stmt::Match { .. }) => {
                    let expr = Self::match_stmt_into_expr(stmt, tail);
                    (stmts, Some(Box::new(expr)))
                }
                Some(other) => {
                    stmts.push(other);
                    (stmts, None)
                }
                None => (stmts, None),
            },
            BlockTail::NoValue => (stmts, None),
        }
    }

    /// Parse a while statement
    fn parse_while(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::While, "Expected 'while'")?;

        let condition = self.parse_expression()?;

        self.consume(Token::LeftBrace, "Expected '{' after while condition")?;

        let mut body = Vec::new();
        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            body.push(self.parse_statement()?);
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after while body")?;

        Ok(Stmt::While {
            condition,
            body,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a for statement
    fn parse_for(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::For, "Expected 'for'")?;

        // Parse the loop variable
        let var = match self.advance()? {
            (Token::Identifier(name), _) => name,
            (token, _) => {
                return Err(CompileError::UnexpectedToken {
                    expected: "variable name".to_string(),
                    found: token.to_string(),
                    span: self.current_span(),
                });
            }
        };

        self.consume(Token::In, "Expected 'in' after for variable")?;

        // Parse the iterator expression (array or range)
        let iter = self.parse_expression()?;

        self.consume(Token::LeftBrace, "Expected '{' after for header")?;

        let mut body = Vec::new();
        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            body.push(self.parse_statement()?);
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after for body")?;

        Ok(Stmt::For {
            var,
            iter,
            body,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a break statement, with or without a value.
    ///
    /// `break;` and `break <expr>;`. The two are told apart by looking for the
    /// `;` rather than by trying to parse an expression and backtracking: every
    /// token that can start an expression is a token that cannot follow `break`
    /// otherwise, so the lookahead is exact and a failed expression parse here
    /// is a real error worth reporting at the operand.
    fn parse_break(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::Break, "Expected 'break'")?;

        let value = if self.check(&Token::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };

        let end_span = self.consume(Token::Semicolon, "Expected ';' after break")?;

        Ok(Stmt::Break {
            value,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a `loop` statement (N5-07).
    fn parse_loop(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::Loop, "Expected 'loop'")?;
        self.consume(Token::LeftBrace, "Expected '{' after loop")?;

        let mut body = Vec::new();
        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            body.push(self.parse_statement()?);
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after loop body")?;

        Ok(Stmt::Loop {
            body,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a `loop` in VALUE position — `let x = loop { …; break v; };`.
    ///
    /// Same parse as the statement form, reinterpreted. There is nothing to
    /// split out the way a block's tail is split out: a `loop` has no tail
    /// expression, and its value arrives through the `break`s in its body,
    /// which the type checker and code generator find by walking it.
    fn parse_loop_expression(&mut self) -> Result<Expr> {
        let Stmt::Loop { body, span } = self.parse_loop()? else {
            unreachable!("parse_loop returned a statement that is not a `loop`");
        };
        Ok(Expr::Loop { body, span })
    }

    /// Parse a continue statement
    fn parse_continue(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::Continue, "Expected 'continue'")?;
        let end_span = self.consume(Token::Semicolon, "Expected ';' after continue")?;

        Ok(Stmt::Continue {
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// Parse a match statement
    fn parse_match(&mut self) -> Result<Stmt> {
        Ok(self.parse_match_with_tail()?.0)
    }

    /// Parse a match statement, keeping the tail of each arm.
    ///
    /// Only the `pattern => expression,` arm form can carry a tail expression:
    /// a block-bodied arm parses its statements with `parse_statement`, which
    /// requires a `;`, so `Red => { 1 }` is a parse error today ("Expected ';'
    /// after expression", measured). The tail therefore comes out of exactly
    /// one branch below.
    fn parse_match_with_tail(&mut self) -> Result<(Stmt, BlockTail)> {
        let start_span = self.consume(Token::Match, "Expected 'match'")?;

        let expr = self.parse_expression()?;

        self.consume(Token::LeftBrace, "Expected '{' after match expression")?;

        let mut arms = Vec::new();
        let mut arm_tails = Vec::new();

        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            // Parse pattern
            let pattern = self.parse_pattern()?;

            // N6-09. `pattern if cond =>`. Parsed with the full expression
            // parser, so a guard is any expression the language has — including
            // the value forms, which is why codegen has to give the guard a
            // statement position to hoist into.
            let guard = if self.check(&Token::If) {
                self.advance()?; // consume 'if'
                Some(self.parse_expression()?)
            } else {
                None
            };

            self.consume(Token::FatArrow, "Expected '=>' after pattern")?;

            // Parse arm body. Both shapes below set `arm_tail`, so it is
            // declared without one: a default here would be a fourth answer to
            // "did this arm end in a value", and the two that exist already
            // disagree often enough.
            let arm_tail;
            let body = if self.check(&Token::LeftBrace) {
                // Block body.
                //
                // Parsed with the implicit-return block parser, not with a bare
                // `parse_statement` loop. The bare loop demanded a `;` on every
                // statement, which made `Circle => { 1 }` the parse error
                // "Expected ';' after expression" — a block-bodied arm could
                // hold statements but could never hold a VALUE, so the only
                // arm form that could produce one was `pattern => expr,`. The
                // tail travels out with the arm for the same reason it does
                // everywhere else (see `BlockTail`).
                self.advance()?; // consume '{'
                let (stmts, block_tail) = self.parse_block_with_implicit_return()?;
                arm_tail = block_tail;
                self.consume(Token::RightBrace, "Expected '}' after match arm body")?;

                // `match_arm = pattern "=>" ( block | expression ) [ ',' ]`
                // (docs/specification/grammar.ebnf:234). The comma is optional
                // after a block body too; leaving it unconsumed made the next
                // iteration read it as a pattern.
                if self.check(&Token::Comma) {
                    self.advance()?;
                }

                stmts
            } else {
                // Single expression body
                let expr = self.parse_expression()?;

                // Comma is optional if this is the last arm
                if !self.check(&Token::RightBrace) {
                    self.consume(Token::Comma, "Expected ',' after match arm expression")?;
                } else if self.check(&Token::Comma) {
                    // Allow optional trailing comma
                    self.advance()?;
                }

                // `Red => 1,` is a value in tail position: the arm body is a
                // single `Stmt::Expr` that was never terminated by a `;`.
                arm_tail = BlockTail::Expr;
                vec![Stmt::Expr(expr)]
            };

            arms.push(MatchArm {
                pattern,
                guard,
                body,
            });
            arm_tails.push(arm_tail);
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after match arms")?;
        let span = Span::new(
            start_span.start,
            end_span.end,
            start_span.line,
            start_span.column,
        );

        Ok((
            Stmt::Match { expr, arms, span },
            BlockTail::Match { arm_tails, span },
        ))
    }

    /// Parse an unsafe block
    fn parse_unsafe(&mut self) -> Result<Stmt> {
        let start_span = self.consume(Token::Unsafe, "Expected 'unsafe'")?;

        self.consume(Token::LeftBrace, "Expected '{' after unsafe")?;

        let mut body = Vec::new();
        while !self.check(&Token::RightBrace) && !self.is_at_end() {
            body.push(self.parse_statement()?);
        }

        let end_span = self.consume(Token::RightBrace, "Expected '}' after unsafe block")?;

        Ok(Stmt::Unsafe {
            body,
            span: Span::new(
                start_span.start,
                end_span.end,
                start_span.line,
                start_span.column,
            ),
        })
    }

    /// `-` then an integer, in pattern position.
    ///
    /// THE MINUS IS READ HERE AND NOT BY AN EXPRESSION PARSER: `-1` in pattern
    /// position is one literal, not a unary operator applied to one, and there
    /// is no other expression a pattern may hold. Only an integer may follow it
    /// — `-"x"` and `-true` are refused by name rather than parsed into a shape
    /// the type checker would have to refuse later.
    fn parse_negative_pattern_integer(&mut self) -> Result<i64> {
        self.advance()?; // consume '-'
        match self.advance()? {
            (Token::Integer(value), _) => Ok(-value),
            (found, _) => Err(CompileError::UnexpectedToken {
                expected: "an integer literal after `-` in a pattern".to_string(),
                found: found.to_string(),
                span: self.current_span(),
            }),
        }
    }

    /// Having read a literal, decide whether it was a literal or a range's low
    /// end (N6-03).
    ///
    /// Both endpoints are required, so a `..` with nothing usable after it is an
    /// error here and not a silently open range.
    fn maybe_range(&mut self, lo: PatternLiteral) -> Result<Pattern> {
        let inclusive = match self.peek() {
            Ok(Token::DotDot) => false,
            Ok(Token::DotDotEq) => true,
            _ => return Ok(Pattern::Literal(lo)),
        };
        self.advance()?; // consume '..' or '..='
        let hi = match self.peek()?.clone() {
            Token::Minus => PatternLiteral::Int(self.parse_negative_pattern_integer()?),
            Token::Integer(value) => {
                self.advance()?;
                PatternLiteral::Int(value)
            }
            Token::String(value) => {
                self.advance()?;
                PatternLiteral::Str(value)
            }
            Token::True => {
                self.advance()?;
                PatternLiteral::Bool(true)
            }
            Token::False => {
                self.advance()?;
                PatternLiteral::Bool(false)
            }
            found => {
                return Err(CompileError::UnexpectedToken {
                    expected: "a literal for the high end of this range pattern (open-ended \
                               range patterns are not in the language)"
                        .to_string(),
                    found: found.to_string(),
                    span: self.current_span(),
                });
            }
        };
        Ok(Pattern::Range { lo, hi, inclusive })
    }

    /// Parse a pattern, including any `|` alternatives (N6-07).
    ///
    /// The entry point every caller uses. `|` binds LOOSER than `@`, so
    /// `n @ 1 | 2` is `(n @ 1) | 2` — which the type checker then refuses,
    /// because an alternative may not bind. There is no grouping form for
    /// patterns yet, so `n @ (1 | 2)` is unwritable rather than silently
    /// reinterpreted.
    fn parse_pattern(&mut self) -> Result<Pattern> {
        let first = self.parse_pattern_primary()?;
        if !self.check(&Token::Pipe) {
            return Ok(first);
        }
        let mut alternatives = vec![first];
        while self.check(&Token::Pipe) {
            self.advance()?; // consume '|'
            alternatives.push(self.parse_pattern_primary()?);
        }
        Ok(Pattern::Or(alternatives))
    }

    /// Parse one pattern, stopping before any `|`.
    ///
    /// A literal here may turn out to be the low end of a RANGE (N6-03), so the
    /// literal cases funnel through `maybe_range`.
    fn parse_pattern_primary(&mut self) -> Result<Pattern> {
        // First, peek and clone the token to avoid borrowing issues
        let token = self.peek()?.clone();

        match token {
            Token::Underscore => {
                self.advance()?;
                Ok(Pattern::Wildcard)
            }
            // N6-02. THE MINUS IS READ HERE AND NOT BY AN EXPRESSION PARSER:
            // `-1` in pattern position is one literal, not a unary operator
            // applied to one, and there is no other expression a pattern may
            // hold. Only an integer may follow it — `-"x"` and `-true` are
            // refused by name rather than parsed into a shape typeck would have
            // to refuse later.
            Token::Minus => {
                let value = self.parse_negative_pattern_integer()?;
                self.maybe_range(PatternLiteral::Int(value))
            }
            Token::Integer(value) => {
                self.advance()?;
                self.maybe_range(PatternLiteral::Int(value))
            }
            Token::String(value) => {
                self.advance()?;
                self.maybe_range(PatternLiteral::Str(value))
            }
            Token::True => {
                self.advance()?;
                self.maybe_range(PatternLiteral::Bool(true))
            }
            Token::False => {
                self.advance()?;
                self.maybe_range(PatternLiteral::Bool(false))
            }
            // N6-05. A tuple pattern. The same arity rule as the VALUE side:
            // `(p)` is grouping, and grouping is not a pattern form, so it is
            // refused by name instead of quietly meaning its inner pattern.
            Token::LeftParen => {
                let start = self.advance()?.1;
                let mut elements = Vec::new();
                if !self.check(&Token::RightParen) {
                    loop {
                        elements.push(self.parse_pattern()?);
                        if !self.check(&Token::Comma) {
                            break;
                        }
                        self.advance()?; // consume ','
                        if self.check(&Token::RightParen) {
                            break; // trailing comma
                        }
                    }
                }
                self.consume(Token::RightParen, "Expected ')' after tuple pattern")?;
                if elements.len() < 2 {
                    let _ = start;
                    return Err(CompileError::UnexpectedToken {
                        expected: "a tuple pattern to have at least two elements; grouping is not \
                                   a pattern form, so `(p)` is not a way to write `p`"
                            .to_string(),
                        found: format!("a pattern with {} element(s) in parentheses", elements.len()),
                        span: self.current_span(),
                    });
                }
                Ok(Pattern::Tuple(elements))
            }
            // N6-03. A range needs a LOW end, and `..5` has none. Refused here
            // by name: the spec's two range forms are both closed, so this is a
            // form the language does not have rather than one nobody wrote yet.
            Token::DotDot | Token::DotDotEq => Err(CompileError::UnexpectedToken {
                expected: "a range pattern to have both endpoints, as `lo..hi` or `lo..=hi` \
                           (open-ended range patterns are not in the language)"
                    .to_string(),
                found: token.to_string(),
                span: self.current_span(),
            }),
            Token::Identifier(name) => {
                self.advance()?;

                // Check if this is an enum pattern
                if self.check(&Token::DoubleColon) {
                    self.advance()?; // consume '::'

                    let variant = match self.advance()? {
                        (Token::Identifier(v), _) => v,
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "variant name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };

                    // Check for pattern data
                    let data = if self.check(&Token::LeftParen) {
                        // Tuple pattern
                        self.advance()?; // consume '('
                        let mut patterns = Vec::new();

                        if !self.check(&Token::RightParen) {
                            loop {
                                patterns.push(self.parse_pattern()?);
                                if !self.check(&Token::Comma) {
                                    break;
                                }
                                self.advance()?; // consume ','
                            }
                        }

                        self.consume(Token::RightParen, "Expected ')' after tuple pattern")?;
                        Some(PatternData::Tuple(patterns))
                    } else if self.check(&Token::LeftBrace) {
                        // Struct pattern
                        self.advance()?; // consume '{'
                        let mut fields = Vec::new();

                        while !self.check(&Token::RightBrace) && !self.is_at_end() {
                            let field_name = match self.advance()? {
                                (Token::Identifier(fname), _) => fname,
                                (token, _) => {
                                    return Err(CompileError::UnexpectedToken {
                                        expected: "field name".to_string(),
                                        found: token.to_string(),
                                        span: self.current_span(),
                                    });
                                }
                            };

                            self.consume(Token::Colon, "Expected ':' after field name in pattern")?;
                            let field_pattern = self.parse_pattern()?;

                            fields.push((field_name, field_pattern));

                            if !self.check(&Token::RightBrace) {
                                self.consume(Token::Comma, "Expected ',' after field pattern")?;
                            }
                        }

                        self.consume(Token::RightBrace, "Expected '}' after struct pattern")?;
                        Some(PatternData::Struct(fields))
                    } else {
                        None
                    };

                    Ok(Pattern::EnumPattern {
                        enum_name: name,
                        variant,
                        data,
                    })
                } else if self.check(&Token::At) {
                    // N6-08. `name @ pattern`. The inner is a PRIMARY pattern:
                    // binding an alternative set would need the grouping form
                    // this language does not have, and reading `n @ 1 | 2` as
                    // `n @ (1 | 2)` would give `|` two meanings depending on
                    // what preceded it.
                    self.advance()?; // consume '@'
                    let inner = self.parse_pattern_primary()?;
                    Ok(Pattern::Binding {
                        name,
                        inner: Box::new(inner),
                    })
                } else {
                    // Simple identifier pattern
                    Ok(Pattern::Ident(name))
                }
            }
            _ => Err(CompileError::UnexpectedToken {
                expected: "pattern".to_string(),
                found: token.to_string(),
                span: self.current_span(),
            }),
        }
    }

    /// Parse an expression
    pub fn parse_expression(&mut self) -> Result<Expr> {
        self.parse_range()
    }

    /// Parse range operators (..)
    fn parse_range(&mut self) -> Result<Expr> {
        let mut left = self.parse_logical_or()?;

        while let Ok(token) = self.peek() {
            // `..` and `..=` (N5-14). Both are the same production with one bit
            // different; `..=` is its own token, so the lexer's longest match
            // has already told them apart.
            let inclusive = match token {
                Token::DotDot => false,
                Token::DotDotEq => true,
                _ => break,
            };

            let left_span = Self::expr_span(&left);
            self.advance()?; // consume '..' or '..='
            let right = self.parse_logical_or()?;
            let right_span = Self::expr_span(&right);
            left = Expr::Range {
                start: Box::new(left),
                end: Box::new(right),
                inclusive,
                span: Span::new(
                    left_span.start,
                    right_span.end,
                    left_span.line,
                    left_span.column,
                ),
            };
        }

        Ok(left)
    }

    /// Parse logical OR (||)
    fn parse_logical_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_logical_and()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::OrOr => {
                    let left_span = Self::expr_span(&left);
                    let _ = self.advance()?; // consume '||'
                    let right = self.parse_logical_and()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op: BinOp::Or,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse logical AND (&&)
    fn parse_logical_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_equality()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::AndAnd => {
                    let left_span = Self::expr_span(&left);
                    let _ = self.advance()?; // consume '&&'
                    let right = self.parse_equality()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op: BinOp::And,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse equality operators (==, !=)
    fn parse_equality(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::EqEq | Token::Ne => {
                    let left_span = Self::expr_span(&left);
                    let op = match self.advance()?.0 {
                        Token::EqEq => BinOp::Eq,
                        Token::Ne => BinOp::Ne,
                        _ => unreachable!(),
                    };
                    let right = self.parse_comparison()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse comparison operators (<, >, <=, >=)
    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_bitor()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::Lt | Token::Gt | Token::Le | Token::Ge => {
                    let left_span = Self::expr_span(&left);
                    let op = match self.advance()?.0 {
                        Token::Lt => BinOp::Lt,
                        Token::Gt => BinOp::Gt,
                        Token::Le => BinOp::Le,
                        Token::Ge => BinOp::Ge,
                        _ => unreachable!(),
                    };
                    let right = self.parse_bitor()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Is the current token the start of a `>>` written as two ADJACENT `>`?
    ///
    /// THERE IS NO `>>` LEXER TOKEN, AND THAT IS DELIBERATE (see
    /// `src/lexer/token.rs`): `Option<Vec<Stmt>>` closes two generic argument
    /// lists with two `>` in a row, and a longest-match `>>` token would eat
    /// both and break every nested generic in the tree. So the shift operator
    /// is recognised HERE, where the two readings can be told apart by the one
    /// thing that distinguishes them — whether the characters touch.
    ///
    /// `a > > b` is therefore not a shift, and `Vec<Vec<i64>>` is not one
    /// either, because in the type the `>`s are adjacent but no expression
    /// parser ever looks at them.
    fn check_shr(&self) -> bool {
        let (Some((Token::Gt, first)), Some((Token::Gt, second))) = (
            self.tokens.get(self.current),
            self.tokens.get(self.current + 1),
        ) else {
            return false;
        };
        first.end == second.start
    }

    /// Parse bitwise OR (`|`) — the loosest of the bitwise levels (N5-12).
    ///
    /// The three bitwise levels and the shifts sit BETWEEN the comparisons and
    /// addition, which is Rust's order and not C's: C binds `==` tighter than
    /// `&`, so `a & b == c` means `a & (b == c)` there — a wart C compilers
    /// themselves warn about. Emitted C is fully parenthesised, so the
    /// difference cannot leak into the object code.
    fn parse_bitor(&mut self) -> Result<Expr> {
        let mut left = self.parse_bitxor()?;

        while matches!(self.peek(), Ok(Token::Pipe)) {
            let left_span = Self::expr_span(&left);
            self.advance()?; // consume '|'
            let right = self.parse_bitxor()?;
            let right_span = Self::expr_span(&right);
            left = Expr::Binary {
                left: Box::new(left),
                op: BinOp::BitOr,
                right: Box::new(right),
                span: Span::new(
                    left_span.start,
                    right_span.end,
                    left_span.line,
                    left_span.column,
                ),
            };
        }

        Ok(left)
    }

    /// Parse bitwise XOR (`^`).
    fn parse_bitxor(&mut self) -> Result<Expr> {
        let mut left = self.parse_bitand()?;

        while matches!(self.peek(), Ok(Token::Caret)) {
            let left_span = Self::expr_span(&left);
            self.advance()?; // consume '^'
            let right = self.parse_bitand()?;
            let right_span = Self::expr_span(&right);
            left = Expr::Binary {
                left: Box::new(left),
                op: BinOp::BitXor,
                right: Box::new(right),
                span: Span::new(
                    left_span.start,
                    right_span.end,
                    left_span.line,
                    left_span.column,
                ),
            };
        }

        Ok(left)
    }

    /// Parse bitwise AND (`&`).
    ///
    /// The same `&` that starts a reference. There is no ambiguity to resolve:
    /// a reference is a PREFIX and is read by `parse_unary` before any operand
    /// exists, while this loop only looks for `&` once a complete left operand
    /// has been parsed. `&&` is its own token, so it cannot be mistaken for
    /// two of these.
    fn parse_bitand(&mut self) -> Result<Expr> {
        let mut left = self.parse_shift()?;

        while matches!(self.peek(), Ok(Token::Ampersand)) {
            let left_span = Self::expr_span(&left);
            self.advance()?; // consume '&'
            let right = self.parse_shift()?;
            let right_span = Self::expr_span(&right);
            left = Expr::Binary {
                left: Box::new(left),
                op: BinOp::BitAnd,
                right: Box::new(right),
                span: Span::new(
                    left_span.start,
                    right_span.end,
                    left_span.line,
                    left_span.column,
                ),
            };
        }

        Ok(left)
    }

    /// Parse the shifts (`<<`, `>>`).
    fn parse_shift(&mut self) -> Result<Expr> {
        let mut left = self.parse_addition()?;

        loop {
            let op = if matches!(self.peek(), Ok(Token::Shl)) {
                self.advance()?; // consume '<<'
                BinOp::Shl
            } else if self.check_shr() {
                self.advance()?; // consume the first '>'
                self.advance()?; // consume the second '>'
                BinOp::Shr
            } else {
                break;
            };

            let left_span = Self::expr_span(&left);
            let right = self.parse_addition()?;
            let right_span = Self::expr_span(&right);
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span: Span::new(
                    left_span.start,
                    right_span.end,
                    left_span.line,
                    left_span.column,
                ),
            };
        }

        Ok(left)
    }

    /// Parse `as` casts (N5-15).
    ///
    /// Sits between multiplication and unary, which is Rust's placement:
    /// `10 / 4.0 as i64` is `10 / (4.0 as i64)`, and `-x as i64` is
    /// `(-x) as i64` because the unary level is read first and the `as` then
    /// applies to whatever it produced.
    ///
    /// The loop is what makes casts CHAINABLE — `3.7 as i64 as i32` — and it
    /// is a loop rather than recursion so the chain is left-associative, which
    /// is the only reading that means anything: each cast takes the previous
    /// one's result.
    fn parse_cast(&mut self) -> Result<Expr> {
        let mut expr = self.parse_unary()?;

        while matches!(self.peek(), Ok(Token::As)) {
            let start_span = Self::expr_span(&expr);
            self.advance()?; // consume 'as'
            let ty = self.parse_type()?;
            let end_span = self.current_span().unwrap_or(start_span);
            expr = Expr::Cast {
                expr: Box::new(expr),
                ty,
                span: Span::new(
                    start_span.start,
                    end_span.end,
                    start_span.line,
                    start_span.column,
                ),
            };
        }

        Ok(expr)
    }

    /// Parse addition and subtraction
    fn parse_addition(&mut self) -> Result<Expr> {
        let mut left = self.parse_multiplication()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::Plus | Token::Minus => {
                    let left_span = Self::expr_span(&left);
                    let op = match self.advance()?.0 {
                        Token::Plus => BinOp::Add,
                        Token::Minus => BinOp::Sub,
                        _ => unreachable!(),
                    };
                    let right = self.parse_multiplication()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse multiplication and division
    fn parse_multiplication(&mut self) -> Result<Expr> {
        let mut left = self.parse_cast()?;

        while let Ok(token) = self.peek() {
            match token {
                Token::Star | Token::Slash | Token::Percent => {
                    let left_span = Self::expr_span(&left);
                    let op = match self.advance()?.0 {
                        Token::Star => BinOp::Mul,
                        Token::Slash => BinOp::Div,
                        Token::Percent => BinOp::Mod,
                        _ => unreachable!(),
                    };
                    // `parse_unary`, NOT `parse_postfix` (N5-16). This one call
                    // was the whole defect: the LEFT operand descended through
                    // the unary level and the right one skipped it, so a `-`
                    // after `*` had nothing to read it and `a * -b` did not
                    // parse. grammar.ebnf states the correction as normative:
                    // `multiplication = unary { ( '*' | '/' | '%' ) unary } ;`
                    //
                    // Every other level of this ladder was already symmetric —
                    // equality descends to comparison on both sides,
                    // comparison to addition, addition to multiplication —
                    // which is why this was the only expression that failed.
                    //
                    // Both sides now go through `parse_cast`, which is
                    // `parse_unary` plus the `as` suffix (N5-15); the unary
                    // level is still reached on both sides, which is the whole
                    // point of this row.
                    let right = self.parse_cast()?;
                    let right_span = Self::expr_span(&right);
                    let span = Span::new(
                        left_span.start,
                        right_span.end,
                        left_span.line,
                        left_span.column,
                    );
                    left = Expr::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse a type
    fn parse_type(&mut self) -> Result<Type> {
        match self.advance()? {
            (Token::SelfType, _) => {
                // Self type in trait or impl contexts
                Ok(Type::Custom("Self".to_string()))
            }
            (Token::Ampersand, _) => {
                // Parse reference type: &T or &mut T or &'a T or &'a mut T
                let mut lifetime = None;
                let mut mutable = false;

                // Check for lifetime annotation
                if matches!(self.peek()?, Token::SingleQuote) {
                    self.advance()?; // consume '
                    match self.advance()? {
                        (Token::Identifier(lt), _) => {
                            lifetime = Some(lt);
                        }
                        _ => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "lifetime name".to_string(),
                                found: self.peek()?.to_string(),
                                span: self.current_span(),
                            });
                        }
                    }
                }

                // Check for mut keyword
                if matches!(self.peek()?, Token::Mut) {
                    self.advance()?;
                    mutable = true;
                }

                // Parse the inner type
                let inner = self.parse_type()?;

                Ok(Type::Reference {
                    lifetime,
                    mutable,
                    inner: Box::new(inner),
                })
            }
            (Token::Identifier(name), _) => {
                // First check if it's a type parameter in scope
                if self.type_params_in_scope.contains(&name) {
                    return Ok(Type::TypeParam(name));
                }

                let base_type = match name.as_str() {
                    "i32" => Type::I32,
                    "i64" | "int" => Type::I64, // "int" is an alias for i64
                    "u32" => Type::U32,
                    "u64" => Type::U64,
                    "f32" => Type::F32,
                    "f64" => Type::F64,
                    "bool" => Type::Bool,
                    "char" => Type::Char,
                    "String" => Type::String,
                    _ => Type::Custom(name.clone()),
                };

                // Check for generic arguments
                if self.check(&Token::Lt) {
                    // Only parse generics for custom types
                    match base_type {
                        Type::Custom(type_name) => {
                            self.advance()?; // consume '<'
                            let mut args = Vec::new();

                            loop {
                                // Try to parse as const value first (for literals)
                                if let Token::Integer(n) = self.peek()? {
                                    let n_val = *n;
                                    self.advance()?; // consume the integer
                                    args.push(GenericArg::Const(ConstValue::Integer(n_val)));
                                } else {
                                    // Otherwise parse as type
                                    let ty = self.parse_type()?;
                                    // If it's an identifier, it could be a const param
                                    match &ty {
                                        Type::Custom(name)
                                            if name
                                                .chars()
                                                .all(|c| c.is_uppercase() || c == '_') =>
                                        {
                                            // Assume uppercase identifiers are const params
                                            args.push(GenericArg::Const(ConstValue::ConstParam(
                                                name.clone(),
                                            )));
                                        }
                                        _ => {
                                            args.push(GenericArg::Type(ty));
                                        }
                                    }
                                }

                                if !self.check(&Token::Comma) {
                                    break;
                                }
                                self.advance()?; // consume ','
                            }

                            self.consume(Token::Gt, "Expected '>' after generic arguments")?;
                            Ok(Type::Generic {
                                name: type_name,
                                args,
                            })
                        }
                        _ => {
                            // Primitive types cannot have generic arguments
                            Err(CompileError::SyntaxError {
                                message: format!("Type '{}' cannot have generic arguments", name),
                                span: self.current_span(),
                            })
                        }
                    }
                } else {
                    Ok(base_type)
                }
            }
            (Token::LeftParen, _) => {
                // Parse tuple type: (), (T,), (T1, T2), etc.
                let mut types = Vec::new();

                // Check for unit type
                if self.check(&Token::RightParen) {
                    self.advance()?; // consume ')'
                    return Ok(Type::Unit);
                }

                // Parse first element
                types.push(self.parse_type()?);

                // Parse remaining elements
                while self.check(&Token::Comma) {
                    self.advance()?; // consume ','

                    // Allow trailing comma
                    if self.check(&Token::RightParen) {
                        break;
                    }

                    types.push(self.parse_type()?);
                }

                self.consume(Token::RightParen, "Expected ')' after tuple type")?;

                // All tuples, including single element ones
                Ok(Type::Tuple(types))
            }
            (Token::LeftBracket, _) => {
                // Parse array type: [T; N]
                let elem_type = self.parse_type()?;
                self.consume(Token::Semicolon, "Expected ';' in array type")?;

                // Parse the size (can be a literal or const parameter)
                let size = match self.peek()? {
                    Token::Integer(n) => {
                        let n_val = *n;
                        self.advance()?; // consume the integer
                        if n_val < 0 {
                            return Err(CompileError::Generic(
                                "Array size must be non-negative".to_string(),
                            ));
                        }
                        ArraySize::Literal(n_val as usize)
                    }
                    Token::Identifier(name) => {
                        let name_val = name.clone();
                        self.advance()?; // consume the identifier
                                         // Check if it's a const parameter in scope
                                         // For now, we'll assume any identifier could be a const param
                        ArraySize::ConstParam(name_val)
                    }
                    token => {
                        return Err(CompileError::UnexpectedToken {
                            expected: "array size (integer or const parameter)".to_string(),
                            found: token.to_string(),
                            span: self.current_span(),
                        });
                    }
                };

                self.consume(Token::RightBracket, "Expected ']' after array type")?;
                Ok(Type::Array(Box::new(elem_type), size))
            }
            (token, _) => Err(CompileError::UnexpectedToken {
                expected: "type".to_string(),
                found: token.to_string(),
                span: self.current_span(),
            }),
        }
    }

    /// Parse a primary expression
    fn parse_primary(&mut self) -> Result<Expr> {
        // `if` and `{` in expression position (N5-03, N5-05). Peeked rather
        // than consumed, because both sub-parsers start by consuming their own
        // opening token.
        //
        // Statement position is unaffected: `parse_statement` routes `Token::If`
        // to `parse_if`, and `parse_block_with_implicit_return` routes it to
        // `parse_if_with_tail`, both of which run before any expression parse.
        if self.check(&Token::If) {
            return self.parse_if_expression();
        }
        if self.check(&Token::LeftBrace) {
            let start_span = self.advance()?.1; // consume '{'
            return self.parse_block_expression(start_span);
        }
        if self.check(&Token::Match) {
            return self.parse_match_expression();
        }
        if self.check(&Token::Loop) {
            return self.parse_loop_expression();
        }

        match self.advance()? {
            (Token::String(s), _) => Ok(Expr::String(s)),
            (Token::Integer(n), _) => Ok(Expr::Integer(n)),
            (Token::Float(x), _) => Ok(Expr::Float(x)),
            (Token::Char(c), _) => Ok(Expr::Char(c)),
            (Token::True, _) => Ok(Expr::Bool(true)),
            (Token::False, _) => Ok(Expr::Bool(false)),
            (Token::SelfParam, _span) => {
                // Handle 'self' as an identifier in expression context
                Ok(Expr::Ident("self".to_string()))
            }
            (Token::Identifier(name), span) => {
                // Check if this is a struct literal
                // We need to be careful here - only parse as struct literal if we see
                // identifier followed by field pattern (identifier + colon)
                if self.check(&Token::LeftBrace) && self.check_struct_literal_pattern() {
                    let start_span = span;
                    self.advance()?; // consume '{'

                    let mut fields = Vec::new();

                    while !self.check(&Token::RightBrace) && !self.is_at_end() {
                        // Parse field name
                        let field_name = match self.advance()? {
                            (Token::Identifier(fname), _) => fname,
                            (token, _) => {
                                return Err(CompileError::UnexpectedToken {
                                    expected: "field name".to_string(),
                                    found: token.to_string(),
                                    span: self.current_span(),
                                });
                            }
                        };

                        self.consume(Token::Colon, "Expected ':' after field name")?;
                        let field_expr = self.parse_expression()?;

                        fields.push((field_name, field_expr));

                        if !self.check(&Token::RightBrace) {
                            self.consume(Token::Comma, "Expected ',' after field")?;
                        }
                    }

                    let end_span =
                        self.consume(Token::RightBrace, "Expected '}' after struct fields")?;

                    Ok(Expr::StructLiteral {
                        name,
                        fields,
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    })
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            (Token::LeftParen, start_span) => {
                // Grouping, or a TUPLE (N4-12). The comma decides, and nothing
                // else does: `(e)` has been grouping in every program this
                // corpus contains, so it stays grouping.
                let first = self.parse_expression()?;
                if !self.check(&Token::Comma) {
                    self.consume(Token::RightParen, "Expected ')' after expression")?;
                    return Ok(first);
                }
                let mut elements = vec![first];
                while self.check(&Token::Comma) {
                    self.advance()?; // consume ','
                    if self.check(&Token::RightParen) {
                        // Trailing comma. `(a, b,)` is a 2-tuple; `(a,)` would be
                        // a ONE-tuple, which this language does not have — see
                        // the refusal below.
                        break;
                    }
                    elements.push(self.parse_expression()?);
                }
                let end_span = self.consume(Token::RightParen, "Expected ')' after tuple")?;
                if elements.len() < 2 {
                    return Err(CompileError::UnexpectedToken {
                        expected: "a tuple to have at least two elements; `(e)` is grouping and \
                                   a one-element tuple `(e,)` is not a form this language has"
                            .to_string(),
                        found: "a one-element tuple".to_string(),
                        span: self.current_span(),
                    });
                }
                Ok(Expr::Tuple {
                    elements,
                    span: Span::new(
                        start_span.start,
                        end_span.end,
                        start_span.line,
                        start_span.column,
                    ),
                })
            }
            (Token::LeftBracket, span) => {
                // Parse array literal: [1, 2, 3] or array repeat: [0; 10]
                if self.check(&Token::RightBracket) {
                    // Empty array
                    let end_span = self.advance()?.1;
                    return Ok(Expr::ArrayLiteral {
                        elements: Vec::new(),
                        span: Span::new(span.start, end_span.end, span.line, span.column),
                    });
                }

                // Parse first element
                let first_elem = self.parse_expression()?;

                // Check if this is array repeat syntax
                if self.check(&Token::Semicolon) {
                    self.advance()?; // consume ';'
                    let count = self.parse_expression()?;
                    let end_span =
                        self.consume(Token::RightBracket, "Expected ']' after array repeat count")?;

                    Ok(Expr::ArrayRepeat {
                        value: Box::new(first_elem),
                        count: Box::new(count),
                        span: Span::new(span.start, end_span.end, span.line, span.column),
                    })
                } else {
                    // Regular array literal
                    let mut elements = vec![first_elem];

                    while self.check(&Token::Comma) {
                        self.advance()?; // consume ','
                        if self.check(&Token::RightBracket) {
                            // Trailing comma
                            break;
                        }
                        elements.push(self.parse_expression()?);
                    }

                    let end_span =
                        self.consume(Token::RightBracket, "Expected ']' after array elements")?;

                    Ok(Expr::ArrayLiteral {
                        elements,
                        span: Span::new(span.start, end_span.end, span.line, span.column),
                    })
                }
            }
            (token, _) => Err(CompileError::UnexpectedToken {
                expected: "expression".to_string(),
                found: token.to_string(),
                span: self.current_span(),
            }),
        }
    }

    /// Parse unary expressions (-, !, &, &mut, *)
    fn parse_unary(&mut self) -> Result<Expr> {
        match self.peek() {
            Ok(Token::Minus) => {
                let (_, start_span) = self.advance()?; // consume '-'
                let operand = self.parse_unary()?; // Right associative
                let end_span = operand.span();
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                    span: Span::new(
                        start_span.start,
                        end_span.end,
                        start_span.line,
                        start_span.column,
                    ),
                })
            }
            Ok(Token::Tilde) => {
                let (_, start_span) = self.advance()?; // consume '~'
                let operand = self.parse_unary()?; // Right associative
                let end_span = operand.span();
                Ok(Expr::Unary {
                    op: UnaryOp::BitNot,
                    operand: Box::new(operand),
                    span: Span::new(
                        start_span.start,
                        end_span.end,
                        start_span.line,
                        start_span.column,
                    ),
                })
            }
            Ok(Token::Not) => {
                let (_, start_span) = self.advance()?; // consume '!'
                let operand = self.parse_unary()?; // Right associative
                let end_span = operand.span();
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                    span: Span::new(
                        start_span.start,
                        end_span.end,
                        start_span.line,
                        start_span.column,
                    ),
                })
            }
            Ok(Token::Ampersand) => {
                let (_, start_span) = self.advance()?; // consume '&'
                let mutable = if matches!(self.peek()?, Token::Mut) {
                    self.advance()?; // consume 'mut'
                    true
                } else {
                    false
                };
                let expr = self.parse_unary()?;
                let end_span = expr.span();
                Ok(Expr::Reference {
                    mutable,
                    expr: Box::new(expr),
                    span: Span::new(
                        start_span.start,
                        end_span.end,
                        start_span.line,
                        start_span.column,
                    ),
                })
            }
            Ok(Token::Star) => {
                let (_, start_span) = self.advance()?; // consume '*'
                let expr = self.parse_unary()?;
                let end_span = expr.span();
                Ok(Expr::Deref {
                    expr: Box::new(expr),
                    span: Span::new(
                        start_span.start,
                        end_span.end,
                        start_span.line,
                        start_span.column,
                    ),
                })
            }
            _ => self.parse_postfix(),
        }
    }

    /// Parse postfix expressions (array indexing, function calls)
    fn parse_postfix(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek() {
                Ok(Token::LeftBracket) => {
                    let start_span = self.advance()?.1; // consume '['
                    let index = self.parse_expression()?;
                    let end_span =
                        self.consume(Token::RightBracket, "Expected ']' after array index")?;

                    expr = Expr::Index {
                        array: Box::new(expr),
                        index: Box::new(index),
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    };
                }
                Ok(Token::LeftParen) => {
                    let start_span = self.advance()?.1; // consume '('

                    let mut args = Vec::new();

                    if !self.check(&Token::RightParen) {
                        loop {
                            args.push(self.parse_expression()?);

                            if !self.check(&Token::Comma) {
                                break;
                            }
                            self.advance()?; // consume ','
                        }
                    }

                    let end_span = self.consume(Token::RightParen, "Expected ')'")?;

                    expr = Expr::Call {
                        func: Box::new(expr),
                        args,
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    };
                }
                Ok(Token::Dot) if self.check_at(1, &Token::Await) => {
                    let start_span = Self::expr_span(&expr);
                    self.advance()?; // consume '.'
                    let end_span = self.consume(Token::Await, "Expected 'await'")?;

                    expr = Expr::Await {
                        expr: Box::new(expr),
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    };
                }
                Ok(Token::Dot) => {
                    let start_span = Self::expr_span(&expr);

                    self.advance()?; // consume '.'

                    match self.advance()? {
                        (Token::Identifier(name), span) => {
                            let end_span = span;
                            expr = Expr::FieldAccess {
                                object: Box::new(expr),
                                field: name,
                                span: Span::new(
                                    start_span.start,
                                    end_span.end,
                                    start_span.line,
                                    start_span.column,
                                ),
                            };
                            continue;
                        }
                        // N4-12. `p.0`. The lexer already tells this apart from a
                        // float: the float rule needs digits on BOTH sides of the
                        // dot, so `.0` after an expression is `Dot` then
                        // `Integer(0)`.
                        (Token::Integer(index), span) => {
                            if index < 0 {
                                return Err(CompileError::UnexpectedToken {
                                    expected: "a tuple index, which is a non-negative integer"
                                        .to_string(),
                                    found: format!("`.{}`", index),
                                    span: self.current_span(),
                                });
                            }
                            // `p.01` IS NOT `p.1`. The lexer parses the digits
                            // and hands over an `i64`, so the spelling is gone by
                            // the time this sees it — but the SPAN is not: an
                            // index written with leading zeros covers more source
                            // than its value has digits. Without this check
                            // `p.01` silently compiled as `.1`, which is the
                            // shape of a typo that changes which element a
                            // program reads.
                            let width = span.end.saturating_sub(span.start);
                            if width > index.to_string().len() {
                                return Err(CompileError::UnexpectedToken {
                                    expected: "a tuple index written without leading zeros"
                                        .to_string(),
                                    found: format!(
                                        "an index whose spelling is {} characters wide for the \
                                         value {} — `.{}{}` is not a way to write `.{}`",
                                        width,
                                        index,
                                        "0".repeat(width - index.to_string().len()),
                                        index,
                                        index
                                    ),
                                    span: self.current_span(),
                                });
                            }
                            let end_span = span;
                            expr = Expr::TupleIndex {
                                expr: Box::new(expr),
                                index: index as usize,
                                span: Span::new(
                                    start_span.start,
                                    end_span.end,
                                    start_span.line,
                                    start_span.column,
                                ),
                            };
                            continue;
                        }
                        // `p.0.1` — REFUSED, NOT GUESSED. `[0-9]+\.[0-9]+` is the
                        // float rule, so `.0.1` lexes as ONE `Float(0.1)` token
                        // and the two indices are gone before this point. The f64
                        // cannot be split back: `p.0.10` and `p.0.1` both hold
                        // 0.1, so recovering "10" from it would be a guess with a
                        // wrong answer available.
                        (Token::Float(_), _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "a field name or a tuple index; a CHAINED tuple index \
                                           has to be parenthesised, as `(p.0).1`, because `.0.1` \
                                           lexes as one float literal and the two indices cannot \
                                           be recovered from it"
                                    .to_string(),
                                found: "a float literal".to_string(),
                                span: self.current_span(),
                            });
                        }
                        (token, _) => {
                            return Err(CompileError::UnexpectedToken {
                                expected: "field name".to_string(),
                                found: token.to_string(),
                                span: self.current_span(),
                            });
                        }
                    };
                }
                Ok(Token::DoubleColon) => {
                    // Handle enum constructor: EnumName::Variant
                    if let Expr::Ident(enum_name) = expr {
                        let start_span = self.tokens[self.current - 2].1; // Get span from before :: token
                        self.advance()?; // consume '::'

                        let variant = match self.advance()? {
                            (Token::Identifier(name), _) => name,
                            (token, _) => {
                                return Err(CompileError::UnexpectedToken {
                                    expected: "variant name".to_string(),
                                    found: token.to_string(),
                                    span: self.current_span(),
                                });
                            }
                        };

                        // Check if this is a function call (has parentheses) or constructor
                        if self.check(&Token::LeftParen) {
                            // Parse tuple-style enum constructor
                            self.advance()?; // consume '('
                            let mut args = Vec::new();

                            if !self.check(&Token::RightParen) {
                                loop {
                                    args.push(self.parse_expression()?);
                                    if !self.check(&Token::Comma) {
                                        break;
                                    }
                                    self.advance()?; // consume ','
                                }
                            }

                            let end_span = self.consume(Token::RightParen, "Expected ')'")?;

                            // Create an enum constructor expression
                            expr = Expr::EnumConstructor {
                                enum_name,
                                variant,
                                data: Some(EnumConstructorData::Tuple(args)),
                                span: Span::new(
                                    start_span.start,
                                    end_span.end,
                                    start_span.line,
                                    start_span.column,
                                ),
                            };
                            continue;
                        }

                        // Check for struct-style constructor.
                        //
                        // GUARDED, the same way the plain struct literal in
                        // `parse_primary` is guarded, and for the same reason:
                        // a `{` after a path is only a constructor when what
                        // follows looks like `field:`. Unguarded, N5-04's
                        // `match Shape::Square { Shape::Circle => … }` read the
                        // MATCH BODY as a field list and died on
                        // "Expected ':' after field name, found '::'" — the
                        // arms of the match, mistaken for the fields of a
                        // variant that has none.
                        //
                        // This cannot reject a program that parsed before: the
                        // guard only fails where the field list would have
                        // failed anyway.
                        let data = if self.check(&Token::LeftBrace)
                            && self.check_struct_literal_pattern()
                        {
                            // Struct constructor
                            self.advance()?; // consume '{'
                            let mut fields = Vec::new();

                            while !self.check(&Token::RightBrace) && !self.is_at_end() {
                                let field_name = match self.advance()? {
                                    (Token::Identifier(fname), _) => fname,
                                    (token, _) => {
                                        return Err(CompileError::UnexpectedToken {
                                            expected: "field name".to_string(),
                                            found: token.to_string(),
                                            span: self.current_span(),
                                        });
                                    }
                                };

                                self.consume(Token::Colon, "Expected ':' after field name")?;
                                let field_expr = self.parse_expression()?;

                                fields.push((field_name, field_expr));

                                if !self.check(&Token::RightBrace) {
                                    self.consume(Token::Comma, "Expected ',' after field")?;
                                }
                            }

                            let _end_span = self.consume(Token::RightBrace, "Expected '}'")?;
                            Some(EnumConstructorData::Struct(fields))
                        } else {
                            None
                        };

                        let end_span = self.tokens[self.current - 1].1; // Get last consumed token span
                        expr = Expr::EnumConstructor {
                            enum_name,
                            variant,
                            data,
                            span: Span::new(
                                start_span.start,
                                end_span.end,
                                start_span.line,
                                start_span.column,
                            ),
                        };
                    } else {
                        return Err(CompileError::SyntaxError {
                            message: "Double colon can only be used after an identifier"
                                .to_string(),
                            span: self.current_span(),
                        });
                    }
                }
                Ok(Token::Question) => {
                    let start_span = Self::expr_span(&expr);
                    let (_, end_span) = self.advance()?; // consume '?'

                    expr = Expr::Question {
                        expr: Box::new(expr),
                        span: Span::new(
                            start_span.start,
                            end_span.end,
                            start_span.line,
                            start_span.column,
                        ),
                    };
                }
                Ok(Token::Not) => {
                    // Macro invocation: name!(args)
                    if let Expr::Ident(name) = expr {
                        let start_span = self.tokens[self.current - 1].1; // Get span from identifier
                        self.advance()?; // consume '!'

                        // Parse macro arguments (simplified for now - just collect tokens in parens)
                        self.consume(Token::LeftParen, "Expected '(' after macro name!")?;

                        let mut args = Vec::new();
                        let mut paren_depth = 1;

                        while paren_depth > 0 && !self.is_at_end() {
                            let (token, _) = self.advance()?;

                            match &token {
                                Token::LeftParen => {
                                    paren_depth += 1;
                                    args.push(self.token_to_ast_token(token)?);
                                }
                                Token::RightParen => {
                                    paren_depth -= 1;
                                    if paren_depth > 0 {
                                        args.push(self.token_to_ast_token(token)?);
                                    }
                                }
                                _ => {
                                    args.push(self.token_to_ast_token(token)?);
                                }
                            }
                        }

                        let end_span = self.current_span().unwrap_or(start_span);

                        expr = Expr::MacroInvocation {
                            name,
                            args,
                            span: Span::new(
                                start_span.start,
                                end_span.end,
                                start_span.line,
                                start_span.column,
                            ),
                        };
                    } else {
                        return Err(CompileError::SyntaxError {
                            message: "Macro invocation '!' can only be used after an identifier"
                                .to_string(),
                            span: self.current_span(),
                        });
                    }
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    // Helper methods

    /// Check if the pattern ahead looks like a struct literal
    /// We look for: { identifier : ... or { }
    fn check_struct_literal_pattern(&self) -> bool {
        if self.current + 1 >= self.tokens.len() {
            return false;
        }

        // Check if next token after { is an identifier or }
        match &self.tokens[self.current + 1].0 {
            Token::Identifier(_) => {
                // Check if token after identifier is :
                if self.current + 2 < self.tokens.len() {
                    matches!(&self.tokens[self.current + 2].0, Token::Colon)
                } else {
                    false
                }
            }
            Token::RightBrace => true, // Empty struct literal
            _ => false,
        }
    }

    /// Check if we're at the end of tokens
    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len()
    }

    /// Peek at the current token without consuming it
    fn peek(&self) -> Result<&Token> {
        if self.is_at_end() {
            Err(CompileError::SyntaxError {
                message: "Unexpected end of file".to_string(),
                span: self.current_span(),
            })
        } else {
            Ok(&self.tokens[self.current].0)
        }
    }

    /// Check if the current token matches the given token
    fn check(&self, token: &Token) -> bool {
        if self.is_at_end() {
            false
        } else {
            std::mem::discriminant(&self.tokens[self.current].0) == std::mem::discriminant(token)
        }
    }

    /// Check if a token at offset matches the given token
    fn check_at(&self, offset: usize, token: &Token) -> bool {
        let index = self.current + offset;
        if index >= self.tokens.len() {
            false
        } else {
            std::mem::discriminant(&self.tokens[index].0) == std::mem::discriminant(token)
        }
    }

    /// Advance to the next token
    fn advance(&mut self) -> Result<(Token, Span)> {
        if self.is_at_end() {
            Err(CompileError::SyntaxError {
                message: "Unexpected end of file".to_string(),
                span: self.current_span(),
            })
        } else {
            let token = self.tokens[self.current].clone();
            self.current += 1;
            self.update_cache(); // Update cache after advancing
            Ok(token)
        }
    }

    /// Consume a specific token or error
    fn consume(&mut self, expected: Token, message: &str) -> Result<Span> {
        let (token, span) = self.advance()?;

        if std::mem::discriminant(&token) == std::mem::discriminant(&expected) {
            Ok(span)
        } else {
            Err(CompileError::UnexpectedToken {
                expected: format!("{} ({})", expected, message),
                found: token.to_string(),
                span: self.current_span(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn test_parse_hello_world() {
        let source = r#"
        fn main() {
            print("Hello, World!");
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.params.len(), 0);
            assert_eq!(func.return_type, None);
            assert_eq!(func.body.len(), 1);

            if let Stmt::Expr(Expr::Call { func: _, args, .. }) = &func.body[0] {
                assert_eq!(args.len(), 1);
                if let Expr::String(s) = &args[0] {
                    assert_eq!(s, "Hello, World!");
                }
            }
        }
    }

    #[test]
    fn test_parse_function_with_return_type() {
        let source = r#"
        fn main() -> i32 {
            return 0;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.params.len(), 0);
            assert_eq!(func.return_type, Some(Type::I32));
            assert_eq!(func.body.len(), 1);

            if let Stmt::Return(Some(Expr::Integer(n))) = &func.body[0] {
                assert_eq!(*n, 0);
            } else {
                panic!("Expected return statement with integer");
            }
        }
    }

    #[test]
    fn test_parse_for_loop() {
        let source = r#"
        fn main() {
            for i in arr {
                print_int(i);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 1);

            if let Stmt::For {
                var, iter, body, ..
            } = &func.body[0]
            {
                assert_eq!(var, "i");
                if let Expr::Ident(name) = iter {
                    assert_eq!(name, "arr");
                }
                assert_eq!(body.len(), 1);
            } else {
                panic!("Expected for loop");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_break_continue() {
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

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.body.len(), 1);

            if let Stmt::While { body, .. } = &func.body[0] {
                assert_eq!(body.len(), 2);

                if let Stmt::If { then_branch, .. } = &body[0] {
                    assert_eq!(then_branch.len(), 1);
                    assert!(matches!(&then_branch[0], Stmt::Break { .. }));
                }

                if let Stmt::If { then_branch, .. } = &body[1] {
                    assert_eq!(then_branch.len(), 1);
                    assert!(matches!(&then_branch[0], Stmt::Continue { .. }));
                }
            } else {
                panic!("Expected while loop");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_for_loop_with_break_continue() {
        let source = r#"
        fn main() {
            for i in arr {
                if i == 0 {
                    continue;
                }
                if i > 10 {
                    break;
                }
                print_int(i);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.body.len(), 1);

            if let Stmt::For { var, body, .. } = &func.body[0] {
                assert_eq!(var, "i");
                assert_eq!(body.len(), 3);

                // First statement: if with continue
                if let Stmt::If { then_branch, .. } = &body[0] {
                    assert_eq!(then_branch.len(), 1);
                    assert!(matches!(&then_branch[0], Stmt::Continue { .. }));
                }

                // Second statement: if with break
                if let Stmt::If { then_branch, .. } = &body[1] {
                    assert_eq!(then_branch.len(), 1);
                    assert!(matches!(&then_branch[0], Stmt::Break { .. }));
                }

                // Third statement: print_int call
                assert!(matches!(&body[2], Stmt::Expr(_)));
            } else {
                panic!("Expected for loop");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_struct() {
        let source = r#"
        struct Point {
            x: i64,
            y: i64,
        }
        
        fn main() {
            let p = Point { x: 10, y: 20 };
            print_int(p.x);
            p.y = 30;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 2);

        // Check struct definition
        if let Item::Struct(struct_def) = &ast.items[0] {
            assert_eq!(struct_def.name, "Point");
            assert_eq!(struct_def.fields.len(), 2);
            assert_eq!(struct_def.fields[0].0, "x");
            assert_eq!(struct_def.fields[0].1, Type::I64);
            assert_eq!(struct_def.fields[1].0, "y");
            assert_eq!(struct_def.fields[1].1, Type::I64);
        } else {
            panic!("Expected struct definition");
        }

        // Check function with struct usage
        if let Item::Function(func) = &ast.items[1] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 3);

            // First statement: struct literal
            if let Stmt::Let { name, value, .. } = &func.body[0] {
                assert_eq!(name, "p");
                if let Expr::StructLiteral { name, fields, .. } = value {
                    assert_eq!(name, "Point");
                    assert_eq!(fields.len(), 2);
                    assert_eq!(fields[0].0, "x");
                    assert_eq!(fields[1].0, "y");
                } else {
                    panic!("Expected struct literal");
                }
            }

            // Second statement: field access
            if let Stmt::Expr(Expr::Call { args, .. }) = &func.body[1] {
                assert_eq!(args.len(), 1);
                if let Expr::FieldAccess { field, .. } = &args[0] {
                    assert_eq!(field, "x");
                } else {
                    panic!("Expected field access");
                }
            }

            // Third statement: field assignment
            if let Stmt::Assign { target, .. } = &func.body[2] {
                if let AssignTarget::FieldAccess { field, .. } = target {
                    assert_eq!(field, "y");
                } else {
                    panic!("Expected field assignment");
                }
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_range_syntax() {
        let source = r#"
        fn main() {
            for i in 0..10 {
                print_int(i);
            }
            
            let start = 5;
            let end = 15;
            for j in start..end {
                print_int(j);
            }
            
            for k in 0..n+1 {
                print_int(k);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 5); // 3 for loops + 2 let statements

            // First for loop: 0..10
            if let Stmt::For { var, iter, .. } = &func.body[0] {
                assert_eq!(var, "i");
                if let Expr::Range { start, end, .. } = iter {
                    assert!(matches!(start.as_ref(), Expr::Integer(0)));
                    assert!(matches!(end.as_ref(), Expr::Integer(10)));
                } else {
                    panic!("Expected range expression");
                }
            } else {
                panic!("Expected for loop");
            }

            // Check let statements
            assert!(matches!(&func.body[1], Stmt::Let { name, .. } if name == "start"));
            assert!(matches!(&func.body[2], Stmt::Let { name, .. } if name == "end"));

            // Second for loop: start..end (with variables)
            if let Stmt::For { var, iter, .. } = &func.body[3] {
                assert_eq!(var, "j");
                if let Expr::Range { start, end, .. } = iter {
                    assert!(matches!(start.as_ref(), Expr::Ident(s) if s == "start"));
                    assert!(matches!(end.as_ref(), Expr::Ident(e) if e == "end"));
                } else {
                    panic!("Expected range expression");
                }
            } else {
                panic!("Expected for loop");
            }

            // Third for loop: 0..n+1
            if let Stmt::For { var, iter, .. } = &func.body[4] {
                assert_eq!(var, "k");
                if let Expr::Range { start, end, .. } = iter {
                    assert!(matches!(start.as_ref(), Expr::Integer(0)));
                    // The end should be a binary expression (n+1)
                    assert!(matches!(end.as_ref(), Expr::Binary { .. }));
                } else {
                    panic!("Expected range expression");
                }
            } else {
                panic!("Expected for loop");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_enum() {
        let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }
        
        enum Option {
            Some(i64),
            None,
        }
        
        enum Shape {
            Circle { radius: i64 },
            Rectangle { width: i64, height: i64 },
            Point,
        }
        
        fn main() {
            let c = Color::Red;
            let opt = Option::Some(42);
            let shape = Shape::Circle { radius: 10 };
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 4);

        // Check first enum (simple)
        if let Item::Enum(enum_def) = &ast.items[0] {
            assert_eq!(enum_def.name, "Color");
            assert_eq!(enum_def.variants.len(), 3);
            assert_eq!(enum_def.variants[0].name, "Red");
            assert!(matches!(enum_def.variants[0].data, EnumVariantData::Unit));
            assert_eq!(enum_def.variants[1].name, "Green");
            assert!(matches!(enum_def.variants[1].data, EnumVariantData::Unit));
            assert_eq!(enum_def.variants[2].name, "Blue");
            assert!(matches!(enum_def.variants[2].data, EnumVariantData::Unit));
        } else {
            panic!("Expected enum definition");
        }

        // Check second enum (with tuple variant)
        if let Item::Enum(enum_def) = &ast.items[1] {
            assert_eq!(enum_def.name, "Option");
            assert_eq!(enum_def.variants.len(), 2);
            assert_eq!(enum_def.variants[0].name, "Some");
            if let EnumVariantData::Tuple(types) = &enum_def.variants[0].data {
                assert_eq!(types.len(), 1);
                assert_eq!(types[0], Type::I64);
            } else {
                panic!("Expected tuple variant");
            }
            assert_eq!(enum_def.variants[1].name, "None");
            assert!(matches!(enum_def.variants[1].data, EnumVariantData::Unit));
        } else {
            panic!("Expected enum definition");
        }

        // Check third enum (with struct variant)
        if let Item::Enum(enum_def) = &ast.items[2] {
            assert_eq!(enum_def.name, "Shape");
            assert_eq!(enum_def.variants.len(), 3);

            assert_eq!(enum_def.variants[0].name, "Circle");
            if let EnumVariantData::Struct(fields) = &enum_def.variants[0].data {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].0, "radius");
                assert_eq!(fields[0].1, Type::I64);
            } else {
                panic!("Expected struct variant");
            }

            assert_eq!(enum_def.variants[1].name, "Rectangle");
            if let EnumVariantData::Struct(fields) = &enum_def.variants[1].data {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0, "width");
                assert_eq!(fields[0].1, Type::I64);
                assert_eq!(fields[1].0, "height");
                assert_eq!(fields[1].1, Type::I64);
            } else {
                panic!("Expected struct variant");
            }

            assert_eq!(enum_def.variants[2].name, "Point");
            assert!(matches!(enum_def.variants[2].data, EnumVariantData::Unit));
        } else {
            panic!("Expected enum definition");
        }

        // Check function with enum usage
        if let Item::Function(func) = &ast.items[3] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 3);

            // First statement: unit enum constructor
            if let Stmt::Let { name, value, .. } = &func.body[0] {
                assert_eq!(name, "c");
                if let Expr::EnumConstructor {
                    enum_name,
                    variant,
                    data,
                    ..
                } = value
                {
                    assert_eq!(enum_name, "Color");
                    assert_eq!(variant, "Red");
                    assert!(data.is_none());
                } else {
                    panic!("Expected enum constructor");
                }
            }

            // Second statement: tuple enum constructor
            if let Stmt::Let { name, value, .. } = &func.body[1] {
                assert_eq!(name, "opt");
                if let Expr::EnumConstructor {
                    enum_name,
                    variant,
                    data,
                    ..
                } = value
                {
                    assert_eq!(enum_name, "Option");
                    assert_eq!(variant, "Some");
                    if let Some(EnumConstructorData::Tuple(args)) = data {
                        assert_eq!(args.len(), 1);
                        if let Expr::Integer(n) = &args[0] {
                            assert_eq!(*n, 42);
                        } else {
                            panic!("Expected integer argument");
                        }
                    } else {
                        panic!("Expected tuple constructor data");
                    }
                } else {
                    panic!("Expected enum constructor");
                }
            }

            // Third statement: struct enum constructor
            if let Stmt::Let { name, value, .. } = &func.body[2] {
                assert_eq!(name, "shape");
                if let Expr::EnumConstructor {
                    enum_name,
                    variant,
                    data,
                    ..
                } = value
                {
                    assert_eq!(enum_name, "Shape");
                    assert_eq!(variant, "Circle");
                    if let Some(EnumConstructorData::Struct(fields)) = data {
                        assert_eq!(fields.len(), 1);
                        assert_eq!(fields[0].0, "radius");
                        if let Expr::Integer(n) = &fields[0].1 {
                            assert_eq!(*n, 10);
                        } else {
                            panic!("Expected integer field value");
                        }
                    } else {
                        panic!("Expected struct constructor data");
                    }
                } else {
                    panic!("Expected enum constructor");
                }
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_match_wildcard() {
        let source = r#"
        fn main() {
            let x = 42;
            match x {
                _ => print("wildcard"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.body.len(), 2);
            if let Stmt::Match { arms, .. } = &func.body[1] {
                assert_eq!(arms.len(), 1);
                match &arms[0].pattern {
                    Pattern::Wildcard => {}
                    _ => panic!("Expected wildcard pattern"),
                }
                assert_eq!(arms[0].body.len(), 1);
            } else {
                panic!("Expected match statement");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_match_identifier() {
        let source = r#"
        fn main() {
            match x {
                value => print("bound"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        if let Item::Function(func) = &ast.items[0] {
            if let Stmt::Match { arms, .. } = &func.body[0] {
                match &arms[0].pattern {
                    Pattern::Ident(name) => {
                        assert_eq!(name, "value");
                    }
                    _ => panic!("Expected identifier pattern"),
                }
            }
        }
    }

    #[test]
    fn test_parse_match_enum_patterns() {
        let source = r#"
        enum Option {
            Some(i64),
            None,
        }
        
        fn main() {
            match opt {
                Option::Some(n) => print_int(n),
                Option::None => print("none"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        if let Item::Function(func) = &ast.items[1] {
            if let Stmt::Match { arms, .. } = &func.body[0] {
                assert_eq!(arms.len(), 2);

                // First arm: Option::Some(n)
                match &arms[0].pattern {
                    Pattern::EnumPattern {
                        enum_name,
                        variant,
                        data,
                    } => {
                        assert_eq!(enum_name, "Option");
                        assert_eq!(variant, "Some");
                        if let Some(PatternData::Tuple(patterns)) = data {
                            assert_eq!(patterns.len(), 1);
                            match &patterns[0] {
                                Pattern::Ident(name) => assert_eq!(name, "n"),
                                _ => panic!("Expected identifier pattern in tuple"),
                            }
                        } else {
                            panic!("Expected tuple pattern data");
                        }
                    }
                    _ => panic!("Expected enum pattern"),
                }

                // Second arm: Option::None
                match &arms[1].pattern {
                    Pattern::EnumPattern {
                        enum_name,
                        variant,
                        data,
                    } => {
                        assert_eq!(enum_name, "Option");
                        assert_eq!(variant, "None");
                        assert!(data.is_none());
                    }
                    _ => panic!("Expected enum pattern"),
                }
            }
        }
    }

    #[test]
    fn test_parse_match_block_body() {
        let source = r#"
        fn main() {
            match x {
                _ => {
                    print("line 1");
                    print("line 2");
                }
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        if let Item::Function(func) = &ast.items[0] {
            if let Stmt::Match { arms, .. } = &func.body[0] {
                assert_eq!(arms[0].body.len(), 2);
            }
        }
    }

    /// `match_arm = pattern "=>" ( block | expression ) [ ',' ]`
    /// (docs/specification/grammar.ebnf:234) — the comma is optional after
    /// EITHER form. The parser used to consume it only after an expression
    /// body, so a comma after a block body was re-read as the next pattern
    /// and reported as "Expected pattern, but found ','".
    #[test]
    fn test_parse_match_block_body_comma_separated() {
        let source = r#"
        fn main() {
            match x {
                Color::Red => {
                    print("red");
                },
                Color::Green => {
                    print("green");
                },
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().expect("comma after a block arm must parse");

        let Item::Function(func) = &ast.items[0] else {
            panic!("expected a function");
        };
        let Stmt::Match { arms, .. } = &func.body[0] else {
            panic!("expected a match statement");
        };
        assert_eq!(arms.len(), 2);
        assert_eq!(arms[0].body.len(), 1);
        assert_eq!(arms[1].body.len(), 1);
    }

    #[test]
    fn test_parse_array_repeat() {
        let source = r#"
        fn main() {
            let arr = [0; 10];
            let arr2 = [42; 5];
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 2);

            // First statement: [0; 10]
            if let Stmt::Let { name, value, .. } = &func.body[0] {
                assert_eq!(name, "arr");
                if let Expr::ArrayRepeat { value, count, .. } = value {
                    if let Expr::Integer(n) = value.as_ref() {
                        assert_eq!(*n, 0);
                    } else {
                        panic!("Expected integer value");
                    }
                    if let Expr::Integer(n) = count.as_ref() {
                        assert_eq!(*n, 10);
                    } else {
                        panic!("Expected integer count");
                    }
                } else {
                    panic!("Expected array repeat expression");
                }
            }

            // Second statement: [42; 5]
            if let Stmt::Let { name, value, .. } = &func.body[1] {
                assert_eq!(name, "arr2");
                if let Expr::ArrayRepeat { value, count, .. } = value {
                    if let Expr::Integer(n) = value.as_ref() {
                        assert_eq!(*n, 42);
                    } else {
                        panic!("Expected integer value");
                    }
                    if let Expr::Integer(n) = count.as_ref() {
                        assert_eq!(*n, 5);
                    } else {
                        panic!("Expected integer count");
                    }
                } else {
                    panic!("Expected array repeat expression");
                }
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_struct_returns() {
        let source = r#"
        struct Point {
            x: i64,
            y: i64,
        }
        
        fn make_point(x: i64, y: i64) -> Point {
            return Point { x: x, y: y };
        }
        
        fn get_origin() -> Point {
            return Point { x: 0, y: 0 };
        }
        
        fn main() {
            let p = make_point(10, 20);
            let origin = get_origin();
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 4);

        // Check struct definition
        if let Item::Struct(struct_def) = &ast.items[0] {
            assert_eq!(struct_def.name, "Point");
            assert_eq!(struct_def.fields.len(), 2);
        } else {
            panic!("Expected struct definition");
        }

        // Check make_point function
        if let Item::Function(func) = &ast.items[1] {
            assert_eq!(func.name, "make_point");
            assert_eq!(func.params.len(), 2);
            assert_eq!(func.params[0].name, "x");
            assert_eq!(func.params[0].ty, Type::I64);
            assert_eq!(func.params[1].name, "y");
            assert_eq!(func.params[1].ty, Type::I64);
            assert_eq!(func.return_type, Some(Type::Custom("Point".to_string())));

            // Check return statement
            assert_eq!(func.body.len(), 1);
            if let Stmt::Return(Some(Expr::StructLiteral { name, fields, .. })) = &func.body[0] {
                assert_eq!(name, "Point");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0, "x");
                assert_eq!(fields[1].0, "y");
            } else {
                panic!("Expected return with struct literal");
            }
        } else {
            panic!("Expected function");
        }

        // Check get_origin function
        if let Item::Function(func) = &ast.items[2] {
            assert_eq!(func.name, "get_origin");
            assert_eq!(func.params.len(), 0);
            assert_eq!(func.return_type, Some(Type::Custom("Point".to_string())));

            // Check return statement
            assert_eq!(func.body.len(), 1);
            if let Stmt::Return(Some(Expr::StructLiteral { name, fields, .. })) = &func.body[0] {
                assert_eq!(name, "Point");
                assert_eq!(fields.len(), 2);
                if let Expr::Integer(n) = &fields[0].1 {
                    assert_eq!(*n, 0);
                }
                if let Expr::Integer(n) = &fields[1].1 {
                    assert_eq!(*n, 0);
                }
            } else {
                panic!("Expected return with struct literal");
            }
        } else {
            panic!("Expected function");
        }

        // Check main function
        if let Item::Function(func) = &ast.items[3] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 2);

            // First statement: let p = make_point(10, 20)
            if let Stmt::Let { name, value, .. } = &func.body[0] {
                assert_eq!(name, "p");
                if let Expr::Call { func, args, .. } = value {
                    if let Expr::Ident(fname) = func.as_ref() {
                        assert_eq!(fname, "make_point");
                    }
                    assert_eq!(args.len(), 2);
                } else {
                    panic!("Expected function call");
                }
            }

            // Second statement: let origin = get_origin()
            if let Stmt::Let { name, value, .. } = &func.body[1] {
                assert_eq!(name, "origin");
                if let Expr::Call { func, args, .. } = value {
                    if let Expr::Ident(fname) = func.as_ref() {
                        assert_eq!(fname, "get_origin");
                    }
                    assert_eq!(args.len(), 0);
                } else {
                    panic!("Expected function call");
                }
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_logical_operators() {
        let source = r#"
        fn main() {
            let a = true && false;
            let b = true || false;
            let c = x < 5 && y > 10;
            let d = (a && b) || (c && d);
            
            if a && b || c {
                print("complex condition");
            }
            
            while i < 10 && running {
                i = i + 1;
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.items.len(), 1);

        if let Item::Function(func) = &ast.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.body.len(), 6);

            // Check first statement: let a = true && false
            if let Stmt::Let { name, value, .. } = &func.body[0] {
                assert_eq!(name, "a");
                if let Expr::Binary {
                    op, left, right, ..
                } = value
                {
                    assert_eq!(*op, BinOp::And);
                    assert!(matches!(left.as_ref(), Expr::Bool(true)));
                    assert!(matches!(right.as_ref(), Expr::Bool(false)));
                } else {
                    panic!("Expected && expression");
                }
            }

            // Check second statement: let b = true || false
            if let Stmt::Let { name, value, .. } = &func.body[1] {
                assert_eq!(name, "b");
                if let Expr::Binary {
                    op, left, right, ..
                } = value
                {
                    assert_eq!(*op, BinOp::Or);
                    assert!(matches!(left.as_ref(), Expr::Bool(true)));
                    assert!(matches!(right.as_ref(), Expr::Bool(false)));
                } else {
                    panic!("Expected || expression");
                }
            }

            // Check third statement: let c = x < 5 && y > 10
            if let Stmt::Let { name, value, .. } = &func.body[2] {
                assert_eq!(name, "c");
                if let Expr::Binary {
                    op, left, right, ..
                } = value
                {
                    assert_eq!(*op, BinOp::And);
                    // Left should be x < 5
                    if let Expr::Binary { op: left_op, .. } = left.as_ref() {
                        assert_eq!(*left_op, BinOp::Lt);
                    } else {
                        panic!("Expected comparison on left side of &&");
                    }
                    // Right should be y > 10
                    if let Expr::Binary { op: right_op, .. } = right.as_ref() {
                        assert_eq!(*right_op, BinOp::Gt);
                    } else {
                        panic!("Expected comparison on right side of &&");
                    }
                } else {
                    panic!("Expected && expression");
                }
            }

            // Check fourth statement: complex expression with parentheses
            if let Stmt::Let { name, value, .. } = &func.body[3] {
                assert_eq!(name, "d");
                if let Expr::Binary { op, .. } = value {
                    assert_eq!(*op, BinOp::Or);
                } else {
                    panic!("Expected || at top level");
                }
            }

            // Check if statement with logical operators
            if let Stmt::If { condition, .. } = &func.body[4] {
                if let Expr::Binary { op, .. } = condition {
                    assert_eq!(*op, BinOp::Or); // || has lower precedence than &&
                } else {
                    panic!("Expected logical expression in if condition");
                }
            }

            // Check while statement with logical operators
            if let Stmt::While { condition, .. } = &func.body[5] {
                if let Expr::Binary { op, .. } = condition {
                    assert_eq!(*op, BinOp::And);
                } else {
                    panic!("Expected && in while condition");
                }
            }
        } else {
            panic!("Expected function");
        }
    }
}
