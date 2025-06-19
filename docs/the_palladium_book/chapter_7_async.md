# Chapter 7: Async is Just Waiting

*Or: How to Do Nothing Efficiently*

Imagine you're making breakfast. You put bread in the toaster and eggs on the stove. Do you stand there staring at the toaster until it pops? Of course not! You watch the eggs while the toaster does its thing. That's async programming: doing other work while waiting.

## The Waiting Game

In traditional programming, waiting looks like this:

```palladium
// Synchronous - one thing at a time
fn make_breakfast() {
    let toast = make_toast();      // Wait 2 minutes...
    let eggs = cook_eggs();        // Wait 3 minutes...
    let coffee = brew_coffee();    // Wait 4 minutes...
    // Total: 9 minutes!
}
```

With async, it looks like this:

```palladium
// Async - everything at once!
async fn make_breakfast() {
    let toast_future = make_toast_async();
    let eggs_future = cook_eggs_async();
    let coffee_future = brew_coffee_async();
    
    // Wait for all to finish
    let (toast, eggs, coffee) = join!(toast_future, eggs_future, coffee_future);
    // Total: 4 minutes (just the longest task!)
}
```

You saved 5 minutes by being smart about waiting!

## What Makes a Function Async?

An async function is just a regular function that might need to wait:

```palladium
async fn fetch_weather() -> String {
    // This might take time...
    "Sunny, 72°F"
}

fn main() {
    // Calling async function creates a "future"
    let weather_future = fetch_weather();
    // Nothing happened yet! It's just a promise to do work
}
```

## Futures: Promises of Work

A future is like a restaurant order ticket:
- You place an order (call async function)
- You get a ticket (the future)
- The kitchen works on it (async runtime)
- Eventually, your food arrives (future completes)

```palladium
async fn download_file(url: String) -> Vec<u8> {
    // Returns immediately with a future
    // Actual download happens when awaited
}

async fn main() {
    let future = download_file("example.com/file.zip");
    // No download yet!
    
    let data = future.await;  // NOW it downloads
    print("Downloaded!");
}
```

## No Colored Functions!

Many languages have "async contamination" - if you call async, you must be async. Palladium treats async as an effect:

```palladium
// Regular function can call async!
fn process_data() ![async] {
    let data = fetch_data().await;  // Just mark the effect
    analyze(data)
}
```

It's like saying "this function might need to wait" - that's all!

## Real-World Example: Web Server

Here's how async shines in practice:

```palladium
async fn handle_request(request: Request) -> Response {
    // These can all happen simultaneously!
    let user = fetch_user_from_db(request.user_id);
    let permissions = check_permissions(request.user_id);
    let data = load_requested_data(request.path);
    
    // Wait for all three
    let (user, perms, data) = join!(user, permissions, data).await;
    
    if perms.can_access(data) {
        Response::ok(data)
    } else {
        Response::forbidden()
    }
}

async fn run_server() {
    let server = Server::bind("127.0.0.1:8080");
    
    // Handle thousands of connections concurrently!
    server.for_each_connection(|conn| {
        spawn(handle_request(conn))  // Each runs independently
    }).await;
}
```

One thread can handle thousands of connections because it's not wasting time waiting!

## What's Available Now (v0.8-alpha)

Currently, Palladium supports:

```palladium
// Define async functions
async fn hello() {
    print("Hello from async!");
}

fn main() {
    // Create futures
    let future = hello();
    // Note: Full async runtime coming in v0.9!
}
```

## The Magic: How It Works

Under the hood, async functions are state machines:

```palladium
// You write:
async fn count_to_three() {
    print("1");
    sleep(1).await;
    print("2");
    sleep(1).await;
    print("3");
}

// Compiler generates something like:
enum CountState {
    Start,
    AfterFirst,
    AfterSecond,
    Done,
}

struct CountFuture {
    state: CountState,
}

impl Future for CountFuture {
    fn poll(&mut self) -> Poll<()> {
        match self.state {
            Start => {
                print("1");
                self.state = AfterFirst;
                // Set up sleep timer...
                Poll::Pending
            }
            AfterFirst => {
                // Check if sleep done...
                print("2");
                self.state = AfterSecond;
                Poll::Pending
            }
            // ... and so on
        }
    }
}
```

The function pauses and resumes at each `await` point!

## Common Patterns

### Concurrent Operations
```palladium
async fn fetch_all_data(ids: Vec<i32>) -> Vec<Data> {
    // Launch all requests at once
    let futures: Vec<_> = ids.iter()
        .map(|id| fetch_data(*id))
        .collect();
    
    // Wait for all to complete
    join_all(futures).await
}
```

### Timeouts
```palladium
async fn fetch_with_timeout(url: String) -> Result<Data, Error> {
    select! {
        data = fetch_data(url) => Ok(data),
        _ = sleep(Duration::from_secs(30)) => Err(Error::Timeout),
    }
}
```

### Sequential Async
```palladium
async fn process_pipeline(input: Data) -> Result<Output, Error> {
    let step1 = transform_data(input).await?;
    let step2 = validate_data(step1).await?;
    let step3 = save_to_database(step2).await?;
    Ok(step3)
}
```

## Async vs Threads

Why not just use threads for everything?

| Threads | Async |
|---------|--------|
| ~1MB overhead each | ~100 bytes per task |
| OS scheduling | Cooperative scheduling |
| Good for CPU work | Good for I/O waiting |
| Limited quantity | Millions possible |

Think of threads as trucks and async tasks as packages - you can fit way more packages!

## Mental Model

Think of async like a restaurant:
- **Waiter** = Async runtime
- **Orders** = Futures
- **Kitchen** = Actual work
- **Tables** = Concurrent tasks

One waiter can handle many tables because they don't stand at each table waiting for food!

## Common Misconceptions

### "Async makes things faster"
Not exactly! Async makes *waiting* more efficient. CPU-bound work doesn't benefit.

### "Async is always better"
Nope! Use async for I/O, threads for CPU work.

### "Async is complicated"
Only if you think too hard! It's just "do other stuff while waiting."

## Simple Example That Works Today

```palladium
async fn delay_print(msg: &str) {
    print(msg);
}

fn main() {
    // Create futures (instant)
    let f1 = delay_print("First");
    let f2 = delay_print("Second");
    let f3 = delay_print("Third");
    
    // In v0.9, you'll be able to:
    // join!(f1, f2, f3).await;
    
    print("All futures created!");
}
```

## The Future of Async (v0.9 and beyond)

Coming soon:
- Full async runtime
- `await` syntax
- Async traits
- Streams (async iterators)
- Channels for communication
- Timer utilities

## Best Practices

1. **Use async for I/O, not computation**
2. **Don't block in async functions**
3. **Keep futures small and focused**
4. **Think in terms of tasks, not threads**

## Real Power: Composition

The beauty of async is how it composes:

```palladium
async fn complex_operation() {
    // These all run concurrently!
    let results = join!(
        fetch_from_database(),
        call_external_api(),
        read_from_file(),
        calculate_something_async()
    ).await;
    
    // Process results...
}
```

No callbacks, no complexity - just write what you mean!

[Next: Chapter 8 - Effects are Side Stories →](chapter_8_effects.md)

---

*Exercise: Design an async file downloader that:*
- *Downloads multiple files concurrently*
- *Shows progress for each file*
- *Retries failed downloads*
- *Has a global timeout*

*How would you structure this with futures?*