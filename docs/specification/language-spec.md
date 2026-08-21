# The Palladium Language Specification

**Version**: 0.2 (reality-based)
**Date**: 2026-08-22
**Supersedes**: `language_specification.md` v1.0.0-alpha (2025-01-19), which described an
intended language rather than the implemented one.

## 0. How to read this document

Every construct carries a status, and every status is backed by a location in the compiler
source or by a command that was run against `pdc`:

| Mark | Meaning |
|---|---|
| **✅** | Implemented end-to-end: parses, typechecks, generates C, runs. |
| **⚠️** | Parses, but breaks downstream — a compile error later, or (worse) wrong C. Each entry names the failure. |
| **❌** | Not implemented. Either unparseable or explicitly rejected. |

The previous specification asserted traits, generics with bounds, lifetimes, effects clauses,
`async`, floats, closures, slices, and `where` clauses as language features. None of those are
implemented; several are not even lexable. They are marked here rather than deleted, so the
gap between intent and implementation stays visible.

A claim in this document without a `file:line` or a reproducible command is a bug in this
document.

## 1. Overview

Palladium is a statement-oriented systems language that compiles to C. The pipeline
(`src/driver/mod.rs:38-201`) is:

```
lex → parse → resolve imports → macro expand → typecheck → borrow check
    → effect analysis (informational only) → optimize → C codegen → gcc
```

The C backend is the real backend. An LLVM text backend exists
(`src/codegen/llvm_text_backend.rs`) but is skeletal — break, continue, pattern matching, enum
construction, `?`, and `await` are all unimplemented there (`:914`, `:921`, `:933`, `:1379`).

Generated C is linked against `runtime/palladium_runtime.c`, which supplies 16 file/path
symbols. `pdc` resolves that runtime relative to its own install location — `pdc --print-runtime`
shows which copy it found, and `$PALLADIUM_RUNTIME` overrides it. (Until 2026-08-22 the path was
hardcoded relative to the working directory, so an installed compiler could not link anything.)

## 2. Lexical structure

Source is UTF-8. The lexer is `logos`-based (`src/lexer/token.rs`).

### 2.1 Literals ✅ / ❌

```ebnf
integer_literal = [ '-' ] digit { digit } ;        (* the sign is part of the token *)
string_literal  = '"' { char | escape } '"' ;
escape          = '\' ( 'n' | 't' | 'r' | '"' | '\' ) ;
boolean_literal = "true" | "false" ;
```

✅ decimal integers, strings, booleans.
❌ **float literals, char literals, hex (`0x`), binary (`0b`), octal, numeric separators, raw
strings, `\0` / `\xNN` / `\u{}` escapes, string interpolation.** The lexer has no rule that
produces any of them (`src/lexer/token.rs:12-30`). The v1.0 spec listed hex and binary
literals; they do not exist.

### 2.2 Identifiers and keywords ✅

```ebnf
identifier = ( letter | '_' ) { letter | digit | '_' } ;
```

The 29 keywords the lexer recognizes (`src/lexer/token.rs:33-118`):

```
fn let mut if else while return true false for in break continue
struct enum trait impl match import pub as Self self type const
unsafe async await macro
```

Note `import`, not `use`. **`loop`, `mod`, `use`, `where`, `dyn`, `move`, `static`, `ref`,
`crate`, `super`, `extern` are NOT keywords** — they lex as ordinary identifiers, so
`let loop = 1;` is legal and `loop { }` is a parse error at the `{`.

### 2.3 Operators ✅ / ❌

✅ `+ - * / % = == != ! < > <= >= && ||`
❌ `+= -= *= /= %=` (no compound assignment), `| ^ ~ << >>` (no bitwise operators — the v1.0
spec listed all of them), `..=`, `as` casts.

`|` and `$` are lexed but never consumed by the parser; `as` is consumed only for import
aliases (`src/parser/mod.rs:259`).

### 2.4 Comments ✅

`// line` and `/* block */`, treated as whitespace.

## 3. Program structure

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
❌ **There is no top-level `const`, `static`, `mod`, or `use` item.**

## 4. Items

### 4.1 Functions ✅

```ebnf
function = [ "pub" ] [ "async" ] "fn" identifier [ generic_params ]
           "(" [ params ] ")" [ "->" type ] block ;
param    = [ "mut" ] identifier ":" type | self_param ;
self_param = [ "&" ] [ "mut" ] "self" ;
```

✅ parameters, return types, `pub`, `self` receivers in `impl` blocks.
❌ default parameter values, pattern parameters, varargs, `where` clauses.
❌ **effect clauses (`!\[io\]`) do not exist in the surface syntax.** `Function.effects` is
hardcoded `None` by the parser (`src/parser/mod.rs:549`). Effects are *inferred* afterwards
(`src/effects/mod.rs`) and only printed by the driver (`src/driver/mod.rs:139-145`); they gate
nothing. The v1.0 spec's `fn read_file(p: String) -> String ![io]` is not parseable.

### 4.2 Structs ✅ with field restrictions

```ebnf
struct_def = [ "pub" ] "struct" identifier [ generic_params ] "{" fields "}" ;
```

Field types that work: `i64`/`i32`/`u32`/`u64`, `bool`, `String`, `[T; N]`, other structs, enums.

⚠️ Field types that parse and then fail in codegen:
- tuple → "Tuple types in structs not yet supported" (`src/codegen/mod.rs:1119`)
- generic → "Generic types in structs not yet supported" (`:1104`)
- reference → "Reference types in structs not yet supported" (`:1109`)

### 4.3 Enums ✅

```ebnf
enum_def = "enum" identifier [ generic_params ] "{" variants "}" ;
variant  = identifier [ "(" types ")" | "{" fields "}" ] ;
```

✅ unit, tuple, and struct variants; construction and `match` both work.
⚠️ `pub` on an enum is parsed and then silently discarded — `EnumDef` has no visibility field
(`src/parser/mod.rs:379`, `src/ast/mod.rs:139-146`).

### 4.4 Traits ❌

Traits parse (`src/parser/mod.rs:736-960`) and then **emit nothing** — codegen produces no C
for a trait (`src/codegen/mod.rs:754-757`); there is no vtable or dispatch mechanism anywhere.
Trait method bodies are never typechecked (`src/typeck/mod.rs:947`). Additionally, a trait
method declared with a `self` receiver is a **parse error**, because trait methods use a
separate parameter loop that does not handle `self` (`src/parser/mod.rs:863-897`).

The v1.0 spec's `trait Display { fn fmt(&self) -> String; }` does not parse.

### 4.5 Impl blocks ✅ (associated functions only)

```ebnf
impl_block = "impl" [ generic_params ] [ type "for" ] type "{" { function } "}" ;
```

✅ Methods become mangled free functions `__pd_Type_method` (`src/codegen/mod.rs:1861`).
❌ Associated constants and associated types are rejected (`src/parser/mod.rs:1030`).
⚠️ **Methods cannot be called with `.` syntax** — see §6.4. Call them as
`Type::method(receiver, args)`.

### 4.6 Macros ⚠️

User macros (`macro name!(a, b) { … }`) parse into a raw token stream that is lossily
converted; unlisted tokens degrade into `AstToken::Ident` of a debug string
(`src/parser/mod.rs:1258`).

Four builtin macros exist (`src/macros/mod.rs:41-54`), each taking **exactly one** expression:

| Macro | Expands to | Status |
|---|---|---|
| `println!(e)` | `print(e); print("\n")` | ✅ (one argument only — `println!()` and `println!(a, b)` fail) |
| `assert!(c)` | `if (!(c)) { panic("Assertion failed"); }` | ✅ |
| `vec![e]` | `[e]` — a **1-element array**, not a growable vector | ⚠️ misleading name |
| `dbg!(e)` | calls `print_debug` | ❌ **broken** — `print_debug` is defined nowhere (`src/macros/mod.rs:161`) |

## 5. Types

| Syntax | Status | Note |
|---|---|---|
| `i64`, `int` | ✅ | `int` is an alias for `i64` (`src/parser/mod.rs:2038`) |
| `i32`, `u32`, `u64` | ✅ | |
| `bool`, `String` | ✅ | |
| `()` | ✅ | unit |
| `[T; N]` | ✅ | `N` is an integer literal or an identifier |
| `&T`, `&mut T` | ⚠️ | parses, but the typechecker is a **no-op**: `&i64` and `i64` are indistinguishable to it (`src/typeck/mod.rs:2470-2486`). There is no reference type in the checker. |
| `Name<A, B>` | ⚠️ | see below |
| `(A, B)` | ⚠️ | becomes `void*` in C (`src/codegen/mod.rs:828`); no tuple expression exists, so no tuple is constructible |
| `f32`, `f64`, `char`, `str`, `u8`, `usize` | ❌ | not in the primitive table (`src/parser/mod.rs:2037-2043`) |
| `fn(A) -> B` | ❌ | function types are unparseable |
| `[T]` slices, `dyn T`, `impl T` | ❌ | |
| `<T: Bound>`, `where` | ❌ | `parse_generic_params` accepts bare names only; the `:` is a parse error |

⚠️ **Generic argument bug**: inside `<…>`, any identifier whose characters are all uppercase or
`_` is reclassified as a *const generic argument* (`src/parser/mod.rs:2054-2079`). So `Foo<T>`
yields a const-generic `T`, not a type argument. Only mixed-case names like `Vec<Item>` reach
the type branch.

⚠️ **Const generics** parse but are dropped: array sizes from a const parameter resolve to `0`
(`src/codegen/mod.rs:1360`).

### 5.1 Option and Result ❌ (as built-ins)

There is no built-in `Option` or `Result` — no prelude, no declaration, no lexer or parser
support. They are ordinary user enums if you declare them. The only special-casing is that `?`
typechecks against a `Generic{name:"Result"}` shape (`src/typeck/mod.rs:2495`) — and then
generates C for a `struct Result` layout that codegen never emits (see §6.5).

The v1.0 spec's prelude (`type Option<T> = enum { Some(T), None };`) does not exist.

## 6. Statements and expressions

### 6.1 Statements ✅

`let`, assignment, `if`/`else`, `while`, `for … in`, `match`, `return`, `break`, `continue`,
`unsafe { }`, expression statements.

- ✅ `let [mut] x [: T] = e;` — **the initializer is mandatory** (`src/parser/mod.rs:1411`);
  the binding must be a plain identifier (no patterns).
- ✅ assignment targets: identifier, index, field, deref.
- ❌ **`else if`** — after `else` the parser requires `{` (`src/parser/mod.rs:1441`). Verified:
  `if a {} else if b {}` → `Expected '{' after else`. Use a nested `if` inside the `else`.
- ❌ **`loop`** — not a keyword. Use `while true`.
- ❌ compound assignment (`i += 1`) — verified: `Expected expression, but found '='`.
- ❌ bare nested blocks as statements.
- ✅ `break` / `continue`, unlabeled, valueless.

### 6.2 `for` loops ✅ over ranges, ⚠️ over arrays

`for i in 0..n { }` ✅.
⚠️ `for x in arr { }` where `arr` is a **function parameter** miscompiles: codegen emits
`sizeof(arr)/sizeof(arr[0])` (`src/codegen/mod.rs:1553-1571`), which is the pointer size after
array-to-pointer decay, and it hardcodes the element type as `long long`. Iterate parameters
with an explicit index and `while`.

### 6.3 Expression forms

✅ literals, identifiers, struct literals, array literals `[a,b,c]` and `[v; n]`, indexing,
field access, calls, enum construction, unary `- ! & *`, binary operators.

❌ **`if`, `match`, and blocks are statements, not expressions** (`src/parser/mod.rs:1301`,
`:1306`). `let x = if c { 1 } else { 2 };` does not parse.
❌ closures — no closure token path and no closure AST node.
❌ tuple expressions and `.0` indexing.
❌ `as` casts, string interpolation.
⚠️ ranges outside a `for` header — codegen error (`src/codegen/mod.rs:2121`).
⚠️ empty array literal `[]` — typeck cannot infer the element type (`src/typeck/mod.rs:1874`).

⚠️ **Precedence bug**: `parse_multiplication` calls `parse_postfix` (not `parse_unary`) for its
right operand (`src/parser/mod.rs:1964`), so `a * -b` fails to parse. Write `a * (0 - b)` or
bind the negation to a variable.

### 6.4 Method calls ❌

`x.f()` parses as a call whose callee is a field access, and the typechecker rejects exactly
that: **"Indirect function calls not yet supported"** (`src/typeck/mod.rs:1712`; same guard at
`src/codegen/mod.rs:1870`). Verified against `pdc`.

Call associated functions as `Type::method(receiver, …)`.

### 6.5 `?` and `async`/`await` ⚠️ — silent breakage

- `?` generates C that references a `struct Result { int is_ok; union {…} data; }` layout
  which **no other part of codegen emits** — user enums are generated with a `.tag` field and
  `__Enum__Variant` constants instead (`src/codegen/mod.rs:2160-2201`, `:1644`). The result is
  C that does not compile.
- `.await` emits `while (!f.poll(&f)) {}`, calling a `poll` member that is never generated
  (`src/codegen/mod.rs:2208-2237`).

Neither is an error at any earlier stage. Both are excluded from the bootstrap subset.

### 6.6 Tail expressions

`fn add(a: i64, b: i64) -> i64 { a + b }` — a function body ending in an expression rather than
a `return`.

This is in the grammar (`grammar.ebnf`) and it previously **compiled cleanly and returned
garbage**: the generated C was `long long add(...) { (a + b); }` with no `return`, and
`add(2,3)` printed `6162934856`. No error, no warning, wrong answer — the project's most
dangerous defect class, and every function in `stdlib/` that ended in an expression was affected.
It is fixed: the parser now lowers a function body's tail expression to a return when a return
type is declared. A tail expression in a *nested* block still does not become a return.

Regardless of that fix, **write explicit `return` in every value-returning function.** The
bootstrap compiler does.

## 7. Patterns ✅ (three forms only)

`src/ast/mod.rs:313-323` defines exactly three pattern variants:

```ebnf
pattern = "_"
        | identifier
        | path "::" identifier [ "(" pattern { "," pattern } ")"
                               | "{" identifier ":" pattern { "," … } "}" ] ;
```

❌ literal patterns (`1 =>`, `"s" =>`, `true =>`), range patterns, or-patterns (`A | B`),
guards (`if cond`), tuple/slice patterns, non-enum struct patterns, `ref`/`mut` bindings,
`@` bindings, field shorthand, `..` rest.

Exhaustiveness is checked only when the scrutinee is an enum
(`src/typeck/mod.rs:2760-2790`). Codegen emits no default arm
(`src/codegen/mod.rs:1731`), so a non-exhaustive match on a non-enum falls through silently.

Consequence: **you cannot dispatch on an integer with `match`.** Use `if`/`else` chains.

## 8. Builtin functions (36)

Defined in two tables that must agree: `src/typeck/mod.rs:352-527` (signatures) and
`src/codegen/mod.rs:1813-1851` (C name mapping). Their C bodies are emitted inline into every
output file (`src/codegen/mod.rs:251-575`).

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
> link time by `runtime/palladium_runtime.c`. Before that file existed, every one of these —
> and in fact every Palladium program — failed to link.

## 9. Memory model

Ownership and borrowing are *checked* (`src/ownership/borrow_checker.rs`) but not *represented*
in the type system: the typechecker treats `&T` as `T` (`src/typeck/mod.rs:2470-2486`).

What the borrow checker actually enforces is a move/initialization discipline plus
conflicting-borrow detection. It is currently stricter than the language needs in at least two
measured cases (`examples/practical/simple_sort.pd`, `tests/misc/test_vec_i64.pd` both fail
with "Conflicting borrows" on code that is sound).

No garbage collector. Strings are allocated from a 64 KiB static arena with a malloc fallback
and are freed at exit (`src/codegen/mod.rs:210-245`).

### 9.1 `String` is a copyable handle (decision, 2026-08-21)

`String` lowers to `const char*`, is allocated from the arena, and is **never freed
individually** — `grep -c '__pd_free\|pd_free_string' src/codegen/mod.rs` returns 0; the only
release is `__pd_cleanup_strings` registered via `atexit`. There are no destructors and no drop
glue.

Treating `String` as a move-only type therefore tracks an ownership that does not exist at
runtime, and — decisively — **cannot be worked around in the surface language**: there is no
`clone`, and `&T` is not a distinct type to the checker (§5), so with move semantics there is
no syntax at all that reads a `String` twice out of an array slot or a struct field.

`String` is therefore a Copy type. Passing it copies a pointer; nothing is duplicated and
nothing is invalidated.

> **Tension.** This contradicts the aspiration that `String` is an owned, heap-allocated value
> with Rust-like move semantics. Restoring that aspiration requires drop glue, per-value
> deallocation, and a real reference type in the checker — none of which exist. Until they do,
> the specification describes the implementation rather than the intent. Struct types
> (`Type::Custom`) remain move-only.

## 10. Execution model

Execution starts at `fn main`. Arguments are evaluated left to right. The driver requires a
`main` function; a library module without one cannot be compiled standalone.

## 11. Conformance

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
- **skip** — declared non-program (no `fn main`).

Because the inventory is closed, a fixture that is deleted, renamed, or added without a declaration
fails the gate rather than silently shrinking or growing it. The gate's own ability to fail is
tested by `make test-conformance-runner` (87 cases).

## 12. Relationship to the bootstrap subset

[`bootstrap-subset.md`](bootstrap-subset.md) defines PBS-1, the subset in which the
self-hosting compiler is written and which that compiler implements. PBS-1 is deliberately
smaller than what `pdc` accepts: it excludes every ⚠️ construct in this document.
