#!/usr/bin/env python3
"""Generate docs/reference/builtins.md from src/builtins.rs.

The builtin list is a single Rust table (`BUILTINS`) that the type checker and the
borrow checker both derive from. Generating the reference from that same table is
what keeps the documentation from drifting away from the compiler — which is how
this repository ended up with 508 non-compiling documentation snippets.

Run after changing src/builtins.rs:  python3 scripts/gen-builtin-docs.py
Check without writing (a gate reads this):  python3 scripts/gen-builtin-docs.py --check

THE PARAMETER PARSER WAS BROKEN AND THE FILE IT GENERATES IS OLDER THAN IT.
Measured at `acda322`, before this branch touched anything: the regex below read
`p(Ty, Mode)` — two arguments — while `src/builtins.rs` has written
`p("name", Ty, Mode)` since `BuiltinParam` gained its user-visible parameter name.
It matched 0 of 51 call sites, so running this generator on `main` rewrote every
signature in docs/reference/builtins.md from `print(String)` to `print()` and
dropped every "borrows its string argument" note. The committed reference is a
fossil emitted by an older version of this script.

That is a two-sided defect and both sides are closed here. The regex is fixed, so
regenerating is no longer destructive; and `--check` exists so that the file
being stale is a red test rather than something discovered by whoever next runs
the generator. src/builtins.rs::test_generated_builtin_reference_is_not_stale
calls it, which is the same shape as the check that keeps runtime/pd_prelude.h
from going stale against the compiler that emits it.
"""

import re
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "src" / "builtins.rs"
OUT = ROOT / "docs" / "reference" / "builtins.md"

TYPE_NAMES = {"I64": "i64", "Str": "String", "Bool": "bool", "Char": "char", "Unit": "()"}

# Prose for each section header found in the table, keyed by the Rust comment text.
SECTION_BLURB = {
    "Output": "Writing to standard output, and aborting.",
    "String manipulation": (
        "Strings are immutable handles into an arena. `string_char_at` returns a "
        "`char` (N14-04), which is what the `char_is_*` predicates take; an index "
        "outside the string traps, because there is no `char` meaning 'no character'."
    ),
    "Character classification": "Predicates over a `char` (N4-04).",
    "Command-line arguments": (
        "C's convention: `arg_count()` counts the program name, so the first real "
        "argument is `arg_at(1)`. Out-of-range indices return an empty string, never null."
    ),
    "File I/O": (
        "The handle-based API. `file_open` returns an integer handle, or a negative "
        "value on failure."
    ),
    "Path and directory operations": "Queries and mutations on the filesystem.",
    "Whole-file helpers": "Read or write a whole file in one call.",
    # "Extended file API" was here, for the `*_ex` builtins. They left
    # src/builtins.rs on 2026-08-23 (N14 does not define them), so the section
    # has no entries and this blurb would never be printed. Removed rather than
    # kept "in case": a blurb for a section that cannot occur is a claim about a
    # part of the language that does not exist.
}


def parse():
    text = SRC.read_text()
    start = text.index("pub const BUILTINS")
    body = text[start:]

    entries = []
    section = None
    # Walk the table, tracking `// ---- Section ----` markers.
    for chunk in re.finditer(
        r"//\s*----\s*(?P<section>[^-]+?)\s*----|"
        r"Builtin\s*\{(?P<entry>.*?)\n\s*\},",
        body,
        re.S,
    ):
        if chunk.group("section"):
            section = chunk.group("section").strip()
            continue

        entry = chunk.group("entry")
        name = re.search(r'name:\s*"([^"]+)"', entry)
        if not name:
            continue
        name = name.group(1)

        params_raw = re.search(r"params:\s*&\[(.*?)\]", entry, re.S)
        params = []
        if params_raw:
            # `p("name", Ty, Mode)`. The first argument is the user-visible
            # parameter name and is not rendered here; the reference shows types.
            # An entry that declares parameters and parses to none is a parser
            # failure, not a nullary builtin — see the module docstring for the
            # run in which exactly that produced `print()`.
            for m in re.finditer(r'p\("(\w+)",\s*(\w+),\s*(\w+)\)', params_raw.group(1)):
                params.append((TYPE_NAMES.get(m.group(2), m.group(2)), m.group(3)))
            declared = params_raw.group(1).count("p(")
            if declared != len(params):
                print("error: %s declares %d parameter(s) and %d parsed — the "
                      "shape of src/builtins.rs changed and this generator would "
                      "emit a wrong signature"
                      % (name, declared, len(params)), file=sys.stderr)
                return None

        ret = re.search(r"ret:\s*(\w+)", entry)
        ret = TYPE_NAMES.get(ret.group(1), ret.group(1)) if ret else "()"

        entries.append({"name": name, "params": params, "ret": ret, "section": section})

    return entries


def signature(e):
    args = ", ".join(t for t, _ in e["params"])
    if e["ret"] == "()":
        return f"{e['name']}({args})"
    return f"{e['name']}({args}) -> {e['ret']}"


def main():
    check_only = "--check" in sys.argv[1:]

    entries = parse()
    if entries is None:
        return 1
    if not entries:
        print("error: no builtins parsed — did src/builtins.rs change shape?", file=sys.stderr)
        return 1

    lines = [
        "# Builtin function reference",
        "",
        "**GENERATED — do not edit by hand.** Regenerate with "
        "`python3 scripts/gen-builtin-docs.py` after changing `src/builtins.rs`.",
        "",
        f"Palladium has {len(entries)} builtin functions. They are ordinary free functions: "
        "there is no prelude to import and no module path to qualify. They are defined in a "
        "single table (`src/builtins.rs`) that the type checker and the borrow checker both "
        "derive from, so a builtin cannot exist in one pass and not the other.",
        "",
        "Their C implementations are emitted inline into every generated file; the file and "
        "path functions are thin wrappers over symbols supplied at link time by "
        "`runtime/palladium_runtime.c`.",
        "",
    ]

    order = []
    for e in entries:
        if e["section"] not in order:
            order.append(e["section"])

    for section in order:
        title = section or "Other"
        lines.append(f"## {title}")
        lines.append("")
        if title in SECTION_BLURB:
            lines.append(SECTION_BLURB[title])
            lines.append("")
        lines.append("| Signature | Notes |")
        lines.append("|---|---|")
        for e in entries:
            if e["section"] != section:
                continue
            borrows = [m for _, m in e["params"] if m == "Borrow"]
            note = "borrows its string argument" if borrows and len(borrows) == len(e["params"]) else ""
            lines.append(f"| `{signature(e)}` | {note} |")
        lines.append("")

    lines += [
        "## Notes that bite",
        "",
        "- `string_char_at` returns a **`char`** (N4-04/N14-04), not an `i64`. Use "
        "`as i64` for the code point; an index outside the string traps rather than "
        "answering a sentinel.",
        "- `file_write` returns `bool`, not a byte count.",
        "- `vec![x]` is a macro that expands to a **one-element array**, not a growable vector.",
        "- `dbg!(x)` expands to a call to `print_debug`, which is defined nowhere; it always fails.",
        "- `println!` takes exactly one argument.",
        "",
        "See the [language specification](../specification/language-spec.md) for the full "
        "behaviour of each construct and the [tutorial](../user-guide/tutorial.md) for worked "
        "examples.",
        "",
    ]

    generated = "\n".join(lines)

    if check_only:
        try:
            on_disk = OUT.read_text()
        except OSError as exc:
            print("error: cannot read %s: %s" % (OUT, exc), file=sys.stderr)
            return 1
        if on_disk == generated:
            print(f"{OUT.relative_to(ROOT)} is in sync with src/builtins.rs "
                  f"({len(entries)} builtins, {len(order)} sections)")
            return 0
        # Name the first divergent line rather than the whole diff: the file is
        # 60 lines and the useful signal is which builtin moved.
        want, got = generated.split("\n"), on_disk.split("\n")
        first = next((i for i, (a, b) in enumerate(zip(want, got)) if a != b), None)
        detail = ("first difference at line %d:\n  from src/builtins.rs: %s\n"
                  "  on disk:              %s" % (first + 1, want[first], got[first])
                  if first is not None else
                  "lengths differ: generated %d lines, on disk %d"
                  % (len(want), len(got)))
        print("error: %s is stale — regenerate with `python3 %s`\n%s"
              % (OUT.relative_to(ROOT), pathlib.Path(__file__).name, detail),
              file=sys.stderr)
        return 1

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(generated)
    print(f"wrote {OUT.relative_to(ROOT)} ({len(entries)} builtins, {len(order)} sections)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
