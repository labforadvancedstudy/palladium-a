// Palladium C prelude — runtime helpers emitted by the Rust pdc, extracted so that
// the Palladium-written bootstrap compiler can emit '#include "pd_prelude.h"'
// instead of re-emitting the whole runtime from string literals.
// GENERATED — do not edit by hand. Regenerate with: scripts/gen-prelude.sh
#ifndef PD_PRELUDE_H
#define PD_PRELUDE_H
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <ctype.h>
#include <stdint.h>

// String memory pool to prevent leaks
#define STRING_POOL_SIZE 65536
#define MAX_STRINGS 1024
static char __pd_string_pool[STRING_POOL_SIZE];
static size_t __pd_string_pool_offset = 0;
static char* __pd_allocated_strings[MAX_STRINGS];
static int __pd_num_strings = 0;

static char* __pd_alloc_string(size_t size) {
    if (__pd_string_pool_offset + size > STRING_POOL_SIZE) {
        // Pool exhausted, fall back to malloc
        char* ptr = (char*)malloc(size);
        if (__pd_num_strings < MAX_STRINGS) {
            __pd_allocated_strings[__pd_num_strings++] = ptr;
        }
        return ptr;
    }
    char* ptr = &__pd_string_pool[__pd_string_pool_offset];
    __pd_string_pool_offset += size;
    return ptr;
}

static void __pd_cleanup_strings() {
    for (int i = 0; i < __pd_num_strings; i++) {
        free(__pd_allocated_strings[i]);
    }
    __pd_num_strings = 0;
    __pd_string_pool_offset = 0;
}

static void __pd_init() __attribute__((constructor));
static void __pd_init() {
    atexit(__pd_cleanup_strings);
}

static int __pd_argc = 0;
static char** __pd_argv = 0;

long long __pd_arg_count(void) {
    return (long long)__pd_argc;
}

const char* __pd_arg_at(long long i) {
    if (i < 0 || i >= (long long)__pd_argc) return "";
    return __pd_argv[i];
}

void __pd_print(const char* str) {
    printf("%s\n", str);
}

void __pd_print_int(long long value) {
    printf("%lld\n", value);
}

void __pd_panic(const char* msg) {
    fprintf(stderr, "panic: %s\n", msg);
    abort();
}

long long __pd_string_len(const char* str) {
    return strlen(str);
}

const char* __pd_string_concat(const char* s1, const char* s2) {
    size_t len1 = strlen(s1);
    size_t len2 = strlen(s2);
    char* result = __pd_alloc_string(len1 + len2 + 1);
    strcpy(result, s1);
    strcat(result, s2);
    return result;
}

int __pd_string_eq(const char* s1, const char* s2) {
    return strcmp(s1, s2) == 0;
}

long long __pd_string_char_at(const char* str, long long index) {
    if (index < 0 || index >= (long long)strlen(str)) return -1;
    return (long long)(unsigned char)str[index];
}

const char* __pd_string_substring(const char* str, long long start, long long end) {
    size_t len = strlen(str);
    if (start < 0) start = 0;
    if (end > (long long)len) end = len;
    if (start >= end) return "";
    size_t sub_len = end - start;
    char* result = __pd_alloc_string(sub_len + 1);
    strncpy(result, str + start, sub_len);
    result[sub_len] = '\0';
    return result;
}

const char* __pd_string_from_char(long long c) {
    char* result = __pd_alloc_string(2);
    result[0] = (char)c;
    result[1] = '\0';
    return result;
}

int __pd_char_is_digit(long long c) {
    return isdigit((int)c);
}

int __pd_char_is_alpha(long long c) {
    return isalpha((int)c);
}

int __pd_char_is_whitespace(long long c) {
    return isspace((int)c);
}

long long __pd_string_to_int(const char* str) {
    return atoll(str);
}

const char* __pd_int_to_string(long long n) {
    char* buffer = __pd_alloc_string(32);
    snprintf(buffer, 32, "%lld", n);
    return buffer;
}

// File I/O support
#define MAX_FILES 256
static FILE* __pd_file_handles[MAX_FILES] = {0};
static int __pd_next_handle = 1;

long long __pd_file_open(const char* path) {
    if (__pd_next_handle >= MAX_FILES) return -1;
    FILE* f = fopen(path, "r+");
    if (!f) f = fopen(path, "w+");
    if (!f) return -1;
    int handle = __pd_next_handle++;
    __pd_file_handles[handle] = f;
    return handle;
}

const char* __pd_file_read_all(long long handle) {
    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return "";
    FILE* f = __pd_file_handles[handle];
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* buffer = __pd_alloc_string(size + 1);
    fread(buffer, 1, size, f);
    buffer[size] = '\0';
    return buffer;
}

const char* __pd_file_read_line(long long handle) {
    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return "";
    static char line_buffer[4096];
    FILE* f = __pd_file_handles[handle];
    if (fgets(line_buffer, sizeof(line_buffer), f)) {
        size_t len = strlen(line_buffer);
        if (len > 0 && line_buffer[len-1] == '\n') line_buffer[len-1] = '\0';
        char* result = __pd_alloc_string(len + 1);
        strcpy(result, line_buffer);
        return result;
    }
    return "";
}

int __pd_file_write(long long handle, const char* content) {
    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return 0;
    FILE* f = __pd_file_handles[handle];
    return fputs(content, f) >= 0;
}

int __pd_file_close(long long handle) {
    if (handle < 1 || handle >= MAX_FILES || !__pd_file_handles[handle]) return 0;
    FILE* f = __pd_file_handles[handle];
    __pd_file_handles[handle] = NULL;
    return fclose(f) == 0;
}

int __pd_file_exists(const char* path) {
    FILE* f = fopen(path, "r");
    if (f) {
        fclose(f);
        return 1;
    }
    return 0;
}

// Enhanced I/O runtime functions
// File handle type (opaque pointer)
typedef void* FileHandle;

// File modes
enum FileMode {
    FileMode_Read = 0,
    FileMode_Write = 1,
    FileMode_Append = 2,
    FileMode_ReadWrite = 3
};

// External runtime I/O functions
extern FileHandle pd_file_open(const char* path, size_t path_len, int mode);
extern int pd_file_close(FileHandle handle);
extern int64_t pd_file_read(FileHandle handle, char* buffer, size_t len);
extern int64_t pd_file_write(FileHandle handle, const char* buffer, size_t len);
extern int64_t pd_file_seek(FileHandle handle, uint8_t whence, int64_t offset);
extern int pd_file_flush(FileHandle handle);
extern int pd_path_exists(const char* path, size_t path_len);
extern int pd_path_is_file(const char* path, size_t path_len);
extern int pd_path_is_dir(const char* path, size_t path_len);
extern int pd_create_dir(const char* path, size_t path_len);
extern int pd_create_dir_all(const char* path, size_t path_len);
extern int pd_remove_file(const char* path, size_t path_len);
extern int pd_remove_dir(const char* path, size_t path_len);
extern int pd_remove_dir_all(const char* path, size_t path_len);
extern int pd_read_file_to_string(const char* path, size_t path_len, char** out_str, size_t* out_len);
extern int pd_write_string_to_file(const char* path, size_t path_len, const char* data, size_t data_len);

FileHandle __pd_file_open_ex(const char* path, int mode) {
    return pd_file_open(path, strlen(path), mode);
}

int __pd_file_close_ex(FileHandle handle) {
    return pd_file_close(handle);
}

int64_t __pd_file_read_ex(FileHandle handle, char* buffer, size_t len) {
    return pd_file_read(handle, buffer, len);
}

int64_t __pd_file_write_ex(FileHandle handle, const char* buffer, size_t len) {
    return pd_file_write(handle, buffer, len);
}

int64_t __pd_file_seek(FileHandle handle, uint8_t whence, int64_t offset) {
    return pd_file_seek(handle, whence, offset);
}

int __pd_file_flush(FileHandle handle) {
    return pd_file_flush(handle);
}

int __pd_path_exists(const char* path) {
    return pd_path_exists(path, strlen(path));
}

int __pd_path_is_file(const char* path) {
    return pd_path_is_file(path, strlen(path));
}

int __pd_path_is_dir(const char* path) {
    return pd_path_is_dir(path, strlen(path));
}

int __pd_create_dir(const char* path) {
    return pd_create_dir(path, strlen(path));
}

int __pd_create_dir_all(const char* path) {
    return pd_create_dir_all(path, strlen(path));
}

int __pd_remove_file(const char* path) {
    return pd_remove_file(path, strlen(path));
}

int __pd_remove_dir(const char* path) {
    return pd_remove_dir(path, strlen(path));
}

int __pd_remove_dir_all(const char* path) {
    return pd_remove_dir_all(path, strlen(path));
}

char* __pd_read_file_to_string(const char* path) {
    char* out_str = NULL;
    size_t out_len = 0;
    if (pd_read_file_to_string(path, strlen(path), &out_str, &out_len) == 0) {
        return out_str;
    }
    return "";
}

int __pd_write_string_to_file(const char* path, const char* data) {
    return pd_write_string_to_file(path, strlen(path), data, strlen(data));
}

#endif // PD_PRELUDE_H
