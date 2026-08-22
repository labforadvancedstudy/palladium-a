> **NORMATIVE LANGUAGE DEFINITION.** On the *language* axis this document is normative: it
> defines part of Palladium, and [`language-spec.md` §N10](../specification/language-spec.md#n10-traits-and-generics)
> incorporates it by reference.
>
> **Compiler status: see [annex A4.4](../specification/language-spec.md#a44-traits).** This banner
> deliberately does not restate the status — an earlier version asserted "nothing here is built"
> while the annex recorded working `import` parsing, because a status written in two places goes
> stale in one of them. The annex classifies every feature; this points at it.
>
> Code in this file is not compiled by `scripts/check-docs.sh`. Material that is genuinely
> undecided is under "Open design questions" below and is explicitly **not** normative.

# Palladium Trait System Design
*Version 1.0 - January 19, 2025*

## Overview

Traits in Palladium provide a way to define shared behavior across types. They enable polymorphism, code reuse, and zero-cost abstractions.

## Design Goals

1. **Zero-cost abstractions** - No runtime overhead for static dispatch
2. **Simplicity** - Easy to understand and use
3. **Expressiveness** - Support common patterns from Rust/Haskell
4. **Safety** - Prevent ambiguity and conflicts
5. **Extensibility** - Allow future enhancements

## Core Concepts

### Trait Definition

```palladium
trait Display {
    fn fmt(ref self) -> String;
}

trait Debug {
    fn debug(ref self) -> String {
        // Default implementation
        return "<debug>";
    }
}

trait Clone {
    fn clone(ref self) -> Self;
}
```

### Trait Implementation

```palladium
struct Point {
    x: i64,
    y: i64,
}

impl Display for Point {
    fn fmt(ref self) -> String {
        return format!("({}, {})", self.x, self.y);
    }
}

impl Clone for Point {
    fn clone(ref self) -> Self {
        return Point { x: self.x, y: self.y };
    }
}
```

### Trait Bounds

```palladium
// Function with trait bounds
fn print_twice<T: Display + Clone>(x: ref T) {
    let copy = x.clone();
    print(x.fmt());
    print(copy.fmt());
}

// Multiple bounds syntax
fn complex<T: Display + Debug, U: Clone>(x: T, y: U) -> String {
    return x.fmt();
}
```

### Associated Types

```palladium
trait Iterator {
    type Item;
    
    fn next(ref mut self) -> Option<Self::Item>;
}

impl Iterator for RangeIter {
    type Item = i64;
    
    fn next(ref mut self) -> Option<i64> {
        // Implementation
    }
}
```

### Trait Objects (Dynamic Dispatch)

```palladium
// Trait object type
let displayable: ref dyn Display = ref point;
print(displayable.fmt());

// Box for owned trait objects
let owned: Box<dyn Display> = Box::new(point);
```

## Technical Design

### AST Changes

```rust
// New AST nodes
pub enum Item {
    // ... existing variants
    Trait(TraitDef),
    Impl(ImplBlock),
}

pub struct TraitDef {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub methods: Vec<TraitMethod>,
}

pub struct TraitMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Option<Vec<Stmt>>, // None for required, Some for default
}

pub struct ImplBlock {
    pub trait_name: Option<String>, // None for inherent impl
    pub type_name: String,
    pub type_args: Vec<Type>,
    pub methods: Vec<Function>,
}
```

### Type System Integration

```rust
// Type checker changes
pub struct TraitInfo {
    pub name: String,
    pub methods: HashMap<String, MethodSig>,
    pub impls: HashMap<TypeId, ImplId>,
}

pub struct TypeChecker {
    // ... existing fields
    traits: HashMap<String, TraitInfo>,
    impls: Vec<ImplBlock>,
}
```

### Method Resolution

1. Check inherent methods first
2. Check trait methods in scope
3. Error on ambiguity
4. Suggest trait imports

### Code Generation

For static dispatch:
```c
// Trait: Display for Point
char* Point_Display_fmt(Point* self) {
    // Implementation
}

// Generic function instantiation
void print_twice_Point(Point* x) {
    Point copy = Point_Clone_clone(x);
    print(Point_Display_fmt(x));
    print(Point_Display_fmt(&copy));
}
```

For dynamic dispatch:
```c
// Vtable for Display trait
typedef struct {
    char* (*fmt)(void* self);
} Display_vtable;

// Trait object
typedef struct {
    void* data;
    Display_vtable* vtable;
} dyn_Display;
```

## Syntax Examples

### Basic Usage

```palladium
trait Drawable {
    fn draw(ref self, canvas: ref mut Canvas);
    fn bounding_box(ref self) -> Rect;
}

struct Circle {
    center: Point,
    radius: f64,
}

impl Drawable for Circle {
    fn draw(ref self, canvas: ref mut Canvas) {
        canvas.draw_circle(self.center, self.radius);
    }
    
    fn bounding_box(ref self) -> Rect {
        return Rect {
            x: self.center.x - self.radius,
            y: self.center.y - self.radius,
            width: self.radius * 2.0,
            height: self.radius * 2.0,
        };
    }
}
```

### Generic Traits

```palladium
trait Container<T> {
    fn len(ref self) -> usize;
    fn get(ref self, index: usize) -> Option<ref T>;
}

impl<T> Container<T> for Vec<T> {
    fn len(ref self) -> usize {
        return self.length;
    }
    
    fn get(ref self, index: usize) -> Option<ref T> {
        if index < self.length {
            return Some(ref self.data[index]);
        }
        return None;
    }
}
```

### Operator Overloading

```palladium
trait Add<Rhs = Self> {
    type Output;
    fn add(self, rhs: Rhs) -> Self::Output;
}

impl Add for Point {
    type Output = Point;
    
    fn add(self, other: Point) -> Point {
        return Point {
            x: self.x + other.x,
            y: self.y + other.y,
        };
    }
}

// Usage
let p3 = p1 + p2; // Calls Point::add(p1, p2)
```

## Error Messages

### Missing Implementation
```
error: the trait `Display` is not implemented for `Point`
  --> src/main.pd:10:5
   |
10 |     print_it(&point);
   |     ^^^^^^^^ required by `print_it`
   |
help: implement the trait for `Point`:
   |
   | impl Display for Point {
   |     fn fmt(&self) -> String {
   |         // implementation
   |     }
   | }
```

### Ambiguous Method
```
error: multiple applicable methods named `fmt`
  --> src/main.pd:15:10
   |
15 |     x.fmt()
   |       ^^^ ambiguous
   |
note: candidate #1 is defined in the trait `Display`
note: candidate #2 is defined in the trait `Debug`
help: disambiguate with:
   |
   | Display::fmt(&x)
   | Debug::fmt(&x)
```

## Comparison with Other Languages

### Rust
- Similar syntax and semantics
- No lifetime parameters (yet)
- Simpler trait objects
- No negative bounds

### Haskell
- Traits = Type classes
- No higher-kinded types (yet)
- More imperative style

### Swift
- Traits = Protocols
- Similar capabilities
- Different syntax

## Future Extensions

1. **Const traits** - Traits usable in const contexts
2. **Async traits** - Traits with async methods
3. **Specialization** - Optimized implementations
4. **Higher-ranked traits** - For<'a> syntax
5. **Negative bounds** - T: !Send

## Conclusion

The trait system provides Palladium with powerful abstraction capabilities while maintaining zero-cost guarantees. Starting with a simple implementation and gradually adding features ensures a solid foundation for the type system.

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

<sub>**Non-normative.** This was above the fold when the dual-axis banner landed, which silently
promoted a work schedule to normative language definition. A schedule is neither a definition nor a
measurement: these week estimates were written in January 2025 and none of the work happened.
Implementation status is the annex's job
([`language-spec.md` Part II](../specification/language-spec.md#part-ii-implementation-status-annex)).</sub>

## Implementation Plan

### Phase 1: Basic Traits (Week 1)
1. Trait definitions in AST
2. Trait implementations
3. Method resolution
4. Trait bounds on functions
5. Static dispatch

### Phase 2: Advanced Features (Week 2)
1. Associated types
2. Default implementations
3. Trait objects (dyn)
4. Multiple trait bounds
5. Where clauses

### Phase 3: Standard Traits (Week 3)
1. Display, Debug
2. Clone, Copy
3. Eq, Ord
4. Iterator
5. From, Into

### Relocated: Implementation Priority

<sub>**Non-normative, and the status marks were false.** Three items were marked complete — basic
trait definitions, simple implementations, method resolution. Measured at `abeb665`: traits parse
(`src/parser/mod.rs:1225`) and then emit nothing. Codegen ignores `Item::Trait`
(`src/codegen/mod.rs:1039`), the type checker skips trait bodies (`src/typeck/mod.rs:854`), and a
trait method declared with a `self` receiver is a parse error. **Nothing in this list is done.** The
ordering is kept because the sequencing is still a design argument; the marks are removed because
they were claims, and false ones.</sub>

1. Basic trait definitions
2. Simple implementations
3. Method resolution
4. Trait bounds
5. Associated types
6. Trait objects
7. Standard library traits
