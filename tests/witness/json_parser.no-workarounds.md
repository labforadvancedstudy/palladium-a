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

**Rows 6, 7, 9, 10, 11, 12, 13, 14 and 15 were re-probed against `7eac786` (2026-08-31), and that
round is still the largest the file has taken.** Four of them — 12 (`else if`), 13 (`loop`), 14
(`+=`) and 15 (bitwise operators) — closed in that round, and closed the way the census is meant to
close a row: the construct was APPLIED in `json_parser.pd` and the workaround deleted, not
re-labelled. `tests/witness/json_parser.expected` was byte-identical across all four, which is the
receipt that a discharge happened rather than a rewrite. Rows 7, 9, 10 and 11 moved the other way
in that round: each named a `satisfied` row, each of those rows measured as satisfied **over an
`i64` scrutinee**, and what actually blocked this parser then was that a char literal was not a
pattern. That was one gap under four rows and three markers, owned by nobody — item 4 of the
UNOWNED findings below, which records the capability closed since and the ownership still open.
Row 6 moved too: `x.f()` worked by then, so the row's diagnostic was stale, and what blocked the
cursor at that round was a receiver that could be written through and called twice. su2
(`b54502b`) supplied that receiver, and the cursor round below applied it.

**Rows 5 and 6 were re-probed at `cece734` (2026-10-07), with `pdc run` throughout, and both are
closed, and APPLIED.** The cursor and the parser are methods on `Json` now — readers on `&self`,
writers on `&mut self`, every call on the document `j.thing(…)` or `self.thing(…)` — and `tests/witness/
json_parser.expected` is byte-identical across the rewrite. The receiver had already landed in su2
(`b54502b`); what the rewrite needed was two borrow-checker fixes that the rewrite itself found
(W1c and W1d, row 6 below), and it compiles on `cece734`, the `main` that carries both. Both cursor
markers are `// WORKAROUND DISCHARGED` records now, the live count fell six -> four and the
UNOWNED count four -> three. One measured cost came with it and is not a workaround: the methods
dropped out of the compiler's effects report, which is N7-07 — see *What the method rewrite made
visible* below.

**Row 4 was re-probed at `fafe870` (2026-11-08), with `pdc run` throughout, and it is closed, and
APPLIED.** W2a (`af5d6ac`, landed on `main` at `fafe870`) made the type checker keep a user enum an
enum inside every composite type, so both spellings the row quotes compile and run. The arena's
kind column is `kind: [Kind; 192]` over `enum Kind { Null, False, True, Number, Str, Array, Object }`,
the seven `k_*()` functions are deleted, every dispatch on a kind is a `match`, and
`tests/witness/json_parser.expected` is byte-identical across the rewrite. The marker is a
`// WORKAROUND DISCHARGED (was UNOWNED)` record now; the live count fell four -> three and the
UNOWNED count three -> two. The ownership did not move — finding 1 below — and the probe measured
one defect beside the construct that the witness does not use: `==` between two values of a user
enum type-checks and gcc refuses the C (row 4).

**The UNOWNED count fell two -> zero on 2026-11-08, and no line of `json_parser.pd` code changed.**
The owner decided issue #46 and M2's exit together. Each of the five UNOWNED findings got a row —
N4-24, N9-10 and N6-12 born `satisfied` for the three this witness already applied, N4-25 and
N14-18 born `owed` for the two it still works around — and the two bare `// WORKAROUND UNOWNED`
markers name those two rows now. The live count stays three, the transcript is untouched, and WT-01
moved from M2 to M8, because none of the three is M2's to pay (the arena waits on N14-09,
M8). The owner named M3 for it; the manifest header records why it landed on M8. Moving a row is not paying it, and WT-01 is still `owed`.

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
(14). **All nine are expressible today, and all nine are applied in the witness.** When this
paragraph was first written, four existed outright — char literals (8), `match` as an expression
(7), `loop { … }` (13) and compound assignment (14) — three existed over every scrutinee type except the
one this parser uses — literal (9), or- (10) and range (11) patterns dispatched on `i64`, `String`
and `bool` and refused a `char` — and two, both of them the receiver, did not exist at all. The
char-pattern round closed the three; the cursor round (`cece734`) closed the receiver, in the
spelling this compiler implements: `&self` / `&mut self`, not the `ref self` / `ref mut self` the
sketch writes. That spelling is still refused, measured at `cece734`: `fn get(ref self)` is
`Expected ':' (Expected ':' after parameter name), but found 'self'`, and `fn b(p: ref P)` is
`Expected ')' (Expected ')'), but found identifier 'P'` — N9-01/N9-02 (M7, `owed`) own it. So the
parser's *skeleton*, the arms of its `match` and the receiver that makes recursive descent a method
at all are all expressible and all written; every gap-list row still open is about the *data* —
rows 2, 3, 16 and 19.

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
| 4 | `[Kind; N]` — an array of a user `enum` | **closed, and APPLIED** at `fafe870` (2026-11-08). Measured before: `Type mismatch: expected [K; 4], found [K; 4]` for **both** `[K::A; 4]` and `[K::A, K::A, K::A, K::A]` — the type checker read the `K` inside a composite type as a struct while every `K::A` was an enum. W2a (`af5d6ac`, on `main` at `fafe870`) keeps an enum an enum inside every composite, witnessed by `tests/regression/enum_in_composites.pd`. Re-probed with `pdc run`: both spellings run, and so do a `[K; 192]` struct field beside `[i64; 192]` and `[String; 192]` ones, a write through `&mut self`, reads through `&self` (discarded, forwarded to a function, bound with and without an annotation, read twice), an enum parameter forwarded to another method, and `match` on an element as a statement and as an expression. Two things beside it do not work, and the witness uses neither. `[K; CAP]` with a `const` length is refused for every element type, `[i64; CAP]` included (`expected [Int; CAP], found [Int; 4]`), because an array length is an integer literal, so the column is `[Kind; 192]` like its neighbours. And `==`/`!=` between two values of a user enum — or of a struct — passes the type checker, and gcc refuses the C it emits with `invalid operands to binary expression ('struct K' and 'struct K')`; N5-10 ("Arithmetic and comparison operators", `satisfied`) names `tests/02_types_primitives.pd` as its evidence, which contains no comparison operator at all, and no row says whether `==` is defined on a user type | none. It was kinds stored as `i64`, named by seven zero-arg functions `k_null()` .. `k_object()`. It is `enum Kind` and `kind: [Kind; 192]`, and every dispatch on a kind is a `match`. `render` and `kind_name` name every variant, so a new one is `Non-exhaustive match: missing patterns Kind::…` at both (measured), where a new code fell through to "object"; `member` and `element` are guards with a `_` arm. `json_parser.expected` is byte-identical | **closed** — N4-24 (M2), `satisfied`, added 2026-11-08 from issue #46. Until then it was closed by capability and owned by no row: N4-09 states it is witnessed for `[i64; N]`/`[String; N]` only (finding 1 below) |
| 5 | `fn peek(ref self)` / `fn walk(p: ref Json)` — a shared borrow that can be **forwarded** | **closed, and APPLIED — in the `&` spelling.** Re-measured at `cece734` (2026-10-07): the diagnostic this row quoted — `fn a(p: &P) -> i64 { return b(p); }` emitting `b((*p))` and dying in gcc on `passing 'const struct P' to parameter of incompatible type 'const struct P *'` — is stale since su2 (`b54502b`), which made the call site and the declaration ask one predicate. That program runs and prints, and `tests/linker_diagnostics.rs::forwarding_a_shared_reference_compiles_links_and_runs` is its witness. In the witness's own emitted C, `peek` is `return __pd_Json_at(self, self->pos);` and `(*self)` occurs zero times. The `ref` spelling is still a parse error (`fn b(p: ref P)` is `Expected ')' (Expected ')'), but found identifier 'P'`) | none. It was `mut j: Json` on every function, readers included; every reader is a `&self` method now, and the one nested call in `main` that needed a temp under two `mut` borrows needs none | **closed** — N9-10 (M2), `satisfied`, added 2026-11-08 from issue #46 for the implemented `&` spelling, which no row named until then (finding 2 below); N9-01/N9-02/N4-13 (M7) own `ref T` |
| 6 | `j.peek()` | **closed, and APPLIED** at `cece734` (2026-10-07). `&self` is a shared borrow, so calling it twice is not a move, and a `&mut self` method writes its receiver and the caller observes it — both since su2 (`b54502b`), witnessed by `tests/04_self_place.pd`. Applying it to this parser found two borrow-checker defects no fixture had. A `self.<field>` read was a MOVE — `Use of moved value: self.pos` for `self.err_pos = self.pos;` then `let start: i64 = self.pos;` — because the checker walked the method with `self: &Self` and `Self` has no fields, so an `i64` field could not be seen to be Copy; W1c (`d159139`, `ad63231`) resolves `Self` there, witnessed by `tests/04_self_field_reads.pd`. And a projection's move leaked from one function body into the next; W1d (merged at `cece734`) scopes it, witnessed by `tests/regression/projection_move_scope.pd`. The witness compiling on `cece734` is the end-to-end receipt for both | none. It was free functions `js_*(j, …)`; the cursor and the parser are methods on `Json` now, in seven `impl Json` blocks beside their sections, and `json_parser.expected` is byte-identical | **closed** — N5-17 (M2) `satisfied`. The `ref mut self` spelling is still refused (`fn get(ref self)` is `Expected ':' (Expected ':' after parameter name), but found 'self'`) and still adjacent to N9-02 (M7) + N10-06 (M3) rather than owned by either, as before |
| 7 | `match self.peek() { Some('n') => … }` as an **expression** | **closed, and APPLIED.** `value`'s dispatch (`js_value` until the cursor round) IS a `match` expression now: `let node: i64 = match c { '\0' => { … } 'n' => { … } … _ => { … } };`. N5-04 was never the blocker and always measured so; row 9 was, and row 9 closed. The local is no longer `mut` and no arm assigns | none. It was `let mut node: i64 = -1;` and one assignment per branch | **closed** — the capability landed in the char-pattern round; N5-04 (M2) `satisfied` throughout |
| 8 | `'n'`, `'"'`, `'\t'` — char literals | **closed.** `let c: i64 = 'a';` compiles (rc=0), and so do `c == 'a'` and `c >= '0' && c <= '9'`. Char literals are expressions now; what still fails is a char literal in *pattern* position, which is row 9 | none. Twenty-nine of the 31 zero-arg byte-code functions are deleted; `ch_backspace` and `ch_formfeed` stay because `'\b'` and `'\f'` are outside the closed escape set and are correctly refused — the set behaving as specified, not a workaround | N2-04 (M2), `satisfied` |
| 9 | literal patterns in the arms | **closed, and APPLIED.** A char literal is a pattern: `'\0'`, `'n'`, `'t'`, `'f'`, `'"'`, `'['`, `'{'` and `'-'` are arms in `value`, and `'!'`-style literals are pinned by `tests/06_char_patterns.pd`. Measured before: `'n' => …` was `Expected pattern, but found char 'n'`. The lowering is numeric — `== 110 /* 'n' */` — not a C character constant, pinned by `tests/m2_char_patterns.rs` | none | **closed** — N6-12 (M2), `satisfied`, added 2026-11-08 from issue #46. N6-02's words remain "Literal patterns (integer, string, bool)"; the char pattern is a row of its own rather than an amendment |
| 10 | `Some(' ') \| Some('\t') \| …` — or-patterns | **closed, and APPLIED.** `js_is_ws` is `match c { ' ' \| '\t' \| '\n' \| '\r' => { true } _ => { false } }`, and `value` carries the mixed form `'-' \| '0'..='9'`. N6-07 was never the blocker — or-patterns over an `i64` always compiled — row 9 was | none. It was four `==` tests joined by `\|\|` | **closed** — N6-07 (M2), `satisfied` throughout; the char half is N6-12 (M2), added 2026-11-08 from issue #46 |
| 11 | `'0'..='9'` — a range pattern | **closed, and APPLIED.** `js_is_digit` is `match c { '0'..='9' => { true } _ => { false } }`. A char range is ordered by CODE POINT and lowers to two numeric comparisons (`>= 48 /* '0' */ && <= 57 /* '9' */`). N6-03 was never the blocker; row 9 was. An empty char range and a mixed-kind range are refused by name, pinned by `tests/reject/char_range_pattern_empty.pd` and `tests/reject/range_pattern_mixed_endpoints.pd` | none. It was `c >= '0' && c <= '9'` | **closed** — N6-03 (M2), `satisfied` throughout; the char half is N6-12 (M2), added 2026-11-08 from issue #46 |
| 12 | `else if` | **closed, and APPLIED.** The witness now carries 12 chained arms across 6 chains (`grep -cE '^[[:space:]]*\} else if ' tests/witness/json_parser.pd` is 12; it was 19 across 7 until `js_value`'s chain became a `match`), and **no `else` block in the file opens with an `if` as its first statement** — the shape the workaround had. The deepest chain, `js_value`, went from eight nested `else` blocks with its last branch eight levels deep to one `if`, seven chained arms and one `else` at a single indent | none. `json_parser.expected` is byte-identical across the rewrite | **closed** — N5-06 (M2), `satisfied` |
| 13 | `loop { … }` | **closed, and APPLIED.** `grep -cE '^[[:space:]]*loop \{' tests/witness/json_parser.pd` is 4 and `grep -cE '^[[:space:]]*while true \{'` is 0; the only surviving `while true` in the file is the header sentence describing this change | none | **closed** — N5-07 (M2), `satisfied` |
| 14 | `self.pos += 1` | **the compound assignment is closed and APPLIED, at every site.** `grep -cE '^[[:space:]]*[a-z_.]+ [-+]= ' tests/witness/json_parser.pd` is 11, `self.pos += 1;` among them, where the file once carried none. Ten were applied in the round that closed this row; the eleventh, `literal`'s cursor advance, was missed then and stayed `self.pos = self.pos + n;` through the cursor rewrite, until review caught it and it became `self.pos += n;`. No read-modify-write is left: `perl -ne 'print if /^\s*([a-z_.]+) = \1 [-+] /' tests/witness/json_parser.pd` prints nothing, where it printed that one line before. `self.pos` is writable too since su2, through `&mut self` — row 6 | none | **closed** — N5-13 (M2), `satisfied`. The `self.` half of this row's own spelling is row 6, also closed |
| 15 | `(v << 4) \| d`, `cp >> 6`, `cp & 63` | **closed, and APPLIED at both sites.** The hex accumulator is `v = (v << 4) \| d;` and the UTF-8 encoder is written in `>>` and `&` throughout (`(224 \| (cp >> 12)) as char`, `(128 \| ((cp >> 6) & 63))`). This is the row that used to carry the file's only duplicate marker; both sites are discharged, so no marker string repeats any more | none | **closed** — N5-12 (M2), `satisfied` |
| 16 | `Number(f64)` | **not float arithmetic — the numeric↔text boundary, in both directions.** `let x: f64 = 1.5;`, `x + 2.25` and `y > 3.0` all compile (rc=0), so N4-02 is not the missing piece. Nothing carries a float across the text boundary either way: `src/builtins.rs` declares no `string_to_float` and no `parse_float`, so a parsed lexeme cannot become a value, and no `float_to_string` and no `print_float`, so a computed value cannot be printed — `print_float(x)` is `Undefined function: 'print_float'. Did you mean 'print_int'?` | integer numbers carry a value; non-integers carry only their source lexeme, and an `exact` flag says which | **N14-18** (M8), `owed`, added 2026-11-08 from issue #46. It was **UNOWNED** until then: N4-02 is `f32 f64` and is `satisfied`, and no row owned either direction of the boundary |
| 17 | `const CAP: usize = 192;` | **closed.** Top-level `const` and `static` items landed with M2 item 9. The witness now writes `const CAP_NODES: i64 = 192;` and `const MAX_DEPTH: i64 = 24;` and reads them by name; the emitted C is `static const long long CAP_NODES = 192;`. `usize` is still not a type, so the row's own spelling remains unavailable and the witness uses `i64` — which is what N4's OPEN `str`/`usize` question owns, not this row | none. The two zero-arg functions are deleted | **closed** — N3-09 / N3-10 (M2), both `satisfied` |
| 18 | `"\\t"` — a backslash followed by `t` | **closed.** Measured now: `"\\t"` → bytes `92 116`, `"\\n"` → `92 110`, `"\\"` → the single byte `92`, and `"[\"tab\\there\"]"` → exactly the 13 bytes of `["tab\there"]`. The `String::replace` chain this row described is gone — `grep -rn 'replace(' src/lexer/` is empty; escapes are lexed against the closed set `\n \t \r \" \\ \'` and anything outside it is a compile error carrying the offending character (`LexError::UnknownEscape`) | none. `bs()` is deleted and the three renderer sites emit the literal | N2-09 (M2), `satisfied` |
| 19 | a JSON document containing `\u0000` | — | refused with a named reason; `String` is a NUL-terminated `char*` (`__pd_string_from_char` writes `result[1] = '\0'`) so the byte cannot be carried | **N4-25** (M8), `owed`, added 2026-11-08 from issue #46. It was **UNOWNED** until then: N4-05 is the single word `String`, and is `satisfied` |

Rows 1–19 above are 19 wants, **of which rows 1, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17 and
18 are closed**; the file carries **three** live `// WORKAROUND` comments — three distinct, one per
comment — because those fifteen rows are closed and carry no live marker, and rows 2 and 3 share the
arena marker. 19 − 15 − 1 = 3 distinct, and 3 comments. The `match`-to-`if`-staircase marker that
rows 7 and 9 used to share is gone with them: char patterns landed, and rows 7, 9, 10 and 11 were one
gap wearing four wants. Rows 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15 and 17 are the closures where the
CONSTRUCT ARRIVED; rows 1, 8 and 18 closed when a claim was re-measured against the compiler that
was already there. The file keeps four `// WORKAROUND DISCHARGED` comments — at row 17's old site,
at the two cursor markers rows 5 and 6 carried, and at the node kinds row 4 carried — and ten
shorter `// <row> DISCHARGED` notes — at the sites rows 12, 13, 14 and 15 changed (row 15 has two),
at the three sites the char-pattern round changed, at the one nested call in `main` the cursor round
un-temped, and at `render`, whose kind dispatch the node-kind round made a `match` — none of which
are counted as live: a closure with no trace at the site it changed is how a census stops being
checkable. The ten are a count, not a reading — this prints **10** (it printed 9 before the
node-kind round and 8 before the cursor round), and the backtick exclusion keeps the header's own
description of the form out of it:

```
grep -cE '^[[:space:]]*// [^`]+ DISCHARGED( here too)?:' tests/witness/json_parser.pd
```

The arena marker names **three** owners, not two — `// WORKAROUND N14-09 + N4-12 + N4-15` — and the
third is why row 2's single line of the table is not the whole bill. `Vec<Json>` needs `Vec`, which
the closed builtin set does not have (N14-09, M8; `Vec::new()` is `Undefined enum type: Vec` today),
*and* it needs the mechanism underneath `Vec<T>`: named generic types `Name<A, B>`, monomorphised,
which is N4-15 (the row of that id in `docs/contributing/1.0-requirements.tsv`, M3, `owed`; it was cited as line 188, which had stopped holding it before the ownership round and moved again in it, so it is spelled by id now). `(String, Json)` for the
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

It prints **7** marker strings over the 7 comments:

```
   1 N4-25
   1 N14-18
   1 N14-09 + N4-12 + N4-15
   1 DISCHARGED (was UNOWNED)
   1 DISCHARGED (was UNOWNED (codegen) + N9-01)
   1 DISCHARGED (was UNAPPLIED)
   1 DISCHARGED (N3-09, N3-10)
```

No string stands for more than one workaround now. Until 2026-11-08 one did, on purpose: bare
`UNOWNED` was a word rather than an id, and its two sites were two *different* gaps — rows 16 and
19 — distinguished by site and not by string. Issue #46 gave each gap a row, N14-18 and N4-25, and
each marker names its own; the code did not change. It had a third site, row 4's node kinds, until the
node-kind round; that marker is `DISCHARGED (was UNOWNED)` now, the one record whose parentheses
kept a string that was still live elsewhere until the ownership round, which is why it is a line of
its own here and not a third `UNOWNED`. The `UNOWNED (char pattern)` string that used to head this
list is gone: it was the opposite case, three sites over one gap, and the gap closed. So are
`UNOWNED (codegen) + N9-01` and `UNAPPLIED (su2 removed the blocker; the rewrite is owed)`, the two
cursor markers: each is a `DISCHARGED (was …)` record now, keeping the string it discharged inside
its parentheses so the history reads off this same pipeline. Subtract the four `DISCHARGED` lines,
which are history rather than workarounds: **3 live strings, 3 distinct workarounds across 3 live
comments**, one per comment. (It read "2 live strings + 1 (the extra bare-`UNOWNED` gap)" while the
two UNOWNED gaps shared one string.) No marker
string repeats an id: `grep -cE '^[[:space:]]*// WORKAROUND [^:]* again:' tests/witness/json_parser.pd`
is **0**, where it used to be 1 and named N5-12, whose two sites are both discharged. The UNOWNED
site count of **0** is the second anchored grep below, not this pipeline — a bare `UNOWNED` would
match it, and the arena marker, the two row markers and the four `DISCHARGED` records do not: a
record that keeps `UNOWNED` inside its parentheses, after `was`, is invisible to an anchor that wants
`UNOWNED` first. It was **2** until the ownership round (2026-11-08), when rows 16 and 19 still
carried a bare `UNOWNED`, **3** until the node-kind round, when the bare `UNOWNED` at the node kinds
still matched, and **4** until the cursor round, when `UNOWNED (codegen) + N9-01` did too.

## The five UNOWNED findings, which are the point of the exercise

An inventory cannot see work nobody declared. These five are what a witness finds that a requirement
filter cannot, and the list has turned over twice. Recursive data compiling to C that gcc rejects was
on it when the file was written and is **closed**; see *What is better than the roadmap assumes*.
Item 5 below took its place when the number workaround was re-measured: it had been tagged N4-02,
which is `satisfied`, so the tag was wrong, and the thing actually missing there is owned by nobody.
Item 4 arrived the same way and is the largest of the five by site count: four rows in the gap list were
filed under `satisfied` N5/N6 rows, all four of those rows measure as satisfied, and every one of
them was hiding the same missing thing. Three of the five — items 1, 2 and 4 — are closed as
capabilities now and applied in the witness, and not one of the three was closed as a row until
**2026-11-08, when all five got rows** on the owner's decision of issue #46: N4-24, N9-10 and N6-12,
born `satisfied`, for items 1, 2 and 4, and N4-25 and N14-18, born `owed`, for items 3 and 5. The
section keeps its name because the findings, not the rows, are the point: they are what a
requirement filter cannot see until someone writes the program.

1. **`[EnumType; N]` is not constructible, and says so wrongly. — CLOSED as a capability by W2a,
   APPLIED here, and OWNED by N4-24 since 2026-11-08.** As written it read: `Type mismatch: expected [K; 4],
   found [K; 4]` — the same type on both sides, in both the repeat and the literal spelling. N4-09
   is `satisfied` and is careful to say it is an instance claim over `[i64; N]` and `[String; N]`;
   the general case is therefore owned by nobody, and it was the reason this parser stored kinds as
   integers instead of as the enum it declared in a comment.
   **The capability landed and the node-kind round APPLIED it.** W2a (`af5d6ac`, on `main` at
   `fafe870`) found the cause in the type checker: a composite type's fallback conversion read a
   bare `K` as a struct, while every expression producing a `K` was an enum, and both print as `K`.
   Re-probed at `fafe870` with `pdc run`, both spellings run. The witness stores its kinds as
   `[Kind; 192]` now, the marker is a discharge record, the live count fell four -> three and the
   UNOWNED count three -> two, and `tests/witness/json_parser.expected` is byte-identical across the
   change.
   **What did NOT close is the ownership question**, exactly as for item 4. N4-09's words are still
   an instance claim over `[i64; N]` and `[String; N]`, its evidence is still
   `tests/02_types_arrays.pd`, and no row was amended or added to claim an array of a user enum. W2a's
   fixture `tests/regression/enum_in_composites.pd` runs in the corpus and would catch a regression;
   the requirement inventory would not have, because no row named what it witnesses. Whether N4-09 is
   amended or a row is added is the owner's call, not this file's, and it is the same call issue #46
   parks for the char pattern. **The call was a row** (2026-11-08): N4-24 claims a user enum in
   the composite positions that fixture runs — and names the ones it does not claim, a tuple as a
   struct field and a generic TYPE at an enum, each measured — and N4-09's text is unchanged.
2. **A `&T` parameter cannot be forwarded — CLOSED by su2**, and it closed for a reason worth
   recording rather than a fix aimed at it. As written it read: `fn a(p: &P) -> i64 { return b(p); }`
   emits `b((*p))` — a dereference into a pointer parameter — silently, and gcc catches it; recursive
   descent is nothing *but* forwarding, so the entire shared-borrow spelling was unusable in the one
   program shape WT-01 asks for; and **no row owned the `&` spelling that the compiler actually
   implements**, so the defect was invisible to every inventory. su2 went after a different symptom —
   a `&self` receiver that could not link — and found ONE cause under both: the call site decided
   whether to take an address from `param.mutable` alone, while the declaration side had always used
   `param.mutable || matches!(ty, Type::Reference { .. })`. Unifying the two predicates fixed the
   receiver and the forwarding case together. Measured after: that exact program compiles, links,
   runs and prints `4`, and `tests/linker_diagnostics.rs::forwarding_a_shared_reference_compiles_links_and_runs`
   is now its witness — the same program that was the subject of a REFUSAL test there before.
   **That did not make the row owned.** Nothing in `docs/contributing/1.0-requirements.tsv` named
   the `&` spelling until N9-10 was added on 2026-11-08 (issue #46), so the finding's point stood:
   the inventory could not see this, and did not see it being fixed either. **How this one has to be probed is itself a finding.** A probe that
   stops at `pdc compile` reports this case as COMPILING, because the front end approves it and the
   defect is in the C that comes out; only `pdc run` reaches gcc and returns rc=3 with
   `passing 'const struct P' to parameter of incompatible type 'const struct P *'`. An earlier pass
   of this census recorded the wrong verdict for exactly that reason. Every probe behind this file
   is now `pdc run`, and any future probe of a codegen-shaped claim has to be, because the front
   end's approval is not the artefact under test.
   **APPLIED here at `cece734` (2026-10-07), and applying it was a probe of its own.** Every reader
   in the witness is a `&self` method now and forwards the pointer through every level of descent:
   in the witness's own emitted C, `peek` is `return __pd_Json_at(self, self->pos);`, and `(*self)`
   occurs nowhere in it. Writing the parser that way found two borrow-checker defects the fixtures
   had never reached, the same lesson one pass earlier in the pipeline — the checker's approval of
   small programs was not the artefact under test either: a `self.<field>` read was a move, because
   the checker typed `self` as a `Self` with no fields (fixed by W1c, `d159139`/`ad63231`), and a
   projection's move leaked into the next function body (fixed by W1d, merged at `cece734`). Each
   has its own fixture, named in row 6 above. The `&` spelling was declared by no row until N9-10.
3. **`String` has no declared byte-level meaning.** N4-05 is one word and is `satisfied`. In the
   implementation it is a NUL-terminated C `char*`, so `\u0000` is not representable — a
   conformance question for any parser of a format that permits it, decided by an implementation
   detail rather than by a row. **OWNED since 2026-11-08 by N4-25** (issue #46), born `owed`: a
   `String` — a UTF-8 value (N12) — can contain U+0000. That is NUL inside UTF-8, not arbitrary
   bytes. The marker at the site names it now.
4. **A char literal is not a pattern, so no `match` in this parser can dispatch. — CLOSED as a
   capability, and OWNED by N6-12 since 2026-11-08.** The four rows this was filed under are all `satisfied` and
   all always measured so over an `i64` scrutinee: `match c { 1 => 10, _ => 20 }` in expression
   position prints 10 (N5-04), `1 | 2 => …` prints (N6-07), `48..=57 => …` prints (N6-03), and
   integer arms are patterns (N6-02). Changing the scrutinee to `char` used to make every one of
   them `Expected pattern, but found char 'n'`; a JSON parser is a byte dispatcher and nothing
   else, so that one gap forced every `match` in the target shape into an `if` staircase — three
   markers, four gap-list rows.
   **The capability landed and this round APPLIED it.** `parse_pattern_primary` has a
   `Token::Char` arm routed through `maybe_range`, so `'n' => …`, `' ' | '\t' => …` and
   `'0'..='9' => …` are patterns; the char range orders by code point and lowers to two numeric
   comparisons. All three markers in the witness are discharged, the live count fell nine -> six
   and the UNOWNED count seven -> four, and `tests/witness/json_parser.expected` is byte-identical
   across the change — which is what makes it a discharge rather than a rewrite.
   **What did NOT close is the ownership question.** N6-02's own words are still "Literal patterns
   (integer, string, bool)", no row was amended to claim `char`, and no new row was added, so the
   construct the witness now depends on is declared by nothing in
   `docs/contributing/1.0-requirements.tsv` — exactly the condition that made this an UNOWNED
   finding. It was parked as issue #46 for the owner, and the answer (2026-11-08) was a row of its
   own, N6-12, with `tests/06_char_patterns.pd` as its evidence. This is the same shape as N4-09's
   instance claim putting `[EnumType; N]` outside item 1: the code moved, and the manifest did not
   until it was told to.
   The subset clause that used to record the gap is at
   `docs/specification/grammar.ebnf:478` and now reads "INTEGER OR `char` LITERALS"; the
   `375-379` citation this finding used to carry pointed at the tuple-index passage after the
   file shifted, which is why it is spelled by content here rather than by line.
5. **Nothing moves a float across the text boundary, in either direction.** `f64` exists and
   arithmetic on it compiles, so N4-02 (`f32 f64`, `satisfied`) is not the missing piece. Inward,
   `src/builtins.rs` declares no `string_to_float` and no `parse_float`, so a lexeme this parser
   has already validated cannot become a value. Outward, it declares no `float_to_string` and no
   `print_float` — `print_float(x)` is `Undefined function: 'print_float'. Did you mean
   'print_int'?` — so a value the language can compute cannot be printed. Grepping that file for
   `float` returns nothing at all, and no row owned *either* direction until **N14-18** was added on
   2026-11-08 (issue #46), born `owed`. Paying it amends N14-01's closed count of 34 builtins in the
   same change, because both directions are builtins the closed set does not hold.

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
  containment edge, and `src/codegen/mod.rs:1869-1871` lowers `Type::Generic` to `"void*"` under a
  `TODO`. `enum J { Num(i64), Arr(Vec<J>) }` therefore reaches exit 0 without the layout rule ever
  seeing the cycle. The rule is measured for `Custom`, `Array` and `Tuple` payloads only.
- **Struct-of-arrays state threads through recursive descent correctly.** A `&mut self` receiver
  lowers to `struct Json*` and a `&self` one to `const struct Json*`; mutations propagate to the
  caller, a receiver forwards to the next method as the same pointer, mutual recursion links (D8's
  prototypes) — `value` calls `array`, which calls `value` — and methods call each other across the
  seven separate `impl Json` blocks: `value` calls `string_body` and `alloc`, each declared in the
  block of its own section. Until the cursor round this was measured with `mut j: Json`, which
  lowers to the same `struct Json*` and is still what the transcript driver's `show_ok`/`show_err`
  take. The arena workaround is *ugly*, not *fragile*.
- **`[String; N]` fields work**, including assigning a run-time-built owned string into a slot —
  which is what makes decoded string values storable at all.
- **Enums with payloads, `match` on them, and exhaustiveness all work.** `match e { E::A => … }`
  missing an arm is `Non-exhaustive match: missing patterns <the ones missing>` — the particulars
  moved onto the primary header in GI-12 su1, where the duplicate second `error:` line that used to
  carry them was deleted. N6-10 says "for EVERY scrutinee type"; for
  the *enum* scrutinee it is already true today, so N6-10's residue is integers and strings, which
  is downstream of N6-02 rather than independent work.
- **Diagnostics carry byte offsets and the parser's own error positions are exact** — all fifteen
  refusal transcript lines name the right offset. Fifteen is a measured number, not a rounded one:
  `grep -cE '^[[:space:]]*show_err\(j,' tests/witness/json_parser.pd` is 15 — the call sites in
  `main`, anchored for the same reason the two greps below are, so that a comment quoting the
  pattern cannot inflate it — matching the fifteen lines after `-- refused --` in the pinned
  transcript `tests/witness/json_parser.expected`.

## What the method rewrite made visible: N7-07

This file used to list, under *better than the roadmap assumes*, that effect inference read this
program correctly with no annotation — `js_render` as `[Memory]`, `show_ok` as `[IO, Memory]`,
propagated transitively through eleven functions. That was true of the free-function parser and is
not true of the method one, and the difference is a requirement row: **N7-07 (M5, `owed`), "Methods
in impl blocks are effect-analysed."** Measured at `cece734` with `pdc compile` on the same compiler,
before and after the rewrite. Before, thirteen lines:

```
   Function 'js_utf8' has effects: [Memory]
   Function 'js_unicode_escape' has effects: [Memory]
   Function 'js_string_body' has effects: [Memory]
   Function 'js_literal' has effects: [Memory]
   Function 'js_number' has effects: [Memory]
   Function 'js_object' has effects: [Memory]
   Function 'js_value' has effects: [Memory]
   Function 'js_parse' has effects: [Memory]
   Function 'js_escape_into' has effects: [Memory]
   Function 'js_render' has effects: [Memory]
   Function 'show_ok' has effects: [IO, Memory]
   Function 'show_err' has effects: [IO, Memory]
   Function 'main' has effects: [IO, Memory]
```

After, five:

```
   Function 'js_utf8' has effects: [Memory]
   Function 'js_escape_into' has effects: [Memory]
   Function 'show_ok' has effects: [IO, Memory]
   Function 'show_err' has effects: [IO, Memory]
   Function 'main' has effects: [IO, Memory]
```

The eight that vanished are methods now. `src/driver/mod.rs:173-174` hands the analyser
`Item::Function` items only, so nothing in an `impl` block is analysed at all; and
`src/effects/mod.rs:312-319` adds a callee's effects only when the callee is an `Ident`, so a
method call — whose callee is a field access — contributes nothing, and the unknown callee is
assumed pure, which is N7-06's subject (also `owed`). `show_ok` still says `[IO, Memory]` only
because it calls `print` and `string_concat` itself; through `j.render(root)` it would get nothing.
The compile still succeeds and the transcript did not move by a byte. Nothing in the witness works
around this — it is written the way it should be — so it carries no marker; it is the
compiler's gap, owned by N7-07 and owed by M5, and this witness is now where it shows.

## The recommended manifest change, and its evidence

**`WT-01` stays `owed`.** The row's text in `docs/contributing/1.0-requirements.tsv` started as the
`-` line below and was later made to carry the counts; the `+` line is the row as it stands, its
counts re-truthed in the same change as this file at every census move — the cursor, node-kind
and ownership rounds included (the ownership round also moved it to M8):

```
-WT-01	M2	N1	Witness 2 exists: a JSON parser written with no workarounds, in the corpus	fixture	tests/witness/json_parser.pd
+WT-01	M8	N1	Witness 2 exists: a JSON parser written with no workarounds, in the corpus. THE FIXTURE EXISTS AND RUNS (tests/witness/json_parser.pd, class=run, transcript pinned); the row is owed on the words NO WORKAROUNDS, and the count is derivable rather than asserted: `grep -cE '^[[:space:]]*// WORKAROUND ' tests/witness/json_parser.pd` is 7, of which 3 are live workarounds and 4 are discharge records, and `grep -cE '^[[:space:]]*// WORKAROUND UNOWNED' tests/witness/json_parser.pd` is 0. Three distinct workarounds, one per live comment, and every one is owned: the arena by N14-09 + N4-12 + N4-15, the NUL byte by N4-25 and the float<->text boundary by N14-18. The constructs the discharged workarounds now lean on are owned too: char patterns by N6-12, the `&` parameter spelling by N9-10, an array of a user enum by N4-24. All five of those rows were added 2026-11-08 from issue #46, when they were UNOWNED findings of this witness. MOVED M2 -> M8 ON 2026-11-08 (see the header): none of the three live workarounds is M2's to pay, and the move did not pay any of them. See tests/witness/json_parser.no-workarounds.md	fixture	tests/witness/json_parser.pd
```

Evidence for leaving it `owed`, as two commands rather than as this paragraph:
`grep -cE '^[[:space:]]*// WORKAROUND ' tests/witness/json_parser.pd` is **7** (the anchor excludes
the self-references in the file's own header; 3 live plus four `WORKAROUND DISCHARGED` records)
and `grep -cE '^[[:space:]]*// WORKAROUND UNOWNED' tests/witness/json_parser.pd` is **0** (it was 2
until the ownership round below). The second
is anchored too, and deliberately: the unanchored `grep -c 'WORKAROUND UNOWNED'` also counts prose
that merely quotes the pattern, so it can be inflated by editing a comment. Both figures in the row
text and in the conformance-manifest note are therefore checkable by command rather than by reading
this file.

**Both numbers fell in the char-pattern round, and they fell together.** The live count went
13 → 9 when N5-06, N5-07, N5-12 and N5-13 landed and were applied, and 9 → 6 when char patterns
landed and were applied here; the UNOWNED count rose 4 → 7 over the first of those edits — three
markers naming `satisfied` N5/N6 rows turned out to be one unowned gap wearing three tags — and
fell 7 → 4 over the second, when that one gap closed.

**Both fell again in the cursor round, at `cece734`.** The live count went 6 → 4 and the UNOWNED
count 4 → 3 when the cursor became methods: the `UNOWNED (codegen) + N9-01` marker and the
`UNAPPLIED` marker each became a `DISCHARGED (was …)` record, which is also why the anchored total
stayed 7. Exactly one of the four workarounds that round left was owned outright.

**And again in the node-kind round, at `fafe870`.** The live count went 4 → 3 and the UNOWNED count
3 → 2 when the kinds became an enum array: the bare `UNOWNED` marker at the node kinds became a
`DISCHARGED (was UNOWNED)` record, which is again why the anchored total stayed 7. **Exactly one of
the three surviving workarounds — the arena, `N14-09 + N4-12 + N4-15` — was owned outright then; the
other two were UNOWNED until the ownership round below.** A roadmap filter that drives M2 to zero-owed therefore moves this witness by
nothing at all from here, which is what the row is for — and note that three capabilities it now
leans on were owned by no row either: char patterns (issue #46), the `&self`/`&T` spelling (finding
2), and an array of a user enum (finding 1).

**And to zero in the ownership round, 2026-11-08, with no code change.** The UNOWNED count went
2 → 0 and the live count stayed 3 when issue #46 was decided: the two bare `UNOWNED` markers name
N4-25 and N14-18 now, and the three capabilities above are N4-24, N9-10 and N6-12. **All three
surviving workarounds are owned, and none by M2**: the arena by `N14-09 + N4-12 + N4-15`, where
N14-09 is M8's, and the other two by the rows just added. So WT-01 moved from M2 to M8 in
the same change, and it is still `owed`.

A second recommendation was offered rather than made, and the owner ADOPTED it on 2026-11-08
(issue #46) — it is kept as written: the five UNOWNED findings want **five new rows**, because the M2 filter can reach zero-owed while nothing declares what this witness either
lacks or depends on. Three of the five closed as capabilities and are applied here — the
`[EnumType; N]` array (finding 1), the `&T` forward (finding 2) and the char pattern (finding 4) —
with no row behind any of them, so the requirement inventory would not see one of them regress. The
other two — the byte meaning of `String` and the float↔text boundary (findings 3 and 5) — are still
missing, with no row to owe them. It is the same hole item 6 of M2 closed for the builtins by adding N14-17. The
char-pattern row was the cheapest of the five and unblocked the most of this file: one arm in
`parse_pattern_primary` closed four gap-list rows and three markers.
