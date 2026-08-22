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
branches terminate; an if without an else never does. `} else if (...) {` is an
else branch holding one nested if — reading it as "no else" and continuing past
it spliced the last arm's `return` into the enclosing block, which said
"terminates" about an if/else-if chain with no final `else`. That is exactly the
C a `match` lowers to.

COVERAGE IS CLOSED, NOT SAMPLED
-------------------------------
The first version of this reader scanned for lines matching one definition
shape and silently ignored every other line in the file. Its only coverage
alarm was "ZERO functions recognised", which fires only on a file that is
entirely unrecognised. In a MIXED file — one definition this reader knows, one
it does not — the denominator is nonzero, the alarm never sounds, and the
unrecognised definition is never analysed. The `} else if` false negative that
certified a shipped milestone was exactly this shape of hole, so a second one
in the same file is not acceptable.

So the top level is now enumerated exhaustively. Every line at column 0 must be
one of: blank, a comment, a `#include`-class directive, a declaration ending in
`;`, a `struct`/`union`/`enum`/`typedef` block, or a function definition — and
anything else STOPS the file with a HARNESS line naming it. There is no
"skipped quietly" outcome left, which is the only way "analysed" can mean what
it says.

WHAT THE GENERATOR ACTUALLY EMITS (measured, 2026-08-22, 538 files in
build_output/, and read in src/codegen/mod.rs)
    cmd: python3 - <<classify every column-0 line of build_output/*.c>>
         -> exactly 7 kinds: 23005 definitions matching DEF_RE, 23005 `}`
            closers, 14994 lines ending in `;`, 3420 `//` comments,
            2690 `#include`, 1614 `#define`, 684 `struct`/`enum`/`typedef`
            openers. No other shape occurs.
    cmd: grep -nE "goto|switch|#if|#ifdef|__attribute__" src/codegen/mod.rs
         -> 1 line, src/codegen/mod.rs:765, and it is a PROTOTYPE
            (`static void __pd_init() __attribute__((constructor));`), not a
            definition
    cmd: grep -cE "goto |switch|case |#if" build_output/*.c -> 0 in all 538
So: no `goto`, no labels, no `switch`, no conditional compilation, and the
opening brace of a definition is always the last character of its line
(`function_signature()` builds one line, then `generate_function_with_name()`
appends `" {\n"`). Those are INVARIANTS OF THE GENERATOR, not properties of C,
so they are enforced here rather than assumed: if codegen ever emits one of
them this reader says HARNESS instead of guessing. In particular `goto` would
defeat `contains_break()` below silently — it would read an escapable loop as
non-fallthrough — which is why its absence is checked rather than trusted.

ENFORCED WHERE THE CONSTRUCT CAN APPEAR, NOT WHERE IT WAS CONVENIENT TO LOOK.
The first version of this refusal lived only in the top-level scan, so a
`#if`/`#endif` written INSIDE a body walked straight past it as an ordinary
statement (see unmodelled_construct). "Enforced" now means both levels.

AND THE CLAIM IS FALSIFIABLE — here is exactly what turns red when codegen
starts emitting a shape this reader does not model:

  * `make stdlib-gate` runs this analyser over the C of all 7 stdlib drivers
    (scripts/stdlib-gate.sh, `$PROBE generated-c "$cfile"`). A HARNESS is a
    malfunction there, and the gate is red.
  * `cargo test --test d3b_tail_if` runs this analyser over the C that the
    parser and codegen emit for every program those tests accept
    (`assert_net_a_accounts_for` in tests/d3b_tail_if.rs). That is the
    end-to-end pin: it is the ONLY check that reads real generated C through
    this reader on the same inputs the parser's own termination analysis just
    decided, so parser/checker disagreement, and any new emitted shape,
    surface as a test failure rather than as a claim in this comment. It runs
    inside `make test-honest` and inside `make m1-exit` (inventory 3 of 3).
  * `make test-gate-probe` fault-injects the shapes themselves
    (scripts/test-gate-probe.sh, "Net A coverage must be CLOSED"), so deleting
    a refusal turns that gate red unless its probe is deleted in the same
    commit — which is a visible edit to a reviewed file, not a silent one.

WHAT IS NOT COVERED, AND WHETHER IT MATTERS — MEASURED, NOT GUESSED
`make conformance` compiles 54 fixtures and passes none of them through this
analyser. Nor does anything else reach them: `link_command` (src/linker.rs:73-86)
invokes gcc with `-O2`/`-O0`/`-O3`, `-I <runtime>` and `-o` — NO `-Wall`, NO
`-Wreturn-type`, NO `-Werror`. So a conformance fixture whose C falls off the
end links silently. The only structural gate on that corpus is the transcript
diff, which catches a wrong VALUE but not a wrong SHAPE, and only for the values
a fixture happens to print. Net B (`-Werror=return-type`) exists solely inside
`gate_probe.py` `cmd_generated_c`, invoked by scripts/stdlib-gate.sh on the 7
stdlib drivers.

Does that matter today? Measured 2026-08-22, by compiling every conformance
fixture whose class is run/untranscribed/vacuous (51 of them) and running this
analyser over the emitted C:

    clean 50 · finding 1 · UNACCOUNTED 0

and the single finding is the already-declared, already-pinned tail-`match`
defect (tests/stdlib/stdlib_tail_match.pd, `known_violation:area_code,sides` in
tests/stdlib/DRIVERS.tsv:31). Zero unaccounted means no codegen shape unique to
the conformance corpus is outside this reader.

So: the gap is real, and it is currently empty. It is NOT closed here on
purpose. The natural home for a structural verdict on those fixtures is a
column in tests/conformance-manifest.txt — that runner's design is that every
row declares its own expectation and an undeclared one fails
(scripts/conformance.sh:10-13, :495 UNDECLARED, :713 MISSING). Bolting a second,
undeclared verdict source onto it from elsewhere would recreate the two-
inventories-that-disagree problem this branch spent itself removing. It is a
change to that manifest's design, not a patch, and it is handed off as one.

EXIT TAXONOMY — a finding and a malfunction must not share an exit code.
    0  every function analysed, none can fall off its end
    1  at least one genuine FINDING, and nothing malfunctioned
    2  a HARNESS error: input missing, unreadable, a top-level construct this
       reader cannot account for, or the analyser itself raised. Harness errors
       DOMINATE: if anything malfunctioned the answer is 2 even when findings
       were also produced, because a partial analysis cannot support "these are
       the defects".

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

# A top-level definition: starts at column 0, has a parameter list, and the
# opening brace is the LAST character of the line. Prototypes end in `);`.
DEF_RE = re.compile(r"^[A-Za-z_][A-Za-z_0-9 *]*\(.*\)[ \t]*\{[ \t]*$")
# `void f(...)` is void; `void* f(...)` is NOT.
VOID_RE = re.compile(r"^(?:static\s+)?(?:inline\s+)?void\s+[A-Za-z_]")
# Calls that do not return, so a body ending in one cannot fall through.
NORETURN_RE = re.compile(r"^(?:__pd_panic|abort|exit|__builtin_unreachable)\s*\(")
# `return` as a whole word. `returning();` is a call, not a return statement.
RETURN_RE = re.compile(r"^return\b")

# A top-level type declaration that opens a brace: `enum FileMode {`,
# `typedef struct Result {`, `typedef struct {`. It declares a type, never a
# body, so it is skipped — but only after being RECOGNISED, and it is consumed
# up to its own `};` / `} Name;` at column 0 so the scan stays in step.
TYPE_OPEN_RE = re.compile(
    r"^(?:typedef\s+|static\s+|const\s+)*(?:struct|union|enum)\b[^;()]*\{[ \t]*$")
# Directives that cannot hide a function definition.
CPP_SAFE_RE = re.compile(r"^#\s*(?:include|define|undef|pragma|line|error|warning)\b")
# Directives that CAN: text between them may or may not be compiled, so which
# definitions exist is no longer a question this reader can answer.
CPP_COND_RE = re.compile(r"^#\s*(?:if|ifdef|ifndef|elif|else|endif)\b")

# Constructs whose control flow this reader does not model. `goto` is the
# dangerous one: `while (1) { … goto done; }` IS escapable, `contains_break`
# below would not see it, and the result would be a silent "terminates".
GOTO_RE = re.compile(r"^goto\b")
LABEL_RE = re.compile(r"^[A-Za-z_][A-Za-z_0-9]*[ \t]*:(?!:)")


# `} else if (...) {` — an else branch that is itself one compound statement.
ELSE_IF_RE = re.compile(r"^\}[ \t]*else[ \t]+(\S.*\{)[ \t]*$")


def parse_compound(lines, header, i, lineno):
    """Parse one compound statement whose body starts at line `i`.

    `lineno` is the 1-based source line of the HEADER, carried through so a
    diagnostic can name the construct's own line rather than the enclosing
    function's. Returns (item, index_after_the_whole_construct).
    """
    then_items, j = parse_block(lines, i)
    closing = lines[j].strip() if j < len(lines) else "}"
    if closing.startswith("} else {"):
        else_items, k = parse_block(lines, j + 1)
        return ("compound", header, then_items, else_items, lineno), k + 1
    m = ELSE_IF_RE.match(closing)
    if m:
        # The else branch holds exactly one nested compound, and it must be
        # recorded AS the else branch.
        #
        # Before this case existed, the branch below treated `} else if` as
        # "no else" and then resumed reading at the line after it — which
        # spliced the else-if's BODY into the parent statement list. So a chain
        # whose last arm ends in `return` looked terminating: that `return`
        # became the parent's last statement. Measured on the C that a
        # `match` lowers to (an if/else-if chain with NO final `else`), this
        # analyser reported the file clean while gcc on the same file said
        # "non-void function does not return a value in all control paths".
        # A false "terminates" is the silent direction this whole check exists
        # to avoid, so it is worth the extra case.
        nested, k = parse_compound(lines, m.group(1), j + 1, j + 1)
        return ("compound", header, then_items, [nested], lineno), k
    # `} else` with the body on following lines — treat as no else rather than
    # guessing; a false "does not terminate" is a loud failure that gets looked
    # at, a false "terminates" is silent.
    return ("compound", header, then_items, None, lineno), j + 1


def parse_block(lines, i):
    """Parse statements until this block's closing brace.

    Returns (items, index_of_closing_line). An item is either
      ('stmt', text, lineno) or
      ('compound', header, then_items, else_items|None, lineno),
    where `lineno` is 1-based and is the line the item STARTS on. It is carried
    so that a construct this reader cannot model is reported at its own line: it
    used to be reported at the enclosing function's `{`, which sends an operator
    to the wrong place in a file they did not write.
    """
    items = []
    while i < len(lines):
        text = lines[i].strip()
        if text.startswith("}"):
            # Closes this block (possibly `} else {`, handled by the caller).
            return items, i
        if text.endswith("{"):
            item, i = parse_compound(lines, text, i + 1, i + 1)
            items.append(item)
            continue
        if text:
            items.append(("stmt", text, i + 1))
        i += 1
    return items, i


def contains_break(items, depth=0):
    """Is there a REACHABLE `break` that escapes THIS loop?

    A `break` inside a nested loop or a `switch` binds to that construct, not to
    ours, so only breaks at loop-depth 0 count.

    REACHABILITY. The scan stops at the first item that cannot fall through,
    because a `break` after one is dead text:

        while (1) { return 2; break; }

    the `return` leaves the function, so the `break` never runs, so this loop
    has no exit edge. Counting every syntactically present `break` called it
    escapable and reported the enclosing function as a fall-through. The parser
    side (`contains_escaping_break` in src/parser/mod.rs) stops at the same
    place for the same reason; the two must agree or a program one accepts is
    flagged by the other.
    """
    for item in items:
        if item[0] == "stmt":
            if depth == 0 and re.match(r"^break\b", item[1]):
                return True
        else:
            _, header, then_items, else_items, _ln = item
            h = header.rstrip("{").strip()
            nested = depth + 1 if re.match(r"^(while|for|do|switch)\b", h) else depth
            if contains_break(then_items, nested):
                return True
            if else_items is not None and contains_break(else_items, depth):
                return True
        if item_terminates(item):
            # Everything after this point is unreachable, breaks included.
            return False
    return False


def terminates(items):
    """Does this statement list definitely return / not fall through?

    ANY item, not only the last. Anything written after a statement that cannot
    fall through is unreachable, so the list as a whole cannot fall through
    either:

        if (n) { return 1; __pd_print_int(2); } else { return 2; }

    Reading only the last item called the `if` arm a fall-through and reported
    this correct function. That matters now that the parser side accepts the
    same shape (`already_terminates` in src/parser/mod.rs uses `any` for the
    same reason): a program one side accepts and the other flags is a gate that
    goes red on valid code, which is how a gate gets switched off.
    """
    return any(item_terminates(item) for item in items)


def item_terminates(item):
    """Does this ONE statement or compound never fall through to the next?"""
    if item[0] == "stmt":
        text = item[1]
        return bool(RETURN_RE.match(text)) or bool(NORETURN_RE.match(text))
    _, header, then_items, else_items, _ln = item
    h = header.rstrip("{").strip()
    # An infinite loop never falls through — UNLESS it can `break` out of itself.
    # `while (1) { ... break; ... }` reaches the code after the loop, so treating
    # every `while (1)` as terminating would wrongly clear a real fall-through.
    # `goto` would escape it too; see unmodelled_construct(), which stops the
    # file rather than letting this read as "terminates".
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


def unmodelled_construct(items):
    """-> (source line, description) of the first unmodellable construct, or None.

    Everything here is a construct the generator provably does not emit (see the
    module docstring). Checking rather than assuming is the point: the moment
    codegen grows one of them, this says HARNESS instead of quietly returning a
    verdict its analysis no longer supports.

    THIS IS WHERE THE TOP-LEVEL REFUSAL WAS INCOMPLETE. check_file() refuses a
    conditional preprocessor directive at column 0, but `parse_block` records
    one written INSIDE a body as an ordinary statement, and this function used
    to walk past it. So

        long long f(long long n) {
        #if FEATURE
            return 1;
        #endif
        }

    read as "there is a return, so it terminates" while the build with FEATURE
    off falls off the end. A refusal that holds only where the reader happened
    to look is a docstring, not an invariant — so directives are refused
    wherever they can appear.
    """
    for item in items:
        if item[0] == "stmt":
            _, text, lineno = item
            if text.startswith("#"):
                return (lineno,
                        "a preprocessor directive inside a function body (%s) — "
                        "text it selects may or may not be compiled, so which "
                        "statements this body HAS is not a question this reader "
                        "can answer" % text[:40])
            if GOTO_RE.match(text):
                return (lineno,
                        "a `goto` (%s) — a jump can leave a loop that "
                        "`contains_break` would call inescapable" % text[:40])
            if re.match(r"^(?:case\b|default[ \t]*:)", text):
                return lineno, "a `switch` case label (%s)" % text[:40]
            if LABEL_RE.match(text):
                return (lineno,
                        "a label (%s) — it is a jump target, so control can "
                        "arrive here from anywhere" % text[:40])
            continue
        _, _, then_items, else_items, _ln = item
        found = unmodelled_construct(then_items)
        if found:
            return found
        if else_items is not None:
            found = unmodelled_construct(else_items)
            if found:
                return found
    return None


def check_file(path):
    """Return (violations, harness_errors, functions_recognised) for one file.

    The third value is what makes "analysed" observable, and the top-level scan
    below is what makes it CLOSED: every line at column 0 is accounted for, so
    `recognised` is the number of definitions in the file rather than the number
    this reader happened to match. The first unaccounted-for construct stops the
    file — once the scan is out of step with the braces, every verdict after it
    is arbitrary, and an arbitrary "clean" is the outcome this whole gate exists
    to prevent.
    """
    try:
        with open(path, "r", errors="replace") as fh:
            lines = fh.read().splitlines()
    except OSError as exc:
        # NOT a finding: we never got to look. Previously this returned 1 and
        # was reported as a structural defect in a file that could not be read.
        print(f"HARNESS {path}: cannot read: {exc}")
        return (0, 1, 0)

    def unaccounted(lineno, what):
        print(f"HARNESS {path}:{lineno}: {what}. This reader analyses the C that "
              f"pdc emits; a shape it cannot account for means the scan is no "
              f"longer in step with the file, so nothing after it was analysed")

    violations = 0
    recognised = 0
    declarations = 0
    n = len(lines)
    i = 0
    while i < n:
        raw = lines[i]
        text = raw.strip()
        if not text:
            i += 1
            continue
        if raw[0] in " \t":
            unaccounted(i + 1, f"indented line where a top-level construct was "
                               f"expected: {text[:60]!r}")
            return (violations, 1, recognised)
        if text.startswith("//"):
            i += 1
            continue
        if text.startswith("/*"):
            j = i
            while j < n and "*/" not in lines[j]:
                j += 1
            if j >= n:
                unaccounted(i + 1, "block comment is never closed")
                return (violations, 1, recognised)
            i = j + 1
            continue
        if text.startswith("#"):
            if CPP_COND_RE.match(text) or not CPP_SAFE_RE.match(text):
                unaccounted(i + 1, f"preprocessor directive that can decide which "
                                   f"definitions exist: {text[:60]!r}")
                return (violations, 1, recognised)
            while text.endswith("\\") and i + 1 < n:   # line continuation
                i += 1
                text = lines[i].strip()
            i += 1
            continue
        if DEF_RE.match(raw):
            recognised += 1
            body, close = parse_block(lines, i + 1)
            if close >= n:
                unaccounted(i + 1, f"function body is never closed: {text[:60]!r}")
                return (violations, 1, recognised)
            bad = unmodelled_construct(body)
            if bad:
                # Reported at the CONSTRUCT's line, not the definition's. It
                # used to be `i + 1` — the function header — which sends an
                # operator to the wrong place in a generated file they did not
                # write. The function is still named, because that is the unit
                # whose verdict is being withheld.
                bad_line, why = bad
                unaccounted(bad_line,
                            f"{why}, inside {text[:60]!r} at line {i + 1}")
                return (violations, 1, recognised)
            if not VOID_RE.match(text) and not terminates(body):
                print(
                    f"FINDING {path}:{i + 1}: non-void function may fall off its end "
                    f"(no return on every path): {text}"
                )
                violations += 1
            i = close + 1
            continue
        if TYPE_OPEN_RE.match(text):
            # A type declaration. Consumed to its own closer at column 0 so the
            # scan stays in step; members are indented, so the first column-0
            # `}` is this declaration's.
            j = i + 1
            while j < n and not lines[j].startswith("}"):
                j += 1
            if j >= n or not lines[j].rstrip().endswith(";"):
                unaccounted(i + 1, "type declaration with no closing `};` at "
                                   "column 0: %r" % text[:60])
                return (violations, 1, recognised)
            declarations += 1
            i = j + 1
            continue
        if text.endswith(";"):
            declarations += 1     # prototype, extern, global, typedef alias
            i += 1
            continue
        unaccounted(i + 1, f"top-level construct this reader does not recognise: "
                           f"{text[:60]!r} (a definition whose `{{` is not the last "
                           f"character of its line, an attributed or multi-line "
                           f"definition, …)")
        return (violations, 1, recognised)

    if recognised == 0:
        # A C file pdc produced always defines functions. Zero means this reader
        # did not understand the file, not that the file is clean.
        print(f"HARNESS {path}: no function definitions recognised — nothing was analysed")
        return (0, 1, 0)
    print(f"ACCOUNTED {path}: {recognised} definition(s) analysed, "
          f"{declarations} declaration(s), 0 unaccounted")
    return (violations, 0, recognised)


def main(argv):
    if len(argv) < 2:
        print("HARNESS: usage: check-c-returns.py <file.c> [...]")
        return 2
    violations = 0
    harness = 0
    recognised = 0
    for path in argv[1:]:
        try:
            v, h, r = check_file(path)
        except Exception:  # noqa: BLE001 - any analyser bug is a HARNESS error
            # Without this, an uncaught exception exits 1 and is indistinguishable
            # from a finding. A crashed analyser has not proved anything.
            print(f"HARNESS {path}: analyser raised:")
            for line in traceback.format_exc().rstrip().splitlines():
                print(f"HARNESS   {line}")
            v, h, r = 0, 1, 0
        violations += v
        harness += h
        recognised += r
    # Always report the denominator, so a caller can see that work was done.
    # `recognised` is now the whole top level of every file that was accounted
    # for, not the subset one regex matched — a file with an unaccounted-for
    # construct contributes a HARNESS instead of a smaller number.
    print(f"ANALYSED {recognised} function definition(s) in {len(argv) - 1} file(s)")
    if harness:
        return 2
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
