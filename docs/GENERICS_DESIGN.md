# Palladium Generics Design

## Overview

Generics allow writing code that works with multiple types without duplication. This is essential for collections, algorithms, and reusable abstractions.

## Design Goals

1. **Simple syntax** - Easy to understand and use
2. **Type safety** - Catch errors at compile time
3. **Zero cost** - No runtime overhead
4. **Monomorphization** - Generate specialized code for each type

## Syntax Design

### Generic Functions
```palladium
fn identity<T>(value: T) -> T {
    return value;
}

fn swap<T>(a: &mut T, b: &mut T) {
    let temp = *a;
    *a = *b;
    *b = temp;
}

fn map<T, U>(arr: [T; 10], f: fn(T) -> U) -> [U; 10] {
    let mut result: [U; 10];
    for i in 0..10 {
        result[i] = f(arr[i]);
    }
    return result;
}
```

### Generic Structs
```palladium
struct Pair<T, U> {
    first: T,
    second: U,
}

struct Vec<T> {
    data: *mut T,
    len: usize,
    capacity: usize,
}

impl<T> Vec<T> {
    fn new() -> Vec<T> {
        Vec {
            data: null_mut(),
            len: 0,
            capacity: 0,
        }
    }
    
    fn push(&mut self, value: T) {
        // Implementation
    }
}
```

### Generic Enums
```palladium
enum Option<T> {
    Some(T),
    None,
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

### Type Constraints (Future)
```palladium
// Phase 2: Add trait bounds
fn sum<T: Add>(a: T, b: T) -> T {
    return a + b;
}
```

## Implementation Plan

### Phase 1: Basic Generic Functions (This week)
1. **Lexer/Parser changes**:
   - Add `<` and `>` for type parameters
   - Parse generic function signatures
   - Parse generic type instantiations

2. **AST changes**:
   ```rust
   struct Function {
       type_params: Vec<String>, // ["T", "U"]
       // ... existing fields
   }
   ```

3. **Type checking**:
   - Track generic parameters in scope
   - Substitute concrete types during instantiation
   - Verify type consistency

4. **Code generation**:
   - Monomorphization: generate specialized versions
   - Name mangling for different instantiations

### Phase 2: Generic Structs (Next week)
- Extend parser for struct type parameters
- Handle generic fields in type checker
- Generate specialized struct definitions

### Phase 3: Generic Enums (Following week)
- Similar to structs but with variants
- Special handling for Option and Result

## Example: Implementation Steps

Starting with the simplest case:
```palladium
fn identity<T>(x: T) -> T {
    return x;
}

fn main() {
    let a = identity(42);      // identity<i64>
    let b = identity("hello"); // identity<String>
}
```

### Step 1: Parse generic function
- Recognize `<T>` after function name
- Store type parameters in AST

### Step 2: Type check calls
- When seeing `identity(42)`:
  - Infer T = i64 from argument
  - Check return type matches

### Step 3: Generate code
- Create `identity_i64` function
- Create `identity_string` function
- Replace calls with specialized versions

## C Code Generation

For the identity example:
```c
// Generated for identity<i64>
long long identity_i64(long long x) {
    return x;
}

// Generated for identity<String>
const char* identity_string(const char* x) {
    return x;
}

int main() {
    long long a = identity_i64(42);
    const char* b = identity_string("hello");
}
```

## Challenges

1. **Type inference** - Determining concrete types from usage
2. **Error messages** - Clear errors for type mismatches
3. **Compilation speed** - Avoiding duplicate instantiations
4. **Recursive types** - Handle `struct Node<T> { next: Option<Node<T>> }`

## Testing Strategy

1. Start with identity function
2. Add simple container (Pair)
3. Test with multiple type parameters
4. Verify monomorphization works
5. Check error cases

## Next Steps

1. Add `<` and `>` tokens to lexer
2. Extend function parsing for type parameters
3. Create simple type substitution system
4. Implement monomorphization in codegen