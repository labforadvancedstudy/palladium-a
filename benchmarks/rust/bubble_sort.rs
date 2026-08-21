// Bubble sort benchmark in Rust -- reference implementation for
// benchmarks/palladium/bubble_sort.pd. Same algorithm, same workload,
// byte-identical stdout.
//
// Fixed-size stack array (NOT Vec) so the memory model matches Palladium's
// `[i64; N]`. Indexing here is bounds-checked; the unchecked counterpart is
// bubble_sort_unchecked.rs.
//
// Build: rustc -O bubble_sort.rs -o bubble_sort_rs

const N: usize = 45000;

fn bubble_sort(arr: &mut [i64; N], n: i64) {
    let mut i: i64 = 0;
    while i < n - 1 {
        let mut j: i64 = 0;
        while j < n - i - 1 {
            if arr[j as usize] > arr[(j + 1) as usize] {
                let temp: i64 = arr[j as usize];
                arr[j as usize] = arr[(j + 1) as usize];
                arr[(j + 1) as usize] = temp;
            }
            j = j + 1;
        }
        i = i + 1;
    }
}

fn checksum(arr: &[i64; N], n: i64) -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        sum = sum + arr[i as usize] * (i % 7 + 1);
        i = i + 1;
    }
    return sum;
}

fn main() {
    let n: i64 = 45000;
    println!("benchmark: bubble_sort");
    println!("n:");
    println!("{}", n);

    let mut arr: [i64; N] = [0; N];
    let mut i: i64 = 0;
    while i < n {
        arr[i as usize] = n - i;
        i = i + 1;
    }

    bubble_sort(&mut arr, n);

    println!("first:");
    println!("{}", arr[0]);
    println!("last:");
    println!("{}", arr[44999]);
    println!("checksum:");
    println!("{}", checksum(&arr, n));
}
