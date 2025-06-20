# Chapter 6: Traits are Promises

*Or: How to Teach Old Types New Tricks*

Imagine you're organizing a talent show. You don't care if performers are singers, dancers, or magicians—you just care that they can perform. That's what traits are: promises about what something can do, not what it is.

## The Basic Promise

A trait is a contract. It says "If you implement me, you promise to provide these abilities":

```palladium
// Coming in v0.9! Currently in development
trait Drawable {
    fn draw(&self);
}
```

This trait makes a simple promise: "I can be drawn." Any type that implements this trait must provide a `draw` method.

## Why Traits Matter

Without traits, you'd write separate functions for each type:

```palladium
fn draw_circle(c: Circle) { /* ... */ }
fn draw_square(s: Square) { /* ... */ }
fn draw_triangle(t: Triangle) { /* ... */ }
```

With traits, you write one function for anything drawable:

```palladium
fn draw_shape(shape: &impl Drawable) {
    shape.draw();
}
```

It's like saying "I don't care if you're a singer or dancer, just perform!"

## Common Traits (Coming in v0.9)

Palladium will provide essential traits that types can implement:

### Display - "I can show myself"
```palladium
trait Display {
    fn fmt(&self) -> String;
}
```

### Clone - "I can duplicate myself"
```palladium
trait Clone {
    fn clone(&self) -> Self;
}
```

### Eq - "I can be compared"
```palladium
trait Eq {
    fn eq(&self, other: &Self) -> bool;
}
```

## How Traits Work (Design)

When v0.9 lands, you'll implement traits like this:

```palladium
struct Point {
    x: i32,
    y: i32,
}

impl Display for Point {
    fn fmt(&self) -> String {
        format!("({}, {})", self.x, self.y)
    }
}

// Now Point has made a promise: it can display itself!
```

## Generic Traits: Ultimate Flexibility

Traits can be generic too:

```palladium
trait Container<T> {
    fn add(&mut self, item: T);
    fn get(&self, index: usize) -> Option<&T>;
}
```

This is like saying "I promise to be a container for any type T you choose."

## Trait Bounds: Requiring Promises

You can require types to make certain promises:

```palladium
fn print_twice<T: Display>(item: T) {
    println!("{}", item.fmt());
    println!("{}", item.fmt());
}
```

This says: "I'll work with any type T, as long as T promises it can display itself."

## Multiple Traits: Many Talents

Types can implement multiple traits:

```palladium
struct Dog {
    name: String,
}

impl Display for Dog {
    fn fmt(&self) -> String {
        format!("Dog named {}", self.name)
    }
}

impl Clone for Dog {
    fn clone(&self) -> Dog {
        Dog { name: self.name.clone() }
    }
}

// Dog can now display itself AND clone itself!
```

## Default Implementations

Traits can provide default behavior:

```palladium
trait Greet {
    fn name(&self) -> &str;
    
    fn greet(&self) {  // Default implementation
        println!("Hello, I'm {}", self.name());
    }
}
```

Types get the default for free but can override if needed!

## Associated Types: Part of the Promise

Sometimes a trait needs to specify related types:

```palladium
trait Iterator {
    type Item;  // What type of items do I produce?
    
    fn next(&mut self) -> Option<Self::Item>;
}
```

It's like saying "I promise to iterate, and I'll tell you what type I produce."

## Real-World Example: Building a Game

Let's see how traits help build clean, extensible code:

```palladium
// Define what game objects can do
trait GameObject {
    fn update(&mut self, delta_time: f32);
    fn render(&self);
}

trait Collidable {
    fn bounds(&self) -> Rectangle;
    fn on_collision(&mut self, other: &dyn GameObject);
}

trait Destructible {
    fn take_damage(&mut self, amount: i32);
    fn is_destroyed(&self) -> bool;
}

// Different game objects implement different combinations
struct Player {
    x: f32,
    y: f32,
    health: i32,
}

impl GameObject for Player {
    fn update(&mut self, delta_time: f32) {
        // Update player position
    }
    
    fn render(&self) {
        // Draw player sprite
    }
}

impl Collidable for Player {
    fn bounds(&self) -> Rectangle {
        Rectangle::new(self.x, self.y, 32.0, 32.0)
    }
    
    fn on_collision(&mut self, other: &dyn GameObject) {
        // Handle collision
    }
}

impl Destructible for Player {
    fn take_damage(&mut self, amount: i32) {
        self.health -= amount;
    }
    
    fn is_destroyed(&self) -> bool {
        self.health <= 0
    }
}
```

## What's Available Now (v0.8-alpha)

Currently, Palladium supports:
- Basic trait definitions
- Simple implementations
- Core traits are being developed

Full trait support with all features lands in v0.9-beta (February 2025).

## Traits vs Inheritance

Unlike inheritance (which says "I am a kind of X"), traits say "I can do X":

```palladium
// Inheritance thinking (not in Palladium):
// class Dog extends Animal { }  // Dog IS an Animal

// Trait thinking (Palladium way):
impl Bark for Dog { }     // Dog CAN bark
impl Run for Dog { }      // Dog CAN run
impl Swim for Dog { }     // Dog CAN swim
```

A dog isn't defined by what it IS, but by what it CAN DO!

## The Orphan Rule

Palladium enforces a simple rule: you can only implement a trait for a type if you own either the trait or the type. This prevents chaos:

```palladium
// ✅ OK - You own Dog
impl Display for Dog { }

// ✅ OK - You own MyTrait
impl MyTrait for String { }

// ❌ Error - You don't own Display or String
// impl Display for String { }
```

This keeps code predictable and prevents conflicts.

## Zero-Cost Abstraction

The beautiful part? Traits compile to zero-overhead code:

```palladium
// This trait-based code:
fn process<T: Display>(item: T) {
    println!("{}", item.fmt());
}

// Compiles to the same machine code as:
fn process_point(item: Point) {
    println!("{}", point_fmt(item));
}
```

Abstraction without performance penalty!

## Try It Today

While waiting for full trait support, you can experiment with basic patterns:

```palladium
// Works in v0.8-alpha
struct Calculator;

impl Calculator {
    fn add(a: i32, b: i32) -> i32 {
        a + b
    }
    
    fn multiply(a: i32, b: i32) -> i32 {
        a * b
    }
}

fn main() {
    let sum = Calculator::add(5, 3);
    print_int(sum);  // 8
    
    let product = Calculator::multiply(4, 7);
    print_int(product);  // 28
}
```

## Mental Model

Think of traits like job requirements:

- **Type** = Job applicant
- **Trait** = Job requirements
- **impl** = Resume showing you meet requirements
- **Trait bounds** = "Must have these skills"

Just like you can have multiple skills, types can implement multiple traits!

## Common Patterns

### The Builder Pattern
```palladium
trait Builder {
    type Output;
    fn build(self) -> Self::Output;
}
```

### The Iterator Pattern
```palladium
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
```

### The From/Into Pattern
```palladium
trait From<T> {
    fn from(value: T) -> Self;
}
```

## Looking Forward

When v0.9 ships with full trait support, you'll be able to:
- Define complex trait hierarchies
- Use trait objects for dynamic dispatch
- Leverage derive macros for automatic implementations
- Build powerful generic abstractions

Until then, focus on understanding the concept: **Traits are promises about capabilities, not identity.**

[Next: Chapter 7 - Async is Just Waiting →](chapter_7_async.md)

---

*Exercise: Design traits for a music player. What abilities would different components need? Think about:*
- *Playing songs*
- *Managing playlists*
- *Showing progress*
- *Handling errors*

*How would traits make the design cleaner than inheritance?*