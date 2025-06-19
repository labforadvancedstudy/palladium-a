# Palladium Semantic Specification

This document specifies the semantic rules and behavior of the Palladium programming language.

## 1. Name Resolution

### 1.1 Scoping Rules

Palladium uses lexical scoping with the following rules:

1. **Block Scope**: Variables declared with `let` are scoped to the enclosing block
2. **Function Scope**: Function parameters are scoped to the function body
3. **Module Scope**: Items declared at module level are visible throughout the module
4. **Import Scope**: Imported items are visible from the import statement onwards

### 1.2 Name Lookup Order

Names are resolved in the following order:

1. Local variables in the current block
2. Parameters of the enclosing function
3. Items in the current module
4. Imported items
5. Items in the prelude

### 1.3 Shadowing

Variables can shadow previous bindings:

```palladium
let x = 5;
let x = "hello";  // Shadows previous x
```

## 2. Type Checking

### 2.1 Type Inference Algorithm

Palladium uses bidirectional type checking:

```
Γ ⊢ e ⇒ τ    (synthesis: expression e synthesizes type τ)
Γ ⊢ e ⇐ τ    (checking: expression e checks against type τ)
```

### 2.2 Subtyping

Palladium has limited subtyping:

1. **Lifetime Subtyping**: `'a: 'b` means `'a` outlives `'b`
2. **Type Parameter Bounds**: `T: Trait` means `T` implements `Trait`

### 2.3 Type Coercion

Implicit coercions are limited to:

1. **Deref Coercion**: `&String` to `&str`
2. **Subtype Coercion**: Longer lifetime to shorter lifetime

## 3. Ownership and Borrowing

### 3.1 Ownership Rules

```
owned(x) ∧ move(x, y) → owned(y) ∧ ¬valid(x)
```

1. Each value has exactly one owner
2. Moving transfers ownership
3. Dropping occurs when owner goes out of scope

### 3.2 Borrowing Rules

```
borrow(&x) → read(x) ∧ ¬move(x)
borrow(&mut x) → read(x) ∧ write(x) ∧ ¬alias(x)
```

1. Multiple shared references OR one mutable reference
2. References cannot outlive their referent
3. No aliasing of mutable references

### 3.3 Lifetime Inference

Lifetime parameters are inferred using these rules:

1. **Input Lifetimes**: Each reference parameter gets its own lifetime
2. **Output Lifetimes**: Must be tied to input lifetimes
3. **Elision Rules**: Common patterns have lifetimes inferred

## 4. Memory Management

### 4.1 Allocation

```palladium
// Stack allocation
let x = 42;
let arr = [1, 2, 3];

// Heap allocation (through stdlib)
let v = Vec::new();
let s = String::from("hello");
```

### 4.2 Deallocation

Values are deallocated when:

1. Owner goes out of scope
2. Owner is reassigned
3. Explicit drop (for types implementing Drop)

### 4.3 Memory Safety Invariants

1. **No use-after-free**: Enforced by borrow checker
2. **No double-free**: Enforced by move semantics
3. **No null pointers**: No null in safe code
4. **No uninitialized memory**: All values must be initialized

## 5. Control Flow

### 5.1 Expression Evaluation Order

1. **Left-to-right**: For function arguments and operators
2. **Short-circuit**: For `&&` and `||` operators
3. **Eager**: For arithmetic and bitwise operators

### 5.2 Pattern Matching Semantics

```
match value {
    pattern₁ => expr₁,
    pattern₂ => expr₂,
    ...
    patternₙ => exprₙ,
}
```

1. Patterns checked top-to-bottom
2. First matching pattern is selected
3. Match must be exhaustive
4. Guards evaluated after pattern match

### 5.3 Loop Semantics

```palladium
// While loop
while condition {
    // condition evaluated before each iteration
}

// For loop
for item in iterator {
    // Desugars to iterator protocol
}
```

## 6. Function Calls

### 6.1 Calling Convention

1. Arguments evaluated left-to-right
2. Arguments moved or copied into parameters
3. Return value moved to caller

### 6.2 Method Resolution

```
receiver.method(args)
```

1. Check inherent methods on receiver type
2. Check trait methods in scope
3. Apply deref coercion if needed

### 6.3 Generic Instantiation

```
fn foo<T: Display>(x: T) { ... }
foo(42);  // T instantiated as i32
```

Type parameters instantiated at call site.

## 7. Traits

### 7.1 Trait Implementation

```palladium
impl Trait for Type {
    // All required methods must be implemented
    // Default methods can be overridden
}
```

### 7.2 Trait Bounds

```
where T: Trait₁ + Trait₂
```

Type must implement all listed traits.

### 7.3 Associated Types

```palladium
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
```

Associated types are determined by implementation.

## 8. Effects System

### 8.1 Effect Inference

Effects are inferred from function bodies:

```palladium
fn read_file(path: String) -> String {
    // Uses IO operations -> infers ![io]
}
```

### 8.2 Effect Polymorphism

```palladium
fn map<T, U, E>(f: fn(T) -> U ![E], list: Vec<T>) -> Vec<U> ![E] {
    // Effect E is polymorphic
}
```

### 8.3 Async as Effect

```palladium
async fn foo() -> i32 {
    // Implicitly has ![async] effect
}
```

## 9. Const Evaluation

### 9.1 Const Context

The following are evaluated at compile time:

1. Array sizes
2. Const generic arguments
3. Const expressions
4. Static initializers

### 9.2 Const Functions

```palladium
const fn compute() -> i32 {
    // Only const operations allowed
    42
}
```

### 9.3 Const Restrictions

Not allowed in const context:

1. Heap allocation
2. Function pointers
3. Trait objects
4. Non-const function calls

## 10. Unsafe Code

### 10.1 Unsafe Operations

The following require `unsafe`:

1. Dereferencing raw pointers
2. Calling unsafe functions
3. Accessing mutable statics
4. Implementing unsafe traits

### 10.2 Unsafe Invariants

Unsafe code must maintain:

1. Memory safety
2. Type safety
3. Thread safety

### 10.3 Undefined Behavior

The following cause undefined behavior:

1. Data races
2. Dereferencing invalid pointers
3. Breaking aliasing rules
4. Invalid type punning

## 11. Module System

### 11.1 Module Resolution

```palladium
mod foo;  // Looks for foo.pd or foo/mod.pd
```

### 11.2 Visibility

```palladium
pub fn public() {}
fn private() {}
pub(crate) fn crate_visible() {}
```

### 11.3 Import Resolution

```palladium
use std::collections::HashMap;
use std::io::{Read, Write};
```

## 12. Macros

### 12.1 Macro Expansion

Macros are expanded before type checking:

```palladium
macro vec(x) => {
    let mut v = Vec::new();
    v.push(x);
    v
}
```

### 12.2 Hygiene

Macros are hygienic:
- Local variables don't leak
- Names resolved in definition context

## 13. Error Handling

### 13.1 Result Type

```palladium
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

### 13.2 Question Mark Operator

```palladium
fn foo() -> Result<i32, Error> {
    let x = bar()?;  // Returns early on Err
    Ok(x + 1)
}
```

### 13.3 Panic Semantics

Panics unwind the stack unless caught.

## 14. Concurrency

### 14.1 Send and Sync

- `Send`: Safe to transfer between threads
- `Sync`: Safe to share between threads

### 14.2 Data Race Prevention

The type system prevents data races:
- No mutable aliasing
- Send/Sync bounds for threading

## 15. Standard Library Integration

### 15.1 Lang Items

Special compiler-known items:

```palladium
#[lang = "drop"]
trait Drop {
    fn drop(&mut self);
}
```

### 15.2 Intrinsics

Compiler magic functions:

```palladium
#[intrinsic]
fn size_of<T>() -> usize;
```

---

This semantic specification is normative and defines the expected behavior of conforming Palladium implementations.