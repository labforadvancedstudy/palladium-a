# Milestones

**Updated**: 2026-08-22 · **Released**: v0.3.0 (M1) · **Target**: 1.0.0 = `make thesis-exit`

Ordered by what unblocks what, not by theme. Every milestone exits on one command that covers its
whole goal, and every milestone ships.

## What 1.0 is

**1.0 is `make thesis-exit`: `bootstrap/pdc.pd` rewritten in the differentiated dialect — `ref` /
`ref mut` with inferred regions, a `#[total]` the compiler discharges, inferred effects, and no
`async` or `await` — still reaching a byte-identical stage1/stage2 fixed point, with a second
witness program meeting the same conditions.**

That gate is in the repository now, and **it refuses to answer**. `make thesis-exit` exits 2:
two of its 26 `thesis` rows — GI-11 and GI-12 — are not scored rows at all but
**preconditions on the command's ability to compute a verdict**, and both are outstanding.
It still prints every row's state (1 of 23 evaluated rows would pass), labelled as
information rather than a verdict ([`scripts/thesis-exit.sh`](../../scripts/thesis-exit.sh) →
[`scripts/thesis_exit.py`](../../scripts/thesis_exit.py)). It is committed red on purpose: the
definition of 1.0 has to live here as a command, because prose drifts and commands do not.

It does not read the manifest's *text*. Conditions 2 and 3 are delegated to
`scripts/conformance.sh`, which compiles, links, runs, diffs stdout against a recorded
transcript, checks the declared failure stage, matches the declared diagnostic fingerprint, and
reports `REJECT_ACCEPTED` when a negative test is accepted. A fixture the gate names and the
corpus does not run is reported **DECLARED, ABSENT** — loudly, not as a pass
([F13](#f13-the-first-thesis-gate-was-blind-in-the-way-m1-spent-itself-curing)).

### Why not an inventory

Two earlier definitions were considered and both are rejected: *"Part I has no unmet rows"* and
*"the feature list has no unmet rows"*. They differ in scope and not in kind — both are
**completeness criteria**, and a completeness criterion is the generator of every fiction this
repository has had to retract: `progress: 85%`, "Generics 85% complete" for a feature that emits no
code, "Bootstrap 100% Complete", "Self-Hosting 100%", "v0.6: Self-hosting achieved". Draw the line
on an inventory and the same disease returns under a new name.

The previous draft of this file made that mistake concretely. Its criterion was *"no row in
`feature-index.toml` whose `spec` names a Part I anchor is other than `implemented`"* — and **it
went green with all three differentiators unimplemented**, because N7, N8 and N9 anchor no rows
there precisely *because* they are unimplemented. A health check that passes with the heart removed.

**One artifact in this repository structurally cannot lie.** A conformance fixture can print "not
yet implemented" and PASS — seven of them did, for a year, and defect D5 survived behind one. A
compiler cannot compile *itself* vacuously. `bootstrap/pdc.pd` reaching a byte-identical fixed point
is the only claim here that no amount of prose can fake, so it is what the definition rests on, and
**scope follows from what the compiler actually uses rather than from argument about what belongs in
a release**.

### The four conditions, and why condition 3 decides everything

| | Condition |
|---|---|
| 1 | `make selfhost` green, **and** `bootstrap/pdc.pd` is written in the dialect: no `async`/`await`, no lifetime parameter list, ≥1 `ref`/`ref mut` parameter, ≥1 discharged `#[total]`, and a file-IO function whose inferred effect reaches its caller |
| 2 | one **non-vacuous** conformance fixture with a transcript, per differentiator |
| 3 | per differentiator, a **reject twin** — a `#[total]` whose proof fails, a `ref` whose region is ambiguous, an ungated effect escape: each a compile error, and the region one must *name the ambiguity* |
| 4 | a **second witness program** — the JSON parser — meeting the same three conditions |

### What `make thesis-exit` green would mean today — and what it would not

Stated here, and printed by the command itself on every run, because a green command called
"the definition of Palladium 1.0" that meant less than its name is the worst available outcome.

**Today there is no green available at all.** The command exits 2. Two things it would have to
reason with are disclosed as unsound — the lexical liveness model and the substring rejection
matcher — so it declines to compute a verdict rather than reporting "not reached yet", which
would itself be a measurement. That refusal is decided by introspecting the gate's own wiring,
not by checking whether some artifact exists: four rounds running, a check on a not-yet-existing
artifact degenerated to *"something by that name did not fail"* — an empty `#[test]` satisfied
one level, `@true` satisfied the next.

**When a verdict becomes available, green would mean**: every differentiator's construct exists
in both witnesses; each has a non-vacuous conformance fixture and a reject twin refused at its
declared fingerprint; and `bootstrap/pdc.pd` still reaches a byte-identical fixed point.

**What it would say about liveness and attribution is not fixed here, deliberately.** The
command derives that from the models actually wired at the time, and prints it. A paragraph
fixed in prose would have gone on denying liveness after GI-11 landed — the same half-applied
retraction the banned-phrase lint exists to catch, which I repaired in code last round and left
here. Today those models are the lexical one and the substring matcher, so today there is no
verdict at all; when they are replaced, the command will say what the replacements establish.

Those two gaps are **GI-11** and **GI-12**. They are not scored rows — scoring them was the
defect. **Five** rungs of the same ladder were climbed and each was satisfiable by a replacement
that did nothing: a name exists, a test exists, a test passes, a target exits 0, and finally the
source has the right shape — that last one measured, by renaming the probes to `cg_*` wrappers.
The class is a boundary, not five near misses: *no check that lives inside the artifact can
establish that a future replacement of that artifact is genuine, because whoever writes the
replacement also writes the check.*

What escapes it is a check satisfiable only by **producing different answers on inputs whose
correct answers are already fixed**. GI-11 requires **two things, neither substituting for the other**:
[`tests/liveness-differential.tsv`](../../tests/liveness-differential.tsv) — 20 programs whose
liveness answers are fixed by review, including the real witness's own shape (`compile_file`
inside an `else`, genuinely reachable) and six metamorphic variants under alpha-renaming,
reordering and inert whitespace — which proves the model's **verdicts**; and
[`tests/callgraph-differential.tsv`](../../tests/callgraph-differential.tsv) — **nine** programs
whose **graph outputs** are fixed by review (scoped call-site identities, declared entry roots,
an order-independent fixed point, per-edge completion both ways, indirect targets
resolved-or-declared **per call site**, on a one-site program, a two-site one, and one that
calls the same expression twice) — which
proves the model's **structure**. The acceptance *observable*
cannot carry that: measured, an empty `#[test]` reports `1 passed`, so a body of `{}` satisfied
the entire structural contract.

**Every structural row is fault-injected**, and **provenance is one snapshot per unit, not a
row and not a per-answer tag**: the provider returns `(provenance, {property: value})`, the
runner asks **once per unit**, checks the digest against the unit it submitted, and **projects**
every property from that one object — 18 calls for 9 rows, counted in the self-test — and the count is derived from the corpus,
not written down. Two earlier
shapes were refuted by review: as its own property it proved only that the provider could hash
its input, and as a tag on each answer it still proved only that, while letting `edges`,
`completion` and `indirect` come from **inconsistent snapshots wearing the same digest**.

**What this establishes, exactly — third narrowing, and it is a claim about the runner:** the
**gate** makes one invocation per distinct unit and reads every property from a **copy**, taken at
return, of the single container that invocation returned, labelled with the digest of the bytes the
gate handed over. It does **not** establish that the provider *assembled* that container from one
observation — the self-test's `_bound` builds it by calling its value source once per property, so
its parts may come from different states, and it scores full marks — and it establishes **nothing
about derivation**, for the same reason. The copy buys one small real thing: a provider handing
back a live reference into a changing world cannot make two projections disagree. Both
counterexamples are executable, and both pass.

**The indirect contract, settled — and keyed by site.** GI-11's text carried two clauses that
read as one. They name two situations: an indirect call site answered as a **resolution naming
one or more targets** or as **unresolved** both discharge the obligation, **omitting** the call
site is a scored graph failure, and an answer that **claims resolution and names no target** is a
**harness failure** — the gate cannot tell a resolution from a declination, so it refuses to
score rather than read it as either. The pinned requirement says exactly that, and the corpus
enforces it **per site**, keyed by **position**: `<caller>#<n>=resolved:…|unresolved`, where `n`
is the 1-based index of the site within the caller. A scalar `resolved:a,b` cannot distinguish two
targets for one site from one target for each of two sites — so "every call site" was enforceable
only on a program with exactly one, which was the only program the corpus had — and a key built
from the *callee expression* could not round-trip, because the answer's delimiters are `,` `=` `;`
`|` and there is no escaping. A position has none of them in it. `indirect-repeated-site` calls the
same parameter twice, which is what makes the index an identity rather than a decoration.

**What that does and does not establish, measured rather than argued.** A hardcoded table of all
nine programs *and* all nine mutations scores **full marks**. Run-time metamorphic renaming takes
that same table to **zero** — but an adversary that normalises identifiers, looks up, and
re-applies the suffix is back to full marks. So: **a finite, public corpus cannot defeat a
reader.** Every score is in the generated scoreboard below, beside the adversary that produced
it; none of them is written by hand here. Every one of
those figures is produced by an adversary that runs in the self-test, whose **scoreboard is pinned
label → score** and rejects duplicate labels, and **no score-shaped token may appear in any file
that carries one** — this file, the corpus, the manifest and the gate itself — without a
measurement behind it. The round-15 check read only the gate's residue string, which is how a
quoted **8/9** stood here with nothing running it; the round-16 check added this file but not the
gate's own source, which then quoted a superseded corpus size in four places. The current figures
are *derived* from the row count, so they cannot rot. What these rows establish is that **those specific
wrong-answer and exact-table strategies fail** — *not* that a wrong implementation fails in
general, which the normalising adversary refutes on the same page. Implementation
authenticity and generalisation beyond these nine programs are human-review judgements at the
point GI-11 lands, and the gate says exactly that. That is **one** boundary, not
a list — provenance used to be a second and is now mechanized. The wired lexical model **fails 7 of the 20**, and
answering `live` everywhere, answering `dead` everywhere, and a renamed wrapper around the probe
all fail it too. The corpus is itself closed — id set both ways, full digest over
(id, answer, subject, source) — because it carries the verdict half of GI-11. While either precondition is outstanding **no verdict is computed at all**. Until
then this is a **fail-closed scaffold, not a language certificate** — which is the only form in which a weaker gate is defensible, and it is why the
safeguards had to become preconditions rather than scheduled work.

**`make selfhost`'s fixed point is green under TWO conditions the table did not state.** The
self-hosting unit **imports nothing** (`grep -c '^import' bootstrap/pdc.pd` = 0) **and uses no
generics** (`grep -cE 'fn [a-zA-Z_]+<' bootstrap/pdc.pd` = 0; the subset spec excludes them, and
`bootstrap/pdc.pd:8` states the exclusion as a virtue — *"This file is written in exactly the
subset it implements"*). Both exclusions mattered because there were **two independent unordered
emission sources**: imported modules in a `HashMap` (`src/codegen/mod.rs:182-182`) emitted by iterating
`.values()`, and generic instantiations in `HashMap`s (`src/typeck/mod.rs:1089-1089`,
`src/typeck/mod.rs:1097-1097`) emitted by iterating `.keys()`.

**Both are now ordered, and this paragraph was written before they were.** Every one of the four
sites sorts before it emits: the imported-module walks at `src/codegen/mod.rs:1971-1972` and
`src/typeck/mod.rs:1479-1479`, the two later codegen walks off their own sorted locals
(`src/codegen/mod.rs:2057-2057`, `src/codegen/mod.rs:3016-3016`), and the instantiation keys at
`src/typeck/mod.rs:6085-6086` and `src/typeck/mod.rs:6147-6148`. Pinned by
`tests/m3_imported_calls.rs` — `test_the_whole_emitted_c_is_byte_stable`,
`test_modules_and_generics_together_are_byte_stable`,
`test_imported_definitions_are_emitted_in_a_stable_order` and
`test_generic_instantiations_are_emitted_in_a_stable_order` — the last of which covers the case
neither original experiment did: modules *and* generics together.

What that does NOT retire is stated below and still holds: ordering makes the choice
REPRODUCIBLE, not correct. Which of two same-named templates wins is still arbitrary and still
undiagnosed, and no measurement here establishes that these were the only two unordered sources
— only that these two are closed.

They were separated **by measurement, not inference**: six modules with no generics was
byte-identical, while **six generics with zero imports produced 30 distinct outputs in 30
compiles** — generics alone break byte-stability with no module in sight — and a two-module
program produced **2 distinct SHA-1s over 8 compiles**. So **today's fixed point is not evidence
that the compiler is deterministic; it is evidence that PBS-1 avoids both sources.** SH-01 now
states both exclusions and **SH-05** carries both obligations, worded so that repairing one
iteration retires neither: the thesis is that byte identity survives the **rewrite**, and the
differentiated dialect uses imports and generics both. SH-01 is also the **only** thesis row
asserting a result rather than an obligation, which is why it was the one carrying unstated
conditions.

**When this branch meets the receipt gates on `main`** (`make gate-receipts`), two things are
decided and not open: the `gates:` target keeps `check-retracted-claims` in the union, and
**`make thesis-exit` is never exposed as a `gate:` receipt.** It exits 2, a generic receipt runner
reads nonzero as FAIL, and the one resolution that must not happen is an exemption that turns
`NO_VERDICT` into PASS. If the thesis needs exposure before M9 it goes in as a **typed status
receipt** recording `THESIS_RESULT 2 NO_VERDICT` — the exit code carries three distinct facts and
a boolean gate can hold only two of them.

<!-- ADVERSARY-SCOREBOARD:BEGIN — generated by `make test-thesis-runner`; regenerate with `scripts/thesis_exit.py --update-scoreboard`. Do not edit. -->

| adversary | score |
|---|---|
| a constant graph | 0/9 |
| a container assembled per property | 9/9 |
| a correct graph carrying another unit's provenance | 0/9 |
| a correct graph returned without provenance | 0/9 |
| a provider that corrupts what it returned | 9/9 |
| a silent graph | 0/9 |
| a snapshot delivered as a read-only mapping | 9/9 |
| a snapshot that is not a property map | 0/9 |
| a snapshot whose values are mutable | 0/9 |
| an exact-source table, seed pinned | 9/9 |
| an identifier-normalising adversary | 9/9 |
| right on the original, silent on every mutation | 0/9 |
| right on the original, wrong on every mutation | 0/9 |
| the expectation oracle | 9/9 |
| the same table under a fresh seed | 0/9 |
| wrong on exactly one mutation | 8/9 |

<!-- ADVERSARY-SCOREBOARD:END -->

**What the gate asserts, and what it deliberately does not.** After three reachability heuristics
each shipped a fail-open path, the differentiator probes stopped guessing: a green TH-03/TH-04/TH-05
means the construct **exists in the source**, and a RED may additionally mean it sits in a function
**nothing in the unit names** — sound in that direction. **Liveness is no longer asserted.** That
obligation is **GI-11**, which is now a *thesis* row, so `make thesis-exit` cannot go green while
this lexical model is in use. Likewise **GI-12** for attributable rejections. Both were ordinary
`1.0` rows and were therefore not preconditions of the gate they safeguard; that is fixed.

**Condition 3 is load-bearing and must never be dropped. For an inference feature, the rejection is
the product.** A region inferencer that accepts everything is a no-op, and a no-op is
indistinguishable from a working one if you only look at green fixtures — which is exactly what
`tests/07_traits_basic.pd` did for a year. Condition 4 exists so that one program's accidental shape
does not become the definition of the language.

### What the requirement manifest is now for

[`1.0-requirements.tsv`](1.0-requirements.tsv) — **197 rows, 72 satisfied · 117 owed · 8 blocked**
— stays, and it is still closed, still reconciled against both debt inventories. Its role changed:
**it enumerates, it does not gate.** Every row carries a `disposition`:

*(This line and the status table below said "192 rows, 31 satisfied" and were wrong on `main`:
the file held 193 rows and 32 `satisfied` at `acda322`, so the count had already drifted by one
before M2 changed anything. Every figure here that is a COUNT OVER THAT FILE is now derived and
GATED — `python3 scripts/requirements.py --check-ledger`, which `make test-requirements-runner`
runs: this line, and the three disposition counts below. A wrong number is red, and so is
rewriting the sentence it sits in, because a rewritten sentence is exactly when the number inside
it stops being checked.*

*The rows of the status table that are RECEIPTS OF OTHER GATES — how many Rust tests passed, how
many self-test cases ran, what conformance verified — are **not** covered by it and cannot be:
they are not derivable from any tracked file. They are re-measured by hand, they drift, and they
had — each stated figure against what its command actually printed: `620 pass` against `764`,
`42 ignored` against `55`, `41 xfail` against `54`, `243 citations` against `404`, and `256`
thesis-gate cases against `291`. Treat every one of them as stale until the named
command has been re-run.)*

| Disposition | Count | Meaning |
|---|---|---|
| `thesis` | 26 = 23 scored + `D1-01` (the aggregate) + GI-11 and GI-12 (preconditions) | `make thesis-exit` reads it directly. These rows *are* the definition, and the id set is pinned in the gate: adding, removing or retyping one is a harness error |
| `1.0` | 165 | the witnesses exercise it, or a `thesis` row rests on it |
| `post-1.0` | 6 | enumerated and **explicitly deferred**, owner `P1` |

Nothing is dropped silently. A requirement the thesis does not exercise is marked `post-1.0` in
writing, with an owner — because an omission nobody wrote down becomes the next generation's
fiction, which is the same failure as a percentage nobody measured.

The three differentiators, which the thesis proves:

| # | Differentiator | Normative | Definition | Rows |
|---|---|---|---|---|
| 1 | Asynchrony is an effect — no `async`, no await operator, no colouring | [N7](../specification/language-spec.md#n7-effects-and-asynchrony) | [`async-as-effect.md`](../reference/features/async-system/async-as-effect.md) | N7-01…N7-19 |
| 2 | Termination is provable — `#![total(strict)]`, `#[decreases(expr)]` | [N8](../specification/language-spec.md#n8-totality) | [`totality-checking.md`](../reference/features/advanced/totality-checking.md) | N8-01…N8-12 |
| 3 | Lifetimes are inferred — `ref` / `ref mut`, no `'a` | [N9](../specification/language-spec.md#n9-references-and-lifetimes) | [`implicit-lifetimes.md`](../reference/features/core-language/implicit-lifetimes.md) | N9-01…N9-09 |

## What actually blocks what

Four capabilities, and the order they impose. Two corrections to the previous draft of this section
are marked, because both changed the plan.

| | Capability | What it is | Required by | Waits on |
|---|---|---|---|---|
| **C0** | Abstraction | Traits, generics, bounds, `where` clauses. Trait/generic/module conformance is **zero** today | the effect system's *signatures* · a bootstrap compiler that can grow · the standard library | the surface |
| **C1** | Reference typing | `Type::Reference` is a distinct type carrying mutability. Today it is mapped to its inner type, so `&i64` and `i64` are the same type (`src/typeck/mod.rs:714-718`) | N9 in full · N12's move semantics and drop glue · moving the array rule out of codegen ([A9.2](../specification/language-spec.md#a92-array-parameters)) · C4 · **soundness** of C0's borrows | nothing |
| **C2** | Call-graph fixed point | Per-function summaries propagated to a fixed point, unknown callees not assumed pure, `impl` methods included. Today a single source-order pass whose fallback is "conservatively assume it's pure" (`src/effects/mod.rs:315-319`) | N7's inference and gating · N8's propagation of totality to callees, the same shape | C0, for signatures to carry effects |
| **C3** | Inductive pattern support | Patterns rich enough for structural recursion to have subterms. Enums, construction and `match` already work ([A4.3](../specification/language-spec.md#a43-enums)); literal, range, or-, tuple and guard forms are missing | N6 in full · N8's automatic structural termination | the parser |
| **C4** | Alias-sensitive scheduling | Deciding two effectful operations are independent, which is an aliasing question | N7's parallel-by-default and structured concurrency only | C1, and decision **D2** |

**Correction 1 — C1 is not a prerequisite for traits and generics, and the previous draft said it
was.** It listed "N10's `self` receivers" among C1's dependants. Measured: `impl` blocks with `self`
and `&self` receivers already compile, because `&T` is erased to `T`
([A4.5](../specification/language-spec.md#a45-impl-blocks)). So C0 does not wait for C1 to *exist*;
it waits for C1 to be *sound about borrowing*, which is a different and later claim. **The reorder
wins over the previous graph, and the previous graph was wrong on this edge.**

**Correction 2 — effects can ship before abstraction, and should not.** The previous draft said
basic effect gating needs neither C1 nor C0. That is true as a statement about *capability*: the
builtin registry already classifies every builtin (`src/builtins.rs:192`), the analyser already
unions effects, and effect polymorphism has no instance today because there are no function types
and no closures ([A5](../specification/language-spec.md#a5-types)). It is false as a statement about
*sequencing*, for two reasons the previous graph did not model:

- **The effect system is a typing judgment.** The moment `fn f<T: Display>(x: T)` exists, "what is
  the effect of `T::fmt`" must be answered. Ship effects against signature machinery that cannot
  carry bounds and you design the effect system twice.
- **The thesis gate makes it decisive.** 1.0 requires `bootstrap/pdc.pd` *rewritten* in the dialect.
  It is 991 lines today because it cannot abstract; you cannot grow a self-hosting compiler to cover
  the language without generics. Under an inventory definition, cheapest-first was defensible. Under
  a thesis definition, **the compiler has to be able to grow first**.

Both statements are true about different questions, and the sequencing one governs.

| Milestone | Version | What it is | What it waits on, and why it moved |
|---|---|---|---|
| M1 ✅ | v0.3.0 | The compiler stops lying | — |
| M2 | v0.4.0 | The surface, and M1's unpaid debt | M1. Unchanged: everything is written in this surface, and M1's debt is a live miscompile |
| M3 | v0.5.0 | **Traits and generics** | M2 → **C0**. *Moved from 6th to 2nd*: the effect system needs signatures that carry bounds, and the thesis needs a compiler that can grow |
| M4 | v0.6.0 | **Modules** | C0. *Split out of the old M6*: the bootstrap compiler and the library both become multi-file here |
| M5 | v0.7.0 | **Effects, static half** · differentiator 1 | C0, C2. *Moved later by two*: not because it cannot be built earlier, but because building it earlier means building it twice |
| M6 | v0.8.0 | **Totality** · differentiator 2 | C2, C3, C0. *Moved later, and un-split*: with generics already present, generic structural recursion (N8-07) lands here too instead of waiting |
| M7 | v0.9.0 | **Reference typing and region inference** · differentiator 3 | **C1**. *Moved later*: it is the deepest single capability, and nothing before it needs it |
| M8 | v0.10.0 | The standard library, and C FFI | M3, M4, M7 |
| M9 | **1.0.0** | **The thesis** — the bootstrap compiler in the dialect | everything. `make thesis-exit` green |
| P1 | post-1.0 | Parallel by default, structured concurrency | C4, **decision D2** |

**Versioning.** M1 shipped as `v0.3.0`; every milestone ships one release; every `0.x` is a
prerelease. **M9 ships as 1.0.0** rather than as a further `0.x`, because M9's exit criterion *is*
the definition of 1.0. v0.10.0 is therefore the last prerelease.

**Self-hosting is a floor throughout.** `make selfhost` stays green at every commit, and PBS-1 grows
with each milestone whose constructs the bootstrap compiler must consume. **`src/` is not retired.**
An earlier draft's M9 said "retire `src/` as the primary compiler"; that is demoted to **parity**.
The thesis is that the *bootstrap* compiler reaches a fixed point in the dialect, which does not
require the Rust compiler to go away — and retiring it early would mean implementing region
inference, an effect fixed point and a totality checker in a Palladium that does not yet have them.
Retirement is a post-1.0 decision.

## Where the project actually is

Measured at this revision; every row names the command that produced it.

| | | Command |
|---|---|---|
| **The thesis** | **exit 2 — no verdict available**; 1 of 23 evaluated rows would pass | `make thesis-exit` |
| Self-hosting | fixed point over PBS-1 — stage1 and stage2 C byte-identical (`9b0cf24e…`) | `make selfhost` |
| Conformance | `verified=81 untranscribed=0 vacuous=6 xfail=1 reject=92 skip=2 failures=0` over 182 (re-measured on the merged tree: `main` added 16 rows, 14 of them `reject`; `fix/d3b-tail-if` added 3 more and turned the D3b defect fixture into a verified one; `fix/m2-async-producer` added `tests/reject/async_producer.pd`, the N7-18 repro; `fix/m2-lexical` added 8 — three `run` fixtures for the N2 literals and escapes, five `reject`s for the unknown attribute, its two other shapes, an unknown escape and an unterminated comment; `feat/m2-witness-json` added one `run` row, `tests/witness/json_parser.pd`; `feat/m2-expressions` added 11 `run` fixtures, one per N5 row it closed, and TRANSITIONED `tests/reject/loop_keyword.pd` from `reject` to `run` — that fixture asserted the absence of `loop`, N5-07 removed the absence, and a reject row whose refusal stops happening is REJECT_ACCEPTED rather than a row to delete, which is why `reject` fell by one while `verified` rose by twelve; then `feat/m2-items` added FIFTEEN across two commits — three `run` fixtures (top-level `const`, top-level `static`, the macro system) plus the `02_types_enums` vacuous->run transition, and twelve `reject`s: five for the const/static rules, `missing_return.pd` for N3-03, and six for the macro system, every one of the six replacing either a SILENT wrong expansion or a diagnostic that named a compiler phase; then `1f64c32` added three
`run` fixtures for the review-round repairs and SEVEN `reject`s for the refusals those repairs
introduced, which is the shape a fix round leaves — the negative rows outnumber the positive ones
because most of what a review finds is a program that should never have been accepted; then review
round 2 added one `run` fixture — `tests/04_macro_in_value_position.pd`, a macro expanded inside an
`if`/`loop` used for its value — and two `reject`s, a generic method called through its path form
and a generic enum's constructor, both of which the front end used to accept and hand to a C
compiler that had no such function and no such type to link; then issue #41's five commits added
EIGHTEEN rows — seven `run` fixtures, one per pattern form and one for tuples as values, and eleven
`reject`s, which is again the shape a feature round leaves once the refusals are written down:
one-element tuples on both sides, a chained `p.0.1`, a tuple in an enum payload, an or-pattern that
binds, three range-pattern defects, a bool match missing `false`, a literal pattern of the wrong
type, and the non-exhaustive integer match N6-10 now refuses; then the round-3 review of
`feat/m2-items` added TWO `reject`s, both of them `<<` branches the count-range fixture beside
them never covered — `1 << 63`, whose shift AMOUNT is legal and whose VALUE is not, and
`(0 - 1) << 3`, a negative left operand C leaves undefined however small the result, so
reverting either guard alone now fails a fixture of its own; then `feat/m2-types-semantics` added four — one `run` fixture, `tests/02_types_nested_arrays.pd` (N4-10), and three `reject`s: `for_over_nested_array.pd` and the two inner-length declarator positions, parameter and struct field) | `make conformance` |
| Conformance gate itself | 133 cases, each pinning a way it must still go RED | `make test-conformance-runner` |
| Thesis gate itself | 292 unique cases, **checked** and digest-pinned; 67 drive `main()` end to end and 225 exercise a helper directly — the decomposition the gate itself prints, replacing a `70 / 16 / 14` split that no longer appeared in its output and that nothing could re-derive. An adversary wrong on exactly one mutation scores one short of full marks — measured, by a control that now exists; the round that first quoted that figure had none, which is why `score < total` looked like coverage | `make test-thesis-runner` |
| Documentation | every snippet compiles; 420 citations fingerprinted, 29 no-compile fences pinned | `make check-docs` |
| Rust tests | 930 pass, **0 fail**, 46 ignored (569 lib + 361 integration, 28 binaries) | `make test-honest` |
| Declared failures | 45 `xfail` + 1 `slow`, none passing; 45 of 45 failing for their DECLARED diagnostic | `make test-xfail` |
| `stdlib/` | 0 of 21 files compile; 34 builtins accounted, the registry is exactly N14's normative 34, and no builtin is registered-and-refused (was 6) | `make stdlib-gate` |
| Traits · generics · effects · async · unsafe · modules | conformance coverage is **zero** for each | `make conformance` |
| 1.0 requirements | 72 satisfied · 117 owed · 8 blocked, over 197 rows | [`1.0-requirements.tsv`](1.0-requirements.tsv) |
| `bootstrap/pdc.pd` | 991 lines, and it cannot abstract — which is why M3 moved to the front | `wc -l bootstrap/pdc.pd` |

## The inventories the manifest was derived from

**1. Part I, by section** —
`sed -n '/^| Normative section | Status/,/^$/p' docs/specification/language-spec.md | awk -F'|' 'NR>2{print $3}' | sort | uniq -c`

| Status | Count | Sections |
|---|---|---|
| implemented | 1 | N13 |
| partial | 9 | N1 N2 N3 N4 N5 N6 N11 N12 N14 |
| unimplemented | 4 | **N7 N8 N9**, N10 |

**2. Per feature** — `feature-index.toml`: 48 rows, **4 implemented · 16 partial · 28
unimplemented**. Ten now carry a `milestone` field recording 1.0 scope; the other 38 are
unclassified — see [Scope](#scope-what-is-in-10-and-what-is-not).

**3. Conformance debt** — `make conformance`: one `xfail`
(`tests/projects/hello_pdm/tests/test_math.pd`, cross-file imports, now **M4**) and six `vacuous`
rows: `07_traits_basic`, `08_generics_basic` (**M3**),
`12_modules_imports` (**M4**), `09_effects_system`, `10_async_await`, `11_unsafe_blocks` (**M5**).
`02_types_enums` was the seventh and was M2's only one; item 9 made it real and transitioned the
row, which is the protocol this file states for a vacuous fixture whose feature gets implemented —
rewrite the fixture, then transition the row, and never delete either.
`reject` WAS empty when this inventory was taken ([F3](#f3-the-conformance-corpus-has-no-negative-tests));
it holds 82 rows now, and the sentence is left in the past tense rather than deleted because F3 is
the finding this paragraph is evidence for. Every one of these rows is named by a requirement, and
that direction of the reconciliation is checked.

**4. Declared Rust failures** — `make test-xfail`; owners parsed by `scripts/test-xfail.py:186`:
18 tagged M4 → now **M3**; 14 tagged M2 → **M2**; 5 tagged `unscheduled` → **M5**; 3 tagged M1 →
**M2**; 1 tagged M5 leaves the inventory
([F8](#f8-one-declared-failure-expects-syntax-the-specification-forbids)). That is
[26 re-tags and one reclassification](#f9-the-milestone-labels-in-the-test-suite-were-written-against-the-old-numbering).

**5. Open defects** — [`CLAUDE.md`](../../CLAUDE.md) "남은 결함" and the annex. Ownership is carried
by the requirement manifest; this is the reading list.

| Defect | Where | Requirement |
|---|---|---|
| D3b — a tail `if` is not lowered to a `return`; `fib(10)` prints `8261746944` and exits 0 | [A6.6](../specification/language-spec.md#a66-tail-expressions) | N3-02, N3-03 |
| The async producer — `async fn g() { … }` compiled and emitted a `Future` struct with a `state` field and a `_poll` function, which N7 forbids outright. **CLOSED**: `async fn` is refused at the construct in typeck and again in codegen, and the emitter is deleted; receipts in `tests/m2_async_producer.rs` | [F11](#f11-the-async-producer-was-alive-and-violated-n7--closed) | N7-18 |
| C-keyword identifiers — `fn double` emitted `long long double(…)`. **CLOSED**: escaped on the way into code generation, `src/codegen/c_ident.rs:440`; the `#[ignore]` is gone and the debt row is `paid` | `tests/e2e_test.rs:277` | N3-01 |
| No missing-return diagnostic — `fn f() -> int { }` compiled silently. **CLOSED**: the parser already decided "returns on every path" and now refuses when it does not, `src/parser/mod.rs:1245-1274`; the `#[ignore]` is gone and the debt row is `paid` | `tests/compiler_comprehensive_test.rs:633` | N3-03 |
| Block comments do not nest, which N2 requires | [F10](#f10-block-comments-do-not-nest-and-nothing-said-so) | N2-08 |
| Nested arrays work in neither locals nor parameters | [A5](../specification/language-spec.md#a5-types) | N4-10 |
| Filesystem builtins return `i64`/`bool` rather than `Result`, because `Result` is not built in *(the handle-representation split that made six of them uncallable is closed — M2)* | [A8](../specification/language-spec.md#a8-builtins) | N14-03 |
| `pub` on an enum discarded; `dbg!` undefined; `println!` takes one argument; no hygiene | [A4.6](../specification/language-spec.md#a46-macros) | N3-05, N3-12, N3-13 |
| `Foo<T>` is parsed as a *const* generic argument; const generics are not monomorphised | [A5](../specification/language-spec.md#a5-types) | N10-03, N4-21 |
| Traits emit no C; a trait method with a `self` receiver is a parse error | [A4.4](../specification/language-spec.md#a44-traits) | N10-06, N10-09 |
| `&mut` of an immutable local is accepted for struct referents | [A9.3](../specification/language-spec.md#a93-mut-of-an-immutable-local-is-accepted) | N12-06 |
| `String` is a Copy handle, contradicting N12 — no drop glue | [A9.1](../specification/language-spec.md#a91-string-is-a-copyable-handle-decision-2026-08-21) | N12-03, N12-04 |
| Effects gate nothing; propagation assumes unknown callees pure; `impl` methods unanalysed | [A4.1](../specification/language-spec.md#a41-functions) | N7-03…N7-08 |
| Attributes do not lex — `#[total]` fails at the character `#` | [A2](../specification/language-spec.md#a2-lexical-structure) | N2-10, N2-11 |
| `src/async_runtime/mod.rs` — 498 lines, one referrer (`src/lib.rs:5`), no consumer | [F11](#f11-the-async-producer-was-alive-and-violated-n7--closed) | N7-19, decision **D5** |

## How a milestone exits

**One command per milestone, covering the whole goal — accepted programs as well as refused ones,
runtime observables where the goal is a runtime property, packaging where the goal is a shipped
artifact.** Three lines of Makefile each:

```make
m5-exit: build
	@REQ_MILESTONE=M5 python3 scripts/requirements.py
```

**`scripts/requirements.py` now exists** (it was specified here as `requirements.sh`; it is the
same contract with the parsing in Python, following `thesis-exit.sh` → `thesis_exit.py`) **and
implements steps 1, 2 and 5 of the five below, plus two closures the specification did not name:
the manifest's own MANDATORY-COLUMN rule (every one of the nine, `-` being how a row says N/A) and
a PINNED OWNERSHIP ROSTER, id by id.** The roster is why step 2 is worth anything: a row deleted or
retagged to `-` leaves every milestone filter, and the three declared-failure inventories stay
clean because a requirement nobody started produces no red test. `EXPECTED_THESIS_CONTRACT` in the
thesis gate pins the 26 `thesis` rows and does not pin the milestone column, so all 46 M2 rows were
pinned by nothing. `make m2-exit` runs the reader as inventory four, and its exit code is
three-valued for `thesis-exit`'s reason: 0 CLEAR, 1 OWED, **2 NO_VERDICT**. Steps 3
and 4 are NOT implemented, are named in the output of every run, and are why a milestone whose rows
are all `satisfied` exits 2 rather than 0 — "no row says owed" is a statement about a status
column, and a gate that returned 0 for it would be the M1 defect in a new inventory.
(`make thesis-exit` is the same shape and already exists — note that it both reads the
manifest *and* carries a version-controlled copy of the thesis contract to compare against
it. That duplication is a reviewed cross-check, not a second definition: the pin catches an
edit to the manifest, and the pin's own validator catches a defect in the pin.)

1. Parse [`1.0-requirements.tsv`](1.0-requirements.tsv) — **nine** tab-separated columns, all
   mandatory. A row with a missing column, an unknown evidence kind, status or disposition is a
   failure of the manifest, not of the milestone. The ninth column is the diagnostic
   fingerprint a `reject` row's refusal must carry, and for a `thesis` reject row it may not
   be `-`: any rejection would satisfy that, including one for incidental unsupported syntax.
2. For the milestone named by `REQ_MILESTONE`, **every** row must be `satisfied`.
3. Resolve each evidence locator by kind, and *run* it: `fixture` → a `run` row whose transcript
   matches · `reject` → a `reject` row refused with its declared diagnostic · `skip` → a proven
   non-program · `observable` → a named Rust test that exists, is not `#[ignore]`d, and passes ·
   `gate` → a make target that exits 0 · `decision` → recorded as resolved in
   [Decisions](#decisions-for-the-owner).
4. Reconcile both debt inventories, in both directions. The conformance half is checkable today, by
   path. The Rust half needs a `req: <id>` tag in each `#[ignore]` reason.
5. `make test-requirements-runner` plants a row for the milestone under test and proves the runner
   goes RED for it — **and proves the exit target still reads every inventory, by running it and
   anchoring each one's output to a number recomputed from a tracked file**
   (`tests/conformance-manifest.txt` class counts · `tests/rust-debt-manifest.txt` state counts ·
   those plus the SLOW allowlist for the ignored total · `1.0-requirements.tsv` row counts). What
   that does not establish, and the runner says so: an adversary that reads the same files and
   prints what it finds would pass. Only running distinguishes it, which is what the target does. A filter nobody has watched fail is not a filter — which is why
   `make test-thesis-runner` already exists for the thesis gate and caught a real defect in it
   ([F12](#f12-the-thesis-gates-first-lexer-could-not-fail-on-what-it-checked)).
   **Done (GI-09):** 42 cases in `scripts/test-requirements-runner.sh`, and it grew a second half
   this specification did not ask for. Half one is the filter: planted `owed`, planted `blocked`,
   another milestone's row, a milestone with no rows, an unset and a typo'd `REQ_MILESTONE`, and
   every structural check of step 1. Half two is the **target, observed by its effects**:
   `make m2-exit` is RUN, and each inventory must have PRODUCED something — with every number
   recomputed in the test, independently, from the same source that inventory reads.
   *(It was `make -n m2-exit | grep <command text>` for one round, and that was the `@true` rung
   this repository has already climbed once: a recipe of `@echo 'REQ_MILESTONE=M2 python3
   scripts/requirements.py'` satisfied every assertion in it while reading no inventory at all. The
   repair then got one of the four right and left the other three as token searches — the same
   finding one layer in — so every inventory now carries an anchor recomputed from a tracked file
   it does not share with the test. Re-proved by reverting: replacing any inventory with that echo
   fails its anchor.)*
   The aggregation has its own driver too, because the real tree cannot exercise it: no inventory
   returns NO_VERDICT today, so inverting the lattice changed nothing about a real run. It is
   driven over all 81 four-inventory combinations plus order independence
   (`scripts/m2-exit.sh --self-test`), and inverting it fails 51 of 82.

**Why an aggregate and not an owner filter.** `CONFORMANCE_FORBID_OWNER` clears only *tagged
proxies*: it proves no declared failure still names the milestone. It cannot prove the feature
works, because a feature nobody wrote a red test for produces no tagged proxy to clear. The filters
stay as fast pre-checks; the manifest decides; and **1.0 is decided by neither — it is decided by
`make thesis-exit`.**

---

## Completed

### M1 — The compiler stops lying (v0.3.0, released 2026-08-22)

Every other kind of work was slower while the compiler could accept a program and emit wrong code.
M1 converted silent wrongness into diagnostics, and — the part that outlives it — made the gates
able to fail.

Receipts:

| What | Evidence |
|---|---|
| **D5** `?` and `.await` emitted C referencing a `struct Result` layout and a `poll` member codegen never generates | Both refused at typecheck with the consequence and a workaround; old lowerings deleted. `tests/d5_unimplemented_constructs.rs`, 12 tests. **The `.await` consumer only. The `async fn` producer was still alive and is now closed too — see [F11](#f11-the-async-producer-was-alive-and-violated-n7--closed)** |
| **D4** `for` over an array *parameter* used `sizeof` on a decayed pointer | The bound comes from the declared length; an unresolvable length is a compile error, not a wrong bound. `tests/regression/for_over_array_param.pd` |
| **D9** `&[T; N]` / `&mut [T; N]` parameters rejected in codegen | Lowered; a write that reaches the caller can only come from a spelling that declared it ([A9.2](../specification/language-spec.md#a92-array-parameters)). `examples/practical/simple_sort.pd` runs |
| **D7** an un-annotated `let` was emitted as `long long` regardless of its initializer | Fixed in `04104c5` |
| **D6** was not a defect | Retracted with five re-run probes ([A9.4](../specification/language-spec.md#a94-defect-d6-retracted)) |
| The LLVM backend fabricated rather than lowered at 14 sites, seven of them silently | `--llvm` refuses unconditionally. `tests/d10_llvm_refuses.rs`, 9 tests |
| `stdlib/` had no coverage at all | `make stdlib-gate`: 21 files pinned per file, 38 builtins accounted (34 since M2 removed the four `*_ex` names), generated C checked structurally. The premise was wrong and is recorded as such — **0 of 21 compile** ([`stdlib/STATUS.md`](../../stdlib/STATUS.md)) |
| A green exit code was counted as a correct program | Every `run` fixture is diffed against a recorded transcript; there is no exit-code-only class |
| Seven fixtures proved nothing while counting as coverage | Declared `vacuous`, each naming the feature it fails to cover. Seven of 84 when that was measured; **six of 158** now, `02_types_enums` having been made real by M2 item 9 — the count is on the summary line of every run |
| The gates could not fail | `make test-conformance-runner` (133 cases), `make test-gate-probe` (every evidence producer fault-injected) |
| `tests/*.rs` never ran under `make test-rust` | `make test-honest`, and every remaining failure converted to a declared `xfail` with an owner |

Not paid, and re-owned by M2: three M1 `#[ignore]` rows
([F2](#f2-m1-shipped-three-of-its-own-declared-failures-and-its-exit-command-could-not-see-them)).

---

## M2 — The surface, and M1's unpaid debt (v0.4.0)

**Waits on**: M1. **Delivers**: the surface everything else is written in, **C3**, the attribute
token N8 sits below, and the first witness program.

**Owns 49 requirement rows, 8 of them still owed**, 6 declared `#[ignore]` failures (all tagged
M2), and **no vacuous fixture** — `tests/02_types_enums.pd` was M2's last one and item 9 turned it
into a `run` fixture that constructs and destructures a unit, a tuple and a struct variant.
*(It read "seventeen … (fourteen tagged M2, three
tagged M1)" in DIGITS-FREE WORDS, which is exactly why it went stale unnoticed: the prose-figure
gates scan for `N` and `N of M`, and a number spelled out is invisible to them. Measured with
`awk -F'\t' '$3=="owed"{print $4}' tests/rust-debt-manifest.txt | sort | uniq -c` — M2 6, M4 33,
unscheduled 5, M5 1, and **M1 zero**: the three M1 rows were paid, and the sentence had gone on
counting them. M2 fell 9 -> 6 and `unscheduled` 6 -> 5 across items 3, 9 and the round between
them, each by PAYING a row rather than by retagging one.)* *(It read "45
rows" while GI-06 was `owed`; GI-06, GI-09 and N14-01 are now `satisfied`, and 46 was the count of
rows OWNED, not of rows outstanding — the two were being used interchangeably. That parenthetical
is historical: it records a confusion at the commit that had it, and 46 is not today's number.
Today's is the bolded figure above, re-derived by `scripts/requirements.py` on every run.)*

1. **The M1 debt is PAID, and it was the live miscompile.** A tail `if` was not lowered to a return
   — `fib(10)` printed `8261746944` (N3-02); the missing-return diagnostic landed with it (N3-03),
   as `tests/compiler_comprehensive_test.rs:633` says it must; and C-keyword identifier escaping
   (N3-01) landed with those. All three `#[ignore]`s are gone, their rows in
   `tests/rust-debt-manifest.txt` are `paid`, and `make m1-exit` exits **0**.
2. **The async producer is CLOSED** (N7-18). `async fn g() { print("x"); }` compiled and emitted
   `typedef struct g_Future { int state; }` plus `int g_poll(g_Future *future)`. N7 says the
   language has **no runtime representation** of effects, so it was a live normative violation.
   `async fn` is now refused at the construct — in the type checker and again at the defect in code
   generation, exactly as `?` and `.await` are — and the Future/poll emitter is deleted rather than
   left unreachable ([F11](#f11-the-async-producer-was-alive-and-violated-n7--closed)). It was NOT
   as cheap as this line said: the refusal had to reach three ingresses the repro does not touch
   (imported bodies, monomorphised instantiations, and `monomorphize_function`'s `is_async: false`,
   which made "monomorphized functions are not async" true by erasing it), and it had to leave
   alone the imported declarations that are not part of the emitted program. The keyword itself
   still dies at M5.
3. **Statements and expressions** (N5-03…N5-17). **ELEVEN ROWS LANDED; the item is not closed,
   because top-level `const` and `static` were never N5 rows at all.** `if`, `match`, blocks and
   `loop`-with-a-value-carrying-`break` are expressions (N5-03, N5-04, N5-05, N5-07); `else if` parses
   (N5-06); the operator surface is complete — bitwise `& | ^ ~ << >>`, compound assignment,
   ranges `..`/`..=` as values, `as` casts, and `a * -b` (N5-12…N5-16); and `x.f()` is method call
   syntax (N5-17). Four commits: `66dab38`, `f729cda`, `ef74eba`, `4690ef0`, and a fifth for what
review found. **EXTERNAL REVIEW ROUND 1 — ten verdicts over those four commits — found two
reproduced MISCOMPILES and three holes where the front end approved C the backend could not
build, all repaired in `1f64c32`.** The miscompiles were both the same mistake in two places: the
hoisted statements of a value expression were spliced in front of the whole statement, which is
right for a position that runs once and wrong for a `while` CONDITION (computed once, before the
loop — `while { i < 3 }` never terminated) and for the RIGHT operand of `&&`/`||` (`flag && {
print("leaked"); true }` printed "leaked" with `flag` false). Both are lowered now rather than
refused. The three holes — a bare `break` in a value `loop` leaving its temporary unwritten, a
generic method with no symbol to link, a `mut` parameter on a method taking a pointer where the
call site passes a value — are REFUSALS, each pinned by a `reject` fixture, because a compiler
that hands gcc code it approved has no way to tell the user what went wrong. Two further gaps were
DECLARED rather than closed: the borrow checker does not see method-call signatures, and an
enum-owned method is unreachable by its path form. Each row carries a
   conformance fixture, and `verified` moved 52 → 64.

   *The decisions worth re-deriving rather than re-discovering, each because the obvious
   alternative was measured and refused:*

   - **`>>` is NOT a lexer token.** `Option<Vec<Stmt>>` closes two generic argument lists with two
     adjacent `>`, and a longest-match `>>` would eat both — `stdlib/std/sync.pd`,
     `stdlib/std/net.pd` and `bootstrap/v2_full_compiler/ast.pd:83` all carry that shape. The shift
     operator is recognised in the parser from two `Gt` tokens whose SPANS TOUCH, so `a > > b` is
     not a shift and nested generics still parse. `<<` needs no such care: the two `<` of
     `Vec<Vec<T>>` always have an identifier between them.
   - **Compound assignment DESUGARS (`t op= v` → `t = t op v`) and does not emit C's `+=`.** Not a
     style choice: Palladium's `+` on `String` lowers to a runtime concatenation call, so `s += "b"`
     has no C compound-operator form at all. The cost is stated rather than hidden — the target is
     written twice, so it is evaluated twice, and `a[next()] += 1` calls `next()` twice.
   - **A cast to `bool` is `((x) != 0)`, not a C cast.** Palladium's `bool` is C's `int`, so
     `(int)5` would be `5` — truthy, but not `true`, and `5 as bool as i64` would print 5.
   - **One range struct with an `inclusive` flag**, not two types and not normalisation to
     `start..end+1`: `0..=<i64 max>` would wrap to an empty range with no diagnostic. It is built
     by a function rather than a compound literal, which is a C99 form — "compound-literal-free"
     is the accurate word for the constraint, not "C89": `long long` and `//` are not strict C89
     either, and the rest of the emitted prelude uses both.
   - **`self` was the THIRD break in N5-17, and it was undocumented.** The two known ones were the
     type checker's "Indirect function calls not yet supported" and the parser building every
     `A::b(...)` as an enum constructor — which meant the workaround this specification itself
     recommends, `Type::method(receiver, args)`, had never worked either. The third: `fn area(self)`
     emitted `struct Self self`, a type nothing declares, so gcc refused C the front end had
     approved. `Self` is now resolved by one function
     (`ImplBlock::methods_with_self_resolved`) that both the type checker and code generation call.
   - **`tests/reject/loop_keyword.pd` TRANSITIONED `reject` → `run`.** It asserted the absence of
     `loop`; N5-07 removed the absence. A `reject` row whose refusal stops happening is
     REJECT_ACCEPTED, and the manifest's own rule is that paying a row is a transition and never a
     deletion — deleting the row makes the fixture UNDECLARED, deleting the fixture makes the row
     MISSING, and both are red. It keeps its path so the directory still records that `loop` was
     refused there by name until it wasn't.

   *What this item deliberately does NOT include:* **N13-03** (arguments are evaluated left to
   right) is untouched and stays `owed`. Method calls make the receiver the first argument and
   evaluate it exactly ONCE, but their position among the arguments is C's unspecified evaluation
   order — the same residual every multi-argument call in this compiler already has. Fixing it is
   an evaluation-order obligation over every call, not a method-call one. **N3-09** and **N3-10**
   (top-level `const` and `static`) were listed in this item and belong to N3: they are still
   `owed`, and nothing in these four commits touched them.
4. **Patterns** (N6-02…N6-11) — **DONE: all EIGHT of the N6 rows M2 owns, plus N4-12, which
   turned out to be underneath one of them. Nine rows in total.** (N6-01 and N6-04 were already
   `satisfied` and belong to nobody; N6-06, slice patterns, is M3's and stays owed.) This was **C3**. Five commits: `d3600e4` (literal patterns, guards),
   `c983653` (or-patterns, `@` bindings), `6b3e501` (range patterns), `316e47b` (tuples as values,
   then tuple patterns), `0ba980f` (exhaustiveness for every scrutinee type, and the trap).
   `verified` moved 68 → 75 and `reject` 30 → 41; `make selfhost` held its fixed point at
   `9b0cf24e…` throughout, because `bootstrap/pdc.pd` uses none of this.

   The decisions worth reading, each of which could have gone the other way:

   - **A literal in a pattern is a `PatternLiteral`, not an `Expr`.** Reusing the expression type
     would have put every expression form into pattern position and left each consumer to re-refuse
     `match x { f() => … }` on its own; it would also have cost `Pattern` its `Eq`/`Hash`, which the
     exhaustiveness checker derives, because `Expr` carries an `f64`. **There is no float pattern**
     for the same reason stated forward: equality on `f64` is not the relation a reader assumes a
     pattern means.
   - **An or-pattern whose alternative BINDS is refused, with the binder named in the diagnostic.**
     Rust's rule is that every alternative binds the same names at the same types; this checker
     cannot verify that yet, and the arm is emitted as ONE `||` condition with no per-alternative
     site to assign from. Accepting it would have meant choosing a branch on the reader's behalf.
   - **`name @ pattern` is transparent to exhaustiveness** — it covers exactly what its inner covers.
     Reading it as a bare binder would have made `all @ Circle` a catch-all that swallows every
     later arm as unreachable.
   - **A defect the first commit shipped, found and fixed in the second.** Literal patterns made
     nested positions writable while the condition still stopped at the enum tag, so `P::Num(1)`
     matched every `Num` — a silent wrong answer, exit 0. The repair is one recursion over a
     SUBJECT (the C lvalue under test) shared by the condition and the binding emission, which is
     also what made tuple elements, `@` under a payload and nested ranges fall out for free.
   - **Tuples became values before tuple patterns existed**, because they had to: `N4-12` was
     `owed`, `Expr` had no tuple form, and `Type::Tuple` lowered to `void*` behind a TODO nobody
     could observe. One C struct per SHAPE, mangled from the element C types (stable — no counter,
     no hash seed), with a constructor apiece.
   - **`p.0.1` is refused rather than guessed.** The float rule is `[0-9]+\.[0-9]+`, so `.0.1`
     after an expression is ONE `Float(0.1)` token and the two indices are gone before the parser
     sees them; `p.0.10` and `p.0.1` both round-trip to 0.1, so recovering the second index would
     be a guess with a wrong answer available. `(p.0).1` is the spelling, and it runs.
   - **A tuple in an ENUM PAYLOAD is refused by name.** Tuple structs are emitted after the enum
     definitions because a tuple's element may be an enum; a payload of tuple type needs the
     reverse order, and satisfying both is a dependency sort over generated types. Without the
     refusal the program reached gcc as `unknown type name '__pd_tuple2_long_long_long_long'` —
     our own C failing on the user's behalf.
   - **An empty range is a typo, not dead code.** `5..=1` and `3..3` can never match, so the arm
     they head is dead the moment it is written and the two numbers are almost always transposed.
     Refused at the type checker with both bounds in the message.
   - **The corpus sweep for N6-10 found NOTHING, and is reported as finding nothing.** Making a
     non-exhaustive match an error for every scrutinee type could have broken fixtures written when
     that position was unchecked; it broke none — conformance and the whole Rust suite stayed green.
     The rule's bite is shown by `tests/reject/nonexhaustive_int_match.pd` and by probes, not by a
     manufactured list.
   - **The trap ARMED the linker, through a four-step handoff that was written down years' worth of
     rounds earlier.** `tests/stdlib/DRIVERS.tsv`'s `stdlib_tail_match` row went from
     `known_violation:area_code,sides` to `clean`, `NetA::StillFindsTheOpenMatchDefect` became
     `NetA::Accepts`, the parser's residual NOTE stopped recording one, and `-Werror=return-type`
     went into the shared gcc invocation — in one change, because the handoff's own interlock test
     turned red the moment the first of them moved.
   - **The value-`match` zero-initialiser stays as belt-and-braces, and the measurement that first
     justified it was WRONG.** This paragraph claimed five `-Wuninitialized` diagnostics in
     `tests/06_match_expression`'s C and two in `tests/06_guards`' after deleting ` = 0`. Re-measured
     when a reviewer could not reproduce it: the earlier pass had stripped initialisers off unrelated
     declarations as well, and that is where its warnings came from. Stripping ONLY the value-`match`
     temporaries (16, 9 and 8 of them across three fixtures' C) and recompiling with Apple clang 21
     `-O2 -Wall -Wextra` gives **zero** uninitialized diagnostics. The store is kept against a
     compiler with weaker flow analysis than this box has — GNU gcc is not installed here, so that
     half is a possibility rather than a fact, and the retraction is recorded at the code.

   *What this item deliberately does NOT include:* **N6-06** (slice patterns) is owned by **M3** and
   stays `owed` — slices are not a type this language has yet. **Field shorthand** in a struct-variant
   pattern (`Message::Move { x, y }`) is not implemented; `test_pattern_matching_guards` stays
   `#[ignore]`d with its reason re-declared to that and nothing else, its guards half having been
   paid. **Destructuring `let`** (`let (a, b) = pair;`) is not implemented either — `grammar.ebnf`
   says `let` patterns do not exist — which is what `test_type_aliases_complex` now waits on, after
   its tuple-expression blocker was paid.
5. **Lexical completion** (N2-03…N2-11): float and char literals, escapes, **nesting block
   comments** ([F10](#f10-block-comments-do-not-nest-and-nothing-said-so)), and **the `#` attribute
   token** — the token only. An attribute that lexes and is then ignored would recreate the class M1
   removed, so N2-11 makes an unknown attribute a compile error from the day `#` lexes.
6. **The six builtins that cannot compile** (N14-01, N14-17). **DONE, both halves.**
   The four `*_ex` names are out of `src/builtins.rs` — measured before deleting rather than
   deleted on this paragraph's authority: all four were refused at typecheck (`Built-in
   file_open_ex is registered but not callable`, exit 1) and `grep -rn --include=*.pd` over the
   tree found **zero callers**. `file_flush` and `file_seek` are **re-based and callable**: their C
   wrappers are lowered onto `__pd_file_handles`, the `long long` handle table `file_write` and
   `file_close` already use. `file_seek` takes `whence` 0, 1 or 2 (start, current, end) and returns
   the new absolute position or `-1`, refusing any other `whence` rather than treating it as a
   seek; `file_flush` returns 1 on success and 0 on failure, its siblings' convention. Measured end to end: `1 · 3 · 5 · 10 · -1 · 0` for flush,
   SEEK_SET/CUR/END, a rejected whence and a bad handle.
   The four dead C wrappers are **gone from `src/codegen/mod.rs`** as well, and with them the
   `FileHandle` typedef, the `FileMode` enum and the six `pd_file_*` externs only they used.
   `PRELUDE_TYPE_MISMATCHES` is now **empty** — eleven dimensions to zero, which is one deletion
   (eight of them) and one repair (three), not one achievement; the constant is derived from
   `BUILTINS` × the emitted prelude on every run, so empty is the strongest form of the assertion
   rather than a disabled check. `tests/stdlib/BUILTINS.tsv` has **no `UNUSABLE` rows left**, and
   `file_flush`/`file_seek` are `COVERED` by the first coverage either has ever had.
   **N14-01 is `satisfied`** (the name set is exactly N14's 34, both directions, pinned by
   `src/builtins.rs::test_registry_is_exactly_the_normative_builtin_set`), and **N14-17 was added
   and is `satisfied`** — "every normative builtin is CALLABLE", evidenced by `make stdlib-gate`
   rather than by a fixture, because the claim is universal and one driver spans one family. That row is new because the
   manifest had none: this item declared the re-base and the wrapper removal while no requirement
   row said so, which meant the M2 filter could have reached zero-owed with both builtins still
   uncallable and four dead wrappers still emitted. That is the unowned-requirement hole inventory
   four exists to close, reproduced inside it; the row closes it, and it goes red the moment any
   normative builtin is registered-and-refused again.
   **A FALSE `ReturnMode::Owned`, found by review and fixed here.** Four builtins —
   `string_substring`, `file_read_all`, `file_read_line`, `read_file_to_string` — declared that
   they allocate their result while having **reachable branches returning the literal `""`**,
   which is static storage they did not allocate. The corpus reaches all of them (bad handle, EOF,
   missing file, `start >= end`). This is not a documentation defect:
   `src/ownership/borrow_checker.rs:127` derives its signatures from this table, so the ownership
   model was wrong on those branches. They return `__pd_empty_owned()` now — one byte from the
   same bump pool every other owned string comes from, which is why allocating was the right fix
   and `strdup` was not: it adds **no failure class the other owned returns do not already have**.
   **The guard that should have caught it compared `ret_mode` against `effects` — two fields of
   the same table.** Two matching declarations do not make an implementation true, and this is the
   fourth time that shape has appeared on this branch and the first time it was in the compiler's
   data rather than in an instrument. It now has a control that reads the emitted C
   (`test_no_owned_wrapper_returns_a_string_literal`), a positive case so the scan cannot pass
   by finding nothing (`arg_at` returns a literal ON PURPOSE and declares `BorrowedStatic`), and a
   behavioural gate that drives `BorrowChecker::check_program` on a program taking each formerly
   borrowed branch.
   **That control is NARROWER THAN THE PROPERTY, and is named for what it does.** It pins the four
   historical literal returns so they cannot come back; it does NOT enforce "every `Owned` return
   is allocated". An `Owned` wrapper returning a parameter or a static buffer would be the same
   defect and **nothing in this repository would detect it**. Widening it was measured and
   declined: provenance is decidable inside the emitted C for six of the seven, and not for
   `read_file_to_string`, whose `out_str` is filled across the FFI boundary from `Box::into_raw`
   (`src/runtime/io.rs:470`). A checker would need a hand-maintained table of which runtime
   functions allocate through out-parameters — a third registry beside this one and
   `PRELUDE_TYPE_MISMATCHES`, and a table agreeing with a declaration is the shape that produced
   the original defect.
   **What is still owed, and it is M3's**: N14-03, signatures — the filesystem family returns
   `i64`/`bool` rather than `Result`, because `Result` is not built in.
   *(This item cited `N14-04` as well. `N14-04` is `string_char_at returns char`, which needs the
   `char` type and belongs to item 4; nothing about the six builtins bears on it. Corrected rather
   than quietly dropped.)*
7. **Witness 1** (WT-01): a JSON parser written with no workarounds, added to the corpus. It becomes
   the thesis gate's second witness at M9.
8. **Gate integrity** (GI-06, GI-08, GI-09). **GI-06 and GI-09 CLOSED; GI-08 STILL OWED, and the
   residual is stated below rather than glossed.**
   GI-06: `make gates` (`Makefile:553`) runs `test-honest` (`Makefile:385-390`), so a non-ignored
   compiler regression can no longer coexist with a green gate.
   **`make m2-exit` now exists** (`Makefile:312-365`), and before this it did not: the Exit line
   below named a target that `grep "^m2-exit:" Makefile` could not find, so M2 had no exit
   criterion at all. That is exactly how v0.3.0 shipped under M1's name while `make m1-exit` was
   RED. It reads **four** inventories — `m1-exit`'s three with the owner changed to M2, plus
   `docs/contributing/1.0-requirements.tsv` through `scripts/requirements.py`. The fourth is what
   closes the hole in the other three: all three are registers of *declared failures*, so a
   requirement nobody has started on leaves every one of them clean.
   **GI-09 is CLOSED: `make test-requirements-runner`** plants a row for the milestone under test
   and requires the runner to go RED for it, refuses a filter with no subject, and **runs
   `make m2-exit`**, requiring each of the four inventories to have produced output that matches a
   number this test recomputes from a tracked file — weakening the exit target is otherwise
   invisible to every other gate in the repo. *(It said `make -n m2-exit` for one round. `make -n`
   proves a recipe names a command, never that the command ran, which is the whole finding it was
   written to fix.)* It is in `gates`; `m2-exit` is not, for
   `thesis-exit`'s reason: a target that is RED by design can never be in that list.
   **GI-08 stays `owed`, and the residual is one sentence: `make m1-exit` does not read inventory
   four.** GI-08 says *every* milestone exit reads both debt inventories **and this manifest**, and
   one of the two that exist does not. That is deliberate rather than forgotten — the requirement
   manifest has **zero** rows owned by M1, so adding the inventory there would make the gate
   abstain (NO_VERDICT, nonzero) and turn a legitimately green target RED for a reason that says
   nothing about M1. Closing GI-08 means deciding what a milestone with no rows means, which is a
   question rather than a line of Make. Its own row's evidence, `make m2-exit`, also cannot exit 0
   until items 1–7 land, so the row is measured by neither thing today.
   **`m2-exit` is RED and that is the correct state.** It reports 8 rows `OWED_TO_M2`, down from
   25 — item 9's seven N3 rows moved to `satisfied` on `feat/m2-items`, after item 4's eight N6
   rows plus N4-12 had taken the figure from 34 to 25, item 3's eleven N5 rows had taken it from 47
   to 36, and item 5 (lexical completion) from 43 to 36 with N2-03, N2-04, N2-08, N2-09, N2-10,
   N2-11 and N4-02. A green `m2-exit` before M2 is done would be the defect.

   *The 8, read off `REQ_MILESTONE=M2 python3 scripts/requirements.py` rather than off this list —
   and the list is where they belong, which is not the same question as which item names them:*
   WT-01 (item 7); GI-08 (item 8); and **six owned by M2 without being ASSIGNED AS AN ITEM
   DELIVERABLE** — N4-04, N4-10, N13-03, N14-02, N14-04, GI-12. That count was THIRTEEN before item
   9, and the six that left it are the record of what an item deliverable is worth: N3-05, N3-09,
   N3-10, N3-12 and N3-14 became item 9's deliverables and are `satisfied`, and N3-13 left M2
   entirely (see below). Item 3's sentence had NAMED N3-09 and N3-10 without shipping them, which
   is the distinction the original paragraph was written to make — being mentioned in an item's
   prose is not the same as being what that item ships. The six that remain are mentioned nowhere,
   and the effect is the one worth recording: a list of items and a manifest of rows drift while
   both look complete.
   **Its verdict is three-valued and Make cannot carry it**, so the aggregation lives in
   `scripts/m2-exit.sh` and the verdict is published on the last line of stdout as
   `M2_EXIT_RESULT <code> <name>` — the contract `scripts/thesis-exit.sh` already defines, reused
   rather than re-invented. *(The first version aggregated with `|| rc=1` inside the recipe.
   Measured: `REQ_MILESTONE=M2 python3 scripts/requirements.py` exited **1 (OWED)** while
   `make m2-exit` exited **2**, which in this repository's own vocabulary says NO_VERDICT. Not
   lossy — wrong: a measurement reported as an abstention. `m1-exit` collapses the same way and is
   deliberately NOT changed here; it is 0 today so the ambiguity is dormant, and giving a shipped
   milestone's exit criterion a new contract is a decision about M1's ledger, not a side-effect of
   building M2's.)*
   **What it does NOT yet do**: steps 3 and 4 of the specification below — resolve each evidence
   locator and *run* it, and reconcile the Rust debt inventory by `req:` id. Both are named in the
   output of every run, and a milestone whose rows are all `satisfied` therefore exits **2
   (NO_VERDICT)** rather than 0, because "no row says owed" is a statement about a status column
   and not about the compiler.
   *(N14-01's evidence changed kind with item 6, and that is a contract transition: it was
   `gate make stdlib-gate`, which compares the registry against `tests/stdlib/BUILTINS.tsv` — a
   second copy of the compiler's own opinion — and is now
   `observable src/builtins.rs::test_registry_is_exactly_the_normative_builtin_set`, which
   compares it against N14's table in the specification. The old evidence could not have gone red
   on the defect the row is about.)*
   *(GI-06's paragraph previously read "GI-06 adds it and is STILL OWED, a one-word change nobody
   has made", and was correct on `main` when it was written. `fix/d3b-tail-if` made the change while
   closing an unrelated hole: `version-source-gate` needed a path to the umbrella, and the same
   reasoning — a target reachable only from `m1-exit`, which is RED by design, is never evidence
   that anything passed — applied to `test-honest`, which was measured green before it was added. The
   `gates:` citation has now been relocated TWICE during integration, each time because two branches
   grew the list independently and the conflict was resolved as a union. Both relocations were
   re-derived from content; neither was `--update`d onto whatever occupied the old line.)*
9. **Program items and the macro system** (N3-02, N3-03, N3-05, N3-09, N3-10, N3-12, N3-14) —
   **DONE: seven rows, on `feat/m2-items`.** Two commits: `e8eb1a9` (top-level `const` and
   `static`, and the three N3 evidence rows) and `90c8443` (the macro system). N3-13 is the one row
   that did NOT land and it is the most informative of the eight — it left M2 (below).
   `make selfhost` held `9b0cf24e…` throughout: `bootstrap/pdc.pd` contains no `static`, no
   top-level `const` and no macro.

   The decisions worth reading, each of which could have gone the other way:

   - **ONE `Item::Global` NODE FOR BOTH `const` AND `static`.** Everything that distinguishes them
     is a two-field answer — the C storage class and whether the name may be assigned — and
     nothing about parsing, name resolution or type checking differs. Two `Item` variants would
     have duplicated every exhaustive `match item` in the compiler, eight of them, to say the same
     thing twice.
   - **`const` LOWERS TO `static const long long X = 5;`, NOT TO A `#define`.** A macro is
     unscoped and untyped and would rewrite every later occurrence of that spelling anywhere in
     the file, including one that is a struct field or a local in an unrelated function — so a
     `const` item would change the meaning of code that never read it. The leading `static` is
     internal linkage rather than the item's own keyword: the emitted C is one translation unit,
     and a file-scope name with external linkage can collide with a libc symbol the program never
     mentions (`index`, `time` and `link` are all plausible item names).
   - **A `static` WITHOUT `mut` IS READ-ONLY**, for the same reason a `let` without it is. Making a
     plain `static` writable would have made it the one binding form in this language whose
     mutability is invisible at the declaration. `static mut` is assignable and its identity is
     observable: `tests/03_static_items.pd` increments a counter from one function and reads it
     from another.
   - **THE INITIALISER SET AND THE TYPE SET ARE BOTH CLOSED, and refused by name.** A top-level
     item becomes a C file-scope definition, and C requires such an initialiser to be a constant
     expression. So: literals and arithmetic over literals, types restricted to the integer
     widths, `f32`, `f64` and `bool` — and a call, a name, a string, an array, a struct or an `if`
     is refused where it is written rather than left to gcc's `initializer element is not
     constant`, which is a diagnostic about generated code that names nothing the author wrote.
   - **A LOCAL MAY NOT SHADOW A TOP-LEVEL ITEM.** C would accept the shadow and so would the type
     checker's own scope stack, and both would be right about their own rules, which is the
     problem: for a `static mut` the two readings differ in whether the program's state changed.
   - **`static` BECAME A KEYWORD, AND THE COST IS RECORDED RATHER THAN ABSORBED.**
     `tests/m1_c_keyword_idents.rs` drove `let static: i64 = x;` through identifier position to
     prove a C keyword survives to valid C; that program is now a parse error before any C exists,
     so the local was moved to `goto` — still a C keyword, and a better witness, since code
     generation emits real `goto` labels for a guarded `match`.
   - **THE FIRST STDLIB BLOCKER RETIRED BY A PARSER CHANGE.** `stdlib/std/math.pd` XPASSed off
     `CONST_ITEM`: its six `pub const` lines parse now. Its next floor is `e >>= 1;` at line 50,
     recorded as `SHIFT_ASSIGN` — `>>=` is not one token, the lexer produces `>` then `>=`. Fourth
     move for that one file and the first that was not lexical, which is the same lesson from the
     other side: a parser-level blocker also masks everything behind it, and measuring it
     establishes where the floor is now and nothing else.
   - **THE MACRO SYSTEM HAD NEVER SUBSTITUTED A PARAMETER, IN EITHER SPELLING, AND THE CAUSE WAS
     ONE MISSING ROW IN ONE TABLE.** `token_to_ast_token` (`src/parser/mod.rs`) ended in
     `_ => AstToken::Ident(format!("{:?}", token))` and did not list `Token::Dollar`, so `$x` in a
     macro body was stored as the identifier `Dollar` followed by `x`, and `substitute_template`
     — which keys on `Token::Dollar` — could never fire. Measured: `macro double!(x) { $x * 2 }`
     failed with "Undefined variable or function: 'Dollar'", and the bare spelling
     `macro double!(x) { x * 2 }` failed with "Undefined variable or function: 'x'". Completing
     that table is what `tests/03_macros.pd` records, and it is a table row rather than a redesign.
   - **FOUR SILENT WRONG-EXPANSION CLASSES BECAME NAMED REFUSALS.** `AstToken::Literal` is a
     `String` carrying no KIND and the reverse conversion guesses with `parse::<i64>()`, so each of
     these compiled, linked, ran and exited 0: `macro s!() { "hi" }` printed an EMPTY line;
     `macro pi!() { 3.5 }` printed `3.5` as a String; `macro yes!() { true }` printed `true` as a
     String; and `macro double!(x) { x * 2 }` with `let x = 3;` at the call site printed **6** for
     `double!(21)`, discarding the argument. The last one is the one to remember: the SAME program
     without an `x` in scope failed loudly, so whether that defect was silent depended on a
     variable name in a different file.
   - **EVERY SUBSTITUTED CAPTURE IS PARENTHESISED, and the parentheses are the difference between
     substituting an expression and splicing text.** `double!(1 + 1)` printed 3 and `double!(2 + 3)`
     printed 8 — both wrong, both green. They are pinned at 4 and 10 in `tests/03_macros.pd` so a
     regression to token-splicing fails a transcript instead of producing arithmetic nobody checks.
     Sound unconditionally only because `register_macro` gives every parameter `CaptureKind::Expr`;
     a capture kind that was a type or a pattern would need the decision re-taken.
   - **N3-14 IS ABOUT THE ABSENCE OF A SPLIT, NOT ABOUT `macro_rules!` BEING UNIMPLEMENTED.** It
     used to fall through to "Expected function, struct, enum, trait, type, impl, or macro
     declaration" — seven nouns, one of which is `macro`, which a reader arriving from Rust reads
     as "no macros here". Exactly backwards, and now refused by name in both the item and the
     invocation position.
   - **N3-13 (macro hygiene) LEFT M2 FOR M5, MEASURED RATHER THAN ESTIMATED.** `macro m!() { secret }`
     with `let secret = 42;` at the call site printed 42; `macro m!() { n }` invoked from two
     functions with different local `n` printed 1 and then 2 — one macro body, two meanings, chosen
     by the caller. Expansion renders the template back to SOURCE TEXT and re-lexes it
     (`src/macros/expander.rs`, `tokens_to_string`), so it is textual by construction and no
     fixture can make it hygienic. The "hygiene by refusal" reading was tested and fails: the
     introduce-a-binding route is not defended by the shadowing rule, it is merely UNWRITABLE,
     because `let` in a macro body is refused for an unrelated reason. A defence that works only
     while a neighbouring bug survives is not a defence. The row is an IMPLEMENTATION row and its
     work sits with the macro system's other remaining work, which is already M5's: expansion to a
     fixed point, the `println!`/`assert!`/`dbg!` builtins, and
     `tests/advanced_features_test.rs::test_macros`. **Moving it is a contract transition** and is
     recorded in the manifest header and in the pinned roster in `scripts/requirements.py`, a
     deliberate two-file edit.
   - **A SPECIFICATION TENSION WAS RESOLVED IN THE HONEST DIRECTION.** `grammar.ebnf` defines
     `static_item` and the requirement manifest owes N3-10, but the N3 normative EBNF in
     `language-spec.md` listed `const_item` and no `static_item` at all. Two normative files
     disagreeing is a fact conflict, not a formatting difference; the spec's production now lists
     both.
   - **WHAT THE MACRO SYSTEM STILL CANNOT DO**, each measured and each LOUD: expansion is a single
     pass, so a macro body may not invoke a macro (refused by name; expanding to a fixed point is a
     driver change); an expression body invoked in statement position fails with "Unexpected end of
     file"; `println!`, `assert!` and `dbg!` are unusable — measured broken BEFORE this round, and
     the string and two-character-operator refusals now say so out loud rather than letting them
     fail three phases later; `vec![e]` does not parse and only `vec!(e)` does, building a
     one-element array. `A4.6` in the specification claimed `println!` and `assert!` were
     implemented; that claim was false when it was written and is corrected.

**Exit**: `make m2-exit` (`Makefile:312-365`) — four inventories. **The condition is not "items
1–9 land"**, and saying so was a category error this line carried for three rounds: the gate reads
the MANIFEST, so it goes green when no row owned by M2 is `owed` or `blocked` — all 49 of them —
and six of the eight still outstanding are named by no item at all. Even then it exits **2
(NO_VERDICT)** rather than 0, because steps 3 and 4 of its own specification do not run: it does
not resolve each evidence locator and execute it, and it does not reconcile the Rust debt inventory
by `req:` id. "No row says owed" is a statement about a status column.

## M3 — Traits and generics (v0.5.0)

**Waits on**: M2 → **C0**. *This is the biggest move in the plan.* An earlier draft had it sixth, on
the argument that effects and totality do not need it. They do not need it to *exist*; they need it
in order not to be **designed twice** — the effect system is a typing judgment, and the moment a
bound exists you must say what the effect of a bounded method is. And the thesis requires a
bootstrap compiler that can grow: 991 lines, no abstraction.

**Owns 24 requirement rows** and the 18 `#[ignore]` rows tagged M4 in the old numbering, plus the
vacuous `07_traits_basic` and `08_generics_basic`.

1. **Generics that work** (N10-01…N10-05, N4-15, N4-21). Inside `<…>` any all-uppercase name is
   reclassified as a *const* generic argument, so `Foo<T>` does not mean what it looks like; generic
   struct fields are rejected in codegen; const generics are not monomorphised.
2. **Traits with real dispatch** (N10-06…N10-10). They parse and emit nothing, method bodies are
   never typechecked, and a `self` receiver in a trait method is a parse error. Design:
   [`trait_system_design.md`](../design/trait_system_design.md), [`generics.md`](../design/generics.md).
3. **`Option<T>` and `Result<T, E>` as generic types with methods** (N4-16), and `?` lowering onto
   the representation enums actually get (N4-18, N4-19). Their **prelude shipping is N4-17 and
   belongs to M8** — representation is what M3 buys; being in scope with no import is a packaging
   property, and one milestone should not claim both.
4. **Closures, function types, slices** (N5-08, N4-14, N4-11, N6-06) — and with them the first real
   instance of effect polymorphism, which is why M5 comes after this and not before.
5. **Function types and signatures reserve a latent effect variable** (N10-11). *This is a
   condition on the reorder's own argument, not a nicety.* Moving abstraction ahead of effects is
   justified by avoiding a redesign; if M3 builds effect-blind function types, M5 redesigns them
   anyway and the justification evaporates. So M3 is not done until a function type carries an
   effect slot — unpopulated is fine, absent is not.
6. `Result`-returning builtin signatures (N14-03), now that `Result` exists.

**Exit**: `make m3-exit`, including N10-09 as an observable — a bounded call must emit no vtable,
because "abstraction costs nothing at runtime" is a claim about generated code that no stdout can
show.

## M4 — Modules (v0.6.0)

**Waits on**: C0. Split out of the old combined milestone because it has a distinct consumer: this
is where both the bootstrap compiler and the standard library become multi-file.

**Owns 8 requirement rows** — N3-11 and N11-01…N11-07 — plus the corpus's one `xfail`
(`tests/conformance-manifest.txt:123`, cross-file imports) and the vacuous `12_modules_imports`.

A `mod` item, file-based nesting, **enforced** visibility (N11-02 is a `reject` row: a private item
imported must be an error, or visibility is decoration), and all four import forms.

**Exit**: `make m4-exit`.

## M5 — Effects, static half (v0.7.0) · differentiator 1

**Waits on**: C0 and **C2**. Everything in N7 except parallel execution.

**Owns 16 requirement rows**, the five `#[ignore]` rows tagged `unscheduled`, and the vacuous
`09_effects_system`, `10_async_await` and `11_unsafe_blocks` — the last because
[N7](../specification/language-spec.md#n7-effects-and-asynchrony) puts unsafe, IO, memory and panic
on one footing.

1. **Give the analysis a consumer** (N7-03, N7-08). The driver runs the analyser
   (`src/driver/mod.rs:172`) and prints the result (`src/driver/mod.rs:176-182`); nothing downstream
   reads it, so it cannot reject a program, change codegen or schedule anything.
2. **Make propagation a fixed point** (N7-04, N7-05, N7-06). It is a single forward pass whose
   fallback is "If function is unknown, we conservatively assume it's pure"
   (`src/effects/mod.rs:315-319`) — the unsound direction.
3. **Analyse methods** (N7-07). The driver's loop matches only `crate::ast::Item::Function`
   (`src/driver/mod.rs:173-174`).
4. **Delete `async` and `await` from the language** (N7-01, N7-02) — the two things N7 says the
   language does not have are the two the implementation has. The producer died at M2; the keywords
   die here.
5. **Effect contexts** (N7-10…N7-12), and N14's classification enforced (N14-05), which is what M6's
   `#![total(strict)]` uses to forbid `unsafe`.

**Exit**: `make m5-exit` — positive fixtures, the reject twin (a pure function calling an I/O
builtin), and the observables stdout cannot show (a callee defined below its caller still
propagates; `impl` methods are analysed).

## M6 — Totality (v0.8.0) · differentiator 2

**Waits on**: C2, C3, C0. **Un-split by the reorder**: with generics already present, generic
structural recursion (N8-07) lands here with the rest of N8 instead of waiting a milestone.

**Owns 12 requirement rows.** It owns no `#[ignore]` row and no conformance row today — the feature
is absent rather than broken, and absence has no fixture — so its first task is to write N8's
evidence ([F4](#f4-two-differentiators-owned-no-failing-row-anywhere)).

1. `#[total]`, `#![total(strict)]`, `#[decreases(expr)]`, `#[total(fuel = N)]`, `#[partial]`.
2. Structural recursion on an inductive type needs no measure — monomorphic (N8-06) and generic
   (N8-07).
3. `unsafe` is not permitted in a `#![total(strict)]` crate (N8-11) — M5's classification working.
4. **There is no mode in which an unproven `#[total]` function is accepted** (N8-12), as an
   observable rather than a rejection, because "no flag downgrades this" is a claim about the whole
   surface.

**Exit**: `make m6-exit`. Note the shape: five rejection rows *and* six acceptance rows. **A checker
that refuses everything passes every rejection and fails N8-01…N8-06**, which is why both halves
are required — the same reason the thesis gate's condition 3 exists.

## M7 — Reference typing and region inference (v0.9.0) · differentiator 3

**Waits on**: nothing but itself — **C1**. It is the deepest single capability and nothing before it
needs it, which is why it sits here rather than at the front.

**Owns 19 requirement rows** and two of the owner's decisions (**D3**, **D4**).

1. **A real reference type** (N4-13). **Spelled `ref` / `ref mut` from the start**, per
   [N9](../specification/language-spec.md#n9-references-and-lifetimes); building it under `&` and
   renaming later is two surface changes for one feature.
2. **Region inference** (N9-05, N9-06) — `grep -rn 'region\|Region' src/ --include='*.rs'` returns
   nothing. The **elision-total fragment** first: the fragment in which inference always succeeds.
   Everything outside it is a compile error naming the ambiguity, never a guess.
3. **Remove `'a` parameter lists** (N9-04) — **but keep `ref<'a> T`**, which N9 explicitly permits
   and N9-03 requires to be **accepted**. The receipt is two parser-level tests, not a grep for `'`:
   a grep would reject conforming programs and, once char literals land at M2, would fire on
   `let c = '<';`. The thesis gate implements exactly this distinction, and its self-test pins it.
4. **N12 becomes true of the implementation** (N12-03…N12-06): drop glue, per-value deallocation,
   `String` with move semantics, `ref mut` of a non-`mut` binding refused for every referent type.
5. **Two owner decisions close here**: **D4** (array parameters) and **D3** (`str`/`usize`).

**Exit**: `make m7-exit`.

## M8 — The standard library, and C FFI (v0.10.0)

**Waits on**: M3, M4, M7. The last prerelease.

**Owns 15 requirement rows.** What the library needs, as features rather than as compile errors:
generic ADTs with bounds (M3), associated types for an iterator protocol (M3), drop glue and moves
so a `Vec<T>` can own its buffer (M7), modules (M4).

`make stdlib-gate`'s per-file blocker column is **a lower bound and not that dependency list** — the
manifest says so itself: the blocker is the *first* construct `pdc` rejects, and a lexer-level
blocker masks every parser-level blocker behind it (`stdlib/prelude.pd` is recorded as `ATTRIBUTE`
while also containing 18 `use` and 2 `mod` declarations). The counts support exactly one claim,
**every one of the 21 files is blocked on at least one earlier milestone**, and not the stronger
claim that this is the earliest correct start.

1. Core, collections, math, string, I/O (N14-09…N14-16), and the prelude (N4-17).
2. **Ship it** (N14-06…N14-08). `make stdlib-gate` is **green right now with 0 of 21 files
   compiling** — it pins a measurement, it does not require a working library — so the evidence is
   every file reaching `ACCEPTED_NO_MAIN` in `stdlib/MANIFEST.tsv`, plus an observable that
   `import std::…` resolves with no environment variable set, plus an observable that both Homebrew
   formulae install the tree. Neither does today; `grep -rn stdlib .github/` returns nothing.
3. **C FFI** (FFI-01…FFI-03) — the one feature-list-only item kept in 1.0. It is nearly free: the
   backend already emits C. FFI-03 is a `reject` row, because an FFI boundary that is not
   effect-classified is a hole in N7.

**Exit**: `make m8-exit`.

## M9 — The thesis (1.0.0)

**Waits on**: everything. **This milestone's exit is the definition of 1.0**, so it ships as `1.0.0`
rather than as another prerelease.

**Owns 16 requirement rows**; 26 rows across the manifest carry `disposition = thesis` —
23 scored, `D1-01` the aggregate, and GI-11 and GI-12 as preconditions.

1. **Rewrite `bootstrap/pdc.pd` in the differentiated dialect** — `ref`/`ref mut` parameters with
   inferred regions, at least one discharged `#[total]`, inferred effects reaching callers, no
   `async`/`await`, no lifetime parameter list (TH-01…TH-05).
2. **`make selfhost` still reaches a byte-identical fixed point.** This is the whole argument: the
   dialect has to survive contact with a real compiler, written in it.
3. **`make selfhost-corpus`** (SH-02…SH-04), which does not exist. Today's `make selfhost` proves
   `bootstrap/pdc.pd` compiles **itself**; the 1.0 claim is that it compiles **the language** —
   every corpus fixture, matching the Rust compiler on acceptances *and* refusals. Matching only on
   acceptances is satisfiable by a compiler that refuses nothing, which is why
   [F3](#f3-the-conformance-corpus-has-no-negative-tests) has to be closed first.
4. **Witness 2 in the dialect** (WT-02, TH-06).
5. **Parity with `src/`, not retirement.**

**Exit**: `make thesis-exit`. Today it **exits 2 and offers no verdict** — see below. When a
verdict is available, every RED line names the milestone that owes it and every absent fixture
says `DECLARED, ABSENT` rather than passing.

## Scope: what is in 1.0, and what is not

Deferred, and recorded as `milestone = "post-1.0"` in `feature-index.toml` so the omission is
written down rather than silent: **a package registry · WASM · the LSP server · a debugger · a
formatter · Rust FFI · Lean/Coq proof export · refinement types · side-channel bounds.**

**C FFI is kept** (`milestone = "1.0"`): it is physically nearly free because the backend already
emits C, and a systems language whose I/O cannot leave the builtin set is not 1.0.

**Parallel-by-default and structured concurrency are `post-1.0`**, owner `P1`, blocked on **D2**.
This does not shrink differentiator 1: the feature list's differentiator is *"async without
coloring — no `async`, no `.await`, effects inferred and propagated"*, and M5 delivers all of it.
Automatic parallelization is a separate bullet on that list, and it is the one item whose semantics
are undefined.

Three feature-index rows remain **unclassified** and need a ruling: incremental compilation,
parallel compilation, and the LLVM backend. The thesis exercises none of them, and
[N1](../specification/language-spec.md#n1-overview-and-design-commitments) calls a second backend "a
second implementation of the same definition" — explicitly not a language property — so `post-1.0`
is the expected answer, but it was not in the reviewed cut list and is not assumed here.

## Decisions for the owner

### D1 — What gates 1.0 · **RESOLVED 2026-08-22**

Neither of the two options previously offered. Both were inventories, and "the inventory has no
unmet rows" is a completeness criterion — the generator of the fiction this repository spent M1
burning out. **1.0 is the thesis gate.** Scope follows from what the self-hosting compiler actually
uses. The manifest enumerates so that nothing is dropped silently; it does not gate.

Recorded as `D1-01`, whose evidence is `make thesis-exit` — currently, and correctly, RED.

### D2 — May the compiler emit an unnamed execution substrate?

Unchanged and still with the owner; it now blocks only `post-1.0` work. **This is not a
contradiction in N7.** Static effect tracking can compile into generated concurrency primitives with
no `Future` boxing and no programmer-visible executor. What N7 does not answer is whether the
compiler may emit **threads, an event loop, scheduling state, cancellation and joins that the
programmer never names**, and parallel-by-default requires *some* execution mechanism. The C backend
is not the obstacle — `runtime/palladium_runtime.c` already ships, and pthreads and kqueue are C.

- **Option A — permitted, unnamed.** *Cost*: N7 must then define three things it does not:
  **(i) sequencing** — what ordering is guaranteed between independent effectful operations, and
  what `effect::sync` guarantees beyond it; **(ii) cancellation** — when a branch fails or a
  `with_timeout` fires, are siblings cancelled, at which points, and are effects already performed
  observable (they are, and that must be stated); **(iii) errors** — if two branches fail, which
  error propagates: deterministically by source order, or nondeterministically by first-to-fail.
  Until those are written, N7-13/15/17 cannot be specified.
- **Option B — not permitted.** *Cost*: "parallel by default" and "automatic parallelization" leave
  N7 and the feature list. *Benefit*: the effect system stays a pure compile-time analysis with no
  scheduling semantics to reconcile with `#![total(strict)]`.

Either way **M5 is unaffected**.

### D3 — `str` and `usize` · blocks M7

**A** — add both primitives. *Cost*: two types through lexer, parser, checker and codegen, and `str`
needs a borrowed-string representation, so it depends on C1 and interacts with D4. *Benefit*:
`ref str` is the natural referent of `ref` and the differentiator's headline example survives.
**B** — rewrite the sites to `ref String` and `u64`. *Cost*: that example changes; lengths stay
signed. *Benefit*: no new primitives.

### D4 — Array parameters: value or reference semantics · blocks M7

Stated in full at [N12.1](../specification/language-spec.md#n121-array-parameters-open-decision).
**A** makes `[T; N]` a value type, so the three spellings mean three different things and every
array argument is a memcpy unless the author writes a reference. **B** makes it alias the caller's
storage, matching C, so the reference spellings are redundant and the specification must say which
is required for a parameter written through. Until answered the rule stays in code generation, where
the type system cannot enforce it.

### D5 — `src/async_runtime/mod.rs` · blocks nothing, `post-1.0`

498 lines whose only referrer in the repository is `src/lib.rs:5` (`pub mod async_runtime;`). No
compiler phase and no generated C uses it. **A** — delete it; N7 has no async runtime, so it can
never become one. **B** — keep it as the substrate D2 might permit. Not acted on: deletion is the
owner's.

## Findings

### F11. The async producer was alive and violated N7 — CLOSED

M1 fixed the `.await` **consumer** — `src/codegen/mod.rs:6162-6166` returns
`CompileError::await_unimplemented`. The **producer** was not touched: code generation dispatched
on `func.is_async` into `generate_async_function_with_name`, which emitted a `Future` struct and a
poll routine commented "Simplified async - immediately ready".

It was reachable, not dead code. Measured at `7484bac`, and unchanged at `acda322`:

```text
async fn g() { print("x"); }
fn main() { print("ok"); }
```

compiled, linked, ran, and the generated C contained
`typedef struct g_Future { int state; } g_Future;`, `int g_poll(g_Future *future)`, and
`g_Future g()`. A *returning* `async fn` was caught earlier by the type checker — "expected
Future<Int>, found Int" — which is why this survived: the shape that reached code generation was
the unit-returning one, and nothing tested it.

[N7](../specification/language-spec.md#n7-effects-and-asynchrony) is explicit: *"There is no async
runtime and no `Future` boxing. Effect tracking is entirely static and has no runtime
representation."* A `struct` with a `state` field, emitted into the program's own C, is a runtime
representation.

**CLOSED.** `async fn` is refused at the construct — in the type checker (`src/typeck/mod.rs`,
`check_function`) and again at the defect in code generation (`src/codegen/mod.rs:3315-3321`), the
same double placement `?` and `.await` already had. The emitter is **deleted**, not merely
unreachable: a private method nothing calls is one edit away from being called again. No line of
`src/codegen/mod.rs` now writes `_Future` or `_poll` into the C, and
`tests/m2_async_producer.rs` asserts that by deriving the site list from the source when it runs
rather than from this paragraph.

Two things came with it, because the refusal has to cover every route into the output and not only
the route the repro took. `monomorphize_function` hardcoded `is_async: false` under the comment
"monomorphized functions are not async", which made the comment true by **erasing** the property:
an instantiated `async fn g<T>` emitted an ordinary `g__i64`, and every downstream `is_async` guard
on an instantiation was dead code. The flag travels on `typeck::GenericFunction` now. And an
imported `pub async fn` is refused only when it is genuinely part of the emitted program — not when
it is private, not when a local definition shadows it, and not when it is a generic that nothing
instantiates — because over-approximating this exact rule has already rejected valid programs
twice.

Refused rather than lowered: dropping the Future struct and compiling the body as an ordinary
function would leave `async` a keyword the compiler silently ignores, which is the class M1 exists
to remove. The keyword itself dies at M5.

Companion: `src/async_runtime/mod.rs` (498 lines, sole referrer `src/lib.rs:5`) is N7-19 and
decision **D5**. Not deleted here.

### F14. The gate that defines 1.0 could never say 1.0 was reached

Round two closed "the gate cannot go RED". Round three found its mirror image, and it is the same
disease: **the gate did not measure.** `D1-01` — the row whose evidence is `make thesis-exit`
itself — was recorded `False` unconditionally, with no transition. Success required every row
green, so **exit 0 was unreachable by construction**. A gate that can only ever say no is exactly
as uninformative as one that can only ever say yes.

A self-referential row is not a member of the set it measures; it *is* the aggregate. It is now
excluded from evaluation and answered by the summary line, and the self-test's **first** case
drives an all-green repository state and asserts **exit 0** — so this cannot return silently.

Six more probes of the same family, each now with a control that fails on revert:

| Was | Now |
|---|---|
| `run_conformance` ignored the exit status and had no timeout: a run that emitted parsable verdicts and then failed was accepted, and a hung one hung the gate | every subprocess goes through [`scripts/gate_probe.py`](../../scripts/gate_probe.py), whose `classify()` yields `Concluded` (has `.text`) or `Malfunction` (**no text attribute at all**), with the timeout inside `run()` |
| TH-05 parsed effect output without requiring the compile to succeed | `effect_report` refuses to return text from a `pdc` that did not conclude *or* did not succeed |
| a `HarnessError` from conformance or witness reading was caught and turned into ordinary red rows, so a failure to measure exited 1 | measurement failure propagates and exits **2**; only an artifact the repository does not contain is a finding, and it says `DECLARED, ABSENT` |
| `p_effect_is_transitive` returned true for any reported function with no *recognised* builtin call — including one that called **nothing** | the edge `caller -> callee -> builtin` must be exhibited, with all three named |
| `p_total_on_fn` called a function "live" if its name appeared in any body — a dead caller, or its **own recursive call**, sufficed | **no other body in the unit mentions the identifier**, self-mentions excluded. Not reachability: a bare mention is treated as possible use, so the refutation refuses to fire rather than guess ([`provably_dead`](../../scripts/thesis_exit.py)) |
| `thesis_rows` validated only the column count: an unknown kind, a duplicate id or a **retyped row** dropped out of dispatch while the summary still printed the full count | the id set is **pinned**, an unknown kind is a harness error, and one result per row is asserted |

And the self-test itself, for the second time: it called the probe helpers directly, so deleting
the production wiring left every case green. It now builds a temporary repository — requirements
TSV, witnesses, conformance verdicts, `make` results, effect reports — and drives `main()`,
asserting the **exit code** and, for the disclaimers, its **output**. A hundred and three cases,
of which six drop the injection entirely and drive the real subprocess boundary — including one where conformance, `pdc` and `make` all run and
conclude successfully, so the *green* path is exercised and not only the failures. The one probe
group with no negative control (the real `make` subprocess) is a **disclosure pinned verbatim**:
emptying *or rewording* it fails the self-test. It is not a derived check, and says so — nothing
computes which probes lack a control.

Two things it caught that review did not. `fn f< 'a>(x: i64)` — a *spaced* lifetime parameter
list — **compiles today**, and `grammar.ebnf:191` makes whitespace insignificant between tokens,
so TH-02's adjacency-only `<'` missed a real violation. And running the repaired gate against the
real repository showed TH-05 compiling a witness *before* checking whether it existed, so an
absent witness exited 2 instead of reporting a finding — the very distinction that round's work
was about, inverted, one function away from where it was being fixed.

**The RED count moved 22 → 21, and no probe got weaker.** The only change is that `D1-01` left
the evaluated set to become the aggregate. `SH-01` is still the sole green row, and it is green
because `make selfhost` genuinely passes.

Cross-branch constraint, now enforced rather than hoped for: a reject fixture can go green on a
sibling branch **without a compiler change**, and a runner that sees only `REJECTED` cannot tell
"refused because the prohibition is enforced" from "refused for incidental unsupported syntax".
So the manifest gained a ninth column and each thesis reject row **names the diagnostic its
refusal must carry**.

**And condition 3 is currently weaker than its own banner says.** *"For an inference feature the
rejection is the product"* requires knowing **which** rejection you got. The corpus cannot tell:
measured, a fixture that fails on a stray `@@@` — an entirely incidental lex error — whose source
line happens to contain the phrase `there is no ``async`` keyword` is reported `REJECTED` at that
fingerprint and counted as `reject=1` coverage, exit 0. The compiler echoes the source line into
the diagnostic and `grep -qF` searches the whole log. That is requirement **GI-12**, owned by M2:
`pdc` emits a stable diagnostic **code**, and a reject row pins the code rather than a phrase that
can appear anywhere. Until it lands, a `reject` row proves *the compiler refused this program* and
*a declared phrase appears in the log* — not that the refusal was the one the row names.

The chain, stated exactly, because condition 3 rests on it: `scripts/conformance.sh:870` runs
`grep_status F "$fp" "$TMPROOT/diag"`, and `grep_status` (`scripts/conformance.sh:204-211`)
mode `F` is `grep -qF`. So the corpus's declared fingerprint is matched as a **literal
substring of any line of the ANSI-stripped compiler log** (`scripts/conformance.sh:869`) — not
an equality, not a regex. A log it cannot read is a third outcome, `HARNESS_ERROR`, kept
distinct from "did not match" (`scripts/conformance.sh:920-925`). The thesis gate then requires
the corpus's declaration to **equal** the fingerprint its row pins. Equality on the half this
gate owns, substring on the half `conformance.sh` owns, and both stated rather than assumed —
a sibling branch was caught doing substring where it meant equality.

### F13. The first thesis gate was blind in the way M1 spent itself curing

The command that defines 1.0 shipped, in `8acfd48`, checking **manifest text**. Its `row_is` asked
whether an editable line said `run` or `reject`; it ran nothing. So a missing fixture, a malformed
row, **a reject twin the compiler happily accepted**, or a rejection for an entirely unrelated
reason all reported green — inside condition 3, which exists because *for an inference feature the
rejection is the product*. Two external reviewers rejected it and found six more probes of the same
shape. Counting F12, that is the thirteenth occurrence of this repository's signature defect, at the
highest-stakes location it has yet occupied.

The repair was not more text validation. `scripts/conformance.sh` already compiles, links, runs,
diffs stdout against a recorded transcript, checks the declared failure *stage*, matches the declared
*diagnostic fingerprint*, reports `REJECT_ACCEPTED`, and reports `MISSING`. The gate now delegates
to it and reads only its verdicts — the same move as replacing a hand-rolled module scanner with
`cargo test --list`.

Six further probes were blind. Each fix has a negative control that fails when the fix is reverted:

| Probe | What it accepted | Now |
|---|---|---|
| TH-02 | `sed "s/ref<'…>/ref/"` had no identifier boundary, so `fn myref<'a>(…)` became `fn my(…)` and a forbidden lifetime list passed | the exemption is anchored; three negative cases, including `myref<'a>` |
| TH-05 | any output containing `has effects` and an IO spelling — and `bootstrap/pdc.pd:49-51` calls `file_write` **directly**, so it passed on a direct effect while claiming propagation | it names a caller that performs no IO itself, the callee it reaches, and the builtin that callee calls |
| TH-03 | any `: ref T` anywhere, including a struct field or a local annotation | it parses each `fn` parameter list; a field and a local are both negative cases |
| TH-04 | the bare text `#[total` plus a whole-file compile, so an unused trivial function satisfied it | an attribute token attached to a `fn` **whose identifier some other body mentions** — which is weaker than "called", and is the most the lexical model can support |
| TH-06 | a manifest label plus lexical decoration; it never ran the witness | witness 2 must be `PASS_VERIFIED` **and** pass every source probe |
| `--self-test` | called the helpers directly, so deleting the production wiring left all six cases green; no control at all for TH-03/04/05, SH-*, C2, C3, C4 | 29 fault-injection cases, and the two probe groups that still have no negative control are **named in the output** instead of left silent |

Two harness defects of the same shape are closed with them: an unreadable file made the scanner
yield the empty string, so TH-01/TH-02 reported **green** — a failure to measure read as a passing
measurement, the `total=0, exit 0` class `conformance.sh` already fixed once; and `MANIFEST` was
assigned and never read, so 1.0 had two definitions and only one was checked. A harness error now
exits 2 and says it is not a verdict, and the gate reads the 23 `thesis` rows out of the manifest
rather than restating them.

**The RED count went from 11 to 22, and that increase is the deliverable.** TH-01 and TH-02 were
green only because `bootstrap/pdc.pd` happens to contain no `async` and no lifetimes — a prohibition
satisfied by absence — while the second witness the same condition covers did not exist at all.
(As of this branch the fixture exists and runs; the condition stays owed on the words *no
workarounds* — see `tests/witness/json_parser.no-workarounds.md`.)
They are now honestly red.

### F12. The thesis gate's first lexer could not fail on what it checked

The first `strip_literals` treated every `'` as a quote. In `fn f<'a>(x: ref String)` the tick has
no partner, so the scanner consumed from it to end of file, and **TH-02 could never fire** — a green
line that could not go red, in the gate that defines 1.0. Caught by writing the self-test, not by
reading the code; it is F13's first instance and the reason the rest were looked for.

One deliberate consequence survives: the gate's scanner treats block comments as **non-nesting**,
because `bootstrap/pdc.pd:164-175` shows the compiler scanning for the first `*/` and breaking, with
no depth counter. [N2](../specification/language-spec.md#n2-lexical-structure) requires nesting and
the compiler does not implement it (F10, requirement N2-08). A gate that nested would disagree with
the compiler about whether a real `async` is commented out, so it matches the implementation, and a
self-test case pins that behaviour — it fails when N2-08 lands, forcing the two to flip in lockstep.

### F2. M1 shipped three of its own declared failures, and its exit command could not see them

`make m1-exit` exits 0 at `2ef170f`. **There** it was `CONFORMANCE_FORBID_OWNER=M1` over
`tests/conformance-manifest.txt` and nothing else, and no row there is owned by M1. On the
integrated tree it reads three inventories (`Makefile:294-295`) and exits **2**; that change
arrived with `fix/d3b-tail-if` and is what the closing paragraph of this finding now records. The second owner inventory — the `(owned by M<n>)` tag every `#[ignore]` reason carries and
`scripts/test-xfail.py:186` parses — had three M1 rows, all red. All three are now closed:

| Row | What was broken, and what closed it |
|---|---|
| `tests/e2e_test.rs:322` **CLOSED** | a tail `if` was not lowered to a return — fixed in `src/parser/mod.rs` (`lower_tail_to_return`); the `#[ignore]` is gone, so `make test-xfail` would report an XPASS if it came back |
| `tests/compiler_comprehensive_test.rs:633` **CLOSED** | `fn f() -> int { }` compiled with no diagnostic — the parser's own `returns_on_every_path` had been deciding the question since D3b and the call site did not act on a `false`; it now refuses (`src/parser/mod.rs:1245-1274`, `CompileError::missing_return`). Accept-side receipts: `tests/m1_missing_return.rs` |
| `tests/e2e_test.rs:277` **CLOSED** | `fn double` emitted `long long double(…)` and gcc rejected the compiler's own output — reserved words are escaped on the way into code generation (`src/codegen/c_ident.rs:440`). Controls on what must NOT be renamed: `tests/m1_c_keyword_idents.rs` |

The first reproduced: `fib(10)` printed `8261746944` and exited 0. **A silent miscompile shipped in
the release named for removing silent miscompiles.**

Re-measured after all three landed: `make m1-exit` **exits 0** and reports no `OWED_TO_M1` row. The
reading between then and now went 0 (blind) -> 2 (seeing) -> 0 (paid), and only the last of those
three zeros means the milestone is finished. At the time this finding was
written it exited **0**: a brief handed to this unit expected it red, it was not, and the reason was
the finding itself — the target read the conformance manifest, no row there is owned by M1, and the
three rows owed to M1 lived in the other inventory. Reading all three inventories fixes the omission but
not the class: owners are editable and the Rust inventory is whatever ignored tests `cargo` lists,
so deleting a test silently shrinks it. Hence a closed manifest — and, above it, a gate that rests
on a fixed point rather than on any inventory at all.

### F3. The conformance corpus has no negative tests

**CLOSED on the integrated tree: `reject=14`.**

*What this finding said, and the tension it carried.* It said `reject=0` on every run — and four
lines later, that 23 manifest rows are `reject` rows and M9's parity claim depends on them
existing. Both sentences stood in one section: `reject=0` counted rows the runner EVALUATED, the 23
counted rows DECLARED in the manifest, and nothing said so, so the file contradicted itself in
plain sight for several rounds. Root `CLAUDE.md` requires a fact conflict to be recorded rather
than left to coexist, and this one was not.

*Resolved by measurement, not by choosing a sentence.* On the integrated tree the runner evaluates
92 of them: `reject=92` over 182 fixtures (was 89 over 178 until `feat/m2-types-semantics` added three — the `for`-over-a-nested-array refusal and the two inner-length declarator positions; and was 87 over 176 until the round-3 review of `feat/m2-items` added two — the two `<<` branches the count-range fixture never covered, a legal AMOUNT with an illegal VALUE and a negative left operand; and was 82 over 171 until the round that followed added five more — the branches of the one-namespace and `pub` rules that the first round stated in prose and pinned in only one spelling each; and was 69 over 158 until the review round of `feat/m2-items` added THIRTEEN, which is what a review round costs when the reviewers probe rather than read: two fresh-binder shadowing shapes, both directions of a cross-kind name collision, four initialisers with no value, `pub` on an item, a macro invocation in an argument, two literal-kind losses and the macro_rules invocation spelling; and was 63 over 151 until the macro round of `feat/m2-items`
added six more: every one of them a refusal that replaced a SILENT wrong expansion or a diagnostic
naming a compiler phase, which is what measuring a subsystem before writing its fixture buys; and
was 57 over 143 until the same branch added six — the
five refusals top-level `const` and `static` needed, and `tests/reject/missing_return.pd`, which is
the corpus half of a payment `tests/m1_missing_return.rs` had been carrying alone; and 30 over 108
until `feat/m2-patterns` wrote down
issue #41's refusals — eleven of them, which is what a feature round costs once every "this shape
is not in the language" is a fixture rather than a sentence; and 22 over 84 until `feat/m2-expressions`
transitioned `tests/reject/loop_keyword.pd` to `run` — N5-07 gave the language the `loop` that
fixture asserted it did not have; and 21 over 82 before
`tests/reject/zero_length_array_self_reference.pd` landed with N4-23). The refusals a second implementation must reproduce are
in the corpus, not only in `tests/d5_unimplemented_constructs.rs` and `tests/d10_llvm_refuses.rs`,
which the bootstrap compiler will never run.

### F4. Two differentiators owned no failing row anywhere

Implicit lifetimes: zero conformance rows, zero `#[ignore]` rows. Totality: zero and zero. The
features are absent rather than broken, and absence has no fixture, so an owner-filter exit would
have gone green the day it was added. The manifest is one repair (M6 owns 12 rows, M7 owns 19, all
`owed`); the thesis gate is the stronger one, because it demands the features be *used* by a program
that cannot fake using them.

### F5. Two documents define 1.0 — superseded by D1

Recorded for provenance: the manifest previously had to choose between Part I and the feature list.
D1 resolved it by choosing neither. The LLVM backend remains the sharpest instance of the
disagreement and is one of the three rows still unclassified in
[Scope](#scope-what-is-in-10-and-what-is-not).

### F6. A second roadmap in the tree was written in the fictional present

[`docs/design/vision-roadmap.md`](../design/vision-roadmap.md) opened "Palladium α v0.7 has achieved
what many thought impossible", carried a benchmark table against Rust 1.74 for a compiler that could
not link a hello-world when it was written, and scheduled "Q4 2026: Language Freeze". Its body is
replaced with a supersession pointer.

### F7. Stale claims in documents this file rests on

- [A11](../specification/language-spec.md#a11-conformance) said "over 44 fixtures", "verified 33 ·
  vacuous 7 · xfail 2 · skip 2" and named "the three failures", against a then-measured
  "verified 43 … xfail 1 … failures 0" over "53 fixtures". **Corrected**, because this file treats
  the annex as an authority and stale data in an authority is release governance, not a
  documentation nit. **It went stale a second time** when the integration took the corpus to 70
  (`reject` 0 → 14), and A11 additionally carried a universally-quantified absence — "No fixture
  uses this class" — which nothing could contradict by passing. Both are corrected, and
  `language-spec.md` is now in `CLAIM_SCANNED` with the corpus size MEASURED from the manifest, so
  a third round of this is a red gate rather than a reading. Figures quoted as history are written
  in quotation marks; an unquoted `over N fixtures` is a live claim and is checked.
- `feature-index.toml`'s `async_as_effect` row claimed `cmd: grep -rn 'effects::' … -> 1 line`
  while a re-run returned **8**, because the evidence gate only regex-matched the *shape*
  `cmd: X -> Y` and never checked a `cmd:` item's output. **REPAIRED ON `main`, by the separate
  unit that owned it** — `make check-doc-evidence` now RUNS every `cmd:` item
  (`scripts/check_doc_evidence.py` executes them; `make test-doc-evidence` proves the gate goes
  RED on a deliberately false one), and `make gates` runs both. Recorded as closed rather than
  deleted: this row is the one place the finding is written down, and the integration of that
  repair is what closed it.

  *This bullet was rewritten during the `main` integration. Its two `path:line` citations were
  pinned against the pre-repair file; `main` rewrote it (+1077 lines) and the pinned text no
  longer exists, so the citations could not be relocated by matching. The claim they supported
  had become false, which is what the pin gate reported — a moved pin is sometimes a stale
  claim, not a stale line number.*
- `CLAUDE.md` describes `bootstrap/pdc.pd` as "~760줄". It is 991 lines — the number that makes
  M3's move to the front an argument rather than a preference.

### F8. One declared failure expects syntax the specification forbids

`tests/advanced_features_test.rs:340` is an `xfail` whose reason is that `macro_rules! vec { … }`
"is not an item". Under [N3](../specification/language-spec.md#n3-program-structure-and-items) it
must never be one, and `scripts/check-doc-evidence.sh` already fails any normative document that
writes it. A row that stays red unless the language changes is a negative test wearing the wrong
class. N3-14 makes it a normal, passing `reject` fixture owned by M2.

**HALF DONE, and the remaining half is an owner's call rather than work.** M2 item 9 landed the
`reject` fixture (`tests/reject/macro_rules.pd`) and the named refusal, so N3-14 is `satisfied`.
The `xfail` row still exists: its declared diagnostic was re-stated to the new refusal — it had been
the generic "Expected function, struct, enum, trait, type, impl, or macro declaration", which the
refusal replaced, and `make test-xfail` went red on the mismatch, which is the inventory working.
So the row is now a debt the language has DECIDED not to pay, held by M5. Retiring it is a
contract transition on someone else's milestone and is not taken here.

### F9. The milestone labels in the test suite were written against the old numbering

Against this sequence: the 14 tagged M2 stay **M2**; the 18 tagged M4 become **M3**; the 3 tagged M1
become **M2**; the 5 tagged `unscheduled` become **M5**; the 1 tagged M5 leaves the inventory (F8).
**26 re-tags and one reclassification.** *(An earlier draft said 27; it counted the reclassified row
as a re-tag as well.)* They must land **atomically, before any owner-filtering target ships**, with
the self-test that proves the filter detects a planted row. Each reason should also gain a
`req: <id>` tag, which is what turns the Rust half of the reconciliation into a command. All of it
is an edit to `tests/`.

### F10. Block comments do not nest, and nothing said so

[N2](../specification/language-spec.md#n2-lexical-structure) requires `/* … */` to nest.
`fn main() { /* a /* b */ c */ print("ok"); }` fails with `Expected expression, but found '/'`.
[A2](../specification/language-spec.md#a2-lexical-structure) records comments as implemented without
qualifying nesting — a normative requirement with no annex row and no test, found by enumerating N2
rather than by any gate. N2-08, owned by M2.

## Keeping this file honest

```bash
make thesis-exit         # the definition of 1.0. RED until M9, by design
make test-thesis-runner  # and the proof that it can still go red
make gates               # conformance + gate self-test + docs + selfhost + stdlib + probe
make test-honest         # every test binary, integration tests included
make test-xfail          # every declared failure, and the milestone that owes it
```

Five rules this file is held to:

1. **1.0 is a command, not a count.** If the definition of done can be satisfied by a table with no
   red cells, it can be satisfied without the language existing. It has been, twice.
2. **If a milestone's exit criterion cannot be written as a command, it is not an exit criterion.**
3. **The exit command covers the goal, not a proxy.** For an inference feature that means the
   rejection, not only the acceptance: an inferencer that accepts everything is a no-op, and a no-op
   is invisible in green fixtures.
4. **A milestone owns rows in a closed inventory.** Deleting a row, moving it between milestones, or
   moving it to `post-1.0` is a contract transition and is reviewed as one.
5. **Paying off a row is a transition, not a deletion.** The fixture stays on disk and its row
   becomes `run` with a transcript, in the same commit.

Every `file:line` here is fingerprinted by `make check-doc-evidence`. What that gate cannot do is
check a `cmd:` item's *output* — see [F7](#f7-stale-claims-in-documents-this-file-rests-on).
