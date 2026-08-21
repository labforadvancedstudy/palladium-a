/* Bubble sort benchmark in C -- reference implementation for
 * benchmarks/palladium/bubble_sort.pd. Same algorithm, same workload,
 * byte-identical stdout.
 *
 * Build: gcc -O2 bubble_sort.c -o bubble_sort_c
 */
#include <stdio.h>

/* NOTE on linkage: the benchmark functions below are deliberately NOT `static`.
 * Palladium's codegen emits every user function with external linkage, and
 * marking these `static` lets clang specialise/inline them into main, which was
 * measured to be worth 4.9% on matrix_multiply (318.2ms static vs 334.7ms
 * extern, min of 12 interleaved runs). Matching Palladium's linkage keeps the
 * comparison about code generation rather than about one C keyword.
 */

#define N 45000

void bubble_sort(long long *arr, long long n) {
    long long i = 0;
    while (i < n - 1) {
        long long j = 0;
        while (j < n - i - 1) {
            if (arr[j] > arr[j + 1]) {
                long long temp = arr[j];
                arr[j] = arr[j + 1];
                arr[j + 1] = temp;
            }
            j = j + 1;
        }
        i = i + 1;
    }
}

long long checksum(long long *arr, long long n) {
    long long sum = 0;
    long long i = 0;
    while (i < n) {
        sum = sum + arr[i] * (i % 7 + 1);
        i = i + 1;
    }
    return sum;
}

int main(void) {
    long long n = N;
    printf("benchmark: bubble_sort\n");
    printf("n:\n");
    printf("%lld\n", n);

    /* stack local with a zero initializer, matching what Palladium's codegen
     * emits for `let mut arr: [i64; 45000] = [0; 45000];` */
    long long arr[N] = {0};
    long long i = 0;
    while (i < n) {
        arr[i] = n - i;
        i = i + 1;
    }

    bubble_sort(arr, n);

    printf("first:\n");
    printf("%lld\n", arr[0]);
    printf("last:\n");
    printf("%lld\n", arr[44999]);
    printf("checksum:\n");
    printf("%lld\n", checksum(arr, n));
    return 0;
}
