/* Fibonacci benchmark in C -- reference implementation for
 * benchmarks/palladium/fibonacci.pd. Same algorithm, same workload,
 * byte-identical stdout.
 *
 * Build: gcc -O2 fibonacci.c -o fibonacci_c
 */
#include <stdio.h>

/* NOTE on linkage: the benchmark functions below are deliberately NOT `static`.
 * Palladium's codegen emits every user function with external linkage, and
 * marking these `static` lets clang specialise/inline them into main, which was
 * measured to be worth 4.9% on matrix_multiply (318.2ms static vs 334.7ms
 * extern, min of 12 interleaved runs). Matching Palladium's linkage keeps the
 * comparison about code generation rather than about one C keyword.
 */

long long fibonacci(long long n) {
    if (n <= 1) {
        return n;
    }
    return fibonacci(n - 1) + fibonacci(n - 2);
}

int main(void) {
    long long n = 42;
    printf("benchmark: fibonacci\n");
    printf("n:\n");
    printf("%lld\n", n);
    long long result = fibonacci(n);
    printf("result:\n");
    printf("%lld\n", result);
    return 0;
}
