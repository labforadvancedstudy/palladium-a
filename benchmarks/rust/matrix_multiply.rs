// Matrix multiplication benchmark in Rust -- reference implementation for
// benchmarks/palladium/matrix_multiply.pd. Same algorithm, same workload,
// byte-identical stdout.
//
// Fixed-size stack arrays (NOT Vec) so the memory model matches Palladium's
// `[i64; N]`. Indexing here is bounds-checked; the unchecked counterpart is
// matrix_multiply_unchecked.rs.
//
// Build: rustc -O matrix_multiply.rs -o matrix_multiply_rs

const NN: usize = 40000; // 200 * 200

fn matmul(a: &[i64; NN], b: &[i64; NN], c: &mut [i64; NN], n: i64) {
    let mut i: i64 = 0;
    while i < n {
        let mut j: i64 = 0;
        while j < n {
            let mut sum: i64 = 0;
            let mut k: i64 = 0;
            while k < n {
                sum = sum + a[(i * n + k) as usize] * b[(k * n + j) as usize];
                k = k + 1;
            }
            c[(i * n + j) as usize] = sum;
            j = j + 1;
        }
        i = i + 1;
    }
}

fn main() {
    let n: i64 = 200;
    let reps: i64 = 200;
    println!("benchmark: matrix_multiply");
    println!("n:");
    println!("{}", n);
    println!("reps:");
    println!("{}", reps);

    let mut a: [i64; NN] = [0; NN];
    let mut b: [i64; NN] = [0; NN];
    let mut c: [i64; NN] = [0; NN];

    let mut i: i64 = 0;
    while i < n {
        let mut j: i64 = 0;
        while j < n {
            a[(i * n + j) as usize] = (i + j) % 7;
            b[(i * n + j) as usize] = (i + 2 * j) % 5;
            j = j + 1;
        }
        i = i + 1;
    }

    let mut acc: i64 = 0;
    let mut r: i64 = 0;
    while r < reps {
        a[r as usize] = a[r as usize] + 1;
        matmul(&a, &b, &mut c, n);
        acc = acc + c[r as usize];
        r = r + 1;
    }

    println!("c[0]:");
    println!("{}", c[0]);
    println!("c[39999]:");
    println!("{}", c[39999]);
    println!("acc:");
    println!("{}", acc);
}
