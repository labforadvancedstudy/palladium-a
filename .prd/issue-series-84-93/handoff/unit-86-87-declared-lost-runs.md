# 유닛 #86 #87 — 선언된 "잃은 실행" 목록 (라운드 7)

````text
# Declared lost runs after round 7: programs main 2ca5eb5 RAN that the round-7 tree refuses by name (PD0082).
# None changed by round 7: every row below is refused by the round-6 tip 3c86348 too (467: r4/sw/r6; extras: r7/sw/tip; class sweep: r7/run/tip).
# Categories (ledger steps 20, 33, 37, 41): record keeps no integer width / reference (round 2); erased-width arithmetic (round 4); binding exact only at its declared C type (round 5); unrecorded use whose C type two kinds share (int: bool|i32, long long: i64|char).
## 467 + extras (big.list)
148	86/tests/reject/generic_call_not_held_exactly.pd
150	86/tests/reject/generic_literal_not_held_exactly.pd
233	86-work/coll/own2.pd
235	86-work/coll/par.pd
241	86-work/probes/b1_body_u32.pd
243	86-work/probes/b3_unrec.pd
244	86-work/probes/b4_imported_main.pd
249	86-work/probes/l1_decoy.pd
250	86-work/probes/l1_nodecoy.pd
251	86-work/probes/l1_toplevel.pd
257	86-work/probes/p86.pd
258	86-work/probes/p86b.pd
259	86-work/probes/p87.pd
261	86-work/probes/p87_meet.pd
262	86-work/probes/p87_toplevel.pd
277	86-work/probes2/c6_array.pd
283	86-work/probes4/b5_field_i64.pd
284	86-work/probes4/b6_field_bool.pd
285	86-work/probes4/b7_call_arg.pd
286	86-work/probes4/b8_nested.pd
288	86-work/probes5/m2.pd
294	86-work/probes7/b3_unrec.pd
295	86-work/probes7/b6_field_bool.pd
296	86-work/probes7/p87.pd
297	86-work/probes7/z86.pd
299	86-work/probes7/zbodyalias.pd
302	86-work/probes7/zshape.pd
305	86-work/probes7/zwidth2.pd
306	86-work/probes7/zwidth3.pd
307	86-work/probes7/zwidth4.pd
315	86-work/matrix/i32_val_free_none/main.pd
317	86-work/matrix/i32_val_free_bool/main.pd
320	86-work/matrix/i32_val_body_bool/main.pd
321	86-work/matrix/i32_val_imported_none/main.pd
323	86-work/matrix/i32_val_imported_bool/main.pd
339	86-work/matrix/u32_val_free_none/main.pd
341	86-work/matrix/u32_val_free_bool/main.pd
344	86-work/matrix/u32_val_body_bool/main.pd
345	86-work/matrix/u32_val_imported_none/main.pd
347	86-work/matrix/u32_val_imported_bool/main.pd
363	86-work/matrix/u64_val_free_none/main.pd
365	86-work/matrix/u64_val_free_bool/main.pd
368	86-work/matrix/u64_val_body_bool/main.pd
369	86-work/matrix/u64_val_imported_none/main.pd
371	86-work/matrix/u64_val_imported_bool/main.pd
411	86-work/r2probes/a02_u64_small_nodecoy.pd
412	86-work/r2probes/a03_i32_bool_decoy_overflow.pd
415	86-work/r2probes/a05b_u32_dec_nodecoy.pd
416	86-work/r2probes/a06_asymmetry.pd
419	86-work/r2probes/a08_i32neg_u64decoy.pd
423	86-work/r2probes/a11b_imported_u64_nodecoy.pd
425	86-work/r2probes/a13_i32neg_u32decoy.pd
432	86-work/r2probes/a22_toplevel_literal_decoy.pd
442	86-work/r2probes/ar2_b3_unrec.pd
443	86-work/r2probes/ar2_b6_field_bool.pd
450	86-work/r2probes/ar2_c_literal_seed.pd
451	86-work/r2probes/ar2_p87.pd
452	86-work/r2probes/ar2_z86.pd
455	86-work/r2probes/ar2_zwidth4.pd
460	86-work/r2probes/disp_p87.pd
461	86-work/r2probes/disp_top.pd
463	86-work/r2probes/p87.pd
464	86-work/r2probes/top.pd
469	86-work/r3/all/block_u64_call_body.pd
470	86-work/r3/all/block_u64_param_lit.pd
472	86-work/r3/all/c17_body_div_after_underflow_z_i64.pd
473	86-work/r3/all/c20_body_let_then_call.pd
474	86-work/r3/all/c22_toplevel_let_adopt_decoy.pd
475	86-work/r3/all/c23_toplevel_let_adopt_nodecoy.pd
476	86-work/r3/all/c24_toplevel_let_mul_decoy.pd
481	86-work/r3/all/exact_if_call.pd
487	86-work/r3/all/generic_exact_type_of_a_value.pd
488	86-work/r3/all/half_mul.pd
489	86-work/r3/all/if_i64_call_body.pd
490	86-work/r3/all/if_mixed_call_body.pd
491	86-work/r3/all/if_u32_call_body.pd
492	86-work/r3/all/if_u32_literal.pd
493	86-work/r3/all/if_u32_mixed.pd
500	86-work/r3/all/mix_lit_body.pd
502	86-work/r3/all/p87_vplus0.pd
505	86-work/r3/all/scope_keep_u32.pd
506	86-work/r3/all/scope_keep_u32_tilde.pd
508	86-work/r3/all/scope_remove.pd
512	86-work/r2final/adv2/f2_p87_plus0.pd
514	86-work/r2final/adv2/f4_top_lit_mul.pd
515	86-work/r2final/adv2/f5_top_lit_shift.pd
516	86-work/r2final/adv2/f6_top_lit_paren.pd
518	86-work/r2final/adv2/f8_u64_lit_plus0.pd
519	86-work/r2final/adv3/g1_onepv.pd
520	86-work/r2final/adv3/g2_letlit.pd
521	86-work/r2final/adv3/g3_letcall.pd
522	86-work/r2final/adv3/g4_mul.pd
525	86-work/r2final/mainfx/enum_in_composites.pd
530	86-work/r2final/adv/e5_lit_plus0.pd
532	86-work/r4/allp/array_erased.pd
533	86-work/r4/allp/array_loop_erased.pd
534	86-work/r4/allp/for_range_u32.pd
535	86-work/r4/allp/generic_exact_type_of_a_value.pd
536	86-work/r4/allp/i32_plus_lit.pd
540	86-work/r4/allp/i64_tuple.pd
541	86-work/r4/allp/let_u32_param.pd
542	86-work/r4/allp/let_u64_param.pd
543	86-work/r4/allp/match_u32_param.pd
544	86-work/r4/allp/range_value_u32.pd
545	86-work/r4/allp/tuple_erased.pd
548	86-work/r5/allp/assign_compound.pd
549	86-work/r5/allp/assign_if.pd
550	86-work/r5/allp/assign_mut.pd
551	86-work/r5/allp/d01_let_param_then_mutate.pd
552	86-work/r5/allp/d02_let_param_then_inc.pd
556	86-work/r5/allp/d06_match_binding_chain.pd
557	86-work/r5/allp/d07_for_over_array_of_param.pd
559	86-work/r5/allp/d09_if_join_of_param.pd
560	86-work/r5/allp/d10_inner_scope_no_leak.pd
562	86-work/r5/allp/d12_reassign_same_param.pd
563	86-work/r5/allp/d13_let_param_then_add_param.pd
566	86-work/r6/allp/array_join_param.pd
573	86-work/r6/allp/ref_let_copy.pd
580	86-work/r6/allp/tuple_param.pd
581	86-work/r6/allp/tuple_pattern.pd
586	86-zhuge-r6/comparison.pd
613	86-zhuge-r6/result_if_u64.pd
635	86-zhuge/r6/allsrc/b09_two_aliases.pd
652	86-zhuge/r6/allsrc/c06_body_arith_wrap_zonly.pd
653	86-zhuge/r6/allsrc/c07_body_literal_adopt.pd
654	86-zhuge/r6/allsrc/c08_body_mixed_u32_i64.pd
655	86-zhuge/r6/allsrc/c09_body_mul_overflow.pd
656	86-zhuge/r6/allsrc/c10_body_cmp_shared.pd
659	86-zhuge/r6/allsrc/c13_body_literal_index.pd
661	86-zhuge/r6/allsrc/c15_body_mul_invert.pd
663	86-zhuge/r6/allsrc/c18_body_half_div_after_underflow.pd
664	86-zhuge/r6/allsrc/c19_body_literal_div_after_underflow.pd
668	86-zhuge/r6/allsrc/d16_let_add_consistency.pd
669	86-zhuge/r6/allsrc/d17_let_add_lit_overflow.pd
672	86-zhuge/r6/allsrc/d20_u32_param_in_generic_body.pd
677	86-zhuge/r6/allsrc/d25_body_compound_param_local.pd
678	86-zhuge/r6/allsrc/d26_body_compound_wrapping_sum.pd
679	86-zhuge/r6/allsrc/d27_body_compound_i64.pd
686	86-zhuge/r6/allsrc/z09_import.pd
691	86-zhuge/r6/allsrc/z14_b6_field.pd
709	86-zhuge/r6/rsrc/r10_let_of_ref.pd
710	86-zhuge/r6/rsrc/r12_imported_ref_body.pd
712	86-zhuge/r6/rsrc/r17_match_binding_of_ref.pd
713	86-zhuge/r6/rsrc/r19_ref_u64_nodecoy.pd
720	86-zhuge/r6/ssrc/s01_field_through_ref_body.pd
726	86-zhuge/r6/ssrc/s08_cmp_ref_body.pd
732	86-zhuge/r6/ssrc/s16_cast_body.pd
733	86-zhuge/r6/ssrc/s17_neg_ref_i32_body.pd
736	86-zhuge/r6/ssrc/s21_let_copies_of_ref_body.pd
## r7 class sweep (gen+gen2)
D_i32_alias_same
D_i32_same
D_u32_alias_same
D_u32_same
D_u64_alias_same
D_u64_same
GM_char
GM_i32
GM_u32
GM_u64
G_u64_alias_same
G_u64_same
KT_i32
KT_u32
KT_u64
## fable r6 optional (s08 same(p < 3), s16 inv(p as i64), s17 half(-p)): NOT relaxed, stay declared (unrecorded call; the argument's KIND from its form is not mapped to the candidate's type argument)
````
