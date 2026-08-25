# WT-01: what the no-workaround JSON parser looks like, and what stands between

`tests/witness/json_parser.pd` runs today. It does **not** satisfy WT-01, which asks for a JSON
parser written with **no workarounds**. This file states the difference so that on the day M2
items 3 and 4 land, the claim *"the workarounds are gone"* can be **checked against a written
target** rather than re-derived by whoever happens to be reading.

Everything below was produced by writing the parser and recording where the compiler stopped, not
by reading the roadmap. Every diagnostic is quoted verbatim from `./target/release/pdc`. The gap
list was measured before `cfa7e7f`; that commit is the recursive-type layout rule, and the only row
it moves is row 1, marked **closed** below. Row 4 was re-probed at `52e629a` and reproduces its
diagnostic verbatim (`Type mismatch: expected [K; 4], found [K; 4]`, followed by `note: Types must
match exactly in Palladium`). Rows 8, 10, 11, 16 and 18 were re-probed at `52e629a` too and carry that measurement, not the
original. Rows 8 and 18 are **closed** there, and the commit that closed them is not `cfa7e7f`
but `bec9635` — the lexical completion naming N2-03/04/08/09/10/11, merged to `main` as
`fb12f6f`, with `c3307ed` later refining the escape fixture whose bytes row 18 quotes. Rows 10
and 11 moved because closing row 8 changed what blocks them, and row 16 moved because its gap
turned out to be the numeric↔text boundary in both directions rather than float arithmetic.
Every row not named in this paragraph carries its pre-`cfa7e7f` measurement.

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

Nine of the constructs in that `impl` block have a row in the gap list below: `ref mut self` (5),
the method call `j.peek()` (6), `match` as an expression (7), char literals (8), literal patterns
(9), or-patterns (10), the range pattern `'0'..='9'` (11), `loop { … }` (13) and `self.pos += 1`
(14). **One of the nine exists — char literals (8) lex today; the other eight do not.** That is the measurement this witness was written to take: `enum`
payloads, `impl` and `match` on enums all work today, so the parser's *skeleton* is expressible —
and then eight of the nine constructs that make a parser a parser still are not.

## The gap list

`site` is where the workaround is marked in `json_parser.pd`. `owner` is the requirement row that
owns the missing construct — or **UNOWNED**, meaning nothing in
`docs/contributing/1.0-requirements.tsv` declares it, or **closed**, meaning the construct now
exists and the row is history.

| # | Wanted | Diagnostic today | Workaround in the witness | Owner |
|---|---|---|---|---|
| 1 | `enum Json { Array(Vec<Json>) }` — a recursive type | **none — closed at `cfa7e7f` for the recursion mechanism, as measured over `Custom`, `Array` and `Tuple` payload slots.** `enum V { Leaf(i64), Pair(V, V) }` compiles, links and runs (a self-recursive `depth` returns 3 for `Pair(Leaf, Pair(Leaf, Leaf))`), and a cycle with no `enum` payload slot to stop at is now refused by name: ``recursive type `J` has no layout: it stores itself by value (J -> J), and nothing on that cycle can stop. …`` Both probes are replayable as tests, in the 940-line suite that commit landed: `tests/m2_recursive_data_types.rs::a_match_arm_binds_a_subtree_and_recurses_on_it` is the depth-3 run, `tests/m2_recursive_data_types.rs::recursion_through_an_array_is_refused_rather_than_guessed_at` is the named refusal. **What was measured is not this row's own spelling.** `Vec<Json>` is a *generic* payload, and the layout rule never sees a generic cycle: `enum J { Num(i64), Arr(Vec<J>) }` reaches exit 0 through `void*` without the layout rule ever seeing the cycle. The row is counted closed because the recursion mechanism is what its workaround needed — the generic-payload case is untested residual, carried in full by the first bullet of *What is better than the roadmap assumes* below | none needed for the recursion; the arena stays for rows 2 and 3 | **closed** (was UNOWNED) |
| 2 | `Vec<Json>` | — | fixed capacity 192 | N14-09 (M8) |
| 3 | `Vec<(String, Json)>` for object members | `Expected ')' after expression, but found ','` | a `key: [String; 192]` array parallel to the nodes | N4-12 (M2) |
| 4 | `[Kind; N]` — an array of a user `enum` | `Type mismatch: expected [K; 4], found [K; 4]` for **both** `[K::A; 4]` and `[K::A, K::A, K::A, K::A]` | kinds are `i64`, named by zero-arg functions | **UNOWNED** (N4-09 is `satisfied` and states it is witnessed for `[i64; N]`/`[String; N]` only) |
| 5 | `fn peek(ref self)` / `fn walk(p: ref Json)` — a shared borrow that can be **forwarded** | `fn a(p: &P) -> i64 { return b(p); }` emits `b((*p))` and dies in gcc: `passing 'const struct P' to parameter of incompatible type 'const struct P *'`. The non-borrow spelling `fn get(p: Json)` is a move: `Use of moved value: p` | every function takes `mut j: Json`, including the ones that only read | **UNOWNED** for the implemented `&` spelling; N9-01/N9-02/N4-13 (M7) own `ref T` |
| 6 | `j.peek()` | `Indirect function calls not yet supported` | free functions `js_*(j, …)` | N5-17 (M2) |
| 7 | `match self.peek() { Some('n') => … }` as an **expression** | `Expected expression, but found 'match'` | a `let mut node: i64 = -1;` and one assignment per branch | N5-04 (M2) |
| 8 | `'n'`, `'"'`, `'\t'` — char literals | **closed.** `let c: i64 = 'a';` compiles (rc=0), and so do `c == 'a'` and `c >= '0' && c <= '9'`. Char literals are expressions now; what still fails is a char literal in *pattern* position, which is row 9 | none. Twenty-nine of the 31 zero-arg byte-code functions are deleted; `ch_backspace` and `ch_formfeed` stay because `'\b'` and `'\f'` are outside the closed escape set and are correctly refused — the set behaving as specified, not a workaround | N2-04 (M2), `satisfied` |
| 9 | literal patterns in the arms | `Expected pattern, but found integer 0` | an `if` staircase over `string_char_at`'s `i64` | N6-02 (M2) |
| 10 | `Some(' ') \| Some('\t') \| …` — or-patterns | `Expected pattern, but found char 'a'` — blocked by row 9 alone, now that 8 is closed | `js_is_ws`, a boolean helper | N6-07 (M2) |
| 11 | `'0'..='9'` — a range pattern | `Expected pattern, but found char '0'` — blocked by row 9 alone, now that 8 is closed | `js_is_digit`, a boolean helper | N6-03 (M2) |
| 12 | `else if` | `Expected '{' after else, but found 'if'` | nested `else { if … }` staircases, nine deep in `js_value` | N5-06 (M2) |
| 13 | `loop { … }` | `loop` lexes as an identifier: `Expected ';' after expression, but found '{'` | `while true` | N5-07 (M2) |
| 14 | `self.pos += 1` | `Expected expression, but found '='` | `j.pos = j.pos + 1` | N5-13 (M2) |
| 15 | `(v << 4) \| d`, `cp >> 6`, `cp & 63` | `<<` → `Expected expression, but found '<'`; `\|` and `&` end the enclosing expression; `^` → `Unexpected character '^'` | `v * 16 + d`, `cp / 64`, `cp % 64` | N5-12 (M2) |
| 16 | `Number(f64)` | **not float arithmetic — the numeric↔text boundary, in both directions.** `let x: f64 = 1.5;`, `x + 2.25` and `y > 3.0` all compile (rc=0), so N4-02 is not the missing piece. Nothing carries a float across the text boundary either way: `src/builtins.rs` declares no `string_to_float` and no `parse_float`, so a parsed lexeme cannot become a value, and no `float_to_string` and no `print_float`, so a computed value cannot be printed — `print_float(x)` is `Undefined function: 'print_float'. Did you mean 'print_int'?` | integer numbers carry a value; non-integers carry only their source lexeme, and an `exact` flag says which | **UNOWNED** (N4-02 is `f32 f64` and is `satisfied`; no row owns either direction of the boundary) |
| 17 | `const CAP: usize = 192;` | `Expected function, struct, enum, trait, type, impl, or macro declaration` | zero-arg functions | N3-09 / N3-10 (M2) |
| 18 | `"\\t"` — a backslash followed by `t` | **closed.** Measured now: `"\\t"` → bytes `92 116`, `"\\n"` → `92 110`, `"\\"` → the single byte `92`, and `"[\"tab\\there\"]"` → exactly the 13 bytes of `["tab\there"]`. The `String::replace` chain this row described is gone — `grep -rn 'replace(' src/lexer/` is empty; escapes are lexed against the closed set `\n \t \r \" \\ \'` and anything outside it is a compile error carrying the offending character (`LexError::UnknownEscape`) | none. `bs()` is deleted and the three renderer sites emit the literal | N2-09 (M2), `satisfied` |
| 19 | a JSON document containing `\u0000` | — | refused with a named reason; `String` is a NUL-terminated `char*` (`__pd_string_from_char` writes `result[1] = '\0'`) so the byte cannot be carried | **UNOWNED** (N4-05 is the single word `String`, and is `satisfied`) |

Rows 1–19 above are 19 wants, **of which rows 1, 8 and 18 are closed**; the file carries **fifteen**
`// WORKAROUND` comments — fourteen distinct, N5-12 twice — because rows 1, 8 and 18 are closed and
carry no marker at all, rows 2 and 3 share the arena marker at `json_parser.pd:88`, and rows 7 and 9
share the `match`-to-`if`-staircase marker at `json_parser.pd:567`. 19 − 3 − 1 − 1 = 14 distinct;
N5-12 is marked at two separate sites, giving 15 comments.

The arena marker names **three** owners, not two — `// WORKAROUND N14-09 + N4-12 + N4-15` — and the
third is why row 2's single line of the table is not the whole bill. `Vec<Json>` needs `Vec`, which
the closed builtin set does not have (N14-09, M8; `Vec::new()` is `Undefined enum type: Vec` today),
*and* it needs the mechanism underneath `Vec<T>`: named generic types `Name<A, B>`, monomorphised,
which is N4-15 (`docs/contributing/1.0-requirements.tsv:188`, M3, `owed`). `(String, Json)` for the
member pairs needs tuples, N4-12 (M2). The gap list gives N4-15 no row of its own because it is not
a separate *want* of this parser — row 2's want already **is** a named generic instantiation, so
the table counts the want once and the marker names every row that has to land before that want
can be written. The same shape is why row 3 shares the marker rather than carrying its own.

That arithmetic has to land on the anchored greps below, and does; it is not asserted against them.
Distinctness is checkable by command too:

```
grep -E '^[[:space:]]*// WORKAROUND ' tests/witness/json_parser.pd \
  | sed -E 's@^[[:space:]]*// WORKAROUND ([^:]+):.*@\1@; s/ again$//' \
  | sort | uniq -c | sort -rn
```

It prints **12** marker strings over the 15 comments, and its decisive property is the top two
lines: `3 UNOWNED` and `2 N5-12`, with every other string at 1. `N5-12` is the only requirement id
that repeats, which is the duplicate this paragraph names — the second `s///` is what makes it
visible, because the file spells the repeat `N5-12 again` on purpose at `json_parser.pd:285`.
`UNOWNED` is a word rather than an id, and its three sites are three *different* workarounds — rows
4, 16 and 19 — distinguished by site and not by string; so 12 strings + 2 (the two extra UNOWNED
workarounds) = 14 distinct workarounds across 15 comments. The fourth UNOWNED finding, row 5, does
not collapse here because its marker is spelled `UNOWNED (codegen) + N9-01`; the UNOWNED count of 4
is the second anchored grep below, not this pipeline. The repeat can also be isolated on its
own: `grep -nE '^[[:space:]]*// WORKAROUND [^:]* again:' tests/witness/json_parser.pd` prints
exactly one line, and that line names N5-12.

## The four UNOWNED findings, which are the point of the exercise

An inventory cannot see work nobody declared. These four are what a witness finds that a requirement
filter cannot, and the list has turned over once. Recursive data compiling to C that gcc rejects was
on it when the file was written and is **closed**; see *What is better than the roadmap assumes*.
Item 4 took its place when the number workaround was re-measured: it had been tagged N4-02, which is
`satisfied`, so the tag was wrong, and the thing actually missing there is owned by nobody.

1. **`[EnumType; N]` is not constructible, and says so wrongly.** `Type mismatch: expected [K; 4],
   found [K; 4]` — the same type on both sides, in both the repeat and the literal spelling. N4-09
   is `satisfied` and is careful to say it is an instance claim over `[i64; N]` and `[String; N]`;
   the general case is therefore owned by nobody, and it is the reason this parser stores kinds as
   integers instead of as the enum it declares in a comment.
2. **A `&T` parameter cannot be forwarded.** `fn a(p: &P) -> i64 { return b(p); }` emits
   `b((*p))` — a dereference into a pointer parameter — silently, and gcc catches it. Recursive
   descent is nothing *but* forwarding, so the entire shared-borrow spelling is unusable in the one
   program shape WT-01 asks for. N9-01/N9-02/N4-13 name `ref T`/`ref mut T` and are owned by M7;
   **no row owns the `&` spelling that the compiler actually implements**, so this defect is
   invisible to every inventory.
3. **`String` has no declared byte-level meaning.** N4-05 is one word and is `satisfied`. In the
   implementation it is a NUL-terminated C `char*`, so `\u0000` is not representable — a
   conformance question for any parser of a format that permits it, decided today by an
   implementation detail rather than by a row.
4. **Nothing moves a float across the text boundary, in either direction.** `f64` exists and
   arithmetic on it compiles, so N4-02 (`f32 f64`, `satisfied`) is not the missing piece. Inward,
   `src/builtins.rs` declares no `string_to_float` and no `parse_float`, so a lexeme this parser
   has already validated cannot become a value. Outward, it declares no `float_to_string` and no
   `print_float` — `print_float(x)` is `Undefined function: 'print_float'. Did you mean
   'print_int'?` — so a value the language can compute cannot be printed. Grepping that file for
   `float` returns nothing at all, and no row owns *either* direction.

## What is *better* than the roadmap assumes

Recorded because a pessimistic manifest costs as much as an optimistic one.

- **Recursive data has a layout rule and a named diagnostic**, as of `cfa7e7f` — which lands *after*
  the gap list above was measured, and closes its row 1. `enum V { Leaf(i64), Pair(V, V) }` compiles,
  links and runs. The probes behind that sentence are not one-off shell runs: they live as
  replayable tests in the 940-line `tests/m2_recursive_data_types.rs` that commit landed —
  `a_match_arm_binds_a_subtree_and_recurses_on_it` is the depth-3 run over bound payloads, and
  `recursion_through_an_array_is_refused_rather_than_guessed_at` is the named refusal of a cycle
  with no `enum` payload slot on it. The one indirection the compiler introduces is an `enum` payload slot whose type
  reaches its own enum; a cycle carrying no such slot is refused with a named, actionable message
  instead of being handed to gcc. What still blocks the natural `enum Json` is `Vec` (row 2, N14-09,
  M8) and tuples (row 3, N4-12, M2) — both **owned**. What the closure does *not* establish is that
  the natural `enum Json` will lay out once those land: `labelled_occurrences` in `src/typeck/mod.rs`
  drops `Type::Generic` into its `_ => {}` arm, so a generic's type arguments contribute no
  containment edge, and `src/codegen/mod.rs:1824-1826` lowers `Type::Generic` to `"void*"` under a
  `TODO`. `enum J { Num(i64), Arr(Vec<J>) }` therefore reaches exit 0 without the layout rule ever
  seeing the cycle. The rule is measured for `Custom`, `Array` and `Tuple` payloads only.
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
  refusal transcript lines name the right offset. Fifteen is a measured number, not a rounded one:
  `grep -cE '^[[:space:]]*show_err\(j,' tests/witness/json_parser.pd` is 15 — the call sites in
  `main`, anchored for the same reason the two greps below are, so that a comment quoting the
  pattern cannot inflate it — matching the fifteen lines after `-- refused --` in the pinned
  transcript `tests/witness/json_parser.expected`.

## The recommended manifest change, and its evidence

**`WT-01` stays `owed`.** Recommended edit to `docs/contributing/1.0-requirements.tsv` — **not made
here**, because that file belongs to another lane this round:

```
-WT-01	M2	N1	Witness 2 exists: a JSON parser written with no workarounds, in the corpus	fixture	tests/witness/json_parser.pd
+WT-01	M2	N1	Witness 2 exists: a JSON parser written with no workarounds, in the corpus. THE FIXTURE EXISTS AND RUNS (tests/witness/json_parser.pd, class=run, transcript pinned); the row is owed on the words NO WORKAROUNDS, and the count is derivable: 15 `// WORKAROUND` comments, 14 distinct, 4 of them UNOWNED. See tests/witness/json_parser.no-workarounds.md	fixture	tests/witness/json_parser.pd
```

Evidence for leaving it `owed`, as two commands rather than as this paragraph:
`grep -cE '^[[:space:]]*// WORKAROUND ' tests/witness/json_parser.pd` is **15** (the anchor excludes
the self-references in the file's own header) and
`grep -cE '^[[:space:]]*// WORKAROUND UNOWNED' tests/witness/json_parser.pd` is **4**. The second is
anchored too, and deliberately: the unanchored `grep -c 'WORKAROUND UNOWNED'` also counts prose that
merely quotes the pattern, so it can be inflated by editing a comment. Both figures in the row text
and in the conformance-manifest note are therefore checkable by command rather than by reading this
file.

A second recommendation, offered rather than made: the four UNOWNED findings want **four new
rows**, because the M2 filter can today reach zero-owed while `fn a(p: &P) -> i64 { return b(p); }`
still emits a dereference into a pointer parameter — the same hole item 6 of M2 closed for the
builtins by adding N14-17.
