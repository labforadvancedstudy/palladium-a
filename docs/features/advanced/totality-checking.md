# Feature: Totality Checking

## Status: ⏳ 30% Complete

## Overview

Palladium can prove that functions terminate, eliminating entire classes of bugs related to infinite loops and non-termination. This feature enables mathematical reasoning about code correctness.

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
```palladium
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
#[total(decreases = m + n)]
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

### 2. Performance Benefits
- Compiler can optimize total functions more aggressively
- No need for runtime checks
- Better inlining decisions
- Loop unrolling opportunities

### 3. Safety Critical Systems
- Aerospace: No infinite loops in flight control
- Medical: Guaranteed response times
- Finance: Predictable execution
- Embedded: Known resource bounds

## How It Works

### Termination Checking Algorithm
```palladium
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
```palladium
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
#[total(decreases = n)]
fn countdown(n: u64) -> u64 {
    if n == 0 { 0 } else { countdown(n - 1) }
}

// 3. Lexicographic ordering
#[total(decreases = (m, n))]
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
```palladium
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

## Implementation Progress

- [x] Basic structural recursion
- [x] Natural number measures  
- [x] Lexicographic ordering
- [ ] General well-founded relations
- [ ] Fuel-based termination
- [ ] Size-change termination
- [ ] Integration with type system
- [ ] Proof export to Lean/Coq

## Performance Impact

### Compile Time
- +20-30% for totality checking
- Can be disabled for fast iteration

### Runtime
- 5-15% faster for proven-total functions
- No termination checks needed
- Better optimization opportunities

### Binary Size
- Slightly smaller (no runtime checks)
- Proofs not included in binary

## Common Patterns

### List Processing
```palladium
#[total]
fn map<T, U>(list: List<T>, f: Fn(T) -> U) -> List<U> {
    match list {
        Nil => Nil,
        Cons(x, xs) => Cons(f(x), map(xs, f))
    }
}
```

### Tree Algorithms
```palladium
#[total]
fn tree_height<T>(tree: Tree<T>) -> u64 {
    match tree {
        Leaf(_) => 1,
        Node(l, r) => 1 + max(tree_height(l), tree_height(r))
    }
}
```

### Number Theory
```palladium
#[total(decreases = b)]
fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}
```

## Future Improvements

1. **Automatic Measure Inference**: Deduce termination measures
2. **Coinductive Types**: Handle infinite data structures
3. **Dependent Types**: More precise termination proofs
4. **SMT Integration**: Use external solvers for complex cases

## Related Features
- [Refinement Types](./refinement_types.md)
- [Proof Generation](./proof_generation.md)
- [Type System](../core-language/type_system.md)