// Error handling for Palladium compiler
// "Even legends make mistakes, but they handle them gracefully"

use thiserror::Error;

pub mod pretty;
pub mod reporter;
pub mod suggestions;

pub type Result<T> = std::result::Result<T, CompileError>;

#[derive(Error, Debug)]
pub enum CompileError {
    // Lexer errors
    #[error("Unexpected character '{ch}' at line {line}, column {col}")]
    UnexpectedChar {
        ch: char,
        line: usize,
        col: usize,
        span: Option<Span>,
    },

    #[error("Unterminated string literal at line {line}")]
    UnterminatedString { line: usize, span: Option<Span> },

    // Parser errors
    #[error("Unexpected token: expected {expected}, found {found}")]
    UnexpectedToken {
        expected: String,
        found: String,
        span: Option<Span>,
    },

    #[error("Syntax error: {message}")]
    SyntaxError { message: String, span: Option<Span> },

    // Type errors
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch {
        expected: String,
        found: String,
        span: Option<Span>,
    },

    #[error("Undefined variable: {name}")]
    UndefinedVariable { name: String, span: Option<Span> },

    #[error("Undefined function: {name}")]
    UndefinedFunction { name: String, span: Option<Span> },

    #[error("Function {name} expects {expected} arguments, but {found} were provided")]
    ArgumentCountMismatch {
        name: String,
        expected: usize,
        found: usize,
        span: Option<Span>,
    },

    /// A built-in that `crate::builtins` describes but marks
    /// `Support::Unsupported`: the registry knows its signature, and the runtime
    /// cannot honour a call to it. Reported here, at type-check time, instead of
    /// letting the program reach gcc and fail there.
    #[error("Built-in {name} is registered but not callable: {reason}")]
    UnsupportedBuiltin {
        name: String,
        reason: String,
        span: Option<Span>,
    },

    // Codegen errors
    #[error("Code generation failed: {message}")]
    CodegenError { message: String },

    // A construct the parser accepts but that the selected backend cannot lower.
    // Emitting approximate code for these is how a compiler starts lying: the
    // program either fails inside generated C the user never wrote, or — worse
    // — links and runs with the wrong semantics. Refusing at the construct's
    // own span is the honest answer until the feature lands.
    //
    // "The selected backend", not "no backend": the LLVM backend raises these
    // too, and its `workaround` is to use the C backend, which lowers the
    // construct fine. The two backends do not have the same gaps, and a comment
    // claiming otherwise would be the same kind of false statement this variant
    // exists to remove.
    //
    // These are raised *before* the operand is examined, so `consequence` and
    // `workaround` must hold for every operand the construct can be written
    // with. Advice that only fits the shape the old type rules used to require
    // is advice that is wrong for `3?`.
    #[error("{construct} is not implemented")]
    Unimplemented {
        /// How the construct is written in source, e.g. "the `?` operator".
        construct: String,
        /// What would happen if the compiler kept pretending.
        consequence: String,
        /// What the programmer can do today instead. Must be true for any
        /// operand, and must name its own limits rather than imply generality.
        workaround: String,
        /// `None` when the refusal is a property of the backend rather than of
        /// any one line of source.
        span: Option<Span>,
    },

    // IO errors
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    // Generic error
    #[error("{0}")]
    Generic(String),

    // Missing semicolon
    #[error("Missing semicolon after statement")]
    MissingSemicolon { span: Option<Span> },

    // Invalid function signature
    #[error("Invalid function signature")]
    InvalidFunctionSignature { message: String, span: Option<Span> },

    // Borrow checker errors
    #[error("Borrow checker error: {message}")]
    BorrowChecker { message: String, span: Option<Span> },

    #[error("Use of moved value: {name}")]
    UseOfMovedValue { name: String, span: Option<Span> },

    #[error("Use of uninitialized value: {name}")]
    UseOfUninitializedValue { name: String, span: Option<Span> },

    #[error("Cannot move out of borrowed content")]
    CannotMoveOutOfBorrowedContent { span: Option<Span> },

    // Unsafe operation errors
    #[error("Unsafe operation '{operation}' requires unsafe block")]
    UnsafeOperation { operation: String, span: Span },

    #[error("Conflicting borrows: {message}")]
    ConflictingBorrows { message: String, span: Option<Span> },

    #[error("Lifetime error: {message}")]
    LifetimeError { message: String, span: Option<Span> },

    // Pattern matching errors
    #[error("Non-exhaustive match: missing patterns {}", missing_patterns.join(", "))]
    NonExhaustiveMatch {
        missing_patterns: Vec<String>,
        span: Option<Span>,
    },

    #[error("Unreachable pattern: {}", patterns.join(", "))]
    UnreachablePattern {
        patterns: Vec<String>,
        span: Option<Span>,
    },
}

/// Source location information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }

    pub fn dummy() -> Self {
        Self {
            start: 0,
            end: 0,
            line: 0,
            column: 0,
        }
    }

    pub fn extend_to(&self, other: &Span) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line.min(other.line),
            column: if self.line < other.line {
                self.column
            } else {
                self.column.min(other.column)
            },
        }
    }
}

impl CompileError {
    /// D5: `?` parses, but nothing lowers it.
    ///
    /// Note what is and is not missing. `Result` is not a missing *type* —
    /// a user can declare one, and before this refusal existed that is exactly
    /// how a program reached code generation. The absent piece is a lowering of
    /// `?` onto the enum representation the compiler actually emits (`.tag`
    /// plus `__Enum__Variant` constants); it emitted a `struct Result { int
    /// is_ok; union … }` layout that nothing else produces, so the program died
    /// inside gcc.
    ///
    /// Raised without inspecting the operand, so the wording may not assume one
    /// — `3?` and `unknown()?` reach here too. It also may not imply that the
    /// `match` alternative generalises further than it does: code generation
    /// skips generic enum definitions entirely (`src/codegen/mod.rs:1244-1245`,
    /// `src/codegen/mod.rs:1244-1245`, `src/codegen/mod.rs:1244-1245`), so `Result<T, E>` is
    /// not a compilable replacement and the help says so rather than leaving the
    /// reader to discover it.
    ///
    /// Every clause is receipted in `tests/d5_unimplemented_constructs.rs`:
    /// `question_workaround_compiles_and_runs` (dispatch),
    /// `question_workaround_propagates_out_of_a_helper` (propagation),
    /// `question_workaround_is_not_limited_to_i64_payloads` (payload types),
    /// and `generic_result_is_not_a_compilable_workaround` (the warned limit).
    pub fn question_unimplemented(span: Span) -> Self {
        CompileError::Unimplemented {
            construct: "the `?` operator".to_string(),
            consequence:
                "code generation has no lowering of `?` onto the enum representation it emits, \
                 and would instead produce C for a `struct Result { int is_ok; union … }` layout \
                 that no enum is ever generated as"
                    .to_string(),
            workaround:
                "there is no error-propagation operator; return the value and dispatch on it with \
                 `match`. Only non-generic enums are compiled, so declare a concrete one such as \
                 `enum Result { Ok(i64), Err(i64) }` — `Result<T, E>` will not compile"
                    .to_string(),
            span: Some(span),
        }
    }

    /// A value-carrying `return` inside an `async fn` has nowhere to put its
    /// value.
    ///
    /// MEASURED at 7d2fc0d. The type space is wider than the spellings:
    /// typeck gives an async function the return type `Future<declared>`, and
    /// an ORDINARY function may declare `Future<()>`, so
    ///
    /// ```text
    /// fn g() -> Future<()> { panic("x"); }
    /// async fn f() -> () { g() }
    /// ```
    ///
    /// type-checks. The parser lowers that tail to `Stmt::Return(Some(..))`,
    /// and the poll function it lands in returns an `int` readiness flag with
    /// no slot for a value — so the value was evaluated and DROPPED, and the
    /// emitted C carried a duplicate `return 1; // Ready`. The same shape with
    /// a non-unit output (`async fn f() -> i64 { g() }`, `g() -> Future<i64>`)
    /// emitted `return <struct>;` from an `int` function and gcc refused it.
    ///
    /// Refused rather than lowered, for the same reason `async fn main` is:
    /// giving the value somewhere to live means a future with a result slot
    /// and something to drive it, which is the async runtime §N7 says does not
    /// exist. The async producer as a whole is recorded as a normative
    /// violation of §N7 owned by M2; this removes the silent value-drop
    /// underneath it.
    pub fn async_value_return_unimplemented(span: Span) -> Self {
        CompileError::Unimplemented {
            construct: "a `return` with a value inside an `async fn`".to_string(),
            consequence:
                "the body is emitted into a poll function that returns only an `int` \
                 readiness flag, so there is nowhere to put the value: it would be \
                 evaluated and discarded, and for a non-unit output the emitted C does \
                 not compile at all"
                    .to_string(),
            workaround:
                "make the function ordinary (`fn`) and return the value directly. There \
                 is no async runtime, so a future's result has nowhere to live and \
                 nothing to deliver it"
                    .to_string(),
            span: Some(span),
        }
    }

    /// The same refusal for a SET of offending imported declarations.
    ///
    /// `CompileError` carries one span, so a compiler that finds several of
    /// these has a choice: report one and drop the rest, or report one that
    /// NAMES the rest. Reporting one and dropping the rest is what the type
    /// checker used to do, and because its input arrived in `HashMap` order the
    /// survivor was not even a function of the program. This constructor takes
    /// offenders the caller has already put in a deterministic order, points the
    /// span at the first, and lists every name in `construct`, so a second
    /// offender is visible without a second compile.
    ///
    /// The leading text is byte-identical to
    /// `async_value_return_unimplemented` because it is the same refusal for the
    /// same reason; only the list of locations differs. `consequence` and
    /// `workaround` are unchanged and remain true of each named declaration
    /// individually, which is the condition this variant's doc comment puts on
    /// advice raised before the operand is examined.
    ///
    /// # Panics
    /// If `offenders` is empty — an error that names no offender is a claim with
    /// no referent, and every caller already knows its list is non-empty.
    pub fn async_value_return_unimplemented_in_imports(offenders: &[(String, Span)]) -> Self {
        let (_, first_span) = offenders
            .first()
            .expect("a refusal must name at least one offending declaration");
        let names = offenders
            .iter()
            .map(|(name, _)| format!("`{}`", name))
            .collect::<Vec<_>>()
            .join(", ");
        let mut err = Self::async_value_return_unimplemented(*first_span);
        if let CompileError::Unimplemented { construct, .. } = &mut err {
            *construct = format!("{} (imported: {})", construct, names);
        }
        err
    }

    /// `async fn main` compiles to an entry point nothing can call.
    ///
    /// MEASURED at d0eebbf: `async fn main() { print_int(7) }` produced no
    /// diagnostic, compiled, linked, ran and exited 0 — having printed nothing.
    /// The emitted entry point was
    ///
    /// ```text
    /// main_Future main() { … }        // not int main(int, char**)
    /// int main_poll(main_Future *f) { … }
    /// ```
    ///
    /// so the body sits inside a poll function that nobody calls, and the C
    /// runtime's entry point returns a struct. A program that compiles, links,
    /// runs, exits 0 and does nothing is the D3/D3b family at the entry point.
    ///
    /// WHY THIS IS A REFUSAL AND NOT A FIX. Making it work needs an async
    /// runtime to drive the future, and the specification says there is none
    /// (§N7). This compiler's rule when it cannot honour a construct is to
    /// refuse it with the reason and a workaround — `?`, `.await` and the LLVM
    /// backend are all refused that way — not to emit something that looks
    /// like a program.
    ///
    /// The ANNOTATED form was already rejected, by accident rather than by
    /// design: `async fn main() -> ()` fails as "Type mismatch: expected
    /// Future<()>, found ()" because typeck wraps the declared return type in
    /// `Future`. With no annotation there is nothing to mismatch, which is why
    /// only this spelling reached code generation.
    pub fn async_main_unimplemented(span: Span) -> Self {
        CompileError::Unimplemented {
            construct: "`async fn main`".to_string(),
            consequence:
                "the entry point would be emitted as `main_Future main()` rather than \
                 `int main(int, char**)`, with the body inside a `main_poll` function that \
                 nothing calls — so the program links, runs, exits 0 and does nothing"
                    .to_string(),
            workaround:
                "make `main` an ordinary function: `fn main() { … }`. There is no async \
                 runtime to drive a future returned from the entry point, so nothing else \
                 can give it its meaning"
                    .to_string(),
            span: Some(span),
        }
    }

    /// D5: `.await` parses, but nothing lowers it.
    ///
    /// Raised without inspecting the operand, so the advice is phrased for any
    /// of them — `some_variable.await` reaches here as readily as a call does,
    /// and telling its author to "change the function's return type" would name
    /// a function that is not there.
    ///
    /// Where a `-> Future<T>` signature *is* involved, the fix has to change it
    /// rather than just drop the `.await`: dropping it leaves a `Future<T>`
    /// where a `T` is required ("Type mismatch: expected Int, found
    /// Future<Int>", measured). Suggesting the shorter edit would repeat the
    /// defect this diagnostic exists to remove, which is why
    /// `deleting_the_await_alone_does_not_compile` guards against it.
    ///
    /// Receipted by `await_workaround_compiles_and_runs`.
    pub fn await_unimplemented(span: Span) -> Self {
        CompileError::Unimplemented {
            construct: "`.await`".to_string(),
            consequence:
                "there is no async runtime, and code generation would emit a call to a `poll` \
                 member that no generated C struct has"
                    .to_string(),
            workaround:
                "nothing can be awaited; write the computation as an ordinary synchronous call. \
                 If a function is declared `-> Future<T>`, change it to `-> T` — deleting \
                 `.await` on its own leaves a Future where a value is required"
                    .to_string(),
            span: Some(span),
        }
    }

    /// D3b: a function body ends in an `if`/`match` that produces a value on
    /// some paths and nothing on others.
    ///
    /// The lowering added for D3b turns the tail expression of every branch
    /// into a `return`. It can only do that when every branch HAS one. When one
    /// does not — most often because the `if` has no `else` — there is nothing
    /// to return on that path, and code generation used to emit a non-void
    /// function that simply falls off its end. Measured before this refusal
    /// existed: `fn f(n: i64) -> i64 { if n > 0 { n } }` compiled clean, exit 0,
    /// no diagnostic, and `f(3)` printed 8261746944.
    ///
    /// Why this is a refusal and not a silent fallthrough: the branch tail is
    /// unambiguous evidence of what the programmer meant. They wrote a value in
    /// the position that *is* the function's value. Guessing zero, or letting
    /// the C fall through, is the compiler answering a question it was not
    /// asked.
    ///
    /// Nothing that used to work is lost. To reach here, a program must already
    /// contain a tail expression inside a branch of a tail `if`/`match`, and
    /// every such program miscompiled before this change — there is no correct
    /// behaviour to regress.
    ///
    /// Both workarounds are receipted as programs that compile AND run in
    /// `tests/d3b_tail_if.rs`: `no_else_workaround_else_branch_compiles_and_runs`
    /// and `no_else_workaround_explicit_returns_compiles_and_runs`.
    pub fn tail_value_not_on_every_path(keyword: &str, missing: &str, span: Span) -> Self {
        CompileError::Unimplemented {
            // Phrased without an article before the keyword: the message is
            // built for both `if` and `match`, and "a `if`" is what a format
            // string with a hardcoded article produces.
            construct: format!(
                "a tail `{}` that produces a value on some paths but not all",
                keyword
            ),
            consequence: format!(
                "the tail expression in a branch is the function's return value, but {} has no \
                 value to return; code generation would emit a non-void function that falls off \
                 its end and yields whatever happens to be in the return register",
                missing
            ),
            workaround:
                "give every path a value — add the missing `else`/arm with its own tail \
                 expression, as in `if n <= 1 { n } else { n * 2 }` — or drop tail position \
                 entirely and write explicit returns, as in `if n <= 1 { return n; } return n * 2;`"
                    .to_string(),
            span: Some(span),
        }
    }

    /// Convert this error into a diagnostic with helpful suggestions
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            CompileError::UnexpectedChar {
                ch,
                line,
                col,
                span,
            } => Diagnostic::error(format!(
                "Unexpected character '{}' at line {}, column {}",
                ch, line, col
            ))
            .with_span(span.unwrap_or(Span::new(0, 1, *line, *col)))
            .with_note("Palladium only allows ASCII letters, numbers, and common symbols")
            .with_suggestion("Remove or replace this character with a valid one", None),

            CompileError::UnterminatedString { line, span } => {
                Diagnostic::error(format!("Unterminated string literal at line {}", line))
                    .with_span(span.unwrap_or(Span::new(0, 0, *line, 1)))
                    .with_note("Strings must be closed with a matching quote")
                    .with_suggestion(
                        "Add a closing quote (\") to end the string",
                        Some("\"".to_string()),
                    )
            }

            CompileError::UnexpectedToken {
                expected,
                found,
                span,
            } => Diagnostic::error(format!("Expected {}, but found {}", expected, found))
                .with_span(span.unwrap_or(Span::dummy()))
                .with_note("The syntax requires a specific token here")
                .with_suggestion(
                    format!("Replace '{}' with '{}'", found, expected),
                    Some(expected.clone()),
                ),

            CompileError::SyntaxError { message, span } => Diagnostic::error(message.clone())
                .with_span(span.unwrap_or(Span::dummy()))
                .with_note("Check the language syntax rules"),

            CompileError::TypeMismatch {
                expected,
                found,
                span,
            } => {
                let mut diag = Diagnostic::error(format!(
                    "Type mismatch: expected {}, found {}",
                    expected, found
                ))
                .with_span(span.unwrap_or(Span::dummy()))
                .with_note("Types must match exactly in Palladium");

                // Add specific suggestions based on common type mismatches
                match (expected.as_str(), found.as_str()) {
                    ("int", "string") => {
                        diag = diag.with_suggestion(
                            "Convert the string to an integer using parse_int()",
                            None,
                        );
                    }
                    ("string", "int") => {
                        diag = diag.with_suggestion(
                            "Convert the integer to a string using to_string()",
                            None,
                        );
                    }
                    ("bool", _) => {
                        diag =
                            diag.with_suggestion("Use 'true' or 'false' for boolean values", None);
                    }
                    _ => {}
                }

                diag
            }

            CompileError::UndefinedVariable { name, span } => {
                Diagnostic::error(format!("Undefined variable: {}", name))
                    .with_span(span.unwrap_or(Span::dummy()))
                    .with_note("Variables must be declared before use")
                    .with_suggestion(
                        format!("Did you mean to declare it? Try: let {} = ...;", name),
                        None,
                    )
                    .with_context_lines(3)
            }

            CompileError::UndefinedFunction { name, span } => {
                let mut diag = Diagnostic::error(format!("Undefined function: {}", name))
                    .with_span(span.unwrap_or(Span::dummy()))
                    .with_note("Functions must be defined before they are called");

                // Suggest common function names if they're close
                match name.as_str() {
                    "print" => {
                        diag = diag.with_suggestion(
                            "Did you mean 'println'? The print function is called 'println' in Palladium",
                            Some("println".to_string())
                        );
                    }
                    "printf" => {
                        diag = diag.with_suggestion(
                            "Palladium uses 'println' instead of 'printf'",
                            Some("println".to_string()),
                        );
                    }
                    _ => {
                        diag = diag.with_suggestion(
                            format!("Define the function first: fn {}() {{ ... }}", name),
                            None,
                        );
                    }
                }

                diag
            }

            CompileError::ArgumentCountMismatch {
                name,
                expected,
                found,
                span,
            } => {
                let mut diag = Diagnostic::error(format!(
                    "Function '{}' expects {} argument{}, but {} {} provided",
                    name,
                    expected,
                    if *expected == 1 { "" } else { "s" },
                    found,
                    if *found == 1 { "was" } else { "were" }
                ))
                .with_span(span.unwrap_or(Span::dummy()));

                if *found < *expected {
                    diag = diag.with_suggestion(
                        format!(
                            "Add {} more argument{}",
                            expected - found,
                            if expected - found == 1 { "" } else { "s" }
                        ),
                        None,
                    );
                } else {
                    diag = diag.with_suggestion(
                        format!(
                            "Remove {} argument{}",
                            found - expected,
                            if found - expected == 1 { "" } else { "s" }
                        ),
                        None,
                    );
                }

                diag
            }

            CompileError::MissingSemicolon { span } => {
                Diagnostic::error("Missing semicolon after statement")
                    .with_span(span.unwrap_or(Span::dummy()))
                    .with_note("Every statement in Palladium must end with a semicolon")
                    .with_suggestion(
                        "Add a semicolon (;) at the end of this line",
                        Some(";".to_string()),
                    )
            }

            CompileError::InvalidFunctionSignature { message, span } => Diagnostic::error(format!(
                "Invalid function signature: {}",
                message
            ))
            .with_span(span.unwrap_or(Span::dummy()))
            .with_note(
                "Function signatures must follow the pattern: fn name(param: Type) -> ReturnType",
            )
            .with_suggestion(
                "Example: fn add(x: int, y: int) -> int { return x + y; }",
                None,
            ),

            CompileError::NonExhaustiveMatch {
                missing_patterns,
                span,
            } => {
                let mut diag = Diagnostic::error("Non-exhaustive match expression")
                    .with_span(span.unwrap_or(Span::dummy()))
                    .with_note("All possible patterns must be covered in a match expression");

                if missing_patterns.len() == 1 {
                    diag = diag.with_suggestion(
                        format!("Add a pattern for: {}", missing_patterns[0]),
                        None,
                    );
                } else if missing_patterns.len() <= 3 {
                    diag = diag.with_suggestion(
                        format!("Add patterns for: {}", missing_patterns.join(", ")),
                        None,
                    );
                } else {
                    diag = diag.with_suggestion(
                        "Add remaining patterns or use a wildcard pattern (_) to match all other cases",
                        None
                    );
                }

                diag
            }

            CompileError::UnreachablePattern { patterns: _, span } => Diagnostic::error(
                "Unreachable pattern detected",
            )
            .with_span(span.unwrap_or(Span::dummy()))
            .with_note(
                "This pattern can never be matched because previous patterns cover all cases",
            )
            .with_suggestion("Remove this pattern or reorder the patterns", None),

            CompileError::Unimplemented {
                construct,
                consequence,
                workaround,
                span,
            } => {
                let diag = Diagnostic::error(format!("{} is not implemented", construct))
                    .with_note(consequence.clone())
                    .with_suggestion(workaround.clone(), None)
                    .with_context_lines(1);
                // Only claim a location when there is one. The arms above
                // default a missing span to `Span::dummy()`, which the reporter
                // renders as `--> file.pd:0:0` — a line and column that do not
                // exist. For a refusal that is a property of the whole backend
                // rather than of any one construct, inventing a source position
                // is a small instance of exactly the fabrication these
                // diagnostics exist to remove.
                match span {
                    Some(s) => diag.with_span(*s),
                    None => diag,
                }
            }

            _ => {
                // Default diagnostic for other errors
                Diagnostic::error(self.to_string())
            }
        }
    }
}

/// A diagnostic message with source location
#[derive(Debug)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub span: Option<Span>,
    pub notes: Vec<String>,
    pub suggestions: Vec<Suggestion>,
    pub context_lines: usize, // Number of lines to show before/after the error
}

/// A suggestion for fixing an error
#[derive(Debug)]
pub struct Suggestion {
    pub message: String,
    pub replacement: Option<String>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Copy)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Info,
    Help,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            message: message.into(),
            span: None,
            notes: Vec::new(),
            suggestions: Vec::new(),
            context_lines: 2,
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_suggestion(
        mut self,
        message: impl Into<String>,
        replacement: Option<String>,
    ) -> Self {
        self.suggestions.push(Suggestion {
            message: message.into(),
            replacement,
            span: self.span,
        });
        self
    }

    pub fn with_context_lines(mut self, lines: usize) -> Self {
        self.context_lines = lines;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_new() {
        let span = Span::new(10, 20, 5, 3);
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 20);
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 3);
    }

    #[test]
    fn test_span_dummy() {
        let span = Span::dummy();
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 0);
        assert_eq!(span.line, 0);
        assert_eq!(span.column, 0);
    }

    #[test]
    fn test_span_extend_to() {
        let span1 = Span::new(10, 20, 5, 3);
        let span2 = Span::new(15, 25, 6, 5);
        let extended = span1.extend_to(&span2);
        
        assert_eq!(extended.start, 10);
        assert_eq!(extended.end, 25);
        assert_eq!(extended.line, 5);
        assert_eq!(extended.column, 3);
    }

    #[test]
    fn test_span_extend_to_same_line() {
        let span1 = Span::new(10, 20, 5, 10);
        let span2 = Span::new(5, 15, 5, 5);
        let extended = span1.extend_to(&span2);
        
        assert_eq!(extended.start, 5);
        assert_eq!(extended.end, 20);
        assert_eq!(extended.line, 5);
        assert_eq!(extended.column, 5);
    }

    #[test]
    fn test_compile_error_display() {
        let err = CompileError::UnexpectedChar {
            ch: '$',
            line: 10,
            col: 5,
            span: None,
        };
        assert_eq!(err.to_string(), "Unexpected character '$' at line 10, column 5");

        let err = CompileError::UnterminatedString {
            line: 42,
            span: None,
        };
        assert_eq!(err.to_string(), "Unterminated string literal at line 42");

        let err = CompileError::TypeMismatch {
            expected: "int".to_string(),
            found: "string".to_string(),
            span: None,
        };
        assert_eq!(err.to_string(), "Type mismatch: expected int, found string");
    }

    #[test]
    fn test_io_error_conversion() {
        use std::io;
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let compile_err: CompileError = io_err.into();
        assert!(matches!(compile_err, CompileError::IoError(_)));
    }

    #[test]
    fn test_diagnostic_builder() {
        let diag = Diagnostic::error("test error")
            .with_span(Span::new(0, 10, 1, 1))
            .with_note("this is a note")
            .with_suggestion("fix it like this", Some("fixed".to_string()))
            .with_context_lines(5);

        assert!(matches!(diag.level, DiagnosticLevel::Error));
        assert_eq!(diag.message, "test error");
        assert_eq!(diag.notes.len(), 1);
        assert_eq!(diag.notes[0], "this is a note");
        assert_eq!(diag.suggestions.len(), 1);
        assert_eq!(diag.suggestions[0].message, "fix it like this");
        assert_eq!(diag.suggestions[0].replacement, Some("fixed".to_string()));
        assert_eq!(diag.context_lines, 5);
    }

    #[test]
    fn test_unexpected_char_diagnostic() {
        let err = CompileError::UnexpectedChar {
            ch: '€',
            line: 10,
            col: 5,
            span: Some(Span::new(100, 101, 10, 5)),
        };
        
        let diag = err.to_diagnostic();
        assert_eq!(diag.message, "Unexpected character '€' at line 10, column 5");
        assert_eq!(diag.notes.len(), 1);
        assert_eq!(diag.suggestions.len(), 1);
        assert!(diag.span.is_some());
    }

    #[test]
    fn test_type_mismatch_suggestions() {
        // int to string
        let err = CompileError::TypeMismatch {
            expected: "string".to_string(),
            found: "int".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("to_string()")));

        // string to int
        let err = CompileError::TypeMismatch {
            expected: "int".to_string(),
            found: "string".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("parse_int()")));

        // bool suggestion
        let err = CompileError::TypeMismatch {
            expected: "bool".to_string(),
            found: "int".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("true") && s.message.contains("false")));
    }

    #[test]
    fn test_undefined_function_suggestions() {
        // print -> println
        let err = CompileError::UndefinedFunction {
            name: "print".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.replacement == Some("println".to_string())));

        // printf -> println
        let err = CompileError::UndefinedFunction {
            name: "printf".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.replacement == Some("println".to_string())));

        // generic function
        let err = CompileError::UndefinedFunction {
            name: "myFunc".to_string(),
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("fn myFunc()")));
    }

    #[test]
    fn test_argument_count_mismatch() {
        // Too few arguments
        let err = CompileError::ArgumentCountMismatch {
            name: "add".to_string(),
            expected: 2,
            found: 1,
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.message.contains("expects 2 arguments, but 1 was provided"));
        assert!(diag.suggestions.iter().any(|s| s.message.contains("Add 1 more argument")));

        // Too many arguments
        let err = CompileError::ArgumentCountMismatch {
            name: "print".to_string(),
            expected: 1,
            found: 3,
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.message.contains("expects 1 argument, but 3 were provided"));
        assert!(diag.suggestions.iter().any(|s| s.message.contains("Remove 2 arguments")));
    }

    #[test]
    fn test_missing_semicolon_diagnostic() {
        let err = CompileError::MissingSemicolon {
            span: Some(Span::new(50, 51, 10, 20)),
        };
        let diag = err.to_diagnostic();
        assert_eq!(diag.message, "Missing semicolon after statement");
        assert!(diag.suggestions.iter().any(|s| s.replacement == Some(";".to_string())));
    }

    #[test]
    fn test_non_exhaustive_match_suggestions() {
        // Single missing pattern
        let err = CompileError::NonExhaustiveMatch {
            missing_patterns: vec!["None".to_string()],
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("Add a pattern for: None")));

        // Multiple missing patterns (<=3)
        let err = CompileError::NonExhaustiveMatch {
            missing_patterns: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("Red, Green, Blue")));

        // Many missing patterns (>3)
        let err = CompileError::NonExhaustiveMatch {
            missing_patterns: vec!["A".to_string(), "B".to_string(), "C".to_string(), "D".to_string(), "E".to_string()],
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.message.contains("wildcard pattern (_)")));
    }

    #[test]
    fn test_borrow_checker_errors() {
        let err = CompileError::UseOfMovedValue {
            name: "x".to_string(),
            span: Some(Span::new(10, 11, 5, 5)),
        };
        assert_eq!(err.to_string(), "Use of moved value: x");

        let err = CompileError::UseOfUninitializedValue {
            name: "y".to_string(),
            span: None,
        };
        assert_eq!(err.to_string(), "Use of uninitialized value: y");

        let err = CompileError::CannotMoveOutOfBorrowedContent {
            span: None,
        };
        assert_eq!(err.to_string(), "Cannot move out of borrowed content");
    }

    #[test]
    fn test_unsafe_operation_error() {
        let err = CompileError::UnsafeOperation {
            operation: "raw pointer dereference".to_string(),
            span: Span::new(0, 10, 1, 1),
        };
        assert_eq!(err.to_string(), "Unsafe operation 'raw pointer dereference' requires unsafe block");
    }

    #[test]
    fn test_generic_error() {
        let err = CompileError::Generic("Something went wrong".to_string());
        assert_eq!(err.to_string(), "Something went wrong");
        
        let diag = err.to_diagnostic();
        assert_eq!(diag.message, "Something went wrong");
    }
}
