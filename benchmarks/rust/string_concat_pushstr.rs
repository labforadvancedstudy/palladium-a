// String concatenation benchmark in Rust -- IDIOMATIC variant.
//
// Same observable result as string_concat.rs, but appends in place with
// push_str, so the buffer grows amortized O(1) instead of being reallocated and
// fully copied on every concat. This is what a Rust programmer would actually
// write, and it is a different algorithm from the Palladium version -- it is
// reported separately for exactly that reason. Comparing this number against
// Palladium is NOT apples-to-apples; comparing string_concat.rs is.
//
// Build: rustc -O string_concat_pushstr.rs -o string_concat_rs_pushstr

fn string_benchmark(iterations: i64) -> String {
    let mut result: String = String::from("Start");
    let mut i: i64 = 0;
    while i < iterations {
        result.push_str(" ");
        result.push_str(&i.to_string());
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
