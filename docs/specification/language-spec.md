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
[`docs/reference/features/status.yaml`](../reference/features/status.yaml).

---

# Part I: Normative specification

## N1. Overview and design commitments

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

Primitives: `i32`, `i64`, `u32`, `u64`, `f32`, `f64`, `bool`, `char`, `String`, `()`.
`int` is an alias for `i64`.

Composites: arrays `[T; N]`, slices `[T]`, tuples `(A, B)`, references
(`ref T` / `ref mut T`, see [N9](#n9-references-and-lifetimes)), function types `fn(A) -> B`,
and named types with generic arguments `Name<A, B>`.

`Option<T>` and `Result<T, E>` are in the prelude. `?` propagates a `Result`'s error to the
caller, converting error types where a conversion exists.

Type inference is local and does not require annotations on `let` bindings or on most
expressions. Const generics (`struct Buffer<const N: usize>`) are generic parameters evaluated at
compile time.

## N5. Statements and expressions

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

Generics are monomorphised: a generic function or type is instantiated per concrete argument, so
abstraction costs nothing at runtime. Type parameters may carry bounds (`<T: Display>`) and
`where` clauses.

Traits define shared behaviour: method signatures with optional defaults, associated types, and
static dispatch through bounds. Trait methods take a `self` receiver.

These two are defined in detail by design documents rather than restated here:

- [`docs/design/trait_system_design.md`](../design/trait_system_design.md)
- [`docs/design/generics.md`](../design/generics.md)

Those documents carry a PROPOSAL banner reflecting their status against the current compiler. As
of this version they are read as **normative for the language and unimplemented in `pdc`** — the
same relationship every other section here has to Part II.

## N11. Modules

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

Ownership and borrowing are Rust's: each value has one owner, moves are the default, borrows are
checked at compile time, and there is no garbage collector.

Values with a destructor are dropped at end of scope. `String` is an owned, heap-allocated,
UTF-8 value with move semantics.

`unsafe { }` is where the compiler's guarantees are suspended and the programmer's take over. It
is restricted rather than unrestricted: it is auditable, isolated, and forbidden inside a
`#![total(strict)]` crate.

## N13. Execution model

Execution begins at `fn main`. Arguments are evaluated left to right. A compilation unit without
a `main` is a library.

## N14. Builtins and the standard library

Builtins are the operations the compiler knows intrinsically — printing, string and character
operations, file and path I/O. The authoritative list is generated from the compiler's own table:
[`docs/reference/builtins.md`](../reference/builtins.md).

Above them sits a standard library: core types and traits, collections (`Vec<T>`, `HashMap<K, V>`,
`String`, `Option<T>`), math, I/O, and process/environment access.

---

# Part II: Implementation status annex

What `pdc` does at commit `abeb665`, per Part I section. Each row is either a source location or a
command that was run.

| Normative section | Status | Where the detail is |
|---|---|---|
| [N1 Overview](#n1-overview-and-design-commitments) | partial | [A1](#a1-pipeline-and-backends) — C backend works; LLVM backend is skeletal |
| [N2 Lexical structure](#n2-lexical-structure) | partial | [A2](#a2-lexical-structure) — no floats, chars, hex, or attributes |
| [N3 Program structure and items](#n3-program-structure-and-items) | partial | [A3](#a3-program-structure), [A4](#a4-items) |
| [N4 Types](#n4-types) | partial | [A5](#a5-types) — no floats, slices, fn types; `Option`/`Result` not built in |
| [N5 Statements and expressions](#n5-statements-and-expressions) | partial | [A6](#a6-statements-and-expressions) — `if`/`match` are statements; no closures, `loop`, `else if`, compound assignment |
| [N6 Patterns](#n6-patterns) | partial | [A7](#a7-patterns) — three forms only; exhaustiveness for enums only |
| [N7 Effects and asynchrony](#n7-effects-and-asynchrony) | unimplemented | [A6.5](#a65-question-mark-async-and-await), and the divergence list in [`async-as-effect.md`](../reference/features/async-system/async-as-effect.md#where-the-implementation-currently-diverges) |
| [N8 Totality](#n8-totality) | unimplemented | [A2](#a2-lexical-structure) — attributes do not lex; no checker exists |
| [N9 References and lifetimes](#n9-references-and-lifetimes) | unimplemented | [A9](#a9-memory-model) — `ref` is not a keyword; no region inference |
| [N10 Traits and generics](#n10-traits-and-generics) | unimplemented | [A4.4](#a44-traits), [A5](#a5-types) |
| [N11 Modules](#n11-modules) | partial | [A3](#a3-program-structure) — `import` works; no `mod` item |
| [N12 Memory model](#n12-memory-model) | partial | [A9](#a9-memory-model) — checked but not typed; `String` is Copy |
| [N13 Execution model](#n13-execution-model) | implemented | [A10](#a10-execution-model) |
| [N14 Builtins and stdlib](#n14-builtins-and-the-standard-library) | partial | [A8](#a8-builtin-functions-36) — 36 builtins work; `stdlib/` does not parse |

## A1. Pipeline and backends

The pipeline (`src/driver/mod.rs:49-228`) is:

```
lex → parse → macro expand → resolve imports → typecheck → borrow check
    → effect analysis (informational only) → unsafe check → optimize → C codegen → gcc
```

The C backend is the real backend. An LLVM text backend exists
(`src/codegen/llvm_text_backend.rs`, 1442 lines) but is skeletal: `break` and `continue` emit
`br label %loop_end_placeholder` under a TODO (`:914`, `:921`), `match` is a TODO if/else chain
(`:933`), and enum construction, `?`, macro invocation and `await` are one unimplemented TODO
together (`:1379`). It also bails on ordinary code — "Unsupported iterator type in for loop"
(`:820`), "Unsupported binary operator" (`:1081`), "Complex function calls not yet supported"
(`:1222`). No conformance row exercises it.

Generated C is linked against `runtime/palladium_runtime.c`, which supplies 16 file/path symbols.
`pdc` resolves that runtime relative to its own install location — `pdc --print-runtime` shows
which copy it found, and `$PALLADIUM_RUNTIME` overrides it. (Until 2026-08-22 the path was
hardcoded relative to the working directory, so an installed compiler could not link anything.)

## A2. Lexical structure

The lexer is `logos`-based (`src/lexer/token.rs`).

**implemented**: decimal integers, strings, booleans, identifiers
(`src/lexer/token.rs:12-30`). The sign is part of the integer token, so `i-1` lexes as `i` then
`-1`.

**unimplemented**: float literals, char literals, hex (`0x`), binary (`0b`), octal, numeric
separators, raw strings, `\0` / `\xNN` / `\u{}` escapes, string interpolation. No lexer rule
produces any of them.

**unimplemented — attributes.** There is no `#` token at all. `#[total]` fails before parsing:

```
error: Unexpected character '#' at line 1, column 1
  = note: Palladium only allows ASCII letters, numbers, and common symbols
```

This is the blocker under [N8](#n8-totality): totality is not merely unimplemented, its syntax is
one level below the parser.

The 29 keywords the lexer recognizes (`src/lexer/token.rs:33-118`):

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

**Operators. implemented**: `+ - * / % = == != ! < > <= >= && ||`.
**unimplemented**: `+= -= *= /= %=` (no compound assignment), `| ^ ~ << >>` (no bitwise
operators), `..=`, `as` casts. `|` and `$` are lexed but never consumed by the parser; `as` is
consumed only for import aliases (`src/parser/mod.rs:259`).

Comments (`// line`, `/* block */`) are implemented.

## A3. Program structure

```ebnf
program = { import } { item } ;
```

**Imports must all precede items** — the parser drains imports first
(`src/parser/mod.rs:176`), so an `import` after a `fn` is a syntax error.

```ebnf
import = "import" path [ "as" identifier ] ";"
       | "import" path "::" "*" ";"
       | "import" path "::" "{" identifier { "," identifier } "}" ";" ;
```

Items (`src/parser/mod.rs:344-406`): `fn`, `struct`, `enum`, `trait`, `impl`, `type`, `macro`.
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
`Function.effects` is hardcoded `None` by the parser (`src/parser/mod.rs:565`, corrected from
v0.2's `:549`, which is where the `Function` literal opens). Effects are *inferred* afterwards
(`src/effects/mod.rs`) and only printed by the driver (`src/driver/mod.rs:151-157`, corrected
from `:139-145`); they gate nothing. `crate::effects::` is referenced from exactly one place in
the compiler, `src/driver/mod.rs:147`.

`async fn` is accepted and typechecked: `async fn g() -> i64 { return 1; }` fails with
"Type mismatch: expected Future<Int>, found Int", i.e. the return type is wrapped in a `Future`.
Under [N7](#n7-effects-and-asynchrony) neither the keyword nor the wrapper should exist.

### A4.2 Structs

**implemented** field types: `i64`/`i32`/`u32`/`u64`, `bool`, `String`, `[T; N]`, other structs,
enums.

**partial** — field types that parse and then fail in codegen (all three corrected from v0.2,
which was ~250 lines low):
- generic → "Generic types in structs not yet supported" (`src/codegen/mod.rs:1367`)
- reference → "Reference types in structs not yet supported" (`:1372`)
- tuple → "Tuple types in structs not yet supported" (`:1382`)

### A4.3 Enums

**implemented**: unit, tuple, and struct variants; construction and `match` both work.
**partial**: `pub` on an enum is parsed and then silently discarded — `EnumDef` has no visibility
field (`src/parser/mod.rs:379`, `src/ast/mod.rs:139-146`).

### A4.4 Traits

**unimplemented.** Traits parse (`src/parser/mod.rs:752-977`, corrected from `:736-960`) and then
emit nothing — codegen ignores `Item::Trait` (`src/codegen/mod.rs:1011`, corrected from
`:754-757`). Trait method bodies are never typechecked (`src/typeck/mod.rs:795-797`, corrected
from `:947`). Additionally, a trait method declared with a `self` receiver is a **parse error**,
because trait methods use a separate parameter loop that does not handle `self`
(`src/parser/mod.rs:875-911`, corrected from `:863-897`).

So `trait Display { fn fmt(&self) -> String; }` does not parse, and
[N10](#n10-traits-and-generics) has no implementation at all.

`tests/07_traits_basic.pd` PASSES conformance while only printing that traits are unimplemented.

### A4.5 Impl blocks

```ebnf
impl_block = "impl" [ generic_params ] [ type "for" ] type "{" { function } "}" ;
```

**implemented**: methods become mangled free functions `__pd_Type_method`
(`src/codegen/mod.rs:1025-1031`, corrected from `:1861`).
**unimplemented**: associated constants and associated types are rejected — an impl body may
contain only `fn` (`src/parser/mod.rs:1045-1051`, corrected from `:1030`).
**partial**: methods cannot be called with `.` syntax — see [A6.4](#a64-method-calls). Call them
as `Type::method(receiver, args)`.

### A4.6 Macros

**partial.** User macros (`macro name!(a, b) { … }`) parse into a raw token stream that is lossily
converted; unlisted tokens degrade into `AstToken::Ident` of a debug string
(`src/parser/mod.rs:1274`, corrected from `:1258`).

Four builtin macros exist (`src/macros/mod.rs:41-54`), each taking **exactly one** expression:

| Macro | Expands to | Status |
|---|---|---|
| `println!(e)` | `print(e); print("\n")` | implemented (one argument only — `println!()` and `println!(a, b)` fail) |
| `assert!(c)` | `if (!(c)) { panic("Assertion failed"); }` | implemented |
| `vec![e]` | `[e]` — a **1-element array**, not a growable vector | partial; misleading name |
| `dbg!(e)` | calls `print_debug` | unimplemented — `print_debug` is defined nowhere (`src/macros/mod.rs:167`, corrected from `:161`) |

Macro hygiene ([N3](#n3-program-structure-and-items)) is unimplemented:
`grep -rn hygien src/ --include='*.rs'` returns nothing.

## A5. Types

| Syntax | Status | Note |
|---|---|---|
| `i64`, `int` | implemented | `int` is an alias for `i64` (`src/parser/mod.rs:2064`, corrected from `:2038`) |
| `i32`, `u32`, `u64` | implemented | primitive table at `src/parser/mod.rs:2062-2070` (corrected from `:2037-2043`) |
| `bool`, `String` | implemented | |
| `()` | implemented | unit |
| `[T; N]` | implemented | `N` is an integer literal or an identifier |
| `&T`, `&mut T` | partial | parses, but the typechecker is a **no-op**: `Type::Reference` maps to its inner type — "For now, treat references as the inner type / TODO: Proper reference type handling" (`src/typeck/mod.rs:121-125`, corrected from `:2470-2486`). `&i64` and `i64` are indistinguishable to it. |
| `ref T`, `ref mut T` | unimplemented | `ref` is not a keyword; `fn f(x: ref String)` fails with "expected ')', found identifier 'String'" |
| `Name<A, B>` | partial | see below |
| `(A, B)` | partial | becomes `void*` in C (`src/codegen/mod.rs:1084-1085`, corrected from `:828`); no tuple expression exists, so no tuple is constructible |
| `f32`, `f64`, `char`, `str`, `u8`, `usize` | unimplemented | not in the primitive table |
| `fn(A) -> B` | unimplemented | function types are unparseable |
| `[T]` slices, `dyn T`, `impl T` | unimplemented | |
| `<T: Bound>`, `where` | unimplemented | `parse_generic_params` accepts bare names only; the `:` is a parse error |

**partial — generic argument bug**: inside `<…>`, any identifier whose characters are all
uppercase or `_` is reclassified as a *const generic argument* (`src/parser/mod.rs:2094`,
corrected from `:2054-2079`). So `Foo<T>` yields a const-generic `T`, not a type argument. Only
mixed-case names like `Vec<Item>` reach the type branch.

**partial — const generics**: they parse, and in codegen an `ArraySize::ConstParam` is emitted
into C verbatim as the parameter's *name* while an `ArraySize::Expr` becomes the literal `"0"`
(`src/codegen/mod.rs:1349-1351`). Neither is monomorphised. *(v0.2 said "array sizes from a const
parameter resolve to `0`" citing `:1360`; that is the expression case, not the const-parameter
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
They are ordinary user enums if you declare them. The only special-casing is that `?` typechecks
against a `Generic{name:"Result"}` shape (`src/typeck/mod.rs:2345`, corrected from `:2495`) — and
then generates C for a `struct Result` layout that codegen never emits (see
[A6.5](#a65-question-mark-async-and-await)).

## A6. Statements and expressions

### A6.1 Statements

`let`, assignment, `if`/`else`, `while`, `for … in`, `match`, `return`, `break`, `continue`,
`unsafe { }`, expression statements (`src/parser/mod.rs:1321-1331`).

- implemented: `let [mut] x [: T] = e;` — **the initializer is mandatory**
  (`src/parser/mod.rs:1405-1435`, corrected from `:1411`); the binding must be a plain identifier
  (no patterns).
- implemented: assignment targets — identifier, index, field, deref.
- **unimplemented: `else if`** — after `else` the parser requires `{`
  (`src/parser/mod.rs:1469`, corrected from `:1441`). Verified: `if a {} else if b {}` →
  `Expected '{' after else`. Use a nested `if` inside the `else`.
- **unimplemented: `loop`** — not a keyword. Use `while true`.
- unimplemented: compound assignment (`i += 1`) — verified: `Expected expression, but found '='`.
- unimplemented: bare nested blocks as statements; `try { }` blocks.
- implemented: `break` / `continue`, unlabeled, valueless.

`unsafe { }` parses and `src/unsafe_ops` runs (`src/driver/mod.rs:162-170`), but raw pointer types
and `unsafe fn` do not exist, so [N12](#n12-memory-model)'s restricted-unsafe is unimplemented.
`tests/11_unsafe_blocks.pd` PASSES while only printing that.

### A6.2 `for` loops

`for i in 0..n { }` — implemented.
**partial**: `for x in arr { }` where `arr` is a **function parameter** miscompiles: codegen emits
`sizeof(arr)/sizeof(arr[0])` (`src/codegen/mod.rs:1920-1922`, corrected from `:1553-1571`), which
is the pointer size after array-to-pointer decay, and it hardcodes the element type as
`long long`. Iterate parameters with an explicit index and `while`.

### A6.3 Expression forms

implemented: literals, identifiers, struct literals, array literals `[a,b,c]` and `[v; n]`,
indexing, field access, calls, enum construction, unary `- ! & *`, binary operators.

- **unimplemented: `if`, `match`, and blocks are statements, not expressions**
  (`src/parser/mod.rs:1325`, `:1330`). `let x = if c { 1 } else { 2 };` does not parse. This is a
  direct contradiction of [N5](#n5-statements-and-expressions).
- unimplemented: closures — no closure token path and no closure AST node.
- unimplemented: tuple expressions and `.0` indexing.
- unimplemented: `as` casts, string interpolation.
- partial: ranges outside a `for` header — codegen error "Range expressions can only be used in
  for loops" (`src/codegen/mod.rs:2498-2501`, corrected from `:2121`).
- partial: empty array literal `[]` — typeck cannot infer the element type
  (`src/typeck/mod.rs:2784`, corrected from `:1874`).

**partial — precedence bug**: `parse_multiplication` calls `parse_postfix` (not `parse_unary`) for
its right operand (`src/parser/mod.rs:1990`, corrected from `:1964`), so `a * -b` fails to parse.
Write `a * (0 - b)` or bind the negation to a variable. [N5](#n5-statements-and-expressions)
requires `a * -b`.

### A6.4 Method calls

**unimplemented.** `x.f()` parses as a call whose callee is a field access, and the typechecker
rejects exactly that: **"Indirect function calls not yet supported"**
(`src/typeck/mod.rs:1562`, corrected from `:1712`). Verified against `pdc`.

*(v0.2 also claimed "same guard at `src/codegen/mod.rs:1870`". `grep -n 'Indirect function calls'
src/codegen/mod.rs` returns nothing; there is no such guard in codegen. Claim withdrawn.)*

Call associated functions as `Type::method(receiver, …)`.

### A6.5 Question mark, async and await

**unimplemented — rejected, not lowered.** *(This section previously read "partial — silent
breakage", describing C that referenced an undefined `struct Result` layout and a `poll` member
nothing generated. Defect D5 was fixed on `main` in commit `439b241`; both are now refused at
typecheck. The silent-breakage description is retracted.)*

Both parse. Neither can be compiled, and since D5 the compiler says so instead of emitting C:

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
generic enum definitions (`src/codegen/mod.rs:841`, `:909`, `:929`) and generic enum construction
infers only the parameters a variant mentions, so `Result::Err(e)` yields `Result<(), E>`. One
syntactic trap is worth stating: a `match` arm that is a block must not be followed by a comma,
and propagation needs block arms because `return` is not an expression.

The refusal is raised by the type checker (`src/typeck/mod.rs:2356`, `:2363`) and again by code
generation (`src/codegen/mod.rs:2537`, `:2549`), which is callable on its own.

What they used to do:

- `?` emitted C referencing a `struct Result { int is_ok; union {…} data; }` layout that **no
  other part of codegen emits** — enums are generated with a `.tag` field and `__Enum__Variant`
  constants instead. gcc reported `variable has incomplete type 'struct Result'`.
- `.await` emitted `while (!f.poll(&f)) {}`. C has no member function calls, and the poll
  routine that *is* generated is the free function `<name>_poll`
  (`src/codegen/mod.rs:2590`), which that call never names. There is no async runtime.

Both lowerings are deleted rather than kept behind a flag: they encoded a representation a real
implementation must not reuse, and version control holds them.

The LLVM backend is a sharper case and is only safe by ordering. Its expression lowering has no
arm for either node — the catch-all at `src/codegen/llvm_text_backend.rs:1378` returns the
constant `0` for `Question`, `Await`, `EnumConstructor` and `MacroInvocation` alike, which
compiles and is wrong. The type checker refuses before a backend is chosen, which is what
`tests/d5_unimplemented_constructs.rs` pins.

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
class, and every function in `stdlib/` that ended in an expression was affected. It is fixed: the
parser now lowers a function body's tail expression to a return when a return type is declared. A
tail expression in a *nested* block still does not become a return.

Regardless of that fix, **write explicit `return` in every value-returning function.** The
bootstrap compiler does.

## A7. Patterns

**partial — three forms only.** `src/ast/mod.rs:313-323` defines exactly three pattern variants:

```ebnf
pattern = "_"
        | identifier
        | path "::" identifier [ "(" pattern { "," pattern } ")"
                               | "{" identifier ":" pattern { "," … } "}" ] ;
```

**unimplemented**: literal patterns (`1 =>`, `"s" =>`, `true =>`), range patterns, or-patterns
(`A | B`), guards (`if cond`), tuple/slice patterns, non-enum struct patterns, `ref`/`mut`
bindings, `@` bindings, field shorthand, `..` rest. [N6](#n6-patterns) requires all of them.

Exhaustiveness is checked only when the scrutinee is an enum (`src/typeck/mod.rs:1349-1381`,
corrected from `:2760-2790`). Codegen lowers `match` to an if/else-if chain
(`src/codegen/mod.rs:1954`, `:1975-1986`) with a wildcard arm becoming the final `else`; when no
arm matches and no wildcard arm was written, control simply falls through — there is no trap.

Consequence: **you cannot dispatch on an integer with `match`.** Use `if`/`else` chains.

## A8. Builtin functions (36)

**implemented.** Since 2026-08-21 there is one source of truth: `src/builtins.rs`. The type
checker derives its signature table from it (`src/typeck/mod.rs:365`) and so does the borrow
checker, which is what stopped the two from drifting apart. Codegen maps names to C symbols
(`src/codegen/mod.rs:2186-2226`, corrected from `:1813-1851`) and emits their C bodies inline into
every output file (`src/codegen/mod.rs:497-...`, corrected from `:251-575`).

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
`file_exists(String)->bool`, `file_flush(i64)->i64`, `file_seek(i64,i64,i64)->i64`

**Extended handle API**: `file_open_ex(String,i64)->i64`, `file_close_ex`, `file_read_ex`,
`file_write_ex`

**Paths and directories**: `path_exists`, `path_is_file`, `path_is_dir`, `create_dir`,
`create_dir_all`, `remove_file`, `remove_dir`, `remove_dir_all`

**Whole-file helpers**: `read_file_to_string(String)->String`,
`write_string_to_file(String,String)->i64`

`String` also supports `+` for concatenation.

> The `*_ex`, path, and directory builtins are thin wrappers over `extern` symbols supplied at
> link time by `runtime/palladium_runtime.c`. Before that file existed, every one of these — and
> in fact every Palladium program — failed to link.

**The standard library above them is unimplemented.** `stdlib/std/*.pd` exists but does not parse:
`pdc compile stdlib/std/option.pd` fails with "Expected 'fn' for method, but found 'pub'".
`scripts/conformance.sh` scans only `tests/` and `examples/`, so `stdlib/` has never had a green
row and its breakage was invisible.

## A9. Memory model

**partial.** Ownership and borrowing are *checked* (`src/ownership/borrow_checker.rs`, 1165 lines)
but not *represented* in the type system: the typechecker treats `&T` as `T`
(`src/typeck/mod.rs:121-125`).

What the borrow checker actually enforces is a move/initialization discipline plus
conflicting-borrow detection. Known defect: a call argument is borrowed as
`Lifetime::Named("fn")` (`src/ownership/borrow_checker.rs:236`) and released against
`Lifetime::Scope(n)` (`src/ownership/mod.rs:109`), so the borrow is never released — the same
value cannot be passed twice.

*(v0.2 said the checker "is currently stricter than the language needs in at least two measured
cases (`examples/practical/simple_sort.pd`, `tests/misc/test_vec_i64.pd` both fail with
"Conflicting borrows"). Re-measured at `abeb665`: `test_vec_i64.pd` now **compiles**, and
`simple_sort.pd` fails with "Unsupported type in reference parameter", not a borrow error. The
v0.2 sentence is retracted; the surviving borrow-checker defect is the lifetime-kind mismatch
above.)*

**[N9](#n9-references-and-lifetimes) is unimplemented in full.** `ref` is not a keyword; the
implemented spelling is Rust's `&`/`&mut` **with** `'a` parameter lists — the exact annotation
burden the definition removes. `fn f<'a>(x: &'a String) -> &'a String { return x; }` compiles.
`Function.lifetime_params` is parsed (`src/parser/mod.rs:553`) and read nowhere outside test and
LSP fixtures. There is no region inference: `grep -rn 'region\|Region' src/ --include='*.rs'`
returns nothing.

No garbage collector. Strings are allocated from a 64 KiB static arena with a malloc fallback and
are freed at exit (`src/codegen/mod.rs:427-476`, corrected from `:210-245`).

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

## A10. Execution model

**implemented.** Execution starts at `fn main`. Arguments are evaluated left to right. The driver
requires a `main` function; a library module without one cannot be compiled standalone — which is
why `scripts/conformance.sh` reports `SKIP_NO_MAIN` for two files rather than failing them.

## A11. Conformance

`scripts/conformance.sh` compiles, links, and runs every `.pd` under `tests/` and `examples/`
against `tests/conformance-manifest.txt`, a **closed inventory** declaring what each fixture is
expected to do. Current status (2026-08-22):

**verified 33 · untranscribed 0 · vacuous 7 · xfail 2 · skip 2 · failures 0**, over 44 fixtures.

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
  that prints prose about async being unimplemented.
- **skip** — a declared non-program, and it must PROVE that: the compiler has to refuse it at a
  declared stage with a declared diagnostic, exactly like an `xfail`. This replaced an `fn main`
  regex, which `fn /* c */ main()`, `fn // c` + newline + `main()`, and plain `fn` + newline +
  `main()` all evaded while compiling and running fine — a real program could be declared `skip`
  and never gated.

Because the inventory is closed, a fixture that is deleted, renamed, or added without a declaration
fails the gate rather than silently shrinking or growing it. The gate's own ability to fail is
tested by `make test-conformance-runner` (96 cases).

The three failures: `examples/practical/simple_sort.pd` ("Unsupported type in reference
parameter"), `tests/misc/test1.pd` ("Expected function, struct, enum, trait, type, impl, or macro
declaration"), `tests/projects/hello_pdm/tests/test_math.pd` ("Undefined function: add").

`scripts/check-docs.sh` does the same for documentation snippets, and `scripts/selfhost.sh` checks
the self-hosting fixed point.

**Six files in `tests/` named after a feature do not exercise it.** `07_traits_basic.pd`,
`08_generics_basic.pd`, `09_effects_system.pd`, `10_async_await.pd`, `11_unsafe_blocks.pd` and
`12_modules_imports.pd` each only `print` a message saying the feature is unimplemented, and pass
trivially. A green conformance run is therefore **not** evidence for traits, generics, effects,
async, unsafe enforcement, or modules. *(v0.2 named two of these six; the other four have the same
shape.)*

## A12. Relationship to the bootstrap subset

[`bootstrap-subset.md`](bootstrap-subset.md) defines PBS-1, the subset in which the self-hosting
compiler is written and which that compiler implements. PBS-1 is deliberately smaller than what
`pdc` accepts: it excludes every **partial** construct in this annex.

`make selfhost` reaches a byte-identical fixed point, which makes PBS-1 the one part of this
document where definition and implementation coincide.
