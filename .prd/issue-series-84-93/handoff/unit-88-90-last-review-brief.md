# Round 7 — issues #88 + #90. Tip 6dade37 (local commit) = 5c20aec + round-6 fix

Same rules as before ($W/iss/review-rules.md).
Round 6 (5c20aec): grok APPROVE, fable APPROVE; astra REJECT — (1) LOST CORRECT RUN unknown_ref_known_annotation `let r:&i64=if true{p}
else{0+false};*r` main 5 → PD0022 "expected &i64, found &_"; (2) merging `&p` with an unknown branch made one `&_`, dropping the known
reference layer: `*rp == *rq` compared addresses (`ne`, + imported twin); (3) two UNRELATED unknown referents (`char *` from `s[0]`,
a recovered `long long *`) were one `&_` → assignment/if/array reached the ICE; (4) holds_unknown_value suppressed the whole composite
comparison: `z=(1,2); z=(c,p)` reached gcc. Evidence .../iss/88-zhuge-r6/.
Round-7 diff only: $W/iss/88-work/r7.diff (5c20aec..6dade37; src = typeck/mod.rs
(+builtins/llvm_text_backend citation hunks); tests m2_reference_types; 4 new fixtures + 1 re-pinned; manifest; pin map; counts/pins).
Snapshot worktree (READ ONLY): $W/iss/88-rev (= 6dade37). Compiler built from
that exact tree: .../iss/88-work/pdc-tip. main's compiler (2ca5eb5): .../iss/93-gates/target/release/pdc. (main is now f2d1dd3 = #84;
this unit rebases after approval.) Full gates on the exact tree: .../88-work/gates-r7.log (queued).
Coder's claims (.../88-work/LEDGER.md "ROUND 7"):
- value_of_parts: a value if/match/array is typed by its FIRST part as codegen declares it — first part known → the known parts
  merged, a part never typed (refused / unknown value) adds nothing; first part unknown → unknown_reference if a known part is a
  reference, else unknown. `&e` over an unknown keeps its one known layer.
- String index → `C char` marker (codegen's `char`): `let c = s[0]` is bound at C char; two C chars agree, nothing else does.
  unknown_reference agrees with NOTHING (not even another `&_`).
- Composites judged position by position (holds_unknown_value no longer suppresses the whole comparison).
- LEFT: generic return `& &T` with T unfixed → one layer; "no program in 913 reaches it".
- RED (red-r7.log): 10 rows differ; GREEN m2_reference_types 17/17 (needed --test-threads=2 under load; HANG rows under load were
  environmental). New fixtures: reference_beside_a_branch_never_typed_runs (RUN 5 5 5; main same; r6 refused),
  reference_compare_through_a_branch_never_typed (PD0083; main ne), reference_merge_of_two_referents_of_unknown_c_types (PD0058; main
  ICE), reference_in_a_tuple_beside_an_unknown_element (PD0022; main gcc). Conformance 251/94/143, coded 145; selfhost e8bd8cda.
- Old-vs-new vs main 2ca5eb5: 691 set — same 266, gcc/ICE→named 227, ran→refused 93, both 94, still gcc 11, 0 output changed, 0 newly
  running. vs r6: one change, a3__generic_tuple_ref_good refused (r6) → gcc as on main (`let a: (i64, &i64) = (true, &n)`: the
  mismatch is `true` vs `i64`, a reference-free class, found/). 85 r7 witnesses: same 30, gcc/ICE→named 29, ran→refused 13 (12 `ne`
  wrong + generic_ref_cast_good coincidental, listed), still gcc 3 (found/). vs r6 tip: the three r6 lost runs run again.
Focus: (1) astra — re-run your r6 witnesses; attack value_of_parts (first-part order dependence: a known part SECOND after an unknown
first part; a non-reference known part beside a reference; nested merges; match with many arms), the C char marker (other unknown
sources than String index: method/field of an unknown receiver, recovered calls), unknown_reference agreeing with nothing (any program
main ran correctly that now refuses?), composite position judgement (nested tuples/arrays/generic structs). (2) Any lost correct run.
(3) Cross-unit loop mirroring (unchanged). Same answer contract.
