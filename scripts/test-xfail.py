#!/usr/bin/env python3
"""Palladium expected-failure gate for the Rust test suite.

`scripts/conformance.sh` settled what an expected failure means for a .pd
program: it is declared with a mandatory reason; a declared failure that still
fails is XFAIL and is fine; a declared failure that PASSES is XPASS and fails the
gate; and a declared entry that is never evaluated is STALE and also fails the
gate, because "never ran" must not be indistinguishable from "failed as
expected". This applies the same three rules to the Rust tests, where the
declaration mechanism is `#[ignore = "…"]` rather than a manifest file.

THE INVENTORY MUST BE CLOSED IN BOTH DIRECTIONS, AND KEYED THE WAY CARGO SPEAKS.
Reading declarations and running cargo is not enough: an `#[ignore]` behind a
`cfg`, in a module nobody links, or in a target that failed to build is neither
run nor reported, and a gate that only counted what it saw would call that green.
So every declaration must be observed exactly once, and every ignored test
observed must have a declaration behind it.

That reconciliation is only as good as the key. Cargo reports a test by its FULL
module path — `optimizer::dead_code_test::tests::test_x`, not `test_x` — so a
key built from the function name alone makes a module-nested test simultaneously
STALE (declared under a name that never runs) and UNDECLARED (observed under a
name never declared), and collapses two same-named tests in different modules
into one bogus duplicate. Both shapes are regression-tested by `--self-test`,
which runs before every real invocation.

Keys are (target, module path), where the target is the source path cargo prints
in its `Running …` line, so the two sides are directly comparable.

TAGS. Rust overloads `#[ignore]` for two unrelated things, so the reason says
which:

  XFAIL: <missing feature> (owned by <milestone>)
      Cannot pass yet. If it passes, that is XPASS: delete the `#[ignore]` and
      let it join the regression net. <milestone> is M1..M9, or the literal
      `unscheduled` for the work MILESTONES.md files under "Not scheduled, and
      why". Nothing else.

  SLOW: <why, and roughly how slow>
      Passes today; excluded only for cost. Allowed to pass, and a failure is a
      real regression. Because relabelling an XPASS as SLOW would silently
      retire it from the suite, the SLOW set is an explicit allowlist below and
      adding to it means editing this file.

Usage: scripts/test-xfail.py [--self-test]
"""

import os
import re
import subprocess
import sys

# Reviewed allowlist of tests that may be #[ignore]d for cost alone.
SLOW_ALLOWLIST = {
    ("tests/stress_test.rs", "test_extremely_large_program"),
}

OWNER_RE = re.compile(r"\(owned by (M[1-9]|unscheduled)([,;: )]|$)")
FN_MODIFIERS = {"pub", "async", "unsafe", "extern", "const", "default"}


# --------------------------------------------------------------------------
# Rust source scanning
# --------------------------------------------------------------------------

def target_of(path):
    """The cargo test target a source file belongs to, or None."""
    path = path.replace(os.sep, "/")
    if path.startswith("tests/"):
        rest = path[len("tests/"):]
        # tests/common/mod.rs and friends are modules of another target, not
        # targets of their own.
        return path if "/" not in rest else None
    if path == "src/main.rs" or path.startswith("src/bin/"):
        return path
    if path.startswith("src/"):
        return "src/lib.rs"
    return None


def module_prefix_of(path):
    """The crate-relative module path of a file, as cargo would print it."""
    path = path.replace(os.sep, "/")
    if target_of(path) != "src/lib.rs":
        return []          # a target root: tests/foo.rs, src/main.rs, src/bin/*
    rest = path[len("src/"):]
    if rest == "lib.rs":
        return []
    if rest.endswith("/mod.rs"):
        rest = rest[: -len("/mod.rs")]
    elif rest.endswith(".rs"):
        rest = rest[: -len(".rs")]
    return rest.split("/")


def tokenize(src):
    """Yield (kind, text, line). Comments and literals collapse to one token so
    that braces inside them cannot move the module stack — `"fn main() { }"` in
    a test fixture is a string, not a scope."""
    i, line, n = 0, 1, len(src)
    while i < n:
        c = src[i]
        if c == "\n":
            line += 1
            i += 1
            continue
        if c.isspace():
            i += 1
            continue
        if src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
            continue
        if src.startswith("/*", i):
            depth, j = 1, i + 2          # Rust block comments nest
            while j < n and depth:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    line += src[j] == "\n"
                    j += 1
            i = j
            continue
        m = re.match(r'(?:b|r|br)?r?#*"', src[i:])
        if m and ("r" in m.group(0)):    # raw string: r"…", r#"…"#, br##"…"##
            hashes = m.group(0).count("#")
            close = '"' + "#" * hashes
            j = src.find(close, i + len(m.group(0)))
            j = n if j < 0 else j + len(close)
            line += src[i:j].count("\n")
            yield ("str", src[i:j], line)
            i = j
            continue
        if c == '"' or (c == "b" and src.startswith('b"', i)):
            j = i + (2 if c == "b" else 1)
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                line += src[j] == "\n"
                j += 1
            yield ("str", src[i:j], line)
            i = j
            continue
        if c == "'":
            # A char literal, or a lifetime. `'a'` vs `'a` — only the former
            # closes on a quote within three characters.
            m = re.match(r"'(\\.|[^\\'])'", src[i:])
            if m:
                yield ("char", m.group(0), line)
                i += len(m.group(0))
            else:
                i += 1
            continue
        if c == "#" and src.startswith("#[", i) or src.startswith("#![", i):
            j = i + src[i:].index("[")
            depth = 0
            while j < n:
                if src[j] == '"':                      # skip strings inside
                    j += 1
                    while j < n and src[j] != '"':
                        j += 2 if src[j] == "\\" else 1
                elif src[j] == "[":
                    depth += 1
                elif src[j] == "]":
                    depth -= 1
                    if depth == 0:
                        j += 1
                        break
                line += src[j] == "\n"
                j += 1
            yield ("attr", src[i:j], line)
            i = j
            continue
        if c.isalpha() or c == "_":
            m = re.match(r"[A-Za-z_][A-Za-z0-9_]*", src[i:])
            yield ("ident", m.group(0), line)
            i += len(m.group(0))
            continue
        if c.isdigit():
            m = re.match(r"[0-9][0-9A-Za-z_.]*", src[i:])
            i += len(m.group(0))
            continue
        yield ("punct", c, line)
        i += 1


def scan_file(path, src):
    """-> list of dicts: tag, target, module path, fn name, file, line, attr."""
    target = target_of(path)
    if target is None:
        return []
    prefix = module_prefix_of(path)
    out, mods, depth, pending = [], [], 0, None
    expect_mod_name = expect_fn_name = False
    mod_name = None
    for kind, text, line in tokenize(src):
        if kind == "attr":
            if re.match(r"#\[\s*ignore", text):
                pending = (text, line)
            continue
        if kind in ("str", "char"):
            continue
        if kind == "punct":
            if text == "{":
                depth += 1
                if mod_name is not None:
                    mods.append((depth, mod_name))
                    mod_name = None
            elif text == "}":
                while mods and mods[-1][0] == depth:
                    mods.pop()
                depth -= 1
            elif text == ";":
                mod_name = None
                expect_mod_name = False
            continue
        # ident
        if expect_mod_name:
            mod_name, expect_mod_name = text, False
            continue
        if expect_fn_name:
            expect_fn_name = False
            if pending is not None:
                attr, attrline = pending
                out.append({
                    "tag": ("XFAIL" if re.search(r'ignore\s*=\s*"XFAIL:', attr)
                            else "SLOW" if re.search(r'ignore\s*=\s*"SLOW:', attr)
                            else "BARE" if re.match(r"#\[\s*ignore\s*\]", attr)
                            else "UNTAGGED"),
                    "target": target,
                    "path": "::".join(prefix + [m[1] for m in mods] + [text]),
                    "name": text,
                    "file": path,
                    "line": attrline,
                    "attr": attr,
                })
            pending = None
            continue
        if text == "mod":
            expect_mod_name = True
            continue
        if text == "fn":
            expect_fn_name = True
            continue
        # An attribute attaches to the next item; only these may sit between.
        if text not in FN_MODIFIERS:
            pending = None
    return out


# --------------------------------------------------------------------------
# Cargo output parsing
# --------------------------------------------------------------------------

RUNNING_RE = re.compile(r"^\s+Running (?:unittests )?(\S+) \(target/")
DOCTEST_RE = re.compile(r"^\s+Doc-tests ")
RESULT_RE = re.compile(r"^test result:")
TEST_RE = re.compile(r"^test (\S+) \.\.\. (ok|FAILED)$")


def parse_cargo(out):
    """-> (observations, targets that started but never reported)."""
    target, obs, unreported, open_target = None, [], [], None
    for raw in out.splitlines():
        m = RUNNING_RE.match(raw)
        if m:
            if open_target:
                unreported.append(open_target)
            target = open_target = m.group(1)
            continue
        if DOCTEST_RE.match(raw):
            if open_target:
                unreported.append(open_target)
            target = open_target = "doc-tests"
            continue
        if RESULT_RE.match(raw):
            open_target = None
            continue
        m = TEST_RE.match(raw)
        if m:
            obs.append({"target": target, "path": m.group(1),
                        "outcome": m.group(2)})
    if open_target:
        unreported.append(open_target)
    return obs, unreported


# --------------------------------------------------------------------------
# Reconciliation
# --------------------------------------------------------------------------

def reconcile(decls, obs):
    """-> (summary counts, problems). Keys are (target, module path)."""
    problems = []
    xf = {(d["target"], d["path"]): d for d in decls if d["tag"] == "XFAIL"}
    sl = {(d["target"], d["path"]): d for d in decls if d["tag"] == "SLOW"}

    seen = {}
    for d in decls:
        if d["tag"] in ("XFAIL", "SLOW"):
            k = (d["target"], d["path"])
            seen.setdefault(k, []).append(d)
    for k, v in sorted(seen.items()):
        if len(v) > 1:
            problems.append(("DUPLICATE",
                             "%s::%s declared %d times, so a result cannot be "
                             "attributed" % (k[0], k[1], len(v))))

    counts = {"xfail": 0, "xpass": 0, "slow_pass": 0}
    observed_keys = set()
    for o in obs:
        k = (o["target"], o["path"])
        observed_keys.add(k)
        if k in xf:
            if o["outcome"] == "ok":
                counts["xpass"] += 1
                problems.append(("XPASS", "%s::%s\n      was: %s"
                                 % (k[0], k[1], xf[k]["attr"].strip())))
            else:
                counts["xfail"] += 1
        elif k in sl:
            if o["outcome"] == "ok":
                counts["slow_pass"] += 1
            else:
                problems.append(("SLOWFAIL", "%s::%s" % k))
        else:
            problems.append(("UNDECLARED", "%s::%s (%s)"
                             % (k[0], k[1], o["outcome"])))

    for k in sorted(set(xf) | set(sl)):
        if k not in observed_keys:
            problems.append(("STALE", "%s::%s" % k))

    counts["declared_xfail"] = len(xf)
    counts["declared_slow"] = len(sl)
    counts["observed"] = len(observed_keys)
    return counts, problems


def validate_tags(decls):
    problems = []
    for d in decls:
        loc = "%s:%d" % (d["file"], d["line"])
        key = "%s::%s" % (d["target"], d["path"])
        if d["tag"] == "BARE":
            problems.append(("TAG", "%s: bare #[ignore] on %s — every expected "
                             "failure needs a reason: #[ignore = \"XFAIL: "
                             "<missing feature> (owned by M<n>)\"] or "
                             "#[ignore = \"SLOW: <why>\"]" % (loc, key)))
        elif d["tag"] == "UNTAGGED":
            problems.append(("TAG", "%s: #[ignore] reason on %s must start with "
                             "'XFAIL: ' or 'SLOW: '" % (loc, key)))
        elif d["tag"] == "XFAIL":
            if not OWNER_RE.search(d["attr"]):
                problems.append(("TAG", "%s: XFAIL reason on %s names no valid "
                                 "owner. Use '(owned by M<n>)' with n in 1..9, "
                                 "or '(owned by unscheduled…)' for work under "
                                 "MILESTONES.md 'Not scheduled, and why'."
                                 % (loc, key)))
        elif d["tag"] == "SLOW":
            if (d["file"], d["name"]) not in SLOW_ALLOWLIST:
                problems.append(("TAG", "%s: %s is tagged SLOW but is not on the "
                                 "reviewed allowlist in scripts/test-xfail.py. A "
                                 "passing test must not be retired from the "
                                 "suite by relabelling it." % (loc, key)))
    return problems


# --------------------------------------------------------------------------
# Self-test: the two bijection shapes, plus the shapes that broke earlier
# --------------------------------------------------------------------------

def self_test():
    failures = []

    def check(label, got, want):
        if got != want:
            failures.append("%s\n     got:  %r\n     want: %r" % (label, got, want))

    # 1. A module-nested ignored test must key on its full path. Keying on the
    #    bare function name made such a test STALE and UNDECLARED at once.
    nested = '''
        #[cfg(test)]
        mod tests {
            #[test]
            #[ignore = "XFAIL: nope (owned by M1)"]
            fn test_a() { let s = "fn main() { }"; }
        }
    '''
    d = scan_file("src/optimizer/dead_code_test.rs", nested)
    check("nested test path", [x["path"] for x in d],
          ["optimizer::dead_code_test::tests::test_a"])
    check("nested test target", [x["target"] for x in d], ["src/lib.rs"])

    # 2. The same test name in two modules must be two keys, not a duplicate.
    dup = '''
        mod a {
            #[test]
            #[ignore = "XFAIL: nope (owned by M2)"]
            fn test_dup() {}
        }
        mod b {
            #[test]
            #[ignore = "XFAIL: nope (owned by M2)"]
            fn test_dup() {}
        }
    '''
    d = scan_file("tests/x_test.rs", dup)
    check("two modules, two keys", sorted(x["path"] for x in d),
          ["a::test_dup", "b::test_dup"])
    obs = [{"target": "tests/x_test.rs", "path": p, "outcome": "FAILED"}
           for p in ("a::test_dup", "b::test_dup")]
    counts, problems = reconcile(d, obs)
    check("two modules reconcile clean", problems, [])
    check("two modules counted", counts["xfail"], 2)

    # 3. Full round trip against real cargo output shape.
    cargo = (
        "   Compiling alan-von-palladium v0.2.0\n"
        "     Running unittests src/lib.rs (target/release/deps/palladium-ab)\n"
        "test optimizer::dead_code_test::tests::test_a ... FAILED\n"
        "test result: FAILED. 0 passed; 1 failed; 0 ignored\n"
        "     Running tests/x_test.rs (target/release/deps/x_test-cd)\n"
        "test a::test_dup ... FAILED\n"
        "test result: FAILED. 0 passed; 1 failed; 0 ignored\n"
    )
    o, unreported = parse_cargo(cargo)
    check("cargo paths kept whole",
          [x["path"] for x in o],
          ["optimizer::dead_code_test::tests::test_a", "a::test_dup"])
    check("cargo targets", [x["target"] for x in o],
          ["src/lib.rs", "tests/x_test.rs"])
    check("no unreported target", unreported, [])

    counts, problems = reconcile(
        scan_file("src/optimizer/dead_code_test.rs", nested)
        + scan_file("tests/x_test.rs", dup), o)
    kinds = sorted(k for k, _ in problems)
    check("one declared test never ran -> STALE only", kinds, ["STALE"])

    # 4. A target that starts and never reports must be caught.
    _, unreported = parse_cargo(
        "     Running tests/x_test.rs (target/release/deps/x_test-cd)\n"
        "test a::test_dup ... FAILED\n")
    check("unreported target detected", unreported, ["tests/x_test.rs"])

    # 5. An observation with no declaration is UNDECLARED.
    counts, problems = reconcile([], [{"target": "tests/x_test.rs",
                                       "path": "a::test_dup",
                                       "outcome": "ok"}])
    check("undeclared detected", [k for k, _ in problems], ["UNDECLARED"])

    # 6. Braces inside strings and comments must not move the module stack.
    tricky = '''
        mod outer {
            // fn fake() { {{{
            /* } } } */
            #[test]
            #[ignore = "XFAIL: has } and { in the reason (owned by M3)"]
            fn test_b() { let s = r#"} } {"#; }
        }
        #[test]
        #[ignore = "XFAIL: top level (owned by M3)"]
        fn test_c() {}
    '''
    d = scan_file("tests/y_test.rs", tricky)
    check("braces in literals ignored", sorted(x["path"] for x in d),
          ["outer::test_b", "test_c"])

    # 7. Tag validation, including the owner grammar.
    bad = '''
        #[test]
        #[ignore]
        fn test_bare() {}
        #[test]
        #[ignore = "because"]
        fn test_untagged() {}
        #[test]
        #[ignore = "XFAIL: x (owned by someday)"]
        fn test_bad_owner() {}
        #[test]
        #[ignore = "SLOW: not on the allowlist"]
        fn test_slow() {}
    '''
    d = scan_file("tests/z_test.rs", bad)
    check("four tag problems", len(validate_tags(d)), 4)

    if failures:
        print("self-test FAILED:", file=sys.stderr)
        for f in failures:
            print("  " + f, file=sys.stderr)
        return False
    print("self-test: %d checks green (module-path keying, both bijection "
          "shapes, literal-safe scanning, tag grammar)" % 13)
    return True


# --------------------------------------------------------------------------

def main():
    os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

    if not self_test():
        print("\nthe gate's own reconciliation is broken; not running it",
              file=sys.stderr)
        return 1
    if "--self-test" in sys.argv:
        return 0

    decls = []
    for root in ("src", "tests"):
        for dirpath, _, names in os.walk(root):
            for name in sorted(names):
                if not name.endswith(".rs"):
                    continue
                p = os.path.join(dirpath, name).replace(os.sep, "/")
                with open(p, encoding="utf-8") as fh:
                    decls += scan_file(p, fh.read())

    problems = validate_tags(decls)

    # The two streams MUST be merged by the OS, not concatenated afterwards:
    # cargo prints `Running <target>` to stderr and `test <name> ... ok` to
    # stdout, and it is their interleaving that says which target a test
    # belongs to. Concatenating puts every target header after every result.
    proc = subprocess.run(
        ["cargo", "test", "--release", "--no-fail-fast", "--", "--ignored"],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    out = proc.stdout

    obs, unreported = parse_cargo(out)
    for t in unreported:
        problems.append(("NO_RESULT", "%s started and never reported a result — "
                                      "it did not run at all" % t))

    if re.search(r"^error\[", out, re.M) or re.search(
            r"^error: (could not compile|failed to|expected)", out, re.M):
        first = "; ".join(re.findall(r"^error.*", out, re.M)[:3])
        problems.append(("BUILD", "the ignored set did not build: " + first))
    elif proc.returncode != 0 and not any(o["outcome"] == "FAILED" for o in obs):
        first = "; ".join(re.findall(r"^error.*", out, re.M)[:3])
        problems.append(("CARGO", "cargo exited %d with no failing test to "
                                  "explain it: %s" % (proc.returncode, first)))

    counts, more = reconcile(decls, obs)
    problems += more

    print("==============================================")
    print("declared: xfail=%d slow=%d   observed: %d"
          % (counts["declared_xfail"], counts["declared_slow"],
             counts["observed"]))
    print("ran:      xfail=%d xpass=%d slow_pass=%d"
          % (counts["xfail"], counts["xpass"], counts["slow_pass"]))
    print("  xfail      = declared missing-feature test, still failing — as expected")
    print("  xpass      = declared failing but PASSED — a stale expectation, fails the gate")
    print("  slow       = passes, excluded only for cost, on the reviewed allowlist")
    print("  stale      = declared but never ran — indistinguishable from failing, fails the gate")
    print("  undeclared = an ignored test with no declaration behind it, fails the gate")
    print("==============================================")

    titles = {
        "XPASS": "XPASS — these now pass; delete the #[ignore] so they join the regression net:",
        "STALE": "STALE — declared expected-failures that never ran (cfg'd out, unlinked, or in a target that did not report):",
        "UNDECLARED": "UNDECLARED — ignored tests with no #[ignore] declaration this script could read:",
        "SLOWFAIL": "SLOW test failed — it is declared as passing-but-expensive, so this is a real regression:",
        "DUPLICATE": "DUPLICATE declarations:",
        "TAG": "#[ignore] declaration errors:",
        "NO_RESULT": "Targets that produced no result:",
        "BUILD": "Build errors:",
        "CARGO": "Unexplained cargo status:",
    }
    for kind in ("XPASS", "STALE", "UNDECLARED", "SLOWFAIL", "DUPLICATE",
                 "TAG", "NO_RESULT", "BUILD", "CARGO"):
        items = [m for k, m in problems if k == kind]
        if items:
            print()
            print(titles[kind])
            for m in items:
                print("  " + m)

    if not problems:
        print("✓ every declared expected failure ran, and every one of them is "
              "still failing")
        return 0
    sys.stdout.flush()
    print("\n%d problem(s) above." % len(problems), file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
