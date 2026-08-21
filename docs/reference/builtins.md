# Builtin function reference

**GENERATED — do not edit by hand.** Regenerate with `python3 scripts/gen-builtin-docs.py` after changing `src/builtins.rs`.

Palladium has 38 builtin functions. They are ordinary free functions: there is no prelude to import and no module path to qualify. They are defined in a single table (`src/builtins.rs`) that the type checker and the borrow checker both derive from, so a builtin cannot exist in one pass and not the other.

Their C implementations are emitted inline into every generated file; the file and path functions are thin wrappers over symbols supplied at link time by `runtime/palladium_runtime.c`.

## Output

Writing to standard output, and aborting.

| Signature | Notes |
|---|---|
| `print(String)` |  |
| `print_int(i64)` |  |
| `panic(String)` | borrows its string argument |

## String manipulation

Strings are immutable handles into an arena. `string_char_at` returns the byte at an index as an integer, which is what the `char_is_*` predicates take.

| Signature | Notes |
|---|---|
| `string_len(String) -> i64` | borrows its string argument |
| `string_concat(String, String) -> String` | borrows its string argument |
| `string_eq(String, String) -> bool` | borrows its string argument |
| `string_char_at(String, i64) -> i64` |  |
| `string_substring(String, i64, i64) -> String` |  |
| `string_from_char(i64) -> String` |  |
| `string_to_int(String) -> i64` | borrows its string argument |
| `int_to_string(i64) -> String` |  |

## Character classification

Predicates over the integer a character position holds.

| Signature | Notes |
|---|---|
| `char_is_digit(i64) -> bool` |  |
| `char_is_alpha(i64) -> bool` |  |
| `char_is_whitespace(i64) -> bool` |  |
| `arg_count() -> i64` |  |
| `arg_at(i64) -> String` |  |

## File I/O

The handle-based API. `file_open` returns an integer handle, or a negative value on failure.

| Signature | Notes |
|---|---|
| `file_open(String) -> i64` | borrows its string argument |
| `file_read_all(i64) -> String` |  |
| `file_read_line(i64) -> String` |  |
| `file_write(i64, String) -> bool` |  |
| `file_close(i64) -> bool` |  |
| `file_exists(String) -> bool` | borrows its string argument |

## Enhanced I/O

| Signature | Notes |
|---|---|
| `path_exists(String) -> bool` | borrows its string argument |
| `path_is_file(String) -> bool` | borrows its string argument |
| `path_is_dir(String) -> bool` | borrows its string argument |
| `create_dir(String) -> i64` | borrows its string argument |
| `create_dir_all(String) -> i64` | borrows its string argument |
| `remove_file(String) -> i64` | borrows its string argument |
| `remove_dir(String) -> i64` | borrows its string argument |
| `remove_dir_all(String) -> i64` | borrows its string argument |
| `read_file_to_string(String) -> String` | borrows its string argument |
| `write_string_to_file(String, String) -> i64` | borrows its string argument |
| `file_flush(i64) -> i64` |  |
| `file_seek(i64, i64, i64) -> i64` |  |

## Enhanced file operations with mode support

| Signature | Notes |
|---|---|
| `file_open_ex(String, i64) -> i64` |  |
| `file_close_ex(i64) -> i64` |  |
| `file_read_ex(i64, String, i64) -> i64` |  |
| `file_write_ex(i64, String, i64) -> i64` |  |

## Notes that bite

- `string_char_at` returns an **integer**, not a character type — there is no `char`.
- `file_write` returns `bool`, not a byte count.
- `vec![x]` is a macro that expands to a **one-element array**, not a growable vector.
- `dbg!(x)` expands to a call to `print_debug`, which is defined nowhere; it always fails.
- `println!` takes exactly one argument.

See the [language specification](../specification/language-spec.md) for the full behaviour of each construct and the [tutorial](../user-guide/tutorial.md) for worked examples.
