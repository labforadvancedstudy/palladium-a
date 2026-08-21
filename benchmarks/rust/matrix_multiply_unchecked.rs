// Matrix multiplication benchmark in Rust, WITHOUT bounds checks.
//
// Fairness variant: Palladium's C backend emits raw `arr[i]` with no bounds
// check, so matrix_multiply.rs is doing strictly more work in the hot loop.
// This variant uses get_unchecked so the generated code is comparable.
//
// Same algorithm, same workload, byte-identical stdout.
// Build: rustc -O matrix_multiply_unchecked.rs -o matrix_multiply_rs_unchecked

const NN: usize = 40000; // 200 * 200

fn matmul(a: &[i64; NN], b: &[i64; NN], c: &mut [i64; NN], n: i64) {
    let mut i: i64 = 0;
    while i < n {
        let mut j: i64 = 0;
        while j < n {
            let mut sum: i64 = 0;
            let mut k: i64 = 0;
            while k < n {
                unsafe {
                    sum = sum
                        + *a.get_unchecked((i * n + k) as usize)
                            * *b.get_unchecked((k * n + j) as usize);
                }
                k = k + 1;
            }
            unsafe {
                *c.get_unchecked_mut((i * n + j) as usize) = sum;
            }
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
