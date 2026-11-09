//! PD0080 — a type alias a program USES must name a type: its expansion may
//! not reach an alias cycle.
//!
//! THE DEFECT (#93 review round 1, and #79). `type Id<T> = T; type A = Id<A>;`
//! overflowed the stack once a capital type argument was read as a type: the
//! checker expands `A` to `Id<A>`, substitutes, meets `A` again, and recurses
//! until the process aborts. Main refused that program only because it read
//! the capital `A` as a const and dropped it; `type Aa = Id<Aa>` overflowed
//! main's stack too, and `type A = B; type B = A;` (#79) hung it. A cycle
//! through an array, a tuple, a reference or a generic struct's argument
//! overflowed it from a signature.
//!
//! WHY AT A USE AND NOT AT THE DECLARATION. Measured on main: every one of
//! those programs compiled and RAN when nothing used the alias (an alias is
//! expanded only where it is written), so refusing the declaration would turn
//! running programs into refusals. A use is any type position outside the
//! aliases' own bodies (`crate::ast::for_each_type_outside_aliases`, the walker
//! alias expansion uses); an alias's body is reached only through a use.
//!
//! THE EDGES ARE THE EXPANSIONS THE CHECKER PERFORMS, no more. A bare name is
//! looked up among the aliases with no type parameter and `Name<…>` among the
//! generic ones (`ast_type_to_checker_type`). A generic alias's TYPE arguments
//! are zipped to its parameters with the const arguments filtered out, as the
//! checker does, and an argument is expanded only if its parameter is LIVE —
//! read somewhere the expansion reaches. `type Pick<T, U> = T; type Aa =
//! Pick<i64, Aa>;` never expands its `Aa`, and main ran it from a signature.
//! With the wrong number of arguments the checker expands every argument and
//! not the alias. Every other position — a generic struct's argument, an
//! array's element, a tuple's, a reference's referent — is expanded.

use super::GenericTypeAlias;
use crate::ast::{GenericArg, Item, Program, Type};
use crate::errors::{CompileError, DiagnosticCode, Result};
use std::collections::{HashMap, HashSet};

/// An alias as the checker can reach it: by a bare name, or as `Name<…>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Node {
    Plain(String),
    Generic(String),
}

impl Node {
    fn name(&self) -> &str {
        match self {
            Node::Plain(n) | Node::Generic(n) => n,
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
                let types = args.iter().filter_map(|a| match a {
                    GenericArg::Type(t) => Some(t),
                    GenericArg::Const(_) => None,
                });
                match self.generic.get(name) {
                    Some((params, _)) if args.len() == params.len() => {
                        alias(Node::Generic(name.clone()));
                        for (k, t) in types.enumerate() {
                            if live.contains(&(name.clone(), k)) {
                                self.walk(t, bound, live, alias, param);
                            }
                        }
                    }
                    _ => types.for_each(|t| self.walk(t, bound, live, alias, param)),
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

    /// The aliases one expansion of `node` reaches next, in source order.
    fn edges(&self, node: &Node, live: &HashSet<(String, usize)>) -> Vec<Node> {
        let (bound, body) = match node {
            Node::Plain(n) => (Vec::new(), &self.plain[n]),
            Node::Generic(n) => (self.generic[n].0.clone(), &self.generic[n].1),
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

/// Refuse the first use of an alias whose expansion reaches an alias cycle,
/// naming the alias used and the cycle. Run before any pass of the checker
/// expands an alias; `plain` and `generic` are the alias tables as the
/// imported modules left them. A use is a type position of `program`, or of
/// an imported function `check` type-checks (public, not generic, not shadowed
/// by a local definition) — the imported code the checker expands.
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
    let mut uses = Vec::new();
    crate::ast::for_each_type_outside_aliases(program.items.iter().chain(imported), |ty, bound| {
        aliases.walk(ty, bound, &live, &mut |n| uses.push(n), &mut |_| {})
    });
    let mut done = HashSet::new();
    for used in uses {
        if let Some(cycle) = aliases.cycle_from(&used, &live, &mut Vec::new(), &mut done) {
            return Err(CompileError::Generic(format!(
                "the type alias `{}` names no type: expanding it reaches the alias cycle \
                 `{}`, which never ends. An alias is another name for a type that exists \
                 without it; for a type that contains itself, declare an `enum` whose \
                 variant holds it",
                used.name(),
                cycle.join(" -> ")
            ))
            .with_code(DiagnosticCode::TypeAliasExpansionReachesNoCycle));
        }
    }
    Ok(())
}
