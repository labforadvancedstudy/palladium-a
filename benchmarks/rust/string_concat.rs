// String concatenation benchmark in Rust -- reference implementation for
// benchmarks/palladium/string_concat.pd. Same algorithm, same workload,
// byte-identical stdout.
//
// This deliberately replicates Palladium's `string_concat` SEMANTICS: strings
// are immutable, and every concat allocates a fresh buffer of len(a)+len(b) and
// copies both operands into it. Total work is therefore quadratic, exactly as
// in the Palladium version. The one thing Rust does that Palladium's runtime
// cannot is free the previous buffer (Drop) -- see the RSS column in the
// results, that difference is the point.
//
// The idiomatic amortized version is string_concat_pushstr.rs.
//
// Build: rustc -O string_concat.rs -o string_concat_rs

fn concat(a: &str, b: &str) -> String {
    let mut s = String::with_capacity(a.len() + b.len());
    s.push_str(a);
    s.push_str(b);
    s
}

fn string_benchmark(iterations: i64) -> String {
    let mut result: String = String::from("Start");
    let mut i: i64 = 0;
    while i < iterations {
        result = concat(&result, " ");
        result = concat(&result, &i.to_string());
        i = i + 1;
    }
    return result;
}

fn main() {
    let iterations: i64 = 20000;
    println!("benchmark: string_concat");
    println!("iterations:");
    println!("{}", iterations);

    let result: String = string_benchmark(iterations);
    let bytes = result.as_bytes();

    println!("length:");
    println!("{}", bytes.len());
    println!("first_char:");
    println!("{}", bytes[0] as i64);
    println!("last_char:");
    println!("{}", bytes[bytes.len() - 1] as i64);
}
