# Chapter 3: Types are Shapes

*Or: Why Fitting Square Pegs in Round Holes is Actually Impossible*

When you were a child, you probably played with a shape-sorting toy. Square blocks go in square holes, circles in round holes. You couldn't force a square into a round hole no matter how hard you tried.

Types in Palladium work exactly the same way. And just like that toy, this isn't a limitation—it's what makes the whole thing work.

## What is a Type, Really?

A type is just a shape. It tells you:
1. How many boxes (bytes) something needs
2. What you can do with it
3. What it means

Let's start simple:

```palladium
let age: i32 = 25;
```

The type `i32` means:
- **Shape**: Uses 4 boxes (32 bits)
- **Abilities**: Can add, subtract, compare, etc.
- **Meaning**: A whole number from -2,147,483,648 to 2,147,483,647

That's it. A type is just a promise about shape and behavior.

## The Basic Shapes

Palladium has a toy box full of basic shapes:

```palladium
// Numbers
let count: i32 = 42;           // 32-bit integer
let pi: f64 = 3.14159;         // 64-bit float
let is_ready: bool = true;     // true or false

// Text
let letter: char = 'A';        // Single Unicode character
let name: &str = "Alice";      // Borrowed text
let message: String = String::from("Hello"); // Owned text

// Collections
let coords: (i32, i32) = (10, 20);  // Tuple
let scores: [i32; 3] = [90, 85, 88]; // Array (fixed size)
let items: Vec<i32> = vec![1, 2, 3]; // Vector (growable)
```

## Custom Shapes: Structs

You can create your own shapes by combining others:

```palladium
struct Point {
    x: f64,
    y: f64,
}

let p = Point { x: 3.0, y: 4.0 };
```

This is like building with Lego blocks. A `Point` is just two `f64`s glued together.

How much space does it need? Easy:
- `f64` = 8 boxes
- Two of them = 16 boxes
- So `Point` = 16 boxes

## The Magic of Enums

Here's where Palladium gets clever. What if something could be different shapes at different times?

```palladium
enum Message {
    Quit,                       // No data
    Move { x: i32, y: i32 },   // Two numbers
    Write(String),              // Some text
    ChangeColor(i32, i32, i32), // Three numbers (RGB)
}
```

An enum is like a Swiss Army knife—it can be different tools, but only one at a time.

How much space? Palladium is smart:
- It needs space for the biggest variant
- Plus a tiny tag to remember which one it is

## Option: The Billion Dollar Solution

Tony Hoare called null references his "billion dollar mistake." Palladium fixes this with `Option`:

```palladium
enum Option<T> {
    Some(T),  // We have a value
    None,     // We don't have a value
}
```

Instead of values that might secretly be null:

```c
// C - danger!
char* name = get_name();  // Might be NULL!
printf("%s", name);       // Crash if NULL!
```

Palladium forces you to handle both cases:

```palladium
let name: Option<String> = get_name();
match name {
    Some(n) => println!("Hello, {}", n),
    None => println!("No name provided"),
}
```

You literally cannot forget to check. The square peg won't fit in the round hole!

## Type Inference: The Smart Assistant

Here's something beautiful—Palladium usually knows what shape you want:

```palladium
let x = 42;          // Palladium knows this is i32
let y = 3.14;        // Palladium knows this is f64
let v = vec![1, 2];  // Palladium knows this is Vec<i32>
```

It's like having an assistant who says, "Oh, you're putting 42 in that box? That's obviously an integer shape!"

But sometimes you need to be specific:

```palladium
let small = 42u8;    // No, I want a tiny box (8 bits)
let big = 42i64;     // No, I want a huge box (64 bits)
```

## Generics: Shape Templates

What if you want to write code that works with any shape? That's what generics are for:

```palladium
struct Box<T> {
    value: T,
}

let int_box = Box { value: 42 };        // Box<i32>
let str_box = Box { value: "Hello" };   // Box<&str>
```

It's like having a box that adjusts its size to whatever you put in it. The `T` is a placeholder—"any shape goes here."

## Functions Have Types Too!

This might blow your mind: functions are shapes too!

```palladium
fn add(a: i32, b: i32) -> i32 {
    a + b
}

let operation: fn(i32, i32) -> i32 = add;
```

The type `fn(i32, i32) -> i32` means:
- Takes two `i32` shapes
- Returns one `i32` shape

You can pass functions around like any other value!

## The Zero-Cost Promise

Here's the beautiful part: all these type checks happen at compile time. Once your shapes fit, the compiler throws away the shape information and generates pure, fast machine code.

```palladium
// This safe Palladium code:
let numbers: Vec<i32> = vec![1, 2, 3, 4, 5];
let sum: i32 = numbers.iter().sum();

// Compiles to the same assembly as this C:
int numbers[] = {1, 2, 3, 4, 5};
int sum = 0;
for (int i = 0; i < 5; i++) {
    sum += numbers[i];
}
```

Safety without speed penalty. That's the zero-cost abstraction promise.

## Real Example: A Safe Calculator

Let's build something real:

```palladium
enum Operation {
    Add(f64, f64),
    Subtract(f64, f64),
    Multiply(f64, f64),
    Divide(f64, f64),
}

fn calculate(op: Operation) -> Result<f64, String> {
    match op {
        Operation::Add(a, b) => Ok(a + b),
        Operation::Subtract(a, b) => Ok(a - b),
        Operation::Multiply(a, b) => Ok(a * b),
        Operation::Divide(a, b) => {
            if b == 0.0 {
                Err("Division by zero!".to_string())
            } else {
                Ok(a / b)
            }
        }
    }
}

fn main() {
    let ops = vec![
        Operation::Add(10.0, 5.0),
        Operation::Divide(10.0, 0.0),
        Operation::Multiply(3.0, 7.0),
    ];
    
    for op in ops {
        match calculate(op) {
            Ok(result) => println!("Result: {}", result),
            Err(error) => println!("Error: {}", error),
        }
    }
}
```

Notice how the types guide us:
- `Operation` ensures we handle all cases
- `Result` forces us to handle errors
- No null pointers, no crashes, no surprises

## The Payoff

By thinking of types as shapes:

1. **Errors caught at compile time** - Wrong shapes don't fit
2. **No type errors at runtime** - If it compiles, the shapes match
3. **Better performance** - Compiler knows exact shapes, optimizes perfectly
4. **Clearer code** - Types document what things are

## A Fun Challenge

Here's broken code. What's wrong with the shapes?

```palladium
fn get_first_word(s: String) -> &str {
    s.split_whitespace().next().unwrap_or("")
}
```

Hint: Think about ownership and lifetimes. Who owns the boxes?

[Next: Chapter 4 - Functions are Machines →](chapter_4_functions.md)

---

*Exercise: Design a type for a chess piece. It should know:*
- *What kind of piece it is (pawn, rook, etc.)*
- *What color it is*
- *What square it's on (or if it's captured)*

*How would you represent this with Palladium's type system?*