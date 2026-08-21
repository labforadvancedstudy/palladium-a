// Fibonacci benchmark in Rust -- reference implementation for
// benchmarks/palladium/fibonacci.pd. Same algorithm, same workload,
// byte-identical stdout.
//
// Build: rustc -O fibonacci.rs -o fibonacci_rs

fn fibonacci(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    return fibonacci(n - 1) + fibonacci(n - 2);
}

fn main() {
    let n: i64 = 42;
    println!("benchmark: fibonacci");
    println!("n:");
    println!("{}", n);
    let result: i64 = fibonacci(n);
    println!("result:");
    println!("{}", result);
}
