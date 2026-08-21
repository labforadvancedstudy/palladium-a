> **NORMATIVE — this is what Palladium is defined to be.** It is not a description of what
> `pdc` implements today. What is implemented, partial, or unimplemented is recorded per
> specification section in the
> [implementation status annex](../../../specification/language-spec.md#part-ii-implementation-status-annex).
> Palladium blocks below are fenced `no-compile`: the syntax is normative, the compiler does not
> accept it yet, and `scripts/check-docs.sh` counts each fence rather than hiding it.

# Feature: Async as Effect

Normative specification section: [`language-spec.md` §N7 Effects and asynchrony](../../../specification/language-spec.md#n7-effects-and-asynchrony).

## Overview

Palladium treats async as an algebraic effect rather than a special function color, eliminating the need for `.await` and preventing the "function coloring" problem that plagues Rust and JavaScript.

Three consequences follow from that single decision, and they are the definition, not a wish list:

1. There is **no `async` keyword and no `.await` operator** in the language. A function that performs an asynchronous operation is written exactly like one that does not.
2. Effects are **inferred** from a function's body and **propagated to its callers**, transitively, without anything being written down.
3. Independent effectful operations are **parallel by default**. Sequencing is what you ask for, not what you get by accident.

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

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
// No async keyword needed - effect is inferred
fn fetch_user(id: u64) -> Result<User> {
    // No .await - compiler handles effects
    let response = client.get(format!("/users/{}", id)).send()?;
    let user = response.json::<User>()?;
    Ok(user)
}

// Can call from anywhere - no coloring
fn get_user_sync(id: u64) -> Result<User> {
    fetch_user(id)  // no wrapper needed: no colour to cross
}

// Automatic parallelization with effects
fn process_users(ids: Vec<u64>) -> Result<Vec<User>> {
    // Compiler sees independent operations
    ids.map(fetch_user).collect()  // Parallel by default!
}

// Explicit sequencing when needed
fn process_users_sequential(ids: Vec<u64>) -> Result<Vec<User>> {
    let users = vec![];
    effect::sync {
        for id in ids {
            users.push(fetch_user(id)?);  // ordered: effect::sync forbids reordering
        }
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
```

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
// Palladium - natural composition
fn compose() {
    baz(bar(foo()?)?)?  // Effects propagate automatically
}
```

### 3. Optimization Opportunity
Because the compiler knows which operations are effectful and which are independent, it can schedule them concurrently without the programmer writing a scheduler, and it does so without tracking anything at runtime.

## How It Works

The first two blocks in this section are compiler-internal pseudocode rather than Palladium
programs — they sketch the algorithm, and are fenced `no-compile` for the same reason as
everything else here.

### Effect System Design

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
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

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
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

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
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

## Where the implementation currently diverges

Everything above is the definition. What follows is what `pdc` does at commit `abeb665`,
measured, with a source location for each row. These are divergences from the definition, not
qualifications of it — the language is not redefined by the state of the compiler.

Line numbers below were re-derived against the working tree at `abeb665`. The corresponding
citations in `language-spec.md` before this change were taken from the pre-cleanup revision
`f323cf1` and no longer pointed at the code they named; they are corrected in the annex.

**1. The surface syntax took the Rust-shaped path this design rejects.**
`async` and `await` are keywords in the lexer (`src/lexer/token.rs:111-115`), the grammar's
`function` production carries an optional `async` (`docs/specification/grammar.ebnf:91`), `.await`
is a postfix operator (`docs/specification/grammar.ebnf:216`), and the keyword list names both
(`docs/specification/grammar.ebnf:56`). The parser sets `Function.is_async` from that keyword
(`src/parser/mod.rs:354`, `src/parser/mod.rs:365`). The implementation therefore offers exactly the two things this
document says the language does not have: an `async` marker and an await operator.

**2. Effects are inferred, but the result is print-only — it gates nothing.**
The parser hardcodes `Function.effects` to `None`, commented "Effects will be inferred during
analysis" (`src/parser/mod.rs:565`). An effect analyser exists (`src/effects/mod.rs`, 409 lines;
`Effect` enum at `src/effects/mod.rs:16-29`, `analyze_function` at `src/effects/mod.rs:151`) and it does union effects across
statements and calls (`src/effects/mod.rs:263`). But `crate::effects::` is referenced from exactly one place in
the compiler — `src/driver/mod.rs:147` — and all the driver does with the result is `println!` it
(`src/driver/mod.rs:151`). No later phase reads it. It cannot reject a program, cannot change
codegen, and cannot schedule anything.

**3. Propagation to callers is order-dependent, and unknown callees are assumed pure.**
`src/effects/mod.rs:280-284` looks a callee's effects up in a map populated only as functions are
analysed, in source order, with the fallback comment "If function is unknown, we conservatively
assume it's pure". Assuming purity is the unsound direction: a function defined below its caller
contributes no effects to that caller. The definition requires propagation to be a fixed point
over the call graph, not a single forward pass.

**4. Effect analysis never sees methods.** The driver's loop matches only
`crate::ast::Item::Function` (`src/driver/mod.rs:148-149`), so functions inside `impl` blocks are
not analysed at all.

**5. Automatic parallelization does not exist.** `grep -rn 'parallel' src/effects/mod.rs
src/codegen/mod.rs` returns nothing. There is no `ParallelMap`, no independence analysis, and no
scheduler.

**6. Effect contexts do not exist.** There is no `with`, no `effect` item, no `effect::sync`, and
no `-> async T` return form. `with`, `effect` and `ref` are not keywords at all
(`docs/specification/grammar.ebnf:58-59`).

**7. `.await` generates C that references a member no part of the compiler emits.**
Codegen for an await expression emits `while (!<tmp>.poll(&<tmp>)) { }`
(`src/codegen/mod.rs:2604-2611`) and then reads `<tmp>.result` (`src/codegen/mod.rs:2613-2615`). Nothing generates a
`poll` member on the produced C type. This is not an error at any earlier stage — it is silent
breakage discovered by the C compiler, and it is the failure mode `language-spec.md` §6.5 already
recorded. The parallel defect for `?` is at `src/codegen/mod.rs:2548-2569`, which emits a
`struct Result { int is_ok; union { ... } data; }` layout that codegen never defines.

Direction of travel: making `.await` a hard compile error is *consistent* with this document,
because `.await` is not part of the language. The end state is that neither `async` nor `await` is
accepted at all, and that effect inference acquires consumers instead of a `println!`.

## Design intent, not measurements

The earlier version of this document carried a "Performance Characteristics" section with
figures — "+10-15% for effect inference", "+5% for parallelization analysis", "10-30% speedup from
automatic parallelization". No benchmark producing any of those numbers exists in this repository,
and none of the machinery they describe is built, so they cannot have been measured. They are
deleted rather than restated.

What survives is qualitative design intent — claims about the shape of the implementation, which
become checkable once it exists:

- Effect tracking is entirely static; nothing about it is represented at runtime.
- There is no async runtime and no `Future` boxing, so effectful code does not allocate on account
  of being effectful.
- Independent effectful operations may be scheduled concurrently, so straight-line source need not
  execute strictly sequentially.

None of these are performance results. They become results when a benchmark times them.

## Translating Rust async into Palladium

These are syntax correspondences, not a usage guide: the right-hand side is target syntax that
`pdc` does not accept today.

### Rust source
```rust
// Rust source
async fn complex_operation() -> Result<Data> {
    let auth = authenticate().await?;
    let token = auth.get_token().await?;
    let data = fetch_data(token).await?;
    let processed = process(data).await?;
    Ok(processed)
}
```

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
// Palladium equivalent (target syntax)
fn complex_operation() -> Result<Data> {
    let auth = authenticate()?;
    let token = auth.get_token()?;
    let data = fetch_data(token)?;
    process(data)
}
```

### Effect annotations, where the design requires them

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
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

These are the only two escape hatches the definition admits: `effect::sync { }` to force
sequencing, and an explicit `-> async T` return type to pin an asynchronous boundary. They exist so
that the absence of `.await` never becomes an absence of control.

## Common Patterns

### Parallel Map

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
// Automatically parallel
let results = items.map(|item| expensive_operation(item));
```

### Timeout with Fallback

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
fn fetch_with_fallback(id: u64) -> User {
    with_timeout(1.second) {
        fetch_user(id)
    }.unwrap_or_else(|| User::default())
}
```

### Retry Logic

<sub>Normative syntax. `pdc` does not accept this today.</sub>
```palladium no-compile
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

## Related

- [Palladium v1.0 feature definition](../PALLADIUM_V1_FEATURES.md) — where this sits among the rest
- [Totality checking](../advanced/totality-checking.md)
- [Implicit lifetimes](../core-language/implicit-lifetimes.md)
- [Feature index](../feature-index.toml)
- [Language specification](../../../specification/language-spec.md)
