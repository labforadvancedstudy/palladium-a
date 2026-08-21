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

# Reviewed allowlist of tests that may be #[ignore]d for cost alone. Keyed by
# the SAME identity reconciliation uses — (target, full module path) — because
# keying it by bare function name would hand permission to every same-named test
# in the file, reintroducing the collision the module-path keying just removed.
SLOW_ALLOWLIST = {
    ("tests/stress_test.rs", "test_extremely_large_program"),
}

OWNER_RE = re.compile(r"\(owned by (M[1-9]|unscheduled)([,;: )]|$)")
FN_MODIFIERS = {"pub", "async", "unsafe", "extern", "const", "default"}


# --------------------------------------------------------------------------
# Rust source scanning
# --------------------------------------------------------------------------

def target_roots(exists=os.path.exists, listdir=os.listdir):
    """The crate roots cargo builds as test targets.

    A target's identity is its root file, which is exactly what cargo prints in
    its `Running …` line. Everything else in the target is reached by walking
    Rust's module graph from here — NOT by guessing a module path from a file's
    location on disk. Those two disagree for every ordinary `mod helpers;`:
    `tests/common/mod.rs` is one file that is compiled into five different test
    binaries, so it holds five declarations, one per target, and a filesystem
    walk cannot express that at all.
    """
    roots = []
    for p in ("src/lib.rs", "src/main.rs"):
        if exists(p):
            roots.append(p)
    if exists("src/bin"):
        roots += ["src/bin/" + n for n in sorted(listdir("src/bin"))
                  if n.endswith(".rs")]
    if exists("tests"):
        roots += ["tests/" + n for n in sorted(listdir("tests"))
                  if n.endswith(".rs")]
    return roots


def module_dir(path, is_root):
    """Where `mod NAME;` inside `path` looks for NAME.

    Crate roots and `mod.rs` own their containing directory; any other file
    `dir/foo.rs` owns `dir/foo/`. This is why `tests/common/mod.rs` is the
    idiom: an integration-test root is a crate root, so it resolves `mod common;`
    beside itself rather than under `tests/<root name>/`.
    """
    d = os.path.dirname(path)
    base = os.path.basename(path)
    if is_root or base == "mod.rs":
        return d
    return os.path.join(d, base[: -len(".rs")])


def resolve_mod(path, is_root, name, exists=os.path.isfile):
    """`mod NAME;` -> the file it pulls in, or None if neither form is present."""
    d = module_dir(path, is_root)
    for cand in (os.path.join(d, name + ".rs"),
                 os.path.join(d, name, "mod.rs")):
        if exists(cand):
            return cand.replace(os.sep, "/")
    return None


def tokenize(src):
    """Yield (kind, text, line).

    THIS IS PARTIAL PARSING, AND IT FAILS CLOSED AND LOUDLY BY DESIGN.
    It is not a Rust parser and must never pretend to be one: it recognises just
    enough — comments, every literal form, attribute token trees, braces, and a
    handful of keywords — to know which scope a `#[ignore]` is in. Anything it
    mis-reads shows up as a declaration that is never observed (STALE) or an
    observation that was never declared (UNDECLARED), both of which fail the
    gate with the offending name printed. That is the right failure direction:
    the gate cannot quietly under-report.

    The cost of failing closed is that a *false* red on legitimate code is a
    usability defect, not a correctness one — and a gate that rejects ordinary
    repository structure is a gate someone will eventually switch off, at which
    point it protects nothing. So every construct that appears in normal Rust
    must be understood here, and `--self-test` pins the ones that have bitten:
    braces inside strings, raw strings, byte strings, char literals versus
    lifetimes, nested block comments, and attribute token trees.

    Comments and literals collapse to a single token so that braces inside them
    cannot move the module stack — `"fn main() { }"` in a fixture is a string,
    not a scope."""
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


DISABLING_INNER_CFG = re.compile(r'#!\[\s*cfg\s*\(\s*(?!test\s*\))')


def literal_text(tok):
    """The contents of a string-literal token, raw or not."""
    m = re.match(r'^(?:b)?(r(#*))?"', tok)
    if m and m.group(1):
        hashes = len(m.group(2))
        return tok[len(m.group(0)):-(1 + hashes)]
    return tok[tok.index('"') + 1:-1]


def scan_file(path, src, target, prefix):
    """Scan one file as part of one target, at one module prefix.

    -> (declarations, external `mod NAME;` sites, `include!` sites)

    The same file scanned for two targets yields two sets of declarations, which
    is correct: `tests/common/mod.rs` really is compiled into five test binaries
    and really does contribute five tests.
    """
    # A file switched off by an inner cfg contributes nothing. This is read from
    # the attribute, not guessed: `#![cfg(skip_for_now)]` at the top of
    # src/lsp/server_test.rs compiles the whole file out, and declaring its
    # contents would produce a STALE for a test that cannot exist.
    if DISABLING_INNER_CFG.search(src[:2000]):
        return [], [], []

    out, ext_mods, includes = [], [], []
    mods, depth, pending = [], 0, None
    expect_mod_name = expect_fn_name = False
    mod_name = None
    # `include!("…")` is matched as a token sequence rather than by a regex over
    # the file, because the included items land at the module path of the
    # INCLUDING SITE — which a file-level regex cannot see.
    inc_state = 0
    for kind, text, line in tokenize(src):
        if kind == "attr":
            if re.match(r"#\[\s*ignore", text):
                pending = (text, line)
            continue
        if kind == "str":
            if inc_state == 3:
                includes.append((literal_text(text), [m[1] for m in mods]))
            inc_state = 0
            continue
        if kind == "char":
            inc_state = 0
            continue
        if kind == "punct":
            if inc_state == 1 and text == "!":
                inc_state = 2
            elif inc_state == 2 and text == "(":
                inc_state = 3
            elif inc_state == 3:
                pass
            else:
                inc_state = 0
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
                if mod_name is not None:
                    # `mod NAME;` — the body is another file.
                    ext_mods.append((mod_name,
                                     [m[1] for m in mods], line))
                    mod_name = None
                expect_mod_name = False
            continue
        # ident
        inc_state = 1 if text == "include" else 0
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

    return out, ext_mods, includes


def walk_target(root, read=None, resolve=resolve_mod):
    """Walk one target's module graph from its root file.

    -> (declarations, set of files reached, unresolved `mod NAME;` sites)
    """
    if read is None:
        def read(p):
            with open(p, encoding="utf-8") as fh:
                return fh.read()

    decls, reached, unresolved = [], set(), []
    # (file, prefix, is_root); `include!` splices a file in at the including
    # site, so it inherits that site's prefix rather than its own location's.
    stack = [(root, [], True)]
    seen = set()
    while stack:
        path, prefix, is_root = stack.pop()
        if (path, tuple(prefix)) in seen:
            continue
        seen.add((path, tuple(prefix)))
        try:
            src = read(path)
        except OSError:
            continue
        reached.add(path)
        d, ext_mods, includes = scan_file(path, src, root, prefix)
        decls += d
        for name, inner_mods, line in ext_mods:
            child = resolve(path, is_root, name)
            if child is None:
                unresolved.append((path, line, name))
                continue
            stack.append((child, prefix + inner_mods + [name], False))
        for rel, inner_mods in includes:
            # include! is textual: relative to the including file's directory,
            # and the included items land at the including file's module path.
            inc = os.path.normpath(
                os.path.join(os.path.dirname(path), rel)).replace(os.sep, "/")
            stack.append((inc, prefix + inner_mods, is_root))
    return decls, reached, unresolved


def collect_declarations(roots=None, read=None, resolve=resolve_mod):
    """Every declaration in every target, plus the files no target reached."""
    if roots is None:
        roots = target_roots()
    decls, reached, unresolved = [], set(), []
    for root in roots:
        d, r, u = walk_target(root, read=read, resolve=resolve)
        decls += d
        reached |= r
        unresolved += u
    return decls, reached, unresolved


# --------------------------------------------------------------------------
# Cargo output parsing
# --------------------------------------------------------------------------

# The artifact path in parentheses is NOT anchored to `target/`: cargo prints
# whatever CARGO_TARGET_DIR says, which may be absolute or elsewhere entirely.
# Anchoring on it made every result unattributable — a false UNDECLARED for the
# whole suite — one environment variable away. The parentheses are enough to
# distinguish this from the compiler-under-test's own "   Running Constant
# Folding" chatter on stdout, which has none.
RUNNING_RE = re.compile(r"^\s+Running (?:unittests )?(\S+) \(")
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
            if (d["target"], d["path"]) not in SLOW_ALLOWLIST:
                problems.append(("TAG", "%s: %s is tagged SLOW but is not on the "
                                 "reviewed allowlist in scripts/test-xfail.py. A "
                                 "passing test must not be retired from the "
                                 "suite by relabelling it." % (loc, key)))
    return problems


# --------------------------------------------------------------------------
# Self-test: the two bijection shapes, plus the shapes that broke earlier
# --------------------------------------------------------------------------

def self_test():
    """Regressions for every way this scanner has been, or could be, wrong.

    These run before every real invocation. They are pure — no cargo, no disk —
    so the module graph is exercised through injected `read`/`resolve` hooks.
    """
    failures = []

    def check(label, got, want):
        if got != want:
            failures.append("%s\n     got:  %r\n     want: %r" % (label, got, want))

    def fake_tree(files):
        """A repository in a dict: path -> source."""
        def read(p):
            if p not in files:
                raise OSError(p)
            return files[p]

        def resolve(path, is_root, name):
            return resolve_mod(path, is_root, name,
                               exists=lambda c: c.replace(os.sep, "/") in files)
        return read, resolve

    XF = '#[ignore = "XFAIL: nope (owned by M1)"]'

    # 1. A module-nested ignored test must key on its full path. Keying on the
    #    bare function name made such a test STALE and UNDECLARED at once.
    nested = """
        #[cfg(test)]
        mod tests {
            #[test]
            %s
            fn test_a() { let s = "fn main() { }"; }
        }
    """ % XF
    d, _, _ = scan_file("src/optimizer/dead_code_test.rs", nested, "src/lib.rs",
                        ["optimizer", "dead_code_test"])
    check("nested test path", [x["path"] for x in d],
          ["optimizer::dead_code_test::tests::test_a"])
    check("nested test target", [x["target"] for x in d], ["src/lib.rs"])

    # 2. The same test name in two modules must be two keys, not a duplicate.
    dup = """
        mod a { #[test] %s fn test_dup() {} }
        mod b { #[test] %s fn test_dup() {} }
    """ % (XF, XF)
    d, _, _ = scan_file("tests/x_test.rs", dup, "tests/x_test.rs", [])
    check("two modules, two keys", sorted(x["path"] for x in d),
          ["a::test_dup", "b::test_dup"])
    obs = [{"target": "tests/x_test.rs", "path": p, "outcome": "FAILED"}
           for p in ("a::test_dup", "b::test_dup")]
    counts, problems = reconcile(d, obs)
    check("two modules reconcile clean", problems, [])
    check("two modules counted", counts["xfail"], 2)

    # 3. EXTERNAL `mod NAME;`. This is the ordinary shape — `tests/common/mod.rs`
    #    pulled in by five integration targets — and a filesystem walk gets it
    #    wrong in both directions at once.
    files = {
        "tests/e2e_test.rs":       "mod common;\n",
        "tests/integration_test.rs": "mod common;\n",
        "tests/common/mod.rs":     "#[test] %s fn test_helper() {}\n" % XF,
    }
    read, resolve = fake_tree(files)
    decls, reached, unresolved = collect_declarations(
        roots=["tests/e2e_test.rs", "tests/integration_test.rs"],
        read=read, resolve=resolve)
    check("external mod: one file, one declaration per target",
          sorted((x["target"], x["path"]) for x in decls),
          [("tests/e2e_test.rs", "common::test_helper"),
           ("tests/integration_test.rs", "common::test_helper")])
    check("external mod: file counted as reached",
          "tests/common/mod.rs" in reached, True)
    check("external mod: nothing unresolved", unresolved, [])
    # ... and it reconciles clean against what cargo would report.
    obs = [{"target": t, "path": "common::test_helper", "outcome": "FAILED"}
           for t in ("tests/e2e_test.rs", "tests/integration_test.rs")]
    _, problems = reconcile(decls, obs)
    check("external mod reconciles clean", problems, [])

    # 3b. `mod NAME;` nested inside an inline module, and the non-root
    #     directory rule: src/a.rs owns src/a/.
    files = {
        "src/lib.rs": "mod a;\n",
        "src/a.rs":   "mod inner { mod b; }\n",
        "src/a/b.rs": "#[test] %s fn test_deep() {}\n" % XF,
    }
    read, resolve = fake_tree(files)
    decls, _, unresolved = collect_declarations(roots=["src/lib.rs"],
                                                read=read, resolve=resolve)
    check("nested external mod path", [x["path"] for x in decls],
          ["a::inner::b::test_deep"])
    check("nested external mod resolved", unresolved, [])

    # 3c. An unresolvable `mod NAME;` is reported rather than silently dropped.
    files = {"tests/x.rs": "mod missing;\n"}
    read, resolve = fake_tree(files)
    _, _, unresolved = collect_declarations(roots=["tests/x.rs"],
                                            read=read, resolve=resolve)
    check("unresolved mod reported", [u[2] for u in unresolved], ["missing"])

    # 4. `include!` is textual: the included items take the INCLUDING file's
    #    module path, not one derived from where the file sits on disk.
    files = {
        "tests/y_test.rs": 'mod outer { include!("gen/table.rs"); }\n',
        "tests/gen/table.rs": "#[test] %s fn test_generated() {}\n" % XF,
    }
    read, resolve = fake_tree(files)
    decls, reached, _ = collect_declarations(roots=["tests/y_test.rs"],
                                             read=read, resolve=resolve)
    check("include! keyed at its insertion site",
          [(x["target"], x["path"]) for x in decls],
          [("tests/y_test.rs", "outer::test_generated")])
    check("include! file counted as reached",
          "tests/gen/table.rs" in reached, True)

    # 5. Full round trip against real cargo output shape.
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

    # 5b. A custom CARGO_TARGET_DIR changes the artifact path. Anchoring on
    #     `target/` made every result unattributable — a false UNDECLARED for
    #     the entire suite, one environment variable away.
    o, _ = parse_cargo(
        "     Running tests/x_test.rs (/abs/build/dir/release/deps/x_test-cd)\n"
        "test a::test_dup ... ok\n"
        "test result: ok. 1 passed\n")
    check("absolute CARGO_TARGET_DIR still attributes",
          [(x["target"], x["path"]) for x in o],
          [("tests/x_test.rs", "a::test_dup")])

    # 6. A target that starts and never reports must be caught.
    _, unreported = parse_cargo(
        "     Running tests/x_test.rs (target/release/deps/x_test-cd)\n"
        "test a::test_dup ... FAILED\n")
    check("unreported target detected", unreported, ["tests/x_test.rs"])

    # 7. An observation with no declaration is UNDECLARED.
    _, problems = reconcile([], [{"target": "tests/x_test.rs",
                                  "path": "a::test_dup", "outcome": "ok"}])
    check("undeclared detected", [k for k, _ in problems], ["UNDECLARED"])

    # 8. Every literal and comment form must be inert to the module stack:
    #    braces in ordinary and raw strings, byte strings, char literals versus
    #    lifetimes, nested block comments, and attribute token trees.
    tricky = r"""
        mod outer {
            // fn fake() { {{{
            /* outer /* nested } { */ still a comment } */
            #[doc = "a } brace { in an attribute"]
            #[cfg_attr(all(), allow(unused))]
            #[test]
            #[ignore = "XFAIL: has } and { in the reason (owned by M3)"]
            fn test_b<'a>(x: &'a str) {
                let _raw = r#"} } {"#;
                let _byte = b"} { ";
                let _braw = br##"} "# {"##;
                let _ch = '}';
                let _esc = '\'';
                let _s = "\" } {";
            }
        }
        #[test]
        #[ignore = "XFAIL: top level (owned by M3)"]
        fn test_c() {}
    """
    d, _, _ = scan_file("tests/z_test.rs", tricky, "tests/z_test.rs", [])
    check("literals and comments are inert",
          sorted(x["path"] for x in d), ["outer::test_b", "test_c"])

    # 9. A file switched off by an inner cfg contributes nothing, so its tests
    #    are not declared as expected failures that can never run.
    off = '#![cfg(skip_for_now)]\n#[test] %s fn test_off() {}\n' % XF
    d, _, _ = scan_file("src/lsp/server_test.rs", off, "src/lib.rs", ["lsp"])
    check("cfg-disabled file declares nothing", d, [])
    on = '#![cfg(test)]\n#[test] %s fn test_on() {}\n' % XF
    d, _, _ = scan_file("src/x.rs", on, "src/lib.rs", ["x"])
    check("cfg(test) file still declares", [x["path"] for x in d],
          ["x::test_on"])

    # 10. Tag validation, including the owner grammar and the allowlist key.
    bad = """
        #[test] #[ignore] fn test_bare() {}
        #[test] #[ignore = "because"] fn test_untagged() {}
        #[test] #[ignore = "XFAIL: x (owned by someday)"] fn test_bad_owner() {}
        #[test] #[ignore = "SLOW: not on the allowlist"] fn test_slow() {}
    """
    d, _, _ = scan_file("tests/z_test.rs", bad, "tests/z_test.rs", [])
    check("four tag problems", len(validate_tags(d)), 4)

    if failures:
        print("self-test FAILED:", file=sys.stderr)
        for f in failures:
            print("  " + f, file=sys.stderr)
        return False
    print("self-test: 22 checks green (module graph incl. external `mod` and "
          "`include!`, module-path keying, literal-safe scanning, cargo "
          "attribution, tag grammar)")
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

    decls, reached, unresolved = collect_declarations()
    problems = validate_tags(decls)

    for path, line, name in unresolved:
        problems.append(("MODULE", "%s:%d: `mod %s;` resolves to no file — the "
                                   "module graph could not be walked past it, so "
                                   "any declaration beyond it is invisible"
                         % (path, line, name)))

    # The module graph is the source of truth for what exists, but a declaration
    # in a file no target reaches is still a declaration nobody will honour.
    # Walking the filesystem afterwards keeps that from going unnoticed.
    for root in ("src", "tests"):
        for dirpath, _, names in os.walk(root):
            for name in sorted(names):
                if not name.endswith(".rs"):
                    continue
                fp = os.path.join(dirpath, name).replace(os.sep, "/")
                if fp in reached:
                    continue
                with open(fp, encoding="utf-8") as fh:
                    if re.search(r"#\[\s*ignore", fh.read()):
                        problems.append(("ORPHAN",
                                         "%s holds an #[ignore] but no target "
                                         "reaches it through the module graph, "
                                         "so the test does not exist" % fp))

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
        "MODULE": "Unresolvable `mod` declarations:",
        "ORPHAN": "Declarations in files no target compiles:",
        "NO_RESULT": "Targets that produced no result:",
        "BUILD": "Build errors:",
        "CARGO": "Unexplained cargo status:",
    }
    for kind in ("XPASS", "STALE", "UNDECLARED", "SLOWFAIL", "DUPLICATE",
                 "TAG", "MODULE", "ORPHAN", "NO_RESULT", "BUILD", "CARGO"):
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
