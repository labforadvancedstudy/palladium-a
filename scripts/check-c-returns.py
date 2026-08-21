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

Usage: check-c-returns.py <file.c> [...]   (exit 1 if any function may fall off)
"""

import re
import sys

# A top-level definition: starts at column 0, has a parameter list, opens a
# brace on the same line. Prototypes end in `);` and are skipped.
DEF_RE = re.compile(r"^[A-Za-z_][A-Za-z_0-9 *]*\(.*\)[ \t]*\{[ \t]*$")
# `void f(...)` is void; `void* f(...)` is NOT.
VOID_RE = re.compile(r"^(?:static\s+)?(?:inline\s+)?void\s+[A-Za-z_]")
# Calls that do not return, so a body ending in one cannot fall through.
NORETURN_RE = re.compile(r"^(?:__pd_panic|abort|exit|__builtin_unreachable)\s*\(")


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


def terminates(items):
    """Does this statement list definitely return / not fall through?"""
    if not items:
        return False
    kind = items[-1]
    if kind[0] == "stmt":
        text = kind[1]
        return text.startswith("return") or bool(NORETURN_RE.match(text))
    _, header, then_items, else_items = kind
    h = header.rstrip("{").strip()
    # An infinite loop never falls through.
    if re.match(r"^(while\s*\(\s*1\s*\)|for\s*\(\s*;\s*;\s*\))", h):
        return True
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
    try:
        with open(path, "r", errors="replace") as fh:
            lines = fh.read().splitlines()
    except OSError as exc:
        print(f"{path}: cannot read: {exc}")
        return 1

    violations = 0
    i = 0
    while i < len(lines):
        line = lines[i]
        if DEF_RE.match(line):
            body, close = parse_block(lines, i + 1)
            if not VOID_RE.match(line.strip()) and not terminates(body):
                print(
                    f"{path}:{i + 1}: non-void function may fall off its end "
                    f"(no return on every path): {line.strip()}"
                )
                violations += 1
            i = close + 1
            continue
        i += 1
    return violations


def main(argv):
    if len(argv) < 2:
        print("usage: check-c-returns.py <file.c> [...]", file=sys.stderr)
        return 2
    total = sum(check_file(p) for p in argv[1:])
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
