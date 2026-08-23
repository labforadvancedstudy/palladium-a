# WT-01: what the no-workaround JSON parser looks like, and what stands between

`tests/witness/json_parser.pd` runs today. It does **not** satisfy WT-01, which asks for a JSON
parser written with **no workarounds**. This file states the difference so that on the day M2
items 3 and 4 land, the claim *"the workarounds are gone"* can be **checked against a written
target** rather than re-derived by whoever happens to be reading.

Everything below was produced by writing the parser and recording where the compiler stopped, not
by reading the roadmap. Every diagnostic is quoted verbatim from `./target/release/pdc` at
`d20b759`.

## The shape it would have

```palladium
enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

struct Error { message: String, offset: usize }
struct Parser { src: String, pos: usize, depth: usize }

impl Parser {
    fn peek(ref self) -> Option<char> { ... }

    fn skip_ws(ref mut self) {
        loop {
            match self.peek() {
                Some(' ') | Some('\t') | Some('\n') | Some('\r') => self.pos += 1,
                _ => break,
            }
        }
    }

    fn value(ref mut self) -> Result<Json, Error> {
        self.skip_ws();
        match self.peek() {
            Some('n')  => self.literal("null", Json::Null),
            Some('t')  => self.literal("true", Json::Bool(true)),
            Some('f')  => self.literal("false", Json::Bool(false)),
            Some('"')  => Ok(Json::Str(self.string()?)),
            Some('[')  => self.array(),
            Some('{')  => self.object(),
            Some(c @ '0'..='9') => self.number(),
            Some('-')  => self.number(),
            Some(_)    => Err(self.err("unexpected character")),
            None       => Err(self.err("unexpected end of input")),
        }
    }
}
```

Nine constructs appear in those twenty lines. **None of the nine exists.** That is the measurement
this witness was written to take: `enum` payloads, `impl` and `match` on enums all work today, so
the parser's *skeleton* is expressible — and then every single line that makes a parser a parser is
not.

## The gap list

`site` is where the workaround is marked in `json_parser.pd`. `owner` is the requirement row that
owns the missing construct — or **UNOWNED**, meaning nothing in
`docs/contributing/1.0-requirements.tsv` declares it.

| # | Wanted | Diagnostic today | Workaround in the witness | Owner |
|---|---|---|---|---|
| 1 | `enum Json { Array(Vec<Json>) }` — a recursive type | *accepted* by parser, typeck AND borrowck, prints "✅ Compilation successful", then gcc: `field has incomplete type 'struct V'`. Constructing one: `Type mismatch: expected V, found V` | flat arena of parallel arrays indexed by `i64`, `-1` = none, capacity failure is a parse error | **UNOWNED** |
| 2 | `Vec<Json>` | — | fixed capacity 192 | N14-09 (M8) |
| 3 | `Vec<(String, Json)>` for object members | `Expected ')' after expression, but found ','` | a `key: [String; 192]` array parallel to the nodes | N4-12 (M2) |
| 4 | `[Kind; N]` — an array of a user `enum` | `Type mismatch: expected [K; 4], found [K; 4]` for **both** `[K::A; 4]` and `[K::A, K::A, K::A, K::A]` | kinds are `i64`, named by zero-arg functions | **UNOWNED** (N4-09 is `satisfied` and states it is witnessed for `[i64; N]`/`[String; N]` only) |
| 5 | `fn peek(ref self)` / `fn walk(p: ref Json)` — a shared borrow that can be **forwarded** | `fn a(p: &P) -> i64 { return b(p); }` emits `b((*p))` and dies in gcc: `passing 'const struct P' to parameter of incompatible type 'const struct P *'`. The non-borrow spelling `fn get(p: Json)` is a move: `Use of moved value: p` | every function takes `mut j: Json`, including the ones that only read | **UNOWNED** for the implemented `&` spelling; N9-01/N9-02/N4-13 (M7) own `ref T` |
| 6 | `j.peek()` | `Indirect function calls not yet supported` | free functions `js_*(j, …)` | N5-17 (M2) |
| 7 | `match self.peek() { Some('n') => … }` as an **expression** | `Expected expression, but found 'match'` | a `let mut node: i64 = -1;` and one assignment per branch | N5-04 (M2) |
| 8 | `'n'`, `'"'`, `'\t'` — char literals | `Expected expression, but found '` | 31 zero-arg functions returning decimal byte codes | N2-04 (M2) |
| 9 | literal patterns in the arms | `Expected pattern, but found integer 0` | an `if` staircase over `string_char_at`'s `i64` | N6-02 (M2) |
| 10 | `Some(' ') \| Some('\t') \| …` — or-patterns | (unreachable: needs 8 and 9 first) | `js_is_ws`, a boolean helper | N6-07 (M2) |
| 11 | `'0'..='9'` — a range pattern | (unreachable: needs 8 and 9 first) | `js_is_digit`, a boolean helper | N6-03 (M2) |
| 12 | `else if` | `Expected '{' after else, but found 'if'` | nested `else { if … }` staircases, nine deep in `js_value` | N5-06 (M2) |
| 13 | `loop { … }` | `loop` lexes as an identifier: `Expected ';' after expression, but found '{'` | `while true` | N5-07 (M2) |
| 14 | `self.pos += 1` | `Expected expression, but found '='` | `j.pos = j.pos + 1` | N5-13 (M2) |
| 15 | `(v << 4) \| d`, `cp >> 6`, `cp & 63` | `<<` → `Expected expression, but found '<'`; `\|` and `&` end the enclosing expression; `^` → `Unexpected character '^'` | `v * 16 + d`, `cp / 64`, `cp % 64` | N5-12 (M2) |
| 16 | `Number(f64)` | `1.5` → `Expected field name, but found integer 5` | integer numbers carry a value; non-integers carry only their source lexeme, and an `exact` flag says which | N4-02 (M2) |
| 17 | `const CAP: usize = 192;` | `Expected function, struct, enum, trait, type, impl, or macro declaration` | zero-arg functions | N3-09 / N3-10 (M2) |
| 18 | `"\\t"` — a backslash followed by `t` | **no diagnostic — a wrong answer.** `src/lexer/token.rs:16-21` decodes escapes as five independent `String::replace` passes in the order `\n \t \r \" \\`, so `\\t` is matched by the `\t` pass at offset 1. Measured: `"\\t"` → bytes `92 9` (want `92 116`), `"\\n"` → `92 10`, `"\\r"` → `92 13`; `"\\\\"` → `92 92` and `"\\/"` → `92 47` are correct | `bs()` = `string_from_char(92)`, concatenated at run time | N2-09 (M2) |
| 19 | a JSON document containing `\u0000` | — | refused with a named reason; `String` is a NUL-terminated `char*` (`__pd_string_from_char` writes `result[1] = '\0'`) so the byte cannot be carried | **UNOWNED** (N4-05 is the single word `String`, and is `satisfied`) |

Rows 1–19 above are 19 wants; the file carries **seventeen** `// WORKAROUND` comments — sixteen
distinct, N5-12 twice — because rows 1–3 share one arena and rows 10–11 sit inside the staircase
that row 12 already pays for.

## The four UNOWNED findings, which are the point of the exercise

Inventory four cannot see work nobody declared. These four are what a witness finds that a
requirement filter cannot.

1. **Recursive data has no owner and no diagnostic.** `enum V { Leaf(i64), Pair(V, V) }` passes the
   parser, the type checker *and* the borrow checker, prints `✅ Compilation successful!`, and dies
   in gcc against C the user never wrote. That is D5's exact shape, still live, in the one
   construct every tree-shaped program needs. The manifest has no `Box` row, no indirection row and
   no recursive-type row; N8-06 and N8-07 (M6) *presuppose* "inductive types" that no row makes
   representable. **A JSON parser cannot have a JSON value type until this is owned.**
2. **`[EnumType; N]` is not constructible, and says so wrongly.** `Type mismatch: expected [K; 4],
   found [K; 4]` — the same type on both sides, in both the repeat and the literal spelling. N4-09
   is `satisfied` and is careful to say it is an instance claim over `[i64; N]` and `[String; N]`;
   the general case is therefore owned by nobody, and it is the reason this parser stores kinds as
   integers instead of as the enum it declares in a comment.
3. **A `&T` parameter cannot be forwarded.** `fn a(p: &P) -> i64 { return b(p); }` emits
   `b((*p))` — a dereference into a pointer parameter — silently, and gcc catches it. Recursive
   descent is nothing *but* forwarding, so the entire shared-borrow spelling is unusable in the one
   program shape WT-01 asks for. N9-01/N9-02/N4-13 name `ref T`/`ref mut T` and are owned by M7;
   **no row owns the `&` spelling that the compiler actually implements**, so this defect is
   invisible to every inventory.
4. **`String` has no declared byte-level meaning.** N4-05 is one word and is `satisfied`. In the
   implementation it is a NUL-terminated C `char*`, so `\u0000` is not representable — a
   conformance question for any parser of a format that permits it, decided today by an
   implementation detail rather than by a row.

## What is *better* than the roadmap assumes

Recorded because a pessimistic manifest costs as much as an optimistic one.

- **Struct-of-arrays state threads through recursive descent correctly.** `mut j: Json` lowers to
  `struct Json*`; mutations propagate to the caller, a `mut` parameter forwards to another `mut`
  parameter, and mutual recursion links (D8's prototypes). The arena workaround is *ugly*, not
  *fragile*.
- **`[String; N]` fields work**, including assigning a run-time-built owned string into a slot —
  which is what makes decoded string values storable at all.
- **Enums with payloads, `match` on them, and exhaustiveness all work.** `match e { E::A => … }`
  missing an arm is `Non-exhaustive match expression`. N6-10 says "for EVERY scrutinee type"; for
  the *enum* scrutinee it is already true today, so N6-10's residue is integers and strings, which
  is downstream of N6-02 rather than independent work.
- **Effect inference already reads this program correctly** with no annotation: it reports
  `js_render` as `[Memory]` and `show_ok` as `[IO, Memory]`, propagated transitively through
  eleven functions.
- **Diagnostics carry byte offsets and the parser's own error positions are exact** — all fifteen
  refusal transcript lines name the right offset.

## The recommended manifest change, and its evidence

**`WT-01` stays `owed`.** Recommended edit to `docs/contributing/1.0-requirements.tsv` — **not made
here**, because that file belongs to another lane this round:

```
-WT-01	M2	N1	Witness 2 exists: a JSON parser written with no workarounds, in the corpus	fixture	tests/witness/json_parser.pd
+WT-01	M2	N1	Witness 2 exists: a JSON parser written with no workarounds, in the corpus. THE FIXTURE EXISTS AND RUNS (tests/witness/json_parser.pd, class=run, transcript pinned); the row is owed on the words NO WORKAROUNDS, and the count is derivable: 17 `// WORKAROUND` comments, 16 distinct, 4 of them UNOWNED. See tests/witness/json_parser.no-workarounds.md	fixture	tests/witness/json_parser.pd
```

Evidence for leaving it `owed`, as two commands rather than as this paragraph:
`grep -cE '^[[:space:]]*// WORKAROUND ' tests/witness/json_parser.pd` is **17** (the anchor excludes
the one self-reference in the file's own header) and `grep -c 'WORKAROUND UNOWNED'` on the same
file is **4**. Both figures in the row text and in the conformance-manifest note are therefore
checkable by command rather than by reading this file.

A second recommendation, offered rather than made: the four UNOWNED findings want **four new rows**,
because the M2 filter can today reach zero-owed with a recursive type still miscompiling — the same
hole item 6 of M2 closed for the builtins by adding N14-17.
