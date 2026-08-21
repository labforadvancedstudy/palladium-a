#!/usr/bin/env python3
"""Generate docs/reference/builtins.md from src/builtins.rs.

The builtin list is a single Rust table (`BUILTINS`) that the type checker and the
borrow checker both derive from. Generating the reference from that same table is
what keeps the documentation from drifting away from the compiler — which is how
this repository ended up with 508 non-compiling documentation snippets.

Run after changing src/builtins.rs:  python3 scripts/gen-builtin-docs.py
"""

import re
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "src" / "builtins.rs"
OUT = ROOT / "docs" / "reference" / "builtins.md"

TYPE_NAMES = {"I64": "i64", "Str": "String", "Bool": "bool", "Unit": "()"}

# Prose for each section header found in the table, keyed by the Rust comment text.
SECTION_BLURB = {
    "Output": "Writing to standard output, and aborting.",
    "String manipulation": (
        "Strings are immutable handles into an arena. `string_char_at` returns the "
        "byte at an index as an integer, which is what the `char_is_*` predicates take."
    ),
    "Character classification": "Predicates over the integer a character position holds.",
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
    "Extended file API": (
        "A second handle API that mirrors the runtime's `pd_file_*` symbols more "
        "directly. Prefer the plain `file_*` functions."
    ),
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
            for m in re.finditer(r"p\((\w+),\s*(\w+)\)", params_raw.group(1)):
                params.append((TYPE_NAMES.get(m.group(1), m.group(1)), m.group(2)))

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
    entries = parse()
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
        "- `string_char_at` returns an **integer**, not a character type — there is no `char`.",
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

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(lines))
    print(f"wrote {OUT.relative_to(ROOT)} ({len(entries)} builtins, {len(order)} sections)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
