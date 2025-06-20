# Chapter 4: Functions are Machines

*Or: How to Build Factories That Never Break*

Think of a function as a machine in a factory. You put something in one end, the machine does its job, and something comes out the other end. Simple, right?

But here's the thing: most programming languages let you build broken machines. Machines that explode. Machines that eat your input and give nothing back. Machines that sometimes work and sometimes don't.

Palladium won't let you build broken machines.

## The Simplest Machine

Let's start with the world's simplest machine:

```palladium
fn add_one(x: i32) -> i32 {
    x + 1
}
```

This machine:
- **Input**: One i32 (integer)
- **Process**: Add 1
- **Output**: One i32

You CANNOT:
- Give it a string (wrong shape input)
- Get a float back (wrong shape output)
- Get nothing back (machine must produce output)

It's like a vending machine that only accepts quarters and always gives you exactly one candy bar. No exceptions.

## Ownership: Who Owns What?

Here's where functions get interesting. When you pass something to a function, who owns it?

### Taking Ownership (The Hungry Machine)

```palladium
fn eat_string(s: String) {
    println!("Nom nom: {}", s);
}  // s is destroyed here

let my_string = String::from("Cookie");
eat_string(my_string);
// println!("{}", my_string);  // Error! The machine ate it!
```

This machine CONSUMES its input. Like feeding paper to a shredder—you're not getting it back!

### Borrowing (The Inspection Machine)

```palladium
fn measure_string(s: &String) -> usize {
    s.len()
}

let my_string = String::from("Hello");
let length = measure_string(&my_string);
println!("'{}' is {} chars long", my_string, length);  // Still have it!
```

This machine just LOOKS at its input. Like a security scanner—your bag goes through, but you get it back.

### Mutable Borrowing (The Modification Machine)

```palladium
fn make_exciting(s: &mut String) {
    s.push_str("!!!");
}

let mut my_string = String::from("Hello");
make_exciting(&mut my_string);
println!("{}", my_string);  // "Hello!!!"
```

This machine MODIFIES its input. Like a car wash—same car, but cleaner!

## Multiple Inputs and Outputs

Real machines often need multiple inputs:

```palladium
fn swap(x: i32, y: i32) -> (i32, i32) {
    (y, x)
}

let (a, b) = swap(10, 20);
// a = 20, b = 10
```

Notice how we return multiple values using a tuple? It's like a machine with multiple output slots.

## Machines That Might Fail

In the real world, machines break. Palladium handles this elegantly:

```palladium
fn divide(a: f64, b: f64) -> Result<f64, String> {
    if b == 0.0 {
        Err("Cannot divide by zero".to_string())
    } else {
        Ok(a / b)
    }
}

// Using it:
match divide(10.0, 2.0) {
    Ok(result) => println!("Result: {}", result),
    Err(error) => println!("Error: {}", error),
}
```

This machine has two output slots:
- Success slot: Contains the result
- Error slot: Contains an error message

You MUST check which slot your output came from!

## Generic Machines (One Size Fits All)

What if you want a machine that works with any type?

```palladium
fn identity<T>(x: T) -> T {
    x
}

let a = identity(42);        // Works with i32
let b = identity("Hello");   // Works with &str
let c = identity(vec![1,2]); // Works with Vec<i32>
```

It's like a tube—whatever shape goes in, the same shape comes out!

Here's a more useful generic machine:

```palladium
fn first<T>(items: &Vec<T>) -> Option<&T> {
    if items.is_empty() {
        None
    } else {
        Some(&items[0])
    }
}
```

This machine:
- Works with vectors of ANY type
- Returns the first item (if it exists)
- Borrows, doesn't consume

## Closures: Portable Mini-Machines

Sometimes you need a tiny machine you can carry around:

```palladium
let add_ten = |x| x + 10;
let result = add_ten(32);  // 42
```

Closures can capture their environment—like a machine with memory:

```palladium
let multiplier = 3;
let multiply = |x| x * multiplier;

let numbers = vec![1, 2, 3, 4, 5];
let tripled: Vec<i32> = numbers.iter()
    .map(|&x| multiply(x))
    .collect();
// tripled = [3, 6, 9, 12, 15]
```

## Function Pointers: Machines as Values

In Palladium, machines themselves are values you can pass around:

```palladium
fn apply_operation(x: i32, y: i32, op: fn(i32, i32) -> i32) -> i32 {
    op(x, y)
}

fn add(a: i32, b: i32) -> i32 { a + b }
fn multiply(a: i32, b: i32) -> i32 { a * b }

let sum = apply_operation(5, 3, add);      // 8
let product = apply_operation(5, 3, multiply); // 15
```

It's like having a universal machine that accepts other machines as attachments!

## Pure Functions: Predictable Machines

The best machines are predictable—same input always gives same output:

```palladium
// Pure function - predictable
fn double(x: i32) -> i32 {
    x * 2
}

// Impure function - unpredictable
fn get_random() -> i32 {
    // ... returns different values each time
}
```

Palladium encourages pure functions because they're:
- Easy to test
- Easy to understand
- Easy to optimize
- Can't cause surprises

## Methods: Machines Attached to Data

Sometimes it makes sense to attach machines directly to data:

```palladium
struct Rectangle {
    width: f64,
    height: f64,
}

impl Rectangle {
    // Constructor machine
    fn new(width: f64, height: f64) -> Rectangle {
        Rectangle { width, height }
    }
    
    // Inspection machine (borrows self)
    fn area(&self) -> f64 {
        self.width * self.height
    }
    
    // Modification machine (mutably borrows self)
    fn double_size(&mut self) {
        self.width *= 2.0;
        self.height *= 2.0;
    }
    
    // Consumption machine (takes ownership)
    fn destroy(self) {
        println!("Rectangle destroyed!");
        // self is dropped here
    }
}

let mut rect = Rectangle::new(10.0, 20.0);
println!("Area: {}", rect.area());
rect.double_size();
println!("New area: {}", rect.area());
```

## Real-World Example: A Text Processing Pipeline

Let's build a real machine assembly line:

```palladium
// Machine 1: Clean text
fn clean_text(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect()
}

// Machine 2: Split into words
fn split_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

// Machine 3: Count frequencies
fn count_words(words: Vec<String>) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }
    counts
}

// Assembly line!
fn analyze_text(text: &str) -> Vec<(String, usize)> {
    let clean = clean_text(text);
    let words = split_words(&clean);
    let frequencies = count_words(words);
    
    let mut result: Vec<_> = frequencies.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1));  // Sort by frequency
    result
}

// Use it
let text = "The quick brown fox jumps over the lazy dog. The dog was lazy!";
let top_words = analyze_text(text);
for (word, count) in top_words.iter().take(3) {
    println!("{}: {} times", word, count);
}
```

## The Magic: No Hidden State

Unlike other languages, Palladium functions can't have hidden state:

```c
// C - Dangerous hidden state!
int counter() {
    static int count = 0;  // Hidden state!
    return ++count;
}
```

In Palladium, if a function needs state, it must be explicit:

```palladium
struct Counter {
    count: i32,
}

impl Counter {
    fn new() -> Counter {
        Counter { count: 0 }
    }
    
    fn increment(&mut self) -> i32 {
        self.count += 1;
        self.count
    }
}
```

No surprises. No hidden behavior. What you see is what you get.

## Key Takeaways

1. **Functions are machines** with clear inputs and outputs
2. **Ownership is explicit** - consume, borrow, or mutably borrow
3. **Errors are values** - handle them explicitly with Result
4. **Generic functions** work with any type (shape-agnostic)
5. **No hidden state** - everything is visible and predictable

[Next: Chapter 5 - Ownership is Responsibility →](chapter_5_ownership.md)

---

*Exercise: Build a function machine that:*
- *Takes a vector of numbers*
- *Returns the average (if the vector isn't empty)*
- *Doesn't consume the vector*

*Hint: What should the return type be?*