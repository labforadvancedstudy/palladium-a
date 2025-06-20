# Chapter 9: Proofs are Certainty

*Or: How to Be 100% Sure Instead of 99.9% Sure*

Imagine building a bridge. You can:
1. **Test it** - Drive trucks over it and hope it holds
2. **Prove it** - Use math to show it will ALWAYS hold

Testing finds bugs. Proofs show there are no bugs to find. That's the difference between "it works in my tests" and "it works, period."

## The Problem with Testing

Tests are like checking a few lottery tickets:

```palladium
#[test]
fn test_divide() {
    assert_eq!(divide(10, 2), Some(5));
    assert_eq!(divide(20, 4), Some(5));
    assert_eq!(divide(5, 0), None);
}
```

Great! But what about divide(i32::MAX, -1)? What about all the cases you didn't think of?

## Proofs: Complete Certainty

Proofs check EVERY possible lottery ticket:

```palladium
// Coming in v1.0+
#[prove]
fn divide_safe(a: i32, b: i32) -> Option<i32> {
    if b == 0 {
        None
    } else if a == i32::MIN && b == -1 {
        None  // Overflow case!
    } else {
        Some(a / b)
    }
}

// Proof: ∀ a,b ∈ i32: divide_safe(a,b) never panics
```

The compiler mathematically proves this function NEVER crashes. Not "hasn't crashed yet"—NEVER.

## What Can We Prove?

### 1. Memory Safety (Already Done!)
```palladium
fn use_after_move() {
    let s = String::from("hello");
    let t = s;  // s moved
    // print(s);  // Compiler proves this is impossible
}
```

Palladium already proves memory safety at compile time!

### 2. Termination (Chapter 3 of Totality)
```palladium
#[total]
fn factorial(n: u64) -> u64 {
    if n == 0 {
        1
    } else {
        n * factorial(n - 1)  // Proven to terminate
    }
}
```

The compiler proves this always finishes—no infinite loops!

### 3. Bounds Safety
```palladium
fn safe_index(arr: &[i32; 10], idx: usize) -> Option<i32> {
    if idx < 10 {
        Some(arr[idx])  // Compiler proves: no bounds error possible
    } else {
        None
    }
}
```

### 4. Overflow Safety
```palladium
#[no_overflow]
fn safe_add(a: u32, b: u32) -> Option<u32> {
    if a <= u32::MAX - b {
        Some(a + b)  // Proven: cannot overflow
    } else {
        None
    }
}
```

## How Proofs Work

Think of the compiler as a paranoid mathematician:

```palladium
fn abs_diff(a: i32, b: i32) -> u32 {
    if a > b {
        (a - b) as u32  // Prove: a > b → a - b ≥ 0
    } else {
        (b - a) as u32  // Prove: a ≤ b → b - a ≥ 0
    }
}
```

For each path, the compiler proves the operation is safe!

## Refinement Types: Types with Conditions

Coming in future versions:

```palladium
// A number that's definitely positive
type Positive = { x: i32 | x > 0 };

// A vector that's not empty  
type NonEmptyVec<T> = { v: Vec<T> | v.len() > 0 };

fn divide_positive(a: i32, b: Positive) -> i32 {
    a / b  // No check needed - b cannot be 0!
}

fn get_first<T>(v: NonEmptyVec<T>) -> T {
    v[0]  // No bounds check - v cannot be empty!
}
```

The type system proves safety!

## Real-World Example: Bank Transfer

```palladium
// Prove money is never created or destroyed
#[invariant(total_money_constant)]
fn transfer(from: &mut Account, to: &mut Account, amount: u64) -> Result<()> {
    // Precondition: amount ≤ from.balance
    if amount > from.balance {
        return Err("Insufficient funds");
    }
    
    // Prove: no overflow in addition
    if to.balance > u64::MAX - amount {
        return Err("Would overflow");
    }
    
    // These operations are proven safe
    from.balance -= amount;
    to.balance += amount;
    
    // Postcondition proven: 
    // from.balance + to.balance = original_sum
    Ok(())
}
```

The compiler proves money is conserved in ALL cases!

## Proof by Construction

Sometimes the best proof is making invalid states impossible:

```palladium
// Instead of proving an index is valid...
fn maybe_unsafe(v: &Vec<i32>, idx: usize) -> i32 {
    if idx < v.len() {
        v[idx]  // Need to prove idx is valid
    } else {
        panic!("Bad index");
    }
}

// ...make invalid indices impossible!
enum SafeIndex<'a, T> {
    Valid { vec: &'a Vec<T>, idx: usize },
}

impl<'a, T> SafeIndex<'a, T> {
    fn new(vec: &'a Vec<T>, idx: usize) -> Option<Self> {
        if idx < vec.len() {
            Some(SafeIndex::Valid { vec, idx })
        } else {
            None
        }
    }
    
    fn get(&self) -> &T {
        match self {
            SafeIndex::Valid { vec, idx } => &vec[*idx], // Always safe!
        }
    }
}
```

If you have a SafeIndex, it's PROVEN valid!

## What's Provable Now (v0.8)

Currently, Palladium automatically proves:
- **Memory safety** - No use-after-free, no double-free
- **Thread safety** - No data races
- **Type safety** - No type confusion
- **Pattern completeness** - All cases handled

## Coming Soon (v1.0+)

Future proof capabilities:
- **Termination checking** - Prove functions finish
- **Refinement types** - Types with predicates
- **Overflow checking** - Prove arithmetic safety
- **Invariant preservation** - Prove properties hold

## Mental Model

Think of proofs like building codes:
- **Testing** = "This building survived an earthquake"
- **Proofs** = "Physics guarantees this building survives ANY earthquake up to 9.0"

Which would you trust more?

## Common Patterns

### Prove by Exhaustion
```palladium
enum State { On, Off }

fn toggle(s: State) -> State {
    match s {
        State::On => State::Off,
        State::Off => State::On,
    }  // Compiler proves: all cases handled
}
```

### Prove by Construction
```palladium
// Make invalid data unrepresentable
struct Email(String);

impl Email {
    fn new(s: String) -> Option<Email> {
        if is_valid_email(&s) {
            Some(Email(s))  // If it exists, it's valid
        } else {
            None
        }
    }
}
```

### Prove by Contradiction
```palladium
fn sqrt_exists(n: u64) -> bool {
    // Prove: either sqrt exists or it doesn't
    for i in 0..=n {
        if i * i == n {
            return true;  // Found it
        }
        if i * i > n {
            return false;  // Passed it, impossible
        }
    }
    false  // Proven: checked all possibilities
}
```

## Why Proofs Matter

1. **Critical Systems** - Aerospace, medical devices, finance
2. **Security** - Prove no buffer overflows, no injections
3. **Optimization** - Compiler can optimize based on proofs
4. **Confidence** - Sleep well knowing your code is correct

## The Cost of Proofs

Proofs aren't free:
- **Compile time** - More analysis = slower compilation
- **Complexity** - Some things are hard to prove
- **Restrictions** - Not all code can be proven

But for critical code, it's worth it!

## Practical Example

```palladium
// Today: Runtime checks
fn current_approach(password: &str) -> bool {
    if password.len() < 8 {
        return false;  // Runtime check
    }
    if !has_uppercase(password) {
        return false;  // Runtime check
    }
    if !has_number(password) {
        return false;  // Runtime check
    }
    true
}

// Future: Compile-time proofs
type StrongPassword = {
    s: String |
    s.len() >= 8 &&
    s.chars().any(|c| c.is_uppercase()) &&
    s.chars().any(|c| c.is_numeric())
};

fn future_approach(password: StrongPassword) -> bool {
    true  // No checks needed - type proves it's valid!
}
```

## Start Small

You don't need to prove everything:

```palladium
// Prove critical properties
#[prove(no_overflow)]
fn calculate_payment(principal: u64, rate: u32) -> u64 {
    // Prove this calculation never overflows
}

// Test non-critical code
#[test]
fn test_format_report() {
    // Regular testing is fine for UI code
}
```

## The Future

Imagine debugging sessions replaced by proof failures:

```
error[P001]: Cannot prove termination
 --> src/main.pd:5:1
  |
5 | fn risky_recursion(n: i32) -> i32 {
  | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: Function may not terminate when n < 0
  = help: Add base case: if n <= 0 { return 0; }
```

The compiler tells you EXACTLY what to fix!

[Next: Chapter 10 - Building Real Things →](chapter_10_applications.md)

---

*Exercise: Which of these can be proven at compile time?*

1. This function returns a prime number
2. This list is sorted
3. This pointer is not null
4. This loop terminates
5. This number is positive
6. This string is valid UTF-8

*Think about what the compiler can know statically vs dynamically!*