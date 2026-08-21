> **NORMATIVE LANGUAGE DEFINITION.** On the *language* axis this document is normative: it
> defines part of Palladium, and [`language-spec.md` §N11](../specification/language-spec.md#n11-modules)
> incorporates it by reference.
>
> **Compiler status: see [annex A3](../specification/language-spec.md#a3-program-structure).** This banner
> deliberately does not restate the status — an earlier version asserted "nothing here is built"
> while the annex recorded working `import` parsing, because a status written in two places goes
> stale in one of them. The annex classifies every feature; this points at it.
>
> Code in this file is not compiled by `scripts/check-docs.sh`. Material that is genuinely
> undecided is under "Open design questions" below and is explicitly **not** normative.

# Palladium Module System Design

## Overview

The module system will allow Palladium code to be organized across multiple files and enable code reuse through imports.

## Design Principles

1. **File-based modules** - Each `.pd` file is a module
2. **Explicit exports** - Only exported items are visible outside the module
3. **Named imports** - Import specific items from modules
4. **No circular dependencies** - Compilation order must be determinable

## Syntax Design

### Module Declaration (implicit)
Every `.pd` file is automatically a module. The module name is derived from the file path.

### Exports
```palladium
// math.pd
pub fn add(a: i64, b: i64) -> i64 {
    return a + b;
}

pub struct Point {
    x: i64,
    y: i64,
}

// Private function - not visible outside
fn helper() -> i64 {
    return 42;
}
```

### Imports
```palladium
// main.pd
import math::add;
import math::Point;

// Or import everything public
import math::*;

fn main() {
    let sum = add(1, 2);
    let p = Point { x: 10, y: 20 };
}
```

### Nested Modules
```palladium
// std/vec.pd
pub struct Vec<T> {
    // ...
}

// main.pd
import std::vec::Vec;
```

## Technical Challenges

1. **C Code Generation**
   - Need to generate header files for each module
   - Handle symbol name mangling to avoid conflicts
   - Manage compilation order

2. **Type Checking**
   - Cross-module type checking
   - Generic type parameters across modules

3. **Circular Dependencies**
   - Detect and prevent circular imports
   - Clear error messages

## Example: Complete Module

```palladium
// collections/list.pd
import std::option::Option;

pub struct List<T> {
    head: Option<Node<T>>,
}

struct Node<T> {
    value: T,
    next: Option<Box<Node<T>>>,
}

pub fn new<T>() -> List<T> {
    List { head: None }
}

pub fn push<T>(list: ref mut List<T>, value: T) {
    // Implementation
}
```

## C Code Generation Strategy

Each Palladium module will generate:
1. A `.h` header file with public declarations
2. A `.c` implementation file

Example:
```c
// math.h
#ifndef PALLADIUM_MATH_H
#define PALLADIUM_MATH_H

long long pd_math_add(long long a, long long b);

typedef struct {
    long long x;
    long long y;
} pd_math_Point;

#endif

// math.c
#include "math.h"

long long pd_math_add(long long a, long long b) {
    return a + b;
}
```

## Open design questions

<sub>**Non-normative.** Everything above defines the language; everything in this section is
undecided and defines nothing. The distinction matters because "not yet built" and "not yet
decided" were previously carried by the same PROPOSAL banner, which made every open question look
like settled design awaiting an implementer.</sub>

Nothing in this document has been escalated as an open *design* question yet. What is here is
material that was carried above the fold as though it were definitional and is not: work
schedules, and status marks that were false. It sits here so that the banner above cannot be read
as blessing it.

### Relocated: Implementation Plan

<sub>**Non-normative.** This was above the fold when the dual-axis banner landed, which
silently promoted a work schedule to normative language definition. A schedule is neither a
definition nor a measurement: these week estimates were written in January 2025 and none of
the work happened. Implementation status is the annex's job
([`language-spec.md` Part II](../specification/language-spec.md#part-ii-implementation-status-annex)).</sub>

## Implementation Plan

### Phase 1: Basic File Imports (1 week)
1. Add `import` keyword to lexer
2. Add `pub` keyword for public visibility
3. Parse import statements
4. Track module dependencies
5. Compile modules in dependency order

### Phase 2: Symbol Resolution (1 week)
1. Build module symbol tables
2. Resolve imported symbols
3. Check visibility rules
4. Generate appropriate C includes

### Phase 3: Standard Library Modules (1 week)
1. Organize stdlib into modules
2. Create std::vec, std::string, std::io modules
3. Update examples to use modules

### Relocated: Next Steps

<sub>**Non-normative.** An implementation task list, which the banner above would otherwise
make part of the language definition.</sub>

1. Implement lexer changes for `import` and `pub`
2. Design AST nodes for imports
3. Create module resolution algorithm
4. Update code generator for multi-file output

