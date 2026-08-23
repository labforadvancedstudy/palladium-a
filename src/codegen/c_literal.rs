//! Palladium literal values → C literal source text.
//!
//! THE ONE DERIVATION, for the same reason `c_ident.rs` is one: what the
//! compiler protected and what it emitted were two different things, and the
//! way that is discovered is a byte in a binary that the source never named.
//!
//! The instance that produced this module: N2-09 made `"\0"` denote a NUL, and
//! the emitter it fed was a chain of five `.replace()` calls that knew about
//! `\ " \n \t \r` and nothing else. A NUL therefore went into the generated C
//! **as a raw zero byte inside a double-quoted literal** — measured, and gcc
//! accepted it, which is the worst of the available outcomes: a C source file
//! that no longer round-trips through a text editor, produced silently.
//!
//! So the rule here is default-deny, not enumerate-the-known-bad: every byte a
//! C string literal cannot carry verbatim gets an escape, and the residue is
//! passed through. `escape_every_control_character_the_lexer_can_produce`
//! derives the set to test from the lexer's own escape table rather than from a
//! list written here, so a new escape in `src/lexer/token.rs` cannot be added
//! without this module being asked about it.

/// The body of a C double-quoted string literal for `s` — WITHOUT the quotes.
pub fn c_string_body(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            // Three octal digits, always. `\0` alone is legal C but changes
            // meaning when a digit follows it — `"\0" + "1"` written adjacently
            // is the single character `\01` — and a C octal escape consumes at
            // most three digits, so the padded form cannot absorb the next
            // character whatever it is.
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\{:03o}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// A C character constant for `c`, quotes included.
///
/// Palladium char literals denote a Unicode scalar; the C backend's char
/// representation is `long long` holding the scalar value (see
/// `src/typeck` and N14's `string_from_char`). Emitting the NUMBER rather
/// than a C character constant is deliberate: `'a'` in C is an `int` whose
/// value is the *execution charset* encoding, which is not required to be
/// ASCII, and no C character constant exists for a scalar above 0xFF at all.
/// The scalar is the value the language defines, so the scalar is what is
/// emitted, with the source spelling in a trailing comment for a reader.
pub fn c_char_constant(c: char) -> String {
    format!("{} /* '{}' */", c as u32, c.escape_debug())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::escape_spellings;

    #[test]
    fn the_five_ordinary_escapes_round_trip() {
        assert_eq!(c_string_body("a\nb"), "a\\nb");
        assert_eq!(c_string_body("a\tb"), "a\\tb");
        assert_eq!(c_string_body("a\rb"), "a\\rb");
        assert_eq!(c_string_body("a\"b"), "a\\\"b");
        assert_eq!(c_string_body("a\\b"), "a\\\\b");
    }

    /// The regression this module exists for: a NUL used to be emitted raw.
    #[test]
    fn a_nul_is_three_octal_digits_and_never_a_raw_byte() {
        let got = c_string_body("nul: [\0]");
        assert_eq!(got, "nul: [\\000]");
        assert!(!got.contains('\0'), "a raw NUL reached the generated C");
    }

    /// `"\0" "1"` adjacent is the case a one-digit `\0` gets wrong.
    #[test]
    fn a_nul_followed_by_a_digit_does_not_absorb_it() {
        assert_eq!(c_string_body("\u{0}1"), "\\0001");
    }

    /// Default-deny, derived from the lexer rather than restated here: every
    /// escape the lexer accepts must survive this emitter as an escape, not as
    /// the byte itself. A new row in `ESCAPES` with no handling here fails.
    #[test]
    fn escape_every_control_character_the_lexer_can_produce() {
        for spelling in escape_spellings() {
            let e = spelling.chars().nth(1).expect("\\x");
            let v = crate::lexer::token::escape_char(e).expect("in the table");
            let emitted = c_string_body(&v.to_string());
            if (v as u32) < 0x20 || v == '"' || v == '\\' || v as u32 == 0x7f {
                assert!(
                    emitted.starts_with('\\') && emitted.len() >= 2,
                    "escape {} produced {:?} — a byte a C literal cannot carry verbatim",
                    spelling,
                    emitted
                );
            } else {
                assert_eq!(emitted, v.to_string(), "escape {} was mangled", spelling);
            }
        }
    }

    #[test]
    fn a_char_constant_is_its_scalar() {
        assert_eq!(c_char_constant('a'), "97 /* 'a' */");
        assert_eq!(c_char_constant('\n'), "10 /* '\\n' */");
        assert_eq!(c_char_constant('한'), "54620 /* '한' */");
    }
}
