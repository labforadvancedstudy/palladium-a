# Round 8 — issues #86 + #87: REBASE onto main f2d1dd3 (#84 landed) + one interaction fix. Tip d042e61

Same rules as before ($W/iss/review-rules.md).
Round 7 (2440dbc, on 2ca5eb5): grok, astra, fable APPROVE (unanimous). Then #84 landed (main = f2d1dd3): a self-reading `let` is lowered
through a temporary (named after the whole unit), tuple-reference parameters are pointers to the tuple's C struct, and a shared tuple
reference lent to a writer is refused (check_tuple_reference_lending). The unit was rebased: 8 commits, range-diff
.../iss/86-work/rb8-range-diff.txt (2ca5eb5..2440dbc vs f2d1dd3..d042e61). Conflicts: codegen `let` (kept both: #84's temporary block
first, then the exact-type block), BindingScope (+#84's reference_locals), bind_non_array now returns Result (`?`), citations/pins
(main's, relocated, re-pinned), counts re-derived 237/96/127.
INTERACTION FIX (a real code change, inside d042e61 — the replay of the round-7 commit; message not updated): instantiated_parameters
compared an argument the call passes BY ADDRESS (a place given to a reference parameter, reference_param_needs_address) with the POINTER
parameter. Effects: #84's m2_tuple_reference_params failed 2/21 (`set(*p, 97)` refused by PD0082 before #84's lending refusal);
`set(t, 97)` with `q: &mut (T, T)` RUNS on f2d1dd3 (97 97) but was refused; and `bump(x, 5)` with `q: &mut T`, `q = v` ran on BOTH
mains (5) but was already refused on 2440dbc — a lost run rounds 1–7 missed. Fix: a reference parameter given a place is held at its
referent's C type (fits / lossy_argument); #84's check_tuple_reference_lending runs for a generic template's call BEFORE
instantiation_called. Probes .../86-work/rb7/p/ (m1 `*q = v` gcc = both mains, also non-generic n1; m3 97 97 = f2d1dd3; m4; m5 u32
place into `&mut T` → PD0082 (f2d1dd3 ICE); m6 #84's refusal = f2d1dd3). New m4 test
a_reference_parameter_given_a_place_holds_the_place_at_its_referents_c_type.
Snapshot worktree (READ ONLY): $W/iss/86-rev (= d042e61). Compiler built from
that exact tree: .../iss/86-work/pdc-tip (the round-7 compiler is now .../86-work/pdc-tip-2440dbc). NEW main's compiler (f2d1dd3):
.../iss/86-work/mainf2-target/release/pdc (or build from .../86-work/mainf2). Full gates on the exact tree:
.../86-work/gates-rebased-r7.log (running).
Coder's claims (.../86-work/LEDGER.md steps 52–59):
- Behaviour check rebased vs 2440dbc over 1052 (467 + r7 fixture + 268 reviewer/ledger probes + #84's fixture + r7 class sweep 315):
  1047 same; 5 differ, all #84's tuple references now lowered (astra alias_tuple_ref/tuple_ref/tuple_ref_local and fable s04 refused →
  4294967288, the right u32 answer; #84's fixture refused → its transcript). Pre-fix vs fixed rebased: identical on all 1052.
- Tests: lib 599, m2_tuple_reference_params 21/21, m3 41, m4 15, m2_enum 43, m2_macro 7, m1 18, n4 18/6, d3b 45, d5 12. Conformance
  237/96/127; selfhost e8bd8cda; receipts green; ledger CLEAR; diag green. PD0082 unchanged.
Focus: (1) Judge the rebased unit against NEW main f2d1dd3: any program that runs correctly on f2d1dd3 and is refused/changed here
(especially #84's shapes: shadowing lets, tuple references, writes through `&mut`, lending); (2) the interaction fix — by-address
arguments to generic reference parameters at every width (`&mut T`/`&T`/`&mut (T,T)`/`&[T;N]`, u32/u64/i32/char/f64, aliases, places
that are fields/elements/derefs); could it pick a wrong instantiation (wrong value) anywhere?; (3) the conflict resolution in codegen
`let` (temporary block before the exact-type block) — exact types of a self-reading `let`. Same answer contract.
