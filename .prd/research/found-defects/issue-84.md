# 범위 밖 결함의 재현 — 유닛 #84 (튜플 참조 파라미터)

유닛 작업 중 코더가 측정해 남긴 것이다. 고치지 않았고 이슈로 등록하지도 않았다. 파일 32개(노트 1, 프로그램 31)를 한 문서로 묶었다.

## 노트: README.txt

````text
Defects of OTHER classes found while measuring #84. Not fixed here. Every verdict measured with main's pdc (d5166e8,
gates-rel) unless noted; line numbers are d5166e8's.

f1  `*p` on a reference parameter (read or write) is emitted `(*((*p)))` / `*((*p)) =`: Expr::Ident already reads a pointer
    param as `(*p)` and the Deref arms wrap it again. gcc "indirection requires pointer operand" for &i64, &P (every non-array,
    non-tuple referent). src/codegen/mod.rs:6552 (Expr::Deref), :3924 (AssignTarget::Deref). #84 fixes only the tuple referent.
f2  Member access through a POINTER-VALUED binding — a local `let r = &t;`, or a parameter typed by a reference alias kept by
    name (`type A = &(i64, i64)`, W2a review round 7) — emits `.`, gcc "member reference type ... is a pointer".
    Tuples: src/codegen/mod.rs:5925 (TupleIndex). Structs: :6394 (FieldAccess, `->` only for mutable_params). Main and cf3f6c4 alike.
    Two more faces of the same pointer-valued-binding class (panel round 1, fable):
    p29 `let r = &t; f(r)` into a `&` parameter: the call site takes `&r` again -> "internal compiler error: the C backend
        emitted ill-typed C" (rc 4). Pre-existing on main for `&P` and `&i64` (f2_p29_struct_*, f2_p29_i64_*: same ICE).
    p40 a tuple-ref parameter beside `-> &P` with `return q;`: emitted `return (*q);` for a `struct P*` result -> gcc. The
        reference-RESULT class: main gives the same gcc error with no tuple in sight (f2_p40_struct_ref_result_main.pd), and
        tests/m2_enum_composites.rs pins its `&i64` twin as "GCC (gcc on main)".
    Both were named refusals on main only because the tuple parameter refused the whole function first.
f3  RAN, silent: a shared borrow `&s` handed to a `&mut` parameter writes an immutable binding (struct prints 7, array prints 7);
    and `let r = &t; *r = (9, 9);` writes immutable `t` (prints 9). Mutability of a reference EXPRESSION is never checked
    against the parameter / target (borrow_checker.rs:772 Deref target takes the place, no mutability check). Tuples inherit it
    after #84 exactly as structs/arrays have it -- EXCEPT on the reference-to-tuple PARAMETER path, which review round 1
    closed by name (check_tuple_reference_lending). Left, pre-existing and unchanged (same output on main and on the tip):
    a tuple reached through a reference alias kept by NAME, `type B = &mut (i64, i64); fn set(q: B)` given `&t` of an
    immutable `t` (prints 97 83; f3_tuple_shared_borrow_to_mut_reference_alias.pd), and the local `let r = &t; *r = ..` above.
f4  RAN, silent: `fn f(p: &P) { p.bump(); }` with `fn bump(&mut self)` mutates the caller's struct (prints 99). The receiver
    rule (typeck/mod.rs:3382) covers `self` only; the const struct P* reaches `struct P*` under the non-fatal
    -Wincompatible-pointer-types-discards-qualifiers (src/linker.rs). The tuple spelling `p.0.bump()` is refused (PD0027).
f5  RAN, silent: `let mut q = p; q.a = 7;` inside `fn set(p: &mut P)` copies the struct and loses the write (prints 3).
    The param is recorded as its referent type (codegen param record), and the type checker erases references.
    The tuple twin after #84 behaves the same (`let mut q = p; q = (97, 83);` leaves 11 29; astra's copy_mut_reference).
f6  A `let` in a nested block shadowing a pointer parameter keeps reading as `(*p)` / `p->` : mutable_params
    (src/codegen/mod.rs:3517) is function-wide while variables/array_bindings are scoped (open_binding_scope :3663). gcc.
    Same for `&(A, B)` params after #84 (probes/c/c11_nested_shadow.pd) and by-value `mut p: (A, B)` on main.
f7  A tuple holding an array as a LOCAL: tuple_definitions (src/codegen/mod.rs:5241) declares `long long[3] f0;`, gcc
    "brackets are not allowed here". #84 refuses it by name only behind a reference parameter.
f8  A tuple literal's C struct is named from its ELEMENTS' inferred C types, never from the type it is written into, and
    `*r = v` is not type-checked (src/typeck/mod.rs:3592 "TODO: Check that ptr_type is actually a reference to value_type").
    `let t: (i32, i64) = (1, 2);`, `v((1, 2))` for `(i32, i64)`, and `*r = (1, 2, 3, 4)` through a local `&mut` are all gcc
    on main. On #84's `*p = v` path the mismatch is refused by name (check_tuple_reference_write, round 1).
f9  A by-value struct/enum PARAMETER used as a tuple element is recorded as `P` (custom_c_type), so the literal's struct is
    `__pd_tuple2_P_long_long` where the declared tuple is `__pd_tuple2_struct_P_long_long`: gcc on main
    (f9_struct_param_as_tuple_element.pd). Refused by name on #84's `*p = v` path.
f3+ (panel round 2, fable a13) `fn f(p: &mut (i64, i64)) { let r = &p; *r = (97, 83); }` runs and prints 9783: a write through
    a SHARED local alias `&p`. The caller lent the tuple writable, so no shared tuple is written — but the alias is `&`, and the
    write through it is the unchecked local-alias write of f3 (`let r = &t; *r = ..` writes immutable t on main). Same class.
f3++ (panel round 3, astra) the NON-tuple twins of the local-reference write: `fn set(p: &mut P)` given `*r` with
    `let mut r: &P = &t;` of an immutable `t` runs and prints 97 83 on main and on the tip (f3_struct_mut_param_through_
    shared_local_ref.pd); the `&mut i64` twin is gcc on both (`*r` of a scalar local, f1). Left for the borrow checker; on the
    TUPLE path #84 opened, the origin must be proven (writable_origin, round 3).
f6+ (panel round 3, fable e18) `fn f(p: &(i64, i64)) { if true { let mut p = (1, 2); set(p); } }` is refused with a message
    that blames the shared PARAMETER: tuple_reference_params, like mutable_params, is function-wide while variables are
    scoped (f6). Refused by name, not wrong; the struct twin is gcc on main.
f6++ (review round 4, astra) the judgement face of f6: `let mut p = &u; set(*p);` inside `fn f(p: &mut (i64, i64))` was lent
    as the writable PARAMETER (writable_origin consulted tuple_reference_params first) and wrote the immutable `u` (97 83).
    Round 4 refuses, by name and at the binding, any `let` / `for` / pattern binding of a reference-to-tuple parameter's
    name (check_binding_keeps_tuple_reference_param): the emission of such a name stays the parameter's (f6, name-keyed and
    function-wide), so judging the local while emitting the parameter would trade a wrong write for gcc. main refused every
    such function (its parameter). f6 itself (non-tuple pointer params shadowed: gcc on main) is untouched.
f10 RAN, silent, on main 903ba02 and on the tip alike: `let x = 5; if true { let x = x + 1; print_int(x); }` prints -16
    (garbage), not 6. The shadowing `let` is emitted `long long x = (x + 1LL);` and in C the new `x` is in scope in its own
    initializer, so the initializer reads the uninitialized inner `x` (f10_shadow_init_reads_new.pd). Codegen also records
    the binding (variables, reference_locals) before generating the initializer, matching C rather than the source.
    src/codegen/mod.rs: Stmt::Let arm of generate_statement_body, the `"{} {}{} = "` declarator then generate_expression(value)
    (903ba02 :3861; round-4 tree :4978). Not #84's class: no reference, no tuple.
f10+ (review round 5, astra) f10 reached this unit's boundary: `let mut r = &mut *p; if true { let mut r = r; set(*r); }` —
    the binding pass resolved the inner initializer to the OUTER proven `r`, the C read the new uninitialized pointer, and
    the program died on signal 5 (rc 133) on f87fb3b; the by-value twin `m(*r)` dies on main too. Round 5 makes a `let`
    whose initializer names its own binding never proven (LetFacts::self_initialized), refused by name where it reaches a
    writable tuple parameter. f10 itself (any self-reading shadow, any type) is untouched and stays open.
f10 FIXED (review round 6, dispatcher's decision: fix the cause, not the boundary): a `let` whose initializer names the
    binding it declares (LetFacts::reads_own_name, by binding and span) is emitted `T __pd_shadowN = INIT; T NAME =
    __pd_shadowN;` (arrays: memcpy), INIT generated while codegen's maps still hold the outer binding; __pd_shadowN is
    checked against every identifier the function binds or names, every function and global. The round-5 refusal is gone.
    Measured (probes/r6, main 2ca5eb5 -> round 6): -16 5 -> 6 5; -16 0 5 -> 53 50 5; tuple swap 11 29 -> 29 11; struct
    -16 -16 -> 4 13; string crash -> abc abc; array -16 -16 -> 2 11; loop -16 x3 -> 7 8 9; param garbage -> 42; call
    garbage -> 6; reassigned 0 -> 12. Hoisted inits (if/block/match values), match binders and `for` were already right.
f10 hygiene (review round 7, astra): the round-6 temporary was checked against SOURCE names only; `enum __pd { shadow0 }`
    emits the constructor `__pd_shadow0()` and the local `long long __pd_shadow0` shadowed it (gcc). The lowering now
    writes a placeholder, named by name_shadow_temporaries after the whole C translation unit exists, against every
    identifier in that text (functions, instances, constructors, tags, structs, globals, runtime helpers called).
````

## f10_shadow_init_reads_new.pd

````text
fn main() { let x = 5; if true { let x = x + 1; print_int(x); } print_int(x); }
````

## f1_deref_of_reference_param_read.pd

````text
fn get(p: &i64) -> i64 { return *p + 1; }
fn main() { let x = 5; print_int(get(&x)); }
````

## f1_deref_of_reference_param_write.pd

````text
fn bump(p: &mut i64) { *p = *p + 1; }
fn main() { let mut x = 5; bump(&mut x); print_int(x); }
````

## f1_deref_of_struct_reference_param_write.pd

````text
struct P { a: i64, b: i64 }
fn set(p: &mut P) { *p = P { a: 8, b: 9 }; }
fn main() { let mut s = P { a: 3, b: 4 }; set(&mut s); print_int(s.a * 10 + s.b); }
````

## f1_i64_mut_param_through_shared_local_ref.pd

````text
fn set(p: &mut i64) { *p = 97; }
fn main() { let t = 11; let mut r = &t; set(*r); print_int(t); }
````

## f2_field_through_local_reference.pd

````text
struct P { a: i64, b: i64 }
fn main() { let s = P { a: 3, b: 4 }; let r = &s; print_int(r.a); }
````

## f2_field_through_reference_alias_param.pd

````text
struct P { a: i64, b: i64 }
type A = &P;
fn h(p: A) -> i64 { return p.a; }
fn main() { let s = P { a: 3, b: 4 }; print_int(h(&s)); }
````

## f2_p29_i64_local_reference_to_ref_param.pd

````text
fn f(p: &i64) -> i64 { return p + 1; }
fn main() { let x = 3; let r = &x; print_int(f(r)); }
````

## f2_p29_struct_local_reference_to_ref_param.pd

````text
struct P { a: i64 }
fn f(p: &P) -> i64 { return p.a; }
fn main() { let s = P { a: 3 }; let r = &s; print_int(f(r)); }
````

## f2_p29_tuple_local_reference_to_ref_param.pd

````text
fn f(p: &(i64, i64)) -> i64 { return p.0 * 10 + p.1; }
fn main() { let t = (3, 7); let r = &t; print_int(f(r)); }
````

## f2_p40_struct_ref_result_main.pd

````text
struct P { a: i64 }
fn f(q: &P, k: i64) -> &P { return q; }
fn main() { let s = P { a: 1 }; let r = f(&s, 2); print_int(0); }
````

## f2_p40_tuple_param_beside_struct_ref_result.pd

````text
struct P { a: i64 }
fn f(p: &(i64, i64), q: &P) -> &P { return q; }
fn main() { let t = (3, 7); let s = P { a: 1 }; let r = f(&t, &s); print_int(0); }
````

## f2_tuple_index_through_local_reference.pd

````text
fn main() { let t = (3, 4); let r = &t; print_int(r.0 * 10 + r.1); }
````

## f2_tuple_index_through_reference_alias_param.pd

````text
type A = &(i64, i64);
fn h(p: A) -> i64 { return p.0 * 10 + p.1; }
fn main() { let t = (3, 4); print_int(h(&t)); }
````

## f3_shared_borrow_to_mut_param_writes_immutable_array.pd

````text
fn set(p: &mut [i64; 2]) { p[0] = 7; }
fn main() { let s = [3, 4]; set(&s); print_int(s[0]); }
````

## f3_shared_borrow_to_mut_param_writes_immutable_struct.pd

````text
struct P { a: i64 }
fn set(p: &mut P) { p.a = 7; }
fn main() { let s = P { a: 3 }; set(&s); print_int(s.a); }
````

## f3_struct_mut_param_through_shared_local_ref.pd

````text
struct P { a: i64, b: i64 }
fn set(p: &mut P) { p.a = 97; p.b = 83; }
fn main() { let t = P { a: 11, b: 29 }; let mut r: &P = &t; set(*r); print_int(t.a); print_int(t.b); }
````

## f3_tuple_shared_borrow_to_mut_reference_alias.pd

````text
type B = &mut (i64, i64);
fn set(q: B) { *q = (97, 83); }
fn main() { let t = (11, 29); set(&t); print_int(t.0); print_int(t.1); }
````

## f3_write_through_shared_alias_of_mut_tuple_param.pd

````text
fn f(p: &mut (i64, i64)) { let r = &p; *r = (97, 83); }
fn main() { let mut t = (11, 29); f(&mut t); print_int(t.0 * 100 + t.1); }
````

## f3_write_through_shared_local_reference.pd

````text
fn main() { let t = (1, 2); let r = &t; *r = (9, 9); print_int(t.0); }
````

## f4_mut_method_through_shared_struct_param.pd

````text
struct P { a: i64 }
impl P { fn bump(&mut self) { self.a = 99; } }
fn f(p: &P) { p.bump(); }
fn main() { let s = P { a: 1 }; f(&s); print_int(s.a); }
````

## f5_copy_of_mut_reference_param_loses_write.pd

````text
struct P { a: i64 }
fn set(p: &mut P) { let mut q = p; q.a = 7; }
fn main() { let mut s = P { a: 3 }; set(&mut s); print_int(s.a); }
````

## f5_copy_of_mut_tuple_reference_param_loses_write.pd

````text
fn set(p: &mut (i64, i64)) { let mut q = p; q = (97, 83); }
fn main() { let mut t = (11, 29); set(&mut t); print_int(t.0); print_int(t.1); }
````

## f6_e18_nested_shadow_of_shared_param_blamed.pd

````text
fn set(p: &mut (i64, i64)) { *p = (97, 83); }
fn f(p: &(i64, i64)) { if true { let mut p = (1, 2); set(p); print_int(p.0); } }
fn main() { let t = (11, 29); f(&t); print_int(t.0); }
````

## f6_nested_let_shadows_pointer_param_i64.pd

````text
fn f(p: &i64) -> i64 { let mut s = 0; if true { let p = 5; s = p; } return s * 10 + p; }
fn main() { let x = 3; print_int(f(&x)); }
````

## f6_nested_let_shadows_pointer_param_struct.pd

````text
struct P { a: i64 }
fn f(p: &P) -> i64 { let mut s = 0; if true { let p = P { a: 5 }; s = p.a; } return s * 10 + p.a; }
fn main() { let x = P { a: 3 }; print_int(f(&x)); }
````

## f7_tuple_holding_array_local.pd

````text
fn main() { let t = ([1, 2, 3], 4); print_int(t.1); }
````

## f8_local_ref_write_other_shape.pd

````text
fn main() { let mut t = ((11, 29), (37, 41)); let r = &mut t; *r = (1, 2, 3, 4); print_int((t.0).0); }
````

## f8_tuple_literal_at_mixed_width_arg.pd

````text
fn v(t: (i32, i64)) -> i64 { return t.1; }
fn main() { print_int(v((1, 2))); }
````

## f8_tuple_literal_at_mixed_width_let.pd

````text
fn main() { let t: (i32, i64) = (1, 2); print_int(t.1); }
````

## f9_struct_param_as_tuple_element.pd

````text
struct P { a: i64 }
fn g(t: (P, i64)) -> i64 { return t.0.a + t.1; }
fn f(q: P) -> i64 { let t = (q, 1); return g(t); }
fn main() { let s = P { a: 9 }; print_int(f(s)); }
````

