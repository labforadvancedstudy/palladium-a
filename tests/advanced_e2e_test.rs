// End-to-end tests for advanced Palladium language features
// (generics, traits, async, effects, and more).
//
// Every test in this file needs a language feature that does not exist yet, so
// every one carries an `#[ignore = "XFAIL: …"]` naming the missing feature, the
// line of `docs/specification/grammar.ebnf` that records its absence, and the
// milestone that owns it. `make test-xfail` runs them and fails if any starts
// passing — a stale expectation is the failure mode this repo exists to kill,
// which is the same rule `scripts/conformance.sh` applies to XPASS.

mod common;

use common::unique_source_name;
use palladium::Driver;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Helper to compile source and run the resulting executable.
///
/// The source file name is unique because the driver turns it into
/// `build_output/<stem>.c`, a path shared with every other test binary.
/// See `tests/common/mod.rs`.
fn compile_and_run(source: &str) -> Result<String, String> {
    let temp_dir = TempDir::new().unwrap();
    let source_path = temp_dir.path().join(unique_source_name("adv_e2e"));
    let exe_path = temp_dir.path().join("test");

    // Write source
    fs::write(&source_path, source).unwrap();
    
    // Compile to C
    let driver = Driver::new();
    match driver.compile_file(&source_path) {
        Ok(c_output_path) => {
            // Compile C to executable
            let cc_output = Command::new("cc")
                .arg("-o")
                .arg(&exe_path)
                .arg(&c_output_path)
                .output()
                .map_err(|e| format!("Failed to run cc: {}", e))?;
            
            if !cc_output.status.success() {
                return Err(String::from_utf8_lossy(&cc_output.stderr).to_string());
            }
            
            // Run executable
            let run_output = Command::new(&exe_path)
                .output()
                .map_err(|e| format!("Failed to run executable: {}", e))?;
            
            if run_output.status.success() {
                Ok(String::from_utf8_lossy(&run_output.stdout).to_string())
            } else {
                Err(String::from_utf8_lossy(&run_output.stderr).to_string())
            }
        }
        Err(e) => Err(format!("Compilation failed: {}", e)),
    }
}

#[test]
#[ignore = "XFAIL: generic functions are not monomorphised per call site — `identity(42)` fixes T, then `identity('hello')` is rejected with 'expected String, found Int' (owned by M4, 'Generics that work')"]
fn test_generic_identity_function() {
    let source = r#"
    fn identity<T>(x: T) -> T {
        x
    }
    
    fn main() {
        print(identity(42));
        print(identity("hello"));
        print(identity(true));
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.contains("42"));
    assert!(output.contains("hello"));
    assert!(output.contains("1")); // true prints as 1
}

#[test]
#[ignore = "XFAIL: generic structs with an impl block — `Pair::new` is resolved as an enum variant path, so it reports 'Undefined enum type: Pair' (owned by M4, 'Generics that work')"]
fn test_generic_pair() {
    let source = r#"
    struct Pair<T, U> {
        first: T,
        second: U
    }
    
    impl<T, U> Pair<T, U> {
        fn new(first: T, second: U) -> Pair<T, U> {
            Pair { first: first, second: second }
        }
        
        fn swap(self) -> Pair<U, T> {
            Pair { first: self.second, second: self.first }
        }
    }
    
    fn main() {
        let p1 = Pair::new(10, "hello");
        print(p1.first);
        print(p1.second);
        
        let p2 = p1.swap();
        print(p2.first);
        print(p2.second);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    let lines: Vec<&str> = output.trim().split('\n').collect();
    assert_eq!(lines[0], "10");
    assert_eq!(lines[1], "hello");
    assert_eq!(lines[2], "hello");
    assert_eq!(lines[3], "10");
}

#[test]
#[ignore = "XFAIL: `&self` receivers in a trait method — grammar.ebnf:146 'A trait method declared with a `self` receiver is a PARSE ERROR' (owned by M4, traits with real dispatch)"]
fn test_trait_implementation() {
    let source = r#"
    trait Drawable {
        fn draw(&self);
    }
    
    struct Circle {
        radius: int
    }
    
    struct Rectangle {
        width: int,
        height: int
    }
    
    impl Drawable for Circle {
        fn draw(&self) {
            print("Drawing circle with radius:");
            print(self.radius);
        }
    }
    
    impl Drawable for Rectangle {
        fn draw(&self) {
            print("Drawing rectangle:");
            print(self.width);
            print(self.height);
        }
    }
    
    fn draw_shape(shape: &impl Drawable) {
        shape.draw();
    }
    
    fn main() {
        let c = Circle { radius: 5 };
        let r = Rectangle { width: 10, height: 20 };
        
        draw_shape(&c);
        draw_shape(&r);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.contains("Drawing circle"));
    assert!(output.contains("5"));
    assert!(output.contains("Drawing rectangle"));
    assert!(output.contains("10"));
    assert!(output.contains("20"));
}

#[test]
#[ignore = "XFAIL: a generic enum variant does not infer the enum's type argument — `Option::None` in a function returning `Option<int>` is checked as bare `Option`, so this fixture dies at 'Type mismatch: expected Option<Int>, found Option' before it reaches the method-call syntax (`x.f()`, grammar.ebnf:248) it was written for. Both are owed; this is the one that fires (owned by M2, item 1)"]
fn test_option_enum() {
    let source = r#"
    enum Option<T> {
        Some(T),
        None
    }
    
    impl<T> Option<T> {
        fn is_some(&self) -> bool {
            match self {
                Option::Some(_) => true,
                Option::None => false
            }
        }
        
        fn unwrap(self) -> T {
            match self {
                Option::Some(value) => value,
                Option::None => panic("unwrap on None")
            }
        }
    }
    
    fn divide(a: int, b: int) -> Option<int> {
        if b == 0 {
            Option::None
        } else {
            Option::Some(a / b)
        }
    }
    
    fn main() {
        let result1 = divide(10, 2);
        if result1.is_some() {
            print(result1.unwrap());
        }
        
        let result2 = divide(10, 0);
        if result2.is_some() {
            print("Should not print");
        } else {
            print("Division by zero");
        }
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.contains("5"));
    assert!(output.contains("Division by zero"));
    assert!(!output.contains("Should not print"));
}

#[test]
#[ignore = "XFAIL: multi-parameter generic enums — `Result<T, E>` loses its second type argument, so `Result::Err(String)` is checked against Int (owned by M4, 'Generics that work')"]
fn test_result_error_handling() {
    let source = r#"
    enum Result<T, E> {
        Ok(T),
        Err(E)
    }
    
    fn parse_int(s: String) -> Result<int, String> {
        // Simplified - just check if it's "42"
        if s == "42" {
            Result::Ok(42)
        } else {
            Result::Err("Not a valid number")
        }
    }
    
    fn main() {
        match parse_int("42") {
            Result::Ok(n) => {
                print("Parsed:");
                print(n);
            },
            Result::Err(e) => {
                print("Error:");
                print(e);
            }
        }
        
        match parse_int("abc") {
            Result::Ok(n) => print(n),
            Result::Err(e) => print(e)
        }
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.contains("Parsed:"));
    assert!(output.contains("42"));
    assert!(output.contains("Not a valid number"));
}

#[test]
#[ignore = "XFAIL: associated types in a trait (`type Item;`) — grammar.ebnf:130-131 admits only `fn` items in a trait body (owned by M4, 'Traits with real dispatch')"]
fn test_iterator_trait() {
    let source = r#"
    trait Iterator {
        type Item;
        fn next(&mut self) -> Option<Self::Item>;
    }
    
    struct Counter {
        count: int,
        max: int
    }
    
    impl Iterator for Counter {
        type Item = int;
        
        fn next(&mut self) -> Option<int> {
            if self.count < self.max {
                let result = self.count;
                self.count = self.count + 1;
                Option::Some(result)
            } else {
                Option::None
            }
        }
    }
    
    fn main() {
        let mut counter = Counter { count: 0, max: 5 };
        
        while let Option::Some(n) = counter.next() {
            print(n);
        }
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    for i in 0..5 {
        assert!(output.contains(&i.to_string()));
    }
}

#[test]
#[ignore = "XFAIL: closures — grammar.ebnf:251 'There are no closures'; `|y| x + y` stops the parser at '|' (owned by M4, 'Abstraction')"]
fn test_closure_capture() {
    let source = r#"
    fn main() {
        let x = 10;
        let add_x = |y| x + y;
        
        print(add_x(5));
        print(add_x(10));
        
        let multiplier = 3;
        let multiply = |n| n * multiplier;
        
        print(multiply(4));
        print(multiply(7));
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.contains("15"));  // 10 + 5
    assert!(output.contains("20"));  // 10 + 10
    assert!(output.contains("12"));  // 4 * 3
    assert!(output.contains("21"));  // 7 * 3
}

#[test]
#[ignore = "XFAIL: function types — grammar.ebnf:178 'No function types'; a parameter declared `f: fn(T) -> U` stops the parser at 'fn' (owned by M4, 'Abstraction')"]
fn test_higher_order_functions() {
    let source = r#"
    fn map<T, U>(arr: [T; 5], f: fn(T) -> U) -> [U; 5] {
        let mut result: [U; 5];
        for i in 0..5 {
            result[i] = f(arr[i]);
        }
        result
    }
    
    fn double(x: int) -> int {
        x * 2
    }
    
    fn main() {
        let numbers = [1, 2, 3, 4, 5];
        let doubled = map(numbers, double);
        
        for i in 0..5 {
            print(doubled[i]);
        }
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.contains("2"));
    assert!(output.contains("4"));
    assert!(output.contains("6"));
    assert!(output.contains("8"));
    assert!(output.contains("10"));
}

#[test]
#[ignore = "XFAIL: an `async fn` body is not wrapped into a Future. The declared blocker used to be the type checker's 'expected Future<Int>, found Int'; since fbcfc39 the fixture is refused EARLIER, by the outright refusal of a value-carrying `return` in an `async fn` — there is nowhere to put the value, because the body is emitted into a poll function returning an int readiness flag. The Future gap is still the reason nothing can be done about it (owned by unscheduled, MILESTONES.md 'Not scheduled, and why')"]
fn test_async_await() {
    let source = r#"
    async fn fetch_data(id: int) -> int {
        // Simulate async work
        id * 2
    }
    
    async fn process_data() -> int {
        let a = fetch_data(5).await;
        let b = fetch_data(10).await;
        a + b
    }
    
    fn main() {
        let runtime = Runtime::new();
        let result = runtime.block_on(process_data());
        print(result);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    assert_eq!(result.unwrap().trim(), "30"); // (5*2) + (10*2)
}

#[test]
#[ignore = "XFAIL: effect declarations — grammar.ebnf:127 'Effect clauses (`![io]`) do NOT exist in the surface syntax'; `effect IO { … }` is not an item (owned by unscheduled, MILESTONES.md 'Not scheduled, and why')"]
fn test_effects_system() {
    let source = r#"
    effect IO {
        fn read_line() -> String;
        fn write_line(s: String);
    }
    
    effect State<T> {
        fn get() -> T;
        fn put(value: T);
    }
    
    fn pure_computation(x: int, y: int) -> int {
        x * y + 10
    }
    
    fn stateful_computation() -> int with State<int> {
        let current = get();
        put(current + 1);
        current
    }
    
    fn main() with IO {
        write_line("Starting computation");
        let result = pure_computation(5, 7);
        write_line("Result computed");
        print(result);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.contains("Starting computation"));
    assert!(output.contains("45")); // 5*7 + 10
    assert!(output.contains("Result computed"));
}

#[test]
#[ignore = "XFAIL: struct-variant patterns with field shorthand and `if` guards — grammar.ebnf:269 'No literal patterns, ranges, or-patterns, guards, tuple/slice patterns, ref/mut bindings, @ bindings, field shorthand' (owned by M2, item 4)"]
fn test_pattern_matching_guards() {
    let source = r#"
    enum Message {
        Move { x: int, y: int },
        Write(String),
        ChangeColor(int, int, int),
        Quit
    }
    
    fn process_message(msg: Message) {
        match msg {
            Message::Move { x, y } if x > 0 && y > 0 => {
                print("Moving to positive quadrant");
            },
            Message::Move { x, y } => {
                print("Moving to other location");
            },
            Message::Write(text) if text == "hello" => {
                print("Greeting received");
            },
            Message::Write(text) => {
                print("Message:");
                print(text);
            },
            Message::ChangeColor(r, g, b) if r == 255 && g == 0 && b == 0 => {
                print("Changing to red");
            },
            Message::ChangeColor(_, _, _) => {
                print("Changing to custom color");
            },
            Message::Quit => {
                print("Quitting");
            }
        }
    }
    
    fn main() {
        process_message(Message::Move { x: 10, y: 20 });
        process_message(Message::Move { x: -5, y: 10 });
        process_message(Message::Write("hello"));
        process_message(Message::Write("world"));
        process_message(Message::ChangeColor(255, 0, 0));
        process_message(Message::ChangeColor(0, 255, 0));
        process_message(Message::Quit);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.contains("Moving to positive quadrant"));
    assert!(output.contains("Moving to other location"));
    assert!(output.contains("Greeting received"));
    assert!(output.contains("Message:"));
    assert!(output.contains("world"));
    assert!(output.contains("Changing to red"));
    assert!(output.contains("Changing to custom color"));
    assert!(output.contains("Quitting"));
}

#[test]
#[ignore = "XFAIL: const generic parameters on an `impl` block — grammar.ebnf:147 admits `const N: T` and `fn`/`struct`/`enum` do parse it, but `parse_impl`'s parameter loop (src/parser/mod.rs:1704-1713) has no `const` arm and reports 'Expected type parameter name, but found const' (owned by M4, 'Generics that work')"]
fn test_const_generics_arrays() {
    let source = r#"
    struct Matrix<T, const ROWS: int, const COLS: int> {
        data: [[T; COLS]; ROWS]
    }
    
    impl<T, const ROWS: int, const COLS: int> Matrix<T, ROWS, COLS> {
        fn new(default: T) -> Matrix<T, ROWS, COLS> where T: Copy {
            Matrix {
                data: [[default; COLS]; ROWS]
            }
        }
        
        fn get(&self, row: int, col: int) -> T where T: Copy {
            self.data[row][col]
        }
        
        fn set(&mut self, row: int, col: int, value: T) {
            self.data[row][col] = value;
        }
    }
    
    fn main() {
        let mut mat: Matrix<int, 2, 3> = Matrix::new(0);
        
        mat.set(0, 0, 1);
        mat.set(0, 1, 2);
        mat.set(0, 2, 3);
        mat.set(1, 0, 4);
        mat.set(1, 1, 5);
        mat.set(1, 2, 6);
        
        for i in 0..2 {
            for j in 0..3 {
                print(mat.get(i, j));
            }
        }
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    for i in 1..=6 {
        assert!(output.contains(&i.to_string()));
    }
}

#[test]
#[ignore = "XFAIL: tuple expressions — grammar.ebnf:251 'no tuple expressions'; tuple *types* parse but lower to void* and cannot be constructed (owned by M2, surface syntax)"]
fn test_type_aliases_complex() {
    let source = r#"
    type NodeId = int;
    type Weight = int;
    type Graph = [(NodeId, NodeId, Weight); 100];
    type Path = [NodeId; 50];
    
    fn add_edge(graph: &mut Graph, edge_count: &mut int, from: NodeId, to: NodeId, weight: Weight) {
        graph[*edge_count] = (from, to, weight);
        *edge_count = *edge_count + 1;
    }
    
    fn main() {
        let mut graph: Graph;
        let mut edge_count = 0;
        
        add_edge(&mut graph, &mut edge_count, 0, 1, 10);
        add_edge(&mut graph, &mut edge_count, 1, 2, 20);
        add_edge(&mut graph, &mut edge_count, 2, 3, 30);
        
        for i in 0..edge_count {
            let (from, to, weight) = graph[i];
            print(from);
            print(to);
            print(weight);
        }
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.contains("0"));
    assert!(output.contains("1"));
    assert!(output.contains("10"));
    assert!(output.contains("2"));
    assert!(output.contains("20"));
    assert!(output.contains("3"));
    assert!(output.contains("30"));
}