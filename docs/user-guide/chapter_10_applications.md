# Chapter 10: Building Real Things

*Or: From Theory to Thunder*

You've learned the concepts. Now let's build something real. This chapter shows how all the pieces—ownership, types, traits, async, effects, and proofs—come together to create powerful applications.

## Project 1: A Safe Web Server

Let's build a web server that can't crash, can't leak memory, and handles thousands of connections:

```palladium
use std::net::{TcpListener, TcpStream};
use std::thread;

struct WebServer {
    port: u16,
    handler: fn(Request) -> Response,
}

impl WebServer {
    fn new(port: u16) -> Self {
        WebServer {
            port,
            handler: default_handler,
        }
    }
    
    async fn start(&self) ![io, net] {
        let listener = TcpListener::bind(("127.0.0.1", self.port));
        println!("Server running on port {}", self.port);
        
        for stream in listener.incoming() {
            // Each connection handled concurrently
            spawn(self.handle_connection(stream));
        }
    }
    
    async fn handle_connection(&self, stream: TcpStream) ![io] {
        let request = parse_request(stream)?;
        let response = (self.handler)(request);
        write_response(stream, response).await?;
    }
}

// Type-safe routing
enum Route {
    Home,
    User(u64),
    Api { endpoint: String, version: u32 },
}

fn router(request: Request) -> Response {
    match parse_route(&request.path) {
        Route::Home => serve_home(),
        Route::User(id) => serve_user_profile(id),
        Route::Api { endpoint, version } => {
            handle_api_request(endpoint, version)
        }
    }
}
```

What makes this Palladium server special:
- **Memory safe** - No buffer overflows, ever
- **Concurrent** - Async handles thousands of connections
- **Type safe** - Routes are typed, not strings
- **Effect tracked** - IO and network effects explicit

## Project 2: A Provably Correct Database

Let's build a key-value store that guarantees data integrity:

```palladium
// Invariant: keys are always sorted
struct SortedKvStore {
    keys: Vec<String>,
    values: Vec<String>,
}

impl SortedKvStore {
    #[ensure(self.keys.is_sorted())]
    fn insert(&mut self, key: String, value: String) {
        match self.keys.binary_search(&key) {
            Ok(idx) => {
                // Key exists, update value
                self.values[idx] = value;
            }
            Err(idx) => {
                // Insert maintaining sort order
                self.keys.insert(idx, key);
                self.values.insert(idx, value);
            }
        }
        // Compiler proves: keys remain sorted!
    }
    
    fn get(&self, key: &str) -> Option<&String> {
        self.keys.binary_search(key)
            .ok()
            .map(|idx| &self.values[idx])
    }
}

// Transaction with rollback
struct Transaction<'a> {
    store: &'a mut SortedKvStore,
    backup: SortedKvStore,
}

impl<'a> Transaction<'a> {
    fn new(store: &'a mut SortedKvStore) -> Self {
        Transaction {
            backup: store.clone(),
            store,
        }
    }
    
    fn commit(self) {
        // Changes are kept
    }
    
    fn rollback(self) {
        *self.store = self.backup;  // Restore original
    }
}
```

Guarantees:
- **Sorted invariant** - Keys always sorted for O(log n) lookup
- **Transactional** - All or nothing updates
- **Memory safe** - No corruption possible

## Project 3: Zero-Copy Parser

Parse network protocols without allocating:

```palladium
// A packet that borrows from a buffer
struct Packet<'a> {
    version: u8,
    flags: u8,
    payload: &'a [u8],
}

impl<'a> Packet<'a> {
    fn parse(buffer: &'a [u8]) -> Result<Self, ParseError> {
        if buffer.len() < 4 {
            return Err(ParseError::TooShort);
        }
        
        Ok(Packet {
            version: buffer[0],
            flags: buffer[1],
            payload: &buffer[4..],  // Zero-copy slice!
        })
    }
}

// Parse without allocation
fn process_packets(data: &[u8]) -> Result<Vec<Summary>, ParseError> {
    let mut summaries = Vec::new();
    let mut offset = 0;
    
    while offset < data.len() {
        let packet = Packet::parse(&data[offset..])?;
        
        summaries.push(Summary {
            version: packet.version,
            size: packet.payload.len(),
        });
        
        offset += 4 + packet.payload.len();
    }
    
    Ok(summaries)
}
```

Benefits:
- **Zero allocations** - Parses in-place
- **Cache friendly** - Data stays put
- **Safe** - Lifetimes prevent use-after-free

## Project 4: Concurrent Task System

Build a work-stealing task executor:

```palladium
trait Task: Send {
    async fn execute(&mut self) ![any];
}

struct TaskExecutor {
    workers: Vec<Worker>,
    queue: WorkStealingQueue<Box<dyn Task>>,
}

impl TaskExecutor {
    fn spawn<T: Task + 'static>(&self, task: T) {
        self.queue.push(Box::new(task));
        self.notify_workers();
    }
    
    async fn run(&self) {
        let handles: Vec<_> = self.workers
            .iter()
            .map(|w| spawn(w.run_loop()))
            .collect();
            
        join_all(handles).await;
    }
}

struct Worker {
    id: usize,
    queue: WorkStealingQueue<Box<dyn Task>>,
}

impl Worker {
    async fn run_loop(&self) {
        loop {
            if let Some(task) = self.queue.pop() {
                task.execute().await;
            } else if let Some(task) = self.steal_from_others() {
                task.execute().await;
            } else {
                yield_now().await;  // Give up time slice
            }
        }
    }
}
```

Features:
- **Work stealing** - Balance load automatically
- **Type safe** - Tasks are typed
- **Concurrent** - Multiple workers
- **Fair** - No task starvation

## Project 5: Embedded Systems Controller

For when every byte counts:

```palladium
#![no_std]  // No standard library!

// Fixed-size allocator for embedded
struct FixedAllocator<const N: usize> {
    memory: [u8; N],
    free_list: Option<*mut Block>,
}

// State machine for hardware control
enum MotorState {
    Stopped,
    Running { speed: u16 },
    Fault { code: u8 },
}

struct MotorController {
    state: MotorState,
    target_speed: u16,
}

impl MotorController {
    #[inline(always)]  // Force inlining
    fn update(&mut self) {
        match self.state {
            MotorState::Stopped => {
                if self.target_speed > 0 {
                    self.start_motor();
                }
            }
            MotorState::Running { speed } => {
                self.adjust_speed(speed);
            }
            MotorState::Fault { .. } => {
                // Stay faulted until reset
            }
        }
    }
    
    fn emergency_stop(&mut self) {
        unsafe {
            // Direct hardware access
            write_volatile(MOTOR_CONTROL_REG, 0);
        }
        self.state = MotorState::Stopped;
    }
}
```

Embedded benefits:
- **No heap** - Everything stack allocated
- **Deterministic** - Predictable timing
- **Small** - Compiles to tiny binary
- **Safe** - Even with hardware access

## Putting It All Together: A Real Application

Let's combine everything into a monitoring system:

```palladium
// Domain types with invariants
#[derive(Debug, Clone)]
struct SensorReading {
    sensor_id: SensorId,
    timestamp: Timestamp,
    value: f32,
}

// Async data collection
async fn collect_readings() -> Vec<SensorReading> ![io, net] {
    let sensors = discover_sensors().await?;
    
    let futures: Vec<_> = sensors
        .iter()
        .map(|s| read_sensor(s))
        .collect();
        
    join_all(futures).await
}

// Pure data processing
fn analyze_readings(readings: &[SensorReading]) -> Analysis {
    let mean = calculate_mean(readings);
    let variance = calculate_variance(readings, mean);
    let anomalies = detect_anomalies(readings);
    
    Analysis {
        mean,
        variance,
        anomalies,
    }
}

// Effect-tracked storage
async fn store_results(analysis: Analysis) ![io, db] {
    let db = connect_database().await?;
    
    transaction(&db, |tx| {
        tx.insert("analysis", &analysis)?;
        tx.update_statistics(&analysis)?;
        Ok(())
    }).await
}

// Main application loop
async fn monitoring_loop() ![io, net, db] {
    loop {
        // Collect (effectful)
        let readings = collect_readings().await;
        
        // Analyze (pure!)
        let analysis = analyze_readings(&readings);
        
        // Store (effectful)
        store_results(analysis).await;
        
        // Wait
        sleep(Duration::from_secs(60)).await;
    }
}
```

This application showcases:
- **Async I/O** - Non-blocking sensor reads
- **Pure analysis** - Testable, parallelizable
- **Effect tracking** - Clear dependencies
- **Type safety** - Domain types
- **Error handling** - Result types throughout

## Performance Tips

1. **Profile First**
```palladium
#[profile]
fn hot_path() {
    // Compiler instruments this
}
```

2. **Zero-Cost Abstractions**
```palladium
// This iterator code...
let sum: i32 = numbers
    .iter()
    .filter(|&&x| x > 0)
    .map(|&x| x * x)
    .sum();

// ...compiles to the same assembly as:
let mut sum = 0;
for &x in numbers {
    if x > 0 {
        sum += x * x;
    }
}
```

3. **Know Your Allocations**
```palladium
// Stack allocation (fast)
let array = [0; 1000];

// Heap allocation (slower)
let vec = vec![0; 1000];

// Reuse allocations
let mut buffer = Vec::with_capacity(1000);
buffer.clear();  // Reuse memory
```

## Debugging Techniques

1. **Type-Driven Debugging**
```palladium
// Make invalid states unrepresentable
enum ConnectionState {
    Disconnected,
    Connected { socket: TcpStream },
}
// Can't use socket when disconnected!
```

2. **Effect Tracking**
```palladium
// See exactly what a function does
fn mystery_function() ![io, net] {
    // Must do I/O and network operations
}
```

3. **Exhaustive Matching**
```palladium
match result {
    Ok(value) => process(value),
    Err(e) => match e {
        Error::Io(io_err) => handle_io(io_err),
        Error::Parse(parse_err) => handle_parse(parse_err),
        // Compiler ensures all errors handled
    }
}
```

## The Palladium Advantage

Building real applications in Palladium gives you:

1. **Confidence** - If it compiles, it works
2. **Performance** - Zero-overhead abstractions
3. **Maintainability** - Types document intent
4. **Scalability** - Async handles load
5. **Reliability** - No crashes, no leaks

## Your Journey Forward

You now understand:
- ✅ Ownership prevents memory bugs
- ✅ Types make invalid states impossible  
- ✅ Traits enable flexible abstractions
- ✅ Async handles concurrent operations
- ✅ Effects track side effects
- ✅ Proofs guarantee correctness

Now go build something amazing! Start small:

```palladium
fn main() {
    println!("Hello, Palladium!");
    println!("Let's build the future!");
}
```

Then grow your ambitions. The compiler has your back.

---

*Final Exercise: Design a system you'd like to build. Consider:*
- *What are the core types?*
- *Which operations are pure vs effectful?*
- *Where would async help?*
- *What invariants need protection?*
- *What properties could be proven?*

*Remember: In Palladium, we don't hope our code works—we know it does.*

## The End... and The Beginning

Congratulations! You've completed The Alan von Palladium Book. You understand not just the syntax, but the philosophy: making correct code the easy path.

Now go forth and build systems that don't crash, don't leak, and don't surprise. Build with confidence. Build with Palladium.

*"Turing's proofs, von Neumann's performance—now in your hands."*