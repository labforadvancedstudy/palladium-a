/* Palladium runtime support library
 *
 * Linked into every program produced by `pdc` (see src/driver/mod.rs:261,
 * src/main.rs:97, src/bootstrap/mod.rs:53, src/package/build.rs:366,503).
 *
 * The generated C emits the `__pd_*` wrappers inline and declares the 16
 * `pd_*` symbols below as `extern`; this file is their only definition.
 * Do NOT define any `__pd_*` symbol here - that would be a duplicate symbol.
 *
 * Semantics mirror the Rust reference implementation in src/runtime/io.rs,
 * which is linked into pdc itself but is not available to generated C.
 *
 * Portable C99 (macOS/arm64 + Linux), no external dependencies.
 */

#if !defined(_POSIX_C_SOURCE)
#define _POSIX_C_SOURCE 200809L
#endif
#if defined(__APPLE__) && !defined(_DARWIN_C_SOURCE)
#define _DARWIN_C_SOURCE
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <sys/stat.h>
#include <unistd.h>
#include <dirent.h>
#include <errno.h>

/* Opaque file handle, as declared by the generated C. */
typedef void *FileHandle;

/* File modes, matching FileMode in src/runtime/io.rs. */
#define PD_MODE_READ 0
#define PD_MODE_WRITE 1
#define PD_MODE_APPEND 2
#define PD_MODE_READ_WRITE 3

/* ------------------------------------------------------------------ */
/* Internal helpers                                                     */
/* ------------------------------------------------------------------ */

/* Paths arrive as (ptr, len) and are NOT NUL-terminated from the caller's
 * point of view, so copy them into a NUL-terminated buffer before use.
 * Returns NULL on allocation failure or a NULL pointer argument. */
static char *pd_path_dup(const char *path, size_t path_len)
{
    char *buf;

    if (path == NULL) {
        return NULL;
    }

    buf = (char *)malloc(path_len + 1);
    if (buf == NULL) {
        return NULL;
    }

    if (path_len > 0) {
        memcpy(buf, path, path_len);
    }
    buf[path_len] = '\0';

    return buf;
}

/* stat(2) a (ptr, len) path. Returns 0 and fills `st` on success, -1 else. */
static int pd_path_stat(const char *path, size_t path_len, struct stat *st)
{
    char *p;
    int rc;

    p = pd_path_dup(path, path_len);
    if (p == NULL) {
        return -1;
    }

    rc = stat(p, st);
    free(p);

    return (rc == 0) ? 0 : -1;
}

/* mkdir -p semantics on a mutable NUL-terminated path. 0 ok, -1 err. */
static int pd_mkdir_all(char *buf)
{
    struct stat st;
    size_t i;

    if (buf[0] == '\0') {
        return -1;
    }

    for (i = 0; buf[i] != '\0'; i++) {
        if (buf[i] == '/' && i > 0) {
            buf[i] = '\0';
            if (mkdir(buf, 0777) != 0 && errno != EEXIST) {
                buf[i] = '/';
                return -1;
            }
            buf[i] = '/';
        }
    }

    if (mkdir(buf, 0777) != 0) {
        if (errno != EEXIST) {
            return -1;
        }
        /* Already exists: only an existing directory counts as success. */
        if (stat(buf, &st) != 0 || !S_ISDIR(st.st_mode)) {
            return -1;
        }
    }

    return 0;
}

/* Recursively remove a NUL-terminated path. 0 ok, -1 err.
 * Symlinks are unlinked, never followed (lstat, like fs::remove_dir_all). */
static int pd_remove_tree(const char *path)
{
    struct stat st;
    struct dirent *entry;
    DIR *dir;
    size_t path_len;
    int rc = 0;

    if (lstat(path, &st) != 0) {
        return -1;
    }

    if (!S_ISDIR(st.st_mode)) {
        return (unlink(path) == 0) ? 0 : -1;
    }

    dir = opendir(path);
    if (dir == NULL) {
        return -1;
    }

    path_len = strlen(path);

    while ((entry = readdir(dir)) != NULL) {
        size_t name_len;
        char *child;

        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) {
            continue;
        }

        name_len = strlen(entry->d_name);
        child = (char *)malloc(path_len + name_len + 2);
        if (child == NULL) {
            rc = -1;
            break;
        }

        memcpy(child, path, path_len);
        child[path_len] = '/';
        memcpy(child + path_len + 1, entry->d_name, name_len);
        child[path_len + name_len + 1] = '\0';

        if (pd_remove_tree(child) != 0) {
            rc = -1;
        }
        free(child);

        if (rc != 0) {
            break;
        }
    }

    closedir(dir);

    if (rc != 0) {
        return -1;
    }

    return (rmdir(path) == 0) ? 0 : -1;
}

/* ------------------------------------------------------------------ */
/* File operations                                                      */
/* ------------------------------------------------------------------ */

FileHandle pd_file_open(const char *path, size_t path_len, int mode)
{
    const char *fmode;
    char *p;
    FILE *f;

    switch (mode) {
    case PD_MODE_READ:
        fmode = "r";
        break;
    case PD_MODE_WRITE:
        fmode = "w";
        break;
    case PD_MODE_APPEND:
        fmode = "a";
        break;
    case PD_MODE_READ_WRITE:
        fmode = "r+";
        break;
    default:
        return NULL;
    }

    p = pd_path_dup(path, path_len);
    if (p == NULL) {
        return NULL;
    }

    f = fopen(p, fmode);
    free(p);

    return (FileHandle)f;
}

int pd_file_close(FileHandle handle)
{
    if (handle == NULL) {
        return -1;
    }

    fclose((FILE *)handle);
    return 0;
}

int64_t pd_file_read(FileHandle handle, char *buffer, size_t len)
{
    FILE *f = (FILE *)handle;
    size_t n;

    if (f == NULL || buffer == NULL) {
        return -1;
    }

    if (len == 0) {
        return 0;
    }

    n = fread(buffer, 1, len, f);
    if (n == 0 && ferror(f)) {
        return -1;
    }

    return (int64_t)n;
}

int64_t pd_file_write(FileHandle handle, const char *buffer, size_t len)
{
    FILE *f = (FILE *)handle;
    size_t n;

    if (f == NULL || buffer == NULL) {
        return -1;
    }

    if (len == 0) {
        return 0;
    }

    n = fwrite(buffer, 1, len, f);
    if (n == 0 && ferror(f)) {
        return -1;
    }

    return (int64_t)n;
}

int64_t pd_file_seek(FileHandle handle, uint8_t whence, int64_t offset)
{
    FILE *f = (FILE *)handle;
    off_t pos;
    int c_whence;

    if (f == NULL) {
        return -1;
    }

    switch (whence) {
    case 0:
        c_whence = SEEK_SET;
        break;
    case 1:
        c_whence = SEEK_CUR;
        break;
    case 2:
        c_whence = SEEK_END;
        break;
    default:
        return -1;
    }

    if (fseeko(f, (off_t)offset, c_whence) != 0) {
        return -1;
    }

    pos = ftello(f);
    if (pos < 0) {
        return -1;
    }

    return (int64_t)pos;
}

int pd_file_flush(FileHandle handle)
{
    FILE *f = (FILE *)handle;

    if (f == NULL) {
        return -1;
    }

    return (fflush(f) == 0) ? 0 : -1;
}

/* ------------------------------------------------------------------ */
/* Path queries                                                         */
/* ------------------------------------------------------------------ */

int pd_path_exists(const char *path, size_t path_len)
{
    struct stat st;

    return (pd_path_stat(path, path_len, &st) == 0) ? 1 : 0;
}

int pd_path_is_file(const char *path, size_t path_len)
{
    struct stat st;

    if (pd_path_stat(path, path_len, &st) != 0) {
        return 0;
    }

    return S_ISREG(st.st_mode) ? 1 : 0;
}

int pd_path_is_dir(const char *path, size_t path_len)
{
    struct stat st;

    if (pd_path_stat(path, path_len, &st) != 0) {
        return 0;
    }

    return S_ISDIR(st.st_mode) ? 1 : 0;
}

/* ------------------------------------------------------------------ */
/* Directory / file mutation                                            */
/* ------------------------------------------------------------------ */

int pd_create_dir(const char *path, size_t path_len)
{
    char *p;
    int rc;

    p = pd_path_dup(path, path_len);
    if (p == NULL) {
        return -1;
    }

    rc = mkdir(p, 0777);
    free(p);

    return (rc == 0) ? 0 : -1;
}

int pd_create_dir_all(const char *path, size_t path_len)
{
    char *p;
    int rc;

    p = pd_path_dup(path, path_len);
    if (p == NULL) {
        return -1;
    }

    rc = pd_mkdir_all(p);
    free(p);

    return rc;
}

int pd_remove_file(const char *path, size_t path_len)
{
    char *p;
    int rc;

    p = pd_path_dup(path, path_len);
    if (p == NULL) {
        return -1;
    }

    rc = unlink(p);
    free(p);

    return (rc == 0) ? 0 : -1;
}

int pd_remove_dir(const char *path, size_t path_len)
{
    char *p;
    int rc;

    p = pd_path_dup(path, path_len);
    if (p == NULL) {
        return -1;
    }

    rc = rmdir(p);
    free(p);

    return (rc == 0) ? 0 : -1;
}

int pd_remove_dir_all(const char *path, size_t path_len)
{
    char *p;
    int rc;

    p = pd_path_dup(path, path_len);
    if (p == NULL) {
        return -1;
    }

    rc = pd_remove_tree(p);
    free(p);

    return rc;
}

/* ------------------------------------------------------------------ */
/* Whole-file convenience                                               */
/* ------------------------------------------------------------------ */

/* Reads the whole file into a malloc'd buffer. *out_len is the byte count,
 * NOT counting the terminator; the buffer is NUL-terminated regardless
 * because the generated wrapper `__pd_read_file_to_string` returns it as a
 * plain C string. The caller frees it with free(). */
int pd_read_file_to_string(const char *path, size_t path_len, char **out_str, size_t *out_len)
{
    char *p;
    FILE *f;
    char *buf;
    char *shrunk;
    size_t cap = 4096;
    size_t len = 0;

    if (out_str == NULL || out_len == NULL) {
        return -1;
    }

    p = pd_path_dup(path, path_len);
    if (p == NULL) {
        return -1;
    }

    f = fopen(p, "rb");
    free(p);
    if (f == NULL) {
        return -1;
    }

    buf = (char *)malloc(cap);
    if (buf == NULL) {
        fclose(f);
        return -1;
    }

    for (;;) {
        size_t space = cap - len - 1; /* reserve one byte for the terminator */
        size_t n;

        if (space == 0) {
            size_t new_cap = cap * 2;
            char *new_buf = (char *)realloc(buf, new_cap);
            if (new_buf == NULL) {
                free(buf);
                fclose(f);
                return -1;
            }
            buf = new_buf;
            cap = new_cap;
            space = cap - len - 1;
        }

        n = fread(buf + len, 1, space, f);
        len += n;
        if (n < space) {
            break; /* EOF or error */
        }
    }

    if (ferror(f)) {
        free(buf);
        fclose(f);
        return -1;
    }
    fclose(f);

    buf[len] = '\0';

    shrunk = (char *)realloc(buf, len + 1);
    if (shrunk != NULL) {
        buf = shrunk;
    }

    *out_str = buf;
    *out_len = len;

    return 0;
}

int pd_write_string_to_file(const char *path, size_t path_len, const char *data, size_t data_len)
{
    char *p;
    FILE *f;

    if (data == NULL && data_len > 0) {
        return -1;
    }

    p = pd_path_dup(path, path_len);
    if (p == NULL) {
        return -1;
    }

    f = fopen(p, "wb");
    free(p);
    if (f == NULL) {
        return -1;
    }

    if (data_len > 0 && fwrite(data, 1, data_len, f) != data_len) {
        fclose(f);
        return -1;
    }

    return (fclose(f) == 0) ? 0 : -1;
}
