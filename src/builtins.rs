// Built-in function registry for Palladium
// "One table to rule them all"
//
// This module is the SINGLE SOURCE OF TRUTH for the compiler's built-in functions.
// Every pass that needs to know about built-ins (type checker, borrow checker, ...)
// must derive its own tables from `BUILTINS` instead of hand-maintaining a copy.
//
// History: the type checker knew 36 built-ins while the borrow checker only knew 25,
// so 11 built-ins (string_len, string_eq, string_char_at, string_from_char,
// char_is_digit, char_is_alpha, char_is_whitespace, file_read_all, file_read_line,
// file_write, panic) type-checked fine but were rejected by the borrow checker with
// "Use of uninitialized value". The tests at the bottom of this file make that class
// of drift impossible to reintroduce.

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

/// One parameter of a built-in: its type plus its ownership mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinParam {
    pub ty: BuiltinType,
    pub mode: ParamMode,
}

/// Shorthand for building a `BuiltinParam` in the const table below.
const fn p(ty: BuiltinType, mode: ParamMode) -> BuiltinParam {
    BuiltinParam { ty, mode }
}

/// A built-in function: name, parameter list, return type and return ownership.
#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub name: &'static str,
    pub params: &'static [BuiltinParam],
    pub ret: BuiltinType,
    pub ret_mode: ReturnMode,
}

use BuiltinType::{Bool, I64, Str, Unit};
use ParamMode::{Borrow, Copy as ByCopy};

/// The canonical list of every built-in function the compiler knows about.
///
/// Adding a built-in here automatically registers it in the type checker and the
/// borrow checker. Adding one anywhere else is a bug the tests below will catch.
pub const BUILTINS: &[Builtin] = &[
    // ---- Output ----
    Builtin {
        name: "print",
        // NOTE: `print` deliberately treats its String argument as Copy (historic
        // behavior of the borrow checker); it neither moves nor borrows the value.
        params: &[p(Str, ByCopy)],
        ret: Unit,
        ret_mode: ReturnMode::Unit,
    },
    Builtin {
        name: "print_int",
        params: &[p(I64, ByCopy)],
        ret: Unit,
        ret_mode: ReturnMode::Unit,
    },
    Builtin {
        name: "panic",
        params: &[p(Str, Borrow)],
        ret: Unit,
        ret_mode: ReturnMode::Unit,
    },
    // ---- String manipulation ----
    Builtin {
        name: "string_len",
        params: &[p(Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "string_concat",
        params: &[p(Str, Borrow), p(Str, Borrow)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
    },
    Builtin {
        name: "string_eq",
        params: &[p(Str, Borrow), p(Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "string_char_at",
        params: &[p(Str, Borrow), p(I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "string_substring",
        params: &[p(Str, Borrow), p(I64, ByCopy), p(I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
    },
    Builtin {
        name: "string_from_char",
        params: &[p(I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
    },
    Builtin {
        name: "string_to_int",
        params: &[p(Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "int_to_string",
        params: &[p(I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
    },
    // ---- Character classification ----
    Builtin {
        name: "char_is_digit",
        params: &[p(I64, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "char_is_alpha",
        params: &[p(I64, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "char_is_whitespace",
        params: &[p(I64, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
    },
    // ---- Command-line arguments ----
    Builtin {
        // Number of command-line arguments, argv[0] included (matches C's argc).
        name: "arg_count",
        params: &[],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        // Argument `i`; returns "" when out of range, never NULL, because every
        // string built-in assumes a non-NULL `const char*`.
        name: "arg_at",
        params: &[p(I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
    },
    // ---- File I/O ----
    Builtin {
        name: "file_open",
        params: &[p(Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "file_read_all",
        params: &[p(I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
    },
    Builtin {
        name: "file_read_line",
        params: &[p(I64, ByCopy)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
    },
    Builtin {
        name: "file_write",
        params: &[p(I64, ByCopy), p(Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "file_close",
        params: &[p(I64, ByCopy)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "file_exists",
        params: &[p(Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
    },
    // ---- Enhanced I/O ----
    Builtin {
        name: "path_exists",
        params: &[p(Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "path_is_file",
        params: &[p(Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "path_is_dir",
        params: &[p(Str, Borrow)],
        ret: Bool,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "create_dir",
        params: &[p(Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "create_dir_all",
        params: &[p(Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "remove_file",
        params: &[p(Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "remove_dir",
        params: &[p(Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "remove_dir_all",
        params: &[p(Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "read_file_to_string",
        params: &[p(Str, Borrow)],
        ret: Str,
        ret_mode: ReturnMode::Owned,
    },
    Builtin {
        name: "write_string_to_file",
        params: &[p(Str, Borrow), p(Str, Borrow)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "file_flush",
        params: &[p(I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "file_seek",
        params: &[p(I64, ByCopy), p(I64, ByCopy), p(I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    // ---- Enhanced file operations with mode support ----
    Builtin {
        name: "file_open_ex",
        params: &[p(Str, Borrow), p(I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "file_close_ex",
        params: &[p(I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "file_read_ex",
        params: &[p(I64, ByCopy), p(Str, ByCopy), p(I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
    },
    Builtin {
        name: "file_write_ex",
        params: &[p(I64, ByCopy), p(Str, ByCopy), p(I64, ByCopy)],
        ret: I64,
        ret_mode: ReturnMode::Copy,
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
