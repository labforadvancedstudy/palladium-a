# The Palladium Tutorial

Every code block in this file is compiled and run by `scripts/check-docs.sh`. If a snippet is
here, the compiler accepts it. That is not a courtesy — this repository previously shipped
documentation in which 508 of 560 snippets did not compile, describing a language that did not
exist. The checker is how that stays fixed.

Blocks marked <code>no-compile</code> are deliberate counter-examples showing what the compiler
rejects; they are excluded from the check.

## 1. Hello

```palladium
fn main() {
    print("Hello, Palladium!");
}
```

```bash
pdc compile hello.pd -o hello
./build_output/hello
```

`print` writes a string and a newline. `print_int` writes an integer.

## 2. Variables and types

Bindings are immutable unless you write `mut`.

```palladium
fn main() {
    let x: i64 = 42;
    let mut count: i64 = 0;
    count = count + 1;

    let name: String = "Palladium";
    let ready: bool = true;

    print_int(x);
    print_int(count);
    print(name);
    if ready {
        print("ready");
    }
}
```

Type annotations on `let` are optional; the compiler infers literals, calls, struct and enum
values, references, field and index expressions. Where it has no rule it now says so:

```
error: cannot infer the type of `r`: no type rule for this range expression.
       Add an explicit type annotation, e.g. `let r: i64 = ...;`
```

That used to be a silent default to a 64-bit integer, which produced broken C for references,
enum values and string copies. Annotating is still good practice in code you intend to keep —
the bootstrap compiler annotates everywhere — but it is no longer load-bearing.

The primitive types are `i64` (also spelled `int`), `i32`, `u32`, `u64`, `bool`, and `String`.
There are no floating-point types and no `char`.

## 3. Functions

```palladium
fn add(a: i64, b: i64) -> i64 {
    return a + b;
}

fn greet(name: String) {
    print("Hello,");
    print(name);
}

fn main() {
    print_int(add(10, 20));
    greet("world");
}
```

Write `return` explicitly. A trailing expression does work, but being explicit costs nothing and
this language spent a year silently discarding tail expressions — and still discards the tail of
an `if`, so a function whose body is `if … { … } else { … }` returns garbage
([`language-spec.md` A6.6](../specification/language-spec.md#a66-tail-expressions)).

Functions may be called before they are defined — the compiler emits C prototypes for you.
Mutual recursion works:

```palladium
fn is_even(n: i64) -> bool {
    if n == 0 {
        return true;
    }
    return is_odd(n - 1);
}

fn is_odd(n: i64) -> bool {
    if n == 0 {
        return false;
    }
    return is_even(n - 1);
}

fn main() {
    if is_even(10) {
        print("10 is even");
    }
}
```

> Always put spaces around a binary `-`. The lexer's integer rule includes the sign, so `n-1`
> tokenises as `n` followed by `-1` and misparses. `n - 1` is correct.

## 4. Control flow

```palladium
fn classify(n: i64) -> String {
    if n < 0 {
        return "negative";
    } else {
        if n == 0 {
            return "zero";
        } else {
            return "positive";
        }
    }
}

fn main() {
    print(classify(-5));
    print(classify(0));
    print(classify(7));

    let mut i: i64 = 0;
    while i < 3 {
        print_int(i);
        i = i + 1;
    }

    for j in 0..3 {
        print_int(j * 10);
    }
}
```

**There is no `else if`.** Nest the `if` inside the `else` block, as above. There is also no
`loop` keyword — use `while true`. And there is no `+=`; write `i = i + 1`.

This is rejected:

```palladium no-compile
fn main() {
    let x: i64 = 5;
    if x > 9 {
        print("big");
    } else if x > 1 {     // error: Expected '{' after else
        print("medium");
    }
}
```

## 5. Arrays

Arrays are fixed size. There is no `Vec`; carry an explicit count alongside the array.

```palladium
fn sum_first(mut values: [i64; 8], n: i64) -> i64 {
    let mut total: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        total = total + values[i];
        i = i + 1;
    }
    return total;
}

fn main() {
    let mut values: [i64; 8] = [0; 8];
    let mut count: i64 = 0;

    values[count] = 10;
    count = count + 1;
    values[count] = 20;
    count = count + 1;
    values[count] = 12;
    count = count + 1;

    print_int(count);
    print_int(sum_first(values, count));
}
```

Two rules make array parameters behave:

- Declare them `mut`. An array parameter lowers to a C pointer, so the callee writes through to
  the caller's array — declaring it `mut` is what tells the borrow checker the same thing the
  code generator already believes.
- Iterate a parameter with `while` and an index, not `for`. `for` over an array parameter
  currently emits `sizeof(arr)/sizeof(arr[0])` on a pointer that has already decayed, which is
  wrong. `for` over a literal range is fine.

## 6. Structs

```palladium
struct Point {
    x: i64,
    y: i64,
    label: String,
}

fn shift(mut p: Point, dx: i64, dy: i64) {
    p.x = p.x + dx;
    p.y = p.y + dy;
}

fn describe(mut p: Point) -> i64 {
    print(p.label);
    return p.x * p.y;
}

fn main() {
    let mut origin: Point = Point { x: 3, y: 4, label: "corner" };
    shift(origin, 1, 1);
    print_int(origin.x);
    print_int(origin.y);
    print_int(describe(origin));
}
```

A struct parameter declared `mut` becomes a pointer in C, so mutations are visible to the caller
— that is how `shift` works. Struct fields may be integers, booleans, strings, arrays, and other
structs. They may **not** be tuples, references, or generic types.

## 7. Enums and pattern matching

```palladium
enum Shape {
    Circle(i64),
    Rect(i64, i64),
    Empty,
}

fn area(s: Shape) -> i64 {
    match s {
        Shape::Circle(r) => {
            return 3 * r * r;
        }
        Shape::Rect(w, h) => {
            return w * h;
        }
        Shape::Empty => {
            return 0;
        }
    }
}

fn main() {
    let c: Shape = Shape::Circle(2);
    let r: Shape = Shape::Rect(3, 4);
    print_int(area(c));
    print_int(area(r));
}
```

`match` works on enums only. There are no literal patterns, so you cannot match on an integer or
a string — use an `if`/`else` chain for those. There are no match guards and no or-patterns.

The annotations on `let c: Shape = ...` are optional — enum construction is inferred — but they
document the intent, and `match` reads better when the scrutinee's type is stated nearby.

## 8. Strings

Strings are immutable handles. Build them with the builtins rather than operators.

```palladium
fn main() {
    let a: String = "Palla";
    let b: String = "dium";
    let joined: String = string_concat(a, b);

    print(joined);
    print_int(string_len(joined));

    if string_eq(joined, "Palladium") {
        print("match");
    }

    let first: i64 = string_char_at(joined, 0);
    print_int(first);

    let head: String = string_substring(joined, 0, 5);
    print(head);

    print(int_to_string(42));
    print_int(string_to_int("99"));
}
```

Character classification takes the integer returned by `string_char_at`:

```palladium
fn count_digits(s: String) -> i64 {
    let mut n: i64 = 0;
    let mut i: i64 = 0;
    let len: i64 = string_len(s);
    while i < len {
        let ch: i64 = string_char_at(s, i);
        if char_is_digit(ch) {
            n = n + 1;
        }
        i = i + 1;
    }
    return n;
}

fn main() {
    print_int(count_digits("a1b22c333"));
}
```

`+` does work on strings, but `string_concat` is preferred: it keeps the meaning explicit and it
is what the bootstrap compiler uses.

## 9. Files and program arguments

```palladium
fn main() {
    let path: String = "/tmp/palladium_tutorial.txt";

    let handle: i64 = file_open(path);
    file_write(handle, "written from Palladium\n");
    file_close(handle);

    let back: String = read_file_to_string(path);
    print(back);
    print_int(string_len(back));
}
```

Command-line arguments follow C's convention — `arg_count()` includes the program name, so the
first real argument is `arg_at(1)`:

```palladium
fn main() {
    let n: i64 = arg_count();
    if n < 2 {
        print("usage: prog <name>");
    } else {
        print(string_concat("hello, ", arg_at(1)));
    }
}
```

## 10. References

```palladium
fn main() {
    let x: i64 = 42;
    let y: &i64 = &x;
    print_int(*y);

    let inferred = &x;      // the annotation is optional
    print_int(*inferred);
}
```

Note that the type checker does not distinguish `&T` from `T` — there is no reference type in it
at all. Borrow checking happens, and code generation emits real pointers, but references are not
yet part of the type system. One consequence: dereferencing a reference *parameter* emits a
double dereference and fails in C, so pass values or arrays rather than `&T` parameters for now.

## 11. A whole program

A word-frequency-style counter, using every mechanism above: arrays with a count, string
scanning, a struct for state, and an explicit `while` loop.

```palladium
struct Counter {
    starts: [i64; 32],
    lengths: [i64; 32],
    n: i64,
}

fn add_word(mut c: Counter, start: i64, length: i64) {
    if c.n < 32 {
        c.starts[c.n] = start;
        c.lengths[c.n] = length;
        c.n = c.n + 1;
    }
}

fn split_words(mut c: Counter, text: String) {
    let len: i64 = string_len(text);
    let mut i: i64 = 0;
    while i < len {
        let ch: i64 = string_char_at(text, i);
        if char_is_whitespace(ch) {
            i = i + 1;
        } else {
            let start: i64 = i;
            while i < len {
                if char_is_whitespace(string_char_at(text, i)) {
                    break;
                }
                i = i + 1;
            }
            add_word(c, start, i - start);
        }
    }
}

fn longest(mut c: Counter) -> i64 {
    let mut best: i64 = 0;
    let mut i: i64 = 0;
    while i < c.n {
        if c.lengths[i] > best {
            best = c.lengths[i];
        }
        i = i + 1;
    }
    return best;
}

fn main() {
    let mut c: Counter = Counter { starts: [0; 32], lengths: [0; 32], n: 0 };
    let text: String = "the quick brown fox jumped over lazy dogs";

    split_words(c, text);

    print("words:");
    print_int(c.n);
    print("longest:");
    print_int(longest(c));

    let mut i: i64 = 0;
    while i < c.n {
        print(string_substring(text, c.starts[i], c.starts[i] + c.lengths[i]));
        i = i + 1;
    }
}
```

## 12. What the language does not have

Being clear about this is the point of the document. None of the following work today, and
several of them fail *without an error message* — see the
[language specification](../specification/language-spec.md) for the exact failure mode of each.

| Not available | Use instead |
|---|---|
| `else if` | nested `if` inside `else` |
| `loop` | `while true` |
| `+=`, `-=`, `*=` | `i = i + 1` |
| method calls `x.f()` | `Type::f(x)` |
| traits | nothing — they parse and emit no code at all |
| generics | nothing — monomorphisation is partial and argument parsing is buggy |
| closures | top-level functions |
| `Vec`, `HashMap` | fixed-size arrays with a count |
| `Option`, `Result` | your own `enum` |
| `?` operator | explicit checks — `?` is rejected: "the `?` operator is not implemented" |
| `async` / `.await` | nothing — `.await` is rejected: "`.await` is not implemented" |
| tuples | a struct |
| floats, `char` | `i64` |
| bitwise `& \| ^ << >>` | nothing — the lexer has no such tokens |
| `as` casts | nothing |
| string interpolation | `string_concat` |
| literal patterns in `match` | `if`/`else` chains |

## Next

- [Language specification](../specification/language-spec.md) — every construct, with its status
  and the source location that proves it.
- [Bootstrap subset](../specification/bootstrap-subset.md) — the subset the self-hosting compiler
  is written in, and the self-hosting gate.
- [Builtin reference](../reference/builtins.md) — all 38 builtin functions with signatures.
