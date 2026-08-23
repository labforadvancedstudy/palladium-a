#!/usr/bin/env python3
"""Write a src/builtins.rs variant with TWO unsupported entries, one unparseable.

A fault injection for scripts/gate_probe.py's `reconcile`, used by
scripts/test-gate-probe.sh. It exists as a file rather than a heredoc because the
shell test is itself quoted inside other tooling and a nested heredoc was one
escaping layer too many — a control nobody can read is a control nobody
maintains.

WHY THIS SHAPE. `reconcile` cross-checks the registry's unsupported set against
tests/stdlib/BUILTINS.tsv. Its completeness check has twice been an EXISTENCE
check: first "no names parsed means the parser broke" (which reported a
malfunction when the unsupported set legitimately went empty), then "some names
parsed, so we are fine" (which a single readable entry satisfies while any number
of malformed ones are silently dropped). This injection is the case that only an
equality check catches: one entry `reconcile` can read, one it cannot.

Usage: scripts/mk_partial_registry.py <output-path>
"""

import re
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: mk_partial_registry.py <output-path>", file=sys.stderr)
        return 2

    src = open("src/builtins.rs").read()
    # The same regex `reconcile` uses, then filtered to REAL table entries. The
    # bare pattern also matches `pub struct Builtin {` and the doc examples, which
    # is why `reconcile` reports more "entries parsed" than there are builtins —
    # harmless there, fatal here, because this injection has to plant its faults
    # in rows that actually carry a name and a support field.
    blocks = [m for m in re.finditer(r"Builtin\s*\{.*?\n    \}", src, re.S)
              if "Support::Callable" in m.group(0) and "name:" in m.group(0)]
    if len(blocks) < 2:
        print("error: src/builtins.rs yielded %d Builtin entries; this injection "
              "needs two" % len(blocks), file=sys.stderr)
        return 2

    a, b = blocks[0], blocks[1]
    unsupported = 'Support::Unsupported("planted")'
    first = a.group(0).replace("Support::Callable", unsupported)
    # The second is unsupported AND its name field is renamed, so the reader sees
    # the entry, knows it is unsupported, and cannot extract who it is.
    second = (b.group(0).replace("Support::Callable", unsupported)
                        .replace("name:", "ident:", 1))
    if unsupported not in first or unsupported not in second:
        print("error: the first two entries are no longer Support::Callable, so "
              "this injection plants nothing", file=sys.stderr)
        return 2

    out = src[:a.start()] + first + src[a.end():b.start()] + second + src[b.end():]
    open(sys.argv[1], "w").write(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
