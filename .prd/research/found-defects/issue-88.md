# 범위 밖 결함의 재현 — 유닛 #88 #90 (참조 타입)

유닛 작업 중 코더가 측정해 남긴 것이다. 고치지 않았고 이슈로 등록하지도 않았다. 파일 11개(노트 0, 프로그램 11)를 한 문서로 묶었다.

## block_value_with_for_over_array_literal.pd

````text
// DIFFERENT CLASS (no reference) — found in #88 review round 3 (fable r2 r25 without its `&i64`).
// A value block whose body has `for x in [<literal>, ..]`: the emitted C does not parse
// (gcc: "expected ';' at end of declaration"). main 903ba02 and #88 alike.
fn main() { let x: u32 = 1; let v = { let mut s: i64 = 0; for x in [4294967297, 1] { s = s + x; } s }; print_int(v); }
````

## generic_body_nonliteral_repeat_count.pd

````text
// DIFFERENT CLASS (no reference) — found in #88 review round 5 (astra r4 generic_array_repeat_refcount).
// An array repeat whose count is not an integer literal is refused by the checker
// ("Array repeat count must be an integer literal") — but a generic body is not checked
// on main, and code generation emits `long long a[0]` and a second `a`: gcc
// "redefinition of 'a' with a different type". main 2ca5eb5 and #88 alike; a reference
// as the count (`[1; r]`) fails the same way, so the reference is not the cause.
fn f<T>(a:T)->i64 { let n=2; let a=[1;n]; 7 } fn main() { print_int(f(1)); }
````

## generic_body_tuple_element_mismatch.pd

````text
// FOUND in round 7 of #88 (different class, not fixed): a tuple annotation in a generic body whose
// element the value does not match (`true` for an `i64`) emits two different C tuple structs -- gcc
// on main 2ca5eb5 and #88 alike; no reference involved. (astra r2 generic_tuple_ref_good is this with
// a matching `&i64` beside it: refused at round 6 only through that reference sibling, which the
// position-by-position judgement of round 7 no longer lets decide.)
fn f<T>(x: T) -> i64 { let a: (i64, i64) = (true, 5); a.1 }
fn main() { print_int(f(1)); }
````

## generic_body_type_param_annotation_void_ptr.pd

````text
// FOUND (different class, not #88/#90): a `T` annotation in a generic body is emitted as `void *` by code generation,
// so `let x: T = a;` at T=i64 fails gcc (int-conversion), main 903ba02 and this tree alike. No reference involved.

fn id<T>(a: T) -> T { let x: T = a; x }
fn main() { print_int(id(5)); }
````

## generic_called_only_from_generic_body.pd

````text
// FOUND in round 7 of #88 (different class, not fixed): a generic called ONLY from another generic's
// body is never instantiated, so code generation calls a function it never emits (gcc: "call to
// undeclared function 'take'") -- main 2ca5eb5 and #88 alike, no reference involved (fable r6 D2/D3
// are this with a `&T` parameter; astra r5 w3e).
fn take<T>(a: T) -> i64 { 7 }
fn f<U>(u: U) -> i64 { take(1) }
fn main() { print_int(f(1)); }
````

## generic_mut_param_call_passes_address.pd

````text
// DIFFERENT CLASS (no reference) — found in #88 review round 3 (astra r2 generic_mut_valid).
// A generic with a `mut` by-value parameter: the call site passes `&n` (C `long long *`)
// to the instantiation's `long long` parameter. gcc: "incompatible pointer to integer
// conversion passing 'long long *' to parameter of type 'long long'". main 903ba02 and #88 alike.
fn f<T>(mut a: T, b: T) -> T { a = b; a }
fn main() { let mut n = 5; print_int(f(n, 7)); print_int(n); }
````

## generic_struct_annotation_in_generic_body_incomplete.pd

````text
// FOUND in round 6 of #88 (different class, not fixed): a generic struct annotated in a generic body
// (`let g: G<T> = ..`) is emitted as the incomplete `struct G` -- gcc on main 2ca5eb5, no reference
// involved (fable r5 N04/N05 are this with `G<&T>`).
struct G<T> { p: T }
fn f<T>(a: T) -> i64 { let g: G<T> = G { p: a }; 7 }
fn main() { print_int(f(5)); }
````

## imported_reference_alias_field_unresolved.pd

````text
// DIFFERENT CLASS (import of type aliases) — found in #88 review round 4 (astra r3 import_struct_ref).
// A module's `pub type A = &i64;` is not resolved by the importing checker: a field `p: A`
// of an imported struct is read as an unknown named type `A`, so `H { p: &n }` and `H { p: 5 }`
// are both refused on main 903ba02/2ca5eb5 and on #88 alike ("expected A, found Int/&i64") —
// and in an imported GENERIC body, which main never checked, `H { p: 5 }` reaches gcc
// (int to pointer), since the checker cannot tell `A` is a reference.
// zrs.pd:   pub type A=&i64; pub struct H { p:A } pub fn f<T>(x:T)->i64 { let h=H { p:5 }; 7 }
import zrs; fn main() { print_int(f(1)); }
````

## loop_value_declared_by_outer_binding.pd

````text
// DIFFERENT CLASS (value-level, no reference) — found in #88 review round 3.
// A value `loop`'s binding is declared with the C type of its first `break`
// value READ IN THE SCOPES OUTSIDE THE LOOP (src/codegen/mod.rs: Stmt::Let ->
// try_infer_expr_type -> Expr::Loop arm ~885, `first_break_value(body)` typed
// against `self.variables` before the body is generated). When the `break`
// names a binding the body itself makes, the outer binding of that name picks
// the declaration: here `v` is `unsigned int` and the i64 value is truncated.
// main 903ba02 and #88 tip both print 1 (expected 4294967297). Silent.
fn main() { let n: u32 = 1; let v = loop { let n: i64 = 4294967297; break n; }; print_int(v); }
````

## mut_scalar_param_has_no_write.pd

````text
// FOUND (different class, not fixed in #88/#90): a write through a `&mut i64`
// PARAMETER has no spelling that works.
//   `*x = v`  -> d5166e8: gcc compilation failed (codegen reads `x` as `(*x)`,
//               so `*x` is `(*((*x)))`); after #88: refused PD0079.
//   `x = v`   -> d5166e8 and after: "Cannot assign to immutable variable 'x'"
//               (src/typeck/mod.rs, Stmt::Assign Ident arm: the parameter is not
//               a `mut` binding), although codegen would emit `(*x) = v`.
// A `&mut S` parameter IS writable through its fields (`c.n = v`); a scalar one
// is not writable at all.
fn set(x: &mut i64, v: i64) {
    x = v;
}

fn main() {
    let mut m: i64 = 5;
    set(&mut m, 9);
    print_int(m);
}
````

## reference_to_array_annotation_declarator.pd

````text
// FOUND in round 6 of #88 (different class, not fixed): `let r: &[i64; 1] = &arr;` emits a C
// pointer-to-array declarator gcc rejects ("brackets are not allowed here") -- gcc on main 2ca5eb5
// in a NON-generic main (fable r5 N09 is this with `&[T; 1]`).
fn main() { let arr: [i64; 1] = [5]; let r: &[i64; 1] = &arr; print_int(7); }
````

