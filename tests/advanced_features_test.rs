use palladium::Driver;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Helper to compile and run a Palladium program
fn compile_and_run(source: &str) -> Result<String, String> {
    let temp_dir = TempDir::new().unwrap();
    let source_path = temp_dir.path().join("test.pd");
    let output_path = temp_dir.path().join("test");
    
    fs::write(&source_path, source).unwrap();
    
    // Compile
    let driver = Driver::new();
    match driver.compile_file(&source_path) {
        Ok(c_path) => {
            // Compile C to executable
            let cc_output = Command::new("cc")
                .arg("-o")
                .arg(&output_path)
                .arg(&c_path)
                .output()
                .map_err(|e| format!("Failed to run cc: {}", e))?;
            
            if !cc_output.status.success() {
                return Err(String::from_utf8_lossy(&cc_output.stderr).to_string());
            }
            
            // Run
            let output = Command::new(&output_path)
                .output()
                .map_err(|e| format!("Failed to run: {}", e))?;
            
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        Err(e) => Err(format!("Compilation failed: {}", e)),
    }
}

#[test]
fn test_async_await_basic() {
    let source = r#"
    async fn fetch_data() -> int {
        // Simulate async operation
        42
    }
    
    async fn main() {
        let result = await fetch_data();
        print(result);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "42");
}

#[test]
fn test_async_await_multiple() {
    let source = r#"
    async fn fetch_a() -> int { 10 }
    async fn fetch_b() -> int { 20 }
    async fn fetch_c() -> int { 30 }
    
    async fn main() {
        let a = await fetch_a();
        let b = await fetch_b();
        let c = await fetch_c();
        print(a + b + c);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "60");
}

#[test]
fn test_effects_system() {
    let source = r#"
    effect IO {
        fn read_line() -> string;
        fn write_line(s: string);
    }
    
    effect Random {
        fn next() -> int;
    }
    
    fn pure_function(x: int) -> int {
        x * 2
    }
    
    fn impure_function() -> int with IO, Random {
        write_line("Generating random number...");
        let n = next();
        write_line("Done!");
        n
    }
    
    fn main() with IO {
        let x = pure_function(21);
        print(x);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "42");
}

#[test]
fn test_trait_system() {
    let source = r#"
    trait Display {
        fn display(&self) -> string;
    }
    
    struct Point {
        x: int,
        y: int
    }
    
    impl Display for Point {
        fn display(&self) -> string {
            "Point(" + self.x + ", " + self.y + ")"
        }
    }
    
    fn print_display<T: Display>(item: &T) {
        print(item.display());
    }
    
    fn main() {
        let p = Point { x: 10, y: 20 };
        print_display(&p);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("Point(10, 20)"));
}

#[test]
fn test_generic_collections() {
    let source = r#"
    struct Vec<T> {
        data: [T; 100],
        len: int
    }
    
    impl<T> Vec<T> {
        fn new() -> Vec<T> {
            Vec { data: [default(); 100], len: 0 }
        }
        
        fn push(&mut self, item: T) {
            self.data[self.len] = item;
            self.len = self.len + 1;
        }
        
        fn get(&self, index: int) -> &T {
            &self.data[index]
        }
    }
    
    fn main() {
        let mut v = Vec::new();
        v.push(1);
        v.push(2);
        v.push(3);
        
        for i in 0..v.len {
            print(*v.get(i));
        }
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("1"));
    assert!(output.contains("2"));
    assert!(output.contains("3"));
}

#[test]
fn test_pattern_matching_advanced() {
    let source = r#"
    enum Option<T> {
        Some(T),
        None
    }
    
    enum Result<T, E> {
        Ok(T),
        Err(E)
    }
    
    fn divide(a: int, b: int) -> Result<int, string> {
        if b == 0 {
            Result::Err("Division by zero")
        } else {
            Result::Ok(a / b)
        }
    }
    
    fn main() {
        match divide(10, 2) {
            Result::Ok(value) => print(value),
            Result::Err(msg) => print(msg)
        }
        
        match divide(10, 0) {
            Result::Ok(value) => print(value),
            Result::Err(msg) => print(msg)
        }
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("5"));
    assert!(output.contains("Division by zero"));
}

#[test]
fn test_closures() {
    let source = r#"
    fn main() {
        let x = 10;
        let add_x = |y| x + y;
        
        print(add_x(5));
        print(add_x(10));
        
        let numbers = [1, 2, 3, 4, 5];
        let sum = numbers.fold(0, |acc, n| acc + n);
        print(sum);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("15"));
    assert!(output.contains("20"));
    assert!(output.contains("15")); // sum
}

#[test]
fn test_lifetime_annotations() {
    let source = r#"
    struct Ref<'a, T> {
        data: &'a T
    }
    
    fn longest<'a>(x: &'a string, y: &'a string) -> &'a string {
        if x.len() > y.len() {
            x
        } else {
            y
        }
    }
    
    fn main() {
        let s1 = "hello";
        let s2 = "world!";
        let result = longest(&s1, &s2);
        print(result);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "world!");
}

#[test]
fn test_unsafe_operations() {
    let source = r#"
    fn main() {
        let mut x = 42;
        let ptr = &mut x as *mut int;
        
        unsafe {
            *ptr = 100;
        }
        
        print(x);
        
        unsafe fn dangerous_function() -> int {
            // Unsafe operations
            123
        }
        
        let result = unsafe { dangerous_function() };
        print(result);
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("100"));
    assert!(output.contains("123"));
}

#[test]
fn test_macros() {
    let source = r#"
    macro_rules! vec {
        () => { Vec::new() };
        ($($x:expr),*) => {
            {
                let mut v = Vec::new();
                $(v.push($x);)*
                v
            }
        };
    }
    
    macro_rules! assert_eq {
        ($left:expr, $right:expr) => {
            if $left != $right {
                panic!("assertion failed: {} != {}", $left, $right);
            }
        };
    }
    
    fn main() {
        let v1 = vec![];
        let v2 = vec![1, 2, 3];
        
        assert_eq!(v2.len, 3);
        print("All assertions passed!");
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("All assertions passed!"));
}

#[test]
fn test_const_generics() {
    let source = r#"
    struct Array<T, const N: int> {
        data: [T; N]
    }
    
    impl<T, const N: int> Array<T, N> {
        fn new(default: T) -> Array<T, N> where T: Copy {
            Array { data: [default; N] }
        }
        
        fn len(&self) -> int {
            N
        }
    }
    
    fn main() {
        let arr1: Array<int, 5> = Array::new(0);
        let arr2: Array<int, 10> = Array::new(42);
        
        print(arr1.len());
        print(arr2.len());
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("5"));
    assert!(output.contains("10"));
}

#[test]
fn test_module_system() {
    let source = r#"
    mod math {
        pub fn add(a: int, b: int) -> int {
            a + b
        }
        
        pub fn multiply(a: int, b: int) -> int {
            a * b
        }
        
        pub mod advanced {
            pub fn power(base: int, exp: int) -> int {
                let mut result = 1;
                for _ in 0..exp {
                    result = result * base;
                }
                result
            }
        }
    }
    
    use math::{add, multiply};
    use math::advanced::power;
    
    fn main() {
        print(add(5, 3));
        print(multiply(4, 7));
        print(power(2, 8));
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("8"));
    assert!(output.contains("28"));
    assert!(output.contains("256"));
}

#[test]
fn test_iterator_protocol() {
    let source = r#"
    trait Iterator {
        type Item;
        fn next(&mut self) -> Option<Self::Item>;
    }
    
    struct Range {
        current: int,
        end: int
    }
    
    impl Iterator for Range {
        type Item = int;
        
        fn next(&mut self) -> Option<int> {
            if self.current < self.end {
                let value = self.current;
                self.current = self.current + 1;
                Option::Some(value)
            } else {
                Option::None
            }
        }
    }
    
    fn main() {
        let mut range = Range { current: 0, end: 5 };
        
        while let Option::Some(value) = range.next() {
            print(value);
        }
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    let output = result.unwrap();
    for i in 0..5 {
        assert!(output.contains(&i.to_string()));
    }
}

#[test]
fn test_error_handling_sugar() {
    let source = r#"
    enum Result<T, E> {
        Ok(T),
        Err(E)
    }
    
    fn might_fail(x: int) -> Result<int, string> {
        if x < 0 {
            Result::Err("Negative number")
        } else {
            Result::Ok(x * 2)
        }
    }
    
    fn process() -> Result<int, string> {
        let a = might_fail(5)?;
        let b = might_fail(10)?;
        let c = might_fail(-1)?; // This will return early
        Result::Ok(a + b + c)
    }
    
    fn main() {
        match process() {
            Result::Ok(value) => print(value),
            Result::Err(msg) => print(msg)
        }
    }
    "#;
    
    let result = compile_and_run(source);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("Negative number"));
}