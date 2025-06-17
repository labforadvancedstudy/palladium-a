# Feature: Async as Effect

## Status: ⏳ 40% Complete

## Overview

Palladium treats async as an algebraic effect rather than a special function color, eliminating the need for `.await` and preventing the "function coloring" problem that plagues Rust and JavaScript.

## Code Comparison

### Rust (Explicit Async/Await)
```rust
// Every async function must be marked
async fn fetch_user(id: u64) -> Result<User, Error> {
    let response = client.get(&format!("/users/{}", id))
        .send()
        .await?;  // Explicit await required
    
    let user = response.json::<User>().await?;  // Another await
    Ok(user)
}

// Calling async from sync requires runtime
fn get_user_sync(id: u64) -> Result<User, Error> {
    // Need to block on runtime
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(fetch_user(id))
}

// Async contamination - everything becomes async
async fn process_users(ids: Vec<u64>) -> Result<Vec<User>, Error> {
    let mut users = Vec::new();
    for id in ids {
        users.push(fetch_user(id).await?);  // Sequential, slow
    }
    Ok(users)
}

// Parallel requires complex syntax
async fn process_users_parallel(ids: Vec<u64>) -> Result<Vec<User>, Error> {
    let futures: Vec<_> = ids.into_iter()
        .map(|id| fetch_user(id))
        .collect();
    
    futures::future::try_join_all(futures).await
}
```

### Go (Goroutines with Channels)
```go
// Go uses goroutines - different model
func fetchUser(id uint64) (*User, error) {
    resp, err := client.Get(fmt.Sprintf("/users/%d", id))
    if err != nil {
        return nil, err
    }
    
    var user User
    err = json.NewDecoder(resp.Body).Decode(&user)
    return &user, err
}

// Concurrent with goroutines and channels
func processUsers(ids []uint64) ([]*User, error) {
    ch := make(chan *User, len(ids))
    errCh := make(chan error, len(ids))
    
    for _, id := range ids {
        go func(id uint64) {
            user, err := fetchUser(id)
            if err != nil {
                errCh <- err
                return
            }
            ch <- user
        }(id)
    }
    
    // Collect results
    var users []*User
    for i := 0; i < len(ids); i++ {
        select {
        case user := <-ch:
            users = append(users, user)
        case err := <-errCh:
            return nil, err
        }
    }
    return users, nil
}
```

### Palladium (Async as Effect)
```palladium
// No async keyword needed - effect is inferred
fn fetch_user(id: u64) -> Result<User> {
    // No .await - compiler handles effects
    let response = client.get(format!("/users/{}", id)).send()?;
    let user = response.json::<User>()?;
    Ok(user)
}

// Can call from anywhere - no coloring
fn get_user_sync(id: u64) -> Result<User> {
    fetch_user(id)  // Just works
}

// Automatic parallelization with effects
fn process_users(ids: Vec<u64>) -> Result<Vec<User>> {
    // Compiler sees independent operations
    ids.map(fetch_user).collect()  // Parallel by default!
}

// Explicit sequencing when needed
fn process_users_sequential(ids: Vec<u64>) -> Result<Vec<User>> {
    let users = vec![];
    for id in ids {
        users.push(fetch_user(id)?);  // seq keyword forces order
    }
    Ok(users)
}

// Effect control
fn with_timeout(duration: Duration) -> effect {
    // Set timeout for all operations in scope
    effect::timeout(duration)
}

fn fetch_with_retry(id: u64) -> Result<User> {
    // Effects compose naturally
    with_timeout(5.seconds) {
        with_retry(3) {
            fetch_user(id)
        }
    }
}
```

## Why This Feature Exists

### 1. Function Coloring Problem
In Rust/JS, async functions can only be called by async functions, creating two incompatible function colors:
- Red functions (async) 
- Blue functions (sync)
- Can't call red from blue without runtime machinery

### 2. Composition Difficulties
```rust
// Rust - difficult to compose
async fn compose() {
    let x = foo().await?;
    let y = bar(x).await?;
    let z = baz(y).await?;  // Await spam
}

// Palladium - natural composition
fn compose() {
    baz(bar(foo()?)?)?  // Effects propagate automatically
}
```

### 3. Performance Optimization
- Compiler can automatically parallelize independent async operations
- No runtime overhead for effect tracking
- Better optimization opportunities

## How It Works

### Effect System Design
```palladium
// Internal representation
type Effect = Async | Pure | IO | Exception

// Function types carry effects
type FnType = (Args, Return, Effects)

// Effect inference
fn infer_effects(ast: AST) -> EffectMap {
    let mut effects = EffectMap::new();
    
    for function in ast.functions {
        if calls_async_operation(function) {
            effects.mark_async(function);
            propagate_effect_to_callers(function, Async);
        }
    }
    
    effects
}
```

### Automatic Parallelization
```palladium
// Compiler transform
fn parallelize(expr: Expr) -> Expr {
    match expr {
        Map(collection, func) if func.has_effect(Async) => {
            // Transform to parallel execution
            ParallelMap(collection, func)
        }
        Sequence(ops) if ops.are_independent() => {
            // Execute independent ops in parallel
            Parallel(ops)
        }
        _ => expr
    }
}
```

### Effect Contexts
```palladium
// Effects can be controlled in scope
effect async_scope {
    timeout: Duration,
    retry: usize,
    trace: bool,
}

fn configure_effects() {
    // All async ops in this scope get these settings
    with async_scope { timeout: 30.sec, retry: 3, trace: true } {
        let users = fetch_all_users();
        process_users(users);
    }
}
```

## Implementation Progress

- [x] Effect type system design
- [x] Basic effect inference
- [x] Effect propagation
- [ ] Automatic parallelization
- [ ] Effect contexts
- [ ] Runtime integration
- [ ] Cancellation support
- [ ] Structured concurrency

## Performance Characteristics

### Compile Time
- +10-15% for effect inference
- +5% for parallelization analysis

### Runtime
- Zero overhead for effect tracking
- 10-30% speedup from automatic parallelization
- Reduced allocations (no Future boxes)

### Memory
- Smaller binaries (no async runtime)
- Less heap allocation
- Better cache locality

## Migration Guide

### From Rust Async
```rust
// Before (Rust)
async fn complex_operation() -> Result<Data> {
    let auth = authenticate().await?;
    let token = auth.get_token().await?;
    let data = fetch_data(token).await?;
    let processed = process(data).await?;
    Ok(processed)
}

// After (Palladium)
fn complex_operation() -> Result<Data> {
    let auth = authenticate()?;
    let token = auth.get_token()?;
    let data = fetch_data(token)?;
    process(data)
}
```

### Effect Annotations (When Needed)
```palladium
// Force synchronous execution
fn must_be_sync() -> Data {
    effect::sync {
        fetch_data()  // Blocks if async
    }
}

// Explicit async boundary  
fn explicit_async() -> async Data {
    // Compiler ensures this is async
    fetch_data()
}
```

## Common Patterns

### Parallel Map
```palladium
// Automatically parallel
let results = items.map(|item| expensive_operation(item));
```

### Timeout with Fallback
```palladium
fn fetch_with_fallback(id: u64) -> User {
    with_timeout(1.second) {
        fetch_user(id)
    }.unwrap_or_else(|| User::default())
}
```

### Retry Logic
```palladium
fn reliable_fetch(id: u64) -> Result<User> {
    with_retry(3, exponential_backoff) {
        fetch_user(id)
    }
}
```

## Future Improvements

1. **Effect Polymorphism**: Generic over effects
2. **Custom Effects**: User-defined algebraic effects  
3. **Effect Handlers**: Intercept and modify effects
4. **Static Analysis**: Prove effect properties

## Related Features
- [No Await Syntax](./no_await_syntax.md)
- [Structured Concurrency](./structured_concurrency.md)
- [Error Handling](../core-language/error_handling.md)