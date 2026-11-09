//! PD0080 — a type alias the checker expands must name a type: its expansion
//! may not reach an alias cycle.
//!
//! THE DEFECT (#93 review rounds 1 and 2, and #79). `type Id<T> = T; type A =
//! Id<A>;` overflowed the stack once a capital type argument was read as a
//! type: the checker expands `A` to `Id<A>`, substitutes, meets `A` again, and
//! recurses until the process aborts. Main refused that program only because
//! it read the capital `A` as a const and dropped it; `type Aa = Id<Aa>`
//! overflowed main's stack too, and `type A = B; type B = A;` (#79) hung it.
//!
//! TWO LAYERS, because the second is what makes the first's mistakes harmless.
//!
//!   * THE GUARD (`ExpansionGuard`) sits in every expansion that resolves an
//!     alias by recursion — the two arms of `ast_type_to_checker_type` that
//!     open one, and code generation's `type_to_c` and tuple registration,
//!     which resolve an alias the checker never expanded — so ANY
//!     expansion that does not end is stopped where it is met, whatever path
//!     reached it: an instantiated generic's signature, an imported one, a
//!     type parameter a filtered const argument left unsubstituted. It trips
//!     when more than `EXPANSION_DEPTH_LIMIT` expansions are open AND one alias
//!     is open twice; with finitely many aliases an expansion that never ends
//!     must repeat one, so it always trips, and a long chain of distinct
//!     aliases or a written `Pair<Pair<…>>` never does. `check` (and code
//!     generation's `compile`) reports a trip as PD0080, ahead of anything the
//!     stopped expansion's placeholder caused.
//!
//!   * THE PRE-CHECK (`refuse_alias_cycle_uses`) refuses, before checking
//!     starts, every use the checker is certain to expand, naming the alias
//!     used and the cycle in the alias graph: the deterministic first report.
//!
//! WHAT IS A USE — only what the checker converts whether or not anything is
//! instantiated, measured on main: a non-generic function's signature and
//! body, a non-generic struct's fields and enum's payloads, an `impl` with no
//! type parameter (its non-generic methods), a global, and the imported
//! functions `check` type-checks. NOT a trait's methods (never converted), and
//! NOT a generic item (converted only when instantiated, which the guard
//! sees): main compiled and ran every one of those with a cyclic alias in it.
//! An alias nothing uses is not refused either: main ran those too.
//!
//! THE EDGES ARE THE EXPANSIONS THE CHECKER PERFORMS, no more. A bare name is
//! looked up among the aliases with no type parameter and `Name<…>` among the
//! generic ones. A generic alias's TYPE arguments are zipped to its parameters
//! with the const arguments filtered out, as the checker does: a parameter
//! left without one is NOT substituted, and the name in the body then resolves
//! globally (`type T = G<1>; type G<T> = T;` reaches `T`). An argument is
//! expanded only if its parameter is LIVE — read somewhere the expansion
//! reaches (`type Pick<T, U> = T; type Aa = Pick<i64, Aa>;` never expands its
//! `Aa`, and main ran it). With the wrong number of arguments the checker
//! expands every argument and not the alias. Every other position — a generic
//! struct's argument, an array's element, a tuple's, a reference's referent —
//! is expanded.

use super::GenericTypeAlias;
use crate::ast::{GenericArg, Item, Program, Type};
use crate::errors::{CompileError, DiagnosticCode, Result};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// The one PD0080 message: the alias whose expansion was asked for, and the
/// cycle it reached.
fn refusal(used: &str, cycle: &[String]) -> CompileError {
    let shown = if cycle.is_empty() {
        format!("more than {} aliases deep", EXPANSION_DEPTH_LIMIT)
    } else {
        format!("the alias cycle `{}`", cycle.join(" -> "))
    };
    CompileError::Generic(format!(
        "the type alias `{}` names no type: expanding it reaches {}, which never ends. \
         An alias is another name for a type that exists without it; for a type that \
         contains itself, declare an `enum` whose variant holds it",
        used, shown
    ))
    .with_code(DiagnosticCode::TypeAliasExpansionReachesNoCycle)
}

/// How many alias expansions may be open at once before a repeated one is
/// taken for a cycle. Real nesting is a handful; the bound only has to be
/// finite and well inside the stack.
const EXPANSION_DEPTH_LIMIT: usize = 256;

/// The checker's alias expansion, guarded (see the module comment).
#[derive(Debug, Default)]
pub(crate) struct ExpansionGuard {
    open: RefCell<Vec<String>>,
    tripped: RefCell<Option<CompileError>>,
}

impl ExpansionGuard {
    /// `expand()` with the alias `name` open, or `None` once the guard has
    /// tripped — on this call or an earlier one, so that a branching cycle
    /// unwinds at once instead of being walked again from every branch.
    pub(crate) fn expand<T>(&self, name: &str, expand: impl FnOnce() -> T) -> Option<T> {
        if !self.enter(name) {
            return None;
        }
        let expanded = expand();
        self.leave();
        Some(expanded)
    }

    /// Open the alias `name`, or `false` (nothing opened) once the guard has
    /// tripped. Every `true` is closed by one `leave` — `expand`'s two halves,
    /// for a caller whose expansion needs `&mut` access its closure cannot have.
    pub(crate) fn enter(&self, name: &str) -> bool {
        if self.tripped.borrow().is_some() {
            return false;
        }
        let open = self.open.borrow();
        if open.len() >= EXPANSION_DEPTH_LIMIT {
            if let Some(cycle) = repeated(&open, name) {
                *self.tripped.borrow_mut() = Some(refusal(&open[0], &cycle));
                return false;
            }
        }
        drop(open);
        self.open.borrow_mut().push(name.to_string());
        true
    }

    /// Close the alias the last successful `enter` opened.
    pub(crate) fn leave(&self) {
        self.open.borrow_mut().pop();
    }

    /// The refusal a trip recorded, if any; the guard is clear afterwards.
    pub(crate) fn take(&self) -> Option<CompileError> {
        self.open.borrow_mut().clear();
        self.tripped.borrow_mut().take()
    }
}

/// The innermost cycle among the open expansions plus `next`: from the last
/// earlier opening of a repeated alias to its next opening.
fn repeated(open: &[String], next: &str) -> Option<Vec<String>> {
    let chain: Vec<&str> = open.iter().map(String::as_str).chain([next]).collect();
    for end in (1..chain.len()).rev() {
        if let Some(start) = chain[..end].iter().rposition(|n| *n == chain[end]) {
            return Some(chain[start..=end].iter().map(|n| n.to_string()).collect());
        }
    }
    None
}

/// An alias as the checker can reach it: by a bare name, or as `Name<…>`
/// with this many of its parameters given a type argument.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Node {
    Plain(String),
    Generic(String, usize),
}

impl Node {
    fn name(&self) -> &str {
        match self {
            Node::Plain(n) | Node::Generic(n, _) => n,
        }
    }
}

/// The two tables the checker resolves an alias from, as they will be once
/// its first pass has registered the program's own aliases over the imported
/// ones (the order `check` registers them in).
struct Aliases {
    plain: HashMap<String, Type>,
    generic: HashMap<String, (Vec<String>, Type)>,
}

impl Aliases {
    fn of(
        program: &Program,
        plain: &HashMap<String, Type>,
        generic: &HashMap<String, GenericTypeAlias>,
    ) -> Self {
        let mut aliases = Aliases {
            plain: plain.clone(),
            generic: generic
                .iter()
                .map(|(n, a)| (n.clone(), (a.type_params.clone(), a.ty.clone())))
                .collect(),
        };
        for item in &program.items {
            if let Item::TypeAlias(a) = item {
                if !a.type_params.is_empty() || !a.lifetime_params.is_empty() {
                    let entry = (a.type_params.clone(), a.ty.clone());
                    aliases.generic.insert(a.name.clone(), entry);
                } else {
                    aliases.plain.insert(a.name.clone(), a.ty.clone());
                }
            }
        }
        aliases
    }

    /// Walk `ty` as the checker expands it, with `bound` the parameters in
    /// scope: `alias` for every alias reached, `param` for every bound name.
    fn walk(
        &self,
        ty: &Type,
        bound: &[String],
        live: &HashSet<(String, usize)>,
        alias: &mut dyn FnMut(Node),
        param: &mut dyn FnMut(&str),
    ) {
        match ty {
            Type::Custom(n) | Type::TypeParam(n) if bound.contains(n) => param(n),
            Type::Custom(n) if self.plain.contains_key(n) => alias(Node::Plain(n.clone())),
            Type::Generic { name, args } => {
                let types: Vec<&Type> = args
                    .iter()
                    .filter_map(|a| match a {
                        GenericArg::Type(t) => Some(t),
                        GenericArg::Const(_) => None,
                    })
                    .collect();
                match self.generic.get(name) {
                    Some((params, _)) if args.len() == params.len() => {
                        alias(Node::Generic(name.clone(), types.len()));
                        for (k, t) in types.into_iter().enumerate() {
                            if live.contains(&(name.clone(), k)) {
                                self.walk(t, bound, live, alias, param);
                            }
                        }
                    }
                    _ => types
                        .into_iter()
                        .for_each(|t| self.walk(t, bound, live, alias, param)),
                }
            }
            Type::Array(inner, _)
            | Type::Reference { inner, .. }
            | Type::Future { output: inner } => self.walk(inner, bound, live, alias, param),
            Type::Tuple(types) => types
                .iter()
                .for_each(|t| self.walk(t, bound, live, alias, param)),
            _ => {}
        }
    }

    /// Which generic aliases' parameters an expansion reads, as a fixpoint: a
    /// parameter is live if its alias's body reads it outside a dead argument.
    fn live_parameters(&self) -> HashSet<(String, usize)> {
        let mut live = HashSet::new();
        loop {
            let mut found = Vec::new();
            for (name, (params, body)) in &self.generic {
                let mut read = Vec::new();
                self.walk(body, params, &live, &mut |_| {}, &mut |p| read.push(p.to_string()));
                for p in read {
                    if let Some(k) = params.iter().position(|q| *q == p) {
                        found.push((name.clone(), k));
                    }
                }
            }
            let before = live.len();
            live.extend(found);
            if live.len() == before {
                return live;
            }
        }
    }

    /// The aliases one expansion of `node` reaches next, in source order. A
    /// generic alias's parameters beyond the `given` ones are not substituted,
    /// so its body reads those names as the checker does: globally.
    fn edges(&self, node: &Node, live: &HashSet<(String, usize)>) -> Vec<Node> {
        let (bound, body) = match node {
            Node::Plain(n) => (Vec::new(), &self.plain[n]),
            Node::Generic(n, given) => {
                let (params, body) = &self.generic[n];
                (params[..(*given).min(params.len())].to_vec(), body)
            }
        };
        let mut next = Vec::new();
        self.walk(body, &bound, live, &mut |n| next.push(n), &mut |_| {});
        next
    }

    /// The first cycle an expansion of `node` reaches, as alias names from its
    /// first repeated alias back to itself. `done` holds aliases already shown
    /// to reach none, so every alias is searched once.
    fn cycle_from(
        &self,
        node: &Node,
        live: &HashSet<(String, usize)>,
        path: &mut Vec<Node>,
        done: &mut HashSet<Node>,
    ) -> Option<Vec<String>> {
        if let Some(i) = path.iter().position(|p| p == node) {
            let mut cycle: Vec<String> = path[i..].iter().map(|n| n.name().to_string()).collect();
            cycle.push(node.name().to_string());
            return Some(cycle);
        }
        if done.contains(node) {
            return None;
        }
        path.push(node.clone());
        for next in self.edges(node, live) {
            if let Some(cycle) = self.cycle_from(&next, live, path, done) {
                return Some(cycle);
            }
        }
        path.pop();
        done.insert(node.clone());
        None
    }
}

/// The part of `item` the checker converts whether or not anything is
/// instantiated (see the module comment), or `None`.
fn converted_part(item: &Item) -> Option<Item> {
    match item {
        Item::Function(f) if f.type_params.is_empty() => Some(item.clone()),
        Item::Struct(s) if s.type_params.is_empty() => Some(item.clone()),
        Item::Enum(e) if e.type_params.is_empty() => Some(item.clone()),
        Item::Global(_) => Some(item.clone()),
        Item::Impl(b) if b.type_params.is_empty() => {
            let mut b = b.clone();
            b.methods.retain(|m| m.type_params.is_empty());
            Some(Item::Impl(b))
        }
        _ => None,
    }
}

/// Refuse the first use of an alias whose expansion reaches an alias cycle,
/// naming the alias used and the cycle. Run before any pass of the checker
/// expands an alias; `plain` and `generic` are the alias tables as the
/// imported modules left them. A use is a type position the checker converts
/// in `program`, or in an imported function `check` type-checks (public, not
/// generic, not shadowed by a local definition).
pub(super) fn refuse_alias_cycle_uses(
    program: &Program,
    modules: &HashMap<String, crate::resolver::ModuleInfo>,
    plain: &HashMap<String, Type>,
    generic: &HashMap<String, GenericTypeAlias>,
) -> Result<()> {
    let aliases = Aliases::of(program, plain, generic);
    if aliases.plain.is_empty() && aliases.generic.is_empty() {
        return Ok(());
    }
    let live = aliases.live_parameters();
    let mut names: Vec<&String> = modules.keys().collect();
    names.sort();
    let imported = names.into_iter().flat_map(|m| &modules[m].ast.items).filter(|item| {
        matches!(item, Item::Function(f) if matches!(f.visibility, crate::ast::Visibility::Public)
            && f.type_params.is_empty()
            && !crate::ast::local_definition_shadows_import(program, &f.name))
    });
    let converted: Vec<Item> = program.items.iter().filter_map(converted_part).collect();
    let mut uses = Vec::new();
    crate::ast::for_each_type_outside_aliases(converted.iter().chain(imported), |ty, bound| {
        aliases.walk(ty, bound, &live, &mut |n| uses.push(n), &mut |_| {})
    });
    let mut done = HashSet::new();
    for used in uses {
        if let Some(cycle) = aliases.cycle_from(&used, &live, &mut Vec::new(), &mut done) {
            return Err(refusal(used.name(), &cycle));
        }
    }
    Ok(())
}
