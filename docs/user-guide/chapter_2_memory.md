# Chapter 2: Memory is Just Boxes

*Or: Why Computers Are Just Very Organized Warehouses*

Let me tell you a secret: Memory isn't complicated. Programmers just make it seem that way.

Imagine a giant warehouse with millions of numbered boxes. That's your computer's memory. Each box can hold one thing. That's it. That's the big secret.

## The Box System

Here's how simple it is:

```palladium
let x = 42;
```

This means: "Put the number 42 in a box and label it 'x'."

```
Box #1000: [42] <- labeled "x"
```

Now let's get fancier:

```palladium
let name = "Alice";
```

This puts each letter in consecutive boxes:

```
Box #2000: ['A'] 
Box #2001: ['l']
Box #2002: ['i'] 
Box #2003: ['c']
Box #2004: ['e']
Box #2005: ['\0'] <- marks the end
```

And we label box #2000 as "name".

See? Not so scary.

## The Problem with Sharing

Here's where things get interesting. What happens when two parts of your program want the same box?

```palladium
let message = String::from("Hello");
let message2 = message;  // Who owns the boxes now?
```

In C, both variables would point to the same boxes. It's like two people having keys to the same house. What happens when one person renovates while the other is sleeping? Chaos!

In Java/Python, there's a garbage collector—like a landlord who decides when to demolish unused houses. But you never know when the landlord will show up!

## The Palladium Way: Ownership

Palladium has a simple rule: **Every box has exactly one owner.**

```palladium
let message = String::from("Hello");  // message owns the boxes
let message2 = message;               // message2 now owns them
// println!("{}", message);           // Error! message gave up ownership
```

It's like selling your car. Once you sell it, you can't drive it anymore. Simple!

"But wait," you say, "what if I want to look at the data without taking ownership?"

Great question! That's what borrowing is for.

## Borrowing: Just Looking

Sometimes you don't want to own something, just look at it:

```palladium
let message = String::from("Hello");
let len = calculate_length(&message);  // & means "borrow"
println!("{}", message);  // Still works! We just borrowed it
```

This is like letting someone look at your photo album. They can see it, but they can't take it home.

Here's the beautiful part—Palladium ensures they can't sneakily modify your album either:

```palladium
fn sneaky_function(s: &String) {
    s.push_str(" World");  // Error! Can't modify borrowed data
}
```

Unless you explicitly let them:

```palladium
fn allowed_function(s: &mut String) {  // "mut" = mutable = changeable
    s.push_str(" World");  // This is fine
}
```

## The Stack and The Heap: A Tale of Two Warehouses

Actually, your computer has TWO warehouses:

1. **The Stack**: A neat stack of boxes, like a cafeteria tray dispenser
2. **The Heap**: A massive warehouse where you can request any amount of space

### The Stack (Fast but Rigid)

When you write:
```palladium
let x = 42;
let y = 7;
```

The stack looks like:
```
|  7  | <- y
|  42 | <- x
+-----+
```

When the function ends, we just remove the boxes from the top. Super fast!

### The Heap (Flexible but Slower)

When you write:
```palladium
let message = String::from("Hello, World!");
```

It's like calling the warehouse and saying "I need 13 boxes please!" They find space somewhere and tell you the address:

```
Stack:           Heap:
| #5000 |  --->  Box #5000: ['H']
+-------+        Box #5001: ['e']
                 Box #5002: ['l']
                 ... and so on
```

## Why This Matters

You might think, "Why do I care about boxes?" Here's why:

### Speed
Stack allocation is like grabbing a tray from a dispenser: instant.
Heap allocation is like calling the warehouse manager: takes time.

```palladium
// Fast (stack)
let point = Point { x: 10, y: 20 };

// Slower (heap)
let data = vec![1, 2, 3, 4, 5];
```

### Predictability
With ownership, you know exactly when memory is freed:

```palladium
{
    let data = vec![1, 2, 3];
    // ... use data ...
}  // <- data's boxes are freed RIGHT HERE
```

No waiting for a garbage collector. No memory leaks. No surprises.

## Common Patterns

Let's see how this plays out in real code:

### Pattern 1: Transferring Ownership
```palladium
fn take_ownership(s: String) {
    println!("{}", s);
}  // s is dropped here, memory freed

let message = String::from("Hello");
take_ownership(message);
// message is no longer valid here
```

### Pattern 2: Borrowing for Reading
```palladium
fn read_data(s: &String) -> usize {
    s.len()  // Just reading, not taking
}

let message = String::from("Hello");
let length = read_data(&message);
println!("'{}' has {} characters", message, length);  // message still valid!
```

### Pattern 3: Borrowing for Modifying
```palladium
fn add_exclamation(s: &mut String) {
    s.push('!');
}

let mut message = String::from("Hello");
add_exclamation(&mut message);
println!("{}", message);  // Prints: Hello!
```

## The Magic Rule

Here's the rule that makes Palladium memory-safe:

**You can have either:**
- One mutable reference, OR
- Any number of immutable references

**But not both at the same time!**

It's like a museum exhibit:
- Everyone can look (immutable references)
- OR one person can be restoring it (mutable reference)
- But you can't restore while people are looking!

```palladium
let mut data = vec![1, 2, 3];

let r1 = &data;      // OK: immutable borrow
let r2 = &data;      // OK: another immutable borrow
println!("{:?} {:?}", r1, r2);

let r3 = &mut data;  // OK: mutable borrow (r1, r2 no longer used)
r3.push(4);
```

## Real-World Example

Let's build something real: a function that finds the longest word:

```palladium
fn find_longest(text: &str) -> &str {
    text.split_whitespace()
        .max_by_key(|word| word.len())
        .unwrap_or("")
}

fn main() {
    let essay = String::from("The quick brown fox jumps");
    let longest = find_longest(&essay);
    println!("Longest word: {}", longest);  // "quick"
}
```

Notice:
- We borrowed `essay` with `&essay`
- The function returns a borrowed slice `&str`
- No memory was allocated or freed
- It's as fast as C but completely safe

## The Payoff

By thinking about memory as just boxes with ownership rules, Palladium gives you:

1. **No use-after-free bugs** - Can't use boxes after the owner is gone
2. **No double-frees** - Each box has one owner who frees it
3. **No data races** - Can't modify while others are reading
4. **No memory leaks** - Owners always clean up their boxes

And the compiler checks all of this *before your program runs*.

## Try It Yourself

Here's a broken program. Can you fix it?

```palladium
fn main() {
    let s1 = String::from("Hello");
    let s2 = s1;
    
    println!("{}, world!", s1);  // Error: s1 was moved
}
```

Hint: You need to either clone or borrow!

[Next: Chapter 3 - Types are Shapes →](chapter_3_types.md)

---

*Exercise: Draw the boxes for this code:*
```palladium
let x = 5;
let y = &x;
let z = Box::new(42);
```
*Where is each value stored? Stack or heap?*