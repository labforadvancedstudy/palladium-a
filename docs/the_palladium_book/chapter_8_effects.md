# Chapter 8: Effects are Side Stories

*Or: Being Honest About What Functions Really Do*

Imagine you hire someone to paint your room. They say "I'll paint your room." Simple, right? But what if they also:
- Play loud music (noise effect)
- Open all your windows (environment effect)
- Order pizza on your credit card (financial effect)

You'd want to know about these "side effects" upfront! That's what Palladium's effect system does—it makes functions honest about what they really do.

## The Lie of "Pure" Functions

Most languages let functions lie:

```c
// C - Looks pure, but who knows?
int calculate(int x) {
    printf("Calculating...\n");  // Surprise! I/O effect
    global_state++;              // Surprise! State mutation
    return x * 2;
}
```

In Palladium, functions must declare their effects:

```palladium
// Honest function - no surprises!
fn calculate(x: i32) -> i32 ![io, state] {
    print("Calculating...");  // IO effect declared
    STATE.increment();        // State effect declared
    x * 2
}
```

## What Are Effects?

Effects are things a function does besides returning a value:
- **IO** - Reading/writing files, network, console
- **State** - Modifying global state
- **Async** - Might need to wait
- **Unsafe** - Direct memory manipulation
- **Exception** - Might fail

## Pure Functions: The Gold Standard

A pure function has no effects—it just transforms input to output:

```palladium
fn add(a: i32, b: i32) -> i32 {
    a + b  // No effects, always same result
}

fn is_even(n: i32) -> bool {
    n % 2 == 0  // Pure computation
}
```

Pure functions are like math: 2 + 2 is always 4, no surprises!

## Effect Tracking in Action

Palladium tracks effects automatically:

```palladium
fn pure_calculation(x: i32) -> i32 {
    x * x  // No effects
}

fn with_output(x: i32) -> i32 {
    print_int(x);  // Compiler knows: IO effect!
    x
}

fn main() {
    let a = pure_calculation(5);    // Pure
    let b = with_output(5);         // Has IO effect
}
```

The compiler analyzes and reports:
```
Function 'pure_calculation' has effects: {}
Function 'with_output' has effects: {IO}
Function 'main' has effects: {IO}
```

## Effect Propagation

Effects bubble up like emotions in a family:

```palladium
fn read_config() -> String ![io] {
    read_file("config.txt")  // IO effect
}

fn process_data() -> i32 ![io] {
    let config = read_config();  // Inherits IO effect
    parse_config(config)
}

fn pure_parse(s: String) -> i32 {
    // Just parsing, no effects
    s.len() as i32
}
```

If you call an effectful function, you inherit its effects!

## Why Effects Matter

### 1. **Predictability**
```palladium
fn calculate_score(data: &[i32]) -> i32 {
    // No effects = same input always gives same output
    data.iter().sum()
}
```

### 2. **Testability**
```palladium
// Pure functions are trivial to test
#[test]
fn test_add() {
    assert_eq!(add(2, 2), 4);  // Always works
}

// Effectful functions need setup
#[test]
fn test_save_file() ![io] {
    create_test_directory();
    save_file("test.txt", "data");
    assert!(file_exists("test.txt"));
    cleanup_test_directory();
}
```

### 3. **Optimization**
```palladium
// Compiler can optimize pure functions aggressively
let x = expensive_pure_calculation(input);
let y = expensive_pure_calculation(input);  // Can reuse x!
```

### 4. **Parallelization**
```palladium
// Pure functions can run in parallel safely
let results = data.par_iter()
    .map(|x| pure_transform(x))  // Safe to parallelize!
    .collect();
```

## Real-World Example: Web Request Handler

```palladium
// Effects make dependencies clear!
fn handle_request(req: Request) -> Response ![io, db, net] {
    // Validate (pure)
    let validated = validate_request(req);  // No effects
    
    // Check auth (database)
    let user = fetch_user(validated.user_id);  // db effect
    
    // Get data (network)
    let data = fetch_external_api(validated.endpoint);  // net effect
    
    // Log (I/O)
    log_request(&user, &validated);  // io effect
    
    // Transform (pure)
    create_response(data)  // No effects
}
```

Just by looking at the signature, you know this function:
- Does I/O operations
- Accesses a database
- Makes network calls

No surprises!

## Effect Polymorphism

Functions can be generic over effects:

```palladium
// Works with any effect E
fn map_with_effect<T, U, E>(
    items: Vec<T>, 
    f: fn(T) -> U ![E]
) -> Vec<U> ![E] {
    items.into_iter().map(f).collect()
}

// Can use with pure functions
let doubled = map_with_effect(vec![1, 2, 3], |x| x * 2);

// Or effectful ones
let printed = map_with_effect(vec![1, 2, 3], |x| {
    print_int(x);  // IO effect
    x
});
```

## Managing Effects

### Effect Boundaries
Keep effects at the edges of your program:

```palladium
// Core logic - pure
fn calculate_invoice(items: Vec<Item>) -> Invoice {
    // Pure business logic
}

// Edge - effectful
fn process_order(order_id: i32) ![io, db] {
    let items = load_items_from_db(order_id);  // db effect
    let invoice = calculate_invoice(items);     // pure!
    save_invoice_to_file(invoice);              // io effect
}
```

### Effect Isolation
Minimize effect scope:

```palladium
// Bad - entire function is effectful
fn process_data_bad(data: Vec<i32>) ![io] {
    print("Starting...");
    let result = data.iter().sum();  // Pure computation with IO effect!
    print("Done!");
    result
}

// Good - isolate effects
fn process_data_good(data: Vec<i32>) -> i32 {
    let result = calculate_sum(data);  // Pure
    log_result(result);                // IO isolated
    result
}

fn calculate_sum(data: Vec<i32>) -> i32 {
    data.iter().sum()  // Pure
}

fn log_result(result: i32) ![io] {
    print_int(result);  // IO effect contained
}
```

## Common Effect Patterns

### The IO Monad (Without Saying Monad)
```palladium
// Chain IO operations
fn copy_file(src: &str, dst: &str) ![io] {
    read_file(src)
        .and_then(|contents| write_file(dst, contents))
        .map_err(|e| log_error(e))
}
```

### Effect Handlers (Future)
```palladium
// Handle effects explicitly
handle_effects! {
    // Pure code that might have effects
    let result = computation();
    
    // Handle each effect
    on IO(op) => mock_io(op),
    on Network(req) => test_network(req),
}
```

## What's Available Now (v0.8-alpha)

Currently, Palladium tracks these effects automatically:
- **IO** - Print, file operations
- **Async** - Async functions
- **Unsafe** - Unsafe blocks

The compiler analyzes and reports effects for every function!

## Mental Model

Think of effects like ingredients labels:
- **Pure function** = Just the main ingredient
- **Effectful function** = "May contain: IO, State, Network"

You want to know what you're consuming!

## Best Practices

1. **Prefer pure functions** - Easier to reason about
2. **Isolate effects** - Keep them at the boundaries
3. **Be explicit** - Don't hide effects
4. **Test pure code extensively** - It's easy!
5. **Mock effects for testing** - Control the environment

## The Future of Effects

Coming in v0.9 and beyond:
- Custom effect definitions
- Effect handlers
- Effect polymorphism
- Algebraic effects

## Simple Example

```palladium
// Pure helper
fn format_message(name: &str, score: i32) -> String {
    // No effects - pure string manipulation
    format!("{} scored {} points!", name, score)
}

// Effectful main
fn main() {
    let message = format_message("Alice", 100);  // Pure
    print(message);  // IO effect only here
}
```

## Why This Matters

Effects make your code:
- **Predictable** - No hidden surprises
- **Testable** - Pure functions test easily
- **Optimizable** - Compiler knows what's safe
- **Maintainable** - Dependencies are explicit

[Next: Chapter 9 - Proofs are Certainty →](chapter_9_proofs.md)

---

*Exercise: Label these functions with their effects:*

```palladium
fn a(x: i32) -> i32 {
    x + random()  // What effect?
}

fn b(name: String) {
    save_to_database(name)  // What effect?
}

fn c(x: i32, y: i32) -> i32 {
    if x > y { x } else { y }  // What effect?
}

fn d() -> String {
    read_file("config.txt")  // What effect?
}
```

*Answers: a: Random/IO, b: DB/IO, c: None (pure!), d: IO*