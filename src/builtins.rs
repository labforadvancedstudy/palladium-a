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
    /// Returns a copy value (primitives)
    Copy,
    /// No return value
    Unit,
}

/// One parameter of a built-in: its name, its type and its ownership mode.
///
/// The name carries no semantics for any compiler pass; it exists so that the LSP
/// can render `fn print(s: String)` instead of `fn print(String)` without a
/// hand-written signature string. Names match the C implementation in
/// `src/codegen/mod.rs` (e.g. `__pd_file_seek(handle, whence, offset)`).
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

/// A built-in function: name, parameter list, return type, return ownership,
/// effects and documentation.
#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub name: &'static str,
    pub params: &'static [BuiltinParam],
    pub ret: BuiltinType,
    pub ret_mode: ReturnMode,
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
use BuiltinType::{Bool, I64, Str, Unit};
use ParamMode::{Borrow, Copy as ByCopy};

/// No effects: calling this built-in leaves the outside world alone.
const PURE: &[Effect] = &[];
/// Reads or writes the outside world (file system, console, process arguments).
const IO: &[Effect] = &[Effect::IO];
/// Writes to the outside world *and* aborts the process.
const IO_PANIC: &[Effect] = &[Effect::IO, Effect::Panic];

/// The canonical list of every built-in function the compiler knows about.
///
/// Adding a built-in here automatically registers it in the type checker, the
/// borrow checker, the effect analyzer and the LSP. Adding one anywhere else is a
/// bug the tests below will catch.
///
/// Effect classification follows the implementation emitted by `src/codegen/mod.rs`:
/// anything that touches a file, the console or the process argument vector is
/// `Effect::IO`; the string and character built-ins are pure. Note that several
/// "pure" built-ins (string_concat, int_to_string, ...) do allocate; the effect
/// system has an `Effect::Memory` but has never attributed it to a built-in
/// ("For now, we don't have explicit allocation functions" — the deleted table in
/// effects/mod.rs), and this consolidation deliberately did not change that.
pub const BUILTINS: &[Builtin] = &[
    // ---- Output ----
    Builtin {
        name: "print",
        // NOTE: `print` deliberately treats its String argument as Copy (historic
        // behavior of the borrow checker); it neither moves nor borrows the value.
        params: &[p("s", Str, ByCopy)],
        ret: Unit,
        ret_mode: ReturnMode::Unit,
        effects: IO,
        doc: "Print a string to stdout",
    },
    Builtin {
        name: "print_int",
        params: &[p("n", I64, ByCopy)],
        ret: Unit,
        ret_mode: ReturnMode::Unit,
        effects: IO,
        doc: "Print an integer to stdout",
    },
    Builtin {
        name: "panic",
        params: &[p("msg", Str, Borrow)],
        ret: Unit,
        ret_mode: ReturnMode::Unit,
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
        effects: PURE,
        doc: "Get the length of a string",
    },
    Builtin {
        name: "string_concat",
        params: &[p("a", Str, Borrow), p("b", Str, Borrow)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        effects: PURE,
        doc: "Concatenate two strings",
    },
    Builtin {
        name: "string_eq",
        params: &[p("s1", Str, Borrow), p("s2", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        effects: PURE,
        doc: "Compare two strings for equality",
    },
    Builtin {
        name: "string_char_at",
        params: &[p("s", Str, Borrow), p("index", I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
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
        effects: PURE,
        doc: "Extract the substring in [start, end)",
    },
    Builtin {
        name: "string_from_char",
        params: &[p("c", I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        effects: PURE,
        doc: "Build a one-character string from a character code",
    },
    Builtin {
        name: "string_to_int",
        params: &[p("s", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: PURE,
        doc: "Parse an integer from a string",
    },
    Builtin {
        name: "int_to_string",
        params: &[p("n", I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        effects: PURE,
        doc: "Convert an integer to a string",
    },
    // ---- Character classification ----
    Builtin {
        name: "char_is_digit",
        params: &[p("c", I64, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        effects: PURE,
        doc: "Is this character code a decimal digit?",
    },
    Builtin {
        name: "char_is_alpha",
        params: &[p("c", I64, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        effects: PURE,
        doc: "Is this character code a letter?",
    },
    Builtin {
        name: "char_is_whitespace",
        params: &[p("c", I64, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
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
        ret_mode: ReturnMode::Owned,
        effects: IO,
        doc: "Command-line argument `i`, or \"\" when out of range",
    },
    // ---- File I/O ----
    Builtin {
        name: "file_open",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Open a file and return a handle",
    },
    Builtin {
        name: "file_read_all",
        params: &[p("handle", I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        effects: IO,
        doc: "Read a whole open file into a string",
    },
    Builtin {
        name: "file_read_line",
        params: &[p("handle", I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        effects: IO,
        doc: "Read one line from an open file",
    },
    Builtin {
        name: "file_write",
        params: &[p("handle", I64, ByCopy), p("content", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Write a string to an open file",
    },
    Builtin {
        name: "file_close",
        params: &[p("handle", I64, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Close an open file handle",
    },
    Builtin {
        name: "file_exists",
        params: &[p("path", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Does this file exist?",
    },
    // ---- Enhanced I/O ----
    Builtin {
        name: "path_exists",
        params: &[p("path", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Does this path exist?",
    },
    Builtin {
        name: "path_is_file",
        params: &[p("path", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Is this path a regular file?",
    },
    Builtin {
        name: "path_is_dir",
        params: &[p("path", Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Is this path a directory?",
    },
    Builtin {
        name: "create_dir",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Create a directory",
    },
    Builtin {
        name: "create_dir_all",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Create a directory and every missing parent",
    },
    Builtin {
        name: "remove_file",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Delete a file",
    },
    Builtin {
        name: "remove_dir",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Delete an empty directory",
    },
    Builtin {
        name: "remove_dir_all",
        params: &[p("path", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Delete a directory and its contents",
    },
    Builtin {
        name: "read_file_to_string",
        params: &[p("path", Str, Borrow)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
        effects: IO,
        doc: "Read a file at `path` into a string",
    },
    Builtin {
        name: "write_string_to_file",
        params: &[p("path", Str, Borrow), p("data", Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Write a string to the file at `path`",
    },
    Builtin {
        name: "file_flush",
        params: &[p("handle", I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Flush buffered writes on an open file",
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
        effects: IO,
        doc: "Move the read/write position of an open file",
    },
    // ---- Enhanced file operations with mode support ----
    Builtin {
        name: "file_open_ex",
        params: &[p("path", Str, Borrow), p("mode", I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Open a file with an explicit mode and return a handle",
    },
    Builtin {
        name: "file_close_ex",
        params: &[p("handle", I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Close a handle opened with file_open_ex",
    },
    Builtin {
        name: "file_read_ex",
        params: &[
            p("handle", I64, ByCopy),
            p("buffer", Str, ByCopy),
            p("len", I64, ByCopy),
        ],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Read up to `len` bytes from an open file into `buffer`",
    },
    Builtin {
        name: "file_write_ex",
        params: &[
            p("handle", I64, ByCopy),
            p("buffer", Str, ByCopy),
            p("len", I64, ByCopy),
        ],
        ret: I64,
        ret_mode: ReturnMode::Copy,
        effects: IO,
        doc: "Write `len` bytes from `buffer` to an open file",
    },
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
                _ => {}
            }
        }
    }

    /// The canonical names, owned, for comparison against a pass's registry.
    fn canonical() -> BTreeSet<String> {
        BUILTINS.iter().map(|b| b.name.to_string()).collect()
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

    /// The 19 built-ins that used to be missing from the effect analyzer. 18 of
    /// them do file or console I/O and were analyzed as pure.
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
            "file_open_ex",
            "file_close_ex",
            "file_read_ex",
            "file_write_ex",
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
            canonical(),
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
}
