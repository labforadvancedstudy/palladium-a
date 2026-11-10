# 범위 밖 결함의 재현 — 유닛 #85 (매크로)

유닛 작업 중 코더가 측정해 남긴 것이다. 고치지 않았고 이슈로 등록하지도 않았다. 파일 5개(노트 0, 프로그램 5)를 한 문서로 묶었다.

## F1_macro_param_as_assign_target.pd

````text
// FOUND during #85 (different class: substitution shape, not expansion reach).
// A macro parameter used as an ASSIGNMENT TARGET does not parse, in a free
// function as anywhere: substitution parenthesises every capture
// (src/macros/expander.rs, substitute_template), so `$v = $v + 1` becomes
// `(i) = (i) + 1`, and the statement parser does not accept a parenthesised
// place. Measured on d5166e8 and on the #85 tree alike:
//   error: Expected ';' (Expected ';' after expression), but found '='
macro tick!(v) { $v = $v + 1; }
fn main() { let mut i = 0; tick!(i); print_int(i); }
````

## F2_imported_struct_method/libimpl.pd

````text
pub struct S { n: i64 }
impl S { fn get(&self) -> i64 { return 2 + self.n; } }
pub fn mk() -> S { return S { n: 40 }; }
````

## F2_imported_struct_method/main.pd

````text
// FOUND during #85 (different class: imported impl blocks, no macro involved).
// A method of a struct declared in an imported module cannot be called:
// measured on d5166e8 and on the #85 tree alike, from this directory:
//   error: no method `get` on `S`: no `impl S` block declares `fn get`
// (an `impl` method cannot be `pub` either: "Expected 'fn' for method, but
// found 'pub'"). So the #85 module expansion covers a module's impl methods
// (src/macros/mod.rs expand_item, Item::Impl) but no program can call one yet.
import libimpl;
fn main() { let s = mk(); print_int(s.get()); }
````

## F3_self_in_macro_argument.pd

````text
// NOTE found during #85 (a NAMED refusal, location-independent; not fixed).
// `self` is a keyword token, and the macro token table (src/parser/mod.rs,
// token_to_ast_token, the `other =>` arm) refuses every keyword in a macro
// body OR ARGUMENT, so a method cannot write `double!(self.n)`. Measured the
// same on d5166e8 and on the #85 tree:
//   error: 'self' may not appear in a macro body or in a macro argument: the
//   macro token stream can carry identifiers, integer literals and
//   single-character punctuation, and nothing else
// The working spelling is `let n = self.n; double!(n)` (tests/04_macro_in_method_body.pd).
macro double!(x) { $x * 2 }
struct S { n: i64 }
impl S { fn twice(&self) -> i64 { return double!(self.n); } }
fn main() { let s = S { n: 21 }; print_int(s.twice()); }
````

## F4_tuple_index_of_tuple_literal_untyped.pd

````text
// FOUND during the #85 extension (different class: code generation's type
// inference, no macro involved). A tuple index over a tuple LITERAL, bound
// without an annotation, is refused by code generation. Measured the same on
// d5166e8 and on the #85 tree:
//   error: Code generation failed: cannot infer the type of `a`: no type rule
//   for this tuple element access expression. Add an explicit type annotation
// With `let a: i64 = ...` it runs. The type checker accepted the program, so
// the two passes disagree about what this expression's type is.
fn main() { let a = (2, 40).0; let b = (2, 40).1; print_int(a + b); }
````

