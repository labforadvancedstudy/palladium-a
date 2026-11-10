# LEDGER — issues #89 + #92 (comparison on composite types)

## Step 0 — intake
- Brief + coder-rules read. Worktree iss/89 @ d5166e8 (fix/issue-89 = main). Main pdc = gates-rel/target/release/pdc.
- typeck: Eq|Ne|Lt|Gt|Le|Ge arm (src/typeck/mod.rs ~4238) admits any two operands that `merged` to one type.
- codegen (src/codegen/mod.rs ~6288) emits a raw C `==`/`<`... for every operand type, incl. `const char*` (String).
- bootstrap/pdc.pd compares strings with `string_eq`, not `==`.

## Step 1 — measure on main (probes/, runprobes.sh, old/summary.tsv)
- CORRECT today: Int (i32/i64/u32/u64; i8/i16/u8/u16/usize/isize unreachable — PD0042 on a literal), Float, Bool, Char,
  aliases of those, reference PARAMS (codegen auto-derefs `a`: value compare), fields/elements of those.
- gcc ill-typed (rc 3): tuple, struct, enum (fieldless + payload), generic struct, Range, unit-returning call, alias of
  tuple/struct, guard/while/let/return/!/&& positions, generic T at struct/enum, `<` in a macro body on tuples.
- WELL-TYPED BUT WRONG (silent): String `==`/`<` = pointer compare (`int_to_string(12) == "12"` -> 0; `"b" < "a"` -> 1;
  guard/while on computed strings wrong); arrays = decayed-address compare (`[1,2] == [1,2]` -> 0, also via macro `<`);
  generic T at String -> 0. Function names compare addresses (fn types are unimplemented per spec).
- Macro `==`/`!=`/`<=`/`>=` unreachable (PD0074 multi-char operator); `<`/`>` reach typeck after expansion.
- Different class (found/): reference LOCALS (`let r = &x`) are typed as the referent but emitted as pointers —
  wrong in every value position (`print_int(r)` gcc error, `r == 5` -> 0); explicit `*a` on a ref param double-derefs;
  mixed signed/unsigned (`u64 1 > i64 -1` -> 0) — int widths merge to one Int.

## Step 2 — RED (unmodified tree + new tests/m2_comparison_operands.rs only)
- `cargo test --release --test m2_comparison_operands`: 3 passed (controls), 23 FAILED, every one at line 78
  "the front end accepted a comparison it must refuse" (red.txt). Controls pin main's exact digits.
- Design: one code PD0079 (comparison_operand_is_int_float_bool_or_char); site 1 = the Eq..Ge arm; site 2 = a
  post-pass that re-checks each generic instantiation's body on a CLONE of the checker (bodies are never checked).

## Step 3 — fix
- src/errors/codes.rs: PD0079 ComparisonOperandIsIntFloatBoolOrChar (ALL 71->72).
- src/typeck/mod.rs: Eq..Ge arm calls `no_comparison_on` (admits Int/Float/Bool/Char/TypeParam/unknown name; names
  type + way to compare). `refuse_instantiated_comparisons` at the end of `check()`: over `get_instantiations()`,
  checks each instantiated body on `self.clone()`, keeps only PD0079. `#[derive(Clone)]` on TypeChecker,
  TraitResolver, TypeErrorHelper. GREEN: m2_comparison_operands 26/26.

## Step 4 — corpus + registry
- run fixture tests/02_comparison_operands.pd (+.expected; main prints identical stdout, sha 7f1d35b2).
- 6 reject rows tests/reject/comparison_*.pd, PD0079;msg~ each; registry row; pin map rows; gen-code-pins.py
  verified all 130 pins and wrote nothing; REFUSAL_SET_SHA 84486334…, SIZE 130.

## Step 5 — old vs new
- probes 103: 55 same, 48 refused PD0079, 0 changed, 0 newly running. Tree sweep 395 .pd: 0 verdict changes;
  132/132 accepted files emit byte-identical C. Final-binary sweep: 0 changes.

## Step 6 — docs + gates
- counts in MILESTONES.md (x2) + language-spec.md; feature-index cmd row 94->95 lines; 56 citations renumbered by an
  exact difflib line map (content verified identical), then --update --allow-repin, then plain run OK.
- gates: build clean; lib 599; tests green; conformance verified=90 reject=128 failures=0 untranscribed=0;
  diag codes every check green (coded=130); ledger CLEAR; doc evidence OK; selfhost e8bd8cda.
- found/: reference locals, *a on ref param, mixed signedness, nested generic call, cyclic alias hang (rc 142).

## Round 1 (review of 71c4d6f: astra REJECT — typed `T` locals + first-error stop in the generic post-pass)
- src/ast/mod.rs: 4th TypeVisit `Substitute` + `substitute_type_params_in_block` (appended at EOF, no cited line moves).
- src/typeck/mod.rs: post-pass = worklist over get_instantiations() (+ instantiations found while probing, so
  forward<T> -> same<T> is checked); whole body substituted; `probe_block`/`probe_statement`/`probe_expr` recover
  from any non-PD0079 refusal (restore symbols/loop/break/unsafe, walk parts, a failed `let` still binds at its
  written type). No ast::Function literal any more -> feature-index cmd row back to 94 lines.
- fable: `in_import` names `module::fn` for a PD0079 raised in an imported public body (call stays one line).
- RED vs 71c4d6f (git worktree, new tests copied): 4 FAILED (3 at :78 accepted, imported at :614); GREEN 31/31.
- feature-index gate: strings re-derived: fixtures=232 evaluated=232 verified=90 reject=128.
- Service restart killed the probe run + a cargo test (exit 137); re-running.
- make gates relaunched DETACHED (setsid) -> 89-work/gates.log after two service restarts
- run 1 of detached make gates: exit 2 at test-xfail — 'cargo test -- --list --ignored' hit gate_probe TIMEOUT_S=300 (exit 124) while rebuilding every test binary; relaunched detached after 'cargo test --release --no-run'
- run 2 of detached make gates: exit 2 at test-xfail — a REAL finding: tests/advanced_e2e_test.rs::test_result_error_handling
  (ignored XFAIL) now failed for PD0079 (`s == "42"`) before its declared generic-enum reason. Swept all 259 `fn main`
  programs embedded in tests/*.rs old vs new (embsweep.sh): exactly 2 change — that one and test_pattern_matching_guards
  (PAID, running: `text == "hello"` passed on main only by literal pooling). Both now use `string_eq` (the refusal's own
  advice); guards test passes, the XFAIL fails for its declared PD0048 reason again.
- run 3 of detached make gates: exit 0, last line '✓ all gates green' (test-honest 37 binaries, 1128 passed, 0 failed; xfail 43/43 declared).

## Round 2 (review of bb324cd: astra REJECT — false refusal through a failed shadowing `let`; block/if/loop/match/cast operands)
- probe_statement: every `let` (and `for` var, every pattern name) BINDS — written type, else recovered type, else
  `<unknown>` (a TypeParam: never refused, agrees only with itself); `probe_bind` REPLACES in the innermost scope (fable).
- `probe_type`: recovered result type per form (block tail, agreeing if/match arms, loop break value via
  BreakTarget::Value, cast target, field/index/tuple-index/call/method/builtin return); a failed comparison is
  re-judged on recovered operand types (both known + merged) in probe_expr. Break recovery records its value.
- astra r2 probes: all String shapes PD0079; all int controls 1 (loop_bind_int rc3 = main); shadow_* print 1.
- tests: +4 (35 total).
- r2 make gates: unlocked run killed (coordinator: use shared lock); relaunched via gates-locked.sh -> gates.log

## Round 3 (review of 3a9567e: astra REJECT — aggregates, branch breaks, disagreeing match arms; dispatcher: add a CODEGEN BACKSTOP)
- src/codegen/mod.rs: Binary emitter calls `refuse_a_comparison_without_one` (one added line); helper in a new
  `impl CodeGenerator` appended at EOF: PD0079 when an operand's C type (try_infer_expr_type) is KNOWN and not a
  by-value scalar (`*`/`[`/struct/tuple/void refused; unknown never). Other C comparisons codegen emits are scalar by
  construction (enum .tag, literal/range patterns, for bounds; String patterns use __pd_string_eq).
- grok + fable APPROVE r3; fable: include both disagreeing-match shapes in tests.
- r3: stale (backstop-only) measurement chain stopped; runall.sh now kills each program's process group on timeout (tmo.pl); rebuild detached -> build-r3.log
- typeck gaps closed: aggregates (array/repeat/tuple/struct/enum) recover parts' types; `probe_leave` keeps break values
  recorded inside scopes; disagreeing match arms stay unknown in typeck -> the codegen backstop refuses them.
- tests +5 (40/40). RED vs 3a9567e (worktree): 5 FAILED (4 at :78, imported at :841), 35 passed.
- old-vs-new RUN over 795 programs (my 177 + 354 panel probes + 259 embedded): 537 same (+2 rows whose output holds a
  TAB, identical raw), 253 refused PD0079, 0 output changed, 0 newly running; 159 refused that ran on main — ALL address
  compares (137 String, 15 array, 3 `long long*`, 3 fn, 1 &struct). fable cost_many_inst: main 142 (load) -> rc 3 both
  when re-timed (typeck 7.8ms -> 179ms). Tree sweep 402 files: 0 verdict changes (+5 ignored stdlib-gate scratch).
- docs: codes.rs variant doc + registry justification name the third site and its corpus gap; 15 codegen citations
  renumbered (+1 line), --update --allow-repin, plain: doc evidence OK.

## Rebase onto 903ba02 (#85 landed with PD0079)
- git rebase origin/main: conflicts in codes.rs, check-diagnostic-codes.sh, pin map, language-spec, MILESTONES, feature-index,
  registry — resolved keeping both; my code renamed PD0079 -> PD0080 everywhere (ALL 73; refusal set 131 rows, sha 5b076afe…);
  counts recounted from the merged manifest (verified 91, reject 129, fixtures 234). HEAD 9c14d6b + uncommitted renames.
- citation-pins.tsv: main's side; re-pin LAST.

## Round 4 (astra REJECT on 9dca568: backstop judged a RECONSTRUCTION)
- false refusal: `(loop {let x=1; break x;}) == 1` with an outer String x — try_infer_expr_type's Loop arm types `break x`
  outside the loop's scope. Escapes: unknown_loop_strings (two const char* hoist temps compared), loop_local_pointer
  (`long long*` temps). Fix direction: judge the operand's EMITTED carrier (hoist temp's declared type), fix the scope bug.
- codegen fix: the comparison emitter generates BOTH operands first, then checks; `comparison_carriers` records the
  DECLARED C type of every value form (if/block/match/loop temp) emitted inside the comparison, keyed by node, read
  only during the check. Inference scope class fixed: loop arm types the break in its own env (`break_value_in`),
  `locals_of` binds an untyped `let` as UNTYPED (shadows), pattern names always shadow; dead `first_break_value` gone.
- typeck probe: loop bodies push their own break frame (fable r4 nested-loop note).
- astra r4 probes: loop_shadow_scalar 1 (was false refusal), loop_local_pointer + unknown_loop_strings PD0080, all
  int controls 1; pointer_same_place / f==f / f!=g refused (policy: identity on addresses).
- tests +4 (44). Measurement chain r4-chain.sh: an untypable operand currently refuses with marker PD-CMP-UNKNOWN to COUNT it.
- unknown carrier: final policy = refuse by name (no marker): measured 2/952 programs (generic enum payload in a generic
  body), both gcc failures on main. 952-program old-vs-new: 619 same, 329 refused PD0080, 0 output changed (2 TAB rows
  identical raw; 2 rc-142 rows re-timed at 90s: identical output), 0 newly running. Tree sweep: 0 verdict changes.
  C identity: 498/498 programs both accept emit byte-identical C (214 DIFF = refused, no C). Tests 44/44.
- 106 citations renumbered (HEAD -> tree, verified identical); citation-pins.tsv reset to origin/main; chain r4-final.sh:
  RED vs 9c14d6b, tests, diag codes, ledger, doc evidence --update --allow-repin then plain.
- lib test codegen::tests::test_codegen_break_continue fed codegen an undeclared `x` (no typeck) -> unknown carrier refused; test now declares x (same line). manifest citation :130 -> :131 (MILESTONES); re-pin OK, plain doc evidence OK. Chain r4-gates.sh: clean RED, tests, then gates-locked -> gates-rebased.log
- clean RED vs 9c14d6b: a_loop_value_is_judged… FAILED (:50 legal program refused), loop_values_the_reconstruction…
  and loop_values_that_are_pointers… FAILED (:78 accepted). Nested-loop test rewritten to fable's two-sided shape;
  measured: main = codegen's own refusal, round-3 binary = PD0079 naming (Int, Int), new = main's refusal.
- focused tests green (lib 599, m2_comparison_operands 44, gi12 34, d3b 45, m3 40, enum_composites 43, n6 4,
  advanced_e2e 4, macro_expansion_reach 7). Gates via lock -> gates-rebased.log.
- make gates (locked) exit=0: '✓ all gates green'

## Round 5 (astra REJECT on be3efd0: `(x+0) == (y+0)` over const char* — arithmetic inference typed ptr+int `long long`)
- DISPATCHER: make the C compiler the judge. Every Palladium comparison is emitted as
  `(sizeof(struct { _Static_assert((0x11E >> __builtin_classify_type(L)) & (0x11E >> __builtin_classify_type(R)) & 1,
  "PD0080|line|col|op|fn"); char __pd_; }) && (L op R))` — measured on Apple clang (`gcc`): scalars 0 warnings
  (-Wall -Wextra too), each operand evaluated once, pointer/array/struct fail with the marker.
- linker: `comparison_refusal` parses the marker -> LinkError::Refused (exit 1, EXIT_REFUSED); main/driver report it
  as error[PD0080]. Codegen backstop kept (earlier, better message).
- arithmetic inference: ptr±int -> ptr (array decays), ptr-ptr -> long long (ptrdiff_t), String+String only for `+`.
- 2 lib tests asserted the old `(a < b)` text -> now assert the checked form (same lines). New linker unit test
  drives the real check through the real link -> Refused/PD0080/line 7 col 9; scalar control links clean.
- tests: lib 600, m2_comparison_operands 48 (+4). fable r5: APPROVE; correction — 1 newly running program
  (fr_nested_loop_shadow), from break_value_in's scope fix.
- r5 measurement over 1155 programs (952 + 203 new panel probes): 725 same (+2 TAB rows identical raw, +6 cost probes:
  main rc 142 under load, rc 3 re-timed = identical), 420 refused PD0080 (282 ran on main: 236 String, 21 array,
  15 pointer, 8 fn, 2 &struct), 0 output changed, 2 newly running — fr_nested_loop_shadow and loop_nested_hoists_int,
  both print the correct 1 (main: break typed outside its loop -> gcc error / "cannot infer"). Tree sweep 0 changes.
  No program reaches the C judge (backstop refuses first). Tests: +11 binaries green; selfhost e8bd8cda unchanged.
- docs: codes.rs + registry name the judge; 103 citations renumbered vs be3efd0; chain: re-pin, plain, gates (lock).

## Round 6 (continuation: r5 gates exit=2 — 3 emitted-C text tests; judge marker said `PD0080|0|0|...`)
- Cause of line 0: a Binary comparison took line/col from its LEFT operand, and a name/literal has a dummy span.
  RED (r6/red-r6.txt): `the_c_compilers_check_names_the_operators_line_and_column` — emitted `PD0080|0|0|<|main`,
  `0|0|>=`, call left `7|9`; `a_comparison_only_the_c_compiler_can_judge_is_pd0080_at_the_operator` — "(in `main`)".
- Fix: src/parser/mod.rs parse_equality/parse_comparison place the span's line/col at the OPERATOR token (start/end
  unchanged; same line count, no citation moves). GREEN 2/2. Only other comparison builder: optimizer's
  `!(x==y)==z -> x!=y` (simplify.rs:178-205) — a silent miscompile (prints 1, must 0; main too) -> found/.
- FOUND: r5's `comparison_refusal` read the tag ANYWHERE in stderr: 12/1155 programs (r5 AND r6-pre) were reported
  PD0080 while the check never failed (tag only in clang's echoed source line: undeclared generic `id`, malformed
  array-literal C) or failed only as a cascade (param redeclared by shadowing `let`). Main: rc 3 gcc failure for all
  12. RED: r5's function verbatim on clang's captured stderr (r6/red-map-r6.txt): echoed->Some, cascade->Some.
  Fix: only error lines (not echoed) that are static-assertion failures carrying the tag, and only if EVERY error
  line is one. Tests: linker `only_the_checks_own_failure_alone_is_pd0080`, integration
  `a_gcc_failure_that_is_not_the_check_is_not_pd0080` (both programs -> GccRejected, as main). Registry text says so.
- tests/compiler_comprehensive_test.rs: 3 tests call `compile_and_verify_comparisons` (EOF helper): asserts one check
  per comparison operator written in the source, then reads the C with each check taken out; original patterns kept
  byte-for-byte. 23 passed, 3 XFAIL ignored. Sweep of every test string asserting C comparison text: only those 3
  + the 2 lib tests r5 updated; test_comparison_operators passes unchanged (`(a == b)` still a substring).
- Stale claim fixed: linker NON_FATAL `-Wparentheses-equality` reason — measured tree .pd: main 179 in 30 files,
  now 3 in 1 (tuple-pattern lowering `if ((_match_expr.f0 == 0LL))`, tests/06_tuple_patterns.pd); cct.rs:289 tense.
- old-vs-new vs main 903ba02 (iss/85-gates pdc, clean, copied r6/pdc-main903) over 1155 (all panel probes incl. 118
  astra r5, by content): 744 same, 408 refused PD0080, 2 newly running (fr_nested_loop_shadow, loop_nested_hoists_int:
  print 1), 1 rc-142 on main (cost_many_inst; re-timed 120s: rc 3 both = same), 0 output changed; 282 refused that ran
  on main (236 String, 21 array, 15 pointer, 8 fn, 2 &struct — r6/refused-ran-r6b.tsv); 22 refused by both, PD0080
  now named first. 0 programs reach the judge. astra: arithmetic_hole PD0080 at (line 4, column 13); deref/distance/
  int controls print 1. r5 vs final: exactly the 12 above change (PD0080 -> main's rc 3). Tree sweep 409: only the 6
  own reject fixtures differ from main.
- checks: lib 600; m2_comparison_operands 51/51; cct 23 (+3 ignored); diag codes; ledger CLEAR.
- codegen c_scalar_check doc: PD0080 at the operator's place, when the check's failure is the only error (+1 line, EOF impl).
- checks (final tree): diag codes every check green (conformance verified=91 reject=129 untranscribed=0 failures=0);
  ledger REQUIREMENTS_RESULT 0 CLEAR; gate-receipts rc 0 (5 commands ok); selfhost e8bd8cda fixed point;
  doc evidence --update --allow-repin (425 pins) then plain: OK; pins on touched files all on unchanged lines.
- m2-exit run 1: M2_EXIT_RESULT 1 OWED — inventory two's `--list --ignored` hit the 300s probe timeout (exit 124,
  load 60, binaries rebuilding; the round-1 artifact); inventory three 1154 passed 0 failed. Run 2 (binaries built):
  inventory two 43/43 declared, 0 owed; three 37 binaries 1154 passed 0 failed 43 ignored; last line
  `M2_EXIT_RESULT 2 NO_VERDICT`. Logs r6/m2-exit-r6.log (+ -run1-xfail-timeout.log).
- gates launched detached (gates-locked.sh) -> 89-work/gates-r6.log, not awaited.

## Round 7 (r6 = 76734e5: grok + fable APPROVE; astra REJECT — exponential C; String range patterns bypass the judge)
- Cause 1: the check embedded both (already-wrapped) operands twice -> nested comparisons doubled per level
  (depth 17: 23,405,093 bytes, clang timeout). Cause 2: pattern_condition / for bounds / as-bool / enum tag wrote
  raw C comparisons ("scalar by construction" false in a generic body: `_match_expr >= "a"` on const char*).
- Fix (src/codegen/mod.rs): ONE door `checked_comparison(lhs, c_op, rhs, what, span)`: sizeof form when neither
  operand holds a check; else GNU statement expression `({ __auto_type l = (L); __auto_type r = (R);
  _Static_assert(...); l op r; })` (measured clang 21: 0 warnings -Wall -Wextra -Wshadow, arrays decay and are
  refused, depth 200 in 0.33s). File scope: initialiser plain + judged twin `__pd_judge_<item>` (uncalled fn).
  Sites routed: comparison emitter, literal/range/char/bool patterns, enum tag, for-range (lo<=hi, k==n, v<hi),
  array for (_i<N), `as bool`; dead comparison arms in the regular operator table -> unreachable!. Marker token
  -> phrase in comparison_refusal ("a range pattern", "a literal pattern", "`as bool`", ...).
- RED on 76734e5 (r7/red-r7.log): 5/5 new tests fail (one more level cost 11,666,944 bytes; `&& (i < __pd_hi`
  not judged; range/literal/bool patterns and `as bool` at String LINKED; file-scope initialiser not plain).
  GREEN 56/56. astra r6 probes vs main: nested_calls_17 C 12,702 B (main 9,107; r6 23.4 MB), prints 7 1;
  string_range(_literal), generic_pattern_pointer/_range_pointer/_bool -> PD0080 at the match (main 0/0/0/134);
  controls side_effect_once 1 2 3, short_circuit 5, macro 1, normal_range 1, string_pattern 1, primitive 1 2 3.
- Tests: for-loop C text (4 lib + cct test_all_keywords/test_arrays/test_control_flow) follow the check; cct
  helper counts a range `for` as one check. New fixture tests/reject/comparison_in_a_generic_range_pattern.pd.
- found/: generic_string_subtraction.pd (fable: `(a - c) == 0` at String prints 0 — String subtraction unrefused
  in a generic body); generic_body_at_char_refused.pd ("expected char, found Char", main too).
- Committed c10b15a (both Co-Authored-By lines). Rebased onto origin/main 2ca5eb5 (#93 took PD0080/PD0081):
  7 commits replayed, tip a15adc7; conflicts = citation-number churn (resolve.py: a side whose change is only
  citation numbers yields; 13/19 files auto) + codes.rs/registry/pin-map/check-script/prose (kept both).
  My code -> PD0082 on exactly the 80 lines my diff adds vs 2ca5eb5 (mylines.py; main's PD0080 untouched);
  ALL 75; pin map 7 rows (+ range-pattern row); refusal set 134 rows sha 924d63a5; counts recounted from the
  merged manifest: fixtures 238, run/verified 92, reject 132. ExpansionGuard (main) given a manual Clone
  (the post-pass clones the checker; CompileError is not Clone). 165 citations re-aimed by content (renum.py:
  origin version by citing line, difflib equal blocks only); citation-pins.tsv = main's, re-pin LAST.
- Post-rebase: main's tests/n4_generic_argument_kind.rs `Wrap<K>` == `Wrap<Kind>` C now differs only in the
  checks' SOURCE positions (enum tag test at the `match` column) -> compared with positions blanked (EOF helper).
  Conformance HARNESS_ERROR on the new fixture: a judge refusal left its C behind (C + failed build = backend
  defect to the gate) -> `link` removes the translation unit on LinkError::Refused (a refusal by name leaves no
  C, as a front-end one never writes it); row now REJECTED at compile. Test fn names pd0080 -> pd0082.
  linker NON_FATAL -Wparentheses-equality note: 179 in 30 files on main 2ca5eb5, 0 now (tuple patterns judged).
- old-vs-new vs main 2ca5eb5 (r7/pdc-main2ca, built from a worktree of 2ca5eb5) over 1249 programs (1155 + 87
  new panel probes + 7 own): 805 same + 5 cost_many_inst (main rc 142 under load, rc 3 re-timed at 150s = same),
  437 refused PD0082, 2 newly running (fr_nested_loop_shadow, loop_nested_hoists_int: print 1), 0 output changed.
  299 refused that ran on main (r7/refused-ran-r7.tsv): 243 String, 21 array, 16 pointer, 10 fn, 2 &struct —
  address comparisons — and 7 the C compiler's check alone catches (4 String range patterns, 2 literal patterns
  at String, 1 `as bool` at String; main answered from addresses). Tree sweep 413 files: only the 7 own reject
  fixtures differ. 8 programs reach the judge (7 above + generic_pattern_bool, main 134).
- checks: lib 601; m2_comparison_operands 56; compiler_comprehensive 23 (+3 XFAIL); n4 18; full suite before the
  last edits 1176 passed + n4 fixed; diag codes every check green (conformance verified=92 reject=132
  failures=0, coded=134); ledger CLEAR; gate-receipts green (10/10); doc evidence --update --allow-repin (133
  re-aimed + 2 MOVED by an inner citation number only) then plain OK.
- m2-exit (binaries prebuilt): inventory 1 conformance green (verified=92 reject=132 failures=0); 2: 42/42 XFAIL
  for their declared diagnostic, 0 owed to M2; 3: 38 binaries 1177 passed 0 failed 43 ignored; last line
  `M2_EXIT_RESULT 2 NO_VERDICT` (r7/m2-exit-r7.log).
- Tip a15adc7 (rebased on 2ca5eb5) + post-rebase edits UNCOMMITTED (51 files: PD0082 rename, counts, pin map,
  refusal set, citations/pins, ExpansionGuard Clone, n4 test, link-refusal removes C, linker note).
  Scratch worktrees in 89-work/r7: red76 (76734e5, RED) and main2ca (2ca5eb5, old side) — detached, left in place.
- Full gates launched DETACHED via gates-locked.sh -> 89-work/gates-r7.log (not awaited).

## Round 8 (b442a21 = a15adc7 + coordinator's commit of the 51 post-rebase edits; astra REJECT: name collision)
- gates-r7 stopped: it HELD the lock (in make gates); process group TERMed, stale lock moved to .trash/gates.lock.89-r7-stopped.
- Class: a SOURCE identifier can equal a GENERATED C name. Inventory (codegen literals): every invented name begins
  with `_` — __pd_val/cl/cr/lo/hi/k/n/done/r<n>, _match_expr, _match_end<n>, _i, __pd_judge_<item>, __E__V tag
  constants, __pd_T_m methods/imports, __pd_tuple*, __pd_range, runtime __pd_* — none with `_u_`; derived names
  (`same__i64`, `Box_i64`) collide LOUDLY on main and here (gcc redefinition) -> found/mangled_name_collides_with_user_name.
- Measured on main 2ca5eb5 (r8/coll, 19 probes): SILENT wrong already on main — `_match_expr` 3 (want 5), `_i` 1 3
  (8 9), `__pd_hi0` 2 3 (9 10), inclusive temps 1 3 (26 27), `__pd_val0` 0 (7, uninitialised self-assign),
  `__C__R` local picked the wrong arm (1 1 vs 2 1); gcc failures on main: `__pd_print_int`/`__pd_P_m`/`__pd_r0`/
  `__pd_range`+`__pd_tuple2`/fn `_match_expr`+`_i`+`__pd_argc`. b442a21 added astra's four (cl0 right 1, cl0 left /
  cr0 / judge_A gcc failures).
- Fix: `c_ident::c_name` = c_ident then, for a name beginning with `_` (not bare `_`), `_u` in front; used by
  `escape_in_place` and both generic-template names (every ingress). Injective (proof in the doc); images `_u_…`.
  Comment at fresh_hoist_name made true (2 lines, no citation moved). m1 keyword test: underscore names expect `_u`.
- Tests: c_ident unit `a_source_name_never_lands_where_code_generation_names_things`, `c_name_is_injective`;
  m2 `a_program_may_name_anything_code_generation_invents` (18 programs, every pattern). RED on b442a21
  (r8/red-r8.log: astra's first case printed 1, want 0); GREEN. Full suite 1180 passed 0 failed (43 ignored).
- old-vs-new vs main 2ca5eb5 over 1327 (1249 + 78 new: astra/fable r7 probes + 18 own): 863 same, 446 refused
  PD0082 (307 ran on main — the 299 of r7 + 8 r7 panel probes: `as bool` on a reference/generic String, String
  range/literal patterns), 7 newly running (5 own collision probes that failed in gcc on main + the known 2), 11
  changed = 5 cost_many_inst (main load timeouts, rc 3 re-timed in r7) + 6 own collision probes where main's answer
  was WRONG (now right). astra's four collision probes print main's output. r7 vs r8 on the shared 1249: identical.
  Tree sweep: 0 verdict changes vs r7.
- checks: diag codes green (conformance verified=92 reject=132 failures=0); ledger CLEAR; doc evidence --update
  --allow-repin (nothing moved) then plain OK. Committed 24c1e9b (both Co-Authored-By lines).
- gates launched DETACHED via gates-locked.sh -> 89-work/gates-r8.log (not awaited).

## Round 9 (24c1e9b; astra: the `_u` rename lost generic runs — `id<_S>`, `id<_E>`, `Box{v:_S{..}}`, imported `id<_S>`)
- gates-r8 stopped while it waited for the lock (process group TERMed; it never held the lock).
- Cause: two side channels from the type checker carry SOURCE names as strings beside the escaped template:
  generic instantiations' type_args (fn and struct) and StructLiteralRecords; monomorphisation substituted the raw
  `_S` into the escaped template (`struct _u_S` declared, `struct _S` used) and the literal matching compared them.
- Enumerated entry points of source names into emitted C: escaped AST at 4 ingresses (program, imported modules,
  generic fn/struct templates) — names, fields, variants/payload members, methods, aliases, statics, macros, module
  items (astra r8 controls keep main's output); PLUS the two string channels above, now escaped at the same ingress
  by `c_ident::escape_type_argument` (c_name on every user-name token; primitives/sizes/`?` untouched), applied
  once to fn type_args, struct type_args and the records (`StructLiteralRecords::with_type_args_mapped`), so they
  compare and mangle (`first___u_S__u_E`) in one spelling, never escaped twice. Runtime/builtins are not source
  names (PD0019 forbids defining them). PD0082 display: spelled_c_fn strips the `_u` prefix. c_name doc corrected
  (codegen's own diagnostics show C type spellings). Traits emit no code (`self` in a trait is a parse error, main too).
- Tests: c_ident `a_type_argument_spells_its_user_names_as_code_generation_does`; m2
  `a_generic_instantiated_at_an_underscore_name_runs` (10 programs) and `an_imported_underscore_item_and_generic_run`
  (two modules). RED on 24c1e9b (r9/red-r9.log: both FAILED); GREEN. Full suite 1183 passed 0 failed 43 ignored.
- old-vs-new vs main 2ca5eb5 over 1351 (1327 + 24 r9 probes incl. astra r8): 887 same, 446 refused (307 ran on
  main — unchanged from r8), 7 newly running + 11 changed — exactly r8's (5 gcc-failing collision probes + 2 known;
  6 collision probes where main was wrong + 5 cost_many_inst load timeouts). All 24 r9 probes = main.
  r8 vs r9: exactly 7 programs differ (the underscore-generic ones: r8 failed, r9 prints main's output). Tree sweep
  r8 vs r9: 0 changes. Generic forms that fail on main (Box<Box<_S>>, id(Box<_S>), id(&_S)) fail identically.
- Feature-index `cmd:` rows grep src/ for "side channel"/"hygien" (features absent): my comments reworded.
- checks: diag codes green (conformance verified=92 reject=132 failures=0); ledger CLEAR; doc evidence --update
  --allow-repin then plain OK (pins unchanged). Committed 56ead94 (both Co-Authored-By lines).
- gates launched DETACHED via gates-locked.sh -> 89-work/gates-r9.log (not awaited).

## Round 10 (56ead94; astra R9: composed C names meet source names — `id___u_S`, `Box__u_S`)
- gates-r9 stopped during prebuild (never held the lock). Two host restarts interrupted (build, then selfhost).
- Fix (uncommitted, worktree): `c_ident::COMPOSED_PREFIX` = `_g_` on every name composed from source names:
  `_g_<f>__<args>` (monomorphize_function + mangle_generic_name), `_g_<S>_<args>` (monomorphize_struct),
  `_g_<E>_<V>` / `_g_<E>_<V>__new` (variant constructors, definitions + calls), `_g_<E>__<V>_Data` (payload
  typedefs, declarations + union members), `_g_<E>Tag` (tag type, typedef + struct member). Already outside every
  source spelling: `__pd_<T>_<m>` methods/static calls, `__pd_<mod>_<fn>`, `__pd_tuple…` (+`_new`), `__pd_judge_*`,
  tag constants `__<E>__<V>`. Methods kept under `__pd_` (moving them to `_g_` would let a method meet an enum
  constructor, which main cannot). `shown_c_fn` strips `_g_`/`_u` for the PD0082 marker and the match trap.
- Probes (r10/p) main / r9 / r10: astra's 3 = 7, 7, 7 9 on main and r10 (r9 gcc failures); main's own loud
  collisions now run: same__i64 beside same<T> -> 1 3, Box_i64 -> 3, E_V/E_W__new/E__W_Data/ETag -> 15;
  method compositions 301; a source name shaped `_g_…` -> 5.
- Test `a_composed_c_name_never_meets_a_source_name` (8 programs): RED on 56ead94 (r10/red-r10.log), GREEN.
  Updated C-text asserts: lib `_g_N_A__new(5LL)`, m2_enum_composites `typedef struct _g_G_bool {`. Full suite:
  1183 passed + the one updated test (m2_enum_composites 43/43 on rerun), 0 other failures.
- old-vs-new vs main 2ca5eb5 over 1393 (1351 + 42 new: astra r9 probes + 8 own): 925 same, 446 refused (307 ran on
  main, unchanged), 12 newly running (r9's 7 + 4 own composition probes main failed in gcc + astra
  keyword_struct_arg, main gcc failure -> 7), 10 changed = 6 r8 collision probes where main was WRONG + 4
  cost_many_inst load timeouts (rc 3 re-timed). r9 vs r10: exactly astra's 3 image collisions + 3 own (r9 gcc
  failure -> main's output / the right answer) + astra_r4 cost_many_inst load timeout (re-timed 150s: rc 3 = main).
  Tree sweep r9 vs r10: 0 changes. Selfhost fixed point e8bd8cda unchanged (bootstrap C spelling moves, its
  output does not).
- Residual (composed vs composed, loud, as on main): two compositions of different source names can still meet
  (e.g. method `__pd_<T>_<m>` vs a runtime `__pd_*`, or `_g_<E>_<V>` vs `_g_<f>__<args>` shapes) — the prefix
  adds no such meeting and removes every composed/source meeting.
- checks: diag codes green (conformance verified=92 reject=132 failures=0); ledger CLEAR; citations unmoved;
  doc evidence --update --allow-repin then plain OK. Committed d1de684 (both Co-Authored-By lines).
- gates launched DETACHED via gates-locked.sh -> 89-work/gates-r10.log (not awaited).

## Round 11 (d1de684; astra REJECT r10: `_`-joined compositions are not injective; fable addendum: judge meets method)
- CORRECTION of round 10: "two composed names can still meet exactly as they did on main / the prefix adds no
  such meeting" was FALSE. With `_u` inside the components, `_`-joins met where main's did not: `f<_S>`/`f_<u_S>`
  -> `_g_f___u_S`; `B<_S>`/`B_<u_S>`; `E::_V`/`E_::u_V` (tag constant + ctor); methods `E::_m`/`E_::u_m` ->
  `__pd_E__u_m` (astra r10, main 1 2 / 7 8 / 1 2 / 1 2). And the round-6 judge `__pd_judge_K` met method `judge::K`
  (fable: main 3 1 / 4 1 / 7 1). gates-r10 was stopped by the coordinator.
- Fix: ONE encoder `c_ident::composed(family, components)` = `_g_` + family letter + per component `<len>_<enc>`
  (enc: ASCII alnum kept, `_` -> `__`, any other byte -> `_xx` hex) with an exact inverse `decode_composed`.
  Families at definition AND use: f generic fn instance (`instance_name`), s generic struct instance, u unit ctor,
  n `__new` ctor, d payload struct (typedef + union member), t tag type, c tag constant (declaration, ctor init,
  pattern test), m method/static call path (`path_name`: definition, prototype, call), p tuple struct
  (`tuple_c_name`), q tuple ctor, j file-scope judge. Path idents escaped per segment (`escape_path_in_place`), so
  `E::_m` meets its definition `_u_m`. Display (`shown_c_fn`) decodes: f -> `f__args` (old spelling, messages
  unchanged), m -> `T::m`, `_u` stripped.
- Measured: qualified import calls `lib::f()` parse as enum constructors (`lib_f__new`) and fail in gcc on main too;
  nested generic struct instances (`B<B<_S>>`) are refused on main too (type args with `<>` are encoded anyway).
- Tests: c_ident `composed_names_decode_to_their_components_and_never_meet` (11 families x ~1200 adversarial
  sequences: decode round-trip + no two equal + not a source image); m2 `composed_names_meet_nothing_main_kept_apart`
  (13 run rows: astra's 4, fable's 3 judge programs, 5 composed-vs-composed pairs loud on main, method vs runtime
  `string_eq`); round-10 rows kept. C-text asserts re-derived from the encoder (cct enums/structs, value_form x3,
  lib x3, enum_composites, m3 `_g_f2_id`, m1 payload-member scan).
- RED on d1de684 (r11/red-r11.log): `composed_names_meet_nothing_main_kept_apart` FAILED at astra's first pair; GREEN.
  Full suite: 5 failures, all pinning old composed spellings or the lossy tuple naming, fixed honestly:
  tuple_shape_tests (`the_mangling_can_collide` said "if these ever differ … delete the refusal": done —
  `the_tuple_name_is_injective`, `two_layouts_that_used_to_collide_both_register`; register_tuple's refusal
  replaced by a debug_assert), m2 judge name via `composed('j')`, m3 instance count via `instance_name`, n4 compares
  with composed names decoded before renaming `Kind` -> `K` (`with_composed_names_decoded`). All re-run green.
- old-vs-new vs main 2ca5eb5 over 1459 (1393 + 66 new: astra r10 + fable r10/r10b/r10c probes): 976 same, 447
  refused (307 ran on main, unchanged; +1 = fable msg_spelling_us, a struct `==` main failed in gcc, now PD0082
  naming `_S`), 25 newly running (all main gcc/refusal failures, now right: astra's enum_old_sources/keyword
  args, fable's loud pairs cc_fn/cc_enum/cc_method/cc_struct, loud_same_i64/box_i64/enum names, rt_string_eq
  method, tag_const meetings, own r8/r10 probes, the known 2), 11 changed = 6 r8 collision probes where main was
  WRONG + 5 cost_many_inst load timeouts. astra's 4 r10 pairs and fable's 3 judge programs print main's output.
  r10 vs r11 on the shared 1393: only cost_many_inst timing. Tree sweep: 0 changes. ns_source_shapes fails on both
  (several `id` instances resolve to the first — the known XFAIL in m3). Selfhost e8bd8cda unchanged.
- language-spec's "methods become mangled free functions `__pd_Type_method`" corrected (path_name). 54 citations
  re-aimed by content (renum vs d1de684); 2 MOVED pins read (the corrected claim; an inner-citation-only change).
- checks: diag codes green (conformance verified=92 reject=132 failures=0); ledger CLEAR; doc evidence --update
  --allow-repin then plain OK; feature-evidence greps clean. Committed 77c4dd7 (both Co-Authored-By lines).
- gates launched DETACHED via gates-locked.sh -> 89-work/gates-r11.log (not awaited).

## Round 12 (77c4dd7; grok APPROVE; astra REJECT: exponential C for nested tuples)
- Cause: tuple_c_name passes inner composed names (`struct _g_p…`) back into composed('p'), and round 11's
  encode_component doubled every `_` again per level: 23 nested `(v, 2)` tuples -> 599,832,240 bytes of C
  (main 52,129). gates-r11 stopped by the coordinator.
- Fix (c_ident): a component is a sequence of RUNS ended by `z`: identifier-character run copied VERBATIM as
  `<len>_<run>`, any other run as `<len>x<hex>`; decode_component reads digits, `_`/`x`, the run, then a digit or
  `z` — exact inverse, so composed stays injective; an embedded inner name appears once -> linear growth.
- Measured (r12/depth, main / r12 C bytes, both print 2): depth 8 15,454/16,786; 13 24,179/27,681; 16
  31,094/36,402; 17 33,679/39,673; 21 45,419/54,577; 23 52,129/63,121 (r11: 599,832,240).
- Tests: c_ident injectivity set extended (`z`, `1z`, `x`, `1x`, `_z`, embedded composed names, `struct <composed>`);
  new `a_nested_composed_name_grows_by_a_constant_per_level` (23 nested tuple names and 23 nested generic instance
  names: each level <= 40 bytes); m2 `nested_tuples_emit_c_of_mains_order` (depth 8/16/23 C size bounds, runs 2).
  Round-10/11 run rows pass.
- RED on 77c4dd7: unit growth test FAILED (name lengths 32, 68, 116, … 100,663,942 at depth 23 — doubling);
  integration `nested_tuples_emit_c_of_mains_order` FAILED at its size bound (r12/red-r12-int.log). GREEN here.
- Full suite (r12/t-all-run1.log, the run that outlived the host restart): 38 binaries, 1188 passed, 0 failed, 43 ignored. The duplicate relaunch was stopped.
- fable addendum (every family): the growth test now runs five chains to depth 32 (tuple of tuple; generic struct
  of generic struct; generic fn at a nested tuple spelling; a generic instance whose type argument IS a composed
  name; tuple of a generic struct instance) — every level <= 48 bytes (measured ~12-13), every name decodes.
  Program level (r12/fam, main / r12): generic fn at a nested tuple and generic struct of a tuple are refused BY
  NAME on both; tuple of generic struct fails in gcc on both, C linear: depth 4 11,186/11,859; 8 15,730/17,691;
  16 31,538/38,091; 32 90,034/113,835 (ratio <= 1.27).
- old-vs-new vs main 2ca5eb5 over 1479 (1459 + 20 astra r11 probes), 60 s per program (r12/runall7.sh also records
  emitted C bytes): 1000 same, 447 refused (307 ran on main, unchanged), 26 newly running (all main failures, now
  right), 6 changed = the r8 collision probes where main was WRONG; 0 lost correct runs; no load timeouts at 60 s.
  Tree sweep 413: verdicts as round 11.
- C size vs main, 747 programs emitting C on both: mean ratio 1.077, min 1.000, max 3.957 — every max is a
  cost_many_inst variant (179,235 -> 709,151 bytes; gcc fails on both): identical line counts (8,323), the growth
  is 3,162 PD0082 checks (~170 bytes each, one per comparison; main emits none) in a program of 3,000 comparisons —
  linear, the check's constant cost, not name growth. Next: the 64/128-deep comparison chains (2.4-3.7, the
  statement-expression check per level, linear since round 7).
- language-spec example name re-derived (`_g_m4_Rectz4_areaz`); citations unmoved; selfhost e8bd8cda.
- checks: diag codes green (conformance verified=92 reject=132 failures=0); ledger CLEAR; doc evidence --update
  --allow-repin then plain OK. Committed 7149822 (Co-Authored-By Zhuge + Claude). main is now f2d1dd3 (#84) — not
  rebased, per the coordinator.
- gates launched DETACHED via gates-locked.sh -> 89-work/gates-r12.log (not awaited).
