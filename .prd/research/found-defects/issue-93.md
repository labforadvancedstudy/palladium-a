# 범위 밖 결함의 재현 — 유닛 #93 #91 (제네릭 인자)

유닛 작업 중 코더가 측정해 남긴 것이다. 고치지 않았고 이슈로 등록하지도 않았다. 파일 7개(노트 1, 프로그램 6)를 한 문서로 묶었다.

## 노트: README.txt

````text
FOUND (different class, NOT fixed in this unit): a type name in a signature or payload is never checked to be DECLARED.

Pre-existing; identical on main (gates-rel d5166e8) and on this unit's tree.
  undeclared_type_name_bare.pd          `fn unused(x: Nope) -> i64`  -> pdc accepts, gcc refuses (rc 3) - the forbidden class
  undeclared_type_name_in_generic_arg.pd `fn unused(w: Wrap<Nope>) -> i64` -> accepted, RUNS (prints 4): the arg is erased to void*
  same for an enum payload: `enum E { X(Nope), Y }` -> gcc refuses; `enum E { X(Wrap<Nope>), Y }` -> runs.
Where: src/typeck/mod.rs:719 - `CheckerType::from`'s `Custom(name)` arm makes ANY name a struct (`named_struct`), and
`ast_type_to_checker_type` (src/typeck/mod.rs:3007) falls through to it via `with_enum_kinds` (src/typeck/mod.rs:6734) with no
existence check. Class: names in TYPE position are not resolved against the declared types. (#93's class is const-vs-type by
spelling; this is declared-vs-undeclared, so it is reported, not fixed.)

FOUND in review round 1 (different classes, NOT fixed; identical on main d5166e8 and this tree):
  alias_const_argument_shifts_type_arguments.pd  `type Pick<T, U> = T;` + `Pick<N, String>` under `const N` -> RUNS, prints
    hello: x is typed String. src/typeck/mod.rs:3083 filters the CONST arguments out (`GenericArg::Const(_) => None, // TODO`)
    and then zips the remaining TYPE arguments to the alias's parameters positionally, so `String` lands in `T`. A const
    argument in a type parameter's place is never refused. (This is also why main RAN astra's alias_string_{global,type,
    absent}: there `N` was misread as a const and dropped; this unit reads it as the type it names and refuses `x: N`.)
  duplicate_generic_parameter_name.pd  `fn f<T, const T: i64>(x: T)` -> accepted, runs (2): a generic list may name one
    parameter twice; nothing refuses it (parse_generic_params, src/parser/mod.rs).

FOUND in review round 4 (different classes, NOT fixed; identical on main 903ba02):
  dup_struct_field.pd   `struct S { a: i64, a: i64 }`  -> pdc accepts, gcc refuses (duplicate member) — the forbidden class.
  dup_enum_variant.pd   `enum E { A, A }`               -> pdc accepts, gcc refuses.
  (A function's duplicate VALUE parameter, `fn f(a: i64, a: i64)`, is refused, uncoded: "Variable 'a' already defined".)
  None of these is a GENERIC parameter list, which PD0081 now covers.
````

## alias_const_argument_shifts_type_arguments.pd

````text
type Pick<T, U> = T;
fn f<const N: i64>() { let x: Pick<N, String> = "hello"; print(x); }
fn main() { f(); }
````

## dup_enum_variant.pd

````text
enum E { A, A }
fn main() { print_int(1); }
````

## dup_struct_field.pd

````text
struct S { a: i64, a: i64 }
fn main() { print_int(1); }
````

## duplicate_generic_parameter_name.pd

````text
fn f<T, const T: i64>(x: T) -> i64 { return 2; }
fn main() { print_int(f(8)); }
````

## undeclared_type_name_bare.pd

````text
fn unused(x: Nope) -> i64 { return 1; }
fn main() { print_int(4); }
````

## undeclared_type_name_in_generic_arg.pd

````text
struct Wrap<T> { v: T }
fn unused(w: Wrap<Nope>) -> i64 { return 1; }
fn main() { print_int(4); }
````

