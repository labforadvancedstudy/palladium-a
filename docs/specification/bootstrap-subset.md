# Palladium Bootstrap Subset (PBS-1)

**Status**: normative for self-hosting work
**Version**: 1 (2026-08-21)
**Evidence base**: measured against `pdc` at commit `f323cf1` + the fixes listed in §7.

## 1. What this document is

PBS-1 is the subset of Palladium in which **the self-hosting compiler is written**, and
simultaneously the subset **that compiler must accept**. Those two sets being the same set is
the whole point: a compiler written in a dialect richer than it implements can never compile
itself.

> This is exactly how the previous bootstrap attempt failed. `bootstrap/v2_full_compiler/` is
> written using if-expressions, `if let`, `matches!`, and `Option<T>` (see
> `bootstrap/v2_full_compiler/parser.pd:178`), none of which its own parser implements — and
> none of which `pdc` implements either. It cannot compile itself, and never could. The
> "100% bootstrap achieved" claims in `bootstrap/v3_incremental/BOOTSTRAP_ACHIEVED.md` and
> `README.md` refer to string-rewriting toys, not to a fixed point.

**Rule PBS-0 (closure rule)**: a construct may enter PBS-1 only when it is (a) accepted by
`pdc`, and (b) implemented in the bootstrap compiler. Adding a construct to the bootstrap
compiler's *source* without implementing it in the bootstrap compiler's *code* is forbidden.

## 2. Lexical

```ebnf
identifier   = (letter | '_') { letter | digit | '_' } ;
integer      = digit { digit } ;                 (* decimal only *)
string       = '"' { char | escape } '"' ;
escape       = '\' ( 'n' | 't' | 'r' | '"' | '\' ) ;
comment      = "//" { char - '\n' } | "/*" { char } "*/" ;
```

Not in PBS-1, because the lexer does not produce them (`src/lexer/token.rs:12-30`):
float literals, char literals, hex/binary/octal literals, numeric separators, raw strings,
`\0` / `\xNN` / `\u{}` escapes, string interpolation.

**Keywords used by PBS-1**: `fn let mut if else while for in return break continue struct
enum match true false`.
Recognized by the lexer but *outside* PBS-1: `trait impl import pub as Self self type const
unsafe async await macro`.
Not keywords at all (they lex as identifiers, so they are silently ordinary names): `loop`,
`mod`, `use`, `where`, `dyn`, `move`, `static`, `ref`.

**Operators**: `+ - * / % = == != ! < > <= >= && ||`.
Absent from the lexer, therefore absent from PBS-1: `+= -= *= /= %=` (no compound assignment),
`| ^ ~ << >>` (no bitwise ops), `..=`, `as` casts.

## 3. Types

| Type | Notes |
|---|---|
| `i64` (alias `int`) | the working integer type; use it for everything numeric |
| `i32`, `u32`, `u64` | parse and codegen, but PBS-1 code should use `i64` only |
| `bool` | |
| `String` | immutable, heap-ish, built by `+` or `string_concat` |
| `[T; N]` | fixed-size array, `N` an integer literal |
| `struct` | fields must be `i64`/`bool`/`String`/array; see restriction below |
| `enum` | unit, tuple, and struct variants |

**Excluded from PBS-1** (verified unsupported downstream):
- Tuples — `type_to_c` yields `void*` (`src/codegen/mod.rs:1397-1400`), and a tuple in a struct
  field is a hard error (`src/codegen/mod.rs:1695`). No tuple *expressions* exist at all, so no
  tuple is constructible.
- Generic types in struct fields — error at `src/codegen/mod.rs:1680`.
- Reference types in struct fields — error at `src/codegen/mod.rs:1685`.
- Returning an array from a function — error at `src/codegen/mod.rs:1902`.
- `f32`/`f64`, `char`, `str`, `u8`, `usize` — no such primitives (`src/parser/mod.rs:2071-2079`).
- Trait bounds (`<T: Display>`) — a parse error; `parse_generic_params` accepts bare names only.
- `Option<T>` / `Result<T,E>` as built-ins — they do not exist. Declaring your own does not
  enable `?`: nothing lowers the operator onto the representation enums are compiled to, so it
  is rejected outright (`src/typeck/mod.rs:2439`). It used to emit a C `struct Result` layout
  that no other part of codegen ever defines.

**Generics**: excluded from PBS-1. They monomorphize in limited cases, but generic-argument
parsing misclassifies any all-uppercase name as a *const* generic argument
(`src/parser/mod.rs:2054-2079`), so `Foo<T>` does not mean what it looks like.

## 3.1 Additional PBS-1 rules (measured, not stylistic)

These are not preferences. Each one exists because the alternative is broken or
unimplementable in a single-pass translator.

1. **Every `let` carries an explicit type.** `let mut i: i64 = 0;`
   This is a requirement of PBS-1 itself, not a workaround: the bootstrap compiler emits C, C
   declarations need a type, and requiring the annotation removes the entire type-inference
   subsystem from it. That is the single largest simplification in PBS-1. (The Rust compiler
   infers `let` types since D7 was fixed; the bootstrap compiler does not, and does not need to.)

2. **Always put spaces around binary `-`.** Write `i - 1`, never `i-1`.
   The lexer's integer rule is `-?[0-9]+` (`src/lexer/token.rs:26`), so the minus sign binds
   into the literal when it is adjacent to digits: `i-1` lexes as `i` followed by `-1`, two
   adjacent expressions, and misparses. With a space, `-` lexes as the operator.

3. **Struct and array parameters that are written are declared `mut`.**
   A `mut` parameter of struct type becomes `struct S*` in C, so mutations propagate to the
   caller — verified: `fn bump(mut s: S)` → `void bump(struct S* s)`. A non-`mut` struct or
   array parameter is classified as a *move* by the borrow checker
   (`src/ownership/borrow_checker.rs:531`) and can never be used again by the caller.

4. **Struct literals appear only as `let` initializers.** They translate to a C99 designated
   initializer, `S { a: 1 }` → `(struct S){ .a = 1 }`.

5. **Array initializers are `[0; N]` or `[""; N]` only**, and translate to `{0}`. PBS-1 code
   must write every slot before reading it.

6. **String concatenation uses `string_concat(a, b)`, not `+`.**
   `+` on strings would force the emitter to be type-directed (C has no `+` for `char*`).
   Using the builtin keeps emission type-free.

7. **A struct-typed local is a C value; a struct-typed parameter is a C pointer.**
   Consequences for the emitter, both mechanical: a struct local passed to a function needs
   `&`, and field access through a struct parameter uses `->` rather than `.`.

## 4. Statements

```ebnf
stmt = let_stmt | assign_stmt | if_stmt | while_stmt | for_stmt
     | match_stmt | return_stmt | break_stmt | continue_stmt | expr_stmt ;

let_stmt      = "let" [ "mut" ] identifier [ ":" type ] "=" expr ";" ;
assign_stmt   = place "=" expr ";" ;
place         = identifier | place "[" expr "]" | place "." identifier | "*" identifier ;
if_stmt       = "if" expr block [ "else" block ] ;
while_stmt    = "while" expr block ;
for_stmt      = "for" identifier "in" ( range | array_expr ) block ;
range         = expr ".." expr ;
match_stmt    = "match" expr "{" { match_arm } "}" ;
match_arm     = pattern "=>" ( block | expr ) [ "," ] ;
return_stmt   = "return" [ expr ] ";" ;
```

Hard constraints, each verified by running `pdc`:

- **`let` requires an initializer.** `let x: i64;` is a parse error (`src/parser/mod.rs:1411`).
- **No `else if`.** After `else` the parser demands `{` (`src/parser/mod.rs:1441`). Write
  nested `if` inside the `else` block. PBS-1 code must follow this; it is the single most
  common source of parse errors when porting Rust-shaped code.
- **No `loop`.** Use `while true { … }`.
- **No compound assignment.** Write `i = i + 1;`.
- **No bare nested block** as a statement.
- **`for` iterates a range or an array only.** Iterating an array *parameter* used to
  miscompile — codegen emitted `sizeof(arr)/sizeof(arr[0])`, which is wrong for a decayed
  pointer — and is now correct: the bound comes from the declared length (D4, fixed). The
  PBS-1 rule to iterate with an explicit `while` and an index is therefore no longer forced,
  though PBS-1 code that already does so needs no change.
- **`break`/`continue` are unlabeled** and carry no value.

## 5. Expressions

Supported: integer/string/bool literals, identifiers, `struct` literals, array literals
`[a, b, c]` and `[v; n]`, indexing `a[i]`, field access `p.x`, calls `f(a, b)`, enum
construction `E::V(...)`, unary `- ! & *`, and the binary operators of §2 with C-like
precedence.

**Excluded from PBS-1**:

| Construct | Why |
|---|---|
| `if` / `match` / block as an *expression* | parsed only as statements (`src/parser/mod.rs:1301`, `src/parser/mod.rs:1306`) |
| method call `x.f()` | typeck rejects: "Indirect function calls not yet supported" (`src/typeck/mod.rs:1795`). Call `Type::method(receiver, …)` instead. |
| `?` operator | rejected: "the `?` operator is not implemented" (`src/typeck/mod.rs:2439`). It used to emit C referencing an undefined `struct Result`. |
| `.await` / `async` | `.await` rejected: "`.await` is not implemented" (`src/typeck/mod.rs:2446`). It used to emit a `poll` member call that is never generated. |
| closures | no closure token path, no closure AST node |
| ranges outside `for` | codegen error (`src/codegen/mod.rs:2147`) |
| empty array literal `[]` | typeck error — element type uninferrable (`src/typeck/mod.rs:1957`) |
| tuple expressions, `.0` indexing | unparseable |
| `dbg!` | expands to `print_debug`, which is not defined anywhere (`src/macros/mod.rs:161`) |

**Tail expressions**: `fn f() -> i64 { a + b }` (no `return`). Historically this compiled to C
with the `return` missing, silently returning garbage. See §7 — once fixed, tail returns are
legal; until then **PBS-1 requires an explicit `return` in every value-returning function**,
and PBS-1 source keeps explicit `return` regardless, because it costs nothing and removes a
whole failure class.

## 6. Patterns

Exactly three forms exist (`src/ast/mod.rs:313-323`):

1. `_` — wildcard
2. `name` — binding
3. `Enum::Variant`, `Enum::Variant(a, b)`, `Enum::Variant { f: a }` — field shorthand is NOT
   allowed; `..` rest is NOT allowed.

No literal patterns, no ranges, no or-patterns, no guards, no tuple/slice patterns. A match on
an integer therefore cannot dispatch on values — **PBS-1 dispatches on integers with
`if`/`else` chains, and on enums with `match`.**

## 7. Compiler defects PBS-1 depends on being fixed

These are tracked because PBS-1 code cannot be written safely without them.

| # | Defect | Location | Status |
|---|---|---|---|
| D1 | `runtime/palladium_runtime.c` was referenced by the driver but absent from the repo, so nothing could ever link. It had never been committed: `.gitignore` carried a blanket `*.c` | `src/driver/mod.rs:286`, `.gitignore` | **fixed** — runtime written, `.gitignore` negated for `runtime/` |
| D2 | 11 builtins registered in typeck but not in the borrow checker, so `string_len`, `string_eq`, `string_char_at`, `string_from_char`, `char_is_digit/alpha/whitespace`, `file_read_all`, `file_read_line`, `file_write` and `panic` failed with `Use of uninitialized value` | `src/ownership/borrow_checker.rs` vs `src/typeck/mod.rs:363-569` | **fixed** — `src/builtins.rs` is now the single table both passes derive from, with drift tests |
| D3 | a tail expression in a value-returning function emitted no `return`, so `fn add(a,b) -> i64 { a + b }` compiled clean and returned garbage. All of `stdlib/` was affected | `src/parser/mod.rs:1263`, `src/codegen/mod.rs:1353` | **fixed** — lowered to `Stmt::Return` in the parser |
| D6 | call-argument borrows were registered with `Lifetime::Named("fn")` while `exit_scope` released only `Lifetime::Scope(n)`, so every argument stayed borrowed forever; and `String`/array parameters were classified `Move` although codegen passes pointers and never frees | `src/ownership/borrow_checker.rs` `collect_function_sig_with_name` / `check_call_args`; `src/ownership/mod.rs:141-178` | **fixed** — borrows end with the call; `String` is Copy (language-spec §9.1); array params are borrows. The `Lifetime::Scope(n)` half of the description was worse than it read: that variant is constructed nowhere, so `exit_scope` released nothing of any lifetime and `borrows` grew for the whole compilation. It now releases by recorded scope depth |
| D8 | codegen emitted no C prototypes, so calling a function defined later in the file produced C that gcc rejects — and mutual recursion was inexpressible | `src/codegen/mod.rs` | **fixed** — prototypes emitted for every user function |
| D4 | `for` over an array *parameter* used `sizeof` on a decayed pointer, so the loop ran once for `i64` and twice for `i32` | `src/codegen/mod.rs` for-in arm | **fixed** — the bound comes from the declared length; a length codegen cannot resolve is a compile error on a parameter, not a wrong bound |
| D5 | `?` emitted C for a `struct Result` layout codegen never defines, and `.await` emitted a call to a `poll` member no generated struct has. Neither was an error: both programs died inside gcc, against C the user never wrote. The LLVM backend was worse — its catch-all returns the constant `0` for both | `src/codegen/mod.rs:2186`, `src/codegen/mod.rs:2234` (pre-fix); `src/codegen/llvm_text_backend.rs:1378` | **fixed** — both rejected with "is not implemented" plus consequence and a workaround that is compiled and run by `tests/d5_unimplemented_constructs.rs` (`src/typeck/mod.rs:2439`, `src/typeck/mod.rs:2446`; backstop at `src/codegen/mod.rs:2563`, `src/codegen/mod.rs:2575`). Old lowerings deleted, not flagged off. PBS-1 still excludes both |
| D7 | a `let` with no type annotation was emitted as `long long` whatever the initializer was, so references, enum values and string copies silently became integers | codegen let-inference | **fixed** — inference now covers literals, calls, struct/enum values, references, deref, field and index expressions; an initializer with no rule is a compile error naming the variable, never a guess |
| D9 | reference-to-array parameter types (`&[T; N]`, `&mut [T; N]`) were rejected by codegen: "Unsupported type in reference parameter" | `src/codegen/mod.rs` reference-parameter arm | **fixed** — both lower to the decayed pointer C gives an array parameter, `&` const-qualifying the element slot. Writing through a shared or a bare array parameter, or passing one on to a parameter that may write, is a compile error (language-spec §9.2) |

## 8. Builtin surface available to PBS-1

The self-hosting compiler is built entirely from these (full table in
`docs/specification/language-spec.md` §8):

- I/O: `print`, `print_int`, `panic`
- String: `string_len`, `string_concat`, `string_eq`, `string_char_at`, `string_substring`,
  `string_from_char`, `string_to_int`, `int_to_string`
- Char classification: `char_is_digit`, `char_is_alpha`, `char_is_whitespace`
- Files: `file_open`, `file_read_all`, `file_read_line`, `file_write`, `file_close`,
  `file_exists`, `read_file_to_string`, `write_string_to_file`

`String` supports `+` for concatenation. There is no `Vec`; use fixed-size arrays with an
explicit length counter — the standard PBS-1 idiom:

```palladium
fn push(mut kinds: [i64; 4096], mut count: i64, k: i64) -> i64 {
    kinds[count] = k;
    return count + 1;
}
```

## 9. The self-hosting gate

Self-hosting is claimed only when this sequence is green, and the receipt is the byte
comparison in step 4 — not a document, not a demo:

```
stage0:  pdc (Rust)  compiles  bootstrap/pdc.pd   ->  pdc1
stage1:  pdc1        compiles  bootstrap/pdc.pd   ->  pdc2
stage2:  pdc2        compiles  bootstrap/pdc.pd   ->  pdc3
gate:    C output of stage1 and stage2 are byte-identical   (fixed point)
```

A compiler that passes stage1 but whose stage2 output differs is not self-hosting; it is a
compiler that happens to parse itself. `scripts/selfhost.sh` implements this gate.

### 9.1 Result (2026-08-21)

**The fixed point was reached.**

```
stage1 C output:  972 lines   sha1 9b0cf24e640eb689a1744ffdf589a44428ef5649
stage2 C output:  972 lines   sha1 9b0cf24e640eb689a1744ffdf589a44428ef5649
cmp -s c1.c c2.c  ->  identical
```

Functional check, not just byte equality: the stage-2 compiler was used to compile
`bootstrap/tests/hello.pd`, and the resulting program's output is identical to the same program
compiled by the Rust `pdc`:

```
demo / 3 / 60 / small / big      (both)
```

`bootstrap/pdc.pd` is ~760 lines of PBS-1. It lexes, resolves a per-function symbol table,
and emits C in three passes (struct definitions, prototypes, bodies), writing the output file
incrementally rather than accumulating a string.

The whole chain runs unaided: `make selfhost`.
