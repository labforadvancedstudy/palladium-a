/* Matrix multiplication benchmark in C -- reference implementation for
 * benchmarks/palladium/matrix_multiply.pd. Same algorithm, same workload,
 * byte-identical stdout.
 *
 * Build: gcc -O2 matrix_multiply.c -o matrix_multiply_c
 */
#include <stdio.h>

/* NOTE on linkage: the benchmark functions below are deliberately NOT `static`.
 * Palladium's codegen emits every user function with external linkage, and
 * marking these `static` lets clang specialise/inline them into main, which was
 * measured to be worth 4.9% on matrix_multiply (318.2ms static vs 334.7ms
 * extern, min of 12 interleaved runs). Matching Palladium's linkage keeps the
 * comparison about code generation rather than about one C keyword.
 */

#define NN 40000 /* 200 * 200 */

void matmul(long long *a, long long *b, long long *c, long long n) {
    long long i = 0;
    while (i < n) {
        long long j = 0;
        while (j < n) {
            long long sum = 0;
            long long k = 0;
            while (k < n) {
                sum = sum + a[i * n + k] * b[k * n + j];
                k = k + 1;
            }
            c[i * n + j] = sum;
            j = j + 1;
        }
        i = i + 1;
    }
}

int main(void) {
    long long n = 200;
    long long reps = 200;
    printf("benchmark: matrix_multiply\n");
    printf("n:\n");
    printf("%lld\n", n);
    printf("reps:\n");
    printf("%lld\n", reps);

    /* stack locals with zero initializers, matching what Palladium's codegen
     * emits for `let mut a: [i64; 40000] = [0; 40000];` */
    long long a[NN] = {0};
    long long b[NN] = {0};
    long long c[NN] = {0};

    long long i = 0;
    while (i < n) {
        long long j = 0;
        while (j < n) {
            a[i * n + j] = (i + j) % 7;
            b[i * n + j] = (i + 2 * j) % 5;
            j = j + 1;
        }
        i = i + 1;
    }

    long long acc = 0;
    long long r = 0;
    while (r < reps) {
        a[r] = a[r] + 1;
        matmul(a, b, c, n);
        acc = acc + c[r];
        r = r + 1;
    }

    printf("c[0]:\n");
    printf("%lld\n", c[0]);
    printf("c[39999]:\n");
    printf("%lld\n", c[39999]);
    printf("acc:\n");
    printf("%lld\n", acc);
    return 0;
}
