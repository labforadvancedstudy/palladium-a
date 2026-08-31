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

**Rows 6, 7, 9, 10, 11, 12, 13, 14 and 15 were re-probed against `7eac786`, and that round is the
largest the file has taken.** Four of them — 12 (`else if`), 13 (`loop`), 14 (`+=`) and 15 (bitwise
operators) — are **closed**, and closed the way the census is meant to close a row: the construct
was APPLIED in `json_parser.pd` and the workaround deleted, not re-labelled. `tests/witness/
json_parser.expected` is byte-identical across all four, which is the receipt that a discharge
happened rather than a rewrite. Rows 7, 9, 10 and 11 moved the other way: each named a `satisfied`
row, each of those rows measures as satisfied **over an `i64` scrutinee**, and what actually blocks
this parser is that a char literal is not a pattern. That is one gap under four rows and three
markers, owned by nobody — the fifth UNOWNED finding below. Row 6 moved too: `x.f()` works now, so
the row's diagnostic was stale, and what blocks the cursor is a receiver that can be written through
and called twice.

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
(14). **Four of the nine exist outright** — char literals (8), `match` as an expression (7),
`loop { … }` (13) and compound assignment (14) — **three exist over every scrutinee type except the
one this parser uses** — literal (9), or- (10) and range (11) patterns all dispatch on `i64`,
`String` and `bool`, and refuse a `char` — **and two do not exist at all**, both of them the
receiver: `ref mut self` (5) does not parse and `j.peek()` (6) parses but moves its receiver and
cannot write it. That is the measurement this witness was written to take: `enum`
payloads, `impl` and `match` on enums all work today, so the parser's *skeleton* is expressible;
the arms of the `match` are expressible over the wrong type; and the receiver, which is what makes
recursive descent a method at all, is not expressible.

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
| 6 | `j.peek()` | **the dot is not the blocker any more.** `impl P { fn get(self) -> i64 }` with `p.get()` compiles, links and prints — N5-17 is `satisfied` and measures so, and this row's old diagnostic (`Indirect function calls not yet supported`) is stale. Two other things stop it. A `self` receiver is a MOVE: calling `p.get()` twice is `Use of moved value: p`, and recursive descent calls the cursor thousands of times. And no method can WRITE its receiver: `self.n = self.n + 1;` is `Expected ';' (Expected ';' after expression), but found '='`, and `ref mut self` is `Expected ':' after parameter name, but found 'mut'` | free functions `js_*(j, …)` | N9-02 (M7, `owed`, `ref mut T`) + N10-06 (M3, `owed`, a receiver that parses). **Neither row's words name an inherent-impl `ref mut self`** — N10-06 is about *trait* methods — so the exact spelling is adjacent to owned work rather than owned by it; recorded as a residual and NOT counted as a sixth UNOWNED |
| 7 | `match self.peek() { Some('n') => … }` as an **expression** | **closed, and APPLIED.** `js_value`'s dispatch IS a `match` expression now: `let node: i64 = match c { '\0' => { … } 'n' => { … } … _ => { … } };`. N5-04 was never the blocker and always measured so; row 9 was, and row 9 closed. The local is no longer `mut` and no arm assigns | none. It was `let mut node: i64 = -1;` and one assignment per branch | **closed** — the capability landed in the char-pattern round; N5-04 (M2) `satisfied` throughout |
| 8 | `'n'`, `'"'`, `'\t'` — char literals | **closed.** `let c: i64 = 'a';` compiles (rc=0), and so do `c == 'a'` and `c >= '0' && c <= '9'`. Char literals are expressions now; what still fails is a char literal in *pattern* position, which is row 9 | none. Twenty-nine of the 31 zero-arg byte-code functions are deleted; `ch_backspace` and `ch_formfeed` stay because `'\b'` and `'\f'` are outside the closed escape set and are correctly refused — the set behaving as specified, not a workaround | N2-04 (M2), `satisfied` |
| 9 | literal patterns in the arms | **closed, and APPLIED.** A char literal is a pattern: `'\0'`, `'n'`, `'t'`, `'f'`, `'"'`, `'['`, `'{'` and `'-'` are arms in `js_value`, and `'!'`-style literals are pinned by `tests/06_char_patterns.pd`. Measured before: `'n' => …` was `Expected pattern, but found char 'n'`. The lowering is numeric — `== 110 /* 'n' */` — not a C character constant, pinned by `tests/m2_char_patterns.rs` | none | **closed by capability, ownership still UNOWNED** — N6-02's words remain "Literal patterns (integer, string, bool)" and no row was amended to claim `char`; parked as issue #46 |
| 10 | `Some(' ') \| Some('\t') \| …` — or-patterns | **closed, and APPLIED.** `js_is_ws` is `match c { ' ' \| '\t' \| '\n' \| '\r' => { true } _ => { false } }`, and `js_value` carries the mixed form `'-' \| '0'..='9'`. N6-07 was never the blocker — or-patterns over an `i64` always compiled — row 9 was | none. It was four `==` tests joined by `\|\|` | **closed** — N6-07 (M2), `satisfied` throughout; the char half is issue #46 |
| 11 | `'0'..='9'` — a range pattern | **closed, and APPLIED.** `js_is_digit` is `match c { '0'..='9' => { true } _ => { false } }`. A char range is ordered by CODE POINT and lowers to two numeric comparisons (`>= 48 /* '0' */ && <= 57 /* '9' */`). N6-03 was never the blocker; row 9 was. An empty char range and a mixed-kind range are refused by name, pinned by `tests/reject/char_range_pattern_empty.pd` and `tests/reject/range_pattern_mixed_endpoints.pd` | none. It was `c >= '0' && c <= '9'` | **closed** — N6-03 (M2), `satisfied` throughout; the char half is issue #46 |
| 12 | `else if` | **closed, and APPLIED.** The witness now carries 12 chained arms across 6 chains (`grep -cE '^[[:space:]]*\} else if ' tests/witness/json_parser.pd` is 12; it was 19 across 7 until `js_value`'s chain became a `match`), and **no `else` block in the file opens with an `if` as its first statement** — the shape the workaround had. The deepest chain, `js_value`, went from eight nested `else` blocks with its last branch eight levels deep to one `if`, seven chained arms and one `else` at a single indent | none. `json_parser.expected` is byte-identical across the rewrite | **closed** — N5-06 (M2), `satisfied` |
| 13 | `loop { … }` | **closed, and APPLIED.** `grep -cE '^[[:space:]]*loop \{' tests/witness/json_parser.pd` is 4 and `grep -cE '^[[:space:]]*while true \{'` is 0; the only surviving `while true` in the file is the header sentence describing this change | none | **closed** — N5-07 (M2), `satisfied` |
| 14 | `self.pos += 1` | **the compound assignment is closed and APPLIED** — the witness carries 10 `+=`/`-=` statements where it carried none, `j.pos += 1;` among them. `self.pos` is still not writable, but that is row 6's receiver, not this row's operator | none for the operator | **closed** — N5-13 (M2), `satisfied`. The `self.` half of this row's own spelling is row 6 |
| 15 | `(v << 4) \| d`, `cp >> 6`, `cp & 63` | **closed, and APPLIED at both sites.** The hex accumulator is `v = (v << 4) \| d;` and the UTF-8 encoder is written in `>>` and `&` throughout (`(224 \| (cp >> 12)) as char`, `(128 \| ((cp >> 6) & 63))`). This is the row that used to carry the file's only duplicate marker; both sites are discharged, so no marker string repeats any more | none | **closed** — N5-12 (M2), `satisfied` |
| 16 | `Number(f64)` | **not float arithmetic — the numeric↔text boundary, in both directions.** `let x: f64 = 1.5;`, `x + 2.25` and `y > 3.0` all compile (rc=0), so N4-02 is not the missing piece. Nothing carries a float across the text boundary either way: `src/builtins.rs` declares no `string_to_float` and no `parse_float`, so a parsed lexeme cannot become a value, and no `float_to_string` and no `print_float`, so a computed value cannot be printed — `print_float(x)` is `Undefined function: 'print_float'. Did you mean 'print_int'?` | integer numbers carry a value; non-integers carry only their source lexeme, and an `exact` flag says which | **UNOWNED** (N4-02 is `f32 f64` and is `satisfied`; no row owns either direction of the boundary) |
| 17 | `const CAP: usize = 192;` | **closed.** Top-level `const` and `static` items landed with M2 item 9. The witness now writes `const CAP_NODES: i64 = 192;` and `const MAX_DEPTH: i64 = 24;` and reads them by name; the emitted C is `static const long long CAP_NODES = 192;`. `usize` is still not a type, so the row's own spelling remains unavailable and the witness uses `i64` — which is what N4's OPEN `str`/`usize` question owns, not this row | none. The two zero-arg functions are deleted | **closed** — N3-09 / N3-10 (M2), both `satisfied` |
| 18 | `"\\t"` — a backslash followed by `t` | **closed.** Measured now: `"\\t"` → bytes `92 116`, `"\\n"` → `92 110`, `"\\"` → the single byte `92`, and `"[\"tab\\there\"]"` → exactly the 13 bytes of `["tab\there"]`. The `String::replace` chain this row described is gone — `grep -rn 'replace(' src/lexer/` is empty; escapes are lexed against the closed set `\n \t \r \" \\ \'` and anything outside it is a compile error carrying the offending character (`LexError::UnknownEscape`) | none. `bs()` is deleted and the three renderer sites emit the literal | N2-09 (M2), `satisfied` |
| 19 | a JSON document containing `\u0000` | — | refused with a named reason; `String` is a NUL-terminated `char*` (`__pd_string_from_char` writes `result[1] = '\0'`) so the byte cannot be carried | **UNOWNED** (N4-05 is the single word `String`, and is `satisfied`) |

Rows 1–19 above are 19 wants, **of which rows 1, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17 and 18 are
closed**; the file carries **six** `// WORKAROUND` comments — six distinct, one per comment —
because those twelve rows are closed and carry no marker at all, and rows 2 and 3 share the arena
marker. 19 − 12 − 1 = 6 distinct, and 6 comments. The `match`-to-`if`-staircase marker that rows 7
and 9 used to share is gone with them: char patterns landed, and rows 7, 9, 10 and 11 were one gap
wearing four wants. Rows 7, 9, 10, 11, 12, 13, 14, 15 and 17 are the closures where the CONSTRUCT
ARRIVED; rows 1, 8 and 18 closed when a claim was re-measured against the compiler that was already
there. The file keeps one `// WORKAROUND DISCHARGED` comment at row 17's old site and eight shorter
`// <row> DISCHARGED` notes — at the sites rows 12, 13, 14 and 15 changed (row 15 has two), and at
the three sites the char-pattern round changed — none of which are counted above:
a closure with no trace at the site it changed is how a census stops being checkable.

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

It prints **5** marker strings over the 7 comments:

```
   3 UNOWNED
   1 UNOWNED (codegen) + N9-01
   1 UNAPPLIED (su2 removed the blocker; the rewrite is owed)
   1 N14-09 + N4-12 + N4-15
   1 DISCHARGED (N3-09, N3-10)
```

One of those strings stands for more than one workaround, and does so on purpose. Bare `UNOWNED` is
a word rather than an id, and its three sites are three *different* gaps — rows 4, 16 and 19 —
distinguished by site and not by string. The `UNOWNED (char pattern)` string that used to head this
list is gone: it was the opposite case, three sites over one gap, and the gap closed. Subtract the
`DISCHARGED` line, which is history rather than a workaround: 4 live strings + 2 (the two extra
bare-`UNOWNED` gaps) = **6 distinct workarounds across 6 live comments**, one per comment. No marker
string repeats an id: `grep -cE '^[[:space:]]*// WORKAROUND [^:]* again:' tests/witness/json_parser.pd`
is **0**, where it used to be 1 and named N5-12, whose two sites are both discharged. The UNOWNED
site count of **4** is the second anchored grep below, not this pipeline — bare `UNOWNED` and
`UNOWNED (codegen) + N9-01` match it, and the `UNAPPLIED` marker and the arena marker do not.

## The five UNOWNED findings, which are the point of the exercise

An inventory cannot see work nobody declared. These five are what a witness finds that a requirement
filter cannot, and the list has turned over twice. Recursive data compiling to C that gcc rejects was
on it when the file was written and is **closed**; see *What is better than the roadmap assumes*.
Item 5 below took its place when the number workaround was re-measured: it had been tagged N4-02,
which is `satisfied`, so the tag was wrong, and the thing actually missing there is owned by nobody.
Item 4 arrived the same way and is the largest of the five by site count: four rows in the gap list were
filed under `satisfied` N5/N6 rows, all four of those rows measure as satisfied, and every one of
them was hiding the same missing thing.

1. **`[EnumType; N]` is not constructible, and says so wrongly.** `Type mismatch: expected [K; 4],
   found [K; 4]` — the same type on both sides, in both the repeat and the literal spelling. N4-09
   is `satisfied` and is careful to say it is an instance claim over `[i64; N]` and `[String; N]`;
   the general case is therefore owned by nobody, and it is the reason this parser stores kinds as
   integers instead of as the enum it declares in a comment.
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
   **This does not make the row owned.** Nothing in `docs/contributing/1.0-requirements.tsv` names
   the `&` spelling still, so the finding's point stands: the inventory could not see this, and did
   not see it being fixed either. **How this one has to be probed is itself a finding.** A probe that
   stops at `pdc compile` reports this case as COMPILING, because the front end approves it and the
   defect is in the C that comes out; only `pdc run` reaches gcc and returns rc=3 with
   `passing 'const struct P' to parameter of incompatible type 'const struct P *'`. An earlier pass
   of this census recorded the wrong verdict for exactly that reason. Every probe behind this file
   is now `pdc run`, and any future probe of a codegen-shaped claim has to be, because the front
   end's approval is not the artefact under test.
3. **`String` has no declared byte-level meaning.** N4-05 is one word and is `satisfied`. In the
   implementation it is a NUL-terminated C `char*`, so `\u0000` is not representable — a
   conformance question for any parser of a format that permits it, decided today by an
   implementation detail rather than by a row.
4. **A char literal is not a pattern, so no `match` in this parser can dispatch. — CLOSED as a
   capability, STILL UNOWNED as a row.** The four rows this was filed under are all `satisfied` and
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
   finding. It is parked as issue #46 for the owner. This is the same shape as N4-09's instance
   claim putting `[EnumType; N]` outside item 1: the code moved, the manifest did not.
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
+WT-01	M2	N1	Witness 2 exists: a JSON parser written with no workarounds, in the corpus. THE FIXTURE EXISTS AND RUNS (tests/witness/json_parser.pd, class=run, transcript pinned); the row is owed on the words NO WORKAROUNDS, and the count is derivable: 7 `// WORKAROUND` comments of which 6 are live and 1 is a discharge record, 6 distinct workarounds, 4 of them UNOWNED. See tests/witness/json_parser.no-workarounds.md	fixture	tests/witness/json_parser.pd
```

Evidence for leaving it `owed`, as two commands rather than as this paragraph:
`grep -cE '^[[:space:]]*// WORKAROUND ' tests/witness/json_parser.pd` is **7** (the anchor excludes
the self-references in the file's own header; 6 live plus the one `WORKAROUND DISCHARGED` record)
and `grep -cE '^[[:space:]]*// WORKAROUND UNOWNED' tests/witness/json_parser.pd` is **4**. The second
is anchored too, and deliberately: the unanchored `grep -c 'WORKAROUND UNOWNED'` also counts prose
that merely quotes the pattern, so it can be inflated by editing a comment. Both figures in the row
text and in the conformance-manifest note are therefore checkable by command rather than by reading
this file.

**Both numbers fell in the char-pattern round, and they fell together.** The live count went
13 → 9 when N5-06, N5-07, N5-12 and N5-13 landed and were applied, and 9 → 6 when char patterns
landed and were applied here; the UNOWNED count rose 4 → 7 over the first of those edits — three
markers naming `satisfied` N5/N6 rows turned out to be one unowned gap wearing three tags — and
fell 7 → 4 over the second, when that one gap closed. **Exactly one of the six surviving
workarounds — the arena, `N14-09 + N4-12 + N4-15` — is owned outright.** Of the rest, four are
UNOWNED and one is the `UNAPPLIED` marker, adjacent to owed rows whose words do not quite reach it.
A roadmap filter that drives M2 to zero-owed therefore moves this witness by nothing at all from
here, which is what the row is for — and note that the char-pattern capability it now leans on is
owned by no row either, which is issue #46.

A second recommendation, offered rather than made: the five UNOWNED findings want **five new
rows**, because the M2 filter can today reach zero-owed while `fn a(p: &P) -> i64 { return b(p); }`
still emits a dereference into a pointer parameter and no `match` in a byte-dispatching program can
be written at all — the same hole item 6 of M2 closed for the builtins by adding N14-17. The
char-pattern row is the cheapest of the five and the one that unblocks the most of this file:
one arm in `parse_pattern_primary` closes four gap-list rows and three markers.
