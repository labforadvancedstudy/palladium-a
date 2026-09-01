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
            // THE RUNTIME TRANSLATION UNIT IS PART OF THE LINK, and leaving it
            // out was invisible for as long as it was: every test in this file
            // was `#[ignore]`d, so this `cc` had never once been asked to
            // produce a binary. The emitted C calls `pd_create_dir`,
            // `pd_read_file_to_string` and eight more file builtins through the
            // prelude's wrappers whether or not the program uses them, so a
            // `fn main() { print("hi"); }` fails to link here exactly as this
            // fixture did — measured, and not a property of the program under
            // test. `runtime_c()` rather than a literal path because that is the
            // one place that answers "where is my runtime?" (src/runtime_paths.rs),
            // and a hardcoded `runtime/palladium_runtime.c` would make this
            // harness cwd-dependent for the reason that module exists to fix.
            let runtime_c = palladium::runtime_paths::runtime_c()
                .map_err(|e| format!("Failed to locate the Palladium runtime: {}", e))?;

            // Compile C to executable
            let cc_output = Command::new("cc")
                .arg("-o")
                .arg(&exe_path)
                .arg(&c_output_path)
                .arg(&runtime_c)
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
#[ignore = "XFAIL: `&self` receivers in a trait method — grammar.ebnf:172 'A trait method declared with a `self` receiver is a PARSE ERROR' (owned by M4, traits with real dispatch)"]
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
#[ignore = "XFAIL: A GENERIC ENUM HAS NO REPRESENTATION. `Option::None` in a function returning `Option<int>` is refused with 'constructs a variant of a GENERIC enum, and generic enums are not implemented: code generation emits no type, no tag and no constructor for one'. RE-MEASURED IN su4, AND THE OLD REASON HERE WAS STALE: it said the fixture died at 'Type mismatch: expected Option<Int>, found Option', which the macro-era diagnostic replaced — column 5 had been updated and this prose had not. OWNER M2 -> M3: the fixture is `Option<T>` with an `impl<T>` method surface (`x.f()`, grammar.ebnf:346-349), which is M3 item 3 verbatim, '`Option<T>` and `Result<T, E>` as generic types with methods' (N4-16); the blocker under it is M3 item 1, 'Generics that work'. M2 owns no requirement row that would pay this (owned by M3, item 3)"]
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
#[ignore = "XFAIL: associated types in a trait (`type Item;`) — grammar.ebnf:169-170 admits only `fn` items in a trait body (`trait_item`) (owned by M4, 'Traits with real dispatch')"]
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
#[ignore = "XFAIL: closures — grammar.ebnf:388 'There are no closures'; `|y| x + y` stops the parser at '|' (owned by M4, 'Abstraction')"]
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
#[ignore = "XFAIL: function types — grammar.ebnf:218 'No function types'; a parameter declared `f: fn(T) -> U` stops the parser at 'fn' (owned by M4, 'Abstraction')"]
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
#[ignore = "XFAIL: effect declarations — grammar.ebnf:153 'Effect clauses (`![io]`) do NOT exist in the surface syntax'; `effect IO { … }` is not an item (owned by unscheduled, MILESTONES.md 'Not scheduled, and why')"]
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
#[ignore = "XFAIL: const generic parameters on an `impl` block — grammar.ebnf:173 admits `const N: T` and `fn`/`struct`/`enum` do parse it, but `parse_impl`'s parameter loop (src/parser/mod.rs:1758-1767) has no `const` arm and reports 'Expected type parameter name, but found const' (owned by M4, 'Generics that work')"]
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

/// COMPLEX TYPE ALIASES. RE-DERIVED WITHIN THE SPECIFICATION.
///
/// THE OLD BODY DEMANDED SYNTAX THIS LANGUAGE DELIBERATELY DOES NOT HAVE, so the row
/// could never be paid as written -- the N2-10 shape, where the kind is UNSATISFIABLE
/// rather than unmet. It asked for two constructs, both refused BY DESIGN by one
/// comment on one production in docs/specification/grammar.ebnf:230-231:
///
///   let_stmt = "let" [ "mut" ] identifier [ ':' type ] '=' expression ';' ;
///   "The initializer is mandatory, and the binding is a bare identifier --
///    `let` patterns do not exist."
///
///   1. `let mut graph: Graph;`              -- no initialiser.
///   2. `let (from, to, weight) = graph[i];` -- a destructuring `let`.
///
/// Rewriting a fixture to match the implementation is how a suite stops measuring
/// anything, so the two refusals were not merely dropped: each is now executed by its
/// own `reject` fixture, `tests/reject/let_needs_an_initializer.pd` and
/// `tests/reject/let_does_not_destructure.pd`. They were normative and UNWITNESSED
/// before this -- the claim rested on that comment alone.
///
/// THE SUBJECT IS UNCHANGED, and it is the alias system rather than the graph.
/// Exercised here: an alias to a primitive (`NodeId`), an ALIAS OF AN ALIAS
/// (`Weight = NodeId`), an alias to a TUPLE type (`Edge`) read with `.0`/`.1`/`.2` --
/// the in-spec spelling of what the old body destructured -- and an alias to a STRUCT
/// (`Graph = GraphData`), across parameter, return, field and `let`-annotation
/// positions. The old assertions' values (0, 1, 10, 2, 20, 3, 30) all still appear,
/// and the transcript is now compared EXACTLY rather than by `contains`, which could
/// not tell "1" from "10".
///
/// NOT COVERED, AND NOT CLAIMED. Probing this subject found SEVEN distinct alias
/// failures, each with its own diagnostic, and none of them a spec refusal. An
/// exclusion that names no rows is disclosure rather than inventory, so every one is
/// an EXECUTABLE row somewhere a gate reads, and this list is the index:
///
///   tests/xfail/alias_array_param_index.pd          indexing an array alias through
///                                                   a parameter
///   tests/xfail/alias_nested_in_tuple_annotation.pd an alias as a TUPLE COMPONENT
///   tests/xfail/alias_as_array_element.pd           an alias as an ARRAY ELEMENT
///   tests/xfail/alias_tuple_behind_reference.pd     a tuple alias behind `&`
///   tests/xfail/alias_struct_behind_reference.pd    a struct alias behind `&`
///        — five `xfail` rows in tests/conformance-manifest.txt, all owned by M3.
///
///   test_type_alias_to_array_lowers_to_valid_c
///   test_alias_typed_params_lower_to_valid_c_across_a_tuple_return
///        — the two below, in tests/rust-debt-manifest.txt instead, because in both
///          the front end ACCEPTS and gcc refuses the emitted C, and
///          scripts/conformance.sh refuses to let any manifest column declare that.
///
/// The five and the two are separated by that rule and not by severity: the two are
/// the more dangerous half.
#[test]
fn test_type_aliases_complex() {
    let source = r#"
    type NodeId = i64;
    type Weight = NodeId;
    type Edge = (i64, i64, i64);

    struct GraphData {
        count: NodeId,
        total: Weight,
    }

    type Graph = GraphData;

    fn heavier(a: Weight, b: Weight) -> Weight {
        if a > b {
            return a;
        }
        return b;
    }

    fn record(g: &mut GraphData, w: Weight) {
        g.count = g.count + 1;
        g.total = g.total + w;
    }

    fn labelled(n: NodeId) -> NodeId {
        return n + 100;
    }

    fn main() {
        let e0: Edge = (0, 1, 10);
        let e1: Edge = (1, 2, 20);
        let e2: Edge = (2, 3, 30);

        let mut g: Graph = GraphData { count: 0, total: 0 };
        record(&mut g, e0.2);
        record(&mut g, e1.2);
        record(&mut g, e2.2);

        print_int(e0.0); print_int(e0.1); print_int(e0.2);
        print_int(e1.0); print_int(e1.1); print_int(e1.2);
        print_int(e2.0); print_int(e2.1); print_int(e2.2);
        print_int(g.count);
        print_int(g.total);
        print_int(heavier(e0.2, e2.2));
        print_int(labelled(e1.0));
    }
    "#;

    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();

    // The three edges, then the struct the aliases are read into, then the two
    // alias-typed functions. Exact, because `contains("1")` is satisfied by "10" too.
    let expected = "0\n1\n10\n1\n2\n20\n2\n3\n30\n3\n60\n30\n101\n";
    assert_eq!(
        output, expected,
        "the alias program did not produce its transcript.\ngot:\n{}",
        output
    );
}

/// THE TWO ALIAS DEFECTS THE .pd INVENTORY IS NOT ALLOWED TO HOLD.
///
/// The other five shapes the su3 probe found are `xfail` rows in
/// tests/conformance-manifest.txt, under tests/xfail/. These two are not, and the
/// reason is structural rather than editorial: in both, the FRONT END ACCEPTS the
/// program and gcc then refuses the C this compiler emitted. scripts/conformance.sh
/// refuses that outcome whatever the manifest says --
///
///   "pdc accepted this source and then gcc refused the C it emitted. That is a
///    defect in this compiler, not a property of the fixture, and no manifest column
///    may declare it: there is no valid Palladium program whose emitted C is allowed
///    not to compile."
///
/// -- and `stage: link` is rejected by the row validator for the same reason. So the
/// .pd corpus cannot carry them, and leaving them in a commit message is what the
/// review round rejected. They live here instead, in the OTHER closed inventory:
/// tests/rust-debt-manifest.txt reconciles these rows by scripts/test-xfail.py, which
/// holds each to the diagnostic its `#[ignore]` names.
///
/// Both assert the SUCCESS they should have: when the lowering is fixed, they pass and
/// the rows transition to `paid`. Neither pins the current broken output.
#[test]
#[ignore = "XFAIL: A TYPE ALIAS TO AN ARRAY MIS-PLACES THE C DECLARATOR. `type Row = [i64; 4]; let r: Row = [...]` emits `long long[4] r = {...}` instead of `long long r[4] = {...}`, so gcc refuses it with \"brackets are not allowed here; to declare an array, place the brackets after the identifier\". The front end approves the program, which makes this the forbidden class rather than a fixture property (owned by M3, alias resolution in the C-name path)"]
fn test_type_alias_to_array_lowers_to_valid_c() {
    let source = r#"
    type Row = [i64; 4];

    fn main() {
        let r: Row = [5, 6, 7, 8];
        print_int(r[2]);
    }
    "#;

    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    assert_eq!(result.unwrap(), "7\n");
}

/// The same family, one boundary out: the alias NAME reaches C as if it were a C type.
#[test]
#[ignore = "XFAIL: AN ALIAS-TYPED PARAMETER LEAKS ITS ALIAS NAME INTO THE SYNTHESISED TUPLE STRUCT. `fn pair(a: NodeId, b: NodeId) -> (i64, i64)` emits `typedef struct { NodeId f0; NodeId f1; } __pd_tuple2_NodeId_NodeId;` — the tuple struct is named and typed from the UNRESOLVED alias, so gcc reports \"unknown type name 'NodeId'\" four times. Constructing the same tuple in a LOCAL works (measured, both annotated and inferred), so the defect is the function boundary, not tuple construction (owned by M3, alias resolution in the C-name path)"]
fn test_alias_typed_params_lower_to_valid_c_across_a_tuple_return() {
    let source = r#"
    type NodeId = i64;

    fn pair(a: NodeId, b: NodeId) -> (i64, i64) {
        return (a, b);
    }

    fn main() {
        let p = pair(7, 8);
        print_int(p.0 + p.1);
    }
    "#;

    let result = compile_and_run(source);
    assert!(result.is_ok(), "{:?}", result);
    assert_eq!(result.unwrap(), "15\n");
}