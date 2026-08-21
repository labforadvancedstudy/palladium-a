#!/usr/bin/env python3
"""Net A of the generated-C invariant: no non-void function may fall off its end.

WHY THIS IS PHRASED OVER THE EMITTED BODY, NOT THE SOURCE CONSTRUCT
-------------------------------------------------------------------
The first version of this check asked "does the function contain at least one
`return`?". That is a statement about the *source construct the parser already
handles*, and it is too weak. Measured:

    fn f(n: i64) -> i64 {
        if n < 0 { return 0; }
        if n <= 1 { n } else { n * 2 }
    }

emits an early `return 0;` and then two bare expression statements, so the
"contains a return" rule PASSES while `f(5)` returns 0 instead of 10.

The rule implemented here is a statement about the emitted body itself:

    every non-void function's body must DEFINITELY RETURN on every path

which catches a tail `if`, a tail `match`, and any other construct the parser
fails to lower — including ones nobody has found yet. That is the entire
argument for a structural check over a transcript diff.

It must be a real terminator analysis, not "the last line is a `return`",
because this is legitimate and must NOT be flagged:

    long long f(long long n) {
        if ((n > 0)) {
            return 1;
        } else {
            return 2;
        }
    }

Its last line is `}`. So the analysis recurses: an if/else terminates iff both
branches terminate; an if without an else never does.

EXIT TAXONOMY — a finding and a malfunction must not share an exit code.
    0  every function analysed, none can fall off its end
    1  at least one genuine FINDING, and nothing malfunctioned
    2  a HARNESS error: input missing, unreadable, or the analyser itself
       raised. Harness errors DOMINATE: if anything malfunctioned the answer is
       2 even when findings were also produced, because a partial analysis
       cannot support "these are the defects".

This matters because an uncaught exception exits 1 by default, which made a
crashed analyser indistinguishable from a defect — the caller printed
"FAIL Net A (falls off the end)" over a Python traceback. Every line of output
is now tagged FINDING or HARNESS so the caller can verify that an exit 1 really
carried a well-formed finding rather than arbitrary output.

Usage: check-c-returns.py <file.c> [...]
"""

import re
import sys
import traceback

# A top-level definition: starts at column 0, has a parameter list, opens a
# brace on the same line. Prototypes end in `);` and are skipped.
DEF_RE = re.compile(r"^[A-Za-z_][A-Za-z_0-9 *]*\(.*\)[ \t]*\{[ \t]*$")
# `void f(...)` is void; `void* f(...)` is NOT.
VOID_RE = re.compile(r"^(?:static\s+)?(?:inline\s+)?void\s+[A-Za-z_]")
# Calls that do not return, so a body ending in one cannot fall through.
NORETURN_RE = re.compile(r"^(?:__pd_panic|abort|exit|__builtin_unreachable)\s*\(")
# `return` as a whole word. `returning();` is a call, not a return statement.
RETURN_RE = re.compile(r"^return\b")


def parse_block(lines, i):
    """Parse statements until this block's closing brace.

    Returns (items, index_of_closing_line). An item is either
      ('stmt', text) or ('compound', header, then_items, else_items|None).
    """
    items = []
    while i < len(lines):
        text = lines[i].strip()
        if text.startswith("}"):
            # Closes this block (possibly `} else {`, handled by the caller).
            return items, i
        if text.endswith("{"):
            header = text
            then_items, j = parse_block(lines, i + 1)
            closing = lines[j].strip() if j < len(lines) else "}"
            if closing.startswith("} else {"):
                else_items, k = parse_block(lines, j + 1)
                items.append(("compound", header, then_items, else_items))
                i = k + 1
            elif closing.startswith("} else"):
                # `} else` with the body on following lines — treat as no else
                # rather than guessing; a false "does not terminate" is a loud
                # failure that gets looked at, a false "terminates" is silent.
                items.append(("compound", header, then_items, None))
                i = j + 1
            else:
                items.append(("compound", header, then_items, None))
                i = j + 1
            continue
        if text:
            items.append(("stmt", text))
        i += 1
    return items, i


def contains_break(items, depth=0):
    """Is there a `break` that escapes THIS loop?

    A `break` inside a nested loop or a `switch` binds to that construct, not to
    ours, so only breaks at loop-depth 0 count.
    """
    for item in items:
        if item[0] == "stmt":
            if depth == 0 and re.match(r"^break\b", item[1]):
                return True
            continue
        _, header, then_items, else_items = item
        h = header.rstrip("{").strip()
        nested = depth + 1 if re.match(r"^(while|for|do|switch)\b", h) else depth
        if contains_break(then_items, nested):
            return True
        if else_items is not None and contains_break(else_items, depth):
            return True
    return False


def terminates(items):
    """Does this statement list definitely return / not fall through?"""
    if not items:
        return False
    kind = items[-1]
    if kind[0] == "stmt":
        text = kind[1]
        return bool(RETURN_RE.match(text)) or bool(NORETURN_RE.match(text))
    _, header, then_items, else_items = kind
    h = header.rstrip("{").strip()
    # An infinite loop never falls through — UNLESS it can `break` out of itself.
    # `while (1) { ... break; ... }` reaches the code after the loop, so treating
    # every `while (1)` as terminating would wrongly clear a real fall-through.
    if re.match(r"^(while\s*\(\s*1\s*\)|for\s*\(\s*;\s*;\s*\))", h):
        return not contains_break(then_items)
    if h.startswith("if"):
        # Needs BOTH arms; an `if` with no `else` always has a fall-through path.
        return else_items is not None and terminates(then_items) and terminates(else_items)
    if h.startswith("switch"):
        # Not analysed: a switch without `default` falls through, and proving
        # otherwise needs case analysis. Report as non-terminating so it is
        # examined rather than silently trusted.
        return False
    # A bare block `{ ... }` terminates if its contents do.
    if h in ("", "do"):
        return terminates(then_items)
    return False


def check_file(path):
    """Return (violations, harness_errors) for one file."""
    try:
        with open(path, "r", errors="replace") as fh:
            lines = fh.read().splitlines()
    except OSError as exc:
        # NOT a finding: we never got to look. Previously this returned 1 and
        # was reported as a structural defect in a file that could not be read.
        print(f"HARNESS {path}: cannot read: {exc}")
        return (0, 1)

    violations = 0
    i = 0
    while i < len(lines):
        line = lines[i]
        if DEF_RE.match(line):
            body, close = parse_block(lines, i + 1)
            if not VOID_RE.match(line.strip()) and not terminates(body):
                print(
                    f"FINDING {path}:{i + 1}: non-void function may fall off its end "
                    f"(no return on every path): {line.strip()}"
                )
                violations += 1
            i = close + 1
            continue
        i += 1
    return (violations, 0)


def main(argv):
    if len(argv) < 2:
        print("HARNESS: usage: check-c-returns.py <file.c> [...]")
        return 2
    violations = 0
    harness = 0
    for path in argv[1:]:
        try:
            v, h = check_file(path)
        except Exception:  # noqa: BLE001 - any analyser bug is a HARNESS error
            # Without this, an uncaught exception exits 1 and is indistinguishable
            # from a finding. A crashed analyser has not proved anything.
            print(f"HARNESS {path}: analyser raised:")
            for line in traceback.format_exc().rstrip().splitlines():
                print(f"HARNESS   {line}")
            v, h = 0, 1
        violations += v
        harness += h
    if harness:
        return 2
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
