# Getting started

Install the compiler, build one program, and confirm the install is sound. The language itself
is in the [tutorial](tutorial.md).

## Install

### Homebrew

```bash
brew tap 2lab-ai/tap
brew install pdc
```

A preview channel tracks `main` and installs alongside the stable one:

```bash
brew install pdc-preview      # binary is `pdc-preview`
```

Use `pdc-preview` to try unreleased fixes; use `pdc` for anything you care about.

### From crates.io

```bash
cargo install alan-von-palladium
```

### From source

```bash
git clone https://github.com/labforadvancedstudy/palladium-a.git
cd palladium-a
cargo build --release
export PATH="$PATH:$(pwd)/target/release"
```

### Requirement

A C compiler on `PATH`. Palladium emits C and then invokes `gcc` — which on macOS is clang under
that name — to produce the executable. If `gcc --version` answers, you are set.

## Your first program

Put this in `hello.pd`:

```palladium
fn main() {
    print("Hello, Palladium!");
}
```

```bash
pdc compile hello.pd -o hello
./build_output/hello
```

Output goes to `build_output/`, alongside the generated C. Read `build_output/hello.c` once —
Palladium's semantics are exactly what that C does, and having seen it makes every later error
message legible.

## What a compile actually does

```
pdc compile hello.pd
  ├─ lex, parse
  ├─ resolve imports, expand macros
  ├─ typecheck
  ├─ borrow check
  ├─ analyse effects        (informational only — gates nothing)
  ├─ optimise
  ├─ emit C                 -> build_output/hello.c
  └─ gcc build_output/hello.c <runtime>/palladium_runtime.c -I<runtime>
                            -> build_output/hello
```

That last step needs the C runtime shipped with the compiler. `pdc` locates it automatically:

```bash
pdc --print-runtime
```

Resolution order, first hit wins:

1. `$PALLADIUM_RUNTIME`, if set. It must contain `palladium_runtime.c` — if it does not, `pdc`
   stops with an error rather than quietly falling back, because a silent fallback is how a
   broken install looks like a working one.
2. Next to the executable: `../share/palladium/runtime`, then `../lib/palladium/runtime`, then
   `runtime/`. This is the packaged layout.
3. `./runtime` in the current directory — what makes a source checkout work.

## Verify the install

Run this from a directory that is **not** a Palladium checkout, so nothing is found by accident:

```bash
cd /tmp
pdc --print-runtime                                  # prints a directory that exists
printf 'fn main() { print("ok"); }\n' > ok.pd
pdc compile ok.pd -o ok && ./build_output/ok         # prints: ok
```

If that works, the install is sound.

## Troubleshooting

**`gcc compilation failed: ... palladium_runtime.c`**
The runtime was not found. `pdc --print-runtime` lists every path it tried. Point
`PALLADIUM_RUNTIME` at the directory holding `palladium_runtime.c`.

**`cannot infer the type of ...: no type rule for this <kind> expression`**
Code generation has no inference rule for that initializer. Add the type:

```palladium
fn main() {
    let n: i64 = 3;
    print_int(n);
}
```

This message replaced a silent default to a 64-bit integer, which used to emit broken C for
references, enum values and string copies instead of saying anything.

**``has no method `len` ``**
Method syntax works — `x.f()` and `Type::f(x)` both do — but only on a type that can carry an
`impl` block, which a primitive cannot. The string operations are free builtins, not methods.

```palladium no-compile
fn main() {
    let s: String = "abc";
    print_int(s.len());        // String has no impl block; use string_len(s)
}
```

**`Use of moved value`**
Struct parameters move. Declare them `mut`, which makes them pointers in the generated C and
borrows rather than moves — and lets the callee's changes reach you.

**`Expected function, struct, enum, trait, type, impl, or macro declaration`**
Usually a top-level `mod` or `use`, neither of which exists. Imports use the
`import` keyword and must all appear before the first item. Top-level `const` and `static` DO exist
now — `const LIMIT: i64 = 10;`, `static mut COUNT: i64 = 0;` — with a mandatory type, a mandatory
initialiser that is a literal or arithmetic over literals, and a numeric or `bool` type; anything
outside that is refused with a message naming what it saw.

**`` `macro_rules!` is not a declaration in this language ``**
There is one macro system and it is spelled `macro name!(params) { body }`. A parameter is
substituted where the body writes `$name`.

## Where next

- [Tutorial](tutorial.md) — the language, worked through. Every snippet is compiled by
  `scripts/check-docs.sh`.
- [Language specification](../specification/language-spec.md) — every construct with its status
  and the source location that proves it.
- [Builtin reference](../reference/builtins.md) — all 34 builtins, generated from the compiler's
  own table, and all of them callable.
- [Palladium vs Rust](../contributing/palladium_vs_rust_comparison.md) — measured comparison.
