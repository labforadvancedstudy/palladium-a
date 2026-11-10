# Round 12 — issues #89 + #92. Tip 7149822 (local commit) = 77c4dd7 + round-11 fix

Same rules as before ($W/iss/review-rules.md).
Round 11 (77c4dd7): grok APPROVE (encoder injective; all 7 collision programs run); astra REJECT and fable REJECT — EXPONENTIAL C for
nested tuples: tuple_c_name passed inner composed names back into composed('p') and encode_component doubled every `_` per level —
23 nested `(v, 2)` tuples (149-byte source) → 599,832,240 bytes of C (main 52,129). Evidence 89-work/panel/astra/r11/tuple_growth*.log.
fable: no other finding in R11 (442 probes); check the same re-encoding in every family.
Round-12 diff only: $W/iss/89-work/r12.diff (77c4dd7..7149822, 3 files: c_ident.rs,
tests/m2_comparison_operands.rs, spec example name).
Snapshot worktree (READ ONLY): $W/iss/89-rev (= 7149822). Compiler built from
that exact tree: .../iss/89-work/pdc-tip. main's compiler (2ca5eb5): .../iss/93-gates/target/release/pdc. (main is now f2d1dd3 = #84
landed; this unit rebases after approval.) Full gates on the exact tree: .../89-work/gates-r12.log (running).
Coder's claims (.../89-work/LEDGER.md "Round 12"):
- A component is a sequence of RUNS ended by `z`: an identifier-character run copied VERBATIM as `<len>_<run>`, any other run as
  `<len>x<hex>`; decode_component reads digits, `_`/`x`, the run, then a digit or `z` — exact inverse (injective); an embedded inner
  name appears once → linear growth.
- Nested tuples main / r12 C bytes (both print 2): depth 8 15,454/16,786; 16 31,094/36,402; 23 52,129/63,121 (r11 599,832,240).
- Tests: injectivity set extended (`z`, `1z`, `x`, `1x`, `_z`, embedded composed names, `struct <composed>`); growth test over five
  chains to depth 32 (tuple of tuple; generic struct of generic struct; generic fn at a nested tuple; a generic instance whose type
  argument IS a composed name; tuple of a generic instance): every level ≤ 48 bytes (measured ~12), every name decodes; program test
  `nested_tuples_emit_c_of_mains_order` (depth 8/16/23 size bounds, prints 2). Both RED on 77c4dd7. Full suite 1188 passed; selfhost
  e8bd8cda.
- Old-vs-new vs 2ca5eb5 over 1479: 1000 same, 447 PD0082 (307 ran on main, unchanged), 26 newly running (main failures, now right), 6
  changed = r8 collision probes where main was wrong; 0 lost correct runs. C size vs main over 747 programs: mean 1.077, max 3.96 —
  cost_many_inst variants (gcc fails on both; same line count; +3,162 PD0082 checks ~170 B each, one per comparison: linear).
Focus: (1) astra + fable — re-run tuple_growth at depth 8..32 and every family chain (generic struct of generic struct, generic fn at
a nested tuple/struct, enum payloads, methods of generic instances, arrays of tuples, references to tuples); name length per level
must be bounded by a constant; decode ambiguity of the new run scheme (`z`/`x`/digits inside runs, empty components, a run of digits
after a length, non-ASCII); C identifier validity; any lost correct run; selfhost. Same answer contract.
