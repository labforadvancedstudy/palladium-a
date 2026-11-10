# 범위 밖 결함의 재현 — 유닛 #86 #87 (제네릭 인스턴스)

유닛 작업 중 코더가 측정해 남긴 것이다. 고치지 않았고 이슈로 등록하지도 않았다. 파일 6개(노트 0, 프로그램 6)를 한 문서로 묶었다.

## generic_at_float_or_char_refused_by_checker.pd

````text
// FOUND (round 7, not fixed — a different symptom of the same spelling, in the TYPE CHECKER).
// The checker spells a float type argument `f64` and a char `char` (checker_type_to_string), and
// reads a type argument back in substitute_type (src/typeck/mod.rs:6455-6463) for `()`, String,
// i64, i32, u64, u32, bool only; `f64`/`f32`/`char` fall to type_argument_as_named_type
// (src/typeck/mod.rs:6968) = Type::Custom("f64"), a struct. So an instantiation at f64 returns a
// struct to the checker and any typed use of it is refused:
//   main 2ca5eb5 AND round-7 tree: "Type mismatch: expected f64, found Float" (named, not gcc).
// Code generation now reads them back (round 7: CodeGenerator::type_argument_type), so only the
// checker's typing of the call's result stands between this and a run. An alias of f64 (carried
// by its name) is the working spelling today.
fn sz<T>(x: T) -> i64 { return 8; }
fn id<T>(x: T) -> T { return x; }
fn main() {
    let p: f64 = 3.5;
    print_int(sz(p));      // fine: the result is i64
    let q: f64 = id(p);    // refused by the checker: id<f64> returns `struct f64`
    if q > 3.0 { print_int(1); }
}
````

## generic_body_local_of_type_param.pd

````text
// FOUND (different class, NOT fixed by #86/#87): a local annotated with a type
// parameter inside a generic body is emitted as `void*`, because codegen's
// `substitute_types_in_body` clones the body unchanged and `type_to_c` erases
// `Type::TypeParam` to `void*`. Same on main (d5166e8) and on this branch:
//   long long wrap__i64(long long x) { void* y = x; return id__i64(y); }
//   gcc: incompatible integer to pointer conversion
// The call `id(y)` IS resolved to its own instantiation (`id__i64`) here; the
// declaration of `y` is the defect. Already listed in CLAUDE.md as open
// ("제네릭 본문에서 타입 파라미터가 치환되지 않는다").
fn id<T>(x: T) -> T { return x; }
fn wrap<T>(x: T) -> T { let y: T = x; return id(y); }
fn main() { print_int(wrap(6)); }
````

## mut_param_shadowed_by_inner_let.pd

````text
// A local that shadows a `&mut` parameter is still emitted as the parameter's
// pointer: `mutable_params` (src/codegen/mod.rs) is keyed by name and never
// shadowed by a binding or scoped, unlike `variables`/`array_bindings`.
fn f(x: &mut i64) -> i64 {
    if true {
        let x = 5;
        print_int(x);
    }
    return *x;
}
fn main() { let mut n = 1; print_int(f(&mut n)); }
// Measured (round 3 of #86): main@903ba02 and this branch both meet gcc —
//   build_output/mut_param_shadowed.c:307: error: indirection requires pointer operand ('long long' invalid)
//   (`long long x = 5LL; __pd_print_int((*x));`)
// Cause: src/codegen/mod.rs `mutable_params` (declared ~:149, filled ~:3573) is read
// by name at the Ident/assignment emitters (~:3949, ~:6025) and is neither shadowed
// by `bind_non_array`/`let` nor saved by `open_binding_scope`. Different class from
// #86/#87 (not instantiation selection); not fixed here.
````

## param_type_tracked_as_long_long.pd

````text
// FOUND (different class, NOT fixed here): code generation records the C type
// of a `u32`, `u64`, `f32` or `f64` PARAMETER as `long long` in its variable
// table (src/codegen/mod.rs, the parameter loop's `_ => "long long"` arm), while
// the parameter itself is declared with its real C type. A `let` copying the
// parameter is then declared `long long`:
//   fn g(x: u32) -> i64 { let y = x; return ~y; }   -> -8, `~7` of a u32 is 4294967288
//   fn h(x: f64) -> f64 { let y = x; return y; }     -> 2.5 truncated
// Same on main (903ba02). This unit only stops trusting that table when it
// chooses an instantiation (`param_exact_c`).
fn g(x: u32) -> i64 { let y = x; return ~y; }
fn main() { print_int(g(7)); }
````

## type_param_only_in_return_type.pd

````text
// MEASURED (review residual): a generic whose type parameter is in no
// parameter's type cannot be inferred, so it cannot be called.
//   * At top level (this file) BOTH main and this branch refuse it by name:
//     "Could not infer type parameter 'T' from function arguments" (typeck
//     `infer_type_args`). Same for `fn mk<T>() -> T` with `let x: i64 = mk();`
//     and `fn mk<T>(n: i64) -> T` — there is no annotation-driven inference.
//   * Called INSIDE a generic body (`fn wrap<T>(t: T) -> i64 { return mk(); }`),
//     the type checker's body walk records nothing and main emitted the bare
//     `mk()` -> gcc "call to undeclared function". This branch refuses that by
//     name in code generation (`generic_templates` / `instantiation_called`);
//     pinned by tests/m4_generic_call_sites.rs.
// Inferring a type argument from the expected type is a feature, not this unit.
fn mk<T>() -> i64 { return 3; }
fn main() { print_int(mk()); }
````

## u32_arithmetic_with_a_literal_is_long_long.pd

````text
// FOUND (different class, NOT fixed here — #82 width erasure in plain code):
// code generation emits an integer literal as `NLL`, so `v * 2` with `v: u32`
// is computed as `v * 2LL`, a `long long`, and an unannotated `let w = v * 2`
// is declared `long long` (try_infer_expr_type answers `long long` for any
// integer arithmetic, src/codegen/mod.rs `exact_c_type_in` doc). A `u32`
// product does not wrap: this prints 6000000000 and 4294967296, where a `u32`
// computes 1705032704 and 0. Same on main (903ba02). No generic is involved;
// this unit only stops such a value being taken as an i64 when an
// instantiation is chosen for it (a generic call or literal of it is refused
// by name, PD0080, tests/m4_generic_call_sites.rs
// a_value_computed_with_a_literal_is_of_its_operands_width).
fn main() {
    let v: u32 = 3000000000;
    let w = v * 2;
    print_int(w);
    let m: u32 = 4294967295;
    print_int(m + 1);
}
````

