> **NORMATIVE — this is what Palladium is defined to be.** It is not a description of what
> `pdc` implements today. What is implemented, partial, or unimplemented is recorded per
> specification section in the
> [implementation status annex](../../../specification/language-spec.md#part-ii-implementation-status-annex).
> Palladium blocks below are fenced `no-compile`: the syntax is normative, the compiler does not
> accept it yet, and `scripts/check-docs.sh` counts each fence rather than hiding it.

# Feature: Totality Checking

Normative specification section: [`language-spec.md` §N8 Totality](../../../specification/language-spec.md#n8-totality).

## Overview

Palladium can prove that functions terminate, eliminating entire classes of bugs related to infinite loops and non-termination. This feature enables mathematical reasoning about code correctness.

### Normative syntax

| Form | Meaning |
|---|---|
| `#![total(strict)]` | Crate-level mode. Every function in the crate must be proven total, and `unsafe` is not permitted inside it. |
| `#[total]` | Opt-in on a single function: the compiler must prove this one terminates. |
| `#[decreases(expr)]` | The termination measure. `expr` must strictly decrease, in a well-founded order, at every recursive call. |
| `#[total(fuel = N)]` | Bounded termination: at most `N` steps, with running out treated as a failure the compiler accounts for. |
| `#[partial]` | Explicit opt-out. The author asserts that termination is not being proven here. |

Structural recursion on an inductive type needs no measure: a recursive call on a strict subterm
is proven automatically.

## Code Comparison

### Rust (No Totality Guarantees)
```rust
// Rust can't prove termination
fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        n * factorial(n - 1)  // Hope it terminates!
    }
}

// Potential infinite loop - compiles fine
fn find_prime(start: u64) -> u64 {
    let mut n = start;
    loop {
        if is_prime(n) {
            return n;
        }
        n += 1;  // What if no prime exists?
    }
}

// Ackermann function - terminates but Rust can't prove it
fn ackermann(m: u64, n: u64) -> u64 {
    match (m, n) {
        (0, n) => n + 1,
        (m, 0) => ackermann(m - 1, 1),
        (m, n) => ackermann(m - 1, ackermann(m, n - 1)),
    }
}

// Collatz conjecture - unknown if it always terminates
fn collatz(mut n: u64) -> u64 {
    let mut steps = 0;
    while n != 1 {
        if n % 2 == 0 {
            n /= 2;
        } else {
            n = 3 * n + 1;
        }
        steps += 1;
    }
    steps
}
```

### Go (No Totality Checking)
```go
// Go also can't prove termination
func factorial(n uint64) uint64 {
    if n == 0 {
        return 1
    }
    return n * factorial(n-1)
}

// Infinite recursion possible
func badRecursion(n int) int {
    return badRecursion(n + 1)  // Stack overflow
}

// Complex termination
func gcd(a, b uint64) uint64 {
    for b != 0 {
        a, b = b, a%b  // Terminates but Go doesn't verify
    }
    return a
}

// Potential deadlock
func riskyGoroutine(ch chan int) {
    for {
        select {
        case v := <-ch:
            if v == 0 {
                return
            }
        // No default - might block forever
        }
    }
}
```

### Palladium (Proven Termination)

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
// Compiler proves this terminates
#[total]
fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        n * factorial(n - 1)  // Proven: n decreases
    }
}

// Must prove termination for total functions
#[total]
fn find_prime_bounded(start: u64, max: u64) -> Option<u64> {
    // Compiler requires bounded iteration
    for n in start..=max {
        if is_prime(n) {
            return Some(n);
        }
    }
    None
}

// Structural recursion automatically proven
#[total]
fn tree_sum(tree: Tree<i32>) -> i32 {
    match tree {
        Leaf(n) => n,
        Node(left, right) => {
            tree_sum(left) + tree_sum(right)  // Subterms are smaller
        }
    }
}

// Well-founded recursion with measure
#[decreases(m + n)]
fn ackermann(m: u64, n: u64) -> u64 {
    match (m, n) {
        (0, n) => n + 1,
        (m, 0) => ackermann(m - 1, 1),
        (m, n) => ackermann(m - 1, ackermann(m, n - 1)),
    }
}

// Partial functions must be marked
#[partial]
fn collatz(n: u64) -> u64 {
    // Compiler accepts we can't prove this
    let mut n = n;
    let mut steps = 0;
    while n != 1 {
        if n % 2 == 0 {
            n /= 2;
        } else {
            n = 3 * n + 1;
        }
        steps += 1;
    }
    steps
}

// Fuel-based termination for complex cases
#[total(fuel = 1000)]
fn complex_search(data: Vec<i32>, target: i32) -> Option<usize> {
    // Compiler ensures we use at most 1000 steps
    binary_search_with_fuel(data, target, 1000)
}
```

## Why This Feature Exists

### 1. Mathematical Correctness
- Prove programs terminate
- Enable formal verification
- Support theorem proving
- Build high-assurance systems

### 2. Optimization Headroom
A function proven total needs no runtime termination guard, can be inlined and unrolled more
aggressively, and is a candidate for compile-time evaluation. Those are consequences of the proof;
the size of the win is unmeasured — see [Design intent, not measurements](#design-intent-not-measurements).

### 3. Safety Critical Systems
- Aerospace: No infinite loops in flight control
- Medical: Guaranteed response times
- Finance: Predictable execution
- Embedded: Known resource bounds

## How It Works

The block below is compiler-internal pseudocode rather than a Palladium program.

### Termination Checking Algorithm

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
// Compiler's termination checker
fn check_termination(func: Function) -> Result<Proof, Error> {
    match analyze_recursion(func) {
        Structural(rec) => prove_structural_recursion(rec),
        WellFounded(rec, measure) => prove_well_founded(rec, measure),
        Bounded(loop, bound) => prove_bounded_iteration(loop, bound),
        Unknown => Err("Cannot prove termination"),
    }
}

// Structural recursion on inductively defined types
fn prove_structural_recursion(rec: Recursion) -> Result<Proof> {
    // Check that recursive calls use strict subterms
    for call in rec.calls {
        if !is_strict_subterm(call.arg, rec.param) {
            return Err("Not structurally recursive");
        }
    }
    Ok(Proof::Structural)
}
```

### Termination Measures

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
// Different ways to prove termination

// 1. Structural recursion
#[total]
fn length<T>(list: List<T>) -> usize {
    match list {
        Nil => 0,
        Cons(_, tail) => 1 + length(tail)  // tail < list
    }
}

// 2. Natural number measure
#[decreases(n)]
fn countdown(n: u64) -> u64 {
    if n == 0 { 0 } else { countdown(n - 1) }
}

// 3. Lexicographic ordering
#[decreases((m, n))]
fn euclid(m: u64, n: u64) -> u64 {
    if n == 0 { m } else { euclid(n, m % n) }
}

// 4. Custom well-founded relation
#[total(wf_relation = tree_size)]
fn tree_fold<T, U>(tree: Tree<T>, init: U, f: Fn(T, U, U) -> U) -> U {
    match tree {
        Leaf(x) => f(x, init, init),
        Node(l, r) => f(
            tree_fold(l, init, f),  // l smaller than tree
            tree_fold(r, init, f),  // r smaller than tree
        ),
    }
}
```

### Fuel-Based Termination

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
// For cases where we can't prove termination statically
#[total(fuel = F)]
fn search_with_fuel<F: Fuel>(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let mut fuel = F::new();
    let mut pos = 0;
    
    while pos <= haystack.len() - needle.len() {
        fuel.consume(1)?;  // Fails if out of fuel
        
        if haystack[pos..].starts_with(needle) {
            return Some(pos);
        }
        pos += 1;
    }
    None
}

// Use at compile time with concrete fuel
let result = search_with_fuel::<Fuel<1000>>(data, pattern);
```

## Where the implementation currently diverges

Measured at commit `abeb665`. None of this qualifies the definition above; it records distance.

**1. Attributes do not lex.** There is no `#` token in the lexer. Compiling
`#[total]` followed by `fn f(n: i64) -> i64 { return n; }` fails before parsing:

```
error: Unexpected character '#' at line 1, column 1
  = note: Palladium only allows ASCII letters, numbers, and common symbols
```

`docs/specification/grammar.ebnf` has no attribute production, and its punctuation set
(`grammar.ebnf:66-67`) contains no `#`. So `#![total(strict)]`, `#[total]`, `#[decreases(...)]`
and `#[partial]` are all unreachable today — the blocker is lexical, one level below the feature
itself.

**2. No termination checker exists.** `grep -rn 'total\|decreases\|Fuel' src/ --include='*.rs'`
returns one unrelated hit (`src/runtime/string_ops.rs:398`, a test named `test_null_termination`).
There is no recursion analysis, no measure checking, and no proof representation.

**3. The prerequisites are missing too.** Structural recursion is stated over inductive types with
pattern matching on subterms. Today `match` has exactly three pattern forms
(`src/ast/mod.rs:313`) — no literal, range, guard or tuple patterns — and generic types do not
survive codegen (`src/codegen/mod.rs:1663`). A totality checker has nothing to be total over yet.

## Design intent, not measurements

The earlier version of this document carried a "Performance Impact" section asserting
"+20-30% for totality checking", "5-15% faster for proven-total functions", and a smaller binary.
No such measurement exists in this repository and the checker does not exist, so those numbers
cannot have come from anywhere. They are deleted rather than restated.

The intent that survives is structural: a proof is compile-time only, contributes nothing to the
binary, and licenses optimisations a partial function cannot receive. Whether that is worth 5% or
0% is a question for a benchmark, not a claim.

## Common Patterns

### List Processing

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
#[total]
fn map<T, U>(list: List<T>, f: Fn(T) -> U) -> List<U> {
    match list {
        Nil => Nil,
        Cons(x, xs) => Cons(f(x), map(xs, f))
    }
}
```

### Tree Algorithms

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
#[total]
fn tree_height<T>(tree: Tree<T>) -> u64 {
    match tree {
        Leaf(_) => 1,
        Node(l, r) => 1 + max(tree_height(l), tree_height(r))
    }
}
```

### Number Theory

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
#[decreases(b)]
fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}
```

## Future Improvements

1. **Automatic Measure Inference**: Deduce termination measures
2. **Coinductive Types**: Handle infinite data structures
3. **Dependent Types**: More precise termination proofs
4. **SMT Integration**: Use external solvers for complex cases

## Resolved: the relationship between `#[total]` and `#[decreases]`

Two spellings existed in this repository. `docs/marketing/avp_marketing.md:17-20` and
`docs/marketing/Turing.md:67` used `#![total(strict)]` with a separate `#[decreases(expr)]`; the
pre-2026-08 version of this document used `#[total(decreases = expr)]`, making the measure an
argument of totality.

**Decided: they are independent.** `#[total]` states the *obligation* — this function must be
proven to terminate. `#[decreases(expr)]` supplies the *evidence* — this expression is the
well-founded measure. They are separate because the two are separately useful:

- `#[total]` alone is the common case. Structural recursion needs no measure, so demanding one
  would be noise on the majority of total functions.
- `#[decreases(expr)]` alone is meaningful **outside** `#[total]`: it is a checked assertion about
  a function the author is not asking to be proven total, and inside a `#![total(strict)]` crate
  every function carries the obligation implicitly, so there is no `#[total]` left to hang the
  measure on. Under the argument form, `#![total(strict)]` would have had no way to express a
  measure at all — which is what settles it.
- A `#[decreases]` that fails to decrease is an error whether or not `#[total]` is present.

Consequently `#[total(decreases = expr)]` is **not** valid syntax, and the examples above use the
independent form throughout.

`#[total(fuel = N)]` and `#[total(wf_relation = f)]` keep the argument form, and that is
deliberate rather than residue: both modify *how the obligation is discharged* rather than
supplying evidence for it. `fuel` weakens the obligation to a bounded one; `wf_relation` names the
order in which a measure is compared. Neither is a measure, so neither belongs in `#[decreases]`.

## Related

- [Palladium v1.0 feature definition](../PALLADIUM_V1_FEATURES.md) — where this sits among the rest
- [Async as effect](../async-system/async-as-effect.md)
- [Implicit lifetimes](../core-language/implicit-lifetimes.md)
- [Feature index](../feature-index.toml)
- [Language specification](../../../specification/language-spec.md)
