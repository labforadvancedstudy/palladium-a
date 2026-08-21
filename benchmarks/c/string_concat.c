/* String concatenation benchmark in C -- reference implementation for
 * benchmarks/palladium/string_concat.pd. Same algorithm, same workload,
 * byte-identical stdout.
 *
 * Replicates Palladium's `string_concat` semantics: every concat mallocs a
 * fresh buffer of len(a)+len(b)+1 and copies both operands into it. Total work
 * is quadratic, exactly as in the Palladium version.
 *
 * Difference to be aware of when reading the numbers: this version FREES the
 * previous buffer. Palladium's runtime cannot -- __pd_alloc_string bump-
 * allocates from a 64KB pool and then falls back to malloc, and nothing is
 * released until atexit (and only the first 1024 pointers are even tracked).
 * That is why the Palladium RSS column is ~2.3GB and this one is not.
 *
 * Build: gcc -O2 string_concat.c -o string_concat_c
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* NOTE on linkage: the benchmark functions below are deliberately NOT `static`.
 * Palladium's codegen emits every user function with external linkage, and
 * marking these `static` lets clang specialise/inline them into main, which was
 * measured to be worth 4.9% on matrix_multiply (318.2ms static vs 334.7ms
 * extern, min of 12 interleaved runs). Matching Palladium's linkage keeps the
 * comparison about code generation rather than about one C keyword.
 */

char *concat(const char *a, const char *b) {
    size_t la = strlen(a);
    size_t lb = strlen(b);
    char *s = (char *)malloc(la + lb + 1);
    if (!s) {
        fprintf(stderr, "out of memory\n");
        exit(1);
    }
    memcpy(s, a, la);
    memcpy(s + la, b, lb);
    s[la + lb] = '\0';
    return s;
}

char *string_benchmark(long long iterations) {
    char *result = concat("Start", "");
    long long i = 0;
    while (i < iterations) {
        char *prev = result;
        result = concat(result, " ");
        free(prev);

        char num[32];
        snprintf(num, sizeof(num), "%lld", i);
        prev = result;
        result = concat(result, num);
        free(prev);

        i = i + 1;
    }
    return result;
}

int main(void) {
    long long iterations = 20000;
    printf("benchmark: string_concat\n");
    printf("iterations:\n");
    printf("%lld\n", iterations);

    char *result = string_benchmark(iterations);
    size_t len = strlen(result);

    printf("length:\n");
    printf("%zu\n", len);
    printf("first_char:\n");
    printf("%d\n", (int)(unsigned char)result[0]);
    printf("last_char:\n");
    printf("%d\n", (int)(unsigned char)result[len - 1]);

    free(result);
    return 0;
}
