// Type checker for Palladium
// "Ensuring legends are logically sound"

use crate::ast::{AssignTarget, UnaryOp, *};
use crate::errors::{CompileError, DiagnosticCode, Result, Span};
use std::collections::{HashMap, HashSet};

mod suggestions;
use suggestions::TypeErrorHelper;

mod exhaustiveness;
use exhaustiveness::{EnumInfo, ExhaustivenessChecker, VariantInfo};

mod trait_resolution;
use trait_resolution::TraitResolver;

/// How a method declared its receiver. `self` is a place base now, and these are the
/// three answers to "may this method write through it".
///
///   `&mut self`  MutRef   -- writes propagate to the caller; the only writable form.
///   `&self`      Shared   -- a shared borrow. Writing lowered to `self->n = v` against
///                            `const struct C*`, which gcc refused: a front-end refusal
///                            replaces that, because this language's rules are not C's
///                            to enforce.
///   `self`       ByValue  -- a COPY, and not a `mut` binding (there is no `mut self`
///                            form). Writing it mutated the copy, so the caller observed
///                            nothing: it compiled, it ran, and it did not do what it
///                            said. Refused for the ordinary reason a non-`mut` binding
///                            cannot be assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfReceiver {
    ByValue,
    Shared,
    MutRef,
}

/// Which enum payload slots must be stored behind a pointer, and which type
/// declarations have no layout at all.
///
/// THE ONE DEFINITION OF THAT QUESTION, because it is asked in five places: the
/// type checker refuses the declarations that have no layout, and code
/// generation chooses the C type of every payload slot, allocates in every
/// constructor, and dereferences in every `match` binding. When it was asked in
/// zero places, `enum V { Leaf(i64), Pair(V, V) }` passed the parser, the type
/// checker AND the borrow checker, and then emitted
///
/// ```c
/// typedef struct { struct V field0; struct V field1; } V__Pair_Data;
/// ```
///
/// inside the definition of `struct V` itself, which gcc rejects
/// (`field has incomplete type 'struct V'`). Splitting the question between the
/// two passes is exactly how they would come to disagree about which slot is a
/// pointer, so both passes read this one answer.
///
/// # The rule
///
/// Build the by-value containment graph over the named types: an edge `A -> B`
/// means a value of `A` stores a value of `B` inside itself. `&T` contributes
/// no edge — a reference is already a pointer in C, so it can close a cycle in
/// the source without closing one in the layout.
///
/// Cut the edge out of an `enum` payload slot whose declared type is *directly*
/// a named type that can reach that enum again. Those slots become pointers.
///
/// Whatever still reaches itself after the cuts has no layout and is refused.
///
/// The cut graph is kept, because it answers a SECOND question with the same
/// edges: in what order must the definitions be emitted. An edge `A -> B` that
/// survived the cut is exactly a `B` stored inside an `A` by value, which is a
/// C field of a complete type, so `B`'s definition must precede `A`'s. A cut
/// edge imposes nothing, because a `struct B*` field only needs the tag.
/// `definition_order` is that answer, and it reads these edges rather than
/// deriving its own — a second notion of "contains by value" is a second thing
/// to keep in sync with this one.
///
/// # Why cutting at enums is enough for every type a program can build
///
/// A value of a type on a by-value cycle is infinite unless some point on the
/// cycle can stop, and the only construct in this language that can stop is an
/// `enum` variant that does not recurse. So an inhabited cycle contains an
/// `enum` node, and the edge leaving an `enum` node is a payload slot: exactly
/// the edge this rule cuts. What survives the cuts is uninhabited — no program
/// can construct a `struct Node { next: Node }` — so refusing it loses nothing
/// a program could have used, and it is refused in the type checker, before any
/// C exists, rather than by gcc.
///
/// The rule is deliberately narrow in one direction: recursion reached through
/// an array or a tuple (`enum V { Many([V; 3]) }`) is NOT cut, because making
/// that slot a pointer would change the shape of the array rather than the
/// element. It therefore survives to the refusal, which is the failing-closed
/// direction.
///
/// # The one place the paragraph above OVER-APPROXIMATES, and why it stays
///
/// "An inhabited cycle contains an `enum` node" is a claim about what can stop
/// a cycle, and an `enum` variant is not the only thing that can: CARDINALITY
/// can. `struct Z { xs: [Z; 0] }` stores zero `Z`s, so its size is finite and
/// the cycle is broken with no `enum` anywhere. This analysis counts that edge
/// anyway — `[T; N]` contributes an edge for every `N`, including `0` — and so
/// refuses a declaration whose size it could in principle have computed.
///
/// That is a DECISION, recorded here and named in the diagnostic, not an
/// oversight of a discarded `_`:
///
///   * There is nothing to emit. A `[Z; 0]` field is `struct Z xs[0];` inside
///     the definition of `struct Z` itself — an array of an INCOMPLETE element
///     type, which C has no spelling for and gcc rejects with
///     `array has incomplete element type`. Accepting the declaration here only
///     moves the failure from a diagnostic into gcc, which is the exact shape
///     this analysis exists to remove.
///   * There is nothing to construct. `[T; 0]` is uninhabited in this language
///     today: the only expression that could produce one is the empty array
///     literal, and the type checker refuses it ("Empty array literals are not
///     supported (cannot infer type)", measured on `Holder { xs: [] }`). So the
///     refusal fails closed onto a type no program has a value of.
///
/// Both halves are pinned: `tests/reject/zero_length_array_self_reference.pd`
/// holds the refusal and its fingerprint, and requirement `N4-23` states the
/// exclusion so the decision outlives the current limitation. `N4-23`, not
/// `N4-22`: N4-22 is the POSITIVE row, about an enum payload naming its own
/// enum, and says nothing about arrays. Pointing the exclusion at it sent a
/// reader who hit this refusal to a row that does not mention their program. If the empty
/// array literal is ever inferrable, this is the site to revisit — and the
/// first thing to check is the C, not this graph.
#[derive(Debug, Clone, Default)]
pub struct RecursiveLayout {
    /// Non-generic type aliases, so `type Tree = V;` names the same node as `V`.
    aliases: HashMap<String, Type>,
    /// Reachability over the uncut graph: `reaches[a]` is every named type a
    /// value of `a` can store inside itself, at any depth. Seeded from
    /// successors, so `a` appears in its own set only when `a` is on a cycle.
    reaches: HashMap<String, HashSet<String>>,
    /// The containment graph AFTER the cuts: `contains_cut[a]` is every named
    /// type stored inside an `a` by value, one hop, in declared order. This is
    /// what `definition_order` walks.
    contains_cut: HashMap<String, Vec<String>>,
    /// Edges whose ONLY route is through a zero-length array, so the diagnostic
    /// can say that this particular cycle is the over-approximated case rather
    /// than assert a mechanism that does not apply to it. An `(a, b)` pair is in
    /// here only when no other field or payload of `a` stores a `b` directly.
    zero_length_edges: HashSet<(String, String)>,
    /// Names that still store themselves after every cut, each with the cycle
    /// that proves it, so the diagnostic can show the path rather than assert
    /// it.
    no_layout: Vec<(String, Vec<String>)>,
    /// Whether any slot was cut. Programs with none must emit byte-identical C
    /// to what they emitted before this analysis existed.
    cut_any: bool,
}

/// The item set a layout analysis is allowed to see.
///
/// A newtype with ONE constructor, and that is the whole point of it. The
/// analysis keys every declaration by BARE NAME, and both callers used to hand
/// it `program.items` chained onto every imported module's items, unfiltered. So
/// an imported declaration merged into the local graph under its name whether or
/// not the program could name it — and this is the pass that decides whether a
/// program is ACCEPTED.
///
/// ```text
/// lib.pd    struct B { x: i64 }        // private, never named downstream
///           struct A { b: B }          // private, never named downstream
///
/// main.pd   import lib;
///           enum A { End, More(B) }    // cuts at the enum payload slot
///           struct B { a: A }
///           fn main() { let b: B = B { a: A::End }; print("built"); }
/// ```
///
/// was refused with `recursive type `A` has no layout ... (A -> B -> A)`, and
/// the same file with the `import` line deleted compiled and ran. The cycle
/// existed only because the hidden imported `struct A { b: B }` was merged into
/// the local graph by name. So an `import` of PRIVATE types the program never
/// mentions decided whether a valid program compiled — N3-15's own headline, one
/// axis over, and failing CLOSED onto a valid program, which is the worse
/// polarity of the two.
///
/// WHY A TYPE RATHER THAN A FILTER AT EACH CALL SITE. `local_type_shadows_import`
/// already governed registration and emission; the layout analysis was a THIRD
/// consumer and nobody remembered it. Adding a third filter call leaves a fourth
/// consumer free to be written wrong the same way. Passing a `LayoutItems` makes
/// the unfiltered input unconstructible: there is no way to call `analyze`
/// without going through `of`, so forgetting is not available.
pub struct LayoutItems<'a> {
    items: Vec<&'a Item>,
}

impl<'a> LayoutItems<'a> {
    /// The program's own items, plus the imported ones the program can NAME.
    ///
    /// Visible and unshadowed, by exactly the two rules the registration and
    /// emission walks apply — `Visibility::Public` and
    /// `crate::ast::local_type_shadows_import` — so all three consumers of the
    /// predicate agree about which `A` is meant.
    ///
    /// Only `struct`, `enum` and `type` are taken from imports because those are
    /// the only kinds `analyze` reads; taking more would be a set the caller
    /// cannot reason about. Local items pass through whole: a local declaration
    /// is always nameable, and `analyze` ignores the kinds it does not want.
    ///
    /// Modules are visited in sorted key order. `imported_modules` is a
    /// `HashMap` and `RandomState` reseeds per process, and this set feeds both
    /// the emission ORDER and the choice of WHICH declaration a refusal names,
    /// so an unsorted walk puts the hash seed into the emitted C and into the
    /// diagnostic.
    pub fn of(
        program: &'a Program,
        imported: &'a HashMap<String, crate::resolver::ModuleInfo>,
    ) -> Self {
        let mut items: Vec<&'a Item> = program.items.iter().collect();

        let mut sorted: Vec<_> = imported.iter().collect();
        sorted.sort_by_key(|(name, _)| *name);
        for (_, module_info) in sorted {
            for item in &module_info.ast.items {
                let (name, public) = match item {
                    Item::Struct(def) => (
                        &def.name,
                        matches!(def.visibility, crate::ast::Visibility::Public),
                    ),
                    Item::Enum(def) => (
                        &def.name,
                        matches!(def.visibility, crate::ast::Visibility::Public),
                    ),
                    Item::TypeAlias(def) => (
                        &def.name,
                        matches!(def.visibility, crate::ast::Visibility::Public),
                    ),
                    _ => continue,
                };
                if public && !crate::ast::local_type_shadows_import(program, name) {
                    items.push(item);
                }
            }
        }

        LayoutItems { items }
    }

    fn iter(&self) -> impl Iterator<Item = &'a Item> + Clone + '_ {
        self.items.iter().copied()
    }
}

impl RecursiveLayout {
    /// Analyse every `struct`, `enum` and `type` item reachable by the program,
    /// including the ones it imported. Generic definitions are skipped: they are
    /// monomorphized into concrete items elsewhere, and `type_to_c` erases their
    /// parameters to `void*` today, which is a separate open defect.
    pub fn analyze(items: &LayoutItems<'_>) -> Self {
        let items = items.iter();
        let mut layout = RecursiveLayout::default();

        for item in items.clone() {
            if let Item::TypeAlias(alias) = item {
                if alias.type_params.is_empty() && alias.lifetime_params.is_empty() {
                    layout.aliases.insert(alias.name.clone(), alias.ty.clone());
                }
            }
        }

        // Edges, before any cut.
        let mut contains: HashMap<String, Vec<String>> = HashMap::new();
        for item in items.clone() {
            match item {
                Item::Struct(def)
                    if def.type_params.is_empty() && def.lifetime_params.is_empty() =>
                {
                    let mut occ = Vec::new();
                    for (_, ty) in &def.fields {
                        layout.labelled_occurrences(ty, false, &mut occ);
                    }
                    layout.record_edges(&def.name, occ, &mut contains);
                }
                Item::Enum(def) if def.type_params.is_empty() && def.lifetime_params.is_empty() => {
                    let mut occ = Vec::new();
                    for variant in &def.variants {
                        for ty in Self::payload_types(&variant.data) {
                            layout.labelled_occurrences(ty, false, &mut occ);
                        }
                    }
                    layout.record_edges(&def.name, occ, &mut contains);
                }
                _ => {}
            }
        }
        layout.reaches = Self::close(&contains);

        // The zero-length labels are a property of the CUT graph, because that
        // is the graph `path_back_to` reports a cycle over. Keeping the uncut
        // pass's labels too would be harmless — they are a subset — but it
        // would leave a reader to derive that, so the pass that matters owns
        // them alone.
        layout.zero_length_edges.clear();

        // Cut, then look for what survived.
        let mut cut: HashMap<String, Vec<String>> = HashMap::new();
        for item in items.clone() {
            match item {
                Item::Struct(def)
                    if def.type_params.is_empty() && def.lifetime_params.is_empty() =>
                {
                    let mut occ = Vec::new();
                    for (_, ty) in &def.fields {
                        layout.labelled_occurrences(ty, false, &mut occ);
                    }
                    layout.record_edges(&def.name, occ, &mut cut);
                }
                Item::Enum(def) if def.type_params.is_empty() && def.lifetime_params.is_empty() => {
                    let mut occ = Vec::new();
                    for variant in &def.variants {
                        for ty in Self::payload_types(&variant.data) {
                            if layout.payload_is_indirect(&def.name, ty) {
                                layout.cut_any = true;
                                continue;
                            }
                            layout.labelled_occurrences(ty, false, &mut occ);
                        }
                    }
                    layout.record_edges(&def.name, occ, &mut cut);
                }
                _ => {}
            }
        }

        let survived = Self::close(&cut);
        let mut names: Vec<&String> = survived.keys().collect();
        names.sort();
        for name in names {
            if survived[name].contains(name) {
                let path = Self::path_back_to(name, &cut).unwrap_or_else(|| vec![name.clone()]);
                layout.no_layout.push((name.clone(), path));
            }
        }
        layout.contains_cut = cut;
        layout
    }

    /// Does this enum payload slot become a pointer?
    ///
    /// The four emission sites in code generation and the refusal in the type
    /// checker all call this with the same AST node they already hold, so none
    /// of them can form its own opinion about which slot is indirect.
    pub fn payload_is_indirect(&self, enum_name: &str, ty: &Type) -> bool {
        match self.resolve_alias(ty) {
            Some(Type::Custom(target)) => self
                .reaches
                .get(&target)
                .is_some_and(|set| set.contains(enum_name)),
            _ => false,
        }
    }

    /// The declarations with no layout, each paired with the containment cycle
    /// that proves it.
    pub fn declarations_without_layout(&self) -> &[(String, Vec<String>)] {
        &self.no_layout
    }

    /// Is this reported cycle held together only by zero-length arrays?
    ///
    /// True when at least one hop of the path is an edge whose only route is a
    /// `[T; 0]`. The diagnostic asks, because on such a cycle the size IS
    /// bounded and a message that says otherwise is false — see the type-level
    /// documentation for why the declaration is refused anyway.
    pub fn cycle_crosses_a_zero_length_array(&self, cycle: &[String]) -> bool {
        cycle.windows(2).any(|hop| {
            self.zero_length_edges
                .contains(&(hop[0].clone(), hop[1].clone()))
        })
    }

    /// The order in which these type definitions must be EMITTED, dependencies
    /// first.
    ///
    /// C needs a complete type to give a field a size, so `B` must be defined
    /// before `A` whenever an `A` stores a `B` by value. Source order does not
    /// deliver that — `struct S { e: E }` written above `enum E { A, B }` is a
    /// perfectly ordinary program, and emitting it in source order produced
    /// `field has incomplete type 'struct E'` from gcc, making the program's
    /// validity depend on the order its declarations happen to be written in.
    ///
    /// The edges are `contains_cut`, i.e. THE SAME cut this analysis performed
    /// for the layout refusal, not a second traversal of the AST. A cut edge is
    /// a `struct B*` field, which needs only the tag, so it constrains nothing;
    /// an edge that survived the cut is a by-value field, which constrains
    /// everything. Deriving a second containment predicate here is how the two
    /// would come to disagree.
    ///
    /// STABLE: a set already in a valid order comes back unchanged, so the C
    /// emitted for every program that compiled before this existed does not
    /// move. Names not in `names` are not traversed — they are either emitted
    /// in an earlier phase (imports) or not emitted at all (generic templates,
    /// which this analysis skips) and in neither case do they order anything
    /// here.
    ///
    /// `Err(cycle)` is a by-value containment cycle among the requested names.
    /// It cannot arise from a program the type checker accepted, because that
    /// is precisely what `declarations_without_layout` refuses; a caller that
    /// gets one must refuse rather than emit C gcc will reject.
    pub fn definition_order(
        &self,
        names: &[String],
    ) -> std::result::Result<Vec<String>, Vec<String>> {
        let wanted: HashSet<&str> = names.iter().map(String::as_str).collect();
        let mut done: HashSet<String> = HashSet::new();
        let mut in_path: HashSet<String> = HashSet::new();
        let mut path: Vec<String> = Vec::new();
        let mut order: Vec<String> = Vec::new();

        // Explicit stack rather than recursion: the depth is the length of a
        // containment chain in the source, which nothing bounds.
        for root in names {
            if done.contains(root) {
                continue;
            }
            let mut work: Vec<(String, bool)> = vec![(root.clone(), false)];
            while let Some((name, expanded)) = work.pop() {
                if expanded {
                    in_path.remove(&name);
                    path.pop();
                    if done.insert(name.clone()) {
                        order.push(name);
                    }
                    continue;
                }
                if done.contains(&name) {
                    continue;
                }
                if in_path.contains(&name) {
                    let from = path.iter().position(|n| *n == name).unwrap_or(0);
                    let mut cycle: Vec<String> = path[from..].to_vec();
                    cycle.push(name);
                    return Err(cycle);
                }
                in_path.insert(name.clone());
                path.push(name.clone());
                work.push((name.clone(), true));

                let mut children: Vec<&String> = Vec::new();
                for child in self.contains_cut.get(&name).into_iter().flatten() {
                    if wanted.contains(child.as_str()) && !children.contains(&child) {
                        children.push(child);
                    }
                }
                // Reversed, so the stack pops them in declared order and the
                // result is source order whenever source order already works.
                for child in children.into_iter().rev() {
                    work.push((child.clone(), false));
                }
            }
        }
        Ok(order)
    }

    /// Did anything become a pointer? Programs where nothing did must emit the
    /// C they emitted before, byte for byte.
    pub fn cuts_anything(&self) -> bool {
        self.cut_any
    }

    fn payload_types(data: &EnumVariantData) -> Vec<&Type> {
        match data {
            EnumVariantData::Unit => vec![],
            EnumVariantData::Tuple(types) => types.iter().collect(),
            EnumVariantData::Struct(fields) => fields.iter().map(|(_, ty)| ty).collect(),
        }
    }

    /// Follow `type` aliases to whatever they finally name. Returns `None` on an
    /// alias that never lands, so a cyclic alias cannot spin here — it is a
    /// separate defect and this analysis will not be the place it appears as a
    /// hang.
    fn resolve_alias(&self, ty: &Type) -> Option<Type> {
        let mut current = ty.clone();
        for _ in 0..self.aliases.len() + 1 {
            match &current {
                Type::Custom(name) => match self.aliases.get(name) {
                    Some(next) => current = next.clone(),
                    None => return Some(current),
                },
                _ => return Some(current),
            }
        }
        None
    }

    /// Every named type a value of `ty` stores inside itself, appended to `out`,
    /// each paired with whether the route to it crossed a zero-length array.
    ///
    /// `Reference` contributes nothing on purpose: it is a pointer already.
    ///
    /// A `[T; 0]` still contributes its edge — the analysis over-approximates on
    /// purpose and the type-level documentation says why — but it contributes a
    /// LABELLED one, so the refusal can describe the cycle it actually found
    /// instead of asserting a mechanism that does not apply to it. Only
    /// `ArraySize::Literal(0)` counts: a `ConstParam` or an `Expr` length is a
    /// length this compiler has not evaluated, and treating an unknown as zero
    /// would be the unsound direction.
    fn labelled_occurrences(&self, ty: &Type, crossed_zero: bool, out: &mut Vec<(String, bool)>) {
        match self.resolve_alias(ty) {
            Some(Type::Custom(name)) => out.push((name, crossed_zero)),
            Some(Type::Array(elem, size)) => {
                let zero = matches!(size, ArraySize::Literal(0));
                self.labelled_occurrences(&elem, crossed_zero || zero, out);
            }
            Some(Type::Tuple(types)) => {
                for t in &types {
                    self.labelled_occurrences(t, crossed_zero, out);
                }
            }
            _ => {}
        }
    }

    /// File one declaration's occurrences into a containment graph, and label
    /// the edges that exist ONLY because of a zero-length array.
    ///
    /// The "only" is load-bearing: `struct Q { a: Z, b: [Z; 0] }` reaches `Z`
    /// directly as well, so `Q -> Z` is an ordinary by-value edge and saying
    /// otherwise in a diagnostic would be a second false sentence in place of
    /// the first.
    ///
    /// The entry is created even when there are no occurrences, because `close`
    /// iterates the keys and a declaration missing from them is a declaration
    /// with no reachability set at all.
    fn record_edges(
        &mut self,
        owner: &str,
        occurrences: Vec<(String, bool)>,
        into: &mut HashMap<String, Vec<String>>,
    ) {
        let direct: HashSet<&str> = occurrences
            .iter()
            .filter(|(_, crossed_zero)| !crossed_zero)
            .map(|(name, _)| name.as_str())
            .collect();
        for (name, crossed_zero) in &occurrences {
            if *crossed_zero && !direct.contains(name.as_str()) {
                self.zero_length_edges
                    .insert((owner.to_string(), name.clone()));
            }
        }
        into.entry(owner.to_string())
            .or_default()
            .extend(occurrences.into_iter().map(|(name, _)| name));
    }

    /// Transitive closure seeded from successors, so a name lands in its own set
    /// only by way of a cycle.
    fn close(edges: &HashMap<String, Vec<String>>) -> HashMap<String, HashSet<String>> {
        let mut out: HashMap<String, HashSet<String>> = HashMap::new();
        for start in edges.keys() {
            let mut seen: HashSet<String> = HashSet::new();
            let mut stack: Vec<String> = edges[start].clone();
            while let Some(node) = stack.pop() {
                if !seen.insert(node.clone()) {
                    continue;
                }
                if let Some(next) = edges.get(&node) {
                    stack.extend(next.iter().cloned());
                }
            }
            out.insert(start.clone(), seen);
        }
        out
    }

    /// A concrete cycle from `start` back to `start`, for the diagnostic.
    fn path_back_to(start: &str, edges: &HashMap<String, Vec<String>>) -> Option<Vec<String>> {
        let mut stack = vec![(start.to_string(), vec![start.to_string()])];
        let mut seen: HashSet<String> = HashSet::new();
        while let Some((node, path)) = stack.pop() {
            for next in edges.get(&node).into_iter().flatten() {
                if next == start {
                    let mut done = path.clone();
                    done.push(next.clone());
                    return Some(done);
                }
                if seen.insert(next.clone()) {
                    let mut deeper = path.clone();
                    deeper.push(next.clone());
                    stack.push((next.clone(), deeper));
                }
            }
        }
        None
    }
}

/// What a `break` may carry out of the loop it binds to.
///
/// Kept as a stack frame rather than as a flag on the loop node, because the
/// question a `break` asks is about the loop it is INSIDE, and only the walk
/// knows that.
#[derive(Debug, Clone)]
enum BreakTarget {
    /// A `loop`/`while`/`for` written for its effect. A `break` may leave it;
    /// a `break` may not hand it a value, because there is nothing on the other
    /// side to receive one.
    Statement,
    /// A `loop` in value position. `Some(t)` once a `break` has fixed the type;
    /// the loop's own type is that `t`, and a loop that never gets one has no
    /// value and is refused.
    Value(Option<CheckerType>),
}

/// Type representation for type checker (wraps AST Type)
#[derive(Debug, Clone, PartialEq)]
pub enum CheckerType {
    Unit,
    String,
    Int,
    /// `f64` and `f32` (N4-02).
    ///
    /// ONE checker type for both widths, deliberately. The checker's job here
    /// is to keep floats and integers from mixing without a cast, and `as`
    /// casts do not exist yet (N5, still owed) — so a program cannot construct
    /// a case where `f32` and `f64` need to be told apart, and a distinction
    /// the language has no syntax to observe would be a distinction that only
    /// the compiler's internals could be wrong about. The AST keeps both
    /// (`Type::F32` / `Type::F64`) because code generation must emit `float`
    /// or `double`, and that IS observable.
    Float,
    Bool,
    /// One Unicode scalar (N4-04), NOT an `i64`.
    ///
    /// Distinct with no implicit conversion in either direction: `'a' + 1`
    /// and `print_int('a')` are both type errors now, and `'a' as i64` /
    /// `97 as char` are how you cross. The carrier in C is unchanged
    /// (`long long`), so this distinction costs nothing at run time — it is
    /// entirely a claim about what the program meant.
    Char,
    Array(Box<CheckerType>, ArraySizeValue),
    Function(Vec<CheckerType>, Box<CheckerType>),
    Struct(String),
    TypeParam(String),
    Enum(String),
    Generic {
        name: String,
        args: Vec<GenericArgValue>,
    },
    Tuple(Vec<CheckerType>),
    /// `a..b` and `a..=b` (N5-14).
    ///
    /// A TYPE OF ITS OWN, replacing the `Array(Int, 0)` this used to answer.
    /// That answer was a lie told to make the `for` header typecheck, and it
    /// type-checked much more than a `for` header: it made a range assignable
    /// to an `[i64; 0]`, indexable, and passable wherever an array was wanted.
    /// The specification names no operations on a range beyond iterating it,
    /// so this type has none.
    Range,
}

/// Map a built-in's type (from the canonical registry) to a checker type.
fn builtin_type(ty: crate::builtins::BuiltinType) -> CheckerType {
    use crate::builtins::BuiltinType;
    match ty {
        BuiltinType::I64 => CheckerType::Int,
        BuiltinType::Str => CheckerType::String,
        BuiltinType::Bool => CheckerType::Bool,
        BuiltinType::Char => CheckerType::Char,
        BuiltinType::Unit => CheckerType::Unit,
    }
}

/// Array size value for type checking
#[derive(Debug, Clone, PartialEq)]
pub enum ArraySizeValue {
    Literal(usize),
    ConstParam(String),
}

impl std::fmt::Display for ArraySizeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArraySizeValue::Literal(n) => write!(f, "{}", n),
            ArraySizeValue::ConstParam(name) => write!(f, "{}", name),
        }
    }
}

/// Generic argument value for type checking
#[derive(Debug, Clone, PartialEq)]
pub enum GenericArgValue {
    Type(CheckerType),
    Const(ConstValueResolved),
}

/// Resolved const value
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValueResolved {
    Integer(i64),
    ConstParam(String),
}

impl From<&crate::ast::Type> for CheckerType {
    fn from(ast_type: &crate::ast::Type) -> Self {
        match ast_type {
            crate::ast::Type::Unit => CheckerType::Unit,
            crate::ast::Type::String => CheckerType::String,
            crate::ast::Type::I32 | crate::ast::Type::I64 => CheckerType::Int,
            crate::ast::Type::F32 | crate::ast::Type::F64 => CheckerType::Float,
            crate::ast::Type::Bool => CheckerType::Bool,
            crate::ast::Type::Char => CheckerType::Char,
            crate::ast::Type::U32 | crate::ast::Type::U64 => CheckerType::Int,
            crate::ast::Type::Array(elem_type, size) => {
                let size_value = match size {
                    ArraySize::Literal(n) => ArraySizeValue::Literal(*n),
                    ArraySize::ConstParam(name) => ArraySizeValue::ConstParam(name.clone()),
                    ArraySize::Expr(_) => {
                        // For now, we don't support expressions
                        ArraySizeValue::Literal(0) // Placeholder
                    }
                };
                CheckerType::Array(Box::new(CheckerType::from(elem_type.as_ref())), size_value)
            }
            crate::ast::Type::Custom(name) => CheckerType::Struct(name.clone()),
            crate::ast::Type::TypeParam(name) => {
                // Type parameters need proper handling through substitution
                // For now, create a placeholder type that can be unified later
                CheckerType::TypeParam(name.clone())
            }
            crate::ast::Type::Generic { name, args } => {
                // Convert generic arguments properly
                let checker_args: Vec<GenericArgValue> = args
                    .iter()
                    .map(|arg| match arg {
                        GenericArg::Type(t) => GenericArgValue::Type(CheckerType::from(t)),
                        GenericArg::Const(c) => GenericArgValue::Const(match c {
                            ConstValue::Integer(n) => ConstValueResolved::Integer(*n),
                            ConstValue::ConstParam(name) => {
                                ConstValueResolved::ConstParam(name.clone())
                            }
                        }),
                    })
                    .collect();
                CheckerType::Generic {
                    name: name.clone(),
                    args: checker_args,
                }
            }
            crate::ast::Type::Reference { inner, .. } => {
                // For now, treat references as the inner type
                // TODO: Proper reference type handling
                CheckerType::from(inner.as_ref())
            }
            crate::ast::Type::Future { output } => {
                // Create a Future generic type
                CheckerType::Generic {
                    name: "Future".to_string(),
                    args: vec![GenericArgValue::Type(CheckerType::from(output.as_ref()))],
                }
            }
            crate::ast::Type::Tuple(types) => {
                CheckerType::Tuple(types.iter().map(CheckerType::from).collect())
            }
        }
    }
}

impl std::fmt::Display for CheckerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckerType::Unit => write!(f, "()"),
            CheckerType::Range => write!(f, "a range"),
            CheckerType::String => write!(f, "String"),
            CheckerType::Int => write!(f, "Int"),
            CheckerType::Float => write!(f, "Float"),
            CheckerType::Bool => write!(f, "Bool"),
            CheckerType::Char => write!(f, "Char"),
            CheckerType::Array(elem_type, size) => match size {
                ArraySizeValue::Literal(n) => write!(f, "[{}; {}]", elem_type, n),
                ArraySizeValue::ConstParam(name) => write!(f, "[{}; {}]", elem_type, name),
            },
            CheckerType::Function(params, ret) => {
                write!(f, "fn(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", param)?;
                }
                write!(f, ") -> {}", ret)
            }
            CheckerType::Struct(name) => write!(f, "{}", name),
            CheckerType::TypeParam(name) => write!(f, "{}", name),
            CheckerType::Enum(name) => write!(f, "{}", name),
            CheckerType::Generic { name, args } => {
                write!(f, "{}<", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    match arg {
                        GenericArgValue::Type(t) => write!(f, "{}", t)?,
                        GenericArgValue::Const(c) => match c {
                            ConstValueResolved::Integer(n) => write!(f, "{}", n)?,
                            ConstValueResolved::ConstParam(name) => write!(f, "{}", name)?,
                        },
                    }
                }
                write!(f, ">")
            }
            CheckerType::Tuple(types) => {
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

/// The value of a top-level `const` or `static` initialiser (N3-09, N3-10).
///
/// THREE CASES BECAUSE THE TYPE SET HAS THREE SHAPES — the integer widths all
/// evaluate as `i64`, floats as `f64`, and `bool` as itself. It is not a type:
/// `register_global` has already type-checked the expression, and this is only
/// the arithmetic.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ConstScalar {
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl ConstScalar {
    fn kind(self) -> &'static str {
        match self {
            ConstScalar::Int(_) => "integer",
            ConstScalar::Float(_) => "float",
            ConstScalar::Bool(_) => "boolean",
        }
    }
}

/// Variable information including type and mutability
#[derive(Debug, Clone)]
struct VarInfo {
    ty: CheckerType,
    mutable: bool,
}

/// Symbol table for storing variable types with scope support
#[derive(Debug, Clone)]
struct SymbolTable {
    scopes: Vec<HashMap<String, VarInfo>>,
}

impl SymbolTable {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()], // Start with global scope
        }
    }

    /// Enter a new scope
    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Exit the current scope
    fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Define a variable in the current scope
    fn define(&mut self, name: String, ty: CheckerType, mutable: bool) -> Result<()> {
        if let Some(scope) = self.scopes.last_mut() {
            if scope.contains_key(&name) {
                return Err(CompileError::Generic(format!(
                    "Variable '{}' already defined in this scope",
                    name
                )));
            }
            scope.insert(name, VarInfo { ty, mutable });
            Ok(())
        } else {
            Err(CompileError::Generic("No active scope".to_string()))
        }
    }

    /// Look up a variable (searches all scopes from innermost to outermost)
    fn lookup(&self, name: &str) -> Option<&VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }
}

/// Information about a generic function
///
/// THIS IS THE ONLY THING CODE GENERATION LEARNS ABOUT A GENERIC. It is handed
/// the template by `get_instantiations` and monomorphises from it, so a
/// property of the source function that is not a field here is a property
/// codegen cannot see. `is_async` was such a property: `monomorphize_function`
/// hardcoded `is_async: false` under the comment "monomorphized functions are
/// not async", which made that TRUE BY ERASURE — an `async fn g<T>` that was
/// instantiated emitted an ordinary synchronous `g__i64`, silently dropping the
/// keyword, and codegen's own `if concrete_func.is_async` guards could never
/// fire. The flag and the span travel with the template now so that the N7-18
/// refusal covers this ingress like the other three.
#[derive(Debug, Clone)]
pub struct GenericFunction {
    pub lifetime_params: Vec<String>,
    pub type_params: Vec<String>,
    pub params: Vec<(String, crate::ast::Type)>,
    pub return_type: Option<crate::ast::Type>,
    pub body: Vec<crate::ast::Stmt>,
    /// Whether the source declaration carried the `async` keyword.
    pub is_async: bool,
    /// The source declaration's span, so a refusal raised against a
    /// monomorphised copy can still point at code the programmer wrote rather
    /// than at the synthetic `Span::new(0, 0, 0, 0)` monomorphisation invents.
    pub span: Span,
}

/// Generic enum definition
#[derive(Debug, Clone)]
pub struct GenericEnum {
    pub lifetime_params: Vec<String>,
    pub type_params: Vec<String>,
    pub variants: Vec<(String, crate::ast::EnumVariantData)>,
}

/// Generic struct definition
#[derive(Debug, Clone)]
pub struct GenericStruct {
    pub lifetime_params: Vec<String>,
    pub type_params: Vec<String>,
    pub fields: Vec<(String, crate::ast::Type)>,
}

/// Generic type alias definition
#[derive(Debug, Clone)]
pub struct GenericTypeAlias {
    pub lifetime_params: Vec<String>,
    pub type_params: Vec<String>,
    pub ty: crate::ast::Type,
}

/// Enum variant information
#[derive(Debug, Clone)]
struct EnumVariant {
    name: String,
    fields: EnumVariantFields,
}

#[derive(Debug, Clone)]
enum EnumVariantFields {
    Unit,
    Tuple(Vec<CheckerType>),
    Named(Vec<(String, CheckerType)>),
}

/// A concrete instantiation of a generic function
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FunctionInstantiation {
    name: String,
    type_args: Vec<String>, // Concrete types like "i64", "String"
}

/// A concrete instantiation of a generic struct
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructInstantiation {
    pub name: String,
    pub type_args: Vec<String>, // Concrete types like "i64", "String"
}

/// WHERE THIS PASS DECIDES THE THINGS REVIEW KEEPS ASKING ABOUT
/// ------------------------------------------------------------
/// This file is ~4,000 lines and has exceeded three reviewers' read limits in a
/// row. The decisions that recent review turns on are these, and nothing else
/// in the file participates:
///
///   `set_imported_modules`      registration of imported items, and the three
///                               deferred refusals recorded there (async main,
///                               async value return, async value return in a
///                               TYPE-PARAMETERISED import). Public-only; the
///                               type-parameterised case goes on its own list
///                               because it is raised on a different condition.
///   `check`, opening lines      where the first two deferrals are RAISED,
///                               after the entry point is knowable. Uses
///                               `crate::ast::local_definition_shadows_import`,
///                               reports EVERY offender, and sorts before it
///                               reports because the list arrives in hash order.
///   `check`, closing lines      where the third is raised — after the body
///                               walk, because its condition is "was it
///                               INSTANTIATED", which nothing earlier knows.
///   `check`, "Check for main"   the main-existence rule the entry-point
///                               question resolves against.
///   `has_value_return`          the statement walk behind the async refusal;
///                               descends `if`/`match`/`while`/`for`/`unsafe`.
///   `check_function`, opening   the LOCAL refusals: `async fn main`, and a
///                               value-carrying return in any async function.
///                               Both are tested BEFORE its generic skip, which
///                               is why local generics need no deferral.
///
/// THE GENERIC-INSTANTIATION PATH, which the sections above depend on and which
/// is past every reviewer's read limit so far:
///
///   `generic_functions`         one map, holding IMPORTED generics (registered
///                               by `set_imported_modules`) and LOCAL ones
///                               (registered by `check`'s first pass, which
///                               therefore OVERWRITES an imported entry of the
///                               same name).
///   `check_call`                consults `generic_functions` BEFORE
///                               `functions`, so a type-parameterised import
///                               wins over an ordinary local definition of the
///                               same name — the reverse of the ordinary
///                               shadowing direction.
///   `instantiate_generic_function`
///                               materialises one, recording the key in
///                               `instantiations`.
///   `get_instantiations`        pairs every key with `generic_functions[name]`
///                               and hands the result to code generation, which
///                               monomorphises and emits it. THIS is the route
///                               by which an imported generic body reaches the
///                               output, and the reason "codegen never emits an
///                               imported generic" was false.
///
/// SHADOWING OF ORDINARY DEFINITIONS IS NOT DECIDED HERE. It is decided once, in
/// `crate::ast::local_definition_shadows_import`, which code generation calls
/// too — see src/codegen/mod.rs's imported-function and imported-prototype
/// loops. That is deliberate: the two passes asked different questions about
/// generics for a round, and a program was diagnosed against a declaration the
/// output never contained. Which body an INSTANTIATION carries is a different
/// question with a different answer, and it is decided here alone.
pub struct TypeChecker {
    /// A PUBLIC imported `async fn main`, recorded at import-registration time
    /// and raised by `check` — but only if it is the EFFECTIVE ENTRY POINT.
    /// Imported functions never reach `check_function`, where the refusal
    /// lives, so without this an imported one reached code generation and
    /// emitted a `main_Future main()` entry point.
    deferred_async_main: Option<Span>,
    /// EVERY public imported `async fn` whose body contains a value-carrying
    /// `return`, with its name, recorded because `set_imported_modules`
    /// performs no equivalent of `check_function`'s validation, so the class
    /// refused there was still declarable through an import. The name is kept
    /// because the raise has to ask whether a local definition shadows it.
    ///
    /// A `Vec`, NOT an `Option`. As an `Option` each qualifying import
    /// overwrote the previous one and `check` validated only the survivor, so
    /// two bad exports in one module whose SECOND was locally shadowed let the
    /// first through — measured. A rule that is right about the construct,
    /// applied through a container that can only hold one, is wrong about the
    /// program.
    ///
    /// THE ORDER IN THIS VEC IS NOT MEANINGFUL. It is filled by iterating
    /// `imported_modules`, a `HashMap`, so which module's offender lands first
    /// varies run to run. Only the SET is deterministic. `check` therefore
    /// sorts before it reports, and reports all of the offenders rather than
    /// returning at the first — three separate properties ("every entry",
    /// "deterministic order", "all diagnostics") that the earlier
    /// return-on-first loop delivered one of while the comment claimed the
    /// three.
    deferred_async_value_returns: Vec<(String, Span)>,
    /// The same violation in a public imported function that HAS type
    /// parameters, kept apart because it is raised on a different condition.
    ///
    /// These used to be dropped at registration time, on the ground that "code
    /// generation never emits an imported generic". That ground is false.
    /// MEASURED: `lib.pd` exporting `pub async fn agen<T>(x: T) -> i64 { return 42; }`
    /// and an app calling `agen(7)` — typeck instantiates it
    /// (`generic_functions` holds imported generics too, and the call site
    /// looks there FIRST), `get_instantiations` hands the imported body to code
    /// generation, and the emitted C contained `long long agen__i64(long long x)`
    /// beside `agen_Future v = agen__i64(7);`, which clang refused. Dropping
    /// the validation permitted exactly the body it was justified by calling
    /// unemittable.
    ///
    /// So the condition is not "is it generic" but "is it INSTANTIATED", which
    /// is knowable only after the body walk — hence a second list raised at the
    /// END of `check` rather than at its opening.
    deferred_generic_async_value_returns: Vec<(String, Span)>,
    /// EVERY public imported generic `async fn`, for the N7-18 refusal — the
    /// superset of the list above, raised after it.
    ///
    /// It is a second list rather than a widened predicate on the first because
    /// the two carry DIFFERENT WORDING for the same rule, exactly as
    /// `check_function`'s three arms do, and the more specific one must win.
    /// Merging them would either lose the value-return diagnostic (whose text
    /// `tests/conformance-manifest.txt` fingerprints for
    /// `tests/reject/async_fn.pd`) or make one list report two messages.
    ///
    /// NON-GENERIC imported async functions need no list at all: `check`'s
    /// third pass hands every public, non-generic, unshadowed imported function
    /// to `check_function`, which is where the refusal lives. Generics are
    /// skipped there — walking one raises at DECLARATION, and an uninstantiated
    /// generic is emitted by nobody — so this list exists for precisely the
    /// functions that pass cannot see.
    deferred_generic_async_imports: Vec<(String, Span)>,
    /// Function signatures
    functions: HashMap<String, CheckerType>,
    /// Generic function definitions
    generic_functions: HashMap<String, GenericFunction>,
    /// Where the template currently winning each name in `generic_functions`
    /// came from: `None` for a local definition, `Some(module)` for an import.
    ///
    /// WRITTEN IN LOCKSTEP WITH THE MAP ABOVE, AT EVERY INSERT, so it always
    /// describes the WINNER and not some earlier candidate. It exists because
    /// `generic_functions` is keyed by bare name and is last-writer-wins, so
    /// the name alone cannot say WHICH `pick<T>` codegen will monomorphize. A
    /// consumer handed only the name checked every same-named template,
    /// including ones nothing emits, and vetoed a build over an error in a body
    /// that never reached the output.
    generic_function_origin: HashMap<String, Option<String>>,
    /// Instantiated generic functions
    instantiations: HashMap<FunctionInstantiation, CheckerType>,
    /// Struct definitions
    structs: HashMap<String, Vec<(String, CheckerType)>>,
    /// Generic struct definitions
    generic_structs: HashMap<String, GenericStruct>,
    /// Trait resolver
    trait_resolver: TraitResolver,
    /// Instantiated generic structs
    struct_instantiations: HashMap<StructInstantiation, CheckerType>,
    /// Enum definitions with their variants
    enums: HashMap<String, Vec<EnumVariant>>,
    /// EVERY name declared by an `enum` item, local or imported, filled before
    /// any type is converted.
    ///
    /// `enums` above cannot answer that question while it is still being built:
    /// its entry for an enum lands only after that enum's own variants have
    /// been converted, so `enum V { Pair(V, V) }` asked about `V` and was told
    /// no, and an enum declared below its user was told no as well. Deciding
    /// `Struct` vs `Enum` from a half-filled map is how the same name got two
    /// kinds and printed one word on both sides of a mismatch.
    enum_names: HashSet<String>,
    /// Generic enum definitions
    generic_enums: HashMap<String, GenericEnum>,
    /// Type alias definitions
    type_aliases: HashMap<String, crate::ast::Type>,
    /// Generic type alias definitions
    generic_type_aliases: HashMap<String, GenericTypeAlias>,
    /// Current function return type (for checking return statements)
    current_function_return: Option<CheckerType>,
    /// Symbol table for variables
    symbols: SymbolTable,
    /// Imported modules and their exported items
    imported_modules: HashMap<String, crate::resolver::ModuleInfo>,
    /// Loop depth counter (for break/continue validation)
    loop_depth: usize,
    /// One frame per enclosing loop, innermost last — the type a `break` may
    /// carry out of it.
    ///
    /// A `break` has no label, so its target is "the innermost loop", and this
    /// stack is that rule made walkable. A frame is `BreakTarget::Value` only
    /// for a `loop` in VALUE position; every other loop pushes
    /// `BreakTarget::Statement`, which is what lets `break <expr>;` inside a
    /// nested `while` be refused instead of silently assigning the outer
    /// loop's temporary.
    break_targets: Vec<BreakTarget>,
    /// Error helper for better suggestions
    error_helper: TypeErrorHelper,
    /// Unsafe block depth counter (for tracking unsafe context)
    unsafe_depth: usize,
    /// Current impl type (for resolving Self types)
    current_impl_type: Option<String>,
    /// The receiver form of the method being checked, if any.
    ///
    /// `None` outside a method or in an associated function. Needed because `self`
    /// became a PLACE BASE in this round: `self.n = v` now parses, and whether it may
    /// be WRITTEN is a property of how the receiver was declared, which nothing
    /// downstream of the signature otherwise knows.
    current_self_receiver: Option<SelfReceiver>,
    /// The receiver form of every method, by qualified name, filled in the FIRST pass.
    ///
    /// `current_self_receiver` says how the CALLER declared its own receiver; this says
    /// how the CALLEE declared its. The write rule needs both, and it needs the callee's
    /// before that callee's body is walked -- a method may call one declared after it, so
    /// reading the form off the method being checked would make the rule order-dependent.
    impl_method_receiver: HashMap<String, SelfReceiver>,
    /// Every top-level `const` or `static` found in an IMPORTED module, with
    /// the module it came from. Recorded here and raised by `check`, because
    /// `set_imported_modules` has no fallible signature — the same shape the
    /// async deferrals above use, and for the same reason.
    deferred_imported_globals: Vec<(String, String)>,
    /// The top-level `const` and `static` names (N3-09, N3-10), with the span
    /// of the declaration.
    ///
    /// SEPARATE FROM THE SYMBOL TABLE, which is where their TYPES live. The
    /// symbol table's outermost scope holds them so that every function body
    /// resolves the name without a second lookup path; this map is what makes
    /// "is this name a top-level item" answerable, which the symbol table
    /// cannot answer — a scope is a scope, and by the time a body is being
    /// checked its own bindings are in there too.
    global_items: HashMap<String, Span>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        // Built-in functions come from the single source of truth in
        // src/builtins.rs so that this pass and the borrow checker can never
        // drift apart (see crate::builtins for the regression this prevents).
        let functions: HashMap<String, CheckerType> = crate::builtins::BUILTINS
            .iter()
            .map(|b| {
                let params = b.params.iter().map(|p| builtin_type(p.ty)).collect();
                (
                    b.name.to_string(),
                    CheckerType::Function(params, Box::new(builtin_type(b.ret))),
                )
            })
            .collect();

        Self {
            deferred_async_main: None,
            deferred_async_value_returns: Vec::new(),
            deferred_generic_async_value_returns: Vec::new(),
            deferred_generic_async_imports: Vec::new(),
            functions,
            generic_functions: HashMap::new(),
            generic_function_origin: HashMap::new(),
            instantiations: HashMap::new(),
            structs: HashMap::new(),
            generic_structs: HashMap::new(),
            trait_resolver: TraitResolver::new(),
            struct_instantiations: HashMap::new(),
            enums: HashMap::new(),
            enum_names: HashSet::new(),
            generic_enums: HashMap::new(),
            type_aliases: HashMap::new(),
            generic_type_aliases: HashMap::new(),
            current_function_return: None,
            symbols: SymbolTable::new(),
            imported_modules: HashMap::new(),
            loop_depth: 0,
            break_targets: Vec::new(),
            error_helper: TypeErrorHelper::new(),
            unsafe_depth: 0,
            current_impl_type: None,
            current_self_receiver: None,
            impl_method_receiver: HashMap::new(),
            global_items: HashMap::new(),
            deferred_imported_globals: Vec::new(),
        }
    }

    /// Names of every function signature this pass knows.
    ///
    /// On a freshly constructed checker this is exactly the built-in set, which is
    /// what the drift test in `crate::builtins` asserts against.
    #[allow(dead_code)] // used by the drift tests in crate::builtins
    pub(crate) fn registered_function_names(&self) -> std::collections::BTreeSet<String> {
        self.functions.keys().cloned().collect()
    }

    /// Set imported modules for type checking
    ///
    /// Modules are visited in SORTED key order, not `HashMap` order — the seventh
    /// site of the class the rest of this branch fixes, and the only one where the
    /// hash seed reaches the program's ANSWER rather than only the order of the
    /// emitted C.
    ///
    /// Every insert below is under the BARE name as well as the qualified one
    /// (`src/typeck/mod.rs:1640-1641`, `src/typeck/mod.rs:1643-1645`,
    /// `src/typeck/mod.rs:1740-1741`), and the map is last-writer-wins. So when two
    /// imported modules export the same name, iteration order decides which
    /// signature — and, for a generic, which BODY — survives. `get_instantiations`
    /// reads `generic_functions` by bare name and hands the winner to codegen's
    /// monomorphizer, so with
    ///
    /// ```text
    /// liba.pd:  pub fn pick<T>(v: T) -> i64 { return 111; }
    /// libb.pd:  pub fn pick<T>(v: T) -> i64 { return 222; }
    /// ```
    ///
    /// imported together, twenty compiles of one unchanged program printed 111 ten
    /// times and 222 ten times. Sorting does not make the choice CORRECT — two
    /// modules exporting one name is a real ambiguity nothing diagnoses, and
    /// `test_ambiguous_import_is_diagnosed_by_the_compiler_not_by_gcc` declares
    /// that — but it makes it the same wrong answer every run, which is the
    /// precondition for anyone noticing it is wrong.
    /// Rewrite the `Struct(name)` leaves of an already-converted type to
    /// `Enum(name)` wherever `name` is known to be an enum.
    ///
    /// The repair for `CheckerType::from` applied where that conversion is used
    /// on an import and the context it lacks is at hand. A free function over
    /// the set rather than a method, so it can be called while the loop that
    /// needs it holds a borrow of `self.imported_modules` and writes to
    /// `self.functions` — three disjoint fields, which is the only reason this
    /// does not have to clone every imported module's AST in order to run.
    ///
    /// Every other leaf comes back unchanged, so a program that imports no enum
    /// is converted exactly as it was.
    /// WHICH NAMES ARE `enum`S — the one answer, with precedence applied.
    ///
    /// `ast_type_to_checker_type` asks this of every named type, so getting it
    /// wrong misclassifies an ordinary declaration. It used to be a bare UNION
    /// of local and imported enum names, which has no way to express either of
    /// the two rules the rest of the compiler already follows:
    ///
    ///   VISIBILITY. A module's `enum` reaches a downstream program only if it
    ///   said `pub`. Until 2026-08-23 `EnumDef` carried no visibility at all and
    ///   `src/parser/mod.rs` discarded the `pub` it had parsed, so every enum in
    ///   every module was reachable from every program that imported it.
    ///
    ///   SHADOWING. A local declaration wins over an imported one. `self.structs`
    ///   and `self.enums` already get this from insertion order — imports are
    ///   registered in `set_imported_modules`, locals in `check`, last writer
    ///   wins — and the union got it from nowhere, so
    ///   `struct Color { v: i64 }` over an imported `pub enum Color` came back
    ///   `Enum("Color")` and was refused with
    ///   `Type mismatch: expected Color, found Color`.
    ///
    /// The shadowing test is `crate::ast::local_type_shadows_import`, which is
    /// where the FUNCTION-namespace version of the same rule already lives. It
    /// is not re-derived here: a second notion of "which `Color` is this" is the
    /// one-fact-two-representations mistake this file keeps paying for.
    ///
    /// Order is imports first, locals second, matching the two maps above, so
    /// this reads the same way they do even though a set cannot show it.
    fn enum_names_in_scope(
        program: &Program,
        imported: &HashMap<String, crate::resolver::ModuleInfo>,
    ) -> HashSet<String> {
        let mut names = HashSet::new();
        for module_info in imported.values() {
            for item in &module_info.ast.items {
                if let Item::Enum(enum_def) = item {
                    if matches!(enum_def.visibility, crate::ast::Visibility::Public)
                        && !crate::ast::local_type_shadows_import(program, &enum_def.name)
                    {
                        names.insert(enum_def.name.clone());
                    }
                }
            }
        }
        for item in &program.items {
            if let Item::Enum(enum_def) = item {
                names.insert(enum_def.name.clone());
            }
        }
        names
    }

    /// Un-register every imported type whose name a LOCAL declaration has taken.
    ///
    /// `enum_names_in_scope` decides the KIND, and it is not enough on its own:
    /// `set_imported_modules` runs before `check` and has already written the
    /// imported enum's variants into `self.enums` and its variant constructors
    /// into `self.functions`, under the bare name. `ast_type_to_checker_type`
    /// consults `self.enums` BESIDE `enum_names`, so leaving those entries in
    /// place reproduces the whole defect through the other half of the `||`.
    ///
    /// This is where the precedence is applied, rather than in
    /// `set_imported_modules`, because that method has the modules and not the
    /// program: nothing there can know a local declaration is coming. `check`
    /// is the first point that holds both.
    ///
    /// A local `enum` of the same name needs nothing here — it overwrites the
    /// entry itself, a few lines further on, which is the same last-writer rule.
    ///
    /// THE FOURTH SITE THAT ASKS THE SHADOWING QUESTION, and the one that does
    /// not ask it through `local_type_shadows_import`. Named here beside its
    /// three siblings — `enum_names_in_scope`, code generation's emission walk,
    /// and `LayoutItems::of` — because a predicate consulted wherever somebody
    /// remembered to consult it is the exact shape this branch spent its budget
    /// removing, and an unnamed fourth is how a fifth gets written.
    ///
    /// It is deliberate and it is NARROWER: the shared predicate answers "has a
    /// local declaration taken this name", and the `.any(...)` below asks the
    /// different question "is the local declaration that took it an `enum`",
    /// which the shared one cannot express. A local `enum` must NOT have its
    /// imported twin's registration dropped, because the local registration
    /// overwrites it a few lines on and dropping it would leave the name with
    /// nothing. Widening the shared predicate to carry the kind would give three
    /// callers a distinction only this one uses.
    ///
    /// KNOWN FRAGILITY IN THE CONSTRUCTOR FILTER BELOW, recorded rather than
    /// fixed: it keys on the STRING `"<name>::"` / `"::<name>::"`, so a module
    /// whose name equals a shadowed type's name loses every qualified function
    /// it exports — `import Color; struct Color {...}` would drop
    /// `Color::helper`. There is no witness for it (module and type names have
    /// not collided in any tracked program) and closing it means keying the
    /// function table by a structured name rather than a string, which is
    /// `docs/contributing/citation-and-predicate-debt.md`'s to own, not this
    /// method's.
    fn drop_imports_shadowed_by_local_types(&mut self, program: &Program) {
        let taken: Vec<String> = self
            .imported_modules
            .values()
            .flat_map(|m| &m.ast.items)
            .filter_map(|item| match item {
                Item::Enum(enum_def)
                    if crate::ast::local_type_shadows_import(program, &enum_def.name)
                        && !program.items.iter().any(
                            |local| matches!(local, Item::Enum(e) if e.name == enum_def.name),
                        ) =>
                {
                    Some(enum_def.name.clone())
                }
                _ => None,
            })
            .collect();

        for name in taken {
            self.enums.remove(&name);
            // The variant constructors too. `Color::Red` resolving to an
            // imported enum's constructor while `Color` is a local `struct` is
            // the same misclassification wearing a call. Both spellings the
            // registration writes are removed: the bare `Color::Red` and the
            // qualified `lib::Color::Red`.
            let bare = format!("{}::", name);
            let qualified = format!("::{}::", name);
            self.functions
                .retain(|key, _| !key.starts_with(&bare) && !key.contains(&qualified));
        }
    }

    fn as_enums_where_known(enum_names: &HashSet<String>, ty: CheckerType) -> CheckerType {
        match ty {
            CheckerType::Struct(name) if enum_names.contains(&name) => CheckerType::Enum(name),
            CheckerType::Array(elem, size) => CheckerType::Array(
                Box::new(Self::as_enums_where_known(enum_names, *elem)),
                size,
            ),
            CheckerType::Tuple(types) => CheckerType::Tuple(
                types
                    .into_iter()
                    .map(|t| Self::as_enums_where_known(enum_names, t))
                    .collect(),
            ),
            CheckerType::Function(params, ret) => CheckerType::Function(
                params
                    .into_iter()
                    .map(|t| Self::as_enums_where_known(enum_names, t))
                    .collect(),
                Box::new(Self::as_enums_where_known(enum_names, *ret)),
            ),
            CheckerType::Generic { name, args } => CheckerType::Generic {
                name,
                args: args
                    .into_iter()
                    .map(|arg| match arg {
                        GenericArgValue::Type(t) => {
                            GenericArgValue::Type(Self::as_enums_where_known(enum_names, t))
                        }
                        other => other,
                    })
                    .collect(),
            },
            other => other,
        }
    }

    pub fn set_imported_modules(&mut self, modules: HashMap<String, crate::resolver::ModuleInfo>) {
        self.imported_modules = modules;

        // Enum names first, for the same reason `check` collects them first:
        // the registration below converts imported field, parameter, return and
        // payload types, and an imported enum in any of those positions was
        // `Struct`. Local enums are absent here and that is correct — an
        // imported module cannot name a type from the program importing it.
        //
        // COLLECTING THE SET IS HALF A FIX. This comment used to stop at the
        // paragraph above while every conversion below still went through
        // `CheckerType::from`, whose `Custom` arm is an associated function that
        // can consult no set at all and answers `Struct` for every named type.
        // The set was gathered and then never read, so the comment described
        // work nobody had done. Measured:
        //
        // ```text
        // lib9.pd:  pub enum Color { Red, Green }
        //           pub fn red() -> Color { return Color::Red; }
        //           pub fn kind(c: Color) -> i64 { return 7; }
        // main.pd:  import lib9;
        //           fn main() { let c: Color = red(); print_int(kind(c)); }
        // ```
        //
        // was refused with `Type mismatch: expected Color, found Color` — the
        // same diagnostic naming one type on both sides that this branch removed
        // for local declarations, reached one container over. `let c: Color` is
        // converted by the context-aware path and is an `Enum`; `red`'s return
        // type was registered here and was a `Struct`.
        //
        // Every conversion below is wrapped in `as_enums_where_known`, which
        // rewrites exactly the `Struct(name)` leaves whose name is in this set
        // and touches nothing else. Deliberately NOT `ast_type_to_checker_type`:
        // that one also resolves type aliases out of a map THIS LOOP IS STILL
        // FILLING, so it would make an imported signature depend on which module
        // was processed first — trading a wrong answer for an unstable one.
        //
        // HALF THE RULE, ON PURPOSE, AND THE OTHER HALF IS IN `check`. Visibility
        // can be applied here because it is a property of the import alone.
        // Shadowing cannot: this method has the modules and not the program, so
        // nothing here can know a local declaration is coming. `check` recomputes
        // the whole set through `enum_names_in_scope` with both rules applied and
        // calls `drop_imports_shadowed_by_local_types` to undo what was registered
        // under a name a local declaration took. What this loop is FOR is the
        // conversions immediately below, which run before `check` exists.
        //
        // `.values()` unsorted is safe here and only here: the result is a SET, so
        // module order cannot reach it. The two `RecursiveLayout::analyze` calls
        // are sorted because their results are ordered.
        for module_info in self.imported_modules.values() {
            for item in &module_info.ast.items {
                if let crate::ast::Item::Enum(enum_def) = item {
                    if matches!(enum_def.visibility, crate::ast::Visibility::Public) {
                        self.enum_names.insert(enum_def.name.clone());
                    }
                }
            }
        }

        // Process imported functions and add them to our function table
        let mut sorted_modules: Vec<_> = self.imported_modules.iter().collect();
        sorted_modules.sort_by_key(|(name, _)| *name);
        for (module_name, module_info) in sorted_modules {
            // For now, process all exported functions from the module
            for item in &module_info.ast.items {
                match item {
                    crate::ast::Item::Function(func) => {
                        // Only process exported (public) functions
                        if matches!(func.visibility, crate::ast::Visibility::Public) {
                            // WHAT AN IMPORT CAN SMUGGLE PAST `check_function`,
                            // which only local functions reach. Both are
                            // RECORDED rather than returned, because this setter
                            // has no fallible signature; `check` raises them.
                            //
                            // INSIDE the visibility condition, deliberately. A
                            // PRIVATE imported function is never registered, so
                            // it can never be called and never becomes the entry
                            // point — refusing it rejected a valid program.
                            // Measured at fbcfc39: a private imported
                            // `async fn main` killed compilation of a program
                            // whose own `main` was perfectly good.
                            // A TYPE-PARAMETERISED IMPORT IS NOT EXEMPT, IT IS
                            // CONDITIONAL. It is skipped by codegen's
                            // imported-function loop, which requires
                            // `type_params.is_empty()` — but that is not the
                            // only route into the output. `generic_functions`
                            // below holds imported generics, the call site
                            // consults it BEFORE `functions`, and every
                            // instantiation reaches codegen through
                            // `get_instantiations`. Measured: an imported
                            // `pub async fn agen<T>` that is called emits
                            // `agen__i64` and does not compile.
                            //
                            // So the offender is recorded on a SEPARATE list and
                            // raised at the end of `check`, when it is known
                            // whether it was instantiated. `async fn main` needs
                            // no such list: an entry point is never called, so a
                            // type-parameterised one is never instantiated and
                            // never emitted.
                            if !func.type_params.is_empty() {
                                if func.is_async
                                    && func.name != "main"
                                    && Self::has_value_return(&func.body)
                                {
                                    self.deferred_generic_async_value_returns
                                        .push((func.name.clone(), func.span));
                                }
                                // N7-18, the superset of the line above and
                                // recorded unconditionally on `is_async`. Same
                                // `name != "main"` exemption for the same
                                // reason: a generic `main` is not an entry
                                // point, so it is never called, never
                                // instantiated and never emitted.
                                if func.is_async && func.name != "main" {
                                    self.deferred_generic_async_imports
                                        .push((func.name.clone(), func.span));
                                }
                            } else if func.is_async && func.name == "main" {
                                self.deferred_async_main = Some(func.span);
                            } else if func.is_async && Self::has_value_return(&func.body) {
                                self.deferred_async_value_returns
                                    .push((func.name.clone(), func.span));
                            }
                            let qualified_name = format!("{}::{}", module_name, func.name);

                            if !func.type_params.is_empty() {
                                // Generic function
                                let generic_func = GenericFunction {
                                    lifetime_params: func.lifetime_params.clone(),
                                    type_params: func.type_params.clone(),
                                    params: func
                                        .params
                                        .iter()
                                        .map(|p| (p.name.clone(), p.ty.clone()))
                                        .collect(),
                                    return_type: func.return_type.clone(),
                                    body: func.body.clone(),
                                    is_async: func.is_async,
                                    span: func.span,
                                };
                                self.generic_functions
                                    .insert(func.name.clone(), generic_func);
                                // Lockstep with the insert above: this import is
                                // now the winner for that bare name.
                                self.generic_function_origin
                                    .insert(func.name.clone(), Some(module_name.clone()));
                            } else {
                                // Regular function
                                let param_types: Vec<CheckerType> = func
                                    .params
                                    .iter()
                                    .map(|param| {
                                        Self::as_enums_where_known(
                                            &self.enum_names,
                                            CheckerType::from(&param.ty),
                                        )
                                    })
                                    .collect();

                                let return_type = func
                                    .return_type
                                    .as_ref()
                                    .map(|ty| {
                                        Self::as_enums_where_known(
                                            &self.enum_names,
                                            CheckerType::from(ty),
                                        )
                                    })
                                    .unwrap_or(CheckerType::Unit);

                                let func_type =
                                    CheckerType::Function(param_types, Box::new(return_type));

                                // Add both qualified and unqualified names
                                // Note: In a full implementation, we'd use a proper module resolution system
                                self.functions.insert(func.name.clone(), func_type.clone());
                                self.functions.insert(qualified_name, func_type);
                            }
                        }
                    }
                    crate::ast::Item::Struct(struct_def) => {
                        if matches!(struct_def.visibility, crate::ast::Visibility::Public) {
                            // Convert field types to CheckerType
                            let fields: Vec<(String, CheckerType)> = struct_def
                                .fields
                                .iter()
                                .map(|(name, ty)| {
                                    (
                                        name.clone(),
                                        Self::as_enums_where_known(
                                            &self.enum_names,
                                            CheckerType::from(ty),
                                        ),
                                    )
                                })
                                .collect();

                            // Add both qualified and unqualified names
                            self.structs.insert(struct_def.name.clone(), fields.clone());
                            self.structs
                                .insert(format!("{}::{}", module_name, struct_def.name), fields);
                        }
                    }
                    crate::ast::Item::Enum(enum_def) => {
                        // TESTED LIKE EVERY OTHER ARM. This said "Assume all
                        // exported enums are public" and assumed it because it
                        // had to: `EnumDef` carried no visibility field and the
                        // parser dropped the `pub` it had parsed
                        // (`src/parser/mod.rs`), so there was nothing here to
                        // test. Both are fixed, so a module's `enum` reaches a
                        // downstream program only if it said `pub`, the same as
                        // its `struct` three arms up.
                        //
                        // Without this the refusal for a private import landed
                        // in gcc rather than in a diagnostic: the type checker
                        // had stopped calling the name an enum, and the variant
                        // constructor registered here still resolved.
                        if matches!(enum_def.visibility, crate::ast::Visibility::Public) {
                            // Store enum type information
                            let enum_type = CheckerType::Enum(enum_def.name.clone());

                            // THE ENUM'S OWN SHAPE, not only its constructors.
                            //
                            // The loop below registers `Color::Red` as a
                            // FUNCTION, which is what an expression in call
                            // position needs. Two other questions are asked of
                            // `self.enums` instead — what variants does this
                            // enum have (the `Enum::Variant` path expression and
                            // the `match` pattern both look the variant up
                            // there) and is the match exhaustive — and nothing
                            // put an imported enum into that map, so
                            //
                            // ```text
                            // error: Undefined enum type: Color
                            // ```
                            //
                            // was every program that named one. That was the
                            // break BEHIND the conversion break above: with the
                            // kinds fixed and this map still empty, no program
                            // could reach the fix, so the fix would have had no
                            // witness. Both are closed together and
                            // `tests/m3_imported_calls.rs` runs the whole chain.
                            //
                            // Shape-for-shape the mirror of the local
                            // registration in `check`, with
                            // `as_enums_where_known` where that one uses
                            // `ast_type_to_checker_type`, for the reason given
                            // at the top of this method. Last writer wins, and
                            // `check` runs after this, so a local enum of the
                            // same name displaces the imported one — the same
                            // direction shadowing takes everywhere else here.
                            let variants: Vec<EnumVariant> = enum_def
                                .variants
                                .iter()
                                .map(|variant| EnumVariant {
                                    name: variant.name.clone(),
                                    fields: match &variant.data {
                                        crate::ast::EnumVariantData::Unit => {
                                            EnumVariantFields::Unit
                                        }
                                        crate::ast::EnumVariantData::Tuple(types) => {
                                            EnumVariantFields::Tuple(
                                                types
                                                    .iter()
                                                    .map(|ty| {
                                                        Self::as_enums_where_known(
                                                            &self.enum_names,
                                                            CheckerType::from(ty),
                                                        )
                                                    })
                                                    .collect(),
                                            )
                                        }
                                        crate::ast::EnumVariantData::Struct(fields) => {
                                            EnumVariantFields::Named(
                                                fields
                                                    .iter()
                                                    .map(|(n, ty)| {
                                                        (
                                                            n.clone(),
                                                            Self::as_enums_where_known(
                                                                &self.enum_names,
                                                                CheckerType::from(ty),
                                                            ),
                                                        )
                                                    })
                                                    .collect(),
                                            )
                                        }
                                    },
                                })
                                .collect();
                            // Bare AND qualified, like the struct and function
                            // arms above. Registering only the bare name left a
                            // qualified `lib::Color` unresolvable where a
                            // qualified `lib::Point` resolves.
                            self.enums.insert(
                                format!("{}::{}", module_name, enum_def.name),
                                variants.clone(),
                            );
                            self.enums.insert(enum_def.name.clone(), variants);

                            // Add variant constructors as functions
                            for variant in &enum_def.variants {
                                let variant_name = format!("{}::{}", enum_def.name, variant.name);
                                let qualified_variant =
                                    format!("{}::{}", module_name, variant_name);

                                // Create constructor function type based on variant fields
                                let func_type = match &variant.data {
                                    crate::ast::EnumVariantData::Unit => {
                                        // Unit variant: no parameters, returns enum type
                                        CheckerType::Function(vec![], Box::new(enum_type.clone()))
                                    }
                                    crate::ast::EnumVariantData::Tuple(types) => {
                                        // Tuple variant: parameters from tuple fields
                                        let param_types: Vec<CheckerType> = types
                                            .iter()
                                            .map(|ty| {
                                                Self::as_enums_where_known(
                                                    &self.enum_names,
                                                    CheckerType::from(ty),
                                                )
                                            })
                                            .collect();
                                        CheckerType::Function(
                                            param_types,
                                            Box::new(enum_type.clone()),
                                        )
                                    }
                                    crate::ast::EnumVariantData::Struct(fields) => {
                                        // Named variant: parameters from named fields
                                        let param_types: Vec<CheckerType> = fields
                                            .iter()
                                            .map(|(_, ty)| {
                                                Self::as_enums_where_known(
                                                    &self.enum_names,
                                                    CheckerType::from(ty),
                                                )
                                            })
                                            .collect();
                                        CheckerType::Function(
                                            param_types,
                                            Box::new(enum_type.clone()),
                                        )
                                    }
                                };

                                // Register variant constructors
                                self.functions
                                    .insert(variant_name.clone(), func_type.clone());
                                self.functions.insert(qualified_variant, func_type);
                            }
                        }
                    }
                    crate::ast::Item::Trait(trait_def) => {
                        if matches!(trait_def.visibility, crate::ast::Visibility::Public) {
                            // Store trait information
                            // TODO: Implement trait tracking
                        }
                    }
                    crate::ast::Item::Impl(_) => {
                        // Impl blocks are processed separately
                    }
                    crate::ast::Item::TypeAlias(type_alias) => {
                        if matches!(type_alias.visibility, crate::ast::Visibility::Public) {
                            // Store type alias information
                            let qualified_name = format!("{}::{}", module_name, type_alias.name);

                            if !type_alias.type_params.is_empty() {
                                // Generic type alias
                                let generic_alias = GenericTypeAlias {
                                    lifetime_params: type_alias.lifetime_params.clone(),
                                    type_params: type_alias.type_params.clone(),
                                    ty: type_alias.ty.clone(),
                                };
                                self.generic_type_aliases
                                    .insert(type_alias.name.clone(), generic_alias.clone());
                                self.generic_type_aliases
                                    .insert(qualified_name, generic_alias);
                            } else {
                                // Regular type alias
                                self.type_aliases
                                    .insert(type_alias.name.clone(), type_alias.ty.clone());
                                self.type_aliases
                                    .insert(qualified_name, type_alias.ty.clone());
                            }
                        }
                    }
                    crate::ast::Item::Macro(_) => {
                        // Macros are handled during expansion phase, skip here
                    }
                    crate::ast::Item::Global(global) => {
                        // NOT REGISTERED, AND NO LONGER SILENT ABOUT IT. Code
                        // generation emits a top-level item only for the program
                        // being compiled, so registering an imported `const`
                        // here would type a name that no C definition backs and
                        // move the failure to the linker.
                        //
                        // Leaving it unregistered was the first half of the fix
                        // and the second half was missing: the module's OWN
                        // functions are type-checked here (the third pass exists
                        // precisely so that an imported body is not accepted
                        // unchecked), and they read their own items. Measured:
                        // a module with `const LIMIT: i64 = 10;` and
                        // `pub fn cap() -> i64 { return LIMIT; }` fails the
                        // IMPORTER'S compile with "Undefined variable or
                        // function: 'LIMIT'" — a name from a file the author of
                        // the failing program may never have opened, and the
                        // same message whether or not the item said `pub`.
                        //
                        // Recorded and raised by name instead.
                        self.deferred_imported_globals
                            .push((module_name.clone(), global.name.clone()));
                    }
                }
            }
        }
    }

    /// Register a top-level `const` or `static` (N3-09, N3-10) and check it.
    ///
    /// IT GOES IN THE OUTERMOST SCOPE, which is the one `check_function` never
    /// leaves and never pops, so every function body sees the name whether it is
    /// written above or below the item. Order independence is not a special rule
    /// here; it is what registering in the FIRST pass means, and it is the same
    /// treatment functions already get.
    ///
    /// THE TYPE SET IS CLOSED, and small on purpose. A `String` is a pointer
    /// into a runtime arena that no file-scope initialiser can produce, and an
    /// array, struct, enum or tuple would need its C aggregate emitted before
    /// the definition and its layout decided by a pass that has not run yet.
    /// Each of those is refused by name rather than left to fail inside gcc.
    fn register_global(&mut self, global: &crate::ast::GlobalDef) -> Result<()> {
        let noun = match global.kind {
            crate::ast::GlobalKind::Const => "const",
            crate::ast::GlobalKind::Static { .. } => "static",
        };

        // D4. `pub` ON A TOP-LEVEL ITEM PROMISES SOMETHING NOTHING DELIVERS.
        // The resolver does not export a `const` or `static` and code generation
        // emits a definition only for the program being compiled, so an importer
        // that writes the name gets "Undefined variable" at the use site — a
        // diagnostic about the USER'S spelling for a visibility decision the
        // language made. Refused at the declaration, where the word is written,
        // until cross-module items exist (N11).
        if matches!(global.visibility, crate::ast::Visibility::Public) {
            return Err(CompileError::Generic(format!(
                "`pub` on a top-level `{}` is not implemented: nothing exports `{}` and nothing \
                 emits a definition for an imported one, so the keyword would promise a \
                 visibility that does not exist. Drop `pub` — the item is visible to every \
                 function in its own file either way",
                noun, global.name
            )));
        }

        if !matches!(
            global.ty,
            crate::ast::Type::I32
                | crate::ast::Type::I64
                | crate::ast::Type::U32
                | crate::ast::Type::U64
                | crate::ast::Type::F32
                | crate::ast::Type::F64
                | crate::ast::Type::Bool
        ) {
            return Err(CompileError::Generic(format!(
                "a top-level `{}` may only have a numeric or `bool` type, and `{}` has type \
                 `{}`: every other type is built by code that runs, and nothing runs before \
                 `main`",
                noun, global.name, global.ty
            )));
        }

        // A BUILT-IN IS ASKED ABOUT FIRST, because `self.functions` is SEEDED
        // with the built-in registry (see `TypeChecker::new`) — so the check
        // below fired on `const print_int: i64 = 3;` and reported that the name
        // "is declared as a top-level `const` and as a function", naming a
        // function the program does not contain. The refusal was right and the
        // reason was fiction.
        self.refuse_builtin_definition(&global.name, &format!("a top-level `{}`", noun), false)?;

        // THE OTHER DIRECTION OF THE ONE-NAMESPACE RULE: a `const` written after
        // the `fn` it collides with. Both orders end in the same C, so both are
        // refused here rather than one of them being left to the linker.
        if self.functions.contains_key(&global.name)
            || self.generic_functions.contains_key(&global.name)
        {
            return Err(CompileError::Generic(format!(
                "`{}` is declared as a top-level `{}` and as a function, and a program has one \
                 namespace for both: the emitted C would define the name twice",
                global.name, noun
            )));
        }
        if self.type_aliases.contains_key(&global.name)
            || self.structs.contains_key(&global.name)
            || self.enums.contains_key(&global.name)
        {
            return Err(CompileError::Generic(format!(
                "`{}` is declared as a top-level `{}` and as a type, and a program has one \
                 namespace for both",
                global.name, noun
            )));
        }

        // A second item under the same name is not a redefinition question the
        // symbol table can answer alone — its `define` reports "Variable 'X'
        // already defined in this scope", which names neither item.
        if let Some(previous) = self.global_items.get(&global.name) {
            return Err(CompileError::Generic(format!(
                "`{}` is declared twice at the top level (the first is at line {})",
                global.name, previous.line
            )));
        }

        let declared = self.ast_type_to_checker_type(&global.ty);
        let found = self.check_expression(&global.value)?;
        if found != declared {
            return Err(CompileError::Generic(format!(
                "the initialiser of `{}` has type `{}`, and it is declared `{}`",
                global.name, found, declared
            )));
        }

        // The initialiser must HAVE a value, not merely have a legal shape.
        self.const_eval(&global.value, &global.name)?;

        let mutable = matches!(
            global.kind,
            crate::ast::GlobalKind::Static { is_mut: true }
        );
        self.symbols.define(global.name.clone(), declared, mutable)?;
        self.global_items.insert(global.name.clone(), global.span);
        Ok(())
    }

    /// Refuse a local binding that reuses a top-level item's name.
    ///
    /// C would accept the shadow and so would this checker's scope stack, and
    /// both would be right about their own rules — which is the problem. A
    /// reader of `X = 1;` inside a function has to know whether a `let X` ran
    /// earlier in the same body to know whether the program's `static mut X`
    /// changed. One name, one meaning, refused at the binding rather than
    /// silently resolved at the use.
    fn refuse_global_shadow(&self, name: &str, what: &str) -> Result<()> {
        self.refuse_builtin_shadow(name, what)?;
        match self.global_items.get(name) {
            Some(span) => Err(CompileError::Generic(format!(
                "{} `{}` has the name of the top-level item declared at line {}: \
                 a local binding may not shadow it",
                what, name, span.line
            ))),
            None => Ok(()),
        }
    }

    /// Refuse a top-level declaration that reuses a BUILT-IN name (N14-02).
    ///
    /// `global_items` holds what the program declares, and a built-in is not
    /// declared by the program — so neither `refuse_global_collision` nor
    /// `refuse_global_shadow` could see one, and every position below was
    /// silently accepted. The two reasons are different and the message says
    /// which one applies, because `callable` decides whether anything actually
    /// breaks:
    ///
    /// * a FUNCTION. MEASURED on `fn print_int(x: i64) -> i64 { return x; }`
    ///   beside `print_int(7)`: the program compiled, exit 0, and printed `7`.
    ///   The emitted C held BOTH `long long print_int(long long x)` and, in
    ///   `main`, `__pd_print_int(7LL)` — the built-in. The definition is
    ///   reachable from nowhere; the call the author wrote went somewhere else.
    /// * a TYPE or a top-level value. Nothing breaks in C — a built-in mangles
    ///   to `__pd_<name>`, so `struct print_int` and `void __pd_print_int(…)`
    ///   coexist, and `struct print_int { n: i64 }` compiled and ran. The
    ///   refusal is the language's one-namespace rule, the same one that
    ///   already refuses a `const` beside a `fn`, and the message says so
    ///   rather than inventing a collision.
    fn refuse_builtin_definition(&self, name: &str, what: &str, callable: bool) -> Result<()> {
        if !crate::builtins::is_builtin(name) {
            return Ok(());
        }
        let reason = if callable {
            "every call to it resolves to the built-in, so this definition is C that \
             nothing can reach"
        } else {
            "nothing collides in the emitted C — a built-in mangles to `__pd_<name>` — \
             but this language has ONE namespace for top-level names, so a reader of \
             the name could not tell which of the two it is"
        };
        Err(CompileError::Generic(format!(
            "`{}` is a built-in, and a program may not define or shadow one: {} is \
             declared under that name and {}. Rename it — the built-in cannot be \
             replaced, only hidden from the reader",
            name, what, reason
        )))
    }

    /// Refuse a LOCAL binder that reuses a built-in name (N14-02).
    ///
    /// The sibling of the above, one scope in, and the sharper of the two.
    /// MEASURED on `fn f(print_int: i64) -> i64 { print_int(3); return print_int; }`:
    /// it compiled, ran, and emitted `__pd_print_int(3LL);` beside
    /// `return print_int;` — one name meaning the BUILT-IN where a call is
    /// written and the BINDING where a value is wanted, in adjacent lines, with
    /// no diagnostic. A local shadowing a top-level item is already refused for
    /// this reason; a built-in is the same question with the declaration off
    /// the page.
    fn refuse_builtin_shadow(&self, name: &str, what: &str) -> Result<()> {
        if !crate::builtins::is_builtin(name) {
            return Ok(());
        }
        Err(CompileError::Generic(format!(
            "{} `{}` has the name of a built-in, and a local binding may not shadow \
             one: in this scope the name would mean the binding where a value is \
             wanted and the built-in where a call is written. Rename the binding",
            what, name
        )))
    }

    /// The value of a top-level initialiser, computed rather than assumed.
    ///
    /// WHY THIS EXISTS AT ALL: the initialiser rule was SYNTACTIC. It said "a
    /// literal, or operators over literals", which `1 / 0` satisfies — so
    /// `const X: i64 = 1 / 0;` passed every check this compiler had and reached
    /// the C compiler as `static const long long X = (1 / 0);`, measured as
    /// "initializer element is not a compile-time constant". The shape was legal
    /// and the VALUE did not exist. Same for `9223372036854775807 + 1`, which
    /// compiled, linked, ran and printed `-9223372036854775808`: C signed
    /// overflow is undefined behaviour, so that number is not a wrong answer,
    /// it is an arbitrary one.
    ///
    /// THE LIST IS NOT WIDENED BY EVALUATING IT. Every form here is one the
    /// parser already accepted (`validate_global_initializer`); this decides
    /// whether the form HAS a value, and refuses by name when it does not.
    /// Nothing new becomes writable.
    ///
    /// AGREEMENT WITH C IS THE POINT, and it is why the refusals are exactly
    /// these three: every accepted integer expression stays inside `i64`, so C's
    /// `long long` arithmetic computes what this did; a zero divisor and a shift
    /// outside 0..63 are undefined in C, so no answer here could be checked
    /// against one there.
    fn const_eval(&self, expr: &Expr, name: &str) -> Result<ConstScalar> {
        // PD0002. One closure, one rule: "the initialiser of a top-level
        // const/static has no value in the target's arithmetic". The fault named
        // after the colon (a zero divisor, an overflow, a shift outside 0..63) is
        // the PARAMETER, which is why all six corpus witnesses share this code
        // and are told apart by the message payload rather than by six codes.
        let refuse = |what: String| -> Result<ConstScalar> {
            Err(
                CompileError::Generic(format!(
                    "the initialiser of `{}` has no value: {}",
                    name, what
                ))
                .with_code(DiagnosticCode::ConstInitialiserHasNoValue),
            )
        };
        match expr {
            Expr::Integer(n) => Ok(ConstScalar::Int(*n)),
            Expr::Float(f) => Ok(ConstScalar::Float(*f)),
            Expr::Bool(b) => Ok(ConstScalar::Bool(*b)),
            // A char literal's TYPE is `char` (N4-04), but a CONST INITIALISER
            // is a different question: a top-level `const` may only have a
            // numeric or `bool` type, so no `const` can be declared `char` and
            // nothing can observe this scalar as anything but the integer it
            // is folded into.
            Expr::Char(c) => Ok(ConstScalar::Int(*c as i64)),
            Expr::Unary { op, operand, .. } => {
                let value = self.const_eval(operand, name)?;
                match (op, value) {
                    (UnaryOp::Neg, ConstScalar::Int(n)) => match n.checked_neg() {
                        Some(v) => Ok(ConstScalar::Int(v)),
                        None => refuse(format!("negating {} overflows i64", n)),
                    },
                    (UnaryOp::Neg, ConstScalar::Float(f)) => Ok(ConstScalar::Float(-f)),
                    (UnaryOp::Not, ConstScalar::Bool(b)) => Ok(ConstScalar::Bool(!b)),
                    (UnaryOp::BitNot, ConstScalar::Int(n)) => Ok(ConstScalar::Int(!n)),
                    (op, value) => refuse(format!(
                        "`{:?}` is not defined on the {} it is applied to",
                        op,
                        value.kind()
                    )),
                }
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let l = self.const_eval(left, name)?;
                // `&&` AND `||` SHORT-CIRCUIT HERE BECAUSE THEY SHORT-CIRCUIT AT
                // RUNTIME, and an evaluator that disagrees with the language it
                // evaluates is worse than no evaluator. Measured when this was
                // eager: `const B: bool = false && (1 / 0 == 0);` was refused
                // for a division the program never performs — while the same
                // expression in a function body runs, skips the right side and
                // yields `false`. The constant folder is not allowed to have
                // stricter semantics than the code it replaces.
                if let ConstScalar::Bool(a) = l {
                    match op {
                        BinOp::And if !a => return Ok(ConstScalar::Bool(false)),
                        BinOp::Or if a => return Ok(ConstScalar::Bool(true)),
                        _ => {}
                    }
                }
                let r = self.const_eval(right, name)?;
                match (l, r) {
                    (ConstScalar::Int(a), ConstScalar::Int(b)) => Self::const_int_op(*op, a, b)
                        .map_or_else(|why| refuse(why), Ok),
                    (ConstScalar::Float(a), ConstScalar::Float(b)) => {
                        Self::const_float_op(*op, a, b).map_or_else(|why| refuse(why), Ok)
                    }
                    (ConstScalar::Bool(a), ConstScalar::Bool(b)) => match op {
                        BinOp::And => Ok(ConstScalar::Bool(a && b)),
                        BinOp::Or => Ok(ConstScalar::Bool(a || b)),
                        BinOp::Eq => Ok(ConstScalar::Bool(a == b)),
                        BinOp::Ne => Ok(ConstScalar::Bool(a != b)),
                        op => refuse(format!("`{:?}` is not defined on two booleans", op)),
                    },
                    (a, b) => refuse(format!(
                        "a {} and a {} have no operator between them",
                        a.kind(),
                        b.kind()
                    )),
                }
            }
            // Unreachable through `register_global`, whose caller has already run
            // `validate_global_initializer`. Stated as a refusal rather than a
            // panic, because "the parser guarantees it" is the kind of sentence
            // that stops being true in a later round.
            other => refuse(format!(
                "{:?} is not one of the forms a top-level initialiser may take",
                std::mem::discriminant(other)
            )),
        }
    }

    /// Integer arithmetic that agrees with C or refuses to answer.
    fn const_int_op(op: BinOp, a: i64, b: i64) -> std::result::Result<ConstScalar, String> {
        let overflow = |what: &str| format!("{} {} {} overflows i64", a, what, b);
        Ok(match op {
            BinOp::Add => ConstScalar::Int(a.checked_add(b).ok_or_else(|| overflow("+"))?),
            BinOp::Sub => ConstScalar::Int(a.checked_sub(b).ok_or_else(|| overflow("-"))?),
            BinOp::Mul => ConstScalar::Int(a.checked_mul(b).ok_or_else(|| overflow("*"))?),
            // `checked_div` answers None for BOTH of C's undefined cases — a
            // zero divisor and `i64::MIN / -1` — so they are separated here to
            // say which one happened.
            BinOp::Div if b == 0 => return Err("division by zero".to_string()),
            BinOp::Mod if b == 0 => return Err("the remainder of a division by zero".to_string()),
            BinOp::Div => ConstScalar::Int(a.checked_div(b).ok_or_else(|| overflow("/"))?),
            BinOp::Mod => ConstScalar::Int(a.checked_rem(b).ok_or_else(|| overflow("%"))?),
            // C leaves a shift by a negative amount or by the width of the type
            // undefined, so there is no answer to agree with.
            BinOp::Shl | BinOp::Shr if !(0..64).contains(&b) => {
                return Err(format!(
                    "a shift by {} — the amount has to be between 0 and 63, because an i64 has \
                     64 bits and C leaves the rest undefined",
                    b
                ))
            }
            // `checked_shl` ONLY CHECKS THE COUNT. It answers Some for
            // `1 << 63` — the shift is performed and the VALUE that comes out,
            // i64::MIN, is not `1` scaled by 2^63 at all. Measured before this:
            // `const B: i64 = 1 << 63;` was accepted, and (before the `LL`
            // suffix) printed -2147483648. The test is the round trip: a shift
            // that fits is undone by shifting back.
            //
            // A NEGATIVE LEFT OPERAND IS REFUSED OUTRIGHT, because C leaves
            // `-1 << 1` undefined however small the result is — there is no
            // answer to agree with. `>>` of a negative is only
            // implementation-defined, and every compiler this targets arithmetic
            // -shifts it, which is what Rust does here too, so that one is
            // allowed.
            BinOp::Shl if a < 0 => {
                return Err(format!(
                    "shifting the negative value {} left, which C leaves undefined",
                    a
                ))
            }
            BinOp::Shl => {
                let shifted = a.checked_shl(b as u32).ok_or_else(|| overflow("<<"))?;
                if (shifted >> b) != a {
                    return Err(format!("{} << {} overflows i64", a, b));
                }
                ConstScalar::Int(shifted)
            }
            BinOp::Shr => ConstScalar::Int(a.checked_shr(b as u32).ok_or_else(|| overflow(">>"))?),
            BinOp::BitAnd => ConstScalar::Int(a & b),
            BinOp::BitOr => ConstScalar::Int(a | b),
            BinOp::BitXor => ConstScalar::Int(a ^ b),
            BinOp::Eq => ConstScalar::Bool(a == b),
            BinOp::Ne => ConstScalar::Bool(a != b),
            BinOp::Lt => ConstScalar::Bool(a < b),
            BinOp::Gt => ConstScalar::Bool(a > b),
            BinOp::Le => ConstScalar::Bool(a <= b),
            BinOp::Ge => ConstScalar::Bool(a >= b),
            BinOp::And | BinOp::Or => {
                return Err(format!("`{:?}` is not defined on two integers", op))
            }
        })
    }

    /// Float arithmetic. IEEE-754 has an answer for almost everything, so the
    /// refusals here are the two places C does not.
    fn const_float_op(op: BinOp, a: f64, b: f64) -> std::result::Result<ConstScalar, String> {
        Ok(match op {
            BinOp::Add => ConstScalar::Float(a + b),
            BinOp::Sub => ConstScalar::Float(a - b),
            BinOp::Mul => ConstScalar::Float(a * b),
            // Refused rather than folded to an infinity: an infinite `const` is
            // a value this language has no literal for, so it could be written
            // but never written down.
            BinOp::Div if b == 0.0 => return Err("division by zero".to_string()),
            BinOp::Div => ConstScalar::Float(a / b),
            // `%` on two doubles is not C: gcc answers "invalid operands to
            // binary %". UNREACHABLE THROUGH `register_global` TODAY, and said
            // so rather than left to look like coverage: `check_expression`
            // types `%` as integer-only, so `const X: f64 = 1.5 % 0.5;` is
            // refused one step earlier as "Type mismatch: expected Int, found
            // Float". The arm stays because the two rules are independent and
            // the other one moving should not silently produce invalid C.
            BinOp::Mod => {
                return Err("`%` between two floats, which C has no operator for".to_string())
            }
            BinOp::Eq => ConstScalar::Bool(a == b),
            BinOp::Ne => ConstScalar::Bool(a != b),
            BinOp::Lt => ConstScalar::Bool(a < b),
            BinOp::Gt => ConstScalar::Bool(a > b),
            BinOp::Le => ConstScalar::Bool(a <= b),
            BinOp::Ge => ConstScalar::Bool(a >= b),
            op => return Err(format!("`{:?}` is not defined on two floats", op)),
        })
    }

    /// Refuse a function or type declaration that reuses a top-level item's name.
    ///
    /// The sibling of `refuse_global_shadow`, one scope out: that one is about a
    /// LOCAL hiding a global, this one is about two TOP-LEVEL declarations of
    /// one name. The distinction matters for the message, because the repair is
    /// different — a local can be renamed freely, a second top-level definition
    /// means one of the two was not meant to exist.
    fn refuse_global_collision(&self, name: &str, what: &str, callable: bool) -> Result<()> {
        self.refuse_builtin_definition(name, what, callable)?;
        match self.global_items.get(name) {
            Some(span) => Err(CompileError::Generic(format!(
                "`{}` is declared as {} and as the top-level item at line {}, and a program has \
                 one namespace for both: the emitted C would define the name twice",
                name, what, span.line
            ))),
            None => Ok(()),
        }
    }

    /// Type check a program
    pub fn check(&mut self, program: &Program) -> Result<()> {
        // WHAT COUNTS AS "THE PROGRAM" MUST BE ONE ANSWER IN BOTH PASSES.
        //
        // I argued last round that a value-carrying async return "cannot be
        // honoured wherever it sits, so there is no shadowing exemption". That
        // was right about the construct and wrong about the program: code
        // generation SKIPS an imported body when a local definition of the same
        // name exists (src/codegen/mod.rs, the imported-function loop), so a
        // shadowed imported declaration is not part of the emitted program at
        // all. Diagnosing it rejected a program for a declaration the output
        // would not contain — the same over-approximation as the entry-point
        // case, one construct over.
        //
        // So both passes now ask the same question: is this imported
        // declaration shadowed by a local one?
        //
        // EVERY offender is VALIDATED, in a DETERMINISTIC ORDER, and ALL of
        // them are DIAGNOSED. Those are three properties and the previous loop
        // delivered the first only: it returned at the first unshadowed
        // offender, so later entries were never tested, and the entries arrive
        // in `imported_modules` hash order, so WHICH module supplied the one
        // reported diagnostic varied between runs of the same compiler on the
        // same program. Sorting by (name, span) makes the report a function of
        // the program; collecting rather than returning makes "every entry" a
        // property of the diagnostic and not just of the loop.
        let mut offenders: Vec<(String, Span)> = self
            .deferred_async_value_returns
            .iter()
            .filter(|(name, _)| !crate::ast::local_definition_shadows_import(program, name))
            .cloned()
            .collect();
        offenders.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.start.cmp(&b.1.start))
                .then(a.1.end.cmp(&b.1.end))
        });
        if !offenders.is_empty() {
            return Err(CompileError::async_value_return_unimplemented_in_imports(
                &offenders,
            ));
        }

        // THE ENTRY POINT, NOT ANY DECLARATION. An imported `pub async fn main`
        // is only a defect if it IS the entry point. A local `main` shadows it —
        // `set_imported_modules` registers imported functions first and the
        // first pass below overwrites them — so the imported one can never run,
        // and refusing then rejected a valid program (measured at fbcfc39: a
        // program with its own good `main` failed to compile because a module it
        // imported happened to declare one).
        //
        // Over-approximating a refusal fails closed onto valid programs, which
        // is the mirror of accepting what cannot be honoured; both are the
        // compiler making a claim it has not established.
        if let Some(span) = self.deferred_async_main {
            if !crate::ast::local_definition_shadows_import(program, "main") {
                return Err(CompileError::async_main_unimplemented(span));
            }
        }

        // A TOP-LEVEL ITEM IN AN IMPORTED MODULE (N3-09, N3-10). Raised here
        // rather than at registration for the same reason the async deferrals
        // are: `set_imported_modules` cannot fail. Sorted before reporting, so
        // WHICH module is named does not depend on `HashMap` order.
        if !self.deferred_imported_globals.is_empty() {
            let mut offenders = self.deferred_imported_globals.clone();
            offenders.sort();
            let (module, item) = &offenders[0];
            return Err(CompileError::Generic(format!(
                "the imported module `{}` declares the top-level item `{}`, and a top-level \
                 `const` or `static` in a module is not implemented: nothing emits a definition \
                 for one, so the module's own functions cannot read it and an importer cannot \
                 either. Make it a zero-argument function, or move the item into the program \
                 that uses it (N11 owns cross-module items)",
                module, item
            )));
        }

        // WHICH NAMES ARE ENUMS, BEFORE ANY TYPE IS CONVERTED.
        //
        // `CheckerType::from` maps every `Custom(n)` to `Struct(n)` because it
        // has no table to consult. Enum payloads and struct fields were
        // registered through it while expressions were checked through
        // `ast_type_to_checker_type`, which does consult one. The two disagreed
        // about the SAME name, and since both spellings print as just the name,
        // the disagreement surfaced as `Type mismatch: expected V, found V` — a
        // diagnostic naming one type on both sides, which no reader can act on.
        //
        // It was not a recursion defect. MEASURED on programs with no recursion
        // anywhere: `enum W { A, B } enum V { Wrap(W) }` and `struct S { w: W }`
        // both failed this way, so NO enum could carry an enum and NO struct
        // could hold one.
        //
        // The set is collected up front rather than filled as items are walked,
        // because an enum's own payload names the enum, and because an enum may
        // be declared after its user; both were `Struct` under the incremental
        // map.
        //
        // ONE CALL, because the first version of this was TWO LOOPS FORMING A
        // UNION — every local enum name, plus every imported enum name, with no
        // visibility test and nothing removing a name a local declaration had
        // taken. That reopened the very diagnostic above through the import
        // path: with `pub enum Color` in a module, a downstream
        // `struct Color { v: i64 }` was classified `Enum("Color")` and refused
        // with `Type mismatch: expected Color, found Color`. A union is the
        // wrong shape for a question whose answer has PRECEDENCE.
        self.enum_names = Self::enum_names_in_scope(program, &self.imported_modules);
        self.drop_imports_shadowed_by_local_types(program);

        // DECLARATIONS WITH NO LAYOUT ARE REFUSED HERE, BEFORE ANY C EXISTS.
        //
        // `struct Node { val: i64, next: Node }` used to pass this pass, the
        // borrow checker AND code generation, and then die in gcc with
        // `field has incomplete type 'struct Node'` — the compiler reporting a
        // C-level error for a Palladium-level mistake it had already accepted.
        // Nothing is lost by refusing: a value on a by-value cycle with no enum
        // on it is infinite, so no program could ever have constructed one.
        // `LayoutItems::of` owns the selection AND the sort. This used to chain
        // `program.items` onto every imported module's items here, unfiltered,
        // which merged private imported declarations into the local graph by
        // name and refused valid programs; and it sorted the modules at the call
        // site, which is a second thing a fourth caller could forget.
        let layout = RecursiveLayout::analyze(&LayoutItems::of(program, &self.imported_modules));
        if let Some((name, cycle)) = layout.declarations_without_layout().first() {
            // WHAT THIS MESSAGE MAY CLAIM. It used to say "only an `enum`
            // payload slot can be stored behind a pointer", which reads as a
            // theorem about what can bound a recursive type, and as a theorem
            // it is false: `struct Z { xs: [Z; 0] }` stores no `Z` at all, so
            // its size is finite with no enum anywhere on the cycle. The
            // sentence is now about the mechanism this compiler HAS — one
            // indirection, at an enum payload slot — and the zero-length case
            // is named where it applies instead of being contradicted
            // everywhere.
            // ONE MESSAGE, BUILT AS ONE EXPLANATION. The first version appended
            // the zero-length clause to a head that had already asserted the
            // opposite: the reader was told "so this compiler cannot give it a
            // size" and then, in the same run of sentences, "which stores no
            // elements and therefore does bound the size". Removing the false
            // theorem from the head was not enough, because the head still
            // committed to a CAUSE before the branch that knows the cause had
            // run. So the head now states only the FACT — this type stores
            // itself by value, here is the cycle — and each branch supplies the
            // whole of its own reason.
            let head = format!(
                "recursive type `{}` has no layout: it stores itself by value ({})",
                name,
                cycle.join(" -> ")
            );
            let repair = format!(
                "Break the cycle by routing it through an `enum` variant that can \
                 stop, e.g. `enum {}Link {{ End, More({}) }}`",
                name, name
            );
            let message = if layout.cycle_crosses_a_zero_length_array(cycle) {
                format!(
                    "{}, through a zero-length array. That array stores no elements, \
                     so the size IS bounded and this cycle does not need an `enum` to \
                     stop it — the refusal is a deliberate exclusion (requirement \
                     N4-23), not a claim that your type is infinite. Two reasons, both \
                     measured: the field would have to be emitted as an array of an \
                     incomplete element type, which C cannot spell; and `[T; 0]` has no \
                     values to lay out, because an empty array literal is refused \
                     (\"Empty array literals are not supported (cannot infer type)\"). \
                     {}",
                    head, repair
                )
            } else {
                format!(
                    "{}, and nothing on that cycle can stop. The one indirection this \
                     compiler introduces is an `enum` payload slot whose type reaches \
                     its own enum again, and this cycle has no such slot, so the size \
                     is unbounded. {}",
                    head, repair
                )
            };
            return Err(CompileError::Generic(message));
        }

        // First pass: collect all function signatures and struct definitions
        for item in &program.items {
            // ONE NAMESPACE, CHECKED IN BOTH DIRECTIONS. A top-level `const` and
            // a `fn` of the same name are two C file-scope definitions of one
            // identifier, and nothing before this asked: `const f: i64 = 1;`
            // beside `fn f() -> i64` reached gcc as
            // "redefinition of 'f' as different kind of symbol" — a diagnostic
            // about generated code, naming a conflict the author could see in
            // their own source. This arm catches the FUNCTION-AFTER-GLOBAL
            // order; `register_global` catches the other, because the first
            // pass walks items in source order and either can come first.
            match item {
                Item::Function(func) => self.refuse_global_collision(&func.name, "a function", true)?,
                Item::TypeAlias(alias) => {
                    self.refuse_global_collision(&alias.name, "a type alias", false)?
                }
                Item::Struct(def) => self.refuse_global_collision(&def.name, "a struct", false)?,
                Item::Enum(def) => self.refuse_global_collision(&def.name, "an enum", false)?,
                _ => {}
            }
            match item {
                Item::Function(func) => {
                    if !func.type_params.is_empty() {
                        // This is a generic function - store it for later instantiation
                        let generic_func = GenericFunction {
                            lifetime_params: func.lifetime_params.clone(),
                            type_params: func.type_params.clone(),
                            params: func
                                .params
                                .iter()
                                .map(|p| (p.name.clone(), p.ty.clone()))
                                .collect(),
                            return_type: func.return_type.clone(),
                            body: func.body.clone(),
                            is_async: func.is_async,
                            span: func.span,
                        };
                        self.generic_functions
                            .insert(func.name.clone(), generic_func);
                        // Lockstep with the insert above. Locals are walked after
                        // imports, so this overwrites any imported template of the
                        // same name -- and the origin has to overwrite with it.
                        self.generic_function_origin.insert(func.name.clone(), None);
                    } else {
                        // Regular function - process as before
                        let param_types: Vec<CheckerType> = func
                            .params
                            .iter()
                            .map(|param| self.ast_type_to_checker_type(&param.ty))
                            .collect();

                        let return_type = func
                            .return_type
                            .as_ref()
                            .map(|t| self.ast_type_to_checker_type(t))
                            .unwrap_or(CheckerType::Unit);

                        let func_type = CheckerType::Function(param_types, Box::new(return_type));
                        self.functions.insert(func.name.clone(), func_type);
                    }
                }
                Item::Struct(struct_def) => {
                    // Check if this is a generic struct
                    if !struct_def.type_params.is_empty() || !struct_def.lifetime_params.is_empty()
                    {
                        // Store as generic struct
                        let generic_struct = GenericStruct {
                            lifetime_params: struct_def.lifetime_params.clone(),
                            type_params: struct_def.type_params.clone(),
                            fields: struct_def.fields.clone(),
                        };
                        self.generic_structs
                            .insert(struct_def.name.clone(), generic_struct);
                    } else {
                        // Convert field types to CheckerType for non-generic
                        // structs. Through `ast_type_to_checker_type`, not
                        // `CheckerType::from`: the latter calls every named type
                        // a struct, so `struct S { w: W }` over an `enum W`
                        // recorded the field as `Struct(W)` while every
                        // expression producing a `W` had type `Enum(W)`, and the
                        // program was refused with `expected W, found W`.
                        let fields: Vec<(String, CheckerType)> = struct_def
                            .fields
                            .iter()
                            .map(|(name, ty)| (name.clone(), self.ast_type_to_checker_type(ty)))
                            .collect();

                        self.structs.insert(struct_def.name.clone(), fields);
                    }
                }
                Item::Enum(enum_def) => {
                    // Check if this is a generic enum
                    if !enum_def.type_params.is_empty() || !enum_def.lifetime_params.is_empty() {
                        // Store as generic enum
                        let generic_enum = GenericEnum {
                            lifetime_params: enum_def.lifetime_params.clone(),
                            type_params: enum_def.type_params.clone(),
                            variants: enum_def
                                .variants
                                .iter()
                                .map(|v| (v.name.clone(), v.data.clone()))
                                .collect(),
                        };
                        self.generic_enums
                            .insert(enum_def.name.clone(), generic_enum);
                    } else {
                        // Store enum variants for type checking
                        let mut variants = Vec::new();

                        for variant in &enum_def.variants {
                            // Payload types go through `ast_type_to_checker_type`
                            // for the same reason struct fields do, and the
                            // symptom here was the reported one:
                            // `enum V { Pair(V, V) }` recorded its payload as
                            // `Struct(V)` while `V::Leaf(1)` had type `Enum(V)`,
                            // so constructing the recursive variant was refused
                            // with `expected V, found V`.
                            let variant_fields = match &variant.data {
                                crate::ast::EnumVariantData::Unit => EnumVariantFields::Unit,
                                crate::ast::EnumVariantData::Tuple(types) => {
                                    let field_types: Vec<CheckerType> = types
                                        .iter()
                                        .map(|ty| self.ast_type_to_checker_type(ty))
                                        .collect();
                                    EnumVariantFields::Tuple(field_types)
                                }
                                crate::ast::EnumVariantData::Struct(fields) => {
                                    let named_fields: Vec<(String, CheckerType)> = fields
                                        .iter()
                                        .map(|(name, ty)| {
                                            (name.clone(), self.ast_type_to_checker_type(ty))
                                        })
                                        .collect();
                                    EnumVariantFields::Named(named_fields)
                                }
                            };

                            variants.push(EnumVariant {
                                name: variant.name.clone(),
                                fields: variant_fields,
                            });

                            // Also register variant constructors as functions
                            let enum_type = CheckerType::Enum(enum_def.name.clone());
                            let variant_name = format!("{}::{}", enum_def.name, variant.name);

                            let func_type = match &variant.data {
                                crate::ast::EnumVariantData::Unit => {
                                    CheckerType::Function(vec![], Box::new(enum_type.clone()))
                                }
                                crate::ast::EnumVariantData::Tuple(types) => {
                                    let param_types: Vec<CheckerType> = types
                                        .iter()
                                        .map(|ty| self.ast_type_to_checker_type(ty))
                                        .collect();
                                    CheckerType::Function(param_types, Box::new(enum_type.clone()))
                                }
                                crate::ast::EnumVariantData::Struct(fields) => {
                                    let param_types: Vec<CheckerType> = fields
                                        .iter()
                                        .map(|(_, ty)| self.ast_type_to_checker_type(ty))
                                        .collect();
                                    CheckerType::Function(param_types, Box::new(enum_type.clone()))
                                }
                            };

                            self.functions.insert(variant_name, func_type);
                        }

                        self.enums.insert(enum_def.name.clone(), variants);
                    }
                }
                Item::Trait(trait_def) => {
                    // Register trait with the trait resolver
                    self.trait_resolver.register_trait(trait_def)?;
                }
                Item::TypeAlias(type_alias) => {
                    // Check if this is a generic type alias
                    if !type_alias.type_params.is_empty() || !type_alias.lifetime_params.is_empty()
                    {
                        // Store as generic type alias
                        let generic_alias = GenericTypeAlias {
                            lifetime_params: type_alias.lifetime_params.clone(),
                            type_params: type_alias.type_params.clone(),
                            ty: type_alias.ty.clone(),
                        };
                        self.generic_type_aliases
                            .insert(type_alias.name.clone(), generic_alias);
                    } else {
                        // Store regular type alias
                        self.type_aliases
                            .insert(type_alias.name.clone(), type_alias.ty.clone());
                    }
                }
                Item::Impl(impl_block) => {
                    // Register impl block with trait resolver
                    self.trait_resolver.register_impl(impl_block)?;

                    // If this is a trait impl, verify all required methods are implemented
                    if let Some(Type::Custom(trait_name)) = &impl_block.trait_type {
                        self.trait_resolver
                            .check_trait_impl_complete(impl_block, trait_name)?;
                    }

                    // Register methods from impl blocks.
                    //
                    // `Self` is resolved FIRST (N5-17): a method registered
                    // with a `Custom("Self")` parameter is a signature no call
                    // site can ever satisfy, because no call site can name that
                    // type. See `ImplBlock::methods_with_self_resolved`.
                    for method in &impl_block.methods_with_self_resolved() {
                        // Create qualified method name
                        let method_name = if let Some(_trait_type) = &impl_block.trait_type {
                            // Trait implementation method
                            format!("{}::{}", impl_block.for_type, method.name)
                        } else {
                            // Inherent method
                            format!("{}::{}", impl_block.for_type, method.name)
                        };

                        // The receiver form, recorded for EVERY method before any body is
                        // walked. Generic ones too: they are refused at the call site for a
                        // different reason, and a map with holes in it is a map whose misses
                        // cannot be told apart from "this method takes no `self`".
                        if let Some(recv) = Self::self_receiver_of(method) {
                            self.impl_method_receiver.insert(method_name.clone(), recv);
                        }

                        if !method.type_params.is_empty() {
                            // Generic method - store for later instantiation
                            let generic_func = GenericFunction {
                                lifetime_params: method.lifetime_params.clone(),
                                type_params: method.type_params.clone(),
                                params: method
                                    .params
                                    .iter()
                                    .map(|p| (p.name.clone(), p.ty.clone()))
                                    .collect(),
                                return_type: method.return_type.clone(),
                                body: method.body.clone(),
                                is_async: method.is_async,
                                span: method.span,
                            };
                            self.generic_function_origin
                                .insert(method_name.clone(), None);
                            self.generic_functions.insert(method_name, generic_func);
                        } else {
                            // Regular method
                            let param_types: Vec<CheckerType> = method
                                .params
                                .iter()
                                .map(|param| CheckerType::from(&param.ty))
                                .collect();

                            let return_type = method
                                .return_type
                                .as_ref()
                                .map(CheckerType::from)
                                .unwrap_or(CheckerType::Unit);

                            let func_type =
                                CheckerType::Function(param_types, Box::new(return_type));
                            self.functions.insert(method_name, func_type);
                        }
                    }
                }
                Item::Macro(_) => {
                    // Macros are handled during expansion phase, skip here
                }
                Item::Global(global) => {
                    self.register_global(global)?;
                }
            }
        }

        // Check for main function
        if !self.functions.contains_key("main") {
            return Err(TypeErrorHelper::missing_main());
        }

        // Second pass: type check function bodies
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.check_function(func)?;
                }
                Item::Struct(_) => {
                    // Structs are already processed in the first pass
                }
                Item::Enum(_) => {
                    // Enums are already processed in the first pass
                }
                Item::Trait(_) => {
                    // Traits are already processed in the first pass
                    // TODO: Type check trait methods with bodies
                }
                Item::TypeAlias(_) => {
                    // Type aliases are already processed in the first pass
                    // No body to check
                }
                Item::Impl(impl_block) => {
                    // Set current impl type for Self resolution
                    self.current_impl_type = Some(match &impl_block.for_type {
                        Type::Custom(name) => name.clone(),
                        Type::Generic { name, .. } => name.clone(),
                        _ => "Unknown".to_string(), // Shouldn't happen for impl blocks
                    });

                    // If this is a generic impl, skip type checking for now
                    // Generic impls will be checked when instantiated
                    if !impl_block.type_params.is_empty() {
                        self.current_impl_type = None;
                        continue;
                    }

                    // Type check impl block methods, with `Self` resolved —
                    // the same substitution the registration above used, from
                    // the same function, so the signature that is checked is
                    // the signature that was registered.
                    for method in &impl_block.methods_with_self_resolved() {
                        // A `mut` PARAMETER ON A METHOD IS REFUSED, and this is now
                        // CONSERVATIVE rather than forced. The rationale here used
                        // to be that the call path resolves through `impl_methods`
                        // while the address-taking decision reads `functions`,
                        // which never holds a `Type::method`, so the definition
                        // took a pointer and every call passed a value. su2 closed
                        // exactly that: methods register their params, and the
                        // call side takes an address for pointer parameters. The
                        // refusal is kept because nothing witnesses `mut` params on
                        // methods end to end, and lifting it is its own row.
                        if let Some(param) = method.params.iter().find(|p| p.mutable) {
                            return Err(CompileError::Generic(format!(
                                "`{}::{}` takes `mut {}`, and `mut` parameters on methods are \
                                 not implemented: the definition would take a pointer while \
                                 every call site passes a value. Take the value and return the \
                                 new one, or write a free `fn`",
                                impl_block.for_type, method.name, param.name
                            )));
                        }
                        // The receiver form, for the write rule below.
                        self.current_self_receiver = Self::self_receiver_of(method);
                        let checked = self.check_function(method);
                        self.current_self_receiver = None;
                        checked?;
                    }

                    // Clear current impl type
                    self.current_impl_type = None;
                }
                Item::Macro(_) => {
                    // Macros are handled during expansion phase, skip here
                }
                Item::Global(_) => {
                    // Registered and checked in the first pass: an initialiser
                    // is a constant expression by the parser's rule, so there is
                    // no body here that could depend on anything the first pass
                    // had not yet seen.
                }
            }
        }

        // Third pass: type check the bodies of the imported modules.
        //
        // Same reason as the borrow checker's third pass
        // (`src/ownership/borrow_checker.rs`, "Third pass"): making an imported
        // signature callable without ever visiting the body behind it means the
        // compiler accepts code it has not checked. Measured before this,
        // `pub fn broken() -> i64 { let s = "x"; return s; }` in a module printed
        // "Compilation successful" and then died in gcc with "incompatible pointer
        // to integer conversion returning 'const char *'" — a C diagnostic against
        // code the user never wrote, which is the class of failure this pass exists
        // to remove.
        //
        // GENERICS ARE SKIPPED HERE, and the reason changed under this pass.
        // It used to say "no generic guard needed: `check_function` already
        // returns early for a function with type parameters". That was true
        // until the async-value-return refusal was placed BEFORE that early
        // return (`src/typeck/mod.rs:3156-3158`), and walking an imported
        // generic now raises it at DECLARATION. An uninstantiated generic is
        // emitted by nobody, so refusing it rejects a declaration the output
        // cannot contain — which is what
        // `an_uninstantiated_imported_generic_async_violation_is_not_diagnosed`
        // pins. Generic imported bodies are owned by the deferred check at the
        // end of this function, which filters on `self.instantiations` and so
        // fires only for the ones that become C.
        //
        // Module order is sorted for the same reason `set_imported_modules` sorts:
        // when two modules both fail, WHICH error the user is shown must not depend
        // on the hash seed.
        let modules = std::mem::take(&mut self.imported_modules);
        let mut module_names: Vec<&String> = modules.keys().collect();
        module_names.sort();
        for module_name in module_names {
            for item in &modules[module_name].ast.items {
                if let Item::Function(func) = item {
                    if !matches!(func.visibility, crate::ast::Visibility::Public) {
                        continue;
                    }
                    if !func.type_params.is_empty() {
                        continue;
                    }
                    // A SHADOWED IMPORT IS NOT IN THE OUTPUT, so checking it is
                    // checking code that cannot run — the same rule as the generic
                    // skip above, and asked through the SHARED predicate codegen
                    // uses to decide what it emits, so the two cannot drift.
                    // Without it, a module exporting `pub async fn main` beside a
                    // program declaring its own `fn main` was refused with
                    // "`async fn main` is not implemented", naming a declaration
                    // the local definition displaces
                    // (`async_main_is_refused_only_when_it_is_the_entry_point`).
                    if crate::ast::local_definition_shadows_import(program, &func.name) {
                        continue;
                    }
                    self.check_function(func)?;
                }
            }
        }
        self.imported_modules = modules;

        // AN IMPORTED GENERIC THAT WAS INSTANTIATED IS PART OF THE EMITTED
        // PROGRAM, so the refusal `check_function` applies to every LOCAL async
        // function has to apply to it too. Raised HERE and not at the opening
        // because "was it instantiated" is only knowable after the body walk
        // above has run every call site.
        //
        // WHICH BODY AN INSTANTIATION CARRIES IS DECIDED HERE, NOT IN CODEGEN.
        // `get_instantiations` pairs each key with whatever
        // `self.generic_functions` holds for that name, and a local generic
        // definition OVERWRITES the imported entry (`set_imported_modules` runs
        // first). So a local generic of the same name means the emitted body is
        // the local one — already validated by `check_function`, which tests
        // `is_async` before its own generic skip — and the imported declaration
        // is not in the output. That, and not shadowing in the
        // `local_definition_shadows_import` sense, is the exemption here: an
        // ordinary local `fn agen` does NOT displace an imported `agen<T>`,
        // because the call site consults `generic_functions` first.
        let generic_offenders =
            self.emitted_generic_offenders(program, &self.deferred_generic_async_value_returns);
        if !generic_offenders.is_empty() {
            return Err(CompileError::async_value_return_unimplemented_in_imports(
                &generic_offenders,
            ));
        }

        // N7-18, the superset. Same filter, applied through the SAME function
        // so the two cannot answer "is this import part of the emitted
        // program?" differently, and raised second so that the more specific
        // wording above wins when a declaration earns both.
        let generic_async_offenders =
            self.emitted_generic_offenders(program, &self.deferred_generic_async_imports);
        if !generic_async_offenders.is_empty() {
            return Err(CompileError::async_fn_unimplemented_in_imports(
                &generic_async_offenders,
            ));
        }

        Ok(())
    }

    /// Which of `deferred` are actually part of the emitted program.
    ///
    /// Two conditions, and both are exemptions that exist because refusing
    /// without them rejected valid programs:
    ///   * INSTANTIATED. An imported generic that nothing instantiates is
    ///     emitted by nobody, so a diagnostic against it names a declaration
    ///     the output cannot contain.
    ///   * NOT DISPLACED BY A LOCAL GENERIC. `get_instantiations` pairs each
    ///     key with whatever `self.generic_functions` holds, and a local
    ///     generic definition OVERWRITES the imported entry, so the emitted
    ///     body is the local one — which `check_function` already validated.
    ///     An ordinary local `fn agen` does NOT displace an imported
    ///     `agen<T>`: the call site consults `generic_functions` first.
    ///
    /// Sorted by (name, span) before it is returned, because it is built from
    /// `imported_modules`, a `HashMap`, and WHICH offender a diagnostic points
    /// at must be a function of the program rather than of the hash seed.
    fn emitted_generic_offenders(
        &self,
        program: &Program,
        deferred: &[(String, Span)],
    ) -> Vec<(String, Span)> {
        let mut offenders: Vec<(String, Span)> = deferred
            .iter()
            .filter(|(name, _)| self.instantiations.keys().any(|k| &k.name == name))
            .filter(|(name, _)| {
                !program.items.iter().any(|item| {
                    matches!(item, Item::Function(f) if &f.name == name && !f.type_params.is_empty())
                })
            })
            .cloned()
            .collect();
        offenders.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.start.cmp(&b.1.start))
                .then(a.1.end.cmp(&b.1.end))
        });
        offenders
    }

    /// Convert AST type to CheckerType considering context (struct vs enum)
    fn ast_type_to_checker_type(&self, ast_type: &crate::ast::Type) -> CheckerType {
        match ast_type {
            crate::ast::Type::Custom(name) => {
                // Handle Self type
                if name == "Self" {
                    if let Some(impl_type) = &self.current_impl_type {
                        // Check if it's an enum or struct
                        if self.enums.contains_key(impl_type) {
                            return CheckerType::Enum(impl_type.clone());
                        } else {
                            return CheckerType::Struct(impl_type.clone());
                        }
                    } else {
                        // Self used outside of impl block - this is an error but return a placeholder
                        return CheckerType::Struct("Self".to_string());
                    }
                }

                // First check if it's a type alias
                if let Some(aliased_type) = self.type_aliases.get(name) {
                    // Recursively resolve the aliased type
                    return self.ast_type_to_checker_type(aliased_type);
                }

                // Check if it's an enum.
                //
                // `enum_names` is consulted BESIDE `enums`, not instead of it:
                // `enums` is the half-filled map this pass builds as it walks,
                // and `enum_names` is the complete set collected before the walk
                // begins. Reading only the first gave the same name two kinds
                // depending on WHEN it was asked (an enum's own payload, and any
                // enum declared after its user, both came back `Struct`); the
                // union makes the answer a property of the program instead of a
                // property of the walk position.
                if self.enums.contains_key(name) || self.enum_names.contains(name) {
                    CheckerType::Enum(name.clone())
                } else {
                    CheckerType::Struct(name.clone())
                }
            }
            crate::ast::Type::Generic { name, args } => {
                // First check if it's a generic type alias
                if let Some(generic_alias) = self.generic_type_aliases.get(name) {
                    // We have a generic type alias, substitute the type parameters
                    if args.len() != generic_alias.type_params.len() {
                        // For now, just return the generic type without substitution
                        // TODO: Proper error handling for wrong number of type arguments
                        let checker_args: Vec<GenericArgValue> = args
                            .iter()
                            .map(|arg| match arg {
                                GenericArg::Type(t) => {
                                    GenericArgValue::Type(self.ast_type_to_checker_type(t))
                                }
                                GenericArg::Const(c) => GenericArgValue::Const(match c {
                                    ConstValue::Integer(n) => ConstValueResolved::Integer(*n),
                                    ConstValue::ConstParam(name) => {
                                        ConstValueResolved::ConstParam(name.clone())
                                    }
                                }),
                            })
                            .collect();
                        return CheckerType::Generic {
                            name: name.clone(),
                            args: checker_args,
                        };
                    }

                    // Create a substitution map for type parameters only
                    let mut substitutions = std::collections::HashMap::new();
                    let type_args: Vec<crate::ast::Type> = args
                        .iter()
                        .filter_map(|arg| match arg {
                            GenericArg::Type(t) => Some(t.clone()),
                            GenericArg::Const(_) => None, // TODO: handle const generics in aliases
                        })
                        .collect();

                    for (param, arg) in generic_alias.type_params.iter().zip(type_args.iter()) {
                        substitutions.insert(param.clone(), arg.clone());
                    }

                    // Substitute type parameters in the aliased type
                    let substituted_type =
                        self.substitute_type_params_map(&generic_alias.ty, &substitutions);
                    return self.ast_type_to_checker_type(&substituted_type);
                }

                // Not a type alias, convert generic types normally
                let checker_args: Vec<GenericArgValue> = args
                    .iter()
                    .map(|arg| match arg {
                        GenericArg::Type(t) => {
                            GenericArgValue::Type(self.ast_type_to_checker_type(t))
                        }
                        GenericArg::Const(c) => GenericArgValue::Const(match c {
                            ConstValue::Integer(n) => ConstValueResolved::Integer(*n),
                            ConstValue::ConstParam(name) => {
                                ConstValueResolved::ConstParam(name.clone())
                            }
                        }),
                    })
                    .collect();

                CheckerType::Generic {
                    name: name.clone(),
                    args: checker_args,
                }
            }
            _ => CheckerType::from(ast_type),
        }
    }

    /// Type check a function
    /// Does `body` contain a `return <value>` anywhere?
    ///
    /// Walks nested blocks because the parser's tail lowering puts the return
    /// inside whichever branch was the tail — an `if` arm or a `match` arm, not
    /// necessarily the top level.
    fn has_value_return(body: &[Stmt]) -> bool {
        for stmt in body {
            let found = match stmt {
                Stmt::Return(Some(_)) => return true,
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    Self::has_value_return(then_branch)
                        || else_branch
                            .as_ref()
                            .is_some_and(|b| Self::has_value_return(b))
                }
                Stmt::While { body, .. } | Stmt::For { body, .. } => Self::has_value_return(body),
                Stmt::Unsafe { body, .. } => Self::has_value_return(body),
                Stmt::Match { arms, .. } => arms.iter().any(|a| Self::has_value_return(&a.body)),
                _ => false,
            };
            if found {
                return true;
            }
        }
        false
    }

    fn check_function(&mut self, func: &Function) -> Result<()> {
        // `async fn main` has no entry point that anything can call. Refused
        // here, before code generation, because codegen would emit
        // `main_Future main()` and a `main_poll` nobody invokes: a program that
        // links, runs, exits 0 and does nothing. See
        // `CompileError::async_main_unimplemented` for the measurement and for
        // why this is a refusal rather than a lowering.
        //
        // Checked BEFORE the generic skip above would have applied, so a
        // hypothetical `async fn main<T>` cannot slip past it.
        if func.is_async && func.name == "main" {
            return Err(CompileError::async_main_unimplemented(func.span));
        }

        // A value-carrying `return` inside an async function has nowhere to go:
        // the poll function it is emitted into returns an `int` readiness flag.
        // See `CompileError::async_value_return_unimplemented` for the
        // measurement — including the ORDINARY function returning `Future<()>`
        // that makes this reachable, which no enumeration of async *spellings*
        // would have found.
        if func.is_async && Self::has_value_return(&func.body) {
            return Err(CompileError::async_value_return_unimplemented(func.span));
        }

        // N7-18: AND `async fn` ITSELF, whatever the body does.
        //
        // The two arms above are named sub-cases of this one and are kept only
        // because their wording is more specific — the entry point says what
        // the emitted `main` would have looked like, the value return says
        // where the value would have gone. They fire first for that reason and
        // for no other; this arm is what makes "no `async fn` reaches code
        // generation" a property of the predicate `is_async` rather than of an
        // enumeration of async SPELLINGS, which is exactly how the plainest one
        // survived three rounds of refusals (`async fn g() { print("x"); }`
        // compiled clean at acda322 and emitted `g_Future`/`g_poll`).
        //
        // WHAT IT DOES NOT REJECT. Only `Function::is_async` — nothing about
        // the name, the return type, the body, or a written `Future<T>`. An
        // ordinary `fn` is untouched, and so is every diagnostic those other
        // shapes already had.
        //
        // Checked BEFORE the generic skip below, like the two arms above, so a
        // local `async fn g<T>` is refused at its declaration rather than at
        // whichever call site happens to instantiate it. That is deliberate
        // ASYMMETRY with imported generics, which are refused only when
        // instantiated: a local declaration is the programmer's own source and
        // the construct cannot be honoured wherever it sits, while an imported
        // one that nothing instantiates is not part of the emitted program at
        // all. See `check`'s closing lines.
        if func.is_async {
            return Err(CompileError::async_fn_unimplemented(func.span));
        }

        // Skip generic functions - they'll be checked when instantiated
        if !func.type_params.is_empty() {
            return Ok(());
        }

        // Enter function scope
        self.symbols.enter_scope();

        // Add function parameters to symbol table
        for param in &func.params {
            self.refuse_global_shadow(&param.name, "the parameter")?;
            let checker_type = self.ast_type_to_checker_type(&param.ty);
            self.symbols
                .define(param.name.clone(), checker_type, param.mutable)?;
        }

        // Set current function return type
        let base_return_type = func
            .return_type
            .as_ref()
            .map(|t| self.ast_type_to_checker_type(t))
            .unwrap_or(CheckerType::Unit);

        // If function is async, wrap return type in Future
        let return_type = if func.is_async {
            CheckerType::Generic {
                name: "Future".to_string(),
                args: vec![GenericArgValue::Type(base_return_type)],
            }
        } else {
            base_return_type
        };

        self.current_function_return = Some(return_type);

        // Type check each statement in the body
        for stmt in &func.body {
            self.check_statement(stmt)?;
        }

        // Exit function scope
        self.symbols.exit_scope();
        self.current_function_return = None;
        Ok(())
    }

    /// Type check a statement
    /// Does this assignment target bottom out in `self`?
    ///
    /// `self.n`, `self.d[i]`, `self.a.b[0]` -- the write rule is about the RECEIVER, so
    /// the question is what the place chain is rooted in, not what its last link is.
    fn assign_target_base_is_self(target: &AssignTarget) -> bool {
        match target {
            AssignTarget::Ident(n) => n == "self",
            AssignTarget::FieldAccess { object, .. } => Self::expr_base_is_self(object),
            AssignTarget::Index { array, .. } => Self::expr_base_is_self(array),
            AssignTarget::Deref { expr } => Self::expr_base_is_self(expr),
        }
    }

    /// Does this expression bottom out in `self`?
    ///
    /// Shared with the CALL rule, which asks the same question of a method call's
    /// receiver: `self.bump()`, `self.inner.bump()`, `self.d[i].bump()`. Two copies of
    /// this walker would be two answers to "is this the receiver", and the call rule and
    /// the write rule are the same rule about the same object.
    fn expr_base_is_self(e: &Expr) -> bool {
        match e {
            Expr::Ident(n) => n == "self",
            Expr::FieldAccess { object, .. } => Self::expr_base_is_self(object),
            Expr::Index { array, .. } => Self::expr_base_is_self(array),
            Expr::Deref { expr, .. } => Self::expr_base_is_self(expr),
            _ => false,
        }
    }

    /// Which receiver form does this signature declare?
    ///
    /// THE ONE DEFINITION, because it is read in two places that must not drift: the
    /// first pass records every method's form for the call rule, and the second pass
    /// records the form of the method it is about to walk for the write rule. Read off
    /// the signature rather than guessed -- the parser records `self` as a param named
    /// "self" whose TYPE carries the form.
    fn self_receiver_of(method: &Function) -> Option<SelfReceiver> {
        method
            .params
            .iter()
            .find(|prm| prm.name == "self")
            .map(|prm| match &prm.ty {
                Type::Reference { mutable: true, .. } => SelfReceiver::MutRef,
                Type::Reference { .. } => SelfReceiver::Shared,
                _ => SelfReceiver::ByValue,
            })
    }

    fn check_statement(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Expr(expr) => {
                self.check_expression(expr)?;
                Ok(())
            }
            Stmt::Return(None) => {
                // Returning nothing is Unit type
                if self.current_function_return != Some(CheckerType::Unit) {
                    return Err(CompileError::TypeMismatch {
                        expected: "()".to_string(),
                        found: "return value".to_string(),
                        span: None,
                    });
                }
                Ok(())
            }
            Stmt::Return(Some(expr)) => {
                let expr_type = self.check_expression(expr)?;
                if let Some(expected) = &self.current_function_return {
                    if expr_type != *expected {
                        return Err(CompileError::TypeMismatch {
                            expected: expected.to_string(),
                            found: expr_type.to_string(),
                            span: None,
                        });
                    }
                }
                Ok(())
            }
            Stmt::Let {
                name,
                ty,
                value,
                mutable,
                ..
            } => {
                self.refuse_global_shadow(name, "the local")?;

                // Type check the value expression
                let value_type = self.check_expression(value)?;

                // If type annotation is provided, check that it matches
                if let Some(annotated_type) = ty {
                    let expected_type = self.ast_type_to_checker_type(annotated_type);
                    if value_type != expected_type {
                        return Err(self.error_helper.type_mismatch(
                            &expected_type.to_string(),
                            &value_type.to_string(),
                            None,
                        ));
                    }
                    // Define variable with annotated type
                    self.symbols.define(name.clone(), expected_type, *mutable)?;
                } else {
                    // Define variable with inferred type
                    self.symbols.define(name.clone(), value_type, *mutable)?;
                }

                Ok(())
            }
            Stmt::Assign { target, value, .. } => {
                // WRITING THROUGH `self` IS A PROPERTY OF THE RECEIVER, and this is the
                // only place that knows both. `self` became a place base this round, so
                // `self.n = v`, `self.d[i] = v` and their chains now parse; whether they
                // MEAN anything depends on how the method took its receiver, and the two
                // wrong answers both used to escape the front end:
                //   `&self`  lowered to `self->n = v` against `const struct C*`, and gcc
                //            refused C this compiler had just approved.
                //   `self`   mutated a COPY. It compiled, it ran, and the caller observed
                //            nothing -- the silent-wrong-result class, not a diagnostic.
                // Both are refused here by name. `&mut self` is the writable form.
                if self.current_self_receiver.is_some() {
                    // THE RECEIVER BINDING ITSELF IS NOT REASSIGNABLE, whatever form it
                    // took. `self = C { .. }` used to reach the stock immutable-binding
                    // message, which advises `let mut self = ...` -- a spelling the parser
                    // refuses, so the reader was sent to write something impossible.
                    if matches!(target, AssignTarget::Ident(n) if n == "self") {
                        return Err(CompileError::Generic(
                            "cannot assign to `self`: the receiver binding is not reassignable. \
                             Assign to its fields (`self.f = v`, which needs `&mut self`), or \
                             return a new value"
                                .to_string(),
                        ));
                    }
                }
                if let Some(recv) = self.current_self_receiver {
                    if recv != SelfReceiver::MutRef && Self::assign_target_base_is_self(target) {
                        let detail = match recv {
                            SelfReceiver::Shared => {
                                "`&self` is a SHARED borrow of the receiver. Take `&mut self` if \
                                 this method is meant to modify it"
                            }
                            _ => {
                                "a by-value `self` receiver is a COPY, and not a `mut` binding, so \
                                 the caller would not observe the write. Take `&mut self` if this \
                                 method is meant to modify the receiver, or return the new value"
                            }
                        };
                        return Err(CompileError::Generic(format!(
                            "cannot assign through `self`: {}",
                            detail
                        )));
                    }
                }
                match target {
                    AssignTarget::Ident(name) => {
                        // Look up the variable and clone necessary info
                        let (var_type, var_mutable) = {
                            match self.symbols.lookup(name) {
                                Some(var_info) => (var_info.ty.clone(), var_info.mutable),
                                None => {
                                    // Update error helper with available variables
                                    let available_vars = self.get_available_variables();
                                    self.error_helper.update_available(
                                        available_vars,
                                        vec![],
                                        vec![],
                                    );
                                    return Err(self.error_helper.undefined_variable(name, None));
                                }
                            }
                        };

                        // Check if variable is mutable
                        if !var_mutable {
                            // A top-level item gets its own wording: the stock
                            // advice is "declare it with `let mut`", and there is
                            // no `let` here to add `mut` to.
                            if self.global_items.contains_key(name) {
                                return Err(CompileError::Generic(format!(
                                    "cannot assign to `{}`: a top-level item is read-only \
                                     unless it is declared `static mut`",
                                    name
                                )));
                            }
                            return Err(self.error_helper.immutable_assignment(name));
                        }

                        // Type check the value expression
                        let value_type = self.check_expression(value)?;

                        // Check that types match
                        if value_type != var_type {
                            return Err(self.error_helper.type_mismatch(
                                &var_type.to_string(),
                                &value_type.to_string(),
                                None,
                            ));
                        }

                        Ok(())
                    }
                    AssignTarget::Index { array, index } => {
                        // Type check the array expression
                        let array_type = self.check_expression(array)?;

                        // Type check the index expression (must be Int)
                        let index_type = self.check_expression(index)?;
                        if index_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "Int".to_string(),
                                found: index_type.to_string(),
                                span: None,
                            });
                        }

                        // Extract element type from array type
                        let elem_type = match array_type {
                            CheckerType::Array(elem_type, _size) => elem_type.as_ref().clone(),
                            _ => {
                                return Err(CompileError::Generic(format!(
                                    "Cannot index into non-array type: {}",
                                    array_type
                                )));
                            }
                        };

                        // Type check the value expression
                        let value_type = self.check_expression(value)?;

                        // Check that types match
                        if value_type != elem_type {
                            return Err(CompileError::TypeMismatch {
                                expected: elem_type.to_string(),
                                found: value_type.to_string(),
                                span: None,
                            });
                        }

                        Ok(())
                    }
                    AssignTarget::FieldAccess { object, field } => {
                        // Type check the object expression
                        let object_type = self.check_expression(object)?;

                        let field_type = match &object_type {
                            // Handle non-generic structs
                            CheckerType::Struct(name) => {
                                // Look up the struct fields
                                let fields = self.structs.get(name).ok_or_else(|| {
                                    CompileError::Generic(format!("Unknown struct type: {}", name))
                                })?;

                                // Find the field type
                                fields
                                    .iter()
                                    .find(|(fname, _)| fname == field)
                                    .map(|(_, ftype)| ftype.clone())
                                    .ok_or_else(|| {
                                        CompileError::Generic(format!(
                                            "Struct '{}' has no field '{}'",
                                            name, field
                                        ))
                                    })?
                            }
                            // Handle generic struct instances
                            CheckerType::Generic { name, args } => {
                                // Look up the generic struct definition
                                let generic_struct =
                                    self.generic_structs.get(name).ok_or_else(|| {
                                        CompileError::Generic(format!(
                                            "Unknown generic struct type: {}",
                                            name
                                        ))
                                    })?;

                                // Find the field's declared type
                                let field_type = generic_struct
                                    .fields
                                    .iter()
                                    .find(|(fname, _)| fname == field)
                                    .map(|(_, ftype)| ftype)
                                    .ok_or_else(|| {
                                        CompileError::Generic(format!(
                                            "Struct '{}' has no field '{}'",
                                            name, field
                                        ))
                                    })?;

                                // Extract type arguments only
                                let type_args: Vec<CheckerType> = args
                                    .iter()
                                    .filter_map(|arg| match arg {
                                        GenericArgValue::Type(t) => Some(t.clone()),
                                        GenericArgValue::Const(_) => None, // TODO: handle const generics
                                    })
                                    .collect();

                                // Substitute type parameters in the field type
                                self.substitute_type_params(
                                    field_type,
                                    &generic_struct.type_params,
                                    &type_args,
                                )?
                            }
                            _ => {
                                return Err(CompileError::Generic(format!(
                                    "Cannot access field on non-struct type: {}",
                                    object_type
                                )));
                            }
                        };

                        // Type check the value expression
                        let value_type = self.check_expression(value)?;

                        // Check that types match
                        if value_type != field_type {
                            return Err(CompileError::TypeMismatch {
                                expected: field_type.to_string(),
                                found: value_type.to_string(),
                                span: None,
                            });
                        }

                        Ok(())
                    }
                    AssignTarget::Deref { expr } => {
                        // Type check the expression being dereferenced
                        let _ptr_type = self.check_expression(expr)?;
                        // For now, we don't have proper reference types, so just check the value
                        let _value_type = self.check_expression(value)?;
                        // TODO: Check that ptr_type is actually a reference to value_type
                        Ok(())
                    }
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                // Type check the condition - must be Bool
                let cond_type = self.check_expression(condition)?;
                if cond_type != CheckerType::Bool {
                    return Err(CompileError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: cond_type.to_string(),
                        span: None,
                    });
                }

                // Type check then branch in new scope
                self.symbols.enter_scope();
                for stmt in then_branch {
                    self.check_statement(stmt)?;
                }
                self.symbols.exit_scope();

                // Type check else branch in new scope if it exists
                if let Some(else_stmts) = else_branch {
                    self.symbols.enter_scope();
                    for stmt in else_stmts {
                        self.check_statement(stmt)?;
                    }
                    self.symbols.exit_scope();
                }

                Ok(())
            }
            Stmt::While {
                condition, body, ..
            } => {
                // Type check the condition - must be Bool
                let cond_type = self.check_expression(condition)?;
                if cond_type != CheckerType::Bool {
                    return Err(CompileError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: cond_type.to_string(),
                        span: None,
                    });
                }

                // Type check body in new scope with incremented loop depth
                self.symbols.enter_scope();
                self.enter_loop(BreakTarget::Statement);
                let result = (|| -> Result<()> {
                    for stmt in body {
                        self.check_statement(stmt)?;
                    }
                    Ok(())
                })();
                self.exit_loop();
                self.symbols.exit_scope();

                result
            }
            Stmt::Loop { body, .. } => {
                // A `loop` written for its effect. Its `break`s may not carry a
                // value: there is no binding on the other side to receive one,
                // and evaluating an expression only to drop it is the silent
                // half of a defect rather than a feature.
                self.symbols.enter_scope();
                self.enter_loop(BreakTarget::Statement);
                let result = (|| -> Result<()> {
                    for stmt in body {
                        self.check_statement(stmt)?;
                    }
                    Ok(())
                })();
                self.exit_loop();
                self.symbols.exit_scope();

                result
            }
            Stmt::For {
                var, iter, body, ..
            } => {
                // Type check the iterator expression
                let iter_type = self.check_expression(iter)?;

                // Extract element type from array
                let elem_type = match iter_type {
                    CheckerType::Array(elem_type, _size) => elem_type.as_ref().clone(),
                    // A range yields the integers between its ends — whether it
                    // was written in the header or bound by a `let` first.
                    CheckerType::Range => CheckerType::Int,
                    _ => {
                        return Err(CompileError::Generic(format!(
                            "For loop requires an array, found {}",
                            iter_type
                        )));
                    }
                };

                // Enter new scope for loop body
                self.symbols.enter_scope();
                self.enter_loop(BreakTarget::Statement);

                // Define loop variable with element type. `for LIMIT in 0..3`
                // is a fresh binder too, and it silently took the name of a
                // top-level item until this line asked.
                self.refuse_global_shadow(var, "the loop variable")?;
                self.symbols.define(var.clone(), elem_type, false)?;

                // Type check body
                let result = (|| -> Result<()> {
                    for stmt in body {
                        self.check_statement(stmt)?;
                    }
                    Ok(())
                })();

                self.exit_loop();
                self.symbols.exit_scope();

                result
            }
            Stmt::Break { value, span } => {
                if self.loop_depth == 0 {
                    return Err(self.error_helper.control_flow_outside_loop("break"));
                }
                match value {
                    // A VALUELESS `break` OUT OF A VALUE `loop` LEAVES THE
                    // TEMPORARY UNWRITTEN. Measured before this arm existed:
                    // `let x = loop { if c { break; } break 1; };` emitted
                    // `long long __pd_val0;` with no initialiser, a plain
                    // `break` on one path, and then read it — undefined
                    // behaviour in C, from a program the front end accepted.
                    //
                    // The mirror of `record_break_value`'s refusal, which
                    // already rejected a VALUED break out of a statement loop.
                    // Both directions of the same rule: the loop and its breaks
                    // must agree about whether there is a value.
                    None => match self.break_targets.last() {
                        Some(BreakTarget::Value(_)) => Err(CompileError::TypeMismatch {
                            expected: "every `break` out of a `loop` used as a value to carry \
                                       one, so the binding on the other side is always written"
                                .to_string(),
                            found: "a `break` with no value".to_string(),
                            span: Some(*span),
                        }),
                        _ => Ok(()),
                    },
                    Some(expr) => {
                        let value_type = self.check_expression(expr)?;
                        self.record_break_value(value_type, *span)
                    }
                }
            }
            Stmt::Continue { .. } => {
                if self.loop_depth == 0 {
                    return Err(self.error_helper.control_flow_outside_loop("continue"));
                }
                Ok(())
            }
            Stmt::Match {
                expr, arms, span, ..
            } => {
                // Type check the match expression
                let expr_type = self.check_expression(expr)?;

                // For each arm, check the pattern matches the expression type
                // and type check the body
                for arm in arms {
                    // Check pattern compatibility with expression type
                    self.check_pattern(&arm.pattern, &expr_type)?;

                    // Type check arm body in new scope
                    self.symbols.enter_scope();

                    // Bind pattern variables if any
                    let checked = (|| -> Result<()> {
                        self.bind_pattern_variables(&arm.pattern, &expr_type)?;
                        // THE GUARD IS CHECKED INSIDE THE ARM'S SCOPE, after the
                        // bindings: `Num(n) if n > 5` reads `n`, so a guard
                        // checked outside would report an undefined variable for
                        // a name the arm defines (N6-09).
                        self.check_guard(arm.guard.as_ref())?;
                        for stmt in &arm.body {
                            self.check_statement(stmt)?;
                        }
                        Ok(())
                    })();

                    self.symbols.exit_scope();
                    checked?;
                }

                // Pattern exhaustiveness checking
                let patterns = Self::unguarded_patterns(arms.iter().map(|a| (&a.pattern, &a.guard)));
                self.check_match_exhaustiveness(&expr_type, &patterns, *span)?;

                Ok(())
            }
            Stmt::Unsafe { body, .. } => {
                // Enter unsafe context
                self.unsafe_depth += 1;

                // Type check body in new scope
                self.symbols.enter_scope();
                for stmt in body {
                    self.check_statement(stmt)?;
                }
                self.symbols.exit_scope();

                // Exit unsafe context
                self.unsafe_depth -= 1;

                Ok(())
            }
        }
    }

    /// Substitute type parameters in a type with concrete types
    fn substitute_type_params(
        &self,
        ty: &crate::ast::Type,
        type_params: &[String],
        concrete_types: &[CheckerType],
    ) -> Result<CheckerType> {
        match ty {
            crate::ast::Type::TypeParam(name) => {
                // Find the index of this type parameter
                if let Some(idx) = type_params.iter().position(|p| p == name) {
                    if idx < concrete_types.len() {
                        Ok(concrete_types[idx].clone())
                    } else {
                        Err(CompileError::Generic(format!(
                            "Type parameter {} not found in substitution",
                            name
                        )))
                    }
                } else {
                    Err(CompileError::Generic(format!(
                        "Unknown type parameter: {}",
                        name
                    )))
                }
            }
            crate::ast::Type::Custom(name) => {
                // Check if this custom type is actually a type parameter
                if let Some(idx) = type_params.iter().position(|p| p == name) {
                    if idx < concrete_types.len() {
                        Ok(concrete_types[idx].clone())
                    } else {
                        Err(CompileError::Generic(format!(
                            "Type parameter {} not found in substitution",
                            name
                        )))
                    }
                } else {
                    // Not a type parameter, just a regular custom type
                    Ok(CheckerType::from(ty))
                }
            }
            // For other types, just convert normally
            _ => Ok(CheckerType::from(ty)),
        }
    }

    /// Substitute type parameters in a type using a substitution map
    #[allow(clippy::only_used_in_recursion)]
    fn substitute_type_params_map(
        &self,
        ty: &crate::ast::Type,
        substitutions: &std::collections::HashMap<String, crate::ast::Type>,
    ) -> crate::ast::Type {
        match ty {
            crate::ast::Type::TypeParam(name) | crate::ast::Type::Custom(name) => {
                // Check if this is a type parameter that should be substituted
                if let Some(replacement) = substitutions.get(name) {
                    replacement.clone()
                } else {
                    ty.clone()
                }
            }
            crate::ast::Type::Generic { name, args } => {
                // Recursively substitute in generic type arguments
                let new_args: Vec<GenericArg> = args
                    .iter()
                    .map(|arg| match arg {
                        GenericArg::Type(t) => {
                            GenericArg::Type(self.substitute_type_params_map(t, substitutions))
                        }
                        GenericArg::Const(c) => GenericArg::Const(c.clone()), // TODO: substitute const params
                    })
                    .collect();
                crate::ast::Type::Generic {
                    name: name.clone(),
                    args: new_args,
                }
            }
            crate::ast::Type::Array(elem_type, size) => crate::ast::Type::Array(
                Box::new(self.substitute_type_params_map(elem_type, substitutions)),
                size.clone(),
            ),
            crate::ast::Type::Reference {
                lifetime,
                inner,
                mutable,
            } => crate::ast::Type::Reference {
                lifetime: lifetime.clone(),
                inner: Box::new(self.substitute_type_params_map(inner, substitutions)),
                mutable: *mutable,
            },
            // Other types don't contain type parameters
            _ => ty.clone(),
        }
    }

    /// Type check an expression and return its type
    fn check_expression(&mut self, expr: &Expr) -> Result<CheckerType> {
        match expr {
            Expr::String(_) => Ok(CheckerType::String),
            Expr::Integer(_) => Ok(CheckerType::Int),
            Expr::Float(_) => Ok(CheckerType::Float),
            // N4-12. A tuple's type is the tuple of its elements' types — there
            // is nothing to unify, because the elements are not required to
            // agree with each other about anything.
            Expr::Tuple { elements, span } => {
                if elements.len() < 2 {
                    return Err(CompileError::TypeMismatch {
                        expected: "a tuple to have at least two elements".to_string(),
                        found: format!("a tuple with {}", elements.len()),
                        span: Some(*span),
                    });
                }
                let mut element_types = Vec::with_capacity(elements.len());
                for element in elements {
                    element_types.push(self.check_expression(element)?);
                }
                Ok(CheckerType::Tuple(element_types))
            }
            // N4-12. `.0` is SYNTAX: the index is read at compile time, and the
            // type it produces is the one that element has. An out-of-range
            // index is refused with both numbers, because "index out of bounds"
            // without the arity is a message the reader has to go and check.
            Expr::TupleIndex { expr, index, span } => {
                let base = self.check_expression(expr)?;
                let CheckerType::Tuple(element_types) = base else {
                    return Err(CompileError::TypeMismatch {
                        expected: "a tuple, which is what `.0` reads an element of".to_string(),
                        found: base.to_string(),
                        span: Some(*span),
                    });
                };
                element_types.get(*index).cloned().ok_or_else(|| {
                    CompileError::TypeMismatch {
                        expected: format!(
                            "a tuple index below {}, which is this tuple's arity",
                            element_types.len()
                        ),
                        found: format!("`.{}`", index),
                        span: Some(*span),
                    }
                })
            }
            // A CHAR LITERAL IS A `Char` (N4-04), distinct from `Int`, with no
            // implicit conversion in either direction.
            //
            // It could not land alone, and did not: N14 gives `string_char_at`
            // the return `char` and the three `char_is_*` predicates the
            // parameter `char`, and `src/builtins.rs` implemented both over
            // `i64` until N14-04 moved with this. A `char` type on the literal
            // by itself would have made `'a'` unusable with every builtin that
            // consumes a character — the literal would lex, type, and have
            // nowhere to go — and retyped builtins with no literal to feed them
            // would have been unreachable from source.
            //
            // The VALUE is unchanged and still asserted on:
            // `tests/02_types_chars.pd` reads the bytes, not just that it
            // compiles.
            Expr::Char(_) => Ok(CheckerType::Char),
            Expr::Bool(_) => Ok(CheckerType::Bool),
            Expr::Ident(name) => {
                // First check if it's a variable
                if let Some(var_info) = self.symbols.lookup(name) {
                    return Ok(var_info.ty.clone());
                }

                // Then check if it's a function
                match self.functions.get(name) {
                    Some(func_type) => Ok(func_type.clone()),
                    None => {
                        // Try to provide helpful suggestions
                        let available_vars = self.get_available_variables();
                        let available_funcs = self.get_available_functions();

                        // Check if it might be a typo for a variable
                        if let Some(suggestion) =
                            crate::errors::suggestions::SuggestionEngine::suggest_similar_name(
                                name,
                                &available_vars,
                            )
                        {
                            return Err(CompileError::Generic(format!(
                                "Undefined variable: '{}'. Did you mean '{}'?",
                                name, suggestion
                            )));
                        }

                        // Check if it might be a typo for a function
                        if let Some(suggestion) =
                            crate::errors::suggestions::SuggestionEngine::suggest_similar_name(
                                name,
                                &available_funcs,
                            )
                        {
                            return Err(CompileError::Generic(format!(
                                "Undefined function: '{}'. Did you mean '{}'?",
                                name, suggestion
                            )));
                        }

                        // No good suggestion found
                        Err(CompileError::Generic(format!(
                            "Undefined variable or function: '{}'",
                            name
                        )))
                    }
                }
            }
            Expr::Call { func, args, span } => {
                // METHOD CALL SYNTAX (N5-17). `x.f(a)` parses as a call whose
                // callee is a field access, and this arm used to refuse every
                // callee that was not a bare identifier.
                //
                // It is REWRITTEN rather than checked in place: `x.f(a)` means
                // `TypeOfX::f(x, a)`, which is a shape this arm already knows
                // how to check completely — argument counts, generic
                // instantiation, built-ins, the lot. Checking it separately
                // would be a second call-checker to keep in step with this one.
                if let Expr::FieldAccess { object, field, .. } = func.as_ref() {
                    let rewritten = self.method_call_as_path_call(object, field, args, *span)?;
                    return self.check_expression(&rewritten);
                }

                // Get function name (for v0.1, only direct calls)
                let func_name = match func.as_ref() {
                    Expr::Ident(name) => name,
                    _ => {
                        return Err(CompileError::Generic(
                            "Indirect function calls not yet supported".to_string(),
                        ))
                    }
                };

                // A built-in the registry describes but marks unsupported is
                // rejected here, with the reason, rather than being allowed through
                // to gcc — where it dies in the generated C with a message about a
                // C type the Palladium programmer has never heard of.
                if let Some(builtin) = crate::builtins::lookup(func_name) {
                    if let Some(reason) = builtin.support.reason() {
                        return Err(CompileError::UnsupportedBuiltin {
                            name: func_name.clone(),
                            reason: reason.to_string(),
                            span: None,
                        });
                    }
                }

                // First check if it's a generic function that needs instantiation
                if let Some(generic_func) = self.generic_functions.get(func_name).cloned() {
                    // Infer type arguments from the call
                    let type_args = self.infer_type_args(&generic_func, args)?;

                    // Create instantiation key
                    let instantiation = FunctionInstantiation {
                        name: func_name.clone(),
                        type_args: type_args.clone(),
                    };

                    // Check if we've already instantiated this combination
                    if let Some(func_type) = self.instantiations.get(&instantiation) {
                        return self.check_call_with_type(func_name, func_type.clone(), args);
                    }

                    // Need to instantiate the generic function
                    let func_type = self.instantiate_generic_function(&generic_func, &type_args)?;
                    self.instantiations.insert(instantiation, func_type.clone());

                    return self.check_call_with_type(func_name, func_type, args);
                }

                // Look up regular function type
                let func_type = match self.functions.get(func_name) {
                    Some(ft) => ft.clone(),
                    None => {
                        // Update error helper with available functions
                        let available_funcs = self.get_available_functions();
                        self.error_helper
                            .update_available(vec![], available_funcs, vec![]);
                        return Err(self.error_helper.undefined_function(func_name, None));
                    }
                };

                // Check function type
                match func_type {
                    CheckerType::Function(param_types, return_type) => {
                        // Check argument count
                        if args.len() != param_types.len() {
                            return Err(CompileError::ArgumentCountMismatch {
                                name: func_name.clone(),
                                expected: param_types.len(),
                                found: args.len(),
                                span: None,
                            });
                        }

                        // Check argument types
                        for (arg, expected_type) in args.iter().zip(param_types.iter()) {
                            let arg_type = self.check_expression(arg)?;
                            if arg_type != *expected_type {
                                return Err(CompileError::TypeMismatch {
                                    expected: expected_type.to_string(),
                                    found: arg_type.to_string(),
                                    span: None,
                                });
                            }
                        }

                        Ok(return_type.as_ref().clone())
                    }
                    _ => Err(CompileError::Generic(format!(
                        "{} is not a function",
                        func_name
                    ))),
                }
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                let left_type = self.check_expression(left)?;
                let right_type = self.check_expression(right)?;

                match op {
                    BinOp::Add => {
                        // Addition can work for both Int and String (concatenation)
                        match (&left_type, &right_type) {
                            (CheckerType::Int, CheckerType::Int) => Ok(CheckerType::Int),
                            (CheckerType::Float, CheckerType::Float) => Ok(CheckerType::Float),
                            (CheckerType::String, CheckerType::String) => Ok(CheckerType::String),
                            _ => {
                                // For Add, we expect both operands to have the same type
                                if left_type == CheckerType::String {
                                    Err(CompileError::TypeMismatch {
                                        expected: "String".to_string(),
                                        found: right_type.to_string(),
                                        span: None,
                                    })
                                } else if left_type == CheckerType::Int
                                    || left_type == CheckerType::Float
                                {
                                    // NO IMPLICIT WIDENING. `1 + 2.5` is a type
                                    // error and not an f64: C would convert
                                    // silently, and a language with no `as`
                                    // cast yet (N5, owed) would then have a
                                    // conversion nobody can see and nobody can
                                    // write. Naming the two types is the whole
                                    // diagnostic.
                                    Err(CompileError::TypeMismatch {
                                        expected: left_type.to_string(),
                                        found: right_type.to_string(),
                                        span: None,
                                    })
                                } else {
                                    Err(CompileError::TypeMismatch {
                                        expected: "Int, Float or String".to_string(),
                                        found: left_type.to_string(),
                                        span: None,
                                    })
                                }
                            }
                        }
                    }
                    BinOp::Sub | BinOp::Mul | BinOp::Div => {
                        // Arithmetic over one numeric type, with no mixing.
                        match (&left_type, &right_type) {
                            (CheckerType::Int, CheckerType::Int) => Ok(CheckerType::Int),
                            (CheckerType::Float, CheckerType::Float) => Ok(CheckerType::Float),
                            (CheckerType::Int, _) | (CheckerType::Float, _) => {
                                Err(CompileError::TypeMismatch {
                                    expected: left_type.to_string(),
                                    found: right_type.to_string(),
                                    span: None,
                                })
                            }
                            _ => Err(CompileError::TypeMismatch {
                                expected: "Int or Float".to_string(),
                                found: left_type.to_string(),
                                span: None,
                            }),
                        }
                    }
                    BinOp::Mod => {
                        // `%` is integer-only. C's `%` does not accept a double
                        // at all (it is `fmod`, a library call), so accepting
                        // Float here would emit C that gcc rejects — the class
                        // of defect D5 was closed for.
                        if left_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "Int".to_string(),
                                found: left_type.to_string(),
                                span: None,
                            });
                        }
                        if right_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "Int".to_string(),
                                found: right_type.to_string(),
                                span: None,
                            });
                        }
                        Ok(CheckerType::Int)
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                        // Comparison operations require same types
                        if left_type != right_type {
                            return Err(CompileError::TypeMismatch {
                                expected: left_type.to_string(),
                                found: right_type.to_string(),
                                span: None,
                            });
                        }
                        // Comparison operations return Bool
                        Ok(CheckerType::Bool)
                    }
                    BinOp::And | BinOp::Or => {
                        // Logical operations require both operands to be Bool
                        if left_type != CheckerType::Bool {
                            return Err(CompileError::TypeMismatch {
                                expected: "Bool".to_string(),
                                found: left_type.to_string(),
                                span: None,
                            });
                        }
                        if right_type != CheckerType::Bool {
                            return Err(CompileError::TypeMismatch {
                                expected: "Bool".to_string(),
                                found: right_type.to_string(),
                                span: None,
                            });
                        }
                        Ok(CheckerType::Bool)
                    }
                    // N5-12. INTEGER OPERANDS ONLY, both sides, no exceptions.
                    // C would happily accept a `double` for `&` after an
                    // implicit conversion nobody wrote, and `bool` operands
                    // would make `true & 2` a number; the specification names
                    // these as bitwise operators, and bits are what integers
                    // have.
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                        if left_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: format!(
                                    "an integer on the left of `{}`, which is a bitwise operator",
                                    op
                                ),
                                found: left_type.to_string(),
                                span: None,
                            });
                        }
                        if right_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: format!(
                                    "an integer on the right of `{}`, which is a bitwise operator",
                                    op
                                ),
                                found: right_type.to_string(),
                                span: None,
                            });
                        }
                        Ok(CheckerType::Int)
                    }
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                if elements.is_empty() {
                    return Err(CompileError::Generic(
                        "Empty array literals are not supported (cannot infer type)".to_string(),
                    ));
                }

                // Type check first element
                let elem_type = self.check_expression(&elements[0])?;

                // Check that all elements have the same type
                for elem in &elements[1..] {
                    let elem_expr_type = self.check_expression(elem)?;
                    if elem_expr_type != elem_type {
                        return Err(CompileError::TypeMismatch {
                            expected: elem_type.to_string(),
                            found: elem_expr_type.to_string(),
                            span: None,
                        });
                    }
                }

                Ok(CheckerType::Array(
                    Box::new(elem_type),
                    ArraySizeValue::Literal(elements.len()),
                ))
            }
            Expr::ArrayRepeat { value, count, .. } => {
                // Type check the value
                let elem_type = self.check_expression(value)?;

                // Type check the count - must be an integer literal
                match count.as_ref() {
                    Expr::Integer(n) => {
                        if *n < 0 {
                            return Err(CompileError::Generic(
                                "Array size must be non-negative".to_string(),
                            ));
                        }
                        Ok(CheckerType::Array(
                            Box::new(elem_type),
                            ArraySizeValue::Literal(*n as usize),
                        ))
                    }
                    _ => Err(CompileError::Generic(
                        "Array repeat count must be an integer literal".to_string(),
                    )),
                }
            }
            Expr::Index { array, index, .. } => {
                // Type check the array expression
                let array_type = self.check_expression(array)?;

                // Type check the index expression (must be Int)
                let index_type = self.check_expression(index)?;
                if index_type != CheckerType::Int {
                    return Err(CompileError::TypeMismatch {
                        expected: "Int".to_string(),
                        found: index_type.to_string(),
                        span: None,
                    });
                }

                // Extract element type from array type
                match array_type {
                    CheckerType::Array(elem_type, _size) => Ok(elem_type.as_ref().clone()),
                    _ => Err(CompileError::Generic(format!(
                        "Cannot index into non-array type: {}",
                        array_type
                    ))),
                }
            }
            Expr::StructLiteral { name, fields, .. } => {
                // First check if this is a generic struct
                if let Some(generic_struct) = self.generic_structs.get(name).cloned() {
                    // For generic structs, we need to infer type parameters from field values
                    let mut type_substitutions: HashMap<String, CheckerType> = HashMap::new();

                    // First pass: check that all provided fields are valid
                    for (field_name, _) in fields {
                        // Find the field's declared type in the generic struct
                        generic_struct
                            .fields
                            .iter()
                            .find(|(fname, _)| fname == field_name)
                            .ok_or_else(|| {
                                CompileError::Generic(format!(
                                    "Unknown field '{}' for struct '{}'",
                                    field_name, name
                                ))
                            })?;
                    }

                    // Second pass: collect type constraints from field values
                    for (field_name, field_expr) in fields {
                        // Find the field's declared type in the generic struct
                        let field_type = generic_struct
                            .fields
                            .iter()
                            .find(|(fname, _)| fname == field_name)
                            .map(|(_, ftype)| ftype)
                            .unwrap(); // Safe because we checked in first pass

                        // Type check the field expression
                        let provided_type = self.check_expression(field_expr)?;

                        // If the field type is a type parameter, record the constraint
                        if let crate::ast::Type::TypeParam(param_name) = field_type {
                            if generic_struct.type_params.contains(param_name) {
                                // Check if we already have a constraint for this type parameter
                                if let Some(existing_type) = type_substitutions.get(param_name) {
                                    if *existing_type != provided_type {
                                        return Err(CompileError::Generic(format!(
                                            "Conflicting type constraints for type parameter '{}': {} vs {}",
                                            param_name, existing_type, provided_type
                                        )));
                                    }
                                } else {
                                    type_substitutions.insert(param_name.clone(), provided_type);
                                }
                            }
                        } else if let crate::ast::Type::Custom(type_name) = field_type {
                            // Check if it's a type parameter referenced as Custom type
                            if generic_struct.type_params.contains(type_name) {
                                if let Some(existing_type) = type_substitutions.get(type_name) {
                                    if *existing_type != provided_type {
                                        return Err(CompileError::Generic(format!(
                                            "Conflicting type constraints for type parameter '{}': {} vs {}",
                                            type_name, existing_type, provided_type
                                        )));
                                    }
                                } else {
                                    type_substitutions.insert(type_name.clone(), provided_type);
                                }
                            }
                        }
                        // TODO: Handle nested generic types like Box<T> where T needs to be inferred
                    }

                    // Check that all type parameters have been inferred
                    let mut inferred_args = Vec::new();
                    for type_param in &generic_struct.type_params {
                        match type_substitutions.get(type_param) {
                            Some(inferred_type) => {
                                inferred_args.push(inferred_type.clone());
                            }
                            None => {
                                return Err(CompileError::Generic(format!(
                                    "Could not infer type parameter '{}' for struct '{}'",
                                    type_param, name
                                )));
                            }
                        }
                    }

                    // Check that all required fields are provided
                    for (field_name, field_type) in &generic_struct.fields {
                        let provided_expr = fields
                            .iter()
                            .find(|(fname, _)| fname == field_name)
                            .map(|(_, expr)| expr)
                            .ok_or_else(|| {
                                CompileError::Generic(format!(
                                    "Missing field '{}' in struct literal",
                                    field_name
                                ))
                            })?;

                        // Substitute type parameters in the field type
                        let concrete_checker_type = self.substitute_type_params(
                            field_type,
                            &generic_struct.type_params,
                            &inferred_args,
                        )?;

                        // Type check the field with the concrete type
                        let provided_type = self.check_expression(provided_expr)?;
                        if provided_type != concrete_checker_type {
                            return Err(CompileError::TypeMismatch {
                                expected: concrete_checker_type.to_string(),
                                found: provided_type.to_string(),
                                span: None,
                            });
                        }
                    }

                    // Track this instantiation for code generation
                    let type_arg_strings: Vec<String> = inferred_args
                        .iter()
                        .map(|ct| {
                            match ct {
                                CheckerType::Int => "i64".to_string(),
                                CheckerType::Bool => "bool".to_string(),
                                CheckerType::String => "String".to_string(),
                                CheckerType::Struct(name) => name.clone(),
                                CheckerType::Generic { name, args } => {
                                    // Handle nested generics like Box<Box<Int>>
                                    let arg_strs: Vec<String> = args
                                        .iter()
                                        .map(|a| match a {
                                            GenericArgValue::Type(t) => match t {
                                                CheckerType::Int => "i64".to_string(),
                                                CheckerType::Bool => "bool".to_string(),
                                                CheckerType::String => "String".to_string(),
                                                CheckerType::Struct(n) => n.clone(),
                                                _ => "Unknown".to_string(),
                                            },
                                            GenericArgValue::Const(c) => match c {
                                                ConstValueResolved::Integer(n) => n.to_string(),
                                                ConstValueResolved::ConstParam(name) => {
                                                    name.clone()
                                                }
                                            },
                                        })
                                        .collect();
                                    format!("{}<{}>", name, arg_strs.join(", "))
                                }
                                _ => "Unknown".to_string(),
                            }
                        })
                        .collect();

                    let instantiation = StructInstantiation {
                        name: name.clone(),
                        type_args: type_arg_strings,
                    };

                    let instantiated_type = CheckerType::Generic {
                        name: name.clone(),
                        args: inferred_args
                            .iter()
                            .map(|t| GenericArgValue::Type(t.clone()))
                            .collect(),
                    };

                    self.struct_instantiations
                        .insert(instantiation, instantiated_type.clone());

                    // Return the generic struct type with inferred type arguments
                    return Ok(instantiated_type);
                }

                // Look up the non-generic struct definition
                let struct_fields = self
                    .structs
                    .get(name)
                    .ok_or_else(|| CompileError::Generic(format!("Unknown struct type: {}", name)))?
                    .clone();

                // Check that all fields are provided and have correct types
                for (field_name, field_type) in &struct_fields {
                    let provided_expr = fields
                        .iter()
                        .find(|(fname, _)| fname == field_name)
                        .map(|(_, expr)| expr)
                        .ok_or_else(|| {
                            CompileError::Generic(format!(
                                "Missing field '{}' in struct literal",
                                field_name
                            ))
                        })?;

                    let provided_type = self.check_expression(provided_expr)?;
                    if provided_type != *field_type {
                        return Err(CompileError::TypeMismatch {
                            expected: field_type.to_string(),
                            found: provided_type.to_string(),
                            span: None,
                        });
                    }
                }

                // Check that no extra fields are provided
                for (provided_name, _) in fields {
                    if !struct_fields
                        .iter()
                        .any(|(fname, _)| fname == provided_name)
                    {
                        return Err(CompileError::Generic(format!(
                            "Unknown field '{}' for struct '{}'",
                            provided_name, name
                        )));
                    }
                }

                Ok(CheckerType::Struct(name.clone()))
            }
            Expr::FieldAccess { object, field, .. } => {
                // Type check the object expression
                let object_type = self.check_expression(object)?;

                match &object_type {
                    // Handle non-generic structs
                    CheckerType::Struct(name) => {
                        // Look up the struct fields
                        let fields = self.structs.get(name).ok_or_else(|| {
                            CompileError::Generic(format!("Unknown struct type: {}", name))
                        })?;

                        // Find the field type
                        let field_type = fields
                            .iter()
                            .find(|(fname, _)| fname == field)
                            .map(|(_, ftype)| ftype.clone())
                            .ok_or_else(|| {
                                CompileError::Generic(format!(
                                    "Struct '{}' has no field '{}'",
                                    name, field
                                ))
                            })?;

                        Ok(field_type)
                    }
                    // Handle generic struct instances
                    CheckerType::Generic { name, args } => {
                        // Look up the generic struct definition
                        let generic_struct = self.generic_structs.get(name).ok_or_else(|| {
                            CompileError::Generic(format!("Unknown generic struct type: {}", name))
                        })?;

                        // Find the field's declared type
                        let field_type = generic_struct
                            .fields
                            .iter()
                            .find(|(fname, _)| fname == field)
                            .map(|(_, ftype)| ftype)
                            .ok_or_else(|| {
                                CompileError::Generic(format!(
                                    "Struct '{}' has no field '{}'",
                                    name, field
                                ))
                            })?;

                        // Extract types from generic args
                        let concrete_types: Vec<CheckerType> = args
                            .iter()
                            .filter_map(|arg| match arg {
                                GenericArgValue::Type(t) => Some(t.clone()),
                                _ => None,
                            })
                            .collect();

                        // Substitute type parameters in the field type
                        let concrete_field_type = self.substitute_type_params(
                            field_type,
                            &generic_struct.type_params,
                            &concrete_types,
                        )?;

                        Ok(concrete_field_type)
                    }
                    _ => Err(CompileError::Generic(format!(
                        "Cannot access field on non-struct type: {}",
                        object_type
                    ))),
                }
            }
            Expr::EnumConstructor {
                enum_name,
                variant,
                data,
                span,
            } => {
                // `Type::method(args)` ARRIVES HERE, not at `Expr::Call`
                // (N5-17). The parser builds every `A::b(...)` as an enum
                // constructor because it has no types to tell them apart; the
                // enum table does, and a name that is not an enum's is a path
                // to a function. Before this, `Rect::area(r)` — the call form
                // the specification itself recommends — was refused with
                // "Undefined enum type: Rect".
                if !self.path_names_an_enum(enum_name) {
                    let call_args = match data {
                        Some(crate::ast::EnumConstructorData::Tuple(exprs)) => exprs.clone(),
                        // `Type::CONST` and `Type::name { .. }` are not calls,
                        // and there is nothing else they could be, so they fall
                        // through to the enum error below, which names the
                        // missing enum.
                        _ => Vec::new(),
                    };
                    if matches!(data, Some(crate::ast::EnumConstructorData::Tuple(_))) {
                        let qualified = format!("{}::{}", enum_name, variant);
                        if self.generic_functions.contains_key(&qualified)
                            && !self.functions.contains_key(&qualified)
                        {
                            // Same refusal as the `x.f()` rewrite, at the other
                            // spelling of the same call — see
                            // `method_call_as_path_call`.
                            return Err(CompileError::Generic(format!(
                                "`{}::{}` is a generic method, and generic methods are not \
                                 implemented: code generation emits no symbol for one, so the \
                                 call would fail at link time. Write a non-generic method, or \
                                 a free `fn` with the type parameter",
                                enum_name, variant
                            )));
                        }
                        if self.functions.contains_key(&qualified)
                            || self.generic_functions.contains_key(&qualified)
                        {
                            return self.check_expression(&Expr::Call {
                                func: Box::new(Expr::Ident(qualified)),
                                args: call_args,
                                span: *span,
                            });
                        }
                    }
                }

                // A GENERIC ENUM'S CONSTRUCTOR HAS NO SYMBOL TO CALL.
                // PRE-EXISTING, older than this branch: code generation skips
                // generic enums at every emission site — no typedef, no tag
                // constants, no `_new` constructor — while this arm typed the
                // constructor happily. `Holder::Full(7)` reached the C compiler
                // as a call to `Holder_Full__new`, a function nothing emits, and
                // a `match` on it named `__Holder__Empty`, a constant nothing
                // defines. Refused by name, exactly as a generic METHOD is, and
                // for the same reason: the front end must not approve C the
                // backend never writes.
                if self.generic_enums.contains_key(enum_name) {
                    return Err(CompileError::Generic(format!(
                        "`{}::{}` constructs a variant of a GENERIC enum, and generic enums are \
                         not implemented: code generation emits no type, no tag and no \
                         constructor for one, so this would fail in the C compiler. Declare a \
                         non-generic enum for each concrete type you need",
                        enum_name, variant
                    )));
                }

                // Type check enum constructors
                // First check if the enum exists (could be generic or regular)
                if let Some(generic_enum) = self.generic_enums.get(enum_name).cloned() {
                    // Handle generic enum - infer type parameters from constructor arguments
                    let mut inferred_types = Vec::new();

                    // Find the variant in the generic enum definition
                    let variant_data = generic_enum
                        .variants
                        .iter()
                        .find(|(v_name, _)| v_name == variant)
                        .map(|(_, data)| data)
                        .ok_or_else(|| {
                            CompileError::Generic(format!(
                                "Unknown variant {}::{}",
                                enum_name, variant
                            ))
                        })?;

                    // Infer type parameters from constructor arguments
                    match (variant_data, data.as_ref()) {
                        (
                            crate::ast::EnumVariantData::Tuple(param_types),
                            Some(crate::ast::EnumConstructorData::Tuple(arg_exprs)),
                        ) => {
                            // For each type parameter in the variant, infer from arguments
                            for (param_type, arg_expr) in param_types.iter().zip(arg_exprs) {
                                let arg_type = self.check_expression(arg_expr)?;

                                // If the parameter type is a type parameter, record the inferred type
                                let is_type_param = match param_type {
                                    crate::ast::Type::TypeParam(param_name) => Some(param_name),
                                    crate::ast::Type::Custom(param_name)
                                        if generic_enum.type_params.contains(param_name) =>
                                    {
                                        Some(param_name)
                                    }
                                    _ => None,
                                };

                                if let Some(param_name) = is_type_param {
                                    // Find the index of this type parameter
                                    if let Some(idx) = generic_enum
                                        .type_params
                                        .iter()
                                        .position(|p| p == param_name)
                                    {
                                        // Ensure we have enough slots
                                        while inferred_types.len() <= idx {
                                            inferred_types.push(CheckerType::Unit);
                                            // placeholder
                                        }
                                        inferred_types[idx] = arg_type;
                                    }
                                }
                            }
                        }
                        _ => {
                            // For other cases, we can't infer yet
                            // Return a basic enum type for now
                        }
                    }

                    // If we inferred any types, return a generic type
                    if !inferred_types.is_empty() {
                        return Ok(CheckerType::Generic {
                            name: enum_name.clone(),
                            args: inferred_types
                                .iter()
                                .map(|t| GenericArgValue::Type(t.clone()))
                                .collect(),
                        });
                    } else {
                        // No type parameters inferred, return basic enum
                        return Ok(CheckerType::Enum(enum_name.clone()));
                    }
                }

                if !self.enums.contains_key(enum_name) {
                    return Err(CompileError::Generic(format!(
                        "Undefined enum type: {}",
                        enum_name
                    )));
                }

                // Find the variant
                let variant_info = self.enums[enum_name]
                    .iter()
                    .find(|v| &v.name == variant)
                    .cloned()
                    .ok_or_else(|| {
                        CompileError::Generic(format!("Unknown variant {}::{}", enum_name, variant))
                    })?;

                // Type check the constructor data based on variant fields
                match (&variant_info.fields, data.as_ref()) {
                    (EnumVariantFields::Unit, None) => {
                        // Unit variant with no data - correct
                    }
                    (EnumVariantFields::Unit, Some(_)) => {
                        // Unit variants shouldn't have constructor data
                        return Err(CompileError::Generic(format!(
                            "Unit variant {}::{} cannot have constructor data",
                            enum_name, variant
                        )));
                    }
                    (
                        EnumVariantFields::Tuple(expected_types),
                        Some(crate::ast::EnumConstructorData::Tuple(exprs)),
                    ) => {
                        // Check tuple constructor
                        if expected_types.len() != exprs.len() {
                            return Err(CompileError::Generic(format!(
                                "Wrong number of arguments for {}::{}: expected {}, found {}",
                                enum_name,
                                variant,
                                expected_types.len(),
                                exprs.len()
                            )));
                        }

                        // Type check each expression
                        for (expected, expr) in expected_types.iter().zip(exprs) {
                            let expr_type = self.check_expression(expr)?;
                            if &expr_type != expected {
                                return Err(CompileError::TypeMismatch {
                                    expected: expected.to_string(),
                                    found: expr_type.to_string(),
                                    span: None,
                                });
                            }
                        }
                    }
                    (
                        EnumVariantFields::Named(expected_fields),
                        Some(crate::ast::EnumConstructorData::Struct(field_exprs)),
                    ) => {
                        // Check named constructor
                        if expected_fields.len() != field_exprs.len() {
                            return Err(CompileError::Generic(format!(
                                "Wrong number of fields for {}::{}: expected {}, found {}",
                                enum_name,
                                variant,
                                expected_fields.len(),
                                field_exprs.len()
                            )));
                        }

                        // Type check each field
                        for (field_name, expr) in field_exprs {
                            let expected_type = expected_fields
                                .iter()
                                .find(|(name, _)| name == field_name)
                                .map(|(_, ty)| ty)
                                .ok_or_else(|| {
                                    CompileError::Generic(format!(
                                        "Unknown field {} in {}::{}",
                                        field_name, enum_name, variant
                                    ))
                                })?;

                            let expr_type = self.check_expression(expr)?;
                            if &expr_type != expected_type {
                                return Err(CompileError::TypeMismatch {
                                    expected: expected_type.to_string(),
                                    found: expr_type.to_string(),
                                    span: None,
                                });
                            }
                        }
                    }
                    _ => {
                        return Err(CompileError::Generic(format!(
                            "Mismatched constructor style for {}::{}",
                            enum_name, variant
                        )));
                    }
                }

                Ok(CheckerType::Enum(enum_name.clone()))
            }
            Expr::Range { start, end, .. } => {
                // Type check start and end expressions
                let start_type = self.check_expression(start)?;
                let end_type = self.check_expression(end)?;

                // Both must be integers
                if start_type != CheckerType::Int {
                    return Err(CompileError::TypeMismatch {
                        expected: "Int".to_string(),
                        found: start_type.to_string(),
                        span: None,
                    });
                }
                if end_type != CheckerType::Int {
                    return Err(CompileError::TypeMismatch {
                        expected: "Int".to_string(),
                        found: end_type.to_string(),
                        span: None,
                    });
                }

                Ok(CheckerType::Range)
            }
            Expr::Unary { op, operand, .. } => {
                let operand_type = self.check_expression(operand)?;

                match op {
                    UnaryOp::Neg => {
                        // Negation requires operand to be Int
                        if operand_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "Int".to_string(),
                                found: operand_type.to_string(),
                                span: None,
                            });
                        }
                        Ok(CheckerType::Int)
                    }
                    UnaryOp::Not => {
                        // Logical not requires operand to be Bool
                        if operand_type != CheckerType::Bool {
                            return Err(CompileError::TypeMismatch {
                                expected: "Bool".to_string(),
                                found: operand_type.to_string(),
                                span: None,
                            });
                        }
                        Ok(CheckerType::Bool)
                    }
                    UnaryOp::BitNot => {
                        // `~` flips bits, so it wants the thing that has them.
                        // Kept distinct from `!` on purpose: folding the two
                        // together would make `!0` and `~0` the same
                        // expression with two different answers.
                        if operand_type != CheckerType::Int {
                            return Err(CompileError::TypeMismatch {
                                expected: "an integer operand for `~`, which flips its bits"
                                    .to_string(),
                                found: operand_type.to_string(),
                                span: None,
                            });
                        }
                        Ok(CheckerType::Int)
                    }
                }
            }
            Expr::Reference {
                mutable: _, expr, ..
            } => {
                // Type check the inner expression
                let inner_type = self.check_expression(expr)?;

                // For now, references have the same type as their inner value
                // TODO: Proper reference type handling
                Ok(inner_type)
            }
            Expr::Deref { expr, .. } => {
                // `*self` IS NOT A SECOND LEVEL OF INDIRECTION. Code generation already
                // dereferences a reference receiver on every field access, so a written
                // `*self` emitted `(*((*self))).n` -- two dereferences of one pointer --
                // and gcc refused it: "indirection requires pointer operand ('struct C'
                // invalid)". The front end had approved it, which is the one outcome no
                // program may reach. Refused here rather than given a meaning, because the
                // meaning it would need is a second reference this language cannot spell.
                if self.current_self_receiver.is_some() {
                    if let Expr::Ident(n) = expr.as_ref() {
                        if n == "self" {
                            return Err(CompileError::Generic(
                                "`*self` is not a place: the receiver is already a reference and \
                                 its fields are reached with `self.f`. Writing `*self` asked for a \
                                 second dereference, which reached gcc as an indirection on a \
                                 non-pointer"
                                    .to_string(),
                            ));
                        }
                    }
                }
                // Type check the expression being dereferenced
                let expr_type = self.check_expression(expr)?;

                // For now, assume dereference returns the same type
                // TODO: Proper reference type handling - should check that expr_type is a reference
                Ok(expr_type)
            }
            // D5. `?` and `.await` parse, but no backend lowers either: the C
            // emitter produced a `struct Result` layout nothing else emits, and
            // an await that calls a `poll` member no generated struct has, so a
            // program that satisfied the old type rules failed inside gcc —
            // against C the user never wrote. The LLVM backend WAS worse: a
            // catch-all returned the constant `0` for both nodes, which compiled
            // and was wrong. That catch-all is gone (D10 made each of the four
            // its own refusal), and it is described here without a citation form
            // because there is no live line to cite. Refusing here, at the
            // construct's own span, is what kept either backend from getting the
            // chance.
            //
            // Refusing in this phase rather than only in codegen is deliberate:
            // it lands before the codegen `let`-inference error (D7), which
            // would otherwise suggest a type annotation that cannot help.
            //
            // The old type rules are deleted, not preserved: they encoded a
            // `Result` shape that a real implementation must not reuse, and git
            // holds them if they are ever wanted.
            Expr::Question { expr: _, span } => Err(CompileError::question_unimplemented(*span)),
            Expr::MacroInvocation { .. } => {
                // Macros should have been expanded before type checking
                Err(CompileError::Generic(
                    "Unexpected macro invocation in type checking - macros should be expanded before this phase".to_string()
                ))
            }
            Expr::Await { expr: _, span } => Err(CompileError::await_unimplemented(*span)),
            Expr::If {
                condition,
                then_branch,
                then_value,
                else_branch,
                else_value,
                span,
            } => {
                let cond_type = self.check_expression(condition)?;
                if cond_type != CheckerType::Bool {
                    return Err(CompileError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: cond_type.to_string(),
                        span: Some(*span),
                    });
                }

                // AN `if` USED AS A VALUE NEEDS AN `else`. Not a parse error,
                // because the parser cannot see the difference: the same tokens
                // are a perfectly good statement one line up. The refusal has
                // to say which of the two readings failed, so it is stated
                // here, where "this `if` is in value position" is a fact rather
                // than a guess.
                if else_branch.is_none() {
                    return Err(CompileError::TypeMismatch {
                        expected: "an `if` used as a value to have an `else` branch, so every \
                                   path produces one"
                            .to_string(),
                        found: "an `if` with no `else`".to_string(),
                        span: Some(*span),
                    });
                }

                let then_type =
                    self.check_value_block(then_branch, then_value.as_deref(), *span)?;
                let else_type = self.check_value_block(
                    else_branch.as_deref().unwrap_or(&[]),
                    else_value.as_deref(),
                    *span,
                )?;

                // ONE TYPE, NOT TWO. The value is stored in a single
                // hoisted temporary (see `src/codegen/mod.rs`), so branches
                // that disagree have no C declaration to be given.
                if then_type != else_type {
                    return Err(CompileError::TypeMismatch {
                        expected: format!("both branches of this `if` to have type {}", then_type),
                        found: format!("an `else` branch of type {}", else_type),
                        span: Some(*span),
                    });
                }

                Ok(then_type)
            }
            Expr::Block { stmts, value, span } => {
                self.check_value_block(stmts, value.as_deref(), *span)
            }
            Expr::Cast { expr, ty, span } => {
                let from = self.check_expression(expr)?;
                let to = CheckerType::from(ty);

                // THE LEGAL CAST SET IS NARROW ON PURPOSE.
                //
                // The specification names `as` casts (language-spec.md N5) and
                // grammar.ebnf gives the form, and NEITHER says which
                // conversions are meant. So the set here is the one that has an
                // unambiguous meaning in the target language — conversions
                // among the numeric primitives and `bool` — and everything else
                // is refused BY NAME rather than guessed at. A cast from
                // `String` to `i64` in particular would be a pointer
                // reinterpreted as a number in C: it would compile, run, and
                // print an address.
                // `char` JOINS THE SET (N4-04) BUT NOT THE WHOLE OF IT, and
                // the restriction follows the same rule the paragraph above
                // states rather than adding an exception to it. `char` pairs
                // with `i64` ONLY, because the code-point correspondence is
                // exactly what the type is defined by — with no implicit
                // conversion in either direction, `as` is the only way to
                // cross, and `print_int(c as i64)` is the sanctioned way to
                // print one.
                //
                // The other pairings have no unambiguous meaning to give them.
                // MEASURED before this restriction: `3.7 as char` compiled and
                // produced 3, `true as char` produced 1, `'a' as bool`
                // produced true. Is a letter truthy? Is a fraction of a
                // character the third one? Neither question has an answer this
                // specification gives, so they are refused by name.
                let numeric = |t: &CheckerType| {
                    matches!(t, CheckerType::Int | CheckerType::Float | CheckerType::Bool)
                };
                let legal = match (&from, &to) {
                    (CheckerType::Char, CheckerType::Char) => true,
                    (CheckerType::Char, t) | (t, CheckerType::Char) => {
                        matches!(t, CheckerType::Int)
                    }
                    (f, t) => numeric(f) && numeric(t),
                };
                if !legal {
                    // PD0003. The code is attached HERE, at the site that knows
                    // which rule is being enforced, because `TypeMismatch` is
                    // raised from dozens of places enforcing different rules and
                    // the variant cannot answer for any of them. The four corpus
                    // witnesses share this one code: `legal`'s middle arm is a
                    // symmetric or-pattern, so direction is never a branch — it
                    // exists only in the formatted `found` clause below, which is
                    // a particular and not identity.
                    return Err(CompileError::TypeMismatch {
                        expected: "a cast among the numeric primitives and `bool`, or between \
                                   `char` and `i64` — `char` crosses only to its code point, \
                                   because that is the only correspondence it is defined by"
                            .to_string(),
                        found: format!("a cast from {} to {}", from, to),
                        span: Some(*span),
                    }
                    .with_code(DiagnosticCode::CastRelation));
                }

                // A LITERAL OPERAND IS KNOWN NOW, SO IT IS REFUSED NOW.
                // `n as char` traps at run time on a value that is not a
                // Unicode scalar (`__pd_char_from_scalar`), because in general
                // the value only exists then. When it is written down, waiting
                // for run time would be a diagnostic the compiler could have
                // given and didn't — so `55296 as char` is a compile error and
                // `tests/reject/char_from_non_scalar.pd` pins it, while the
                // computed case stays with the trap and `tests/n4_char_traps.rs`.
                if matches!(to, CheckerType::Char) {
                    if let Expr::Integer(n) = expr.as_ref() {
                        let scalar = *n;
                        if !(0..=0x10FFFF).contains(&scalar) || (0xD800..=0xDFFF).contains(&scalar)
                        {
                            return Err(CompileError::TypeMismatch {
                                expected: "a Unicode scalar — 0 to 1114111, and not a UTF-16 \
                                           surrogate (55296 to 57343), because a surrogate is \
                                           half of a pair and not a character"
                                    .to_string(),
                                found: format!("`{} as char`", scalar),
                                span: Some(*span),
                            });
                        }
                    }
                }

                Ok(to)
            }
            Expr::Loop { body, span } => {
                // The loop's type is decided by its `break`s, so the frame is
                // pushed EMPTY and read back after the body has been walked.
                self.symbols.enter_scope();
                self.enter_loop(BreakTarget::Value(None));
                let walked = (|| -> Result<()> {
                    for stmt in body {
                        self.check_statement(stmt)?;
                    }
                    Ok(())
                })();
                let target = self.break_targets.pop();
                self.loop_depth -= 1;
                self.symbols.exit_scope();
                walked?;

                match target {
                    Some(BreakTarget::Value(Some(ty))) => Ok(ty),
                    // Either no `break` at all, or only valueless ones. Both
                    // mean the `let` on the other side has nothing to bind —
                    // an infinite loop in value position is not a value, it is
                    // a program that never gets there.
                    _ => Err(CompileError::TypeMismatch {
                        expected: "a `loop` used as a value to be left by a `break` carrying one"
                            .to_string(),
                        found: "a `loop` no `break` gives a value to".to_string(),
                        span: Some(*span),
                    }),
                }
            }
            Expr::Match { expr, arms, span } => {
                if arms.is_empty() {
                    return Err(CompileError::TypeMismatch {
                        expected: "a `match` used as a value to have at least one arm".to_string(),
                        found: "a `match` with no arms".to_string(),
                        span: Some(*span),
                    });
                }

                let scrutinee = self.check_expression(expr)?;

                let mut unified: Option<CheckerType> = None;
                for arm in arms {
                    self.check_pattern(&arm.pattern, &scrutinee)?;

                    self.symbols.enter_scope();
                    let arm_type = (|| {
                        self.bind_pattern_variables(&arm.pattern, &scrutinee)?;
                        self.check_guard(arm.guard.as_ref())?;
                        for stmt in &arm.body {
                            self.check_statement(stmt)?;
                        }
                        match &arm.value {
                            Some(value) => self.check_expression(value),
                            None => Err(CompileError::TypeMismatch {
                                expected: "every arm of a `match` used as a value to end in an \
                                           expression"
                                    .to_string(),
                                found: "an arm whose last statement ends in `;`".to_string(),
                                span: Some(*span),
                            }),
                        }
                    })();
                    self.symbols.exit_scope();
                    let arm_type = arm_type?;

                    match &unified {
                        None => unified = Some(arm_type),
                        Some(first) if *first == arm_type => {}
                        Some(first) => {
                            return Err(CompileError::TypeMismatch {
                                expected: format!(
                                    "every arm of this `match` to have type {}, as the first one \
                                     does",
                                    first
                                ),
                                found: format!("an arm of type {}", arm_type),
                                span: Some(*span),
                            });
                        }
                    }
                }

                // EXHAUSTIVENESS IS THE SAME OBLIGATION AS FOR THE STATEMENT
                // FORM, and strictly sharper here: a statement `match` that
                // matches nothing simply does nothing, while a value one leaves
                // its temporary unwritten. Same checker, so the two cannot
                // drift apart.
                let patterns = Self::unguarded_patterns(arms.iter().map(|a| (&a.pattern, &a.guard)));
                self.check_match_exhaustiveness(&scrutinee, &patterns, *span)?;

                // `unified` is `Some` because `arms` is non-empty and every
                // iteration either sets it or returns.
                unified.ok_or_else(|| CompileError::TypeMismatch {
                    expected: "a `match` used as a value to have at least one arm".to_string(),
                    found: "a `match` with no arms".to_string(),
                    span: Some(*span),
                })
            }
        }
    }

    /// Enum exhaustiveness for a set of match patterns.
    ///
    /// Extracted so the STATEMENT and the VALUE `match` ask the same question
    /// of the same checker. Two copies of this would be two definitions of
    /// "exhaustive", and the value form is the one that cannot survive a wrong
    /// answer — its temporary is simply never written.
    ///
    /// A non-enum scrutinee is not checked, which is the pre-existing position:
    /// there are no literal or range patterns yet (N6), so the only patterns an
    /// `i64` can carry are a wildcard and a binding, and both match everything.
    /// The first name a pattern would bind, if any.
    ///
    /// Used to refuse binders inside an or-pattern with the offending name in
    /// the message: "this alternative binds `x`" is actionable, "or-patterns may
    /// not bind" is a rule the reader has to map onto their own code.
    fn first_binder(pattern: &Pattern) -> Option<&str> {
        match pattern {
            Pattern::Ident(name) => Some(name),
            Pattern::Binding { name, .. } => Some(name),
            Pattern::Wildcard | Pattern::Literal(_) | Pattern::Range { .. } => None,
            Pattern::Or(alternatives) => alternatives.iter().find_map(Self::first_binder),
            Pattern::Tuple(elements) => elements.iter().find_map(Self::first_binder),
            Pattern::EnumPattern { data, .. } => match data {
                None => None,
                Some(PatternData::Tuple(patterns)) => {
                    patterns.iter().find_map(Self::first_binder)
                }
                Some(PatternData::Struct(fields)) => {
                    fields.iter().find_map(|(_, pattern)| Self::first_binder(pattern))
                }
            },
        }
    }

    /// A guard is a condition, so it must be a `bool`.
    ///
    /// Called from inside the arm's scope by both `match` forms, which is what
    /// lets a guard read the pattern's bindings.
    fn check_guard(&mut self, guard: Option<&Expr>) -> Result<()> {
        let Some(guard) = guard else {
            return Ok(());
        };
        let guard_type = self.check_expression(guard)?;
        if guard_type == CheckerType::Bool {
            Ok(())
        } else {
            Err(CompileError::TypeMismatch {
                expected: "a match guard to be a `bool`".to_string(),
                found: guard_type.to_string(),
                span: Some(guard.span()),
            })
        }
    }

    /// The patterns that COUNT toward exhaustiveness: the unguarded ones.
    ///
    /// A GUARDED ARM COVERS NOTHING. `Num(n) if n > 5` is taken only sometimes,
    /// and which times is not decidable from the pattern, so an exhaustiveness
    /// checker that counted it would call a match complete that falls through
    /// at run time — the value form's temporary would then never be written.
    /// Dropping the arm entirely (rather than passing it and ignoring it) also
    /// keeps it from making a later arm look unreachable, which it does not.
    fn unguarded_patterns<'a>(
        arms: impl Iterator<Item = (&'a Pattern, &'a Option<Expr>)>,
    ) -> Vec<Pattern> {
        arms.filter(|(_, guard)| guard.is_none())
            .map(|(pattern, _)| pattern.clone())
            .collect()
    }

    fn check_match_exhaustiveness(
        &self,
        scrutinee: &CheckerType,
        patterns: &[Pattern],
        span: Span,
    ) -> Result<()> {
        // N6-02's one completeness case. `bool` has two values and literal
        // patterns can now name both, so a `match` on one is checkable without
        // a catch-all — the only scrutinee type of which that is true.
        if matches!(scrutinee, CheckerType::Bool) {
            return ExhaustivenessChecker::new(HashMap::new()).check_bool_match(patterns, span);
        }

        let CheckerType::Enum(enum_name) = scrutinee else {
            // N6-10. EVERY OTHER SCRUTINEE TYPE NEEDS AN IRREFUTABLE ARM. The
            // spec's sentence is "a non-exhaustive match is a compile error, not
            // a silent fall-through, and this applies to every scrutinee type,
            // not only enums" — it asks for no interval arithmetic, and this
            // does none: an `i64` match is complete when some arm matches every
            // `i64`, which is what `is_irrefutable` answers.
            //
            // This was an UNCHECKED POSITION until now, which is why the sweep
            // that landed this rule had to repair fixtures: a match with no
            // catch-all compiled, fell through every arm at run time, and left a
            // value-match temporary holding its zero-initialiser.
            if patterns.iter().any(ExhaustivenessChecker::is_irrefutable) {
                return Ok(());
            }
            return Err(CompileError::NonExhaustiveMatch {
                missing_patterns: vec![format!(
                    "a `_` or binding arm — the scrutinee is `{}`, whose values this checker \
                     cannot enumerate, so no set of literal or range arms is complete (coverage \
                     by ranges is not completeness: proving it takes interval arithmetic, which \
                     this checker does not do)",
                    scrutinee
                )],
                span: Some(span),
            });
        };

        let mut enum_infos = HashMap::new();
        for (name, variants) in &self.enums {
            let variant_infos: Vec<VariantInfo> = variants
                .iter()
                .map(|v| {
                    let arity = match &v.fields {
                        EnumVariantFields::Unit => 0,
                        EnumVariantFields::Tuple(types) => types.len(),
                        EnumVariantFields::Named(fields) => fields.len(),
                    };
                    VariantInfo {
                        name: v.name.clone(),
                        arity,
                    }
                })
                .collect();

            enum_infos.insert(
                name.clone(),
                EnumInfo {
                    name: name.clone(),
                    variants: variant_infos,
                },
            );
        }

        ExhaustivenessChecker::new(enum_infos).check_match(enum_name, patterns, span)
    }

    /// Push a loop frame. `loop_depth` and `break_targets` move together —
    /// they are two views of the same stack and a pass that updated one alone
    /// would answer "am I in a loop" and "which loop" differently.
    fn enter_loop(&mut self, target: BreakTarget) {
        self.loop_depth += 1;
        self.break_targets.push(target);
    }

    fn exit_loop(&mut self) {
        self.loop_depth -= 1;
        self.break_targets.pop();
    }

    /// Attribute `break <expr>`'s type to the innermost loop.
    ///
    /// Refuses two things by name: a value handed to a loop that is not in
    /// value position, and a second `break` that disagrees with the first about
    /// the type. The second is not a courtesy — the value is stored in ONE C
    /// temporary, so two types have no declaration to share.
    fn record_break_value(&mut self, value_type: CheckerType, span: Span) -> Result<()> {
        match self.break_targets.last_mut() {
            None => Err(self.error_helper.control_flow_outside_loop("break")),
            Some(BreakTarget::Statement) => Err(CompileError::TypeMismatch {
                expected: "a `break` without a value, because the loop it exits is not used as \
                           a value"
                    .to_string(),
                found: format!("`break` carrying a {}", value_type),
                span: Some(span),
            }),
            Some(BreakTarget::Value(slot)) => match slot {
                None => {
                    *slot = Some(value_type);
                    Ok(())
                }
                Some(existing) if *existing == value_type => Ok(()),
                Some(existing) => Err(CompileError::TypeMismatch {
                    expected: format!(
                        "every `break` out of this `loop` to carry a {}, as the first one does",
                        existing
                    ),
                    found: format!("a `break` carrying a {}", value_type),
                    span: Some(span),
                }),
            },
        }
    }

    /// The name of the type a method call dispatches on, for `TypeName::method`.
    ///
    /// Only NAMED types can carry an `impl`, so anything else has no method to
    /// find and says so rather than producing a qualified name that could not
    /// resolve.
    fn method_owner_name(ty: &CheckerType) -> Option<String> {
        match ty {
            CheckerType::Struct(name) | CheckerType::Enum(name) => Some(name.clone()),
            CheckerType::Generic { name, .. } => Some(name.clone()),
            _ => None,
        }
    }

    /// Rewrite `object.field(args)` into `TypeOfObject::field(object, args)`.
    ///
    /// The receiver becomes the FIRST argument, before the written ones, which
    /// is what makes `self` an ordinary parameter for every pass after this.
    fn method_call_as_path_call(
        &mut self,
        object: &Expr,
        field: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<Expr> {
        let receiver_type = self.check_expression(object)?;

        let Some(owner) = Self::method_owner_name(&receiver_type) else {
            return Err(CompileError::Generic(format!(
                "`{}` has no method `{}`: only a struct or an enum can have an `impl` block, \
                 and the receiver here is {}",
                object, field, receiver_type
            )));
        };

        let qualified = format!("{}::{}", owner, field);

        // A GENERIC METHOD TYPE-CHECKS AND THEN HAS NO SYMBOL TO CALL.
        // `generic_functions` accepts it here, and code generation SKIPS
        // generic impl methods entirely — no definition, no prototype — while
        // the `::` name is mangled before the generic-mangling path is
        // consulted. The result was a call to `__pd_Rect_id`, which nothing
        // emits, discovered by the linker rather than by this compiler.
        // Refused by name until method monomorphisation exists.
        if self.generic_functions.contains_key(&qualified) {
            return Err(CompileError::Generic(format!(
                "`{}::{}` is a generic method, and generic methods are not implemented: \
                 code generation emits no symbol for one, so the call would fail at link \
                 time. Write a non-generic method, or a free `fn` with the type parameter",
                owner, field
            )));
        }

        if !self.functions.contains_key(&qualified)
            && !self.generic_functions.contains_key(&qualified)
        {
            // NAME THE RIGHT FAILURE. A field that exists but is not a
            // function is a different mistake from a method that does not
            // exist, and the two used to arrive as the same message.
            let has_field = self
                .structs
                .get(&owner)
                .is_some_and(|fields| fields.iter().any(|(name, _)| name == field));
            if has_field {
                return Err(CompileError::Generic(format!(
                    "`{}.{}` is a field, not a method: it cannot be called. There is no \
                     `{}` in an `impl {}` block, and a field holding a function is not \
                     callable either — function values do not exist in this language",
                    object, field, field, owner
                )));
            }
            return Err(CompileError::Generic(format!(
                "no method `{}` on `{}`: no `impl {}` block declares `fn {}`",
                field, owner, owner, field
            )));
        }

        // CALLING A MUTATING METHOD THROUGH A NON-MUTATING RECEIVER IS A WRITE.
        //
        // The write rule above guards `Stmt::Assign`, so it sees `self.n = v` and nothing
        // else. `self.bump()` reaches the same field through a callee that declares
        // `&mut self`, and the guarantee the caller's own receiver form states -- that
        // this method does not modify the receiver -- was one call away from vacuous.
        //
        // MEASURED before this refusal: `fn covert(&self) { self.bump(); }` compiled,
        // linked and ran, and the CALLER observed 42. The emitted C is
        // `void __pd_C_covert(const struct C* self) { __pd_C_bump(self); }` against
        // `void __pd_C_bump(struct C* self)`, so the only complaint anywhere is cc's
        // discard-qualifiers warning -- which src/linker.rs:223 tags NON-FATAL because
        // the emitted prelude fires it in 108/108 compiles. The backstop is structurally
        // blind to this, which is why the rule has to be stated here.
        //
        // BOTH non-mutating forms are refused, by the same predicate as the write rule
        // (`!= MutRef`), because the by-value path launders the same guarantee with even
        // less to catch it: `fn take(self) { self.bump(); }` emits
        // `__pd_C_bump(&self)` against the COPY -- no `const` anywhere, so not even a
        // warning -- and it printed 42 from a mutation the caller never saw.
        if let Some(caller_recv) = self.current_self_receiver {
            if caller_recv != SelfReceiver::MutRef
                && self.impl_method_receiver.get(&qualified) == Some(&SelfReceiver::MutRef)
                && Self::expr_base_is_self(object)
            {
                let detail = match caller_recv {
                    SelfReceiver::Shared => {
                        "`&self` is a SHARED borrow of the receiver, and the callee takes \
                         `&mut self`. Take `&mut self` here if this method is meant to \
                         modify the receiver"
                    }
                    _ => {
                        "a by-value `self` receiver is a COPY, and the callee takes \
                         `&mut self`, so it would modify that copy and the caller would not \
                         observe it. Take `&mut self` here if this method is meant to modify \
                         the receiver, or return the new value"
                    }
                };
                return Err(CompileError::Generic(format!(
                    "cannot call `{}::{}` through `self`: {}",
                    owner, field, detail
                )));
            }
        }

        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(object.clone());
        call_args.extend(args.iter().cloned());

        Ok(Expr::Call {
            func: Box::new(Expr::Ident(qualified)),
            args: call_args,
            span,
        })
    }

    /// Is `name` an enum this program declares?
    ///
    /// THE ONE RULE THAT SEPARATES `Color::Red(1)` FROM `Rect::area(r)`, and
    /// the parser cannot apply it: it builds every `A::b(...)` as an enum
    /// constructor because it has no types. Both the type checker and the code
    /// generator ask this same question of their own copy of the enum table,
    /// and the answer decides which of the two the expression is. Code
    /// generation states the rule again where it applies it; if these two ever
    /// disagree, a constructor is emitted as a call or a call as a constructor.
    fn path_names_an_enum(&self, name: &str) -> bool {
        self.enums.contains_key(name) || self.generic_enums.contains_key(name)
    }

    /// Type check `{ stmts...; value }` in VALUE position and return the type
    /// of `value`.
    ///
    /// A block with no trailing expression has no value to give — `()` is not
    /// an answer here, because nothing in this language can hold one, and
    /// accepting it would send codegen looking for a C type for a variable that
    /// stores nothing. So it is refused, and the message names the missing
    /// tail rather than a type mismatch nobody wrote.
    fn check_value_block(
        &mut self,
        stmts: &[Stmt],
        value: Option<&Expr>,
        span: Span,
    ) -> Result<CheckerType> {
        self.symbols.enter_scope();
        let result = (|| {
            for stmt in stmts {
                self.check_statement(stmt)?;
            }
            match value {
                Some(expr) => self.check_expression(expr),
                None => Err(CompileError::TypeMismatch {
                    expected: "this block to end in an expression, so it has a value".to_string(),
                    found: "a block whose last statement ends in `;`".to_string(),
                    span: Some(span),
                }),
            }
        })();
        self.symbols.exit_scope();
        result
    }

    /// Check that a pattern is compatible with the given type
    fn check_pattern(&self, pattern: &Pattern, expected_type: &CheckerType) -> Result<()> {
        match pattern {
            // N6-07. Every alternative is asked the same question, because an
            // arm accepting several shapes still accepts ONE type.
            //
            // AND NO ALTERNATIVE MAY BIND. Rust's rule is that all alternatives
            // bind the same names at the same types; this checker does not know
            // how to verify that yet, and code generation emits an or-pattern as
            // a single `||` condition with no per-alternative site to assign a
            // binder from. Accepting one would mean choosing a branch for the
            // reader. Refused by name instead, and pinned by
            // tests/reject/or_pattern_binds.pd. A binding OUTSIDE the or —
            // `n @ …` — is N6-08 and is fine.
            Pattern::Or(alternatives) => {
                for alternative in alternatives {
                    if let Some(binder) = Self::first_binder(alternative) {
                        return Err(CompileError::TypeMismatch {
                            expected: "an alternative of an `|` pattern to bind nothing"
                                .to_string(),
                            found: format!(
                                "the alternative `{}`, which binds `{}` — every alternative would \
                                 have to bind the same names at the same types for this to mean \
                                 anything, and that is not checked yet",
                                alternative, binder
                            ),
                            span: None,
                        });
                    }
                    self.check_pattern(alternative, expected_type)?;
                }
                Ok(())
            }
            // N6-08. The binding names this position; what it may match is
            // decided by the inner pattern.
            Pattern::Binding { inner, .. } => self.check_pattern(inner, expected_type),
            // N6-05. Arity and element types both, against the tuple this match
            // is on. An arity mismatch is reported with BOTH numbers: "expected a
            // tuple pattern" without them sends the reader to count parentheses.
            Pattern::Tuple(elements) => {
                let CheckerType::Tuple(element_types) = expected_type else {
                    return Err(CompileError::TypeMismatch {
                        expected: format!(
                            "a pattern of type {}, which is what this `match` is on",
                            expected_type
                        ),
                        found: "a tuple pattern".to_string(),
                        span: None,
                    });
                };
                if elements.len() != element_types.len() {
                    return Err(CompileError::TypeMismatch {
                        expected: format!(
                            "a tuple pattern with {} elements, matching the tuple this `match` \
                             is on",
                            element_types.len()
                        ),
                        found: format!("a tuple pattern with {}", elements.len()),
                        span: None,
                    });
                }
                for (element, element_type) in elements.iter().zip(element_types.iter()) {
                    self.check_pattern(element, element_type)?;
                }
                Ok(())
            }
            // N6-03. Three questions, all answerable here and nowhere later:
            // the endpoints must be an ORDERED literal kind and BOTH THE SAME
            // one, the scrutinee's type must be that kind, and the interval must
            // be able to contain something.
            //
            // TWO ORDERED KINDS NOW, NOT ONE. This rule read "the endpoints must
            // be integers", and refused `char` with "(`char` endpoints wait on
            // N4-04, which owes `char` as a type at all)". N4-04 is `satisfied`:
            // `char` is a type, and a char literal is a pattern as of this
            // change, so the refusal outlived its reason. `char` is ordered by
            // CODE POINT, which is the only order the language defines for it,
            // and that is exactly what a range needs. `String` and `bool` stay
            // refused: `bool` has two values and wants no interval, and a string
            // range would be an ordering this language has never defined.
            //
            // MIXED ENDPOINTS ARE REFUSED BY NAME. `'a'..=9` has one end of each
            // kind, and there is no comparison between a scalar and an integer
            // here that is not a coercion N4-04 exists to forbid.
            //
            // AN EMPTY RANGE IS REFUSED RATHER THAN COMPILED. `5..=1`, `3..3` and
            // now `'z'..='a'` can never match, so the arm they head is dead the
            // moment it is written — and a reader who wrote one meant a different
            // pair. Emitting `x >= 5 && x <= 1` and letting the arm never fire is
            // how a typo becomes a silent behaviour change.
            Pattern::Range { lo, hi, inclusive } => {
                // Code point for a `char`, value for an `i64`: the endpoint's
                // ORDER, paired with the type a scrutinee must have to be
                // compared against it.
                let ordered = |literal: &PatternLiteral| match literal {
                    PatternLiteral::Int(v) => Some((*v, CheckerType::Int)),
                    PatternLiteral::Char(c) => Some((*c as i64, CheckerType::Char)),
                    PatternLiteral::Str(_) | PatternLiteral::Bool(_) => None,
                };
                for (literal, which) in [(lo, "low"), (hi, "high")] {
                    if ordered(literal).is_none() {
                        return Err(CompileError::TypeMismatch {
                            expected: "the endpoints of a range pattern to be integer or `char` \
                                       literals, the two literal kinds this language orders"
                                .to_string(),
                            found: format!(
                                "the {} end `{}`",
                                which,
                                Pattern::Literal(literal.clone())
                            ),
                            span: None,
                        });
                    }
                }
                let (low, low_type) = ordered(lo).expect("checked just above");
                let (high, high_type) = ordered(hi).expect("checked just above");
                if low_type != high_type {
                    return Err(CompileError::TypeMismatch {
                        expected: "both endpoints of a range pattern to be the same kind of \
                                   literal"
                            .to_string(),
                        found: format!(
                            "`{}`, whose low end is {} and whose high end is {} — there is no \
                             order between them that is not a conversion `char` does not have",
                            pattern, low_type, high_type
                        ),
                        span: None,
                    });
                }
                if *expected_type != low_type {
                    return Err(CompileError::TypeMismatch {
                        expected: format!(
                            "a pattern of type {}, which is what this `match` is on",
                            expected_type
                        ),
                        found: format!("a range pattern, which matches {}", low_type),
                        span: None,
                    });
                }
                let empty = if *inclusive { low > high } else { low >= high };
                if empty {
                    let unit = if low_type == CheckerType::Char {
                        "code point"
                    } else {
                        "integer"
                    };
                    return Err(CompileError::TypeMismatch {
                        expected: "a range pattern that can match something".to_string(),
                        found: format!(
                            "`{}`, which is empty — no {} is both >= {} and {} {}",
                            pattern,
                            unit,
                            low,
                            if *inclusive { "<=" } else { "<" },
                            high
                        ),
                        span: None,
                    });
                }
                Ok(())
            }
            // N6-02. A literal pattern is an EQUALITY TEST, so its type has to
            // be the scrutinee's: `match n { "x" => … }` on an `i64` compares
            // two things C would happily compare and Palladium must not.
            Pattern::Literal(literal) => {
                let (literal_type, spelling) = match literal {
                    PatternLiteral::Int(v) => (CheckerType::Int, v.to_string()),
                    PatternLiteral::Str(v) => (CheckerType::String, format!("{:?}", v)),
                    PatternLiteral::Bool(v) => (CheckerType::Bool, v.to_string()),
                    PatternLiteral::Char(v) => {
                        (CheckerType::Char, format!("'{}'", v.escape_debug()))
                    }
                };
                if literal_type == *expected_type {
                    Ok(())
                } else {
                    Err(CompileError::TypeMismatch {
                        expected: format!(
                            "a pattern of type {}, which is what this `match` is on",
                            expected_type
                        ),
                        found: format!("the {} literal `{}`", literal_type, spelling),
                        span: None,
                    })
                }
            }
            Pattern::Wildcard => {
                // Wildcard matches any type
                Ok(())
            }
            Pattern::Ident(_) => {
                // Identifier pattern matches any type and binds it
                Ok(())
            }
            Pattern::EnumPattern {
                enum_name,
                variant,
                data,
            } => {
                // Check that the expected type matches the enum
                match expected_type {
                    CheckerType::Enum(name) if name == enum_name => {}
                    CheckerType::Generic { name, .. } if name == enum_name => {
                        // A generic enum's payload types are its type parameters,
                        // which are not resolved here; the constructor for one is
                        // refused before code generation either way.
                        return Ok(());
                    }
                    _ => {
                        return Err(CompileError::TypeMismatch {
                            expected: format!("enum {}", enum_name),
                            found: expected_type.to_string(),
                            span: None,
                        });
                    }
                }

                // AND THE PAYLOAD IS CHECKED TOO. This arm used to stop at the
                // enum's name, which meant every rule this function enforces was
                // skipped one level down: `P::Num(x | 1)` bypassed the
                // or-alternative-may-not-bind refusal and matched every `Num`,
                // `P::Num("a")` on an `i64` payload reached gcc as a type error
                // in OUR C, and `P::Num(true)` compiled to a comparison against
                // 1. A pattern that is checked at the top and unchecked inside is
                // not checked.
                for (sub_pattern, field_type) in self.payload_pattern_types(enum_name, variant, data.as_ref())? {
                    self.check_pattern(sub_pattern, &field_type)?;
                }
                Ok(())
            }
        }
    }

    /// An enum variant's payload sub-patterns, paired with the field types they
    /// apply to.
    ///
    /// ONE lookup shared by the two passes that need it. `bind_pattern_variables`
    /// grew its own copy first, which is how `check_pattern` came to have none:
    /// the binding walk descended into payloads and the CHECKING walk did not.
    fn payload_pattern_types<'p>(
        &self,
        enum_name: &str,
        variant: &str,
        data: Option<&'p PatternData>,
    ) -> Result<Vec<(&'p Pattern, CheckerType)>> {
        let Some(pattern_data) = data else {
            return Ok(Vec::new());
        };
        let Some(variants) = self.enums.get(enum_name) else {
            return Ok(Vec::new());
        };
        let Some(variant_info) = variants.iter().find(|v| v.name == variant) else {
            return Ok(Vec::new());
        };
        Ok(match (pattern_data, &variant_info.fields) {
            (PatternData::Tuple(patterns), EnumVariantFields::Tuple(field_types)) => {
                if patterns.len() != field_types.len() {
                    return Err(CompileError::Generic(format!(
                        "Pattern has wrong number of fields for {}::{}",
                        enum_name, variant
                    )));
                }
                patterns
                    .iter()
                    .zip(field_types.iter().cloned())
                    .collect()
            }
            (PatternData::Struct(field_patterns), EnumVariantFields::Named(expected)) => {
                let mut out = Vec::with_capacity(field_patterns.len());
                for (field_name, pattern) in field_patterns {
                    let Some((_, field_type)) =
                        expected.iter().find(|(name, _)| name == field_name)
                    else {
                        return Err(CompileError::Generic(format!(
                            "Unknown field {} in {}::{}",
                            field_name, enum_name, variant
                        )));
                    };
                    out.push((pattern, field_type.clone()));
                }
                out
            }
            _ => Vec::new(),
        })
    }

    /// Bind variables from patterns to the symbol table
    fn bind_pattern_variables(
        &mut self,
        pattern: &Pattern,
        value_type: &CheckerType,
    ) -> Result<()> {
        match pattern {
            Pattern::Wildcard => {
                // No bindings
                Ok(())
            }
            // A literal binds nothing either — it constrains the value instead
            // of naming it. Nor does a range.
            Pattern::Literal(_) | Pattern::Range { .. } => Ok(()),
            // N6-08. `name @ inner` binds `name` to THIS position's value and
            // then lets `inner` bind whatever it binds under it.
            Pattern::Binding { name, inner } => {
                // `name @ inner` binds `name`, so it is the same question as
                // `Pattern::Ident` one line of syntax over.
                self.refuse_global_shadow(name, "the `@` binding")?;
                self.symbols.define(name.clone(), value_type.clone(), false)?;
                self.bind_pattern_variables(inner, value_type)
            }
            // N6-07. No alternative may bind (refused in `check_pattern`), so
            // there is nothing to define here.
            Pattern::Or(_) => Ok(()),
            // N6-05. Each element binds against its own element type.
            Pattern::Tuple(elements) => {
                let CheckerType::Tuple(element_types) = value_type else {
                    // `check_pattern` refused this shape already; binding
                    // nothing is the honest answer for a pattern that will not
                    // be reached.
                    return Ok(());
                };
                for (element, element_type) in elements.iter().zip(element_types.iter()) {
                    self.bind_pattern_variables(element, element_type)?;
                }
                Ok(())
            }
            Pattern::Ident(name) => {
                // A PATTERN BINDER IS A BINDING, AND THE SAME RULE APPLIES.
                // This is where the shadowing refusal is worth the most: a bare
                // name in pattern position is a FRESH BINDER, not a read, so
                // `match x { LIMIT => 111, _ => 222 }` over a top-level
                // `const LIMIT` always takes the first arm, leaves the second
                // dead, and prints 111 whatever `x` is. Measured, with no
                // diagnostic anywhere. A reader who wrote that meant the
                // comparison.
                self.refuse_global_shadow(name, "the pattern binding")?;
                // Bind the identifier to the value type
                self.symbols.define(
                    name.clone(),
                    value_type.clone(),
                    false, // Pattern bindings are immutable by default
                )?;
                Ok(())
            }
            Pattern::EnumPattern {
                enum_name,
                variant,
                data,
                ..
            } => {
                // Bind variables from nested patterns
                if let Some(pattern_data) = data {
                    // Get enum variant info to determine field types

                    // Handle both regular and generic enums
                    match value_type {
                        CheckerType::Enum(expected_enum) if expected_enum == enum_name => {
                            // Regular enum - check if it's actually a generic enum
                            if self.generic_enums.contains_key(enum_name) {
                                // This shouldn't happen - generic enums should have Generic type
                                return Err(CompileError::Generic(format!(
                                    "Generic enum {} used without type parameters",
                                    enum_name
                                )));
                            }

                            // Handle regular enum
                            let variants = self
                                .enums
                                .get(enum_name)
                                .ok_or_else(|| {
                                    CompileError::Generic(format!(
                                        "Undefined enum type: {}",
                                        enum_name
                                    ))
                                })?
                                .clone();

                            let variant_info = variants
                                .iter()
                                .find(|v| &v.name == variant)
                                .ok_or_else(|| {
                                    CompileError::Generic(format!(
                                        "Unknown variant {}::{}",
                                        enum_name, variant
                                    ))
                                })?
                                .clone();

                            match (pattern_data, &variant_info.fields) {
                                (
                                    PatternData::Tuple(patterns),
                                    EnumVariantFields::Tuple(field_types),
                                ) => {
                                    // Bind each tuple pattern with its corresponding type
                                    if patterns.len() != field_types.len() {
                                        return Err(CompileError::Generic(format!(
                                            "Pattern has wrong number of fields for {}::{}",
                                            enum_name, variant
                                        )));
                                    }

                                    for (pattern, field_type) in patterns.iter().zip(field_types) {
                                        self.bind_pattern_variables(pattern, field_type)?;
                                    }
                                }
                                (
                                    PatternData::Struct(field_patterns),
                                    EnumVariantFields::Named(expected_fields),
                                ) => {
                                    // Bind each struct field pattern with its type
                                    for (field_name, pattern) in field_patterns {
                                        let field_type = expected_fields
                                            .iter()
                                            .find(|(name, _)| name == field_name)
                                            .map(|(_, ty)| ty)
                                            .ok_or_else(|| {
                                                CompileError::Generic(format!(
                                                    "Unknown field {} in {}::{}",
                                                    field_name, enum_name, variant
                                                ))
                                            })?;

                                        self.bind_pattern_variables(pattern, field_type)?;
                                    }
                                }
                                _ => {
                                    return Err(CompileError::Generic(format!(
                                        "Pattern structure doesn't match variant {}::{}",
                                        enum_name, variant
                                    )));
                                }
                            }
                        }
                        CheckerType::Generic { name, args } if name == enum_name => {
                            // Generic enum with type arguments
                            if let Some(generic_enum) = self.generic_enums.get(enum_name).cloned() {
                                // Find the variant
                                let variant_data = generic_enum
                                    .variants
                                    .iter()
                                    .find(|(v_name, _)| v_name == variant)
                                    .map(|(_, data)| data)
                                    .ok_or_else(|| {
                                        CompileError::Generic(format!(
                                            "Unknown variant {}::{}",
                                            enum_name, variant
                                        ))
                                    })?;

                                // Bind pattern variables based on variant data
                                match (pattern_data, variant_data) {
                                    (
                                        PatternData::Tuple(patterns),
                                        crate::ast::EnumVariantData::Tuple(param_types),
                                    ) => {
                                        if patterns.len() != param_types.len() {
                                            return Err(CompileError::Generic(format!(
                                                "Pattern has wrong number of fields for {}::{}",
                                                enum_name, variant
                                            )));
                                        }

                                        // For each pattern, determine its type by substituting type parameters
                                        let type_params = generic_enum.type_params.clone();
                                        for (pattern, param_type) in
                                            patterns.iter().zip(param_types)
                                        {
                                            // Extract types from generic args
                                            let concrete_types: Vec<CheckerType> = args
                                                .iter()
                                                .filter_map(|arg| match arg {
                                                    GenericArgValue::Type(t) => Some(t.clone()),
                                                    _ => None,
                                                })
                                                .collect();
                                            let concrete_type = self.substitute_type_params(
                                                param_type,
                                                &type_params,
                                                &concrete_types,
                                            )?;
                                            self.bind_pattern_variables(pattern, &concrete_type)?;
                                        }
                                        return Ok(());
                                    }
                                    _ => {
                                        // TODO: Handle other pattern types
                                        return Ok(());
                                    }
                                }
                            } else {
                                return Err(CompileError::Generic(format!(
                                    "Generic enum {} not found in definitions",
                                    enum_name
                                )));
                            }
                        }
                        _ => {
                            return Err(CompileError::TypeMismatch {
                                expected: format!("enum {}", enum_name),
                                found: value_type.to_string(),
                                span: None,
                            });
                        }
                    }
                }
                Ok(())
            }
        }
    }

    /// Infer type arguments for a generic function call
    fn infer_type_args(
        &self,
        generic_func: &GenericFunction,
        args: &[Expr],
    ) -> Result<Vec<String>> {
        let mut type_map: HashMap<String, String> = HashMap::new();

        // Check argument count
        if args.len() != generic_func.params.len() {
            return Err(CompileError::Generic(format!(
                "Function expects {} arguments, got {}",
                generic_func.params.len(),
                args.len()
            )));
        }

        // Infer types from each argument
        for (arg_expr, (_param_name, param_type)) in args.iter().zip(&generic_func.params) {
            self.infer_from_expr_and_type(arg_expr, param_type, &mut type_map)?;
        }

        // Make sure all type parameters were inferred
        let mut type_args = Vec::new();
        for type_param in &generic_func.type_params {
            match type_map.get(type_param) {
                Some(concrete_type) => type_args.push(concrete_type.clone()),
                None => {
                    return Err(CompileError::Generic(format!(
                        "Could not infer type parameter '{}' from function arguments",
                        type_param
                    )));
                }
            }
        }

        Ok(type_args)
    }

    /// Helper to infer type parameters from an expression and expected type
    fn infer_from_expr_and_type(
        &self,
        expr: &Expr,
        expected_type: &crate::ast::Type,
        type_map: &mut HashMap<String, String>,
    ) -> Result<()> {
        match expected_type {
            crate::ast::Type::TypeParam(param_name) => {
                // This is a type parameter - infer its type from the expression
                let expr_type = self.infer_expr_type(expr)?;

                // Check if we already have a mapping for this type parameter
                if let Some(existing_type) = type_map.get(param_name) {
                    if existing_type != &expr_type {
                        return Err(CompileError::Generic(format!(
                            "Type parameter '{}' has conflicting types: '{}' and '{}'",
                            param_name, existing_type, expr_type
                        )));
                    }
                } else {
                    type_map.insert(param_name.clone(), expr_type);
                }
                Ok(())
            }
            crate::ast::Type::Array(elem_type, _size) => {
                // For arrays, we need to infer the element type
                match expr {
                    Expr::ArrayLiteral { elements, .. } => {
                        if !elements.is_empty() {
                            // Use first element to infer the type parameter
                            self.infer_from_expr_and_type(&elements[0], elem_type, type_map)?;
                        }
                    }
                    Expr::ArrayRepeat { value, .. } => {
                        // Use the repeated value to infer the type parameter
                        self.infer_from_expr_and_type(value, elem_type, type_map)?;
                    }
                    Expr::Ident(name) => {
                        // For identifiers, we need to look up their type and extract element type
                        if let Some(var_info) = self.symbols.lookup(name) {
                            if let CheckerType::Array(var_elem_type, _) = &var_info.ty {
                                // If elem_type is a type parameter, map it
                                if let crate::ast::Type::TypeParam(param_name) = elem_type.as_ref()
                                {
                                    let elem_type_str = self.checker_type_to_string(var_elem_type);
                                    type_map.insert(param_name.clone(), elem_type_str);
                                }
                            }
                        }
                    }
                    _ => {
                        // For other expressions, try to infer their type
                        if let crate::ast::Type::TypeParam(_) = elem_type.as_ref() {
                            self.infer_from_expr_and_type(expr, elem_type, type_map)?;
                        }
                    }
                }
                Ok(())
            }
            _ => {
                // Non-generic type - nothing to infer
                Ok(())
            }
        }
    }

    /// Get a string representation of the expression's type for inference
    fn infer_expr_type(&self, expr: &Expr) -> Result<String> {
        match expr {
            Expr::String(_) => Ok("String".to_string()),
            Expr::Integer(_) => Ok("i64".to_string()), // Default to i64
            Expr::Bool(_) => Ok("bool".to_string()),
            Expr::Ident(name) => {
                // Look up variable type
                if let Some(var_info) = self.symbols.lookup(name) {
                    Ok(self.checker_type_to_string(&var_info.ty))
                } else {
                    Err(CompileError::Generic(format!("Unknown variable: {}", name)))
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                // Infer array type from elements
                if elements.is_empty() {
                    return Err(CompileError::Generic(
                        "Cannot infer type from empty array".to_string(),
                    ));
                }

                // Get type of first element (assume all elements have same type)
                let elem_type_str = self.infer_expr_type(&elements[0])?;
                let size = elements.len();
                Ok(format!("[{}; {}]", elem_type_str, size))
            }
            Expr::ArrayRepeat { value, count, .. } => {
                // Infer array type from repeated value
                let elem_type_str = self.infer_expr_type(value)?;

                // Extract count from expression (simplified - assumes integer literal)
                let size = match count.as_ref() {
                    Expr::Integer(n) => *n as usize,
                    _ => {
                        return Err(CompileError::Generic(
                            "Array size must be a constant integer".to_string(),
                        ))
                    }
                };

                Ok(format!("[{}; {}]", elem_type_str, size))
            }
            _ => {
                // For other complex expressions, we'd need full type checking
                Err(CompileError::Generic(
                    "Cannot infer type from complex expression".to_string(),
                ))
            }
        }
    }

    /// Convert CheckerType to string for type arguments
    #[allow(clippy::only_used_in_recursion)]
    fn checker_type_to_string(&self, ty: &CheckerType) -> String {
        match ty {
            CheckerType::Unit => "()".to_string(),
            // Not a spellable type argument: there is no surface name for a
            // range, so a generic can never be instantiated at one.
            CheckerType::Range => "range".to_string(),
            CheckerType::String => "String".to_string(),
            CheckerType::Int => "i64".to_string(),
            CheckerType::Float => "f64".to_string(),
            CheckerType::Bool => "bool".to_string(),
            CheckerType::Char => "char".to_string(),
            CheckerType::Array(elem, size) => {
                format!("[{}; {}]", self.checker_type_to_string(elem), size)
            }
            CheckerType::Struct(name) => name.clone(),
            CheckerType::TypeParam(name) => name.clone(),
            CheckerType::Enum(name) => name.clone(),
            CheckerType::Function(params, ret) => {
                let param_strs: Vec<String> = params
                    .iter()
                    .map(|p| self.checker_type_to_string(p))
                    .collect();
                format!(
                    "fn({}) -> {}",
                    param_strs.join(", "),
                    self.checker_type_to_string(ret)
                )
            }
            CheckerType::Generic { name, args } => {
                let arg_strs: Vec<String> = args
                    .iter()
                    .map(|a| match a {
                        GenericArgValue::Type(t) => self.checker_type_to_string(t),
                        GenericArgValue::Const(c) => match c {
                            ConstValueResolved::Integer(n) => n.to_string(),
                            ConstValueResolved::ConstParam(name) => name.clone(),
                        },
                    })
                    .collect();
                format!("{}<{}>", name, arg_strs.join(", "))
            }
            CheckerType::Tuple(types) => {
                let type_strs: Vec<String> = types
                    .iter()
                    .map(|t| self.checker_type_to_string(t))
                    .collect();
                format!("({})", type_strs.join(", "))
            }
        }
    }

    /// Instantiate a generic function with concrete types
    fn instantiate_generic_function(
        &mut self,
        generic_func: &GenericFunction,
        type_args: &[String],
    ) -> Result<CheckerType> {
        // Create a substitution map
        let mut subst_map: HashMap<String, String> = HashMap::new();
        for (type_param, type_arg) in generic_func.type_params.iter().zip(type_args) {
            subst_map.insert(type_param.clone(), type_arg.clone());
        }

        // Substitute types in parameters
        let mut param_types = Vec::new();
        for (_param_name, param_type) in &generic_func.params {
            let substituted_type = self.substitute_type(param_type, &subst_map)?;
            param_types.push(CheckerType::from(&substituted_type));
        }

        // Substitute return type
        let return_type = match &generic_func.return_type {
            Some(ret_type) => {
                let substituted = self.substitute_type(ret_type, &subst_map)?;
                CheckerType::from(&substituted)
            }
            None => CheckerType::Unit,
        };

        Ok(CheckerType::Function(param_types, Box::new(return_type)))
    }

    /// Substitute type parameters in a type
    #[allow(clippy::only_used_in_recursion)]
    fn substitute_type(
        &self,
        ty: &crate::ast::Type,
        subst_map: &HashMap<String, String>,
    ) -> Result<crate::ast::Type> {
        match ty {
            crate::ast::Type::TypeParam(param_name) => {
                match subst_map.get(param_name) {
                    Some(concrete_type) => {
                        // Convert string back to Type
                        match concrete_type.as_str() {
                            "()" => Ok(crate::ast::Type::Unit),
                            "String" => Ok(crate::ast::Type::String),
                            "i64" => Ok(crate::ast::Type::I64),
                            "i32" => Ok(crate::ast::Type::I32),
                            "u64" => Ok(crate::ast::Type::U64),
                            "u32" => Ok(crate::ast::Type::U32),
                            "bool" => Ok(crate::ast::Type::Bool),
                            _ => Ok(crate::ast::Type::Custom(concrete_type.clone())),
                        }
                    }
                    None => Err(CompileError::Generic(format!(
                        "Type parameter '{}' not found in substitution map",
                        param_name
                    ))),
                }
            }
            crate::ast::Type::Array(elem_type, size) => {
                let substituted_elem = self.substitute_type(elem_type, subst_map)?;
                Ok(crate::ast::Type::Array(
                    Box::new(substituted_elem),
                    size.clone(),
                ))
            }
            crate::ast::Type::Reference {
                lifetime,
                mutable,
                inner,
            } => {
                let substituted_inner = self.substitute_type(inner, subst_map)?;
                Ok(crate::ast::Type::Reference {
                    lifetime: lifetime.clone(),
                    mutable: *mutable,
                    inner: Box::new(substituted_inner),
                })
            }
            _ => Ok(ty.clone()),
        }
    }

    /// Check a function call with a known function type
    fn check_call_with_type(
        &mut self,
        func_name: &str,
        func_type: CheckerType,
        args: &[Expr],
    ) -> Result<CheckerType> {
        match func_type {
            CheckerType::Function(param_types, return_type) => {
                // Check argument count
                if args.len() != param_types.len() {
                    return Err(CompileError::Generic(format!(
                        "Function '{}' expects {} arguments, got {}",
                        func_name,
                        param_types.len(),
                        args.len()
                    )));
                }

                // Type check each argument
                for (arg, expected_type) in args.iter().zip(&param_types) {
                    let arg_type = self.check_expression(arg)?;
                    if arg_type != *expected_type {
                        return Err(CompileError::TypeMismatch {
                            expected: expected_type.to_string(),
                            found: arg_type.to_string(),
                            span: None,
                        });
                    }
                }

                Ok(*return_type)
            }
            _ => Err(CompileError::Generic(format!(
                "'{}' is not a function",
                func_name
            ))),
        }
    }

    /// Get all generic function instantiations for code generation
    ///
    /// Ordered by `(name, type_args)`, not by `HashMap` iteration.
    /// `FunctionInstantiation` is a hash key, and `RandomState` reseeds every
    /// process, so returning them in map order put the hash seed into the
    /// emitted C: six generic functions in a program that imports nothing
    /// produced thirty distinct outputs in thirty compiles.
    ///
    /// Emission order is not all that rides on this. `get_mangled_name_for_call`
    /// (`src/codegen/mod.rs:6719-6783`) scans this list for every instantiation
    /// of a name and, when a function has more than one, picks by inferring from
    /// the first argument — so before this, *which monomorphization a call
    /// resolved to* could also vary between runs. Sorting does not make that
    /// selection correct; it makes it reproducible, which is the precondition
    /// for ever seeing that it is wrong.
    ///
    /// AND IT IS WRONG. The cited range covers the whole function, including its
    /// silent default, because that is where the defect lives: the match loop
    /// accepts ANY type argument in ANY position, so `fn snd<A, B>(a: A, b: B)`
    /// instantiated at both `(i64, String)` and `(i64, i64)` resolves the call
    /// `snd(1, 2)` against the first sorted key — `(i64, String)`, since `S` sorts
    /// before `i`. Measured: the emitted C contains both `snd__i64_String` and
    /// `snd__i64_i64`, and calls `snd__i64_String(1, 2)`; the correct
    /// monomorphization is emitted and never called. When nothing matches at all
    /// the function still returns the first entry rather than `None`, with no
    /// diagnostic. Sorting made that reproducible instead of a coin flip, which is
    /// how it became visible. It is declared as
    /// `test_a_generic_call_resolves_to_its_own_monomorphization` and is owned by
    /// M4, not by this branch.
    ///
    /// This matters for the same reason the module ordering does: `make selfhost`
    /// asserts stage1 and stage2 emit byte-identical C, and it passes today only
    /// because `bootstrap/pdc.pd` uses no generics — they are excluded from PBS-1
    /// (`docs/specification/bootstrap-subset.md:132`).
    pub fn get_instantiations(&self) -> Vec<(String, Vec<String>, GenericFunction)> {
        let mut result = Vec::new();

        let mut keys: Vec<&FunctionInstantiation> = self.instantiations.keys().collect();
        keys.sort_by(|a, b| (&a.name, &a.type_args).cmp(&(&b.name, &b.type_args)));

        for instantiation in keys {
            if let Some(generic_func) = self.generic_functions.get(&instantiation.name) {
                result.push((
                    instantiation.name.clone(),
                    instantiation.type_args.clone(),
                    generic_func.clone(),
                ));
            }
        }

        result
    }

    /// For every generic name this compilation instantiates, WHERE the template
    /// codegen will monomorphize came from: `None` local, `Some(module)` imported.
    ///
    /// This is `get_instantiations` without the lossy step. That function returns
    /// the winning `GenericFunction` itself, but a consumer that only needs to
    /// decide "is THIS declaration the one that gets emitted" was collapsing the
    /// result to the bare name, and the name is shared by every same-named
    /// template across every module. Measured before this existed: a module
    /// exporting `pick<T>` with an ownership error in its body vetoed a build
    /// whose LOCAL `pick<T>` was the instantiated template and whose C contained
    /// no trace of the imported one. Renaming the imported function -- changing
    /// nothing else -- compiled.
    ///
    /// Restricted to instantiated names on purpose: an uninstantiated template is
    /// emitted by nobody, so no consumer should be deciding anything about it
    /// from this map.
    pub fn get_instantiated_generic_origins(&self) -> HashMap<String, Option<String>> {
        let mut result = HashMap::new();
        for instantiation in self.instantiations.keys() {
            if let Some(origin) = self.generic_function_origin.get(&instantiation.name) {
                result.insert(instantiation.name.clone(), origin.clone());
            }
        }
        result
    }

    /// Get all generic struct instantiations for code generation
    ///
    /// Ordered by `(name, type_args)` for the same reason as
    /// [`TypeChecker::get_instantiations`]: this list decides the order in which
    /// codegen emits monomorphized struct definitions, and a `HashMap` key order
    /// would make that depend on the per-process hash seed.
    ///
    /// NOT COVERED BY ANY TEST, and it cannot be until generic structs compile.
    /// The sentence above states what this ordering WOULD decide; no program can
    /// currently reach it, because `struct Box<T> { v: T }` lowers to `void*` and
    /// gcc rejects "initializing 'void *' with an expression of incompatible type
    /// 'struct Box_alpha_i64'". So this is sorted for symmetry with its sibling,
    /// and this paragraph — not the one above it — is the honest statement of its
    /// coverage. The same statement appears beside the test that covers the
    /// sibling (`tests/m3_imported_calls.rs`,
    /// `test_generic_instantiations_are_emitted_in_a_stable_order`), because a
    /// reader who arrives at either one should not have to find the other.
    pub fn get_struct_instantiations(&self) -> Vec<(String, Vec<String>, GenericStruct)> {
        let mut result = Vec::new();

        let mut keys: Vec<&StructInstantiation> = self.struct_instantiations.keys().collect();
        keys.sort_by(|a, b| (&a.name, &a.type_args).cmp(&(&b.name, &b.type_args)));

        for instantiation in keys {
            if let Some(generic_struct) = self.generic_structs.get(&instantiation.name) {
                result.push((
                    instantiation.name.clone(),
                    instantiation.type_args.clone(),
                    generic_struct.clone(),
                ));
            }
        }

        result
    }

    /// Get all available variable names for suggestions
    fn get_available_variables(&self) -> Vec<String> {
        let mut vars = Vec::new();
        for scope in &self.symbols.scopes {
            for var_name in scope.keys() {
                vars.push(var_name.clone());
            }
        }
        vars
    }

    /// Get all available function names for suggestions
    fn get_available_functions(&self) -> Vec<String> {
        let mut funcs: Vec<String> = self.functions.keys().cloned().collect();
        funcs.extend(self.generic_functions.keys().cloned());
        funcs
    }

    /// Get all available type names for suggestions
    #[allow(dead_code)]
    fn get_available_types(&self) -> Vec<String> {
        let mut types = vec!["String".to_string(), "i64".to_string(), "bool".to_string()];
        types.extend(self.structs.keys().cloned());
        types.extend(self.enums.keys().cloned());
        types
    }

    /// Check if we're currently in an unsafe context
    pub fn in_unsafe_context(&self) -> bool {
        self.unsafe_depth > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Span;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn check(source: &str) -> Result<()> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        TypeChecker::new().check(&ast)
    }

    // N14-02. ALL ELEVEN POSITIONS THAT CAN TAKE A NAME, and the count is
    // MEASURED rather than remembered: it is the number of distinct `what`
    // slots handed to `refuse_builtin_definition` and `refuse_builtin_shadow`
    // — six definitions (`const` and `static` share one call site but are two
    // nouns, plus fn, type alias, struct, enum) and five binders (parameter,
    // local, loop variable, `@` binding, pattern binding). It read TEN for two
    // rounds because this table was missing `static`, which is exactly the
    // failure the table exists to catch. The before-transcript
    // for each is in the fixture headers under tests/reject/shadow_builtin*.pd.
    // All eight of these compiled and ran with exit 0 before the check existed;
    // three of them ran the WRONG THING silently (the function definition was
    // unreachable, the local and the parameter meant the built-in at a call and
    // the binding at a use).
    //
    // Kept as one table rather than eight tests because the property is
    // "no position is missed": `refuse_builtin_definition` and
    // `refuse_builtin_shadow` sit inside `refuse_global_collision` and
    // `refuse_global_shadow`, so a NEW binder position that forgets to call its
    // sibling is the regression this is watching for, and a table makes adding
    // the row the obvious repair.
    #[test]
    fn a_builtin_name_is_refused_at_every_binding_position() {
        let cases: [(&str, &str, &str); 11] = [
            (
                "function",
                "fn print_int(x: i64) -> i64 { return x; }\nfn main() { print_int(7); }",
                "a function is declared under that name",
            ),
            (
                "struct",
                "struct print_int { n: i64 }\nfn main() { print(\"ok\"); }",
                "a struct is declared under that name",
            ),
            (
                "enum",
                "enum print_int { A }\nfn main() { print(\"ok\"); }",
                "an enum is declared under that name",
            ),
            (
                "type alias",
                "type print_int = i64;\nfn main() { print(\"ok\"); }",
                "a type alias is declared under that name",
            ),
            (
                "const",
                "const print_int: i64 = 3;\nfn main() { print(\"ok\"); }",
                "a top-level `const` is declared under that name",
            ),
            (
                "static",
                "static print_int: i64 = 3;\nfn main() { print(\"ok\"); }",
                "a top-level `static` is declared under that name",
            ),
            (
                "parameter",
                "fn f(print_int: i64) -> i64 { return print_int; }\nfn main() { print_int(f(1)); }",
                "the parameter `print_int` has the name of a built-in",
            ),
            (
                "local",
                "fn main() { let print_int: i64 = 5; print_int(9); }",
                "the local `print_int` has the name of a built-in",
            ),
            (
                "loop variable",
                "fn main() { for print_int in 0..2 { print(\"x\"); } }",
                "the loop variable `print_int` has the name of a built-in",
            ),
            (
                "pattern binding",
                "enum E { A(i64), B }\nfn main() { let e: E = E::A(4); \
                 match e { E::A(print_int) => { print(\"a\"); } E::B => { print(\"b\"); } } }",
                "the pattern binding `print_int` has the name of a built-in",
            ),
            (
                "`@` binding",
                "fn main() { let n: i64 = 3; \
                 match n { print_int @ 1..=5 => { print(\"in\"); } _ => { print(\"out\"); } } }",
                "the `@` binding `print_int` has the name of a built-in",
            ),
        ];
        for (position, source, expected) in cases {
            let err = check(source)
                .expect_err(position)
                .to_string();
            assert!(
                err.contains(expected),
                "{} position: expected {:?} in {:?}",
                position,
                expected,
                err
            );
        }
    }

    // THE TWO REASONS ARE DIFFERENT AND THE DIAGNOSTIC SAYS WHICH. A function
    // under a built-in's name produces C that cannot be reached; a TYPE under
    // one produces C that is perfectly fine (`struct print_int` beside
    // `void __pd_print_int(...)`, measured) and is refused on the language's
    // one-namespace rule instead. Claiming a collision for the second would be
    // a diagnostic that does not survive being checked.
    #[test]
    fn the_refusal_gives_the_reason_that_actually_applies() {
        let callable = check("fn print_int(x: i64) -> i64 { return x; }\nfn main() { print_int(7); }")
            .unwrap_err()
            .to_string();
        assert!(callable.contains("C that nothing can reach"), "{}", callable);
        assert!(!callable.contains("ONE namespace"), "{}", callable);

        let type_name = check("struct print_int { n: i64 }\nfn main() { print(\"ok\"); }")
            .unwrap_err()
            .to_string();
        assert!(type_name.contains("ONE namespace for top-level names"), "{}", type_name);
        assert!(
            type_name.contains("nothing collides in the emitted C"),
            "{}",
            type_name
        );
    }

    // OVER-REFUSAL CONTROL. `is_builtin` is an equality on the whole name, and
    // this is what says so: widen it to a prefix or a substring test and every
    // reject fixture above stays green while this test goes red.
    // `tests/03_functions_basic.pd` runs the same names end to end.
    #[test]
    fn a_name_that_merely_contains_a_builtin_name_is_accepted() {
        assert!(check(
            "fn print_int_of(n: i64) -> i64 { print_int(n); return n; }\n\
             fn printer(n: i64) -> i64 { return n + 1; }\n\
             struct print_state { n: i64 }\n\
             type print_kind = i64;\n\
             fn main() {\n\
                 let print_all: i64 = 8;\n\
                 let printing: print_kind = 1;\n\
                 print_int(print_int_of(print_all));\n\
                 print_int(printer(printing));\n\
                 for print_index in 0..2 { print_int(print_index); }\n\
             }"
        )
        .is_ok());
    }

    // D5. Both programs below used to type check, and then code generation
    // emitted C against a `struct Result` and a `poll` member that no part of
    // the compiler ever defines. They are kept as the measured repros — the
    // shapes that reached the backend back when type rules gated these
    // operators. The refusal no longer depends on that: it fires on any
    // operand, which `both_constructs_are_rejected_whatever_the_operand_is` in
    // tests/d5_unimplemented_constructs.rs covers.

    /// The measured `?` repro, kept verbatim as a source constant so the tests
    /// can slice it with the reported span instead of asserting magic numbers.
    const SOURCE_WITH_QUESTION: &str = r#"
        enum Result<T, E> {
            Ok(T),
            Err(E),
        }

        fn might_fail(x: i64) -> Result<i64, i64> {
            return might_fail(x);
        }

        fn helper(x: i64) -> Result<i64, i64> {
            let v: i64 = might_fail(x)?;
            print_int(v);
            return might_fail(v);
        }

        fn main() {
            helper(3);
        }
        "#;

    const SOURCE_WITH_AWAIT: &str = r#"
        fn work(x: i64) -> Future<i64> {
            return work(x);
        }

        fn main() {
            let v: i64 = work(3).await;
            print_int(v);
        }
        "#;

    const SOURCE_WITH_MULTILINE_AWAIT: &str = r#"
        fn work(x: i64) -> Future<i64> {
            return work(x);
        }

        fn main() {
            let v: i64 = work(
                3
            ).await;
            print_int(v);
        }
        "#;

    fn unimplemented_span(source: &str) -> Span {
        match check(source).unwrap_err() {
            CompileError::Unimplemented { span, .. } => span.expect("span"),
            other => panic!("expected an Unimplemented error, got: {}", other),
        }
    }

    #[test]
    fn test_question_operator_is_reported_as_unimplemented() {
        let err = check(SOURCE_WITH_QUESTION).unwrap_err();
        let construct = match &err {
            CompileError::Unimplemented { construct, .. } => construct.clone(),
            other => panic!("expected an Unimplemented error, got: {}", other),
        };
        assert!(construct.contains('?'), "{}", construct);

        // Two properties that must survive any later narrowing of the span:
        // it is on the operator's line, and it *ends* at the operator. Width is
        // asserted separately, in the known-quirk test, so that fixing the
        // start position does not require editing a test which would then read
        // as though the wide span had been correct.
        let span = unimplemented_span(SOURCE_WITH_QUESTION);
        assert_eq!(span.line, 12);
        let text = &SOURCE_WITH_QUESTION[span.start..span.end];
        assert!(
            text.ends_with('?'),
            "span must end at the operator: {:?}",
            text
        );
        assert!(
            SOURCE_WITH_QUESTION[..span.end].ends_with("might_fail(x)?"),
            "span must end where the operator ends"
        );
    }

    #[test]
    fn test_await_is_reported_as_unimplemented() {
        let err = check(SOURCE_WITH_AWAIT).unwrap_err();
        let construct = match &err {
            CompileError::Unimplemented { construct, .. } => construct.clone(),
            other => panic!("expected an Unimplemented error, got: {}", other),
        };
        assert!(construct.contains("await"), "{}", construct);

        let span = unimplemented_span(SOURCE_WITH_AWAIT);
        assert_eq!(span.line, 7);
        let text = &SOURCE_WITH_AWAIT[span.start..span.end];
        assert!(
            text.ends_with(".await"),
            "span must end at the operator: {:?}",
            text
        );
    }

    #[test]
    fn test_await_span_survives_a_multiline_postfix_chain() {
        // A span is only useful if it still covers the construct when the
        // postfix chain is broken across lines.
        let span = unimplemented_span(SOURCE_WITH_MULTILINE_AWAIT);
        let text = &SOURCE_WITH_MULTILINE_AWAIT[span.start..span.end];
        assert!(text.ends_with(".await"), "{:?}", text);
        assert!(text.contains('\n'), "the chain spans lines: {:?}", text);
        // The end is what a reader follows; it lands on the `.await` itself,
        // two lines below where the span starts.
        assert!(
            SOURCE_WITH_MULTILINE_AWAIT[..span.end].ends_with(").await"),
            "span must end where the operator ends"
        );
    }

    /// Known imprecision, pinned deliberately and separately.
    ///
    /// Postfix spans cover the whole suffix, so `?` is reported over `(x)?` and
    /// `.await` over `(3).await` rather than over the operator alone
    /// (`src/parser/mod.rs:4663-4671`, `src/parser/mod.rs:4426-4434`). That is not
    /// what these diagnostics
    /// *should* point at — it is what they currently point at. Narrowing the
    /// span to the operator is a welcome change: it will fail exactly this
    /// test, and no other, which is the point of keeping it apart from the
    /// assertions above.
    #[test]
    fn known_quirk_postfix_spans_cover_the_operand_too() {
        let q = unimplemented_span(SOURCE_WITH_QUESTION);
        assert_eq!(&SOURCE_WITH_QUESTION[q.start..q.end], "(x)?");

        let a = unimplemented_span(SOURCE_WITH_AWAIT);
        assert_eq!(&SOURCE_WITH_AWAIT[a.start..a.end], "(3).await");
    }

    #[test]
    fn test_unimplemented_diagnostic_names_the_consequence_and_a_workaround() {
        // "Not implemented" on its own sends the reader back to the source. The
        // house style (see the let-inference error) states what would otherwise
        // happen and what to do today.
        //
        // The note must describe the missing *lowering*. Saying "there is no
        // Result type" would contradict the program that triggers it, which
        // reaches this diagnostic precisely by declaring one.
        let diag = CompileError::question_unimplemented(Span::new(0, 1, 1, 1)).to_diagnostic();
        assert!(diag.span.is_some());
        let note = diag.notes.join(" ");
        assert!(note.contains("no lowering of `?`"), "{}", note);
        assert!(
            !note.contains("there is no Result type"),
            "the note must not deny a type the triggering program declares: {}",
            note
        );
        let help = diag
            .suggestions
            .iter()
            .map(|s| s.message.clone())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(help.contains("match"), "{}", help);
        // …and it must name its own limit rather than imply the `match`
        // replacement generalises to a generic Result, which does not compile.
        assert!(help.contains("Result<T, E>` will not compile"), "{}", help);

        // No diagnostic may promise a release. The roadmap changes; the binary
        // that was already shipped does not.
        for text in [
            CompileError::question_unimplemented(Span::new(0, 1, 1, 1)),
            CompileError::await_unimplemented(Span::new(0, 1, 1, 1)),
        ]
        .iter()
        .flat_map(|e| {
            let d = e.to_diagnostic();
            let mut v = d.notes.clone();
            v.push(d.message.clone());
            v.extend(d.suggestions.iter().map(|s| s.message.clone()));
            v
        }) {
            for promise in ["M1", "M2", "M3", "M4", "M5", "MILESTONES", "scheduled"] {
                assert!(
                    !text.contains(promise),
                    "diagnostic promises a roadmap item ({}): {}",
                    promise,
                    text
                );
            }
        }
    }

    #[test]
    fn test_await_diagnostic_does_not_suggest_merely_deleting_the_await() {
        // Measured on a `-> Future<T>` function, the case the advice is for:
        // deleting `.await` yields "Type mismatch: expected Int, found
        // Future<Int>". A suggestion that trades one error for another is the
        // defect this whole change exists to remove, so it is asserted against
        // by name. The phrasing must also stay conditional, because the operand
        // is never inspected and may not involve a function at all.
        let diag = CompileError::await_unimplemented(Span::new(0, 1, 1, 1)).to_diagnostic();
        let help = diag
            .suggestions
            .iter()
            .map(|s| s.message.clone())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(help.contains("change it to `-> T`"), "{}", help);
        assert!(
            !help.contains("call the function without `.await`"),
            "that suggestion does not compile: {}",
            help
        );
        // Phrased conditionally, because the operand is never inspected: a
        // `some_variable.await` has no function whose signature could change.
        assert!(help.contains("If a function is declared"), "{}", help);
        assert!(
            !help.starts_with("declare the function"),
            "unconditional phrasing presumes an operand shape: {}",
            help
        );
    }

    #[test]
    fn test_type_check_hello_world() {
        let source = r#"
        fn main() {
            print("Hello, World!");
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_undefined_function() {
        let source = r#"
        fn main() {
            unknown_function();
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
    }

    #[test]
    fn test_let_binding() {
        let source = r#"
        fn main() {
            let x = 42;
            let y: i32 = 10;
            let message = "Hello";
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_variable_usage() {
        let source = r#"
        fn main() {
            let x = 42;
            let y = x;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_undefined_variable() {
        let source = r#"
        fn main() {
            let x = y;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
    }

    #[test]
    fn test_binary_operations() {
        let source = r#"
        fn main() {
            let x = 10 + 20;
            let y = x - 5;
            let z = y * 2;
            let w = z / 3;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_type_mismatch_in_binary() {
        let source = r#"
        fn main() {
            let x = "hello" + 42;
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());

        if let Err(CompileError::TypeMismatch {
            expected,
            found,
            span: _,
            ..
        }) = result
        {
            assert_eq!(expected, "String");
            assert_eq!(found, "Int");
        }
    }

    #[test]
    fn test_type_annotation_mismatch() {
        let source = r#"
        fn main() {
            let x: i32 = "not an int";
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());

        if let Err(CompileError::TypeMismatch {
            expected,
            found,
            span: _,
            ..
        }) = result
        {
            assert_eq!(expected, "Int");
            assert_eq!(found, "String");
        }
    }

    #[test]
    fn test_variable_redefinition() {
        let source = r#"
        fn main() {
            let x = 42;
            let x = "redefined";
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());
    }

    #[test]
    fn test_for_loop_type_checking() {
        let source = r#"
        fn main() {
            let arr = [1, 2, 3, 4, 5];
            for i in arr {
                print_int(i);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_for_loop_wrong_type() {
        let source = r#"
        fn main() {
            let x = 42;
            for i in x {
                print_int(i);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());

        if let Err(CompileError::Generic(msg)) = result {
            assert!(msg.contains("For loop requires an array"));
        }
    }

    #[test]
    fn test_break_continue_in_loops() {
        let source = r#"
        fn main() {
            let arr = [1, 2, 3, 4, 5];
            
            // Test break and continue in while loop
            let mut i = 0;
            while i < 10 {
                if i == 5 {
                    break;
                }
                if i == 3 {
                    i = i + 1;
                    continue;
                }
                i = i + 1;
            }
            
            // Test break and continue in for loop
            for n in arr {
                if n == 3 {
                    continue;
                }
                if n > 4 {
                    break;
                }
                print_int(n);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_string_len_typecheck() {
        let source = r#"
        fn main() {
            let s = "Hello";
            let len = string_len(s);
            print_int(len);
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_string_concat_typecheck() {
        let source = r#"
        fn main() {
            let s1 = "Hello";
            let s2 = " World";
            let s3 = string_concat(s1, s2);
            print(s3);
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    // N4-04 retyped these predicates over `char`, so `let c = 65;` no longer
    // reaches them — that binding is an `i64`, and an integer is not a
    // character without saying so. The literal is what changed here, not the
    // test's subject.
    #[test]
    fn test_string_char_predicates() {
        let source = r#"
        fn main() {
            let c = 'A';
            let is_alpha = char_is_alpha(c);
            let is_digit = char_is_digit(c);
            let is_space = char_is_whitespace(c);
            if is_alpha {
                print("Is alphabetic");
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_string_type_errors() {
        let source = r#"
        fn main() {
            let n = 42;
            let len = string_len(n); // Error: expects string
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_err());
    }

    #[test]
    fn test_file_io_typecheck() {
        let source = r#"
        fn main() {
            let path = "test.txt";
            let exists = file_exists(path);
            if exists {
                let handle = file_open(path);
                let content = file_read_all(handle);
                file_close(handle);
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_file_write_typecheck() {
        let source = r#"
        fn main() {
            let handle = file_open("output.txt");
            let success = file_write(handle, "test content");
            let closed = file_close(handle);
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_file_io_type_errors() {
        let source = r#"
        fn main() {
            let handle = file_open(123); // Error: expects string
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_err());
    }

    #[test]
    fn test_result_enum_definition() {
        let source = r#"
        enum Result {
            Ok(String),
            Err(String),
        }
        
        fn main() {
            let ok = Result::Ok("success");
            let err = Result::Err("failure");
            
            match ok {
                Result::Ok(_) => print("ok"),
                Result::Err(_) => print("err"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        match type_checker.check(&ast) {
            Ok(_) => {}
            Err(e) => panic!("Type check failed: {}", e),
        }
    }

    #[test]
    fn test_result_pattern_matching() {
        let source = r#"
        enum IntResult {
            Ok(i64),
            Err(String),
        }
        
        fn main() {
            let result = IntResult::Ok(42);
            
            match result {
                IntResult::Ok(_) => print("Success"),
                IntResult::Err(_) => print("Error"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_multiple_result_types() {
        let source = r#"
        enum StringResult {
            Ok(String),
            Err(String),
        }
        
        enum FileResult {
            Ok(i64),
            Err(String),
        }
        
        fn main() {
            let s_result = StringResult::Ok("test");
            let f_result = FileResult::Err("not found");
            
            match s_result {
                StringResult::Ok(_) => {
                    print("string ok");
                }
                StringResult::Err(_) => {
                    print("string err");
                }
            }
            
            match f_result {
                FileResult::Ok(_) => {
                    print("file ok");
                }
                FileResult::Err(_) => {
                    print("file err");
                }
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_exhaustive_enum_match() {
        let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }
        
        fn main() {
            let c = Color::Red;
            
            match c {
                Color::Red => print("red"),
                Color::Green => print("green"),
                Color::Blue => print("blue"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_non_exhaustive_enum_match() {
        let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }
        
        fn main() {
            let c = Color::Red;
            
            match c {
                Color::Red => print("red"),
                Color::Green => print("green"),
                // Missing Blue!
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());

        if let Err(CompileError::NonExhaustiveMatch {
            missing_patterns, ..
        }) = result
        {
            assert!(missing_patterns.contains(&"Color::Blue".to_string()));
        } else {
            panic!("Expected NonExhaustiveMatch error");
        }
    }

    #[test]
    fn test_wildcard_makes_match_exhaustive() {
        let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }
        
        fn main() {
            let c = Color::Red;
            
            match c {
                Color::Red => print("red"),
                _ => print("other"),
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        assert!(type_checker.check(&ast).is_ok());
    }

    #[test]
    fn test_unreachable_pattern_after_wildcard() {
        let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }
        
        fn main() {
            let c = Color::Red;
            
            match c {
                Color::Red => print("red"),
                _ => print("any"),
                Color::Blue => print("blue"), // Unreachable!
            }
        }
        "#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut type_checker = TypeChecker::new();
        let result = type_checker.check(&ast);
        assert!(result.is_err());

        if let Err(CompileError::UnreachablePattern { .. }) = result {
            // Expected
        } else {
            panic!("Expected UnreachablePattern error");
        }
    }
}
