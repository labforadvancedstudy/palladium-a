//! PD0080 — a type alias the compiler expands must name a type: its expansion
//! may not reach an alias cycle.
//!
//! THE DEFECT (#93 review rounds 1-4, and #79). `type Id<T> = T; type A =
//! Id<A>;` overflowed the stack once a capital type argument was read as a
//! type: the checker expands `A` to `Id<A>`, substitutes, meets `A` again, and
//! recurses until the process aborts. Main refused that program only because
//! it read the capital `A` as a const and dropped it; `type Aa = Id<Aa>`
//! overflowed main's stack too, and `type A = B; type B = A;` (#79) hung it.
//!
//! WHERE IT IS JUDGED: ONLY WHERE AN ALIAS IS OPENED. `ExpansionGuard` sits in
//! every expansion that resolves an alias by recursion — the two arms of the
//! checker's `ast_type_to_checker_type` that open one, and code generation's
//! `type_to_c` and tuple registration. Nothing models WHICH positions the
//! compiler expands: an alias is judged exactly when one of those opens it,
//! so a struct or enum the checker skips (one with a lifetime or a type
//! parameter), an uninstantiated generic, a trait's method, an import nobody
//! checks, and an alias nothing uses are never judged, as main never expanded
//! them (review round 4 — a separate pre-check that modelled the positions
//! drifted from the checker twice). `check` and `compile` report a trip as
//! PD0080, ahead of anything the stopped expansion's placeholder caused.
//!
//! WHAT A CYCLE IS. In the checker, an opened alias's expansion never ends
//! exactly when its route through alias BODIES — the graph below — reaches a
//! cycle: then the same syntactic route is open again from every alias on it.
//! If it does not, every alias opened is either on an acyclic route through
//! bodies (bounded) or a subterm of a written argument (finite), so the
//! expansion ends — however deep: `Id<Id<…<i64>…>>` 4000 deep, or a chain of
//! 4000 distinct aliases through `Id`, runs on main and is no cycle. A repeated
//! NAME is no criterion for a generic alias (`Id` is reopened by every level of
//! `Id<Id<i64>>`, with a smaller argument each time), and there is NO depth
//! limit (review round 3). An alias with no type parameter opened again inside
//! its own expansion is a cycle by itself — it has no argument, so its name is
//! its whole state — which is all code generation needs (it resolves only
//! those) and a backstop in the checker.
//!
//! THE GRAPH IS THE CHECKER'S OWN EXPANSION, NOT A MODEL OF IT. It is built
//! from the checker's alias tables as they are when the alias is opened, and
//! an edge is exactly what `ast_type_to_checker_type` expands next: a bare name
//! in the table of aliases with no type parameter (the `Custom` arm), a
//! `Name<…>` in the generic table with as many arguments as it has parameters,
//! whose body is substituted by the checker's own three steps
//! (`alias_type_arguments`, `alias_substitution`, `substitute_alias_params`),
//! and the type arguments of any other `Name<…>` (a generic struct, or an
//! alias given the wrong number). An array, a tuple, a reference or a future
//! is converted without expanding an alias in it, so it has no edge. WHICH
//! argument an expansion reads is found by substituting numbered markers and
//! seeing which survive where the walk goes — a const argument filtered out, a
//! parameter left unsubstituted (then resolved globally: `type T = G<1>; type
//! G<T> = T;` reaches `T`), an argument the body never reads (`type Pick<T, U>
//! = T; type Aa = Pick<i64, Aa>;` never expands its `Aa`, and main ran it).
//! A parameter list naming one parameter twice is refused by the parser
//! (PD0081), so a substitution is a function of the arguments.

use super::{alias_substitution, alias_type_arguments, substitute_alias_params, GenericTypeAlias};
use crate::ast::Type;
use crate::errors::{CompileError, DiagnosticCode};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// The one PD0080 message: the alias whose expansion was asked for, and the
/// cycle it reached.
fn refusal(used: &str, cycle: &[String]) -> CompileError {
    CompileError::Generic(format!(
        "the type alias `{}` names no type: expanding it reaches the alias cycle `{}`, \
         which never ends. An alias is another name for a type that exists without it; \
         for a type that contains itself, declare an `enum` whose variant holds it",
        used,
        cycle.join(" -> ")
    ))
    .with_code(DiagnosticCode::TypeAliasExpansionReachesNoCycle)
}

/// The checker's alias tables, as `ast_type_to_checker_type` reads them.
#[derive(Clone, Copy)]
pub(crate) struct AliasTables<'a> {
    pub plain: &'a HashMap<String, Type>,
    pub generic: &'a HashMap<String, GenericTypeAlias>,
}

/// The alias an expansion opens: one with no type parameter by name, or a
/// generic one with this many TYPE arguments given (a const one is filtered).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Opening<'a> {
    Plain(&'a str),
    Generic(&'a str, usize),
}

impl Opening<'_> {
    fn name(&self) -> &str {
        match self {
            Opening::Plain(n) | Opening::Generic(n, _) => n,
        }
    }

    fn node(&self) -> Node {
        match self {
            Opening::Plain(n) => Node::Plain(n.to_string()),
            Opening::Generic(n, given) => Node::Generic(n.to_string(), *given),
        }
    }
}

/// An alias as the checker opens it: by a bare name, or as `Name<…>` with
/// this many of its parameters given a type argument.
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

/// A numbered stand-in for the `k`th type argument: no identifier can spell it.
fn marker(k: usize) -> Type {
    Type::Custom(format!("\u{1}{}", k))
}

fn marker_index(name: &str) -> Option<usize> {
    name.strip_prefix('\u{1}').and_then(|k| k.parse().ok())
}

/// The graph over `tables`, with what has been worked out about it.
struct Graph<'a> {
    tables: AliasTables<'a>,
    /// For a generic alias opened with `given` type arguments: which of them
    /// its expansion goes on to expand. A fixpoint (`live`).
    live: HashMap<(String, usize), HashSet<usize>>,
}

impl<'a> Graph<'a> {
    fn new(tables: AliasTables<'a>) -> Self {
        let mut graph = Graph {
            tables,
            live: HashMap::new(),
        };
        graph.live = graph.live_fixpoint();
        graph
    }

    /// The generic alias `name`'s body with its first `given` parameters
    /// substituted by markers — the checker's substitution, step for step.
    fn substituted(&self, name: &str, given: usize) -> Type {
        let alias = &self.tables.generic[name];
        let markers: Vec<Type> = (0..given).map(marker).collect();
        let args: Vec<crate::ast::GenericArg> =
            markers.into_iter().map(crate::ast::GenericArg::Type).collect();
        let substitutions = alias_substitution(&alias.type_params, &alias_type_arguments(&args));
        substitute_alias_params(&alias.ty, &substitutions)
    }

    /// Walk `ty` as `ast_type_to_checker_type` expands it: `alias` for every
    /// alias it opens, `arg` for every marker it reaches.
    fn walk(&self, ty: &Type, alias: &mut dyn FnMut(Node), arg: &mut dyn FnMut(usize)) {
        match ty {
            Type::Custom(n) => {
                if let Some(k) = marker_index(n) {
                    arg(k);
                } else if n != "Self" && self.tables.plain.contains_key(n) {
                    alias(Node::Plain(n.clone()));
                }
            }
            Type::Generic { name, args } => {
                let types = alias_type_arguments(args);
                match self.tables.generic.get(name) {
                    Some(g) if args.len() == g.type_params.len() => {
                        alias(Node::Generic(name.clone(), types.len()));
                        if let Some(live) = self.live.get(&(name.clone(), types.len())) {
                            for (k, t) in types.iter().enumerate() {
                                if live.contains(&k) {
                                    self.walk(t, alias, arg);
                                }
                            }
                        }
                    }
                    _ => types.iter().for_each(|t| self.walk(t, alias, arg)),
                }
            }
            // `ast_type_to_checker_type`'s last arm: converted, no alias opened.
            _ => {}
        }
    }

    /// Which type arguments each generic alias's expansion goes on to expand,
    /// as a fixpoint: it reaches a marker through what is already known live.
    fn live_fixpoint(&mut self) -> HashMap<(String, usize), HashSet<usize>> {
        let mut nodes = Vec::new();
        for (name, alias) in self.tables.generic {
            for given in 0..=alias.type_params.len() {
                nodes.push((name.clone(), given));
            }
        }
        self.live = nodes.iter().map(|n| (n.clone(), HashSet::new())).collect();
        loop {
            let mut grew = false;
            for (name, given) in &nodes {
                let body = self.substituted(name, *given);
                let mut reached = HashSet::new();
                self.walk(&body, &mut |_| {}, &mut |k| {
                    reached.insert(k);
                });
                let known = self.live.get_mut(&(name.clone(), *given)).unwrap();
                let before = known.len();
                known.extend(reached);
                grew |= known.len() != before;
            }
            if !grew {
                return std::mem::take(&mut self.live);
            }
        }
    }

    /// The aliases one expansion of `node` opens next.
    fn edges(&self, node: &Node) -> Vec<Node> {
        let body = match node {
            Node::Plain(n) => self.tables.plain[n].clone(),
            Node::Generic(n, given) => self.substituted(n, *given),
        };
        let mut next = Vec::new();
        self.walk(&body, &mut |n| next.push(n), &mut |_| {});
        next
    }

    /// The first cycle an expansion of `node` reaches, as alias names from its
    /// first repeated alias back to itself. `acyclic` holds nodes shown to
    /// reach none, so every node is searched once.
    fn cycle_from(
        &self,
        node: &Node,
        path: &mut Vec<Node>,
        acyclic: &mut HashSet<Node>,
    ) -> Option<Vec<String>> {
        if let Some(i) = path.iter().position(|p| p == node) {
            let mut cycle: Vec<String> = path[i..].iter().map(|n| n.name().to_string()).collect();
            cycle.push(node.name().to_string());
            return Some(cycle);
        }
        if acyclic.contains(node) {
            return None;
        }
        path.push(node.clone());
        for next in self.edges(node) {
            if let Some(cycle) = self.cycle_from(&next, path, acyclic) {
                return Some(cycle);
            }
        }
        path.pop();
        acyclic.insert(node.clone());
        None
    }
}

/// The compiler's alias expansion, guarded (see the module comment).
#[derive(Default)]
pub(crate) struct ExpansionGuard {
    /// The aliases with no type parameter currently open, outermost first.
    open_plain: RefCell<Vec<String>>,
    /// Nodes shown to reach no cycle, and the table sizes that was shown over.
    acyclic: RefCell<(HashSet<Node>, (usize, usize))>,
    tripped: RefCell<Option<CompileError>>,
}

/// A copy for PD0082's generic post-pass, which probes each instantiation on a
/// copy of the checker (#89/#92): its own state, shared with nothing. A tripped
/// guard stays tripped in the copy, holding the error's text — the error the
/// compilation reports is the original's (`TypeChecker::check`), never the copy's.
impl Clone for ExpansionGuard {
    fn clone(&self) -> Self {
        let tripped = self.tripped.borrow().as_ref().map(|e| CompileError::Generic(e.to_string()));
        Self {
            open_plain: self.open_plain.clone(),
            acyclic: self.acyclic.clone(),
            tripped: RefCell::new(tripped),
        }
    }
}

impl std::fmt::Debug for ExpansionGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExpansionGuard")
            .field("open_plain", &self.open_plain)
            .finish()
    }
}

impl ExpansionGuard {
    /// `expand()` with `opening` open, or `None` once the guard has tripped —
    /// on this call or an earlier one, so that a branching cycle unwinds at
    /// once instead of being walked again from every branch. `tables` are the
    /// checker's alias tables; code generation, which opens only aliases with
    /// no type parameter, passes none.
    pub(crate) fn expand<T>(
        &self,
        opening: Opening<'_>,
        tables: Option<AliasTables<'_>>,
        expand: impl FnOnce() -> T,
    ) -> Option<T> {
        if !self.enter(opening, tables) {
            return None;
        }
        let expanded = expand();
        self.leave(opening);
        Some(expanded)
    }

    /// Open `opening`, or `false` (nothing opened) once the guard has tripped.
    /// Every `true` is closed by one `leave` of the same opening — `expand`'s
    /// two halves, for a caller whose expansion needs `&mut` access.
    pub(crate) fn enter(&self, opening: Opening<'_>, tables: Option<AliasTables<'_>>) -> bool {
        if self.tripped.borrow().is_some() {
            return false;
        }
        if let Some(cycle) = self.cycle_at(opening, tables) {
            *self.tripped.borrow_mut() = Some(refusal(opening.name(), &cycle));
            return false;
        }
        if let Opening::Plain(name) = opening {
            self.open_plain.borrow_mut().push(name.to_string());
        }
        true
    }

    /// Close an opening the last successful `enter` opened.
    pub(crate) fn leave(&self, opening: Opening<'_>) {
        if let Opening::Plain(_) = opening {
            self.open_plain.borrow_mut().pop();
        }
    }

    /// The cycle `opening` reaches: by being open already (an alias with no
    /// type parameter), or in the graph over `tables`.
    fn cycle_at(&self, opening: Opening<'_>, tables: Option<AliasTables<'_>>) -> Option<Vec<String>> {
        if let Opening::Plain(name) = opening {
            let open = self.open_plain.borrow();
            if let Some(i) = open.iter().position(|n| n == name) {
                let mut cycle: Vec<String> = open[i..].to_vec();
                cycle.push(name.to_string());
                return Some(cycle);
            }
        }
        let tables = tables?;
        let node = opening.node();
        let mut acyclic = self.acyclic.borrow_mut();
        // The tables only grow (registration); a node shown acyclic over
        // fewer aliases is shown again.
        let sizes = (tables.plain.len(), tables.generic.len());
        if acyclic.1 != sizes {
            *acyclic = (HashSet::new(), sizes);
        }
        if acyclic.0.contains(&node) {
            return None;
        }
        let graph = Graph::new(tables);
        graph.cycle_from(&node, &mut Vec::new(), &mut acyclic.0)
    }

    /// Forget what was shown: the checker replaced an alias in its tables.
    pub(crate) fn forget(&self) {
        self.acyclic.borrow_mut().0.clear();
    }

    /// The refusal a trip recorded, if any; the guard is clear afterwards.
    pub(crate) fn take(&self) -> Option<CompileError> {
        self.open_plain.borrow_mut().clear();
        self.tripped.borrow_mut().take()
    }
}
