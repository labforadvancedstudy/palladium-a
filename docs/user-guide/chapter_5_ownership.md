# Chapter 5: Ownership is Responsibility

*Or: Why Sharing is Actually Caring (When Done Right)*

Imagine you have a really nice bicycle. Now, what happens when your friend wants to borrow it?

1. **Give it away forever** - They keep it, you lose it
2. **Let them look at it** - They can see it but not ride it
3. **Let them borrow it** - They use it and return it
4. **Never share** - Nobody gets to use it

Most programming languages force you into option 4 (never share) or create chaos (everyone thinks they own it). Palladium gives you all four options, but makes sure everyone knows the rules.

## The Golden Rule: One Owner

Every piece of data in Palladium has exactly one owner. When the owner goes away, so does the data. It's like this:

```palladium
fn main() {
    let x = 42;  // x owns the value 42
}  // x goes away, so does 42
```

Simple, right? But what about more complex data?

## Stack vs Heap: The Two Neighborhoods

Remember our warehouse analogy? Actually, there are two neighborhoods:

1. **Stack Street** - Small, tidy houses (numbers, booleans, small structs)
2. **Heap Heights** - Big mansions (arrays, large structs, dynamic data)

Stack Street houses get copied easily:

```palladium
fn main() {
    let x = 42;      // Lives on Stack Street
    let y = x;       // Gets a copy of the house
    print_int(x);    // Still have original - prints 42
    print_int(y);    // Have the copy too - prints 42
}
```

But Heap Heights mansions are different - they're too big to copy casually!

## The Move: Changing Addresses

When you have data in Heap Heights, assignment means moving:

```palladium
// If we had Vec type (coming in v0.9!):
// let v1 = vec![1, 2, 3];  // v1 owns a mansion in Heap Heights
// let v2 = v1;             // v1 MOVES to v2 (change of address)
// print(v1);               // ERROR! v1 doesn't live there anymore
```

It's like selling your house. Once you sell it, you can't live there anymore!

## Borrowing: Just Visiting

But what if you just want to show someone your data without giving it away? That's borrowing:

```palladium
fn main() {
    let x = 42;
    let y = &x;      // y borrows x
    print_int(x);    // x still owns 42
    // y can look but not change
}
```

Think of it like giving someone your address. They can visit, but they can't redecorate!

## Mutable Borrowing: Renovation Permission

Sometimes you want someone to help fix up your place:

```palladium
fn add_one(x: &mut i64) {
    *x = *x + 1;  // Dereferencing to modify
}

fn main() {
    let mut num = 41;
    add_one(&mut num);
    print_int(num);  // Prints 42
}
```

This is like giving your contractor exclusive access to renovate. Nobody else can visit while work is happening!

## The Rules That Keep Us Safe

Palladium enforces these rules at compile time:

1. **Each value has one owner**
2. **When the owner goes out of scope, the value is dropped**
3. **You can have EITHER:**
   - One mutable reference, OR
   - Any number of immutable references
   - But never both at the same time!

Let's see why this matters:

```palladium
fn main() {
    let mut x = 42;
    
    let r1 = &x;     // Immutable borrow
    let r2 = &x;     // Another immutable borrow - OK!
    print_int(*r1);  // Can read through r1
    print_int(*r2);  // Can read through r2
    
    // let r3 = &mut x;  // ERROR! Can't mutably borrow while immutably borrowed
}
```

## Real-World Example: Safe Counter

Here's how ownership prevents bugs:

```palladium
// A function that takes ownership
fn consume_value(x: i64) {
    print_int(x);
    // x is dropped here
}

// A function that borrows
fn peek_value(x: &i64) {
    print_int(*x);
    // x is NOT dropped - we just borrowed it
}

// A function that mutably borrows
fn increment_value(x: &mut i64) {
    *x = *x + 1;
}

fn main() {
    let mut value = 10;
    
    // Borrowing is fine
    peek_value(&value);
    print("After peek: ");
    print_int(value);  // Still 10
    
    // Mutable borrowing changes it
    increment_value(&mut value);
    print("After increment: ");
    print_int(value);  // Now 11
    
    // This would take ownership
    consume_value(value);
    // print_int(value);  // ERROR! value was moved
}
```

## The Lifetime Story

Every reference has a lifetime - how long it's valid for. Usually, Palladium figures this out for you:

```palladium
fn main() {
    let x = 42;          // x is born
    let r = &x;          // r borrows x
    print_int(*r);       // Using r while x is alive - OK!
}  // Both die together - safe!
```

But this wouldn't work:

```palladium
// fn dangling() -> &i64 {
//     let x = 42;
//     &x  // ERROR! Returning reference to local variable
// }  // x dies here, but we're returning a reference to it!
```

It's like giving someone your address, then demolishing your house before they visit!

## Why This Matters

The ownership system prevents:

1. **Use-after-free** - Can't use data after it's gone
2. **Double-free** - Can't free the same memory twice
3. **Data races** - Can't modify data while others read it
4. **Memory leaks** - Automatic cleanup when owner goes away

All checked at compile time. Zero runtime cost!

## Practical Patterns

### Pattern 1: Giving Ownership (Move)
Use when the function needs to own the data:
```palladium
fn process_data(data: i64) {
    // Function owns data now
    print_int(data * 2);
}
```

### Pattern 2: Lending for Reading (Immutable Borrow)
Use when function just needs to look:
```palladium
fn calculate_double(x: &i64) -> i64 {
    *x * 2  // Just reading, not taking
}
```

### Pattern 3: Lending for Modification (Mutable Borrow)
Use when function needs to change:
```palladium
fn double_in_place(x: &mut i64) {
    *x = *x * 2;  // Modifying the original
}
```

## A Mental Model

Think of ownership like a library book:

- **Owning** = You bought the book
- **Moving** = You gave the book to someone else
- **Borrowing** = You're reading it in the library
- **Mutable borrowing** = You have exclusive access to make notes

Only one person can write notes at a time, but many can read!

## Common Mistakes and Solutions

### Mistake 1: Using After Move
```palladium
// let x = SomeType::new();
// let y = x;
// use_value(x);  // ERROR: x was moved to y
```

**Solution**: Clone if you need two copies, or borrow if you just need to look.

### Mistake 2: Multiple Mutable Borrows
```palladium
// let mut x = 42;
// let r1 = &mut x;
// let r2 = &mut x;  // ERROR: Already mutably borrowed
```

**Solution**: Finish with one mutable borrow before starting another.

## The Payoff

By following these simple rules, Palladium gives you:

- **Memory safety** without garbage collection
- **Thread safety** without locks everywhere
- **Predictable performance** - you control when things happen
- **No hidden costs** - ownership is zero-overhead

And the compiler ensures you follow the rules. It's like having a very helpful friend who stops you before you make mistakes!

## Try It Yourself

Here's a working example you can run:

```palladium
fn main() {
    // Stack values (Copy)
    let a = 10;
    let b = a;      // a is copied to b
    print_int(a);   // Still valid - prints 10
    print_int(b);   // Also valid - prints 10
    
    // References
    let x = 100;
    let y = &x;     // y borrows x
    print_int(x);   // Can still use x - prints 100
    
    // Mutable references
    let mut counter = 0;
    let r = &mut counter;
    *r = *r + 1;    // Increment through reference
    print_int(counter);  // Prints 1
}
```

[Next: Chapter 6 - Traits are Promises →](chapter_6_traits.md)

---

*Exercise: Fix this broken code:*
```palladium
fn main() {
    let mut x = 50;
    let r1 = &x;
    let r2 = &mut x;  // Oops!
    print_int(*r1);
}
```
*Hint: When can you have mutable and immutable references?*