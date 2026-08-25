# The Palladium Language Specification

**Version**: 0.3
**Date**: 2026-08-22
**Supersedes**: v0.2 (2026-08-22), which described only the implemented subset and disowned the
rest; and `language_specification.md` v1.0.0-alpha (2025-01-19), which described an intended
language as though it were built.

## 0. How to read this document

This document has two parts, and the separation between them is the whole point.

**[Part I — Normative](#part-i-normative-specification)** says what Palladium *is*. It is the
definition of the language. It does not change when the compiler changes, and it is not a claim
about `pdc`. Every real language specification is written this way: ISO C defines C, and no
sentence in it becomes false because a particular compiler is incomplete.

**[Part II — Implementation status annex](#part-ii-implementation-status-annex)** says what `pdc`
does today, section by section, with a source location or a command output for every row. It is
allowed to be embarrassing. It is not allowed to be absent, and it is not allowed to be vague.

Neither part may stand in for the other. A specification that silently shrinks to fit the
implementation stops being a specification; an implementation status page that inherits the
specification's confidence stops being a measurement. The failure this project actually suffered
was the second kind — documentation that read as status while describing a language nobody had
built — and the repair is the split, not the amputation.

Annex vocabulary:

| Mark | Meaning |
|---|---|
| **implemented** | Works end-to-end: parses, typechecks, generates C, runs. |
| **partial** | Parses, but breaks downstream — a compile error later, or (worse) wrong C. Each entry names the failure. |
| **unimplemented** | Not built. Either unparseable or explicitly rejected. |

A claim in Part II without a `file:line` or a reproducible command is a bug in this document. A
claim in Part I needs no such citation, because it is a definition; what it needs is to be
consistent with the rest of Part I and with the feature documents it links to.

**Citations in the annex were re-derived against the working tree at commit `abeb665`.** The v0.2
citations into `src/codegen`, `src/parser`, `src/typeck` and `src/driver` had been taken from the
pre-cleanup revision `f323cf1` and were off by 16 to 380 lines; several named unrelated code.
Those are corrected below and the corrections are noted where the difference changes the claim.

Per-feature index with the same evidence, organised by feature rather than by section:
[`docs/reference/features/feature-index.toml`](../reference/features/feature-index.toml).

---

# Part I: Normative specification

## N1. Overview and design commitments

<sub>Non-normative pointer, not part of the definition: **implementation status → [A1 — partial: the C backend works, the LLVM backend is skeletal](#a1-pipeline-and-backends)**</sub>

Palladium is a statement-oriented systems language, compiled ahead of time, with no garbage
collector and no runtime type information. It exists to make three claims true at once, and those
three are the reason for it to exist rather than to be a Rust dialect:

1. **Asynchrony is an effect, not a colour.** There is no `async` keyword and no `.await`
   operator. See [N7](#n7-effects-and-asynchrony).
2. **Termination is provable.** A function or a whole crate can be required to terminate, and the
   compiler discharges the obligation. See [N8](#n8-totality).
3. **Lifetimes are inferred.** Memory safety is Rust's, but the `'a` bookkeeping is the
   compiler's job. See [N9](#n9-references-and-lifetimes).

The reference implementation compiles to C and links with the system C compiler. That is an
implementation strategy, not a language property: nothing in Part I depends on the target being C,
and an LLVM backend is a second implementation of the same definition.

## N2. Lexical structure

<sub>Non-normative pointer, not part of the definition: **implementation status → [A2 — partial: floats, chars, escapes, nesting comments and the attribute token all lex; no hex/binary/octal literals, and every attribute is refused because none is implemented](#a2-lexical-structure)**</sub>

Source is UTF-8.

```ebnf
identifier      = ( letter | '_' ) { letter | digit | '_' } ;
integer_literal = [ '-' ] digit { digit } ;
float_literal   = digit { digit } '.' digit { digit } ;
char_literal    = "'" ( char | escape ) "'" ;
string_literal  = '"' { char | escape } '"' ;
boolean_literal = "true" | "false" ;
```

Comments are `//` to end of line and `/* … */`, nesting, and are whitespace.

Attributes are lexical: `#[name]`, `#[name(args)]` on an item, and `#![name(args)]` at the top of
a compilation unit. They carry totality obligations ([N8](#n8-totality)) and are the extension
point for future annotations.

## N3. Program structure and items

<sub>Non-normative pointer, not part of the definition: **implementation status → [A3, A4 — partial](#a3-program-structure)**</sub>

A compilation unit is a sequence of imports followed by items.

```ebnf
item = function | struct_def | enum_def | trait_def | impl_block
     | type_alias | macro_def | const_item | module ;
```

**Functions** take typed parameters, may declare a return type, and are expression-oriented: a
body's trailing expression is its value. There is no `async` modifier ([N7](#n7-effects-and-asynchrony)).

**Structs and enums** are the product and sum types. Enum variants may be unit, tuple or struct
shaped.

**Macros** are one system — pattern-based, hygienic by default. There is no split between a
declarative macro language and a procedural one.

Full feature list: [`PALLADIUM_V1_FEATURES.md`](../reference/features/PALLADIUM_V1_FEATURES.md).

## N4. Types

<sub>Non-normative pointer, not part of the definition: **implementation status → [A5 — partial: no floats, slices or fn types; Option and Result are not built in](#a5-types)**</sub>

Primitives: `i32`, `i64`, `u32`, `u64`, `f32`, `f64`, `bool`, `char`, `String`, `()`.
`int` is an alias for `i64`.

> **OPEN: `str` and `usize` are used normatively elsewhere and are not in this list.**
> [`implicit-lifetimes.md`](../reference/features/core-language/implicit-lifetimes.md) writes
> `ref str` and `position: usize` in normative examples, and
> [N14](#n14-builtins-and-the-standard-library) gives `string_char_at` a `char` return while
> `string_len` returns `i64` rather than a size type. Under this list `ref str` names no type.
> Two ways out, and the choice is the owner's: **add** `str` (a borrowed string slice, the natural
> referent of `ref`) and `usize` (an index type) to the primitive set, or **rewrite** those
> examples to `ref String` and `u64`. This is flagged rather than decided because adding two
> primitives is a language change, and because the page carrying the inconsistency had never been
> reviewed — it was found by reading it for the first time in four rounds, not by any gate.

Composites: arrays `[T; N]`, slices `[T]`, tuples `(A, B)`, references
(`ref T` / `ref mut T`, see [N9](#n9-references-and-lifetimes)), function types `fn(A) -> B`,
and named types with generic arguments `Name<A, B>`.

`Option<T>` and `Result<T, E>` are in the prelude. `?` propagates a `Result`'s error to the
caller, converting error types where a conversion exists.

Type inference is local and does not require annotations on `let` bindings or on most
expressions. Const generics (`struct Buffer<const N: usize>`) are generic parameters evaluated at
compile time.

## N5. Statements and expressions

<sub>Non-normative pointer, not part of the definition: **implementation status → [A6 — partial: `if`/`match`/blocks/`loop` are expressions, with `else if`, compound assignment, bitwise, ranges, `as` and `x.f()`; no closures and no `try { }`](#a6-statements-and-expressions)**</sub>

Statements: `let`, assignment, `return`, `break`, `continue`, `unsafe { }`, and expression
statements.

`if`, `match`, and blocks are **expressions**. `let x = if c { 1 } else { 2 };` is well-formed, and
so is `else if`. `loop` is an infinite loop, exited with `break`, which may carry a value.

Closures are expressions: anonymous functions with inferred capture mode, and a `move` form that
transfers ownership.

`try { … }` scopes error handling, catching and transforming a `Result` locally.

Operators: arithmetic `+ - * / %`, comparison `== != < > <= >=`, logical `&& || !`, bitwise
`& | ^ ~ << >>`, compound assignment `+= -= *= /= %=`, ranges `..` and `..=`, and `as` casts.
Unary minus binds tighter than multiplication, so `a * -b` is `a * (-b)`.

## N6. Patterns

<sub>Non-normative pointer, not part of the definition: **implementation status → [A7 — partial: literal, range, tuple, or- and `@` patterns with guards; no slices, non-enum struct patterns, `ref`/`mut` or field shorthand; exhaustiveness for every scrutinee type](#a7-patterns)**</sub>

Patterns appear in `match` arms, `let` bindings and parameters:

```ebnf
pattern = '_' | identifier | literal | range_pattern
        | path [ '(' pattern { ',' pattern } ')' | '{' field_patterns '}' ]
        | tuple_pattern | slice_pattern
        | pattern '|' pattern
        | identifier '@' pattern ;
```

Arms may carry guards (`if cond`). `match` is exhaustive: a non-exhaustive match is a compile
error, not a silent fall-through, and this applies to every scrutinee type, not only enums.

## N7. Effects and asynchrony

<sub>Non-normative pointer, not part of the definition: **implementation status → [A6.5 — unimplemented: the compiler has async and .await, which this section removes](#a65-question-mark-async-and-await)**</sub>

Full definition: [`async-as-effect.md`](../reference/features/async-system/async-as-effect.md).

Asynchrony is an **algebraic effect**, not a function colour.

- **There is no `async` keyword and no `.await` operator.** A function that performs an
  asynchronous operation is written exactly like one that does not.
- Effects are **inferred** from a function's body and **propagated to callers**, transitively.
  The propagation is a fixed point over the call graph, not a single pass, and an unresolved
  callee is not assumed pure.
- **Independent effectful operations are parallel by default.** Sequencing is requested, not
  accidental.
- Effect contexts scope policy over a block rather than threading it through every call:
  `with_timeout(5.seconds) { with_retry(3) { … } }`.
- There is **no async runtime and no `Future` boxing**. Effect tracking is entirely static and has
  no runtime representation.

Two escape hatches, and only two: `effect::sync { … }` forces sequential execution, and an
explicit `-> async T` return type pins an asynchronous boundary.

The effect vocabulary is not limited to asynchrony: IO, memory, panic and unsafe are effects on
the same footing, which is what makes "this function is pure" a statement the compiler can check.

## N8. Totality

<sub>Non-normative pointer, not part of the definition: **implementation status → [A2 — unimplemented: attributes do not lex, so no totality syntax reaches the parser](#a2-lexical-structure)**</sub>

Full definition: [`totality-checking.md`](../reference/features/advanced/totality-checking.md).

Palladium can prove that a function terminates.

| Form | Meaning |
|---|---|
| `#![total(strict)]` | Crate-level: every function must be proven total, and `unsafe` is not permitted. |
| `#[total]` | Per function: the compiler must prove this one terminates. |
| `#[decreases(expr)]` | The termination measure: `expr` strictly decreases, in a well-founded order, at every recursive call. |
| `#[total(fuel = N)]` | Bounded termination: at most `N` steps. |
| `#[partial]` | Explicit opt-out; termination is not being proven here. |

Structural recursion on an inductive type needs no measure — a recursive call on a strict subterm
is proven automatically. Failure to discharge an obligation is a compile error; there is no mode
in which an unproven `#[total]` function is accepted.

## N9. References and lifetimes

<sub>Non-normative pointer, not part of the definition: **implementation status → [A9 — unimplemented: ref is not a keyword and there is no region inference](#a9-memory-model)**</sub>

Full definition: [`implicit-lifetimes.md`](../reference/features/core-language/implicit-lifetimes.md).

| Form | Meaning |
|---|---|
| `ref T` | Shared borrow. Replaces Rust's `&T`. |
| `ref mut T` | Mutable borrow. Replaces Rust's `&mut T`. |
| `ref<'a> T` | An explicitly named region, for cases inference cannot resolve. |

There are **no `'a` parameter lists** on functions, structs or impls. A region name appears only
inside a `ref<…>`, and only where the compiler has asked for one.

The safety guarantee is Rust's, unchanged: no use after free, no aliasing `ref mut`, checked at
compile time with no runtime cost. What is removed is the annotation burden, not the analysis.
When inference cannot determine a region, that is a **compile error naming the ambiguity**, never
a guess.

## N10. Traits and generics

<sub>Non-normative pointer, not part of the definition: **implementation status → [A4.4, A5 — unimplemented: traits emit no code; generic struct fields are rejected in codegen](#a44-traits)**</sub>

Generics are monomorphised: a generic function or type is instantiated per concrete argument, so
abstraction costs nothing at runtime. Type parameters may carry bounds (`<T: Display>`) and
`where` clauses.

Traits define shared behaviour: method signatures with optional defaults, associated types, and
static dispatch through bounds. Trait methods take a `self` receiver.

These two are defined in detail by design documents rather than restated here:

- [`docs/design/trait_system_design.md`](../design/trait_system_design.md)
- [`docs/design/generics.md`](../design/generics.md)

Those three documents carry a **dual-axis banner**: *normative language definition, compiler status
unimplemented*. That is the same relationship every section of Part I has to Part II, and it
replaces the older single-axis "PROPOSAL — not implemented" banner, which was accurate about the
compiler and wrong about the language. Material in them that is genuinely still undecided sits
under an explicitly non-normative open-design heading in each file, so that "not yet built" and
"not yet decided" cannot be confused again.

## N11. Modules

<sub>Non-normative pointer, not part of the definition: **implementation status → [A3 — partial: import works, there is no mod item](#a3-program-structure)**</sub>

Modules are file-based, with nested paths and public/private visibility. Imports:

```
import std::math;
import std::io::{read, write};
import std::collections as col;
import std::prelude::*;
```

Design detail: [`docs/design/module-system.md`](../design/module-system.md), read on the same terms
as [N10](#n10-traits-and-generics).

## N12. Memory model

<sub>Non-normative pointer, not part of the definition: **implementation status → [A9 — partial: checked but not typed; String is Copy in the implementation](#a9-memory-model)**</sub>

Ownership and borrowing are Rust's: each value has one owner, moves are the default, borrows are
checked at compile time, and there is no garbage collector.

Values with a destructor are dropped at end of scope. `String` is an owned, heap-allocated,
UTF-8 value with move semantics.

`unsafe { }` is where the compiler's guarantees are suspended and the programmer's take over. It
is restricted rather than unrestricted: it is auditable, isolated, and forbidden inside a
`#![total(strict)]` crate.

A `&mut T` may be taken only of a binding declared `mut`. Taking `&mut` of an immutable binding is
a compile error.

### N12.1 Array parameters — OPEN DECISION

**This subsection is not yet decided. It is written as two options because the choice is the
owner's, and choosing silently would be worse than leaving it visibly open.** Everything else in
Part I is settled; this is not, and no reader should treat either option below as the rule.

The question: **does a `[T; N]` parameter copy the caller's array, or alias it?**

Nothing in this specification has ever answered that. §N12 defines moves, borrow-checking and
destructors and says nothing about array parameters. It matters because `[T; N]` has a
compile-time size, so a copy is a coherent option in a way it is not for a slice, and because
without a rule the callee's `a[0] = x` is either a local edit or a caller-visible mutation, with
no way for a reader to tell which.

The two options are not symmetric — they differ in what they cost and in what they make of the
other two questions below.

**Option A — value semantics.** `[T; N]` is a value type. Passing one copies `N * sizeof(T)`
bytes; the callee's writes are invisible to the caller. Then `&[T; N]` and `&mut [T; N]` have the
job every reference has: avoiding the copy, and opting into caller-visible mutation respectively.
This is coherent with `[T; N]` being sized and with §N12's "moves are the default" — and it is the
only option under which the three spellings mean three different things.
*Cost:* every array argument is a memcpy unless the author remembers `&`. For a compiler written
in this language that is a real cost, and the bootstrap subset has been avoiding it by convention
already.

**Option B — reference semantics.** `[T; N]` always aliases the caller's storage, matching C's
array-to-pointer decay. Then `&[T; N]` and `&mut [T; N]` are **redundant spellings**, and the
specification should say so and pick one: either they are forbidden, or `&mut` becomes the *only*
permitted spelling for a parameter the callee writes through, with bare `[T; N]` read-only.
*Cost:* the language inherits a C wart, and "moves are the default" acquires an exception that has
to be stated everywhere arrays are discussed.

Two dependent questions, which cannot be answered before the choice above:

1. **What do `&[T; N]` and `&mut [T; N]` mean?** Under A they are the no-copy and mutable-alias
   forms. Under B they are noise unless promoted to the mutation marker.
2. **Is `mut` on a parameter part of the type, or a binding mode?**
   [`bootstrap-subset.md`](bootstrap-subset.md) currently requires `mut` on every struct and array
   parameter, and `benchmarks/palladium/bubble_sort.pd:11` follows it
   (`fn bubble_sort(mut arr: [i64; 45000], n: i64)`). If `mut` is a binding mode — a statement about
   the callee's local name — it cannot also be what makes a mutation caller-visible. If it is part
   of the type, then `mut arr: [T; N]` is a third reference spelling and Option B's redundancy
   problem gets worse, not better.

Until this is decided, the bootstrap subset's convention governs by default, because it is written
down and followed: struct and array parameters are declared `mut`, and a write through a parameter
not declared `mut` is refused. That is a placeholder with an owner, not a rule with a rationale.

Measured consequences of each choice are recorded in [A9.2](#a92-array-parameters).

## N13. Execution model

<sub>Non-normative pointer, not part of the definition: **implementation status → [A10 — implemented](#a10-execution-model)**</sub>

Execution begins at `fn main`. Arguments are evaluated left to right. A compilation unit without
a `main` is a library.

## N14. Builtins and the standard library

<sub>Non-normative pointer, not part of the definition: **implementation status → [A8 — partial: the registry is exactly these 34 names and all 34 are callable; signatures still differ (no `Result`); stdlib/ does not parse](#a8-builtins)**</sub>

A **builtin** is an operation the compiler knows intrinsically: it is in scope without an import,
its name is reserved, and it has no Palladium definition a program could read or replace. The
normative content of this section is the *surface* — which capabilities are builtin, what their
signatures look like, and what distinguishes them from library code — not a name list.

> An earlier draft of this section delegated the normative list to
> `docs/reference/builtins.md`, which `scripts/gen-builtin-docs.py` generates from
> `src/builtins.rs`. That was a mistake and contradicted this document's own premise: it made the
> language definition change whenever the compiler's table changed, so `pdc` could have *redefined
> Palladium* by adding a row. The generated table is evidence about the implementation and lives in
> [A8](#a8-builtins). Part I defines the surface; Part II reports what is built.

**The normative set, enumerated.** "Closed" is meaningless unless the set can be named, so it is
named here. These identifiers are reserved: a program may not define or shadow them, and a
conforming implementation provides all of them with these signatures.

| Name | Signature | Effects |
|---|---|---|
| `print` | `(String) -> ()` | io |
| `print_int` | `(i64) -> ()` | io |
| `panic` | `(String) -> !` | panic |
| `string_len` | `(String) -> i64` | pure |
| `string_concat` | `(String, String) -> String` | pure |
| `string_eq` | `(String, String) -> bool` | pure |
| `string_char_at` | `(String, i64) -> char` | pure |
| `string_substring` | `(String, i64, i64) -> String` | pure |
| `string_from_char` | `(char) -> String` | pure |
| `string_to_int` | `(String) -> Result<i64, ParseError>` | pure |
| `int_to_string` | `(i64) -> String` | pure |
| `char_is_digit` | `(char) -> bool` | pure |
| `char_is_alpha` | `(char) -> bool` | pure |
| `char_is_whitespace` | `(char) -> bool` | pure |
| `arg_count` | `() -> i64` | io |
| `arg_at` | `(i64) -> String` | io |
| `file_open` | `(String, OpenMode) -> Result<File, IoError>` | io |
| `file_read_all` | `(File) -> Result<String, IoError>` | io |
| `file_read_line` | `(File) -> Result<Option<String>, IoError>` | io |
| `file_write` | `(File, String) -> Result<(), IoError>` | io |
| `file_close` | `(File) -> Result<(), IoError>` | io |
| `file_flush` | `(File) -> Result<(), IoError>` | io |
| `file_seek` | `(File, i64, SeekFrom) -> Result<i64, IoError>` | io |
| `file_exists` | `(String) -> bool` | io |
| `path_exists` | `(String) -> bool` | io |
| `path_is_file` | `(String) -> bool` | io |
| `path_is_dir` | `(String) -> bool` | io |
| `create_dir` | `(String) -> Result<(), IoError>` | io |
| `create_dir_all` | `(String) -> Result<(), IoError>` | io |
| `remove_file` | `(String) -> Result<(), IoError>` | io |
| `remove_dir` | `(String) -> Result<(), IoError>` | io |
| `remove_dir_all` | `(String) -> Result<(), IoError>` | io |
| `read_file_to_string` | `(String) -> Result<String, IoError>` | io |
| `write_string_to_file` | `(String, String) -> Result<(), IoError>` | io |

Thirty-four names. `File`, `OpenMode`, `SeekFrom`, `IoError` and `ParseError` are prelude types
([N4](#n4-types)); a `File` is an opaque handle, not an integer.

> **Reconciliation against `src/builtins.rs`, name by name.** The implementation defines **34**
> names; this table defines **34**. Set arithmetic, computed from the table above and the compiler's
> own registry:
>
> - normative − implemented = **none**. Every one of the 34 names exists in `pdc`.
> - implemented − normative = **none**, since 2026-08-23.
>
> The two sets are equal, and that equality is now **a check rather than a paragraph**:
> `src/builtins.rs::test_registry_is_exactly_the_normative_builtin_set` parses the table above out
> of this file and compares it against the registry in both directions, so a thirty-fifth builtin
> is a red test and not a reader's job.
>
> *(Until 2026-08-23 the implementation defined **38**, and `implemented − normative` was
> `file_open_ex`, `file_close_ex`, `file_read_ex` and `file_write_ex` — a parallel handle API that
> existed because `OpenMode` does not. None of the four was callable, all four were refused at
> typecheck, and no `.pd` file in the tree named one. They were REMOVED FROM THE REGISTRY rather
> than repaired: a compiler table that carries names this section does not define is a second
> definition of the builtin surface. Their C wrappers are still emitted — dead code in
> `src/codegen/mod.rs`, recorded as owed in [A8](#a8-builtins) and not as done.)*
>
> What does *not* match is **signatures**, and closing the name sets did nothing about that: the
> filesystem builtins return `i64`/`bool` handles rather than `Result`, and `string_char_at`
> returns `i64` rather than `char` because `char` is not a type yet. Itemised in
> [A8](#a8-builtins). *(A third divergence — `file_flush` and `file_seek` registered but not
> callable, their C wrappers taking an opaque `FileHandle` no Palladium type can hold — was closed
> on 2026-08-23 by re-basing both onto the `long long` handle table.)*
>
> *(A previous version of this annex said "36 builtins", inherited from the pre-cleanup
> specification's section heading. It was never right: `src/builtins.rs` had 38 from `191f8c1`,
> which made it the single table, until the four `*_ex` names left it. Corrected at every site.)*
>
> This table is the definition, and it is written independently of the generated one on purpose —
> an earlier draft delegated to it, which would have let `pdc` redefine Palladium by adding a row.

Three normative constraints, which are properties of the language rather than of any table:

1. **Builtins are closed.** The set is exactly the 34 names above. A program cannot define a new
   builtin, and the set does not vary by target. A capability that varies by target belongs in the
   standard library.
2. **Builtins are not privileged in the type system.** They take and return ordinary types; there
   is no builtin-only type and no builtin-only calling convention.
3. **Filesystem builtins are effectful** in the sense of [N7](#n7-effects-and-asynchrony), and
   their effects propagate to callers. Output builtins are effectful. String, character and
   conversion builtins are pure.

Above the builtins sits a **standard library**, written in Palladium and read like any other
module: core types and traits, collections (`Vec<T>`, `HashMap<K, V>`, `String`, `Option<T>`),
math, buffered and networked I/O, and process/environment access. The dividing line is constraint
1: if it can be written in Palladium, it is library, not builtin.

---

# Part II: Implementation status annex

What `pdc` does at commit `abeb665`, per Part I section. Each row is either a source location or a
command that was run.

| Normative section | Status | Where the detail is |
|---|---|---|
| [N1 Overview](#n1-overview-and-design-commitments) | partial | [A1](#a1-pipeline-and-backends) — C backend works; LLVM backend is skeletal |
| [N2 Lexical structure](#n2-lexical-structure) | partial | [A2](#a2-lexical-structure) — no hex/binary/octal, no numeric separators, no raw strings; every attribute is refused |
| [N3 Program structure and items](#n3-program-structure-and-items) | partial | [A3](#a3-program-structure), [A4](#a4-items) |
| [N4 Types](#n4-types) | partial | [A5](#a5-types) — no floats, slices, fn types; `Option`/`Result` not built in |
| [N5 Statements and expressions](#n5-statements-and-expressions) | partial | [A6](#a6-statements-and-expressions) — `if`, `match`, blocks and `loop` ARE expressions now, with `else if`, compound assignment, bitwise operators, ranges, `as` casts and `x.f()`. What is left: no closures (N5-08) and no `try { }` (N5-09), both owed to M3; `unsafe { }` parses but N5-02 is owed to M5 |
| [N6 Patterns](#n6-patterns) | partial | [A7](#a7-patterns) — literal, range, tuple, or- and `@` patterns with guards; slices, non-enum struct patterns, `ref`/`mut` and field shorthand still absent; exhaustiveness for every scrutinee type, with a trap where no arm is taken |
| [N7 Effects and asynchrony](#n7-effects-and-asynchrony) | unimplemented | [A6.5](#a65-question-mark-async-and-await), and the divergence list in [`async-as-effect.md`](../reference/features/async-system/async-as-effect.md#where-the-implementation-currently-diverges) |
| [N8 Totality](#n8-totality) | unimplemented | [A2](#a2-lexical-structure) — attributes do not lex; no checker exists |
| [N9 References and lifetimes](#n9-references-and-lifetimes) | unimplemented | [A9](#a9-memory-model) — `ref` is not a keyword; no region inference |
| [N10 Traits and generics](#n10-traits-and-generics) | unimplemented | [A4.4](#a44-traits), [A5](#a5-types) |
| [N11 Modules](#n11-modules) | partial | [A3](#a3-program-structure) — `import` works; no `mod` item |
| [N12 Memory model](#n12-memory-model) | partial | [A9](#a9-memory-model) — checked but not typed; `String` is Copy; array parameters [A9.2](#a92-array-parameters); `&mut` of an immutable local refused [A9.3](#a93-mut-of-an-immutable-local-is-refused-was-accepted) |
| [N13 Execution model](#n13-execution-model) | implemented | [A10](#a10-execution-model) |
| [N14 Builtins and stdlib](#n14-builtins-and-the-standard-library) | partial | [A8](#a8-builtins) — the registry is exactly the normative 34 and all are callable; signatures differ (no `Result`); `stdlib/` does not parse |

## A1. Pipeline and backends

The pipeline (`src/driver/mod.rs:49`) is:

```
lex → parse → macro expand → resolve imports → typecheck → borrow check
    → effect analysis (informational only) → unsafe check → optimize → C codegen → gcc
```

The C backend is the real backend. An LLVM text backend exists
(`src/codegen/llvm_text_backend.rs`, 2543 lines) but is skeletal: `break` and `continue` are refused
outright, naming the `%loop_end_placeholder` / `%loop_inc_placeholder` the TODO would have emitted
(`src/codegen/llvm_text_backend.rs:944-946`, `src/codegen/llvm_text_backend.rs:948-950`), `match` is a TODO if/else chain
(`src/codegen/llvm_text_backend.rs:952-964`), and enum construction, `?`, macro invocation and `await` are four
separate refusals (`src/codegen/llvm_text_backend.rs:1479-1492`) — they were ONE catch-all
returning the constant `0`, which is how two distinct enum variants compiled to byte-identical IR. It also bails on ordinary code — "Unsupported iterator type in for loop"
(`src/codegen/llvm_text_backend.rs:845-847`), "Unsupported binary operator" (`src/codegen/llvm_text_backend.rs:1161-1165`), "Complex function calls not yet supported"
(`src/codegen/llvm_text_backend.rs:1302-1305`). No conformance row exercises it.

Generated C is linked against `runtime/palladium_runtime.c`, which supplies 16 file/path symbols.
`pdc` resolves that runtime relative to its own install location — `pdc --print-runtime` shows
which copy it found, and `$PALLADIUM_RUNTIME` overrides it. (Until 2026-08-22 the path was
hardcoded relative to the working directory, so an installed compiler could not link anything.)


**The C backend is the only backend.** An LLVM text backend exists in the tree
(`src/codegen/llvm_text_backend.rs`) and `--llvm` selects it, but as of 2026-08-22 it **refuses
unconditionally**: `LLVMTextBackend::compile` returns `CompileError::Unimplemented` before it looks
at the program. It is retained for development, not for building.

It refuses wholesale rather than per-construct because its gaps are not all loud ones. Seven
constructs failed visibly — `break`, `continue`, enum patterns, enum construction, `?`, `await`,
and stray macro invocations — and each now carries its own diagnostic underneath the gate. Seven
more fabricated rather than refused, without saying so:

- struct field access uses index 0 for reads and writes alike, so `p.y` reads `p.x`
- every unenumerated type becomes `i8*`, and every call is typed `i64` regardless of signature
- `match` on a wildcard or identifier pattern discards the scrutinee and never binds the identifier
- a plain `main` emits `ret void`, putting the process exit status outside the program's semantics
- non-function items (structs, enums) are dropped while expressions still refer to them
- string collection skips `Stmt::Match` and `Stmt::Unsafe`, leaving an undefined `@.str.unknown`

These do not all fail the same way. The last two emit **invalid** IR, which an assembler rejects.
*Some* of the others emit IR that is **valid and means something other than the source**. The
demonstrated case is field-zero access: `struct Point { x: i64, y: i64 }` with `print_int(p.y)`
lowers to `getelementptr i64, i64* %4, i32 0, i32 0` and reads `x`, in a module that assembles,
links and runs. The C backend prints `22`.

That case is why the gate is wholesale: verifying the assembly cannot detect it, so a gate covering
only the loud half would read as protection while providing none.

## A2. Lexical structure

The lexer is `logos`-based (`src/lexer/token.rs`).

**implemented**: decimal integers, floats, chars, strings, booleans, identifiers
(`src/lexer/token.rs`). The sign is part of the integer token, so `i-1` lexes as `i` then
`-1` — and part of the float token too, so `x-1.5` lexes as `x` then `-1.5`. One convention, not
two.

**implemented — floats and chars (N2-03, N2-04).** `3.5` is one token of type `f64`; `f32` and
`f64` are distinct C types (`float`, `double`) and one checker type, because `as` casts do not
exist yet and no program can observe the difference the checker would be drawing. There is **no
implicit widening**: `1 + 2.5` is a type error naming both types, because C would convert it
silently and the language has no cast to write the conversion with. `%` on a float is refused —
C's `%` does not take a double, so accepting it would emit C that gcc rejects.

A char literal denotes its Unicode scalar and **its type is `i64`, not a distinct `char`**. That
is a recorded decision, not an oversight: [N14](#n14-builtins-and-the-standard-library) gives
`string_char_at` a `char` return and `char_is_digit` a `char` parameter, and `src/builtins.rs`
implements both over `i64`. A `char` type on the literal alone would leave `'a'` unusable with
every builtin that consumes a character. N4-04 and N14-04 move together, and both are still owed.

Neither has a printer. N14's builtin set is closed and contains no `float_to_string`, so a float's
digits cannot reach stdout at all; `tests/02_types_floats.pd` asserts values by bracketing them
between comparisons instead.

**implemented — escapes (N2-09).** In a string literal the set is
`\n \t \r \" \\ \'`; a char literal takes those **and `\0`**. Anything else is a
**compile error** that lists the set; it is not passed through. It used to be: `"\q"`
produced the two characters `\` and `q` with no diagnostic, and `"\\n"` produced a backslash
and a LINE FEED because the unescaper was a chain of `.replace()` calls in which the `\n` rule
reached the second backslash's `n` first.

**`\0` is refused in a string literal and legal in a char literal**, and the asymmetry is the
decision rather than an omission. A String is a non-NULL, NUL-terminated `const char*`
([N14](#n14-builtins-and-the-standard-library)), so `"a\0b"` denotes three characters and every
String operation sees one: `print` stops at `a`, `string_len` answers 1. That was ACCEPTED for one
round, with the consequence documented here — and documenting a representation leak does not stop
it, it records that it shipped. A literal whose value no operation in the language can observe is
the same defect [N2-11](#n2-lexical-structure) refuses one construct along, where an attribute that
lexes and is ignored makes the source say one thing and the binary another.

`'\0'` stays legal because a char literal is a Unicode scalar held as `i64` and zero is an
ordinary value there, so nothing that was expressible is lost. The route to making the string form
legal is **length-aware String semantics** — a pointer plus a length — which is an N4/N14 change
and not a lexer one. Pinned by
`tests/m2_lexical.rs::{a_nul_in_a_string_literal_is_refused, a_nul_in_a_char_literal_is_still_the_value_zero, the_string_escape_set_is_the_char_set_minus_nul}`.

**unimplemented**: hex (`0x`), binary (`0b`), octal, numeric separators, raw strings, `\xNN` and
`\u{}` escapes, string interpolation. No lexer rule produces any of them.

**implemented — attributes lex, and every one of them is refused (N2-10, N2-11).** `#`, `#!`,
`#[name]`, `#[name(args)]` and `#![name(args)]` all lex and parse. The set of attributes the
compiler implements (`KNOWN_ATTRIBUTES`, `src/parser/mod.rs`) is **empty**, so every attribute is
a compile error naming the attribute:

```
error: unknown attribute `total`
  = note: this compiler implements no attributes yet: `#` lexes so that the surface
    exists, and every attribute is refused so that none can be silently ignored
```

The emptiness is the point, and the two rows are one change. An attribute that lexed and was then
dropped would compile `#[total]` into a binary with no totality check in it — a source that
requests a property and a binary without it, which is the defect class M1 was spent deleting.
[N8](#n8-totality) is therefore no longer blocked *lexically*; it is blocked on M6 having something
to discharge the obligation with.

The 29 keywords the lexer recognizes (`src/lexer/token.rs:232`):

```
fn let mut if else while return true false for in break continue
struct enum trait impl match import pub as Self self type const
unsafe async await macro
```

Note `import`, not `use`. **`loop`, `mod`, `use`, `where`, `dyn`, `move`, `static`, `ref`,
`crate`, `super`, `extern`, `try`, `with`, `effect` are NOT keywords** — they lex as ordinary
identifiers, so `let loop = 1;` is legal and `loop { }` is a parse error at the `{`. The absence
of `ref` is why the normative reference syntax of [N9](#n9-references-and-lifetimes) does not
parse; the absence of `with` and `effect` is why the effect contexts of
[N7](#n7-effects-and-asynchrony) do not.

`async` and `await` **are** keywords — the two things [N7](#n7-effects-and-asynchrony) says the
language does not have are the two the implementation has.

**Operators. implemented**: `+ - * / % = == != ! < > <= >= && ||`, the compound assignments
`+= -= *= /= %=`, the bitwise set `& | ^ ~ << >>`, `..=`, and `as` casts. `$` is lexed and never
consumed by the parser; `|` now IS consumed, as bitwise or.

**`>>` IS NOT A TOKEN, AND THAT IS DELIBERATE.** `Option<Vec<Stmt>>` closes two generic argument
lists with two adjacent `>`, so a longest-match `>>` token would swallow both and break every
nested generic in the tree. The shift is recognised in the parser from two `Gt` tokens whose spans
TOUCH, which is why `a > > b` is not a shift. `<<` is an ordinary token: the two `<` of
`Vec<Vec<T>>` always have an identifier between them.

Comments (`// line`, `/* block */`) are implemented **and they nest** (N2-08):
`/* a /* b */ c */` is one comment, and the `c` between the two closes is not live source. A
regular expression cannot count, so comment scanning is a callback on the `/` token
(`slash_or_comment`, `src/lexer/token.rs`) rather than a `#[logos(skip)]` pattern. A `/*` that
nothing closes is an error rather than a comment to end of file — otherwise deleting one `*/`
would compile a shorter program than the author wrote.

`bootstrap/pdc.pd`'s own hand-written scanner still stops at the first close with no depth
counter; the divergence is recorded in
[`bootstrap-subset.md`](bootstrap-subset.md) and is not observable, because no PBS-1 source
contains a nested comment.

## A3. Program structure

```ebnf
program = { import } { item } ;
```

**Imports must all precede items** — the parser drains imports first
(`src/parser/mod.rs:674-675`), so an `import` after a `fn` is a syntax error.

```ebnf
import = "import" path [ "as" identifier ] ";"
       | "import" path "::" "*" ";"
       | "import" path "::" "{" identifier { "," identifier } "}" ";" ;
```

Items (`src/parser/mod.rs:937`): `fn`, `struct`, `enum`, `trait`, `impl`, `type`, `macro`.
**unimplemented: there is no top-level `const`, `static`, `mod`, or `use` item** — so
[N11](#n11-modules)'s file-based modules exist only as far as `import` reaches.

## A4. Items

### A4.1 Functions

```ebnf
function = [ "pub" ] [ "async" ] "fn" identifier [ generic_params ]
           "(" [ params ] ")" [ "->" type ] block ;
param    = [ "mut" ] identifier ":" type | self_param ;
self_param = [ "&" ] [ "mut" ] "self" ;
```

**implemented**: parameters, return types, `pub`, `self` receivers in `impl` blocks.
**unimplemented**: default parameter values, pattern parameters, varargs, `where` clauses.

**unimplemented — effect clauses.** `![io]` does not exist in the surface syntax.
`Function.effects` is hardcoded `None` by the parser (`src/parser/mod.rs:1275`, corrected from
v0.2's `src/parser/mod.rs:1259`, which is where the `Function` literal opens). Effects are *inferred* afterwards
(`src/effects/mod.rs`) and only printed by the driver (`src/driver/mod.rs:176`, corrected
from `src/driver/mod.rs:164-170`); they gate nothing. `crate::effects::` is referenced from exactly one place in
the compiler, `src/driver/mod.rs:172`.

`async fn` is accepted and typechecked: `async fn g() -> i64 { return 1; }` fails with
"Type mismatch: expected Future<Int>, found Int", i.e. the return type is wrapped in a `Future`.
Under [N7](#n7-effects-and-asynchrony) neither the keyword nor the wrapper should exist.

### A4.2 Structs

**implemented** field types: `i64`/`i32`/`u32`/`u64`, `bool`, `String`, `[T; N]`, other structs,
enums.

**partial** — field types that parse and then fail in codegen (all three corrected from v0.2,
which was ~250 lines low):
- generic → "Generic types in structs not yet supported" (`src/codegen/mod.rs:2721-2724`)
- reference → "Reference types in structs not yet supported" (`src/codegen/mod.rs:2728-2728`)
- tuple → "Tuple types in structs not yet supported" (`src/codegen/mod.rs:2736-2740`)

### A4.3 Enums

**implemented**: unit, tuple, and struct variants; construction and `match` both work.
`pub` is honoured (`src/parser/mod.rs:988-992`, `src/ast/mod.rs:148`): a module's `enum` reaches a
downstream program only if it said `pub`, and the refusal for one that did not is
`Undefined enum type: <name>` before any C exists.

*This paragraph said the opposite until 2026-08-23 — "`pub` on an enum is parsed and then silently
discarded, `EnumDef` has no visibility field" — and it was accurate: the parser read the keyword and
dropped it for this one item kind, so `enum X` and `pub enum X` produced the same AST. The
consequence was not cosmetic. Enum-kind discovery unioned every imported enum name into one bare-name
set, so an enum in any imported module could misclassify a downstream local type of the same name and
the program was refused with `Type mismatch: expected Color, found Color`. The keyword now decides,
and `local_type_shadows_import` decides the rest.*

### A4.4 Traits

**unimplemented.** Traits parse (`src/parser/mod.rs:1465`, corrected from line 736–960 of the pre-cleanup revision) and then
emit nothing — codegen ignores `Item::Trait` (`src/codegen/mod.rs:2185-2188`, corrected from line 754–757 of the pre-cleanup revision). Trait method bodies are never typechecked (`src/typeck/mod.rs:2186-2187`, corrected
from `src/typeck/mod.rs:2586-2586`). Additionally, a trait method declared with a `self` receiver is a **parse error**,
because trait methods use a separate parameter loop that does not handle `self`
(`src/parser/mod.rs:1588-1589`, corrected from line 863–897 of the pre-cleanup revision).

So `trait Display { fn fmt(&self) -> String; }` does not parse, and
[N10](#n10-traits-and-generics) has no implementation at all.

`tests/07_traits_basic.pd` PASSES conformance while only printing that traits are unimplemented.

### A4.5 Impl blocks

```ebnf
impl_block = "impl" [ generic_params ] [ type "for" ] type "{" { function } "}" ;
```

**implemented**: methods become mangled free functions `__pd_Type_method`
(`src/codegen/mod.rs:2202-2207`, corrected THREE TIMES: from line 1861 of the pre-cleanup
revision; on 2026-08-23 from `1174-1180`, which was the file-I/O prelude and had nothing to do with
method mangling — the line numbers had been tracked through an edit while the target was never
re-read; and on 2026-08-25, when `4690ef0` inserted above it).
**implemented**: `Self` in a method signature resolves to the type the block is for
(`src/ast/mod.rs:230`), and BOTH the type checker and code generation call that one function. Until
`4690ef0` the return type was substituted in code generation alone, so `fn new(..) -> Self` worked
while `fn area(self)` reached the C compiler as `struct Self self` — a type nothing declares.
**unimplemented**: associated constants and associated types are rejected — an impl body may
contain only `fn` (`src/parser/mod.rs:1756-1762`, corrected from line 1030 of the pre-cleanup revision).
**implemented**: methods are called with `.` syntax — see [A6.4](#a64-method-calls).
`Type::method(receiver, args)` also works, which it did not when this section recommended it.

### A4.6 Macros

**partial.** User macros (`macro name!(a, b) { … }`) parse into a raw token stream that is lossily
converted; unlisted tokens degrade into `AstToken::Ident` of a debug string
(`src/parser/mod.rs:1987`, corrected from line 1258 of the pre-cleanup revision).

Four builtin macros exist (`src/macros/mod.rs:41`), each taking **exactly one** expression:

| Macro | Expands to | Status |
|---|---|---|
| `println!(e)` | `print(e); print("\n")` | implemented (one argument only — `println!()` and `println!(a, b)` fail) |
| `assert!(c)` | `if (!(c)) { panic("Assertion failed"); }` | implemented |
| `vec![e]` | `[e]` — a **1-element array**, not a growable vector | partial; misleading name |
| `dbg!(e)` | calls `print_debug` | unimplemented — `print_debug` is defined nowhere (`src/macros/mod.rs:167`, corrected from line 161 of the pre-cleanup revision) |

Macro hygiene ([N3](#n3-program-structure-and-items)) is unimplemented:
`grep -rn hygien src/ --include='*.rs'` returns nothing.

## A5. Types

| Syntax | Status | Note |
|---|---|---|
| `i64`, `int` | implemented | `int` is an alias for `i64` (`src/parser/mod.rs:3535`, corrected from line 2038 of the pre-cleanup revision) |
| `i32`, `u32`, `u64` | implemented | primitive table at `src/parser/mod.rs:3524-3531` (corrected from line 2037–2043 of the pre-cleanup revision) |
| `bool`, `String` | implemented | |
| `()` | implemented | unit |
| `[T; N]` | implemented | one dimension, `N` an integer literal. `N` as an identifier parses but is dropped (const generics, below), so such an array is uncallable and its `for` loop is a compile error |
| `[[T; M]; N]` | unimplemented | nested arrays do not work in either position. As a **local**, `type_to_c` builds `T[M][N]` as a *type* and emits `long long[2] grid[2]`, which gcc rejects ("brackets are not allowed here"); as a **parameter** the declarator refuses it by name. Separate unit |
| `&T`, `&mut T` | partial | parses, but the typechecker is a **no-op**: `Type::Reference` maps to its inner type — "For now, treat references as the inner type / TODO: Proper reference type handling" (`src/typeck/mod.rs:714-718`, corrected from line 2470–2486 of the pre-cleanup revision). `&i64` and `i64` are indistinguishable to it. |
| `ref T`, `ref mut T` | unimplemented | `ref` is not a keyword; `fn f(x: ref String)` fails with "expected ')', found identifier 'String'" |
| `Name<A, B>` | partial | see below |
| `(A, B)` | **implemented** | one C struct per SHAPE, mangled from the element C types and emitted with a constructor (`src/codegen/mod.rs:4354`); `void*` is gone. Arity two or more. A tuple in an ENUM PAYLOAD is refused by name — tuple structs are emitted after the enum definitions because an element may be an enum, and a payload of tuple type needs the reverse order |
| `f64` | **implemented** | the type of a float literal since N2-03; C `double` |
| `f32` | partial | parses and maps to C `float`, but shares one checker type with `f64`, so nothing can observe the difference |
| `char` | partial | `'a'` LEXES and carries the right scalar (N2-04), and its TYPE is `i64` — N4-04, still owed, moves with N14-04's `string_char_at` |
| `str`, `u8`, `usize` | unimplemented | not in the primitive table |
| `fn(A) -> B` | unimplemented | function types are unparseable |
| `[T]` slices, `dyn T`, `impl T` | unimplemented | |
| `<T: Bound>`, `where` | unimplemented | `parse_generic_params` accepts bare names only; the `:` is a parse error |

**partial — generic argument bug**: inside `<…>`, any identifier whose characters are all
uppercase or `_` is reclassified as a *const generic argument* (`src/parser/mod.rs:3564-3574`,
corrected from line 2054–2079 of the pre-cleanup revision). So `Foo<T>` yields a const-generic `T`, not a type argument. Only
mixed-case names like `Vec<Item>` reach the type branch.

**partial — const generics**: they parse, and in codegen an `ArraySize::ConstParam` is emitted
into C verbatim as the parameter's *name* while an `ArraySize::Expr` becomes the literal `"0"`
(`src/codegen/mod.rs:2237-2241`, corrected on 2026-08-23 from `1204-1206`, which was
`return pd_file_flush(handle);` — a citation about const generics pointing at the file-I/O
prelude). Neither is monomorphised. *(v0.2 said "array sizes from a const
parameter resolve to `0`" citing `src/codegen/mod.rs:631-631`; that is the expression case, not the const-parameter
case.)*

`tests/08_generics_basic.pd` PASSES conformance while only printing that generics are
unimplemented.

### A5.1 Option and Result

**unimplemented as built-ins.** There is no built-in `Option` or `Result` — no prelude, no
declaration, no lexer or parser support. They are ordinary user enums if you declare them, with no
methods and no `?`. Declaring one does not make `?` work: the operator is rejected outright (see
[A6.5](#a65-question-mark-async-and-await)), because nothing lowers it onto the representation your
enum is compiled to. Use `match`.

**unimplemented as built-ins.** There is no prelude, no declaration, no lexer or parser support.
They are ordinary user enums if you declare them. The only special-casing left is the REFUSAL: `?` is
rejected outright by the type checker (`src/typeck/mod.rs:4262-4262`) and again by code generation
(`src/codegen/mod.rs:5869-5873`). It used to typecheck against a `Generic{name:"Result"}` shape
and then emit C for a `struct Result` layout nothing defines (see
[A6.5](#a65-question-mark-async-and-await)).

## A6. Statements and expressions

### A6.1 Statements

`let`, assignment, `if`/`else`, `while`, `for … in`, `match`, `return`, `break`, `continue`,
`unsafe { }`, expression statements (`src/parser/mod.rs:2075`).

- implemented: `let [mut] x [: T] = e;` — **the initializer is mandatory**
  (`src/parser/mod.rs:2203`, corrected from line 1411 of the pre-cleanup revision); the binding must be a plain identifier
  (no patterns).
- implemented: assignment targets — identifier, index, field, deref.
- **implemented: `else if`** (N5-06). After `else` the parser looks for `if` and recurses
  (`src/parser/mod.rs:2014-2016`), so a chain is nesting and there is no `ElseIf` node. The branch tail
  travels with it, which is what keeps a tail-position chain returning. *(This bullet read
  "unimplemented — after `else` the parser requires `{`" until `66dab38`.)*
- **implemented: `loop`** (N5-07), a keyword since `src/lexer/token.rs:250`, parsed at
  `src/parser/mod.rs:2603` and emitted as C `while (1)` (`src/codegen/mod.rs:3635`). Its `break`
  may carry a value. *(It read "not a keyword. Use `while true`" until `f729cda`.)*
- **implemented: compound assignment** `+= -= *= /= %=` (N5-13), DESUGARED at
  `src/parser/mod.rs:2110-2112` into `t = t op v` rather than emitted as C's own compound operator —
  Palladium's `+` on `String` is a runtime concatenation call, which C's `+=` cannot express. The
  residual that buys: the target is written twice, so it is evaluated twice, and `a[next()] += 1`
  calls `next()` twice.

  **AND THAT IS NOT A NORMATIVE VIOLATION, WHICH IS WORTH STATING RATHER THAN ASSUMING.**
  [N5](#n5-statements-and-expressions) names the compound operators and says nothing about how
  often the place is evaluated; [N13](#n13-execution-model)'s only evaluation-order sentence is
  "Arguments are evaluated left to right", and the left-hand side of an assignment is not an
  argument. So N5-13 is `satisfied` on the requirement as written. It is a deviation from the
  language this one is modelled on, where the place is evaluated once, and it is recorded here
  because the next person to write that rule down should know it costs a place-expression
  lowering — binding the subscript to a temporary before the read — and not a one-line change. *(It read "unimplemented — verified: `Expected expression, but found
  '='`" until `ef74eba`.)*
- implemented: bare nested blocks as statements — a `{ … }` in statement position parses, as a
  side effect of blocks becoming expressions (N5-05). unimplemented: `try { }` blocks (N5-09).
- implemented: `break` / `continue`, unlabeled. **`break` MAY CARRY A VALUE, and only out of a
  `loop` used as one** (N5-07): `let x = loop { … break v; };`. The two directions are both
  refused — a valued `break` out of a loop written for its effect has nowhere to put the value,
  and a VALUELESS `break` out of a value `loop` leaves the binding on the other side unwritten,
  which emitted an uninitialised C temporary and then read it until `1f64c32` refused it.
  `continue` is valueless in every position. *(This bullet read "unlabeled, valueless" until
  N5-07.)*

`unsafe { }` parses and `src/unsafe_ops` runs (`src/driver/mod.rs:189-197`), but raw pointer types
and `unsafe fn` do not exist, so [N12](#n12-memory-model)'s restricted-unsafe is unimplemented.
`tests/11_unsafe_blocks.pd` PASSES while only printing that.

### A6.2 `for` loops

`for i in 0..n { }` — implemented. `for x in arr { }` — implemented, including where `arr` is a
**function parameter**. Codegen used to emit `sizeof(arr)/sizeof(arr[0])`, the pointer size after
array-to-pointer decay, so the loop silently visited 1 element (`i64`) or 2 (`i32`), and it
hardcoded the element type as `long long`. The bound now comes from the declared length and the
element type from the declared element type. A length codegen cannot resolve — a const generic,
which [N4](#n4-types) records as dropped — is a compile error on a parameter rather than a wrong
bound, because a decayed pointer cannot supply the length at run time either.

### A6.3 Expression forms

implemented: literals, identifiers, struct literals, array literals `[a,b,c]` and `[v; n]`,
indexing, field access, calls, enum construction, unary `- ! & *`, binary operators.

- **implemented: `if`, `match`, blocks and `loop` are EXPRESSIONS** (N5-03/04/05/07). All four are
  read at the primary level (`src/parser/mod.rs:3687-3699`) and each reuses the statement parser it
  already had, reinterpreting the statements-plus-tail it returns as statements-plus-value. C has
  no expression with a block in it, so they lower by HOISTING: a temporary, a statement-form
  computation that assigns it, and a use of the name. GNU statement-expressions would say it in one
  line and are not available — the backend is whatever `cc` the host has. *(This bullet read
  "unimplemented: they are statements, not expressions … a direct contradiction of N5" until
  `66dab38`, `f729cda`.)*
- unimplemented: closures — no closure token path and no closure AST node.
- **implemented: tuple expressions and `.0` indexing** (N4-12). `(a, b)` builds a value and `p.0`
  reads an element; the index is SYNTAX, read at compile time, because a tuple's elements may have
  different types and an index the compiler cannot read has no type to be. Two subsets, both
  refused by name: a tuple takes two or more elements (`(e)` is grouping, and a one-element `(e,)`
  would be a meaning decided by a trailing comma), and a chained index must be parenthesised —
  `(p.0).1`, because `.0.1` lexes as one float literal (`[0-9]+\.[0-9]+`) and `p.0.10` and `p.0.1`
  both round-trip to 0.1, so the second index cannot be recovered without guessing.
- **implemented: `as` casts** (N5-15), parsed between multiplication and unary
  (`src/parser/mod.rs:3375-3376`) so `10 / 4.0 as i64` is `10 / (4.0 as i64)`, and chainable. THE LEGAL
  SET IS NARROW BECAUSE THIS DOCUMENT DOES NOT SAY WHAT IT IS: N5 names `as` casts and the grammar
  gives the form, neither says which conversions are meant, so conversions among the numeric
  primitives and `bool` are implemented and every other cast is refused by name. A cast to `bool`
  emits `((x) != 0)` rather than a C cast, because `bool` is C's `int` and `(int)5` is 5.
  unimplemented: string interpolation.
- **implemented: ranges outside a `for` header** (N5-14), including `..=`. A range is a value —
  `typedef struct { long long start; long long end; int inclusive; } __pd_range` — with the end
  kept as written beside a flag rather than normalised to `end + 1`, which would wrap at the
  maximum. A range written IN a `for` header keeps its old fast path and builds no struct.
  *(This bullet read "partial — codegen error 'Range expressions can only be used in for loops'"
  until `ef74eba`.)*
- partial: empty array literal `[]` — typeck cannot infer the element type
  (`src/typeck/mod.rs:5354-5358`, corrected from line 1874 of the pre-cleanup revision and again on 2026-08-25, when it had come to rest on a closing brace).

**FIXED — the precedence bug**: `parse_multiplication` parsed its RIGHT operand with
`parse_postfix` rather than `parse_unary`, so the left side descended through the unary level and
the right side could not, and `a * -b` did not parse. It now parses both sides through the cast
level, which is `parse_unary` plus the `as` suffix (`src/parser/mod.rs:3463-3463`). Every other level of
the ladder was already symmetric, which is why this was the only expression that failed.
[N5](#n5-statements-and-expressions) requires `a * -b`; `ef74eba` delivered it.

### A6.4 Method calls

**implemented** (N5-17, `4690ef0`). `x.f(a)` parses as a call whose callee is a field access, and
both the type checker (`src/typeck/mod.rs:3307`) and code generation (`src/codegen/mod.rs:5398-5401`)
REWRITE it into the path call it means — `TypeOfX::f(x, a)` — rather than checking and emitting it
as a second kind of call. The receiver becomes the first argument and is evaluated exactly once;
its position among the arguments is C's unspecified evaluation order, which is the same residual
every multi-argument call here already has, and is why [N13](#n13-evaluation-and-arithmetic)'s
left-to-right row (N13-03) stays owed.

**THREE THINGS WERE BROKEN, AND THIS SECTION KNEW ABOUT ONE.** It said `x.f()` was refused with
"Indirect function calls not yet supported", which was true. It also said, twice — here and in
[A4.5](#a45-impl-blocks) — to call methods as `Type::method(receiver, args)` instead. **That did
not work either**, and had not: the parser builds every `A::b(...)` as an enum constructor, so a
struct on the left produced "Undefined enum type: Rect". The recommended workaround for an
unimplemented feature was itself unimplemented, and nothing measured it because no fixture used
it. The third was `Self` — see [A4.5](#a45-impl-blocks).

The rule that separates a constructor from a path call cannot live in the parser, which has no
types: **it is an enum constructor if and only if the name is an enum's**, asked of the enum table
by the type checker and again by code generation, each stating it where it applies it.

**unimplemented: the `&self` receiver.** `fn area(&self)` emits a `const struct Rectangle*`
parameter while the call site passes the value, and the C compiler refuses our own output with
"take the address with &". Auto-referencing a receiver needs a real reference type; the debt is
`test_struct_with_methods` in `tests/rust-debt-manifest.txt`, owned by M4.

*(v0.2 also claimed a "same guard" in codegen at line 1870 of the pre-cleanup revision.
`grep -n 'Indirect function calls' src/codegen/mod.rs` returns nothing; there is no such guard in
codegen at any line. Claim withdrawn — and written without a citation form on purpose, so the gate
does not pin a line this document is calling wrong.)*

### A6.5 Question mark, async and await

**unimplemented — rejected, not lowered.** *(This section previously read "partial — silent
breakage", describing C that referenced an undefined `struct Result` layout and a `poll` member
nothing generated. Defect D5 was fixed on `main` in commit `439b241`; both are now refused at
typecheck. The silent-breakage description is retracted.)*

*Two bullets stood here restating the retracted description in the PRESENT tense — "`?`
generates C that references a `struct Result` layout", "`.await` emits
`while (!<tmp>.poll(&<tmp>)) { }`" — three lines after the retraction above and thirty before
"What they used to do" below said the same thing in the past tense. A reader could not tell
which paragraph described the compiler. They are deleted, not repointed: their line citations
had drifted onto an enum-variant lookup and a bare `));` respectively, so they were not
evidence for the claim either way.*

```
error: the `?` operator is not implemented
  --> prog.pd:11:28
11 |     let v: i64 = might_fail(x)?;
   |                            ^~~~
  = note: code generation has no lowering of `?` onto the enum representation it emits,
          and would instead produce C for a `struct Result { int is_ok; union … }` layout
          that no enum is ever generated as
  = help: there is no error-propagation operator; return the value and dispatch on it
          with `match`. Only non-generic enums are compiled, so declare a concrete one
          such as `enum Result { Ok(i64), Err(i64) }` — `Result<T, E>` will not compile
```

Note what is *not* claimed: `Result` is not a missing type — you can declare one, and before
this refusal existed that is how a program reached code generation. What is missing is the
lowering onto the representation enums actually get.

The refusal fires on the operator itself, before the operand is examined, so `3?` and
`unknown()?` reach it too. The wording is therefore phrased for any operand: it does not assert
that what precedes `?` is a Result, because in those programs it is not.

The `match` alternative is bounded, and the help says where it stops rather than leaving it to be
discovered. Measured: dispatch works, propagation out of a helper works, payload types other than
`i64` work — but a generic `Result<T, E>` does **not** compile, because code generation skips
generic enum definitions (`src/codegen/mod.rs:2048-2052`, `src/codegen/mod.rs:2018-2022`, `src/codegen/mod.rs:1920-1922`) and generic enum construction
infers only the parameters a variant mentions, so `Result::Err(e)` yields `Result<(), E>`. One
syntactic trap is worth stating: a `match` arm that is a block must not be followed by a comma,
and propagation needs block arms because `return` is not an expression.

The refusal is raised by the type checker (`?` at `src/typeck/mod.rs:4262-4262`, `.await` at
`src/typeck/mod.rs:4269-4269`) and again by code generation (`?` at `src/codegen/mod.rs:5869-5873`,
`.await` at `src/codegen/mod.rs:5881-5885`), which is callable on its own.

What they used to do:

- `?` emitted C referencing a `struct Result { int is_ok; union {…} data; }` layout that **no
  other part of codegen emits** — enums are generated with a `.tag` field and `__Enum__Variant`
  constants instead. gcc reported `variable has incomplete type 'struct Result'`.
- `.await` emitted `while (!f.poll(&f)) {}`. C has no member function calls, and the poll
  routine that *was* generated was the free function `<name>_poll`, which that call never named.
  That generator is deleted too — an `async fn` is refused at
  `generate_function_with_name` — so no live line carries this claim and it is written
  without a citation form, as A6.4's withdrawn claim is. There is no async runtime.

Both lowerings are deleted rather than kept behind a flag: they encoded a representation a real
implementation must not reuse, and version control holds them.

The LLVM backend WAS a sharper case, and this paragraph used to describe it in the present tense
while §A6 two hundred lines up already described the repair. Its expression lowering had no arm for
either node: a catch-all returned the constant `0` for `Question`, `Await`, `EnumConstructor` and
`MacroInvocation` alike, which compiled and was wrong. That catch-all is gone — each of the four is
its own refusal now (`src/codegen/llvm_text_backend.rs:1479-1492`), and the type checker refuses
before a backend is chosen besides, which is what `tests/d5_unimplemented_constructs.rs` pins. The
deleted catch-all is described without a citation form, because there is no live line to cite.

`async fn` still *declares* fine; only `.await` is rejected, and it is rejected on any operand —
`some_variable.await` as much as a call. Historically the only shape that reached code generation
was a plain function declared `-> Future<T>`, because a call to an `async fn` is typed as its bare
return type and so awaiting one never type checked; that is a fact about the old type rules, which
no longer gate anything. The workaround is phrased conditionally for the same reason. Where a
`-> Future<T>` signature *is* involved, note that deleting `.await` alone leaves a `Future<T>`
where a `T` is required, so the signature has to change too.

Both are excluded from the bootstrap subset.

Under [N7](#n7-effects-and-asynchrony) the correct end state is not a working `.await` but no
`.await`: the operator is not part of the language. Making it a hard compile error is a step
toward the definition, not away from it. The full divergence list, including the ordering bug in
effect propagation and the fact that `impl` methods are never effect-analysed, is in
[`async-as-effect.md`](../reference/features/async-system/async-as-effect.md#where-the-implementation-currently-diverges).

`tests/09_effects_system.pd` and `tests/10_async_await.pd` PASS conformance while only printing
that the features are unimplemented.

### A6.6 Tail expressions

`fn add(a: i64, b: i64) -> i64 { a + b }` — a function body ending in an expression rather than a
`return`.

This is in the grammar (`grammar.ebnf`) and it previously **compiled cleanly and returned
garbage**: the generated C was `long long add(...) { (a + b); }` with no `return`, and `add(2,3)`
printed `6162934856`. No error, no warning, wrong answer — the project's most dangerous defect
class.

**Two corrections to the previous version of this paragraph.**

*Retracted: the blast radius.* It said "and every function in `stdlib/` that ended in an expression
was affected." That is false. Measured at `abeb665`, **0 of the 21 `.pd` files under `stdlib/`
compile** — every one is rejected at lex or parse time, so nothing there was ever compiled and the
defect cannot have lived there. The affected-`stdlib/` claim was a counterfactual stated as a
finding. A related over-correction is also retracted: an earlier phrasing that the resolver "never
loads" the prelude was too strong. The resolver is live and reads `$PALLADIUM_PATH`
(`src/resolver/mod.rs:51-52`); imports use `import`, not `use`. What holds is narrower and
measured: `stdlib/` is on no default search path, and forcing it on does not help —
`PALLADIUM_PATH=…/stdlib/std` with `import option;` gives
`error: Unexpected token: expected 'fn' for method, found 'pub'`. See [A8](#a8-builtins) for
packaging.

*Corrected: "It is fixed" was half true.* The parser lowers a tail **expression** to
`Stmt::Return`, but not a tail `if`. Measured at `abeb665`:

```
fn fib(n: i64) -> i64 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }
fn main() { print_int(fib(10)); }
```

compiles clean and prints **`8261746944`** where the answer is 55. The generated C is

```c
long long fib(long long n) {
    if ((n <= 1)) {
    n;
    } else {
    (fib((n - 1)) + fib((n - 2)));
    }
}
```

— bare expression statements, no `return`, in the single most idiomatic shape a recursive function
takes. So **D3 is open, not fixed**, for every function whose body is a tail `if`. A tail expression
in any other nested block is likewise not lowered.

The "437 affected sites" figure quoted elsewhere is an **understatement, not an upper bound**: the
heuristic that produced it requires a bare expression immediately before the closing brace, and a
tail-`if` function ends with the `else` block's `}`. A companion scan found **369 further sites** of
that shape. *(Both counts are the stdlib unit's measurement, reproduced here by reference; this unit
verified the `fib` reproduction and the generated C above directly.)*

`CLAUDE.md:66` records D3 as fixed. That is accurate only for the tail-expression case.

Regardless, **write explicit `return` in every value-returning function.** The bootstrap compiler
does, which is why `make selfhost` is unaffected by any of this.

## A7. Patterns

**partial — the eight [N6](#n6-patterns) rows M2 owns are satisfied; four pattern forms N6 names
remain absent.** `src/ast/mod.rs:531` defines the variants:

```ebnf
pattern = pattern_primary { "|" pattern_primary } ;

pattern_primary = "_"
        | identifier [ "@" pattern_primary ]
        | literal
        | literal ( ".." | "..=" ) literal
        | "(" pattern "," pattern { "," pattern } ")"
        | path "::" identifier [ "(" pattern { "," pattern } ")"
                               | "{" identifier ":" pattern { "," … } "}" ] ;
```

**implemented**: literal patterns (`1 =>`, `"s" =>`, `true =>`, and `-1 =>`, whose minus the pattern
parser reads as part of the literal) — N6-02; range patterns `lo..hi` and `lo..=hi` — N6-03; tuple
patterns — N6-05; or-patterns `A | B` — N6-07; `@` bindings — N6-08; arm guards `if cond` — N6-09.
A guard is checked inside the arm's own scope, after its bindings, so `Num(n) if n > 5` can read
`n`.

**Two named SUBSETS of N6 and one rule this implementation OWNS, each pinned by a `reject` fixture
rather than left to a reader.** The subsets: a range's endpoints are integer literals where the
normative production says `expression` — nothing in pattern position can be evaluated — and both
ends are required; and a tuple pattern takes two or more elements, because `(p)` is grouping. The
owned rule: an EMPTY range (`5..=1`, `3..3`) is refused, which N6 does not ask for. An arm that can
never be taken is a transposition far more often than a deliberate no-op, and the alternative is a
silently dead arm; if the owner prefers the spec's literal reading, that is the rule to delete. An
ALTERNATIVE MAY NOT BIND: `P::Num(x) | P::Pair(x, _)` is refused with `x` named, because every
alternative would have to bind the same names at the same types for it to mean anything, and the
arm is emitted as one `||` condition with no per-alternative site to assign from.

**unimplemented**: slice patterns (N6-06, owned by M3 — there is no slice type yet), non-enum
struct patterns, `ref`/`mut` bindings, field shorthand (`Move { x, y }`), `..` rest.

**Exhaustiveness holds for every scrutinee type** (N6-10). An enum must cover its variants; a
`bool` is covered by `true` and `false` together; every other type needs an arm that matches every
value — `_`, a binding, `name @ <irrefutable>`, an `a | b` with an irrefutable alternative, or a
tuple of irrefutables (`src/typeck/exhaustiveness.rs:114`, `src/typeck/mod.rs:4554`). NO INTERVAL
ARITHMETIC IS PROMISED: `0..=59` beside `60..=<i64 max>` beside `<i64 min>..=-1` covers every
integer and is still refused, and the diagnostic says why rather than leaving the reader to infer
it. A guarded arm counts toward nothing — whether it is taken is not decidable from the pattern.

Codegen lowers `match` to an if/else-if chain (`src/codegen/mod.rs:3974-3976`,
`src/codegen/mod.rs:4068-4073`) whose final `else` TRAPS (N6-11): it prints
`no match arm was taken in <function> at line <n>` to stderr and calls `abort()`
(`src/codegen/mod.rs:4095-4097`). An arm carrying a guard cannot live in that chain — the guard
needs a statement position after the bindings it reads, and a guard that FAILS must fall through to
the next arm — so a `match` with any guard is emitted as a sequence of `if (pattern) { … goto
_match_endN; }` ending in the same trap before the label (`src/codegen/mod.rs:4010`). The `goto` is
what makes the fall-through path unconditional, which is what lets `-Werror=return-type` be armed
in the shared gcc invocation.

With N6-10 enforced the trap is unreachable for a well-typed program, which is the point of it:
it defends the gap between what the checker proves and what a process can hold. `tests/n6_match.rs`
proves it behaviourally by corrupting an enum tag in the generated C, because the language itself
offers no route to a fall-through.

Consequence: **you can dispatch on an integer, a string or a bool with `match`.**

## A8. Builtins

**partial.** 34 builtins are registered — exactly the 34 that
[N14](#n14-builtins-and-the-standard-library) defines — and **all 34 can be called**. Registry
membership and callability are different claims and the earlier wording ("38 builtins exist and
work") conflated them; `file_flush` and `file_seek` had never been callable at all until
2026-08-23, when their C wrappers were re-based onto the `long long` handle table. The generated
table [`docs/reference/builtins.md`](../reference/builtins.md) is produced by
`scripts/gen-builtin-docs.py` from `src/builtins.rs` and is the authoritative record of *what
`pdc` provides today*; it is checked against the registry on every test run
(`src/builtins.rs::test_generated_builtin_reference_is_not_stale`), which it was not when it went
four names stale.

Measured against N14: the name sets are **equal in both directions** — `normative − implemented =
none` and `implemented − normative = none` — and that is a test, not a reading
(`src/builtins.rs::test_registry_is_exactly_the_normative_builtin_set`). *(Until 2026-08-23 there
were 38, the extra four being `file_open_ex`, `file_close_ex`, `file_read_ex` and `file_write_ex`;
see the reconciliation note under [N14](#n14-builtins-and-the-standard-library).)*

One divergence remains, and two are closed:

- **Signatures — OPEN.** Filesystem builtins return `i64`/`bool` handles rather than `Result`,
  because `Result` is not built in ([A5.1](#a51-option-and-result)); and `string_char_at` returns
  `i64` rather than `char`, because `char` is not a type ([A5](#a5-types)). This is N14-03 and it
  belongs to M3, which is where `Result` arrives.
- *(CLOSED 2026-08-23 — **`file_flush` and `file_seek` could not be compiled**. Both were declared
  over an `i64` handle here and over an opaque `FileHandle` (`typedef void*`) in the emitted C
  prelude, and `file_seek`'s `whence` narrowed to `uint8_t`, so `256` arrived as `0`. The type
  checker refused the calls rather than letting gcc fail on generated code. Their wrappers are now
  lowered onto `__pd_file_handles`, the `long long` table `file_write` and `file_close` already
  use. `file_seek` takes `whence` 0/1/2 and returns the new absolute position or `-1`, refusing any
  other `whence` rather than treating it as a seek; `file_flush` returns 1 or 0, its siblings'
  convention. Both are exercised by `tests/stdlib/stdlib_builtins_file.pd`.)*
- *(CLOSED 2026-08-23 — **dead C wrappers**. `__pd_file_open_ex`, `__pd_file_close_ex`,
  `__pd_file_read_ex` and `__pd_file_write_ex` were still written into the prelude of every
  generated program although no builtin named them. They are deleted, and with them the
  `FileHandle` typedef, the `FileMode` enum and the six `pd_file_*` externs that only they used.)*

**N14's effect classification is unenforced**, because effects gate nothing
([A4.1](#a41-functions)).

Since 2026-08-21 there is one source of truth: `src/builtins.rs`. The type
checker derives its signature table from it (`src/typeck/mod.rs:1128-1128`) and so does the borrow
checker, which is what stopped the two from drifting apart. Codegen maps names to C symbols
(`src/codegen/mod.rs:5436-5436`, corrected from line 1813–1851 of the pre-cleanup revision) and emits their C bodies inline into
every output file (`src/codegen/mod.rs:1498-1498`, corrected from line 251–575 of the pre-cleanup revision).

*(v0.2 described this as "two tables that must agree". That was true before `src/builtins.rs`
became the SSOT; it is no longer the mechanism.)*

**Core**: `print(String)`, `print_int(i64)`, `panic(String)`

**String / char**: `string_len(String)->i64`, `string_concat(String,String)->String`,
`string_eq(String,String)->bool`, `string_char_at(String,i64)->i64`,
`string_substring(String,i64,i64)->String`, `string_from_char(i64)->String`,
`string_to_int(String)->i64`, `int_to_string(i64)->String`,
`char_is_digit(i64)->bool`, `char_is_alpha(i64)->bool`, `char_is_whitespace(i64)->bool`

**File I/O (handle = i64)**: `file_open(String)->i64`, `file_read_all(i64)->String`,
`file_read_line(i64)->String`, `file_write(i64,String)->bool`, `file_close(i64)->bool`,
`file_exists(String)->bool`, `file_flush(i64)->i64` (1 ok, 0 fail),
`file_seek(i64,i64,i64)->i64` (whence 0=start, 1=current, 2=end; new position, or -1)

**Paths and directories**: `path_exists`, `path_is_file`, `path_is_dir`, `create_dir`,
`create_dir_all`, `remove_file`, `remove_dir`, `remove_dir_all`

**Whole-file helpers**: `read_file_to_string(String)->String`,
`write_string_to_file(String,String)->i64`

`String` also supports `+` for concatenation.

*(An "Extended handle API" section stood here listing `file_open_ex`, `file_close_ex`,
`file_read_ex` and `file_write_ex`. Those names left `src/builtins.rs` on 2026-08-23 and no
Palladium program can name them; the section is deleted rather than marked, because a builtin
listing is a list of what a program may call.)*

> The path and directory builtins are thin wrappers over `extern` symbols supplied at link time by
> `runtime/palladium_runtime.c`. Before that file existed, every one of these — and in fact every
> Palladium program — failed to link.

**The standard library above them is unimplemented, and unshipped.** Measured at `abeb665`:

- **0 of 21** `.pd` files under `stdlib/` compile. Each is rejected at lex or parse time;
  `pdc compile stdlib/std/option.pd` fails with `Expected 'fn' for method, but found 'pub'`.
- It is not merely unreachable by default. The resolver is live and honours `$PALLADIUM_PATH`
  (`src/resolver/mod.rs:51-52`), but pointing it at the tree does not help:
  `PALLADIUM_PATH=…/stdlib/std` with `import option;` gives
  `error: Unexpected token: expected 'fn' for method, found 'pub'`.
- It is not packaged. `grep -rn stdlib .github/` returns **0 hits** (exit 1), and neither Homebrew
  formula installs it — `pdc.rb` installs `share/palladium/runtime`, `pdc-preview.rb` installs
  `lib/palladium/runtime`, and neither names `stdlib`. *(Formula paths are the stdlib unit's
  measurement of the tap; the `.github` grep is this unit's.)*
- `scripts/conformance.sh:270` defaults its scope to `tests` and `examples`, so `stdlib/` has never had a
  green row and its breakage was invisible.

A consequence worth stating plainly: because nothing under `stdlib/` has ever compiled, no defect
in the compiler can have "silently miscompiled the standard library". See
[A6.6](#a66-tail-expressions), where exactly that claim is retracted.

## A9. Memory model

**partial.** Ownership and borrowing are *checked* (`src/ownership/borrow_checker.rs`, 2268 lines)
but not *represented* in the type system: the typechecker treats `&T` as `T`
(`src/typeck/mod.rs:714-718`).

What the borrow checker actually enforces is a move/initialization discipline plus
conflicting-borrow detection. **A previous version of this annex asserted a defect here that does
not exist; it is retracted and re-measured in [A9.4](#a94-defect-d6-retracted).**

*(v0.2 said the checker "is currently stricter than the language needs in at least two measured
cases (`examples/practical/simple_sort.pd`, `tests/misc/test_vec_i64.pd` both fail with
"Conflicting borrows"). Re-measured at `abeb665`: `test_vec_i64.pd` now **compiles**, and
`simple_sort.pd` fails with "Unsupported type in reference parameter", not a borrow error. The
v0.2 sentence is retracted; the surviving borrow-checker defect is
[A9.3](#a93-mut-of-an-immutable-local-is-refused-was-accepted).)*

**[N9](#n9-references-and-lifetimes) is unimplemented in full.** `ref` is not a keyword; the
implemented spelling is Rust's `&`/`&mut` **with** `'a` parameter lists — the exact annotation
burden the definition removes. `fn f<'a>(x: &'a String) -> &'a String { return x; }` compiles.
`Function.lifetime_params` is parsed (`src/parser/mod.rs:1265`) and read nowhere outside test and
LSP fixtures. There is no region inference: `grep -rn 'region\|Region' src/ --include='*.rs'`
returns nothing.

No garbage collector. Strings are allocated from a 64 KiB static arena with a malloc fallback and
are freed at exit (`src/codegen/mod.rs:1463-1467`, corrected from line 210–245 of the pre-cleanup revision).

### A9.1 `String` is a copyable handle (decision, 2026-08-21)

`String` lowers to `const char*`, is allocated from the arena, and is **never freed
individually** — `grep -c '__pd_free\|pd_free_string' src/codegen/mod.rs` returns 0; the only
release is `__pd_cleanup_strings` registered via `atexit`. There are no destructors and no drop
glue.

Treating `String` as a move-only type therefore tracks an ownership that does not exist at
runtime, and — decisively — **cannot be worked around in the surface language**: there is no
`clone`, and `&T` is not a distinct type to the checker ([A5](#a5-types)), so with move semantics
there is no syntax at all that reads a `String` twice out of an array slot or a struct field.

`String` is therefore a Copy type in the implementation. Passing it copies a pointer; nothing is
duplicated and nothing is invalidated. Struct types (`Type::Custom`) remain move-only.

> **Tension.** [N12](#n12-memory-model) defines `String` as an owned, heap-allocated value with
> move semantics and a destructor. The implementation contradicts that, and the contradiction is
> not a bug to be papered over: restoring the definition requires drop glue, per-value
> deallocation, and a real reference type in the checker, none of which exist. Until they do, this
> annex records the deviation rather than the specification adopting it. This is the one place in
> the document where an implementation decision was previously allowed to rewrite the definition;
> it is now recorded as a divergence instead.

### A9.2 Array parameters

Every array parameter — `[T; N]`, `&[T; N]` and `&mut [T; N]` alike — is passed as a pointer
into the **caller's** array, because that is what C does to an array parameter. Nothing is
copied, at any of the three spellings, so a write through any of them is visible to the caller.

Whether `[T; N]` parameters *should* copy or alias is **not decided**: §9 defines the memory
model without mentioning array parameters, and §5 records that the typechecker cannot tell
`&T` from `T`. Until that decision is made, code generation refuses the writes it cannot
justify rather than picking one silently:

| spelling | may write through it | why |
|---|---|---|
| `&mut [T; N]` | ✅ | the declaration says so |
| `mut xs: [T; N]` | ✅ | the bootstrap subset's spelling for a mutable array parameter (bootstrap-subset.md §4) |
| `&[T; N]` | ❌ compile error | a shared reference does not permit mutation; the C declarator also const-qualifies the element slot |
| `[T; N]` | ❌ compile error | the write would reach the caller's array, which is the undecided semantics above |

The rule is enforced on **calls** as well as assignments: a function may not pass an array it
only holds shared, or by value, to a parameter that may write to it. Without that, the
permission could be laundered one hop — `fn f(xs: &[i64; 3]) { mutate(xs); }` — and the write
would happen under the callee's `&mut` binding, where it looks legitimate.

**Supported element types** for an array parameter are exactly `i32`, `i64`, `u32`, `u64`,
`bool`, `String` and a struct/enum name. Anything else is a compile error naming the type
("Unsupported array element type in function parameter"), not invalid C: in particular a
**nested array parameter** (`[[T; M]; N]`) is rejected rather than emitted, and a function type
never reaches here because §5 records that the parser refuses it ("expected type, found `fn`").
Nested arrays do not work as locals either — see the `[[T; M]; N]` row in §5; that is a
declarator defect in `type_to_c`, tracked separately, and it fails before any rule here
applies.

A `mut` parameter must be given something with storage. `bump(1)`, `retitle(make())` and
`bump(a + 1)` are refused: a `mut` parameter receives a pointer to the caller's storage, and an
rvalue has none — codegen emitted `bump(&1)` and gcc rejected the compiler's own output. The
alternative, materialising a temporary, would require this specification to say what a write
nobody can observe means; it does not, so the case is refused rather than invented. An argument
that *is* storage but that the borrow checker cannot model as a place, such as `xs[i]` with a
non-literal index, is checked against the mutability of the name it is rooted in.

Taking `&mut x` of a binding that was not declared `mut` is a borrow-check error, whatever `x`
is: array, scalar or `String`. The same check applies to passing a binding to a `mut x: T`
parameter, which writes through its pointer identically — codegen emits *every* `mut`
parameter as a pointer to the caller's storage, so `fn bump(mut x: i64)` mutates its caller's
variable exactly as `&mut i64` does. Both were previously unchecked:
`let v = [1, 2, 3]; set(&mut v);` compiled and modified an immutable binding, and
`fn bump(mut x: i64) { x = 42; }` called with an immutable `let n = 1;` printed 42.

The bindings this covers are every binder the grammar has — parameters, `let`, the `for`
variable, and match patterns. `for` and match bindings are immutable (there is no `mut` form of
either), and a name that reaches the check without having been registered by any binder is
**refused**, not permitted: an invariant with a permissive default stops being an invariant the
first time a binder is added and forgotten, which is precisely how `for` variables and match
bindings slipped through.

#### What the rule actually covers

This is a *bounded* enforcement, not a reference-safety model, and the boundary is worth
stating exactly, because a guard that reads as protection while having quiet gaps is worse than
no guard.

Enforced, in code generation, for a call whose callee is a plain `fn` this compilation knows:

- the argument is a name bound to an array in the current function — a local, or a parameter
  whose declared form is one of the four in the table above.

Refused, rather than assumed safe, because the capability cannot be established:

- the callee's parameter list is unknown to this pass (any callee not in the function table),
  and an array is being passed;
- the array argument is not a plain name this pass tracks — a struct field, an **element of an
  array** (`grid[0]`), a call result — and the parameter may write to it. An element is refused
  even when its array is a local the caller owns: letting it inherit the array's capability
  would be a more permissive rule than this one, and the rule stated here is the contract.

Not covered by this rule at all, and not claimed to be:

- **aliasing** between two array arguments beyond what the borrow checker already rejects;
- **references to anything other than arrays**: `&T` and `&mut T` are the same type to the
  type checker (§5), so a scalar reference carries no capability information here;
- any guarantee that survives a construct the front end has not implemented.

The reason the rule lives in code generation, and is shaped as a refusal, is that there is no
reference type in the type checker to carry the permission (§5). A complete model needs one —
that is M4's work, not this rule's. Until then this rule buys exactly one property: an array
write that reaches the caller can only come from a spelling that declared it.

#### The stale account that used to sit here

An orphan `## 10. Execution model` section stood between A9.2 and A9.3 and gave a
DIFFERENT account of the same current behaviour, measured at `abeb665`: that
`&mut [i64; 3]` "does not compile: Unsupported type in reference parameter", that a bare
`[i64; 3]` parameter mutates its caller "with **no diagnostic**", and that therefore the
implementation was "wrong in both directions at once". Every one of those three was true
before D9 and is false now; A9.2's table above is the measured behaviour. Re-measured:
`&mut [i64; 3]` compiles and the write is caller-visible; a write through a bare `[T; N]`
parameter is a code-generation error naming the undecided semantics; `mut a: [T; N]`
compiles and writes through.

It is deleted rather than corrected because nothing in it was both true and unique: the
open normative question and its Option A / Option B consequences are
[N12.1](#n121-array-parameters-open-decision), which is where they belong and where they
are stated at more length, and the interim rule it described is the table above. It was
also a `## 10.` heading inside `## A9`, duplicating [A10](#a10-execution-model) — which is
how a whole section came to be stale without any reader noticing it was there.


### A9.3 `&mut` of an immutable local is refused (was: accepted)

[N12](#n12-memory-model) requires that `&mut` be takeable only of a `mut` binding, and the
implementation now enforces it for every referent kind. The check is
`check_mutable_borrow_allowed` (`src/ownership/borrow_checker.rs:391-397`), which reads the
`mutable_bindings` map described in [A9.2](#a92-array-parameters); a name no binder
registered is refused rather than permitted.

```
struct S { x: i64 }
fn bump(s: &mut S) { s.x = 77; }
fn main() { let v: S = S { x: 1 }; bump(&mut v); print_int(v.x); }
```

is refused with "cannot borrow `v` as mutable: it is not declared mutable". With
`let mut v` it compiles, links and prints `77`.

*Historical.* This section asserted the opposite — that the program above "compiles, links,
and prints `77` — an immutable local mutated, with no diagnostic", measured at `abeb665` —
and that the defect reproduced for struct referents while not reproducing for arrays. Both
halves are obsolete: the mutability check landed with the array-parameter work above, and
re-measured on this tree the struct case is refused and so is the array case.

What is still true from the old scope note, and is a *different* defect: `&mut i64` is not
a working spelling. `fn bump(s: &mut i64) { *s = 77; }` reaches gcc, which rejects the
compiler's own output with "indirection requires pointer operand ('long long' invalid)".
That is a code-generation gap in scalar references, not a mutability-checking one.

### A9.4 Defect D6, retracted

A previous version of this annex, and of
[`feature-index.toml`](../reference/features/feature-index.toml), stated that a call argument is
borrowed as `Lifetime::Named("fn")` and released against `Lifetime::Scope(n)`, so the borrow is
never released and a value cannot be passed twice. That claim cited a line of
`src/ownership/borrow_checker.rs` whose content is at
`src/ownership/borrow_checker.rs:77` today. The old number is deliberately not repeated here:
a bare `path:line` naming a revision this tree no longer has is unpinnable, and an unpinnable
citation cannot be told from one that has silently drifted.

**The claim is false and the citation was wrong.** `src/ownership/borrow_checker.rs:582` is
`ReturnOwnership::Borrowed(Lifetime::Named("fn".to_string()))` — the ownership classification for a
function's **borrowed return value**, which has nothing to do with argument lifetimes. The citation
had a green fingerprint the whole time, which is exactly the gate's limit: a pin proves a line has
not moved, never that it supports the claim.

Re-measured from scratch at `abeb665`:

| Program | Result |
|---|---|
| `t(s); t(s);` — same `String` passed to two separate calls | accepted, prints `5 5` |
| `take2(s, s)` — same `String` twice in one call | accepted, prints `10` |
| `s1(v); s1(v);` — same array to two separate calls | accepted, prints `1 1` |
| `bump(&mut p); bump(&mut p);` — successive mutable borrows | accepted, prints `2` |
| `print(p.name); f(p.name); p.n` — field to a builtin, then reused | accepted, prints `abc 3 1` |

None of D6's symptoms reproduce. The call path creates a per-call lifetime and ends its borrows
when the call finishes: `src/ownership/borrow_checker.rs:954` (`let call_lifetime =
self.context.new_lifetime();`) and `src/ownership/borrow_checker.rs:960` (`self.context.end_borrows(&call_lifetime);`), with the
contract stated at `src/ownership/borrow_checker.rs:78-80` — "the caller-side borrow always lasts exactly for the call
expression".

D6 was **fixed in commit `191f8c1`** ("fix(compiler): five defects that made the language
unusable", 2026-08-21), twelve commits before `abeb665`. Its message says so directly: *"D6 call
arguments were borrowed forever. Argument borrows were tagged `Lifetime::Named("fn")` while release
only removed `Lifetime::Scope(n)`, so a value could never be passed to two functions. Borrows now
end with the call."* The description was accurate about the original defect and was carried forward
into documentation written after the fix landed.

Two rejections do still occur, and both are correct rather than defects:

- `take2(p, p)` where `p` is a **struct** — `Use of moved value: p`. Struct parameters are moves
  (`src/ownership/borrow_checker.rs:63-64`), so this is move semantics working.
- `sum2(v, v)` with two `mut [i64; 3]` parameters — `Conflicting borrows`. A `mut` array parameter
  is a mutable borrow (`src/ownership/borrow_checker.rs:546-548`), so passing the same array as two
  simultaneous mutable borrows is refused. **This is expected under the current aliasing
  convention, not unconditionally correct**: it follows from Option B's reading of
  [N12.1](#n121-array-parameters-open-decision), which is still open. Under Option A a `[T; N]`
  parameter would be a value, `sum2(v, v)` would pass two independent copies, and refusing it
  would be a bug. The struct rejection above needs no such qualification — moves are settled.

> **Action outside this repository's documentation:** the project's `CLAUDE.md` lists D6 under
> "남은 결함 (열림)" — remaining open defects. That is stale by twelve commits and should be moved
> to the fixed list. This unit did not edit `CLAUDE.md`.

## A10. Execution model

**implemented.** Execution starts at `fn main`. Arguments are evaluated left to right. The driver
requires a `main` function; a library module without one cannot be compiled standalone — which is
why `scripts/conformance.sh` reports `SKIP_NO_MAIN` for two files rather than failing them.

## A11. Conformance

`scripts/conformance.sh` compiles, links, and runs every `.pd` under `tests/` and `examples/`
against `tests/conformance-manifest.txt`, a **closed inventory** declaring what each fixture is
expected to do. Current status, re-measured on the tree integrating `feat/m2-patterns`
(2026-08-26):

**verified 76 · untranscribed 0 · vacuous 7 · xfail 1 · reject 57 · skip 2 · failures 0**, over 143
fixtures. (Issue #41 — the pattern forms, exhaustiveness and the trap, plus tuples as values —
added eight `run` fixtures and TWENTY `reject`s, nine of them from the review round that followed:
an `i64::MIN` bound that inverted its own comparison, a payload the type checker never looked
inside, a nested tuple literal whose struct was defined after the one that used it, and six shapes
nobody had pinned. The ratio is the point: most of what a pattern
feature adds to a language is a set of shapes it must refuse, and each of those is a program
somebody would otherwise have written and had accepted. Before it, review round 2 added one `run` fixture — a macro expanded in a value position — and TWO
more `reject`s: a generic method reached through its path form, and a generic enum's constructor.
Both were accepted by the front end and unbuildable by the C backend, so both became refusals.
Round 1 before it grew `reject` by SEVEN, which is the shape a review round leaves: external
review of the N5 work found two miscompiles and three places where the front end approved C the
backend could not build, and the repairs for the second kind are REFUSALS — each one a program
that should never have been accepted, each pinned by a `reject` fixture. Before that round the
figure was 64 verified, reject 21, over 95, and `reject` had FALLEN while the corpus grew — the
only revision of this figure that ever did: `feat/m2-expressions` added eleven `run` fixtures, one
per N5 row it closed, and transitioned `tests/reject/loop_keyword.pd` from `reject` to `run`, that
fixture having asserted there was no `loop` keyword until N5-07 gave the language one. The figure
before those — 52 verified, reject 22, over 84 — was taken before that branch. The one before it — 51 verified, reject 22, over 83 — was taken
before
`feat/m2-witness-json` added `tests/witness/json_parser.pd`, the second witness program thesis
condition 4 names; it is one `run` row, so only `verified` and the corpus size move. The one
before that — 48 verified, reject 16, over 74 — was taken before
`fix/m2-lexical` landed the N2-03/04/08/09/10/11 lexical rows: three `run` fixtures
(`01_lexical_escapes`, `02_types_chars`, `02_types_floats`) and five `reject` fixtures
(`unknown_attribute`, `attribute_with_args`, `attribute_inner`, `unknown_escape`,
`unterminated_block_comment`), which is 51 verified, reject 21, over 82; the single `reject`
row between that and 83 is `tests/reject/zero_length_array_self_reference.pd` (N4-23). The one before that — 48 verified, reject 15, over 73 — was taken
before
`fix/m2-async-producer` added `tests/reject/async_producer.pd`, the N7-18 repro, which is a
`reject` and moves both counts by one. The one before that — 46 verified, reject 14, over 70 — was
taken before `fix/d3b-tail-if` landed 3 fixtures and closed D3b, which moved its defect fixture
into `verified`. The figure before
that — 43 verified, reject 0, over 53 — was taken before 17 rows landed, 14 of them `reject`. A11 is the authority a release plan
reads, so a stale number here is release governance and not a documentation nit; that sentence is
this file's own, and it is why the figure is re-measured rather than left.)

> *(This paragraph previously read "verified 33 · … · xfail 2 · skip 2 · failures 0, over 44
> fixtures" and, below, listed "the three failures". Both were true before M1 and false when M1
> shipped: the corpus gained the nine `tests/stdlib/` drivers and `tests/regression/` rows, D9 made
> `examples/practical/simple_sort.pd` run, `tests/misc/test1.pd` was transitioned, and the run has
> `failures=0`. The annex is the authority a release plan reads, so stale numbers here are release
> governance and not a documentation nit.)*

`scripts/check-docs.sh` does the same for documentation snippets, and `scripts/selfhost.sh` checks
the self-hosting fixed point.

Each fixture declares a class:

- **run** — must compile, link, run, *and* have its stdout diffed byte-for-byte against a sibling
  `.expected` transcript. There is no exit-code-only spelling: a missing C `return` is undefined
  behaviour, so a tail-return miscompile (defect D3) prints garbage and still exits 0, and an
  exit-code verdict cannot see a wrong answer.
- **untranscribed** — ran, but carries no transcript. The reviewed allowance for a fixture that
  genuinely cannot have one; it needs an owner and a `why:` reason and is reported as a debt on
  every run, so "no transcript" is a written decision rather than a default. Currently zero. (The
  owner field is an editable label; the authorisation boundary is review of the manifest, not the
  runner, which cannot distinguish an honest reclassification from an evasive one.)
- **vacuous** — runs, but only prints that its feature is unimplemented. Its note must name the
  feature it fails to cover. ⚠️ **Seven** files are in this state: `02_types_enums`,
  `07_traits_basic`, `08_generics_basic`, `09_effects_system`, `10_async_await`,
  `11_unsafe_blocks` and `12_modules_imports`. A green conformance run is **not** evidence that
  enums, traits, generics, effects, async, unsafe or modules work. They do not (§4.4, §5).
- **xfail** — a known failure pinned to a stage *and* a diagnostic fingerprint. Failing at a
  different stage, or with a different message, fails the gate, so a fresh bug cannot hide behind
  an old excuse. A listed program that starts passing is `XPASS` and fails the gate.
- **reject** — a negative test: the compiler *must* refuse it with the declared diagnostic. This is
  real coverage, and it is how "the compiler rejects `.await`" gets tested instead of a program
  that prints prose about async being unimplemented. **CLOSED**: `reject=14` on the integrated
  tree. This paragraph carried a universally-quantified absence — "No fixture uses this class" —
  that was false the moment the rows landed, and an absence claim is the kind that stays wrong
  quietly, because nothing about a passing run contradicts it. The refusals a second
  implementation must reproduce are now in the corpus rather than only in Rust integration tests.
- **skip** — a declared non-program, and it must PROVE that: the compiler has to refuse it at a
  declared stage with a declared diagnostic, exactly like an `xfail`. This replaced an `fn main`
  regex, which `fn /* c */ main()`, `fn // c` + newline + `main()`, and plain `fn` + newline +
  `main()` all evaded while compiling and running fine — a real program could be declared `skip`
  and never gated.

Because the inventory is closed, a fixture that is deleted, renamed, or added without a declaration
fails the gate rather than silently shrinking or growing it. The gate's own ability to fail is
tested by `make test-conformance-runner` (133 cases).

**There are no failures.** The one remaining `xfail` is
`tests/projects/hello_pdm/tests/test_math.pd` ("Undefined function: add"), which needs cross-file
module imports. *(A previous version of this paragraph listed three failures. Two of them are gone:
`examples/practical/simple_sort.pd` runs since D9 was fixed, and `tests/misc/test1.pd` was
transitioned in the same change.)*

**Seven files in `tests/` named after a feature do not exercise it.** `02_types_enums.pd`,
`07_traits_basic.pd`, `08_generics_basic.pd`, `09_effects_system.pd`, `10_async_await.pd`,
`11_unsafe_blocks.pd` and `12_modules_imports.pd` each only `print` a message saying the feature is
unimplemented, and pass trivially. A green conformance run is therefore **not** evidence for enums,
traits, generics, effects, async, unsafe enforcement, or modules. *(v0.2 named two of these; a
later revision said six and omitted `02_types_enums.pd`, which the manifest has always declared
`vacuous`.)*

## A12. Relationship to the bootstrap subset

[`bootstrap-subset.md`](bootstrap-subset.md) defines PBS-1, the subset in which the self-hosting
compiler is written and which that compiler implements. PBS-1 is deliberately smaller than what
`pdc` accepts: it excludes every **partial** construct in this annex.

`make selfhost` reaches a byte-identical fixed point, which makes PBS-1 the one part of this
document where definition and implementation coincide.
