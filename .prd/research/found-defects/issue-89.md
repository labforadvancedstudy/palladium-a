# 범위 밖 결함의 재현 — 유닛 #89 #92 (비교 피연산자)

유닛 작업 중 코더가 측정해 남긴 것이다. 고치지 않았고 이슈로 등록하지도 않았다. 파일 11개(노트 2, 프로그램 9)를 한 문서로 묶었다.

## 노트: mangled_name_collides_with_user_name.md

````text
# Monomorphisation names are not injective with user names — LOUD, unchanged by #89/#92

Class: name MANGLING (src/codegen/mod.rs: a generic fn instantiated at T is emitted `<name>__<T>`, a generic
struct `<Name>_<T>`), not generated temporaries. A program may define the mangled spelling itself:
  found/mangled_name_collides_with_user_name.pd (fn `same__i64` beside `same<T>` at i64)  and
  r8/mangle/struct_mangle.pd (struct `Box_i64` beside `Box<i64>`).
Both: gcc "redefinition of 'same__i64'" / "redefinition of 'Box_i64'", rc 3, on main 2ca5eb5 AND on #89's round 8.
Loud (never a wrong answer), so not the silent class round 8 closes; the round-8 hygiene (`c_ident::c_name`) moves
every source name beginning with `_` out of the generated namespace, and these names begin with a letter.
````

## 노트: optimizer_not_eq_drops_operand.md

````text
# Optimizer: `!(x == y) == z` is rewritten to `x != y` (z dropped) — silent wrong output

Class: optimizer expression simplification, NOT comparison operand types (#89/#92). Not fixed here.
Site: src/optimizer/simplify.rs:178-205 (the "!(x == y) => x != y" arm matches a Binary `==` whose LEFT is
`!(x == y)` and replaces the whole node with `x != y`, discarding the right operand; the new node gets Span::dummy()).
Repro: found/optimizer_not_eq_drops_operand.pd — must print 0.
  main 903ba02: rc 0, prints 1 (C: `if ((a != b))`).  round-6 tree: rc 0, prints 1 (same rewrite).
Side effect on this unit: the only comparison node built outside the parser; its PD0080 check carries no place
(`PD0080|0|0|!=|main`) because the rewrite gives it a dummy span.
````

## cyclic_type_alias_hangs.pd

````text
// FOUND (different class): two type aliases that name each other HANG the compiler (rc 142
// under a 20s alarm) on d5166e8, before any comparison: a parameter typed by either alias is
// enough. Alias resolution has no cycle check. Not #89/#92: no operator is involved.
type A = B;
type B = A;
fn f(x: A, y: A) -> bool { true }
fn main() { print_int(1); }
````

## explicit_deref_of_reference_param.pd

````text
// FOUND (different class): `*a` on a reference PARAMETER dereferences twice. Codegen already
// emits `(*a)` for every use of `a`, so a written `*a` becomes `(*((*a)))` and gcc refuses
// it: "indirection requires pointer operand ('long long' invalid)". The same shape `*self`
// was refused by name (PD0050, deref_self_is_not_a_place); a non-self reference parameter was not.
// Not a comparison defect: `let v = *a;` fails the same way. Measured on d5166e8 (rc 3).
fn get(a: &i64) -> i64 { let v = *a; v }
fn main() { let x = 5; print_int(get(&x)); }
````

## generic_body_at_char_refused.pd

````text
// Found during #89/#92 round 7 (different class: generic instantiation at `char`, not comparison operands).
// A generic body instantiated at `char` is refused with "Type mismatch: expected char, found Char" —
// the type checker's two spellings of the char type disagree. Same on main 903ba02 and this tree.
fn test<T>(a: T) -> bool { match a { 'a'..='z' => true, _ => false } }
fn main() { let a = 'm'; if test(a) { print_int(1); } else { print_int(0); } }
````

## generic_string_subtraction.pd

````text
// Found by fable (review round 6 of #89/#92), recorded here; a different class: ARITHMETIC on a String in an
// unchecked generic body, not a comparison operand. `a - c` over two `const char*` is a pointer DISTANCE
// (an integer), so `(a - c) == 0` is a well-typed integer comparison and the judge rightly passes it; what is
// unrefused is the String subtraction itself, which a checked body refuses ("expected Int or Float, found String").
// Two distinct strings with equal text: the distance is nonzero, so this prints 0 (main and tip).
fn same<T>(a: T, c: T) -> bool { (a - c) == 0 }
fn main() { let a = int_to_string(12); let b = "12"; if same(a, b) { print_int(1); } else { print_int(0); } }
````

## mangled_name_collides_with_user_name.pd

````text
fn same<T>(a: T, b: T) -> bool { true }
fn same__i64(a: i64, b: i64) -> bool { false }
fn main() { if same(1, 2) { print_int(1); } if same__i64(1, 2) { print_int(2); } }
````

## mixed_signedness_compare.pd

````text
// FOUND (different class): the checker merges every integer width into one `Int`, so a
// `u64` and an `i64` meet in C's usual arithmetic conversions and `-1` becomes 2^64-1.
// `1 > -1` prints 0. Not composite-type comparison: `+ - * /` over mixed widths go through
// the same conversions. Measured on d5166e8 (rc 0, prints 0).
fn main() { let a: u64 = 1; let c: i64 = 0 - 1; if a > c { print_int(1); } else { print_int(0); } }
````

## nested_generic_call.pd

````text
// FOUND (different class): a generic function that calls another generic function with its
// own type parameter emits a call to the UNMANGLED name. gcc: "call to undeclared function
// 'id'". No comparison involved. Measured on d5166e8 (rc 3). This is also why PD0079's
// instantiation check needs no propagation through nested generics: none reach C.
fn id<T>(a: T) -> T { a }
fn wrap<U>(a: U) -> U { id(a) }
fn main() { print_int(wrap(1)); }
````

## optimizer_not_eq_drops_operand.pd

````text
// Found during #89/#92 round 6 (different class: the optimizer, not comparison operands).
// src/optimizer/simplify.rs:178-205 rewrites `!(x == y) == z` to `x != y`, dropping `z`
// (and gives the new `!=` a dummy span).
// !(1 == 2) is true; c is false; true == false is false, so this must print 0.
fn main() {
    let a = 1;
    let b = 2;
    let c = false;
    if !(a == b) == c { print_int(1); } else { print_int(0); }
}
````

## reference_local_is_a_pointer.pd

````text
// UPDATE (round 3): the COMPARISON position is now refused by name — PD0079's code-generation backstop sees the
// emitted `long long*` operands. The other value positions below (print_int(r), r + 1) are still this defect.
// FOUND (different class from #89/#92): a reference LOCAL is typed by the checker as its
// referent (src/typeck/mod.rs, Expr::Reference arm: "references have the same type as their
// inner value"), but code generation declares it `long long* r` and never reads through it.
// Wrong in EVERY value position, not only comparisons — so it is reference erasure, not a
// comparison rule. Measured on d5166e8:
//   print_int(r)  -> gcc: incompatible pointer to integer conversion
//   r == 5        -> compiles (gcc -Wpointer-integer-compare), prints 0
//   r == s        -> prints 0 for x == y == 5 (address compare)
// A reference PARAMETER is fine (codegen dereferences it on every use).
fn main() {
    let x = 5;
    let y = 5;
    let r = &x;
    let s = &y;
    if r == 5 { print_int(1); } else { print_int(0); }
    if r == s { print_int(1); } else { print_int(0); }
}
````

