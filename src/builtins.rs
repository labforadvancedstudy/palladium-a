// Built-in function registry for Palladium
// "One table to rule them all"
//
// This module is the SINGLE SOURCE OF TRUTH for the compiler's built-in functions.
// Every pass that needs to know about built-ins (type checker, borrow checker,
// effect analyzer, LSP) must derive its own tables from `BUILTINS` instead of
// hand-maintaining a copy.
//
// History, part 1 (type checker / borrow checker): the type checker knew 36
// built-ins while the borrow checker only knew 25, so 11 built-ins (string_len,
// string_eq, string_char_at, string_from_char, char_is_digit, char_is_alpha,
// char_is_whitespace, file_read_all, file_read_line, file_write, panic)
// type-checked fine but were rejected by the borrow checker with "Use of
// uninitialized value".
//
// History, part 2 (effect analyzer / LSP): three more hand-written copies survived
// that first consolidation, and all three had drifted:
//
//   consumer            entries  missing from it
//   effects/mod.rs      19       19 (panic, arg_count, arg_at, every path_*/dir/
//                                remove_* built-in, read_file_to_string,
//                                write_string_to_file, file_flush, file_seek and
//                                the four *_ex built-ins)
//   lsp/completion.rs    6       32
//   lsp/hover.rs         6       32
//
// The effects gap was the dangerous one: `EffectAnalyzer` treats a built-in with no
// entry as contributing no effect (effects/mod.rs, `Expr::Call` arm), so 19 built-ins
// — 18 of which do real file or console I/O — were invisible to effect analysis.
//
// The tests at the bottom of this file make that class of drift impossible to
// reintroduce: every consumer's view is compared against `BUILTINS` by set equality.
//
// History, part 3 (the table stopped being a second definition of the language).
// This table held 38 names against N14's normative 34; the four extra were
// `file_open_ex`, `file_close_ex`, `file_read_ex` and `file_write_ex`, none of
// them callable and none of them reachable from any `.pd` file in the tree. A
// registry that carries names the specification does not define is a second
// definition of the builtin surface, and it can only be seen by someone reading
// both documents. The four are gone (see the note where they used to sit), and
// the set is now pinned against N14 itself — the specification, not a count —
// by `test_registry_is_exactly_the_normative_builtin_set`.

/// Type of a built-in parameter or return value.
///
/// Built-ins only use the primitive surface of the language, so this is a small
/// closed set rather than the full `Type`/`CheckerType` lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinType {
    /// 64-bit signed integer (`i64`)
    I64,
    /// Owned string
    Str,
    /// Boolean
    Bool,
    /// One Unicode scalar (`char`, N4-04/N14-04).
    ///
    /// The registry had no way to say this, so all five character built-ins
    /// were declared over `I64` while N14 described them over `char` — the
    /// specification and the implementation disagreeing in the one file that
    /// exists to stop them disagreeing.
    Char,
    /// No value (`()`), return position only
    Unit,
}

/// How a built-in takes one of its parameters, for ownership analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    /// Parameter is Copy (no ownership transfer)
    Copy,
    /// Parameter borrows immutably for the duration of the call
    Borrow,
    /// Parameter takes ownership
    Move,
}

/// How a built-in returns its value, for ownership analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnMode {
    /// Returns an owned value (caller must manage it)
    Owned,
    /// Returns a pointer into storage the built-in did not allocate and that
    /// outlives the call — the caller must never free it.
    ///
    /// `arg_at` is the case that forced this variant to exist: it hands back a
    /// pointer into `argv`, or a literal `""`. Calling that `Owned` made the
    /// canonical metadata false, and `ReturnMode` is what the ownership pass
    /// derives its signatures from; a comment could not fix that.
    BorrowedStatic,
    /// Returns a copy value (primitives)
    Copy,
    /// No return value
    Unit,
}

/// One parameter of a built-in: its name, its type and its ownership mode.
///
/// The name carries no semantics for any compiler pass; it exists so that the LSP
/// can render `fn print(s: String)` instead of `fn print(String)` without a
/// hand-written signature string. These are the user-visible Palladium-level names,
/// so where the C implementation in `src/codegen/mod.rs` is less readable the
/// Palladium name wins: `string_len(s)` here vs `__pd_string_len(str)` there,
/// `string_concat(a, b)` vs `__pd_string_concat(str1, str2)`. They coincide where
/// the C name was already the clearer one (`file_seek(handle, whence, offset)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinParam {
    pub name: &'static str,
    pub ty: BuiltinType,
    pub mode: ParamMode,
}

/// Shorthand for building a `BuiltinParam` in the const table below.
const fn p(name: &'static str, ty: BuiltinType, mode: ParamMode) -> BuiltinParam {
    BuiltinParam { name, ty, mode }
}

/// Whether a call to a built-in can actually be compiled.
///
/// The registry describes more built-ins than the runtime can currently honour.
/// Recording that as data — rather than leaving the built-in advertised and
/// letting it fail in gcc, or deleting it and losing the description — lets the
/// type checker refuse the call with a reason and the LSP stop offering it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// A call compiles, links and runs.
    Callable,
    /// Registered and described, but a call cannot be compiled. The string says
    /// why, and is shown to the user by the type checker and by LSP hover.
    Unsupported(&'static str),
}

impl Support {
    pub fn is_callable(self) -> bool {
        matches!(self, Support::Callable)
    }

    /// Why this built-in cannot be called, if it cannot.
    pub fn reason(self) -> Option<&'static str> {
        match self {
            Support::Callable => None,
            Support::Unsupported(reason) => Some(reason),
        }
    }
}

/// A built-in function: name, parameter list, return type, return ownership,
/// effects, support status and documentation.
#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub name: &'static str,
    pub params: &'static [BuiltinParam],
    pub ret: BuiltinType,
    pub ret_mode: ReturnMode,
    /// Whether a call to this built-in can be compiled today. Unsupported
    /// built-ins are still registered — every pass and the seam test keep
    /// describing them — but the type checker rejects calls to them.
    pub support: Support,
    /// Effects this built-in contributes to its caller. An empty slice means pure:
    /// `EffectSet::is_pure()` is true for the empty set.
    pub effects: &'static [Effect],
    /// One-line description, shown by LSP hover and completion. This is the only
    /// piece of LSP metadata that cannot be derived from the typed data above.
    pub doc: &'static str,
}

impl BuiltinType {
    /// How this type is spelled in Palladium source (LSP-facing).
    pub fn display(self) -> &'static str {
        match self {
            BuiltinType::I64 => "i64",
            BuiltinType::Str => "String",
            BuiltinType::Bool => "bool",
            BuiltinType::Char => "char",
            BuiltinType::Unit => "()",
        }
    }
}

impl Builtin {
    /// The Palladium signature of this built-in, derived from its typed parameter
    /// and return data — never hand-written.
    ///
    /// A `Unit` return is rendered by omitting the arrow, matching how the language
    /// spells a function with no return value.
    pub fn signature(&self) -> String {
        let params = self
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ty.display()))
            .collect::<Vec<_>>()
            .join(", ");
        match self.ret {
            BuiltinType::Unit => format!("fn {}({})", self.name, params),
            ret => format!("fn {}({}) -> {}", self.name, params, ret.display()),
        }
    }
}

use crate::effects::Effect;
use BuiltinType::{Bool, Char, I64, Str, Unit};
use ParamMode::{Borrow, Copy as ByCopy};

/// No effects: calling this built-in leaves the outside world alone.
const PURE: &[Effect] = &[];
/// Reads or writes the outside world (file system, console, process arguments).
const IO: &[Effect] = &[Effect::IO];
/// Writes to the outside world *and* aborts the process.
const IO_PANIC: &[Effect] = &[Effect::IO, Effect::Panic];
/// Allocates the string it returns; the caller inherits that storage.
const MEMORY: &[Effect] = &[Effect::Memory];
/// Touches the outside world *and* allocates the string it returns.
const IO_MEMORY: &[Effect] = &[Effect::IO, Effect::Memory];

/// The canonical list of every built-in function the compiler knows about.
///
/// Adding a built-in here automatically registers it in the type checker, the
/// borrow checker, the effect analyzer and the LSP. Adding one anywhere else is a
/// bug the tests below will catch.
///
/// Effect classification follows the implementation emitted by `src/codegen/mod.rs`
/// and `runtime/palladium_runtime.c`: anything that touches a file, the console or
/// the process argument vector is `Effect::IO`, and anything that allocates the
/// value it hands back is `Effect::Memory`.
///
/// The seven allocating built-ins are string_concat, string_substring,
/// string_from_char, int_to_string, file_read_all, file_read_line (all six call
/// `__pd_alloc_string` in the emitted prelude) and read_file_to_string (mallocs one
/// layer down, at runtime/palladium_runtime.c:470). `Effect::Memory` had been
/// defined but never attributed to anything ("For now, we don't have explicit
/// allocation functions" — the deleted table in effects/mod.rs); an effect no value
/// can ever carry is a claim about the model that the model does not honour, so it
/// is attributed here rather than left decorative.
///
/// `arg_at` allocates nothing — it returns a pointer into `argv` (or a literal
/// "") — so it is `ReturnMode::BorrowedStatic`, not `Owned`. Owned is therefore
/// exactly the set that allocates, and `Owned` and `Memory` agree on every row.
pub const BUILTINS: &[Builtin] = &[
    // ---- Output ----
    Builtin {
        name: "print",
        // NOTE: `print` deliberately treats its String argument as Copy (historic
        // behavior of the borrow checker); it neither moves nor borrows the value.
        params: &[p("s", Str, ByCopy)],
        ret: Unit,
        ret_mode: ReturnMode::Unit,
        support: Support::Callable,
        effects: IO,
        doc: "Print a string to stdout",
    },
    Builtin {
        name: "print_int",
        params: &[p("n", I64, ByCopy)],
        ret: Unit,
        ret_mode: ReturnMode::Unit,
        support: Support::Callable,
        effects: IO,
        doc: "Print an integer to stdout",
    },
    Builtin {
        name: "panic",
        params: &[p("msg", Str, Borrow)],
        ret: Unit,
        ret_mode: ReturnMode::Unit,
        support: Support::Callable,
        // `__pd_panic` writes the message to stderr and then calls abort()
        // (src/codegen/mod.rs), so it is both console I/O and a panic.
        effects: IO_PANIC,
        doc: "Print a message to stderr and abort the process",
    },
    // ---- String manipulation ----
    Builtin {
        name: "string_len",
        params: &[p("s", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: PURE,
        doc: "Get the length of a string",
    },
    Builtin {
        name: "string_concat",
        params: &[p("a", Str, Borrow), p("b", Str, Borrow)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        support: Support::Callable,
        effects: MEMORY,
        doc: "Concatenate two strings",
    },
    Builtin {
        name: "string_eq",
        params: &[p("s1", Str, Borrow), p("s2", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: PURE,
        doc: "Compare two strings for equality",
    },
    Builtin {
        name: "string_char_at",
        params: &[p("s", Str, Borrow), p("index", I64, ByCopy)],
        ret: Char,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: PURE,
        doc: "Get the character code at a byte index",
    },
    Builtin {
        name: "string_substring",
        params: &[
            p("s", Str, Borrow),
            p("start", I64, ByCopy),
            p("end", I64, ByCopy),
        ],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        support: Support::Callable,
        effects: MEMORY,
        doc: "Extract the substring in [start, end)",
    },
    Builtin {
        name: "string_from_char",
        params: &[p("c", Char, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        support: Support::Callable,
        effects: MEMORY,
        doc: "Build a one-character string from a character code",
    },
    Builtin {
        name: "string_to_int",
        params: &[p("s", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: PURE,
        doc: "Parse an integer from a string",
    },
    Builtin {
        name: "int_to_string",
        params: &[p("n", I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        support: Support::Callable,
        effects: MEMORY,
        doc: "Convert an integer to a string",
    },
    // ---- Character classification ----
    Builtin {
        name: "char_is_digit",
        params: &[p("c", Char, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: PURE,
        doc: "Is this character code a decimal digit?",
    },
    Builtin {
        name: "char_is_alpha",
        params: &[p("c", Char, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: PURE,
        doc: "Is this character code a letter?",
    },
    Builtin {
        name: "char_is_whitespace",
        params: &[p("c", Char, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: PURE,
        doc: "Is this character code whitespace?",
    },
    // ---- Command-line arguments ----
    Builtin {
        // Number of command-line arguments, argv[0] included (matches C's argc).
        name: "arg_count",
        params: &[],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        // Reads the process argument vector captured by main(): program input from
        // outside, so it is not pure.
        effects: IO,
        doc: "Number of command-line arguments, including argv[0]",
    },
    Builtin {
        // Argument `i`; returns "" when out of range, never NULL, because every
        // string built-in assumes a non-NULL `const char*`.
        name: "arg_at",
        params: &[p("i", I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::BorrowedStatic,
        support: Support::Callable,
        effects: IO,
        doc: "Command-line argument `i`, or \"\" when out of range",
    },
    // ---- File I/O ----
    Builtin {
        name: "file_open",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Open a file and return a handle",
    },
    Builtin {
        name: "file_read_all",
        params: &[p("handle", I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        support: Support::Callable,
        effects: IO_MEMORY,
        doc: "Read a whole open file into a string",
    },
    Builtin {
        name: "file_read_line",
        params: &[p("handle", I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        support: Support::Callable,
        effects: IO_MEMORY,
        doc: "Read one line from an open file",
    },
    Builtin {
        name: "file_write",
        params: &[p("handle", I64, ByCopy), p("content", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Write a string to an open file",
    },
    Builtin {
        name: "file_close",
        params: &[p("handle", I64, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Close an open file handle",
    },
    Builtin {
        name: "file_exists",
        params: &[p("path", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Does this file exist?",
    },
    // ---- Enhanced I/O ----
    Builtin {
        name: "path_exists",
        params: &[p("path", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Does this path exist?",
    },
    Builtin {
        name: "path_is_file",
        params: &[p("path", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Is this path a regular file?",
    },
    Builtin {
        name: "path_is_dir",
        params: &[p("path", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Is this path a directory?",
    },
    Builtin {
        name: "create_dir",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Create a directory",
    },
    Builtin {
        name: "create_dir_all",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Create a directory and every missing parent",
    },
    Builtin {
        name: "remove_file",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Delete a file",
    },
    Builtin {
        name: "remove_dir",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Delete an empty directory",
    },
    Builtin {
        name: "remove_dir_all",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Delete a directory and its contents",
    },
    Builtin {
        name: "read_file_to_string",
        params: &[p("path", Str, Borrow)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        support: Support::Callable,
        effects: IO_MEMORY,
        doc: "Read a file at `path` into a string",
    },
    Builtin {
        name: "write_string_to_file",
        params: &[p("path", Str, Borrow), p("data", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Write a string to the file at `path`",
    },
    // RE-BASED 2026-08-23, and now callable. Both were `Support::Unsupported`
    // because their C wrappers took the enhanced file API's opaque `FileHandle`
    // (`typedef void*`) while this table types every handle as `I64` — a call
    // type-checked, borrow-checked, and then could not be compiled. The wrappers
    // are lowered onto `__pd_file_handles`, the `long long` table `file_write`
    // and `file_close` already use, so the representation split that made them
    // uncallable is gone rather than described.
    Builtin {
        name: "file_flush",
        params: &[p("handle", I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Flush buffered writes on an open file (1 on success, 0 on failure)",
    },
    Builtin {
        name: "file_seek",
        params: &[
            p("handle", I64, ByCopy),
            p("whence", I64, ByCopy),
            p("offset", I64, ByCopy),
        ],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        support: Support::Callable,
        effects: IO,
        doc: "Move the read/write position of an open file (whence 0=start, 1=current, 2=end); returns the new position, or -1",
    },
    // The section that was here — "Enhanced file operations with mode support" —
    // IS GONE, 2026-08-23. Its `// ---- … ----` marker is deliberately not left
    // behind: scripts/gen-builtin-docs.py tracks those markers to build the
    // reference's sections, and an empty one would print a heading over nothing.
    //
    // `file_open_ex`, `file_close_ex`, `file_read_ex` and `file_write_ex` were
    // registered here, and this comment is the record of their deletion.
    //
    // THEY ARE NOT PART OF THE LANGUAGE. N14 enumerates the normative builtin
    // surface and names thirty-four identifiers; none of the four is among them
    // (docs/specification/language-spec.md, "The normative set, enumerated").
    // They were the whole of `implemented − normative`.
    //
    // MEASURED BEFORE DELETING, because a disposition in MILESTONES.md is a
    // document and a document is not evidence about code. All four, compiled at
    // `acda322` with the probe bodies tests/stdlib/BUILTINS.tsv records:
    //
    //   $ pdc compile file_open_ex.pd -o x.out
    //   error: Built-in file_open_ex is registered but not callable: returns an
    //   opaque FileHandle (typedef void*), which no Palladium type can hold …
    //   $ echo $?
    //   1
    //
    // — refused at typecheck, all four, and `grep -rn --include=*.pd` over the
    // whole tree finds ZERO callers. Nothing could reach them, nothing did, and
    // none of the four was quietly working.
    //
    // WHY DELETION AND NOT `Support::Unsupported`. `Unsupported` says "the
    // language has this and this implementation cannot compile it yet": the type
    // checker prints the reason and the LSP still describes the name. That is
    // right for `file_flush` and `file_seek` above, which N14 does define. It is
    // wrong here — keeping a name the language does not define, reserved and
    // described, makes this table a second definition of Palladium's surface,
    // which is the mistake N14 records having made once already when it
    // delegated its list to `docs/reference/builtins.md`.
    //
    // THEIR C WRAPPERS ARE GONE TOO, as of the same day. `src/codegen/mod.rs` no
    // longer writes `__pd_file_open_ex`, `__pd_file_close_ex`, `__pd_file_read_ex`
    // or `__pd_file_write_ex` into the prelude, and `runtime/pd_prelude.h` is
    // regenerated without them; the `FileHandle` typedef, the `FileMode` enum and
    // the six `pd_file_*` externs that only they used went with them.
    // *(For one round this comment said the wrappers were still emitted and their
    // removal was owed. That was true when it was written — the codegen file was
    // owned by another lane — and it is recorded here rather than silently
    // overwritten, because a stale OWED is exactly as misleading as a stale DONE.)*
    //
    // The name set is pinned against N14 by
    // `test_registry_is_exactly_the_normative_builtin_set`, so re-adding any of
    // the four is a red test rather than a review miss.
];

/// Look up a built-in by name.
pub fn lookup(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.name == name)
}

/// Is `name` a built-in function?
pub fn is_builtin(name: &str) -> bool {
    lookup(name).is_some()
}

/// The canonical set of built-in names.
pub fn names() -> std::collections::BTreeSet<&'static str> {
    BUILTINS.iter().map(|b| b.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The canonical table must not contain duplicate names.
    #[test]
    fn test_builtin_names_are_unique() {
        let mut seen = BTreeSet::new();
        for b in BUILTINS {
            assert!(seen.insert(b.name), "duplicate builtin in table: {}", b.name);
        }
        assert_eq!(seen.len(), BUILTINS.len());
    }

    /// A built-in never returns Unit in a value position and never takes Unit params.
    #[test]
    fn test_builtin_table_is_well_formed() {
        for b in BUILTINS {
            for (i, param) in b.params.iter().enumerate() {
                assert_ne!(
                    param.ty,
                    BuiltinType::Unit,
                    "{}: param {} has type Unit",
                    b.name,
                    i
                );
            }
            match (b.ret, b.ret_mode) {
                (BuiltinType::Unit, ReturnMode::Unit) => {}
                (BuiltinType::Unit, _) => panic!("{}: Unit return with non-Unit mode", b.name),
                (_, ReturnMode::Unit) => panic!("{}: non-Unit return with Unit mode", b.name),
                // Only a string can be a pointer into storage someone else owns.
                (ty, ReturnMode::BorrowedStatic) => assert_eq!(
                    ty,
                    BuiltinType::Str,
                    "{}: only a String return can be BorrowedStatic",
                    b.name
                ),
                _ => {}
            }
        }
    }

    /// The canonical names, owned, for comparison against a pass's registry.
    fn canonical() -> BTreeSet<String> {
        BUILTINS.iter().map(|b| b.name.to_string()).collect()
    }

    /// The names a `.pd` program can actually call. Every pass still *registers*
    /// the full canonical set — an unsupported built-in is described, not hidden —
    /// but surfaces that propose code to the user are limited to these.
    fn callable() -> BTreeSet<String> {
        BUILTINS
            .iter()
            .filter(|b| b.support.is_callable())
            .map(|b| b.name.to_string())
            .collect()
    }

    /// The names N14 enumerates, read out of the specification itself.
    ///
    /// The table is `| `name` | `signature` | effects |`, introduced by a
    /// `| Name | Signature | Effects |` header inside the `## N14.` section. The
    /// scan is anchored on the section heading rather than on the first table in
    /// the file, so an unrelated table gaining a `Name` column cannot redirect it.
    fn normative_names() -> (BTreeSet<String>, String) {
        let spec = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/specification/language-spec.md"
        ))
        .expect("docs/specification/language-spec.md");
        let start = spec
            .find("\n## N14.")
            .expect("the spec has no `## N14.` section");
        let section = &spec[start..];
        let end = section[1..]
            .find("\n## ")
            .map(|i| i + 1)
            .unwrap_or(section.len());
        let section = &section[..end];

        let header = section
            .find("| Name | Signature | Effects |")
            .expect("N14 has no normative-set table");
        let mut names = BTreeSet::new();
        for line in section[header..].lines().skip(2) {
            let line = line.trim();
            if !line.starts_with('|') {
                break;
            }
            let cell = line
                .trim_matches('|')
                .split('|')
                .next()
                .expect("a table row has a first cell")
                .trim();
            let name = cell.trim_matches('`').trim();
            assert!(
                !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "N14's normative table has a row whose first cell is not a builtin name: {:?}",
                cell
            );
            assert!(
                names.insert(name.to_string()),
                "N14 lists {} twice",
                name
            );
        }
        (names, section.to_string())
    }

    /// THE DURABLE GATE THAT POINTS AT THE LANGUAGE, not at another copy of the
    /// compiler's opinion: `BUILTINS` must be exactly the set N14 enumerates.
    ///
    /// Every other set test in this file compares one pass against `BUILTINS`,
    /// which keeps the compiler internally consistent and says nothing about
    /// whether the table describes Palladium. It did not: the table held 38
    /// names against N14's 34, and the four extra (`file_open_ex`,
    /// `file_close_ex`, `file_read_ex`, `file_write_ex`) were visible only to
    /// someone reading both documents at once. A registry that can name what the
    /// specification does not define is a second definition of the language.
    ///
    /// This is the control for that removal: putting any of the four back turns
    /// this red, and so does adding a thirty-fifth name of any kind.
    ///
    /// DIRECTION MATTERS AND BOTH ARE CHECKED. `normative − implemented` is a
    /// missing capability; `implemented − normative` is an invented one. A test
    /// that only checked the first would have been green throughout the four
    /// years those names existed.
    #[test]
    fn test_registry_is_exactly_the_normative_builtin_set() {
        let (normative, section) = normative_names();

        // The spec states its own count in prose. Pinning it here is what stops
        // the table being weakened to match the compiler: deleting a row to make
        // this test pass leaves the sentence disagreeing with the table.
        assert!(
            section.contains("Thirty-four names."),
            "N14 no longer states its own count; the table and the prose can now drift"
        );
        assert_eq!(
            normative.len(),
            34,
            "N14's table lists {} names but the section says thirty-four",
            normative.len()
        );

        let implemented = canonical();
        let invented: Vec<&String> = implemented.difference(&normative).collect();
        let missing: Vec<&String> = normative.difference(&implemented).collect();
        assert!(
            invented.is_empty(),
            "src/builtins.rs registers {:?}, which N14 does not define. A builtin \
             the language does not have is a second definition of the builtin \
             surface — remove it from the table, or add it to N14 and say why \
             the language grew",
            invented
        );
        assert!(
            missing.is_empty(),
            "N14 requires {:?}, which src/builtins.rs does not register. A \
             conforming implementation provides all of them",
            missing
        );
    }

    /// THE DURABLE GATE: a fresh type checker must know exactly the canonical set.
    /// Adding a built-in to `typeck` alone fails here.
    #[test]
    fn test_typeck_registers_exactly_the_canonical_builtins() {
        let tc = crate::typeck::TypeChecker::new();
        assert_eq!(
            tc.registered_function_names(),
            canonical(),
            "typeck builtin set drifted from src/builtins.rs"
        );
    }

    /// THE DURABLE GATE: a fresh borrow checker must know exactly the canonical
    /// set. This is the regression that let 11 built-ins type-check but die in
    /// `🔒 Borrow checking...` with "Use of uninitialized value".
    #[test]
    fn test_borrow_checker_registers_exactly_the_canonical_builtins() {
        let bc = crate::ownership::BorrowChecker::new();
        assert_eq!(
            bc.registered_function_names(),
            canonical(),
            "borrow checker builtin set drifted from src/builtins.rs"
        );
    }

    /// And, explicitly, the two passes must agree with each other.
    #[test]
    fn test_typeck_and_borrow_checker_agree() {
        let tc = crate::typeck::TypeChecker::new();
        let bc = crate::ownership::BorrowChecker::new();
        assert_eq!(
            tc.registered_function_names(),
            bc.registered_function_names()
        );
    }

    // ---- Registration-level gates -------------------------------------------
    //
    // The tests from here to `test_signature_rendering` inspect what a consumer has
    // *registered*. That is necessary but not sufficient: a consumer could keep a
    // correct registry-derived table and stop consulting it. The behavioural gates
    // at the bottom of this file drive the real entry points — effect analysis of
    // parsed source, and the LSP completion/hover requests — and are what catch a
    // consumer that unhooks itself. Both layers are kept deliberately: the
    // registration gates localise the fault, the behavioural gates prove the fault
    // would be user-visible.

    /// THE DURABLE GATE: a fresh effect analyzer must know exactly the canonical
    /// set. The hand-written table this replaced knew 19 of 38, and a built-in with
    /// no entry contributes *no effect*, so a file-writing call was analyzed as pure.
    #[test]
    fn test_effect_analyzer_registers_exactly_the_canonical_builtins() {
        let ea = crate::effects::EffectAnalyzer::new();
        assert_eq!(
            ea.registered_builtin_names(),
            canonical(),
            "effect analyzer builtin set drifted from src/builtins.rs"
        );
    }

    /// And the effects it attributes must be the ones this table declares — being
    /// *registered* with the wrong effect set is the same bug one level down.
    #[test]
    fn test_effect_analyzer_attributes_the_canonical_effects() {
        use crate::effects::{Effect, EffectSet};

        let ea = crate::effects::EffectAnalyzer::new();
        for b in BUILTINS {
            let mut expected = EffectSet::new();
            for effect in b.effects {
                expected.add(effect.clone());
            }
            let actual = ea
                .builtin_effects_of(b.name)
                .unwrap_or_else(|| panic!("effect analyzer is missing builtin: {}", b.name));
            assert_eq!(actual, &expected, "effects drifted for builtin: {}", b.name);
            assert_eq!(
                actual.is_pure(),
                b.effects.is_empty(),
                "purity drifted for builtin: {}",
                b.name
            );
            // A built-in that does I/O must never be analyzed as pure: that is the
            // exact failure the 19 missing entries produced.
            if b.effects.contains(&Effect::IO) {
                assert!(
                    !actual.is_pure() && actual.contains(&Effect::IO),
                    "{} does I/O but the analyzer treats it as effect-free",
                    b.name
                );
            }
        }
    }

    /// The built-ins that used to be missing from the effect analyzer. There
    /// were 19, 18 of which do file or console I/O and were analyzed as pure.
    ///
    /// FIFTEEN ARE LISTED, NOT NINETEEN, and the four absentees are named here
    /// rather than silently dropped: `file_open_ex`, `file_close_ex`,
    /// `file_read_ex` and `file_write_ex` left `BUILTINS` altogether (see the
    /// note in the table). A historical list is a record of a regression, so
    /// shrinking one has to be accounted for — this shrank because the names no
    /// longer exist, not because the regression stopped mattering for them.
    #[test]
    fn test_previously_effect_free_builtins_have_effects() {
        const PREVIOUSLY_MISSING: &[&str] = &[
            "panic",
            "arg_count",
            "arg_at",
            "path_exists",
            "path_is_file",
            "path_is_dir",
            "create_dir",
            "create_dir_all",
            "remove_file",
            "remove_dir",
            "remove_dir_all",
            "read_file_to_string",
            "write_string_to_file",
            "file_flush",
            "file_seek",
        ];
        let ea = crate::effects::EffectAnalyzer::new();
        for name in PREVIOUSLY_MISSING {
            let effects = ea
                .builtin_effects_of(name)
                .unwrap_or_else(|| panic!("effect analyzer is missing builtin: {}", name));
            assert!(
                !effects.is_pure(),
                "{} is back to being silently effect-free",
                name
            );
        }
    }

    /// THE DURABLE GATE: LSP completion must offer exactly the canonical set.
    /// The empty prefix matches every built-in, so this is a full set comparison.
    #[test]
    fn test_lsp_completion_offers_exactly_the_canonical_builtins() {
        let offered: BTreeSet<String> = crate::lsp::completion::builtin_completions("")
            .into_iter()
            .map(|item| item.label)
            .collect();
        assert_eq!(
            offered,
            callable(),
            "LSP completion builtin set drifted from src/builtins.rs"
        );
    }

    /// THE DURABLE GATE: LSP hover must answer for exactly the canonical set, and
    /// with the derived signature — not a hand-written one that can go stale (the
    /// deleted list claimed `string_to_int` returned `Option<i64>`).
    #[test]
    fn test_lsp_hover_answers_for_exactly_the_canonical_builtins() {
        for b in BUILTINS {
            let hover = crate::lsp::hover::builtin_hover(b.name)
                .unwrap_or_else(|| panic!("LSP hover is missing builtin: {}", b.name));
            assert!(
                hover.contents.value.contains(&b.signature()),
                "hover for {} does not show its derived signature",
                b.name
            );
            assert!(
                hover.contents.value.contains(b.doc),
                "hover for {} does not show its documentation",
                b.name
            );
        }
        assert!(
            crate::lsp::hover::builtin_hover("definitely_not_a_builtin").is_none(),
            "LSP hover invented a builtin that is not in src/builtins.rs"
        );
    }

    /// Completion and hover must describe a built-in identically; they used to keep
    /// two lists with two different wordings of the same doc.
    #[test]
    fn test_lsp_completion_and_hover_agree() {
        for item in crate::lsp::completion::builtin_completions("") {
            let hover = crate::lsp::hover::builtin_hover(&item.label)
                .unwrap_or_else(|| panic!("LSP hover is missing builtin: {}", item.label));
            let detail = item.detail.expect("completion item without a signature");
            let doc = item
                .documentation
                .expect("completion item without documentation");
            assert!(
                hover.contents.value.contains(&detail) && hover.contents.value.contains(&doc),
                "completion and hover disagree about builtin: {}",
                item.label
            );
        }
    }

    /// Every built-in must carry LSP metadata; a new one added with an empty doc
    /// would show up as a blank tooltip rather than a compile error.
    #[test]
    fn test_every_builtin_has_lsp_metadata() {
        for b in BUILTINS {
            assert!(!b.doc.is_empty(), "{} has no documentation", b.name);
            for (i, param) in b.params.iter().enumerate() {
                assert!(!param.name.is_empty(), "{}: param {} has no name", b.name, i);
            }
            let sig = b.signature();
            assert!(
                sig.starts_with(&format!("fn {}(", b.name)),
                "{}: derived signature is malformed: {}",
                b.name,
                sig
            );
        }
    }

    /// The signature renderer, pinned on the shapes the LSP used to hard-code.
    #[test]
    fn test_signature_rendering() {
        let sig = |name: &str| lookup(name).expect(name).signature();
        assert_eq!(sig("print"), "fn print(s: String)");
        assert_eq!(sig("print_int"), "fn print_int(n: i64)");
        assert_eq!(sig("string_len"), "fn string_len(s: String) -> i64");
        assert_eq!(
            sig("string_concat"),
            "fn string_concat(a: String, b: String) -> String"
        );
        assert_eq!(sig("int_to_string"), "fn int_to_string(n: i64) -> String");
        assert_eq!(sig("arg_count"), "fn arg_count() -> i64");
        assert_eq!(sig("file_exists"), "fn file_exists(path: String) -> bool");
        // The deleted LSP lists said `-> Option<i64>`; the compiler emits
        // `long long __pd_string_to_int(const char*)` (src/codegen/mod.rs) and the
        // type checker derives i64 from this table, so i64 is the truth.
        assert_eq!(sig("string_to_int"), "fn string_to_int(s: String) -> i64");
    }

    /// The 11 built-ins that used to be missing from the borrow checker.
    #[test]
    fn test_previously_missing_builtins_are_registered() {
        const PREVIOUSLY_MISSING: &[&str] = &[
            "string_len",
            "string_eq",
            "string_char_at",
            "string_from_char",
            "char_is_digit",
            "char_is_alpha",
            "char_is_whitespace",
            "file_read_all",
            "file_read_line",
            "file_write",
            "panic",
        ];
        let bc = crate::ownership::BorrowChecker::new();
        let registered = bc.registered_function_names();
        for name in PREVIOUSLY_MISSING {
            assert!(
                registered.contains(*name),
                "borrow checker is missing builtin: {}",
                name
            );
        }
    }

    // ---- Behavioural gates ---------------------------------------------------
    //
    // Everything above asks a consumer what it has registered. These ask what the
    // consumer actually *does*, through the same entry points the driver and an
    // editor use. A consumer that keeps a correct table and stops consulting it —
    // `analyze_expression` no longer unioning built-in effects, a completion or
    // hover handler no longer calling its registry-derived helper — passes every
    // test above and fails every test below.

    /// Palladium source calling `builtin` with literal arguments, as a statement.
    fn probe_source(b: &Builtin) -> String {
        let args = b
            .params
            .iter()
            .map(|param| match param.ty {
                BuiltinType::I64 => "0".to_string(),
                BuiltinType::Str => "\"x\"".to_string(),
                BuiltinType::Bool => "true".to_string(),
                // N4-04: a `char` parameter needs a char literal, not a number.
                BuiltinType::Char => "'x'".to_string(),
                BuiltinType::Unit => unreachable!("no builtin takes a Unit parameter"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        // Named `main` because the type checker requires a main function, and
        // these probes are fed to it as whole programs.
        format!("fn main() {{\n    {}({});\n}}\n", b.name, args)
    }

    /// Parse Palladium source the way the driver does and hand back its one function.
    fn parse_probe(source: &str) -> crate::ast::Function {
        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer
            .collect_tokens()
            .unwrap_or_else(|e| panic!("probe failed to lex: {:?}\n{}", e, source));
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser
            .parse()
            .unwrap_or_else(|e| panic!("probe failed to parse: {:?}\n{}", e, source));
        match program.items.into_iter().next() {
            Some(crate::ast::Item::Function(f)) => f,
            other => panic!("probe did not produce a function: {:?}", other),
        }
    }

    /// THE BEHAVIOURAL GATE for effects: analyzing real source that calls a built-in
    /// must attribute that built-in's effects to the calling function.
    ///
    /// This is the property the driver depends on (`src/driver/mod.rs`, phase 3.6).
    /// It goes red if `analyze_expression` stops consulting the built-in table, which
    /// no registration-level test can see.
    #[test]
    fn test_effect_analysis_of_real_source_attributes_builtin_effects() {
        use crate::effects::{EffectAnalyzer, EffectSet};

        for b in BUILTINS {
            let func = parse_probe(&probe_source(b));
            let mut analyzer = EffectAnalyzer::new();
            let actual = analyzer
                .analyze_function(&func)
                .unwrap_or_else(|e| panic!("{}: effect analysis failed: {:?}", b.name, e));

            let mut expected = EffectSet::new();
            for effect in b.effects {
                expected.add(effect.clone());
            }
            assert_eq!(
                actual, expected,
                "calling {} does not carry its registered effects into the caller",
                b.name
            );
            assert_eq!(
                actual.is_pure(),
                b.effects.is_empty(),
                "a function whose whole body is a call to {} reports the wrong purity",
                b.name
            );
        }
    }

    /// A function that calls nothing is pure — the control for the test above, so
    /// that "everything is effectful" could not make it pass.
    #[test]
    fn test_effect_analysis_control_pure_function() {
        use crate::effects::EffectAnalyzer;

        let func = parse_probe("fn main() {\n    let x: i64 = 1 + 2;\n}\n");
        let mut analyzer = EffectAnalyzer::new();
        let effects = analyzer.analyze_function(&func).unwrap();
        assert!(
            effects.is_pure(),
            "a function with no calls should be pure, got {:?}",
            effects.sorted()
        );
    }

    /// An LSP server with one open document, no AST attached.
    ///
    /// With no AST the only `Function`-kind completions the server can produce are
    /// the built-ins (`src/lsp/completion.rs`: the other source is the document's
    /// own functions), which is what makes the set comparison below exact.
    fn server_with(content: &str) -> crate::lsp::LanguageServer {
        let mut server = crate::lsp::LanguageServer::new();
        server.initialize(None).expect("initialize");
        server
            .open_document("file:///probe.pd".to_string(), 1, content.to_string())
            .expect("open_document");
        server
    }

    /// THE BEHAVIOURAL GATE for completion: a real completion request must offer
    /// exactly the canonical built-ins, with their derived signature and doc.
    ///
    /// The document is a bare `_`, and the server's word scan stops at any
    /// non-alphanumeric character (`get_completion_context`), so the prefix is empty
    /// and every built-in is in scope. Goes red if the handler stops calling
    /// `builtin_completions`, however healthy that helper is.
    #[test]
    fn test_lsp_completion_request_offers_exactly_the_canonical_builtins() {
        use crate::lsp::completion::CompletionItemKind;
        use crate::lsp::Position;

        let server = server_with("_");
        let items = server.get_completions(
            "file:///probe.pd",
            Position {
                line: 0,
                character: 1,
            },
        );
        let functions: Vec<_> = items
            .iter()
            .filter(|item| matches!(item.kind, Some(CompletionItemKind::Function)))
            .collect();

        let offered: BTreeSet<String> = functions.iter().map(|i| i.label.clone()).collect();
        assert_eq!(
            offered,
            callable(),
            "a completion request no longer offers exactly the callable builtin set"
        );

        for b in BUILTINS.iter().filter(|b| b.support.is_callable()) {
            let item = functions
                .iter()
                .find(|i| i.label == b.name)
                .unwrap_or_else(|| panic!("completion request omitted {}", b.name));
            assert_eq!(
                item.detail.as_deref(),
                Some(b.signature().as_str()),
                "completion request shows a stale signature for {}",
                b.name
            );
            assert_eq!(
                item.documentation.as_deref(),
                Some(b.doc),
                "completion request shows stale documentation for {}",
                b.name
            );
        }
    }

    // ---- The registry-to-runtime seam ---------------------------------------
    //
    // Every gate above lives inside the Rust compiler. None of them look at the C
    // the compiler emits, and that is where the last copy of built-in knowledge
    // lives: the prelude in src/codegen/mod.rs writes a `__pd_<name>` C function
    // for every built-in, by hand, and runtime/pd_prelude.h is generated from it
    // (scripts/gen-prelude.sh) for the bootstrap compiler to #include.
    //
    // A built-in whose C wrapper disagrees with this table type-checks, borrow-
    // checks, and then dies in gcc — which is the same D2 drift class, one layer
    // below the compiler. NO built-in is in that state today:
    // `PRELUDE_TYPE_MISMATCHES` is empty, and the test below derives that set
    // from `BUILTINS` × the emitted prelude on every run, so empty is an
    // assertion rather than an absence.
    //
    // SIX WERE, and they closed two different ways — `file_flush` and `file_seek`
    // by having their wrappers re-based onto `__pd_file_handles`, the four `*_ex`
    // names by leaving both this table and the emitted prelude entirely. The
    // account is in `PRELUDE_TYPE_MISMATCHES` below, which is where the eleven
    // dimensions are itemised; this paragraph said they were still broken and
    // still emitted for one round after they were neither.

    /// The C shape of a value, recorded finely enough to see a lossy conversion.
    ///
    /// The first version of this test compared integer-vs-pointer only, on the
    /// argument that width differences are value-preserving. That is true of
    /// widening and false of narrowing, and the prelude does both: measured with
    /// gcc, `256` passed to a `uint8_t whence` arrives as `0`, `-1` passed to a
    /// `size_t len` arrives as `18446744073709551615`, and `2^32` passed to an
    /// `int mode` arrives as `0`. Width, signedness and mutability are all recorded
    /// here because each of them can silently corrupt a value or a pointer.
    #[derive(Debug, PartialEq, Eq, Clone)]
    enum CShape {
        Integer { bits: u32, signed: bool },
        /// `char*` / `const char*`; `mutable` is true for the non-const form, which
        /// is a *writable destination* the callee may store into.
        StringPointer { mutable: bool },
        Void,
        /// A pointer that is not a string — an opaque handle. No Palladium built-in
        /// type can be passed as one.
        OpaquePointer,
    }

    impl CShape {
        fn describe(&self) -> String {
            match self {
                CShape::Integer { bits, signed } => {
                    format!("{}{}", if *signed { "i" } else { "u" }, bits)
                }
                CShape::StringPointer { mutable: true } => "char* (writable)".to_string(),
                CShape::StringPointer { mutable: false } => "const char*".to_string(),
                CShape::Void => "void".to_string(),
                CShape::OpaquePointer => "opaque pointer".to_string(),
            }
        }
    }

    /// Assumes the LP64 model this compiler targets (`long` and `size_t` are 64-bit).
    fn classify_c_type(c_type: &str) -> CShape {
        let t = c_type.trim();
        let is_const = t.starts_with("const ");
        let bare = t.trim_start_matches("const ").trim();
        if bare == "void" {
            return CShape::Void;
        }
        if bare.ends_with('*') {
            let pointee = bare.trim_end_matches('*').trim();
            return if pointee == "char" {
                CShape::StringPointer { mutable: !is_const }
            } else {
                CShape::OpaquePointer
            };
        }
        match bare {
            "char" | "int8_t" => CShape::Integer {
                bits: 8,
                signed: true,
            },
            "uint8_t" | "unsigned char" => CShape::Integer {
                bits: 8,
                signed: false,
            },
            "short" | "int16_t" => CShape::Integer {
                bits: 16,
                signed: true,
            },
            "uint16_t" | "unsigned short" => CShape::Integer {
                bits: 16,
                signed: false,
            },
            "int" | "int32_t" => CShape::Integer {
                bits: 32,
                signed: true,
            },
            "uint32_t" | "unsigned" | "unsigned int" => CShape::Integer {
                bits: 32,
                signed: false,
            },
            "long" | "long long" | "int64_t" | "ssize_t" | "ptrdiff_t" => CShape::Integer {
                bits: 64,
                signed: true,
            },
            "uint64_t" | "size_t" | "unsigned long" | "unsigned long long" => CShape::Integer {
                bits: 64,
                signed: false,
            },
            // FileHandle and friends are typedefs for pointers.
            _ => CShape::OpaquePointer,
        }
    }

    /// The C shape a Palladium type has when the compiler hands it to C.
    ///
    /// `I64` is a 64-bit signed integer and `Str` is an immutable string; those are
    /// the only things a `.pd` program can produce.
    fn palladium_shape(ty: BuiltinType) -> CShape {
        match ty {
            BuiltinType::I64 => CShape::Integer {
                bits: 64,
                signed: true,
            },
            // N4-04: a `char` is a DISTINCT TYPE with the SAME CARRIER. It rides
            // in `long long` because a C `char` holds 8 bits and a Unicode
            // scalar needs 21 — so its C shape is the `I64` one, and this arm
            // saying so is what keeps the split from reaching the ABI.
            BuiltinType::Char => CShape::Integer {
                bits: 64,
                signed: true,
            },
            // Bool is `int` in the emitted C; there is no C bool in the prelude.
            BuiltinType::Bool => CShape::Integer {
                bits: 32,
                signed: true,
            },
            BuiltinType::Str => CShape::StringPointer { mutable: false },
            BuiltinType::Unit => CShape::Void,
        }
    }

    /// Can a Palladium value of shape `from` be passed *into* a C parameter of
    /// shape `to` without loss?
    ///
    /// Directional on purpose. Into a parameter, the C type must be able to hold
    /// every value the Palladium type can produce: narrowing loses the high bits and
    /// an unsigned destination turns negatives into huge positives. A writable
    /// `char*` destination cannot be supplied at all, because the only string a
    /// `.pd` program has is immutable and may live in read-only memory — measured:
    /// writing through such a pointer is SIGBUS.
    fn param_is_lossless(from: &CShape, to: &CShape) -> bool {
        match (from, to) {
            (
                CShape::Integer {
                    bits: fb,
                    signed: fs,
                },
                CShape::Integer {
                    bits: tb,
                    signed: ts,
                },
            ) => {
                if *fs == *ts {
                    tb >= fb
                } else if *fs && !*ts {
                    // signed -> unsigned: negatives wrap
                    false
                } else {
                    // unsigned -> signed needs a strictly wider destination
                    tb > fb
                }
            }
            (CShape::StringPointer { .. }, CShape::StringPointer { mutable: true }) => false,
            (CShape::StringPointer { .. }, CShape::StringPointer { mutable: false }) => true,
            (CShape::Void, CShape::Void) => true,
            _ => false,
        }
    }

    /// Can a C return value of shape `from` be received into Palladium's `to`?
    ///
    /// The mirror image: here widening is what happens, and it is safe. `int` into
    /// an `i64` result is fine; an unsigned 64-bit result is not, because it can
    /// exceed `i64::MAX` and arrive negative.
    fn return_is_lossless(from: &CShape, to: &CShape) -> bool {
        match (from, to) {
            (
                CShape::Integer {
                    bits: fb,
                    signed: fs,
                },
                CShape::Integer {
                    bits: tb,
                    signed: ts,
                },
            ) => {
                if *fs == *ts {
                    tb >= fb
                } else if !*fs && *ts {
                    tb > fb
                } else {
                    false
                }
            }
            // Returning a writable char* into an immutable Palladium String is fine;
            // the language simply never writes through it.
            (CShape::StringPointer { .. }, CShape::StringPointer { .. }) => true,
            (CShape::Void, CShape::Void) => true,
            _ => false,
        }
    }

    /// The C prelude the compiler emits, obtained from the real code generator.
    fn emitted_prelude() -> String {
        let mut generator =
            crate::codegen::CodeGenerator::new("prelude_probe").expect("codegen::new");
        let empty = crate::ast::Program {
            imports: vec![],
            items: vec![],
        };
        generator.compile(&empty).expect("compile empty program");
        generator.generated_c().to_string()
    }

    /// The C definition of `__pd_<name>` in `prelude`, as (return type, params).
    ///
    /// Every wrapper in the prelude is written with its whole signature on one
    /// line, ending in `{`, which is what makes this scan sufficient.
    fn c_signature_of(prelude: &str, name: &str) -> Option<(String, Vec<String>)> {
        let needle = format!(" __pd_{}(", name);
        for line in prelude.lines() {
            let line = line.trim_end();
            if !line.ends_with('{') || !line.contains(&needle) {
                continue;
            }
            let at = line.find(&needle)?;
            let ret = line[..at].trim().to_string();
            let open = at + needle.len();
            let close = line.rfind(')')?;
            let inside = line[open..close].trim();
            let params = if inside.is_empty() || inside == "void" {
                Vec::new()
            } else {
                inside.split(',').map(|p| p.trim().to_string()).collect()
            };
            return Some((ret, params));
        }
        None
    }

    /// The C type of a parameter declaration such as `const char* path`.
    fn param_c_type(decl: &str) -> String {
        let decl = decl.trim();
        // Strip the parameter name: everything after the last space, unless that
        // space is part of the type (`long long`), which the `*` case also covers.
        match decl.rfind(['*', ' ']) {
            Some(idx) if decl[idx..].starts_with('*') => decl[..=idx].trim().to_string(),
            Some(idx) => decl[..idx].trim().to_string(),
            None => decl.to_string(),
        }
    }

    /// The built-ins whose C wrapper contradicts this table today.
    ///
    /// **EMPTY, since 2026-08-23, and empty is a real state rather than a
    /// disabled check.** The test below DERIVES this set from `BUILTINS` × the
    /// emitted prelude on every run and requires it to equal exactly this
    /// constant, so an empty list is the strongest form of the assertion: any
    /// new disagreement between the registry and the C it generates is a new
    /// string and fails.
    ///
    /// It held ELEVEN dimensions across SIX built-ins, all of them the enhanced
    /// file API's opaque `FileHandle` (`typedef void*`, a cast FILE*) meeting a
    /// table that types every handle as `I64`. They passed the type checker and
    /// the borrow checker and then failed to compile:
    ///
    ///   incompatible integer to pointer conversion passing 'long long'
    ///   to parameter of type 'FileHandle' (aka 'void *')
    ///
    /// The eleven closed in two unrelated ways, and the difference is worth
    /// keeping straight because "eleven to zero" reads like one achievement:
    ///
    ///   * EIGHT belonged to `file_open_ex`, `file_close_ex`, `file_read_ex` and
    ///     `file_write_ex`, which N14 does not define. Those names left this
    ///     table, and their C wrappers have now been deleted from
    ///     `src/codegen/mod.rs` as well — nothing emits them any more.
    ///   * THREE belonged to `file_flush` and `file_seek`, which N14 does
    ///     define. Those were REPAIRED: the wrappers are lowered onto
    ///     `__pd_file_handles`, the `long long` handle table the rest of the
    ///     file API uses, and both builtins are `Support::Callable` and
    ///     exercised by `tests/stdlib/stdlib_builtins_file.pd`.
    ///
    /// The narrowing measurements that produced the deleted entries, kept
    /// because they are what a future handle representation has to survive:
    /// with gcc, `256` passed to a `uint8_t whence` arrives as `0`, `-1` passed
    /// to a `size_t len` arrives as `18446744073709551615`, and `2^32` passed to
    /// an `int mode` arrives as `0`. The `char* (writable)` entry was the
    /// memory-safety one: `file_read_ex` wanted a destination to write into, and
    /// a Palladium String may be a literal in read-only memory — writing through
    /// it is SIGBUS.
    ///
    /// This list states known defects; it is not permission. Fixing a dimension
    /// must delete its line, and the test fails if it does not.
    const PRELUDE_TYPE_MISMATCHES: &[&str] = &[];


    /// THE SEAM GATE: for every built-in, the C wrapper the compiler emits must be
    /// able to carry the values this table describes — every parameter and the
    /// return, checked in the direction the value actually travels.
    ///
    /// The known-broken dimensions must be exactly `PRELUDE_TYPE_MISMATCHES`.
    #[test]
    fn test_emitted_prelude_agrees_with_the_registry() {
        let prelude = emitted_prelude();
        let mut mismatched: BTreeSet<String> = BTreeSet::new();

        for b in BUILTINS {
            let (ret, params) = c_signature_of(&prelude, b.name).unwrap_or_else(|| {
                panic!(
                    "{} is registered but the emitted C prelude defines no __pd_{}",
                    b.name, b.name
                )
            });

            assert_eq!(
                params.len(),
                b.params.len(),
                "{}: registry declares {} parameters, C prelude declares {} ({:?})",
                b.name,
                b.params.len(),
                params.len(),
                params
            );

            // Return: the C value travels back into a Palladium value.
            let c_ret = classify_c_type(&ret);
            let pd_ret = palladium_shape(b.ret);
            if !return_is_lossless(&c_ret, &pd_ret) {
                mismatched.insert(format!(
                    "{} return: {} -> {}",
                    b.name,
                    c_ret.describe(),
                    pd_ret.describe()
                ));
            }

            // Parameters: the Palladium value travels into the C parameter.
            for (i, (param, decl)) in b.params.iter().zip(params.iter()).enumerate() {
                let pd = palladium_shape(param.ty);
                let c = classify_c_type(&param_c_type(decl));
                if !param_is_lossless(&pd, &c) {
                    mismatched.insert(format!(
                        "{} param {} ({}): {} -> {}",
                        b.name,
                        i,
                        param.name,
                        pd.describe(),
                        c.describe()
                    ));
                }
            }
        }

        let known: BTreeSet<String> = PRELUDE_TYPE_MISMATCHES
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            mismatched, known,
            "the ways in which the C prelude contradicts src/builtins.rs changed. \
             New lines = a value that will be corrupted or a call that will not \
             compile; missing lines = a fix that should also delete the line from \
             PRELUDE_TYPE_MISMATCHES"
        );
    }

    /// THE UNSUPPORTED SET IS EMPTY, AND THAT IS SAID OUT LOUD BECAUSE THE
    /// BEHAVIOURAL GATE BELOW NOW ITERATES NOTHING.
    ///
    /// Every built-in in this table is callable as of 2026-08-23. That is the
    /// good state, and it makes `test_calling_an_unsupported_builtin_is_rejected
    /// _by_typeck` a loop over an empty set — a test that passes while proving
    /// nothing, which is the exact species this milestone exists to remove. It
    /// is not deleted (the machinery it guards is still live in
    /// `src/typeck/mod.rs:4028-4036` and the moment any built-in is marked
    /// unsupported the loop has work again), and it is not left to look like
    /// coverage either. This assertion is the declaration: if it ever fails,
    /// the loop below has become meaningful again and this comment is stale.
    #[test]
    fn test_the_unsupported_builtin_set_is_empty_so_the_gate_below_is_vacuous() {
        let unsupported: Vec<&str> = BUILTINS
            .iter()
            .filter(|b| !b.support.is_callable())
            .map(|b| b.name)
            .collect();
        assert_eq!(
            unsupported,
            Vec::<&str>::new(),
            "a built-in is unsupported again — which is fine, but it means \
             test_calling_an_unsupported_builtin_is_rejected_by_typeck is no \
             longer vacuous and this test's own comment must be updated to say so"
        );
    }

    /// THE BEHAVIOURAL GATE for support status: a program that calls an unsupported
    /// built-in must be rejected by the type checker, naming the built-in and the
    /// reason — not passed through to gcc to fail there in generated C.
    ///
    /// **VACUOUS TODAY.** Its iteration set is `BUILTINS` filtered to the
    /// unsupported, and that set is empty; see the test above, which declares the
    /// emptiness so this one cannot be mistaken for coverage. Kept because the
    /// rejection path is still live and this is the only test that drives it.
    #[test]
    fn test_calling_an_unsupported_builtin_is_rejected_by_typeck() {
        for b in BUILTINS.iter().filter(|b| !b.support.is_callable()) {
            let func = probe_source(b);
            let mut lexer = crate::lexer::Lexer::new(&func);
            let tokens = lexer.collect_tokens().expect("probe lexes");
            let program = crate::parser::Parser::new(tokens)
                .parse()
                .expect("probe parses");

            let err = crate::typeck::TypeChecker::new()
                .check(&program)
                .expect_err(&format!(
                    "{} is unsupported but the type checker accepted a call to it",
                    b.name
                ));
            match err {
                crate::errors::CompileError::UnsupportedBuiltin { name, reason, .. } => {
                    assert_eq!(name, b.name);
                    assert_eq!(Some(reason.as_str()), b.support.reason());
                }
                other => panic!("{}: expected UnsupportedBuiltin, got {:?}", b.name, other),
            }
        }
    }

    /// A supported built-in must still type-check, so the rejection above cannot be
    /// passing by rejecting everything.
    #[test]
    fn test_calling_a_supported_builtin_is_accepted_by_typeck() {
        let b = lookup("print").expect("print");
        let source = probe_source(b);
        let mut lexer = crate::lexer::Lexer::new(&source);
        let tokens = lexer.collect_tokens().expect("probe lexes");
        let program = crate::parser::Parser::new(tokens)
            .parse()
            .expect("probe parses");
        crate::typeck::TypeChecker::new()
            .check(&program)
            .expect("a call to print must type-check");
    }

    /// Hover does not hide an unsupported built-in — it says so. Completion omits
    /// them (asserted by the `callable()` comparisons); hover is the surface where
    /// a user asks *about* a name they have already typed, so silence there would
    /// answer the question with nothing.
    #[test]
    fn test_hover_marks_unsupported_builtins_as_not_callable() {
        for b in BUILTINS {
            let hover = crate::lsp::hover::builtin_hover(b.name)
                .unwrap_or_else(|| panic!("hover is missing {}", b.name));
            match b.support.reason() {
                Some(reason) => {
                    assert!(
                        hover.contents.value.contains("Not callable"),
                        "hover for the unsupported {} does not say it is not callable",
                        b.name
                    );
                    assert!(
                        hover.contents.value.contains(reason),
                        "hover for {} does not give the reason",
                        b.name
                    );
                }
                None => assert!(
                    !hover.contents.value.contains("Not callable"),
                    "hover for the callable {} claims it is not callable",
                    b.name
                ),
            }
        }
    }

    /// `Owned` must mean "this built-in allocated it", which is exactly the set
    /// carrying `Effect::Memory`. `arg_at` was the counter-example: it returned
    /// storage belonging to `argv` while claiming `Owned`, so the ownership pass was
    /// told the caller may free process memory. It is `BorrowedStatic` now.
    ///
    /// **THIS TEST IS NOT SUFFICIENT AND WAS TREATED AS IF IT WERE.** It compares
    /// `ret_mode` against `effects` — TWO FIELDS OF THIS TABLE — so it certifies
    /// that the metadata agrees with itself and says nothing about the C the
    /// compiler emits. Measured: while it was green, four `Owned` builtins had
    /// reachable branches returning the literal `""`, which is static storage they
    /// did not allocate. Two matching declarations do not make an implementation
    /// true. The test that reads the implementation is
    /// `test_no_owned_wrapper_returns_a_string_literal`, immediately below — and
    /// it covers ONE way of violating the property, which its own note states.
    #[test]
    fn test_owned_returns_are_exactly_the_allocating_builtins() {
        for b in BUILTINS {
            let allocates = b.effects.contains(&Effect::Memory);
            let owned = b.ret_mode == ReturnMode::Owned;
            assert_eq!(
                owned, allocates,
                "{}: ret_mode Owned = {} but Effect::Memory = {}",
                b.name, owned, allocates
            );
        }
        assert_eq!(
            lookup("arg_at").expect("arg_at").ret_mode,
            ReturnMode::BorrowedStatic,
            "arg_at returns a pointer into argv; it must not claim Owned"
        );
    }

    /// THE OTHER SEAM: the prelude's `extern pd_*` declarations against the real
    /// definitions in `runtime/palladium_runtime.c`.
    ///
    /// Those live in different translation units, so C never compares them: a
    /// disagreement links cleanly and corrupts arguments at run time. Forcing the
    /// declarations into the same translation unit as the definitions makes the
    /// compiler check them. No existing gate covered this — conformance and
    /// selfhost link the two objects together, which is exactly the situation in
    /// which C does *not* check.
    #[test]
    fn test_prelude_externs_match_the_runtime_definitions() {
        use std::process::Command;

        let root = env!("CARGO_MANIFEST_DIR");

        // No skip. Every language-level gate in this repo already requires gcc, so
        // skipping could not buy portability — it could only report success for a
        // check that never ran, which is the failure mode this milestone exists to
        // remove.
        let output = Command::new("gcc")
            .arg("-fsyntax-only")
            .arg("-I")
            .arg(format!("{}/runtime", root))
            .arg("-include")
            .arg(format!("{}/runtime/pd_prelude.h", root))
            .arg(format!("{}/runtime/palladium_runtime.c", root))
            .output()
            .expect("gcc must be available: every gate in this repo compiles C");

        assert!(
            output.status.success(),
            "the prelude's `extern pd_*` declarations disagree with the definitions \
             in runtime/palladium_runtime.c. C does not diagnose this across \
             translation units, so it would link and then corrupt arguments:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// A C expression with pointer casts and whitespace removed: `(char*)0`
    /// becomes `0`. An inner `(a + b)` is left alone — stripping it would change
    /// what the expression says.
    fn strip_casts(expr: &str) -> String {
        let mut s: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
        while s.starts_with('(') {
            let Some(close) = s.find(')') else { break };
            let inside = &s[1..close];
            let is_pointer_cast = inside.ends_with('*')
                && inside
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '*');
            if !is_pointer_cast {
                break;
            }
            s = s[close + 1..].to_string();
        }
        s
    }

    /// Every value returned by `body`, normalised by `strip_casts`.
    fn returned_values(body: &str) -> Vec<String> {
        let mut values = Vec::new();
        let mut rest = body;
        while let Some(at) = rest.find("return") {
            rest = &rest[at + "return".len()..];
            // `returns_x` is not a return statement.
            if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let Some(semi) = rest.find(';') else { break };
            values.push(strip_casts(&rest[..semi]));
            rest = &rest[semi..];
        }
        values
    }

    /// A Palladium String is a non-NULL `const char*` and the string built-ins
    /// dereference it immediately (`string_len` is `strlen`), so no wrapper may
    /// hand back a null pointer. `read_file_to_string` used to, which turned a
    /// missing file into SIGSEGV.
    ///
    /// SCOPE, stated precisely because the name of the first version of this test
    /// claimed more than it checked: this rejects a returned null *constant* in any
    /// spelling — `NULL`, `0`, `(char*)0`, `(void*)0`. It does **not** prove a
    /// wrapper never returns null at run time. `char* p = NULL; ...; return p;`
    /// returns a variable, and nothing here evaluates it — which is not
    /// hypothetical: `__pd_read_file_to_string` has exactly that shape, returning
    /// `out_str`, which the callee fills in only on success. This is a guard
    /// against the constant coming back, not a proof of non-nullness.
    #[test]
    fn test_no_builtin_wrapper_returns_a_null_constant() {
        const NULL_CONSTANTS: &[&str] = &["NULL", "0", "nullptr"];

        let prelude = emitted_prelude();
        for b in BUILTINS.iter().filter(|b| b.ret == BuiltinType::Str) {
            let needle = format!(" __pd_{}(", b.name);
            let start = prelude
                .find(&needle)
                .unwrap_or_else(|| panic!("no wrapper for {}", b.name));
            let body_start = start + prelude[start..].find('{').expect("wrapper body");
            let end = body_start
                + prelude[body_start..]
                    .find("\n}")
                    .expect("wrapper body ends");
            let body = &prelude[body_start..end];

            for value in returned_values(body) {
                assert!(
                    !NULL_CONSTANTS.contains(&value.as_str()),
                    "__pd_{} returns the null constant `{}`, but its Palladium type \
                     is a non-NULL String:\n{}",
                    b.name,
                    value,
                    body
                );
            }
        }
    }

    /// AN `Owned` WRAPPER MUST NOT RETURN A STRING LITERAL. That is the whole of
    /// what this test checks, and the name says so.
    ///
    /// THE REGRESSION IT PINS, exactly. Measured on `acda322` and every revision
    /// before it: four of the seven `Owned` built-ins returned the literal `""` on
    /// branches the corpus reaches — `file_read_all` with a bad handle,
    /// `file_read_line` at EOF and with a bad handle, `read_file_to_string` on a
    /// missing file, `string_substring` with `start >= end`. A literal is static
    /// storage the built-in did not allocate, so the declaration was false and
    /// `src/ownership/borrow_checker.rs:127` derives the ownership model from it.
    /// They return `__pd_empty_owned()` now. This test exists so those four cannot
    /// come back.
    ///
    /// **WHAT IT DOES NOT CHECK, AND NOTHING ELSE DOES EITHER.** This is a
    /// SYNTACTIC check for one shape. An `Owned` wrapper that returned a
    /// parameter, a static or global buffer, or a borrowed pointer held in a local
    /// would pass it, and no gate in this repository would notice. The property
    /// "every `Owned` return is allocated" is NOT enforced anywhere; only this one
    /// way of violating it is.
    ///
    /// *(An earlier version of this test was named
    /// `test_owned_wrappers_never_return_borrowed_storage` and its comment claimed
    /// a literal was "the one thing an owned return may not be" — a general name
    /// and a false universal over a syntactic check. That is the defect class this
    /// milestone exists to remove, inside the control written to remove it, and it
    /// is fixed by narrowing the claim rather than by widening the code.)*
    ///
    /// WHY NOT WIDEN THE CODE — MEASURED, because "just do the analysis" was the
    /// alternative. Provenance is decidable inside the emitted C for six of the
    /// seven: `__pd_alloc_string` appears in the same function body. It is NOT
    /// decidable for `read_file_to_string`, whose returned `out_str` is filled by
    /// `pd_read_file_to_string`, an out-parameter across the FFI boundary whose
    /// storage comes from `Box::into_raw` in `src/runtime/io.rs:470` — nothing in
    /// the C says so. A provenance checker would therefore need a hand-maintained
    /// table of which `pd_*` runtime functions allocate through out-parameters,
    /// which is a third registry beside this one and `PRELUDE_TYPE_MISMATCHES`,
    /// and a table agreeing with a declaration is the shape that produced the
    /// original defect.
    ///
    /// `arg_at` is the control, and it is why this cannot simply ban the literal
    /// everywhere: it returns `""` out of range ON PURPOSE and declares
    /// `BorrowedStatic`, so the same C is correct there. The filter is the
    /// DECLARATION, which is what makes this a check on the pair rather than on
    /// the text.
    #[test]
    fn test_no_owned_wrapper_returns_a_string_literal() {
        let prelude = emitted_prelude();
        let mut offenders: Vec<String> = Vec::new();

        for b in BUILTINS.iter().filter(|b| b.ret_mode == ReturnMode::Owned) {
            let needle = format!(" __pd_{}(", b.name);
            let start = prelude
                .find(&needle)
                .unwrap_or_else(|| panic!("no wrapper for {}", b.name));
            let body_start = start + prelude[start..].find('{').expect("wrapper body");
            let end = body_start
                + prelude[body_start..]
                    .find("\n}")
                    .expect("wrapper body ends");
            let body = &prelude[body_start..end];

            for value in returned_values(body) {
                // A string literal is ONE way to return storage the callee did
                // not produce — not the only one. `""` is the one that has
                // actually occurred, four times; any literal is rejected. A
                // returned parameter or static buffer is the same defect and is
                // invisible here; see the note above the test.
                if value.starts_with('"') {
                    offenders.push(format!("__pd_{} returns the literal {}", b.name, value));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "a built-in declared ReturnMode::Owned returns a string LITERAL, which \
             is static storage it did not allocate. src/ownership/borrow_checker.rs \
             derives its signatures from this table, so the ownership model is \
             wrong on that branch. (This test sees only literal returns; a returned \
             parameter or static buffer would be the same defect and is not \
             checked by anything.):\n  {}",
            offenders.join("\n  ")
        );
    }

    /// The control for the test above: `arg_at` DOES return a literal, and must,
    /// because it is `BorrowedStatic`. Without this, the literal scan could be
    /// passing because it never finds a literal anywhere — the same "green by
    /// looking at nothing" this milestone exists to remove.
    #[test]
    fn test_the_borrowed_literal_scan_can_still_see_a_literal() {
        let prelude = emitted_prelude();
        let start = prelude.find(" __pd_arg_at(").expect("arg_at wrapper");
        let body_start = start + prelude[start..].find('{').expect("body");
        let end = body_start + prelude[body_start..].find("\n}").expect("body ends");
        let values = returned_values(&prelude[body_start..end]);
        assert!(
            values.iter().any(|v| v.starts_with('"')),
            "arg_at no longer returns a literal, so the scan above has no positive \
             case in the tree and may be vacuous: {:?}",
            values
        );
        assert_eq!(
            lookup("arg_at").expect("arg_at").ret_mode,
            ReturnMode::BorrowedStatic,
            "arg_at returns a literal, so it must not be Owned"
        );
    }

    /// THE BEHAVIOURAL GATE for `Owned`: the ownership pass must actually reason
    /// about a let-bound owned built-in result, on the branch that used to
    /// hand back borrowed storage.
    ///
    /// The two tests above are about the registry and about the emitted C. This
    /// one drives `BorrowChecker::check_program` — the pass that
    /// `src/ownership/borrow_checker.rs:127` builds its signatures from — on real
    /// source that takes each formerly-borrowed branch and then USES the value.
    /// That is the live path a false `Owned` propagates into: it is not a
    /// documentation defect, it is an input to the ownership model.
    ///
    /// What this establishes and what it does not: it proves the pass accepts and
    /// tracks these values, so the consequence path is real and reachable. It
    /// does NOT prove the declaration true — nothing at this level can, because
    /// the pass reads the same declaration. `test_owned_wrappers_never_return_
    /// borrowed_storage` is what makes it true, by reading the C.
    #[test]
    fn test_the_ownership_pass_reasons_about_owned_results_on_the_empty_branch() {
        // Each program takes a branch that returned a static literal before
        // 2026-08-23, stores the result in a `let`, and then consumes it.
        const PROGRAMS: &[(&str, &str)] = &[
            (
                "string_substring, start >= end",
                "fn main() {\n    let s: String = string_substring(\"abc\", 2, 1);\n    print_int(string_len(s));\n}\n",
            ),
            (
                "file_read_all, bad handle",
                "fn main() {\n    let s: String = file_read_all(0);\n    print_int(string_len(s));\n}\n",
            ),
            (
                "file_read_line, bad handle",
                "fn main() {\n    let s: String = file_read_line(-1);\n    print_int(string_len(s));\n}\n",
            ),
            (
                "read_file_to_string, missing file",
                "fn main() {\n    let s: String = read_file_to_string(\"definitely-absent\");\n    print_int(string_len(s));\n}\n",
            ),
        ];

        for (label, source) in PROGRAMS {
            let mut lexer = crate::lexer::Lexer::new(source);
            let tokens = lexer
                .collect_tokens()
                .unwrap_or_else(|e| panic!("{}: lex failed: {:?}", label, e));
            let program = crate::parser::Parser::new(tokens)
                .parse()
                .unwrap_or_else(|e| panic!("{}: parse failed: {:?}", label, e));

            crate::typeck::TypeChecker::new()
                .check(&program)
                .unwrap_or_else(|e| panic!("{}: type check failed: {:?}", label, e));
            crate::ownership::BorrowChecker::new()
                .check_program(&program)
                .unwrap_or_else(|e| {
                    panic!(
                        "{}: the ownership pass rejected a let-bound owned \
                         built-in's result: {:?}",
                        label, e
                    )
                });
        }

        // And the pass really is deriving `Owned` from this table for these names
        // — otherwise the four programs above would prove nothing about ownership.
        let bc = crate::ownership::BorrowChecker::new();
        let registered = bc.registered_function_names();
        for b in BUILTINS.iter().filter(|b| b.ret_mode == ReturnMode::Owned) {
            assert!(
                registered.contains(b.name),
                "{} is Owned but the ownership pass does not know it",
                b.name
            );
        }
    }

    /// The null-constant scan must recognise the spellings it claims to, including
    /// the ones the exact-match version of it missed.
    #[test]
    fn test_null_constant_scan_recognises_every_spelling() {
        for spelling in ["NULL", "0", "(char*)0", "(void*)0", " ( char * ) 0 "] {
            let body = format!("{{\n    return {};\n}}", spelling);
            let expected = if spelling.contains("NULL") { "NULL" } else { "0" };
            assert_eq!(
                returned_values(&body),
                vec![expected.to_string()],
                "null spelling not normalised: {}",
                spelling
            );
        }
        // And it must not mistake ordinary returns for null.
        assert_eq!(
            returned_values("{\n    return result;\n    return \"\";\n}"),
            vec!["result".to_string(), "\"\"".to_string()]
        );
        assert_eq!(
            returned_values("{\n    return (a + b);\n}"),
            vec!["(a+b)".to_string()]
        );
    }

    /// Every built-in with a pinned mismatch must also be marked unsupported, and
    /// every unsupported built-in must have a pinned mismatch. Otherwise the two
    /// records of "this does not work" could drift apart, and the type checker would
    /// let through a call that gcc rejects.
    #[test]
    fn test_unsupported_builtins_are_exactly_the_mismatched_ones() {
        let mismatched: BTreeSet<&str> = PRELUDE_TYPE_MISMATCHES
            .iter()
            .map(|entry| entry.split(' ').next().expect("mismatch entry has a name"))
            .collect();
        let unsupported: BTreeSet<&str> = BUILTINS
            .iter()
            .filter(|b| !b.support.is_callable())
            .map(|b| b.name)
            .collect();
        assert_eq!(
            unsupported, mismatched,
            "Support::Unsupported and PRELUDE_TYPE_MISMATCHES disagree about which \
             built-ins are broken"
        );
    }

    /// The generated header the bootstrap compiler includes must not go stale: it
    /// has to declare the same `__pd_*` signatures the Rust compiler emits.
    /// Regenerate with scripts/gen-prelude.sh.
    #[test]
    fn test_generated_prelude_header_matches_the_compiler() {
        // Reproduce scripts/gen-prelude.sh in process: compile its probe and take
        // everything above the generated `int main(`.
        const PROBE: &str = concat!(
            "fn main() {\n",
            "    let s: String = \"x\";\n",
            "    print(s);\n",
            "    print_int(string_len(s));\n",
            "    print_int(arg_count());\n",
            "}\n"
        );
        let mut lexer = crate::lexer::Lexer::new(PROBE);
        let tokens = lexer.collect_tokens().expect("probe lexes");
        let program = crate::parser::Parser::new(tokens).parse().expect("probe parses");
        let mut generator =
            crate::codegen::CodeGenerator::new("pd_prelude_probe").expect("codegen::new");
        generator.compile(&program).expect("probe compiles");
        let generated = generator.generated_c().to_string();
        let boundary = generated
            .lines()
            .position(|line| line.starts_with("int main("))
            .expect("generated C has an int main(");
        let from_compiler: Vec<&str> = generated.lines().take(boundary).collect();

        let header = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/runtime/pd_prelude.h"
        ))
        .expect("runtime/pd_prelude.h");

        // Strip only the scaffolding gen-prelude.sh wraps around the extract: four
        // comment lines, the include guard, and the closing #endif.
        let from_header: Vec<&str> = header
            .lines()
            .skip_while(|l| l.starts_with("//"))
            .skip_while(|l| l.starts_with("#ifndef") || l.starts_with("#define PD_PRELUDE_H"))
            .take_while(|l| !l.starts_with("#endif"))
            .collect();

        // Whole-text comparison, not just signatures. A change to a wrapper *body*
        // or to a helper leaves every signature intact while making the header
        // behaviourally stale — and the bootstrap compiler #includes this header,
        // so it would be compiling against different code than the Rust compiler
        // emits.
        if from_compiler != from_header {
            let first_diff = from_compiler
                .iter()
                .zip(from_header.iter())
                .position(|(a, b)| a != b);
            let detail = match first_diff {
                Some(i) => format!(
                    "first difference at prelude line {}:\n  compiler: {}\n  header:   {}",
                    i + 1,
                    from_compiler[i],
                    from_header[i]
                ),
                None => format!(
                    "prelude lengths differ: compiler {} lines, header {} lines",
                    from_compiler.len(),
                    from_header.len()
                ),
            };
            panic!(
                "runtime/pd_prelude.h is stale — regenerate with scripts/gen-prelude.sh\n{}",
                detail
            );
        }
    }

    /// The generated user-facing reference must not go stale either, and it had.
    ///
    /// Same shape as `test_generated_prelude_header_matches_the_compiler` above:
    /// a file in the tree is produced from this table by a script, and nothing
    /// noticed when the two stopped agreeing. Measured at `acda322`:
    /// `scripts/gen-builtin-docs.py` read `p(Ty, Mode)` while this table has
    /// written `p("name", Ty, Mode)` since `BuiltinParam` gained its name field,
    /// so it matched 0 of 51 parameters — running the generator turned
    /// `print(String)` into `print()` for every builtin in the reference. The
    /// committed file was correct and OLDER THAN ITS OWN GENERATOR, which is the
    /// one state a "GENERATED — do not edit by hand" banner cannot survive.
    ///
    /// The generator does the comparison (`--check`) rather than this test
    /// re-implementing the rendering: a second renderer here would agree with
    /// the file and disagree with the script, which is the defect one level over.
    #[test]
    fn test_generated_builtin_reference_is_not_stale() {
        use std::process::Command;

        let root = env!("CARGO_MANIFEST_DIR");
        let output = Command::new("python3")
            .arg("scripts/gen-builtin-docs.py")
            .arg("--check")
            .current_dir(root)
            .output()
            .expect("python3 must be available: the gate scripts in this repo are python");

        assert!(
            output.status.success(),
            "docs/reference/builtins.md disagrees with src/builtins.rs — \
             regenerate it with `python3 scripts/gen-builtin-docs.py`:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// THE BEHAVIOURAL GATE for hover: a real hover request over a call to any
    /// built-in must answer with that built-in's derived signature and doc.
    /// Goes red if the handler stops calling `builtin_hover`.
    ///
    /// The document is the same well-typed probe used for effect analysis, because
    /// `get_hover` bails out on a document that did not parse (`src/lsp/hover.rs`,
    /// `doc.ast.as_ref()?`) — the call has to sit inside a real function.
    #[test]
    fn test_lsp_hover_request_answers_for_every_builtin() {
        use crate::lsp::Position;

        for b in BUILTINS {
            // probe_source puts the call on line 1, indented by four spaces.
            let source = probe_source(b);
            let server = server_with(&source);
            let hover = server
                .get_hover(
                    "file:///probe.pd",
                    Position {
                        line: 1,
                        character: 4,
                    },
                )
                .unwrap_or_else(|| {
                    panic!("hover request returned nothing for {}\n{}", b.name, source)
                });
            assert!(
                hover.contents.value.contains(&b.signature()),
                "hover request does not show the derived signature for {}",
                b.name
            );
            assert!(
                hover.contents.value.contains(b.doc),
                "hover request does not show the documentation for {}",
                b.name
            );
        }
    }
}
