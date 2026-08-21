// Bubble sort benchmark in Rust, WITHOUT bounds checks.
//
// This exists only for fairness: Palladium's C backend emits raw `arr[i]` with
// no bounds check at all, so the safe Rust version in bubble_sort.rs is doing
// strictly more work. This variant uses get_unchecked/get_unchecked_mut so the
// generated code is comparable to Palladium's and C's.
//
// Same algorithm, same workload, byte-identical stdout.
// Build: rustc -O bubble_sort_unchecked.rs -o bubble_sort_rs_unchecked

const N: usize = 45000;

fn bubble_sort(arr: &mut [i64; N], n: i64) {
    let mut i: i64 = 0;
    while i < n - 1 {
        let mut j: i64 = 0;
        while j < n - i - 1 {
            unsafe {
                let a = *arr.get_unchecked(j as usize);
                let b = *arr.get_unchecked((j + 1) as usize);
                if a > b {
                    *arr.get_unchecked_mut(j as usize) = b;
                    *arr.get_unchecked_mut((j + 1) as usize) = a;
                }
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
        unsafe {
            sum = sum + *arr.get_unchecked(i as usize) * (i % 7 + 1);
        }
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
