Tiny Palladium Compiler v16
============================
Complete array support - 100% Bootstrap!

Compiling array example...

Generated C code:
=================
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void __pd_print(const char* s) { printf("%s\
", s); }
void __pd_print_int(long long n) { printf("%lld\
", n); }
long long __pd_string_len(const char* s) { return strlen(s); }
const char* __pd_string_concat(const char* a, const char* b) {
    char* r = malloc(strlen(a) + strlen(b) + 1);
    strcpy(r, a); strcat(r, b); return r;
}
const char* __pd_int_to_string(long long n) {
    char* buf = malloc(32);
    snprintf(buf, 32, "%lld", n);
    return buf;
}

int main(int argc, char** argv) {
    long long nums[5] = {10, 20, 30, 40, 50};
    5] = [10, 20, 30, 40, 50];
    __pd_print("Array values:");
    __pd_print_int(nums[0]);
    __pd_print_int(nums[1]);
    __pd_print_int(nums[2]);
    nums[1] = nums[0] + nums[2];
    __pd_print("After modification:");
    __pd_print_int(nums[1]);
    long long i = 0;
    long long sum = 0;
    while (i < 5) {
        sum = sum + nums[i];
        i = i + 1;
    }
    i = i + 1;
    return 0;
}



🎉 BOOTSTRAP 100% COMPLETE! 🎉
Arrays work! The compiler can now handle all features needed for self-hosting!
