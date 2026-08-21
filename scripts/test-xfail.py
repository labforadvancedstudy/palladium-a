#!/usr/bin/env python3
"""Palladium expected-failure gate for the Rust test suite.

`scripts/conformance.sh` settled what an expected failure means for a .pd
program: it is declared with a mandatory reason; a declared failure that still
fails is XFAIL and is fine; a declared failure that PASSES is XPASS and fails the
gate; and a declared entry that is never evaluated is STALE and also fails the
gate, because "never ran" must not be indistinguishable from "failed as
expected". This applies the same three rules to the Rust tests, where the
declaration mechanism is `#[ignore = "…"]` rather than a manifest file.

ASK CARGO WHAT EXISTS; DO NOT RE-DERIVE IT.
Earlier versions of this gate built the declared inventory by parsing the crate:
walking the module graph, resolving `mod NAME;`, following `include!`, deciding
which `#[cfg]` was active. That is re-implementing rustc's module resolution in
Python, and it is unbounded — successive reviews each found another legitimate
construct it did not model (`#[path]`, mutually exclusive `cfg`s, directory
targets, `include!` lookup directories). A gate that rejects ordinary Rust is a
gate someone eventually switches off, and then it protects nothing.

So both inventories now come from the same authority:

    declared  = cargo test -- --list --ignored     every ignored test, per target
    observed  = cargo test -- --ignored            what running them did

Both print the FULL module path (`optimizer::dead_code_test::tests::test_x`) and
both attribute to a target by the `Running …` line that precedes them, so the key
is (target, module path) on both sides — and reconciliation compares two cargo
outputs instead of comparing cargo against a hand-written parser.

That deletes target discovery, the module graph walk, `mod`/`#[path]`/`include!`
resolution, the cfg reachability question, and the ORPHAN check: a file no target
compiles contributes no test to the listing, which is the fact ORPHAN was trying
to infer from the filesystem.

WHAT STILL NEEDS THE SOURCE: the reason text.
Cargo reports *that* a test is ignored, never *why*. So for each name cargo has
already given us, we look for the `#[ignore = "…"]` attached to `fn <name>` — a
local search keyed on a symbol known to exist, not a model of the crate. A test
cargo calls ignored whose reason cannot be found, or which carries a bare
`#[ignore]`, fails the gate: that is an undeclared expected failure.

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

MILESTONE EXIT, AS A COMMAND — AND WHY IT HAD TO BE ADDED HERE
`scripts/conformance.sh` already turns a milestone's exit criterion into a
command: `CONFORMANCE_FORBID_OWNER=M1` fails if any evaluated untranscribed /
vacuous / xfail row in `tests/conformance-manifest.txt` is still owed to M1.

But this repository has TWO owner inventories, not one. The manifest owns .pd
fixtures; `#[ignore]` reasons own Rust tests, and they carry `(owned by M<n>)`
in exactly the same grammar (`OWNER_RE` below). `make m1-exit` consulted only
the first, so it exited 0 while three M1-owned `#[ignore]` rows were still
failing — one of them the tail-`if` miscompile that M1 was named for. The
release that exists to remove silent miscompiles shipped one, and its own exit
criterion could not see it.

So `TEST_XFAIL_FORBID_OWNER=M<n>` is the mirror of `CONFORMANCE_FORBID_OWNER`,
deliberately using the same word (`OWED_TO_M<n>`) and the same shape of line, so
that "what does milestone N still owe" is one idea with one vocabulary rather
than two dialects. `make m1-exit` now runs both and is red unless both are
clean.

Note the same limit the conformance runner states about itself: the owner is an
editable label, so retagging a row to another milestone slips this check. The
authorisation boundary is REVIEW OF THE REASON TEXT, not this script.

Env:
  TEST_XFAIL_FORBID_OWNER   fail if any still-failing XFAIL is owned by this
                            milestone (e.g. M1). Unset by default, so the
                            ordinary `make test-xfail` is unaffected.

Usage: scripts/test-xfail.py [--self-test]
"""

import os
import re
import subprocess
import sys

# Reviewed allowlist of tests that may be #[ignore]d for cost alone. Keyed by
# the SAME identity reconciliation uses — (target, full module path) — because
# keying it by bare function name would hand permission to every same-named test
# in the file.
SLOW_ALLOWLIST = {
    ("tests/stress_test.rs", "test_extremely_large_program"),
}

OWNER_RE = re.compile(r"\(owned by (M[1-9]|unscheduled)([,;: )]|$)")
FN_MODIFIERS = {"pub", "async", "unsafe", "extern", "const", "default"}


# --------------------------------------------------------------------------
# Cargo output parsing — the only source of truth for what tests exist
# --------------------------------------------------------------------------

# The artifact path in parentheses is NOT anchored to `target/`: cargo prints
# whatever CARGO_TARGET_DIR says, which may be absolute or elsewhere entirely.
# Anchoring on it made every result unattributable — a false UNDECLARED for the
# whole suite — one environment variable away. The parentheses are enough to
# tell this from the compiler-under-test's own "   Running Constant Folding"
# chatter on stdout, which has none.
RUNNING_RE = re.compile(r"^\s+Running (?:unittests )?(\S+) \(")
DOCTEST_RE = re.compile(r"^\s+Doc-tests ")
RESULT_RE = re.compile(r"^test result:")
TEST_RE = re.compile(r"^test (\S+) \.\.\. (ok|FAILED)$")
LIST_RE = re.compile(r"^(\S+): test$")


def parse_list(out):
    """`cargo test -- --list --ignored` -> [(target, module path)]."""
    target, listed = None, []
    for raw in out.splitlines():
        m = RUNNING_RE.match(raw)
        if m:
            target = m.group(1)
            continue
        if DOCTEST_RE.match(raw):
            target = "doc-tests"
            continue
        m = LIST_RE.match(raw)
        if m:
            listed.append((target, m.group(1)))
    return listed


def parse_run(out):
    """`cargo test -- --ignored` -> (observations, targets that never reported)."""
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
            obs.append((target, m.group(1), m.group(2)))
    if open_target:
        unreported.append(open_target)
    return obs, unreported


# --------------------------------------------------------------------------
# Finding the reason for a test cargo has already named
# --------------------------------------------------------------------------

def tokenize(src):
    """Yield (kind, text, line).

    THIS IS PARTIAL PARSING, AND IT FAILS CLOSED AND LOUDLY BY DESIGN.
    It is not a Rust parser and must never pretend to be one. Its whole job is
    now: given a function name cargo has already said exists, find the
    `#[ignore]` attached to it without being fooled by a `fn` or a brace inside
    a string or a comment. It no longer decides WHAT EXISTS, only what a known
    thing says — which is why the module graph, `#[path]`, `include!` and the
    `cfg` reachability question are gone from this file entirely.

    When it mis-reads something the result is a reason it cannot find, which is
    printed by name and fails the gate. It cannot quietly under-report.

    Comments and literals collapse to one token so braces inside them cannot
    move the module stack — `"fn main() { }"` in a fixture is a string, not a
    scope."""
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


def find_ignore_sites(path, src):
    """Every `#[ignore…]`-carrying fn in one file.

    `inline_path` is the function qualified by the `mod` blocks written IN THIS
    FILE. That is a suffix of the module path cargo prints, and a suffix is all
    the disambiguation needed — the rest of cargo's path comes from how the file
    is included, which is exactly what this script no longer tries to model.
    """
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
                    "name": text,
                    "inline_path": "::".join([m[1] for m in mods] + [text]),
                    "attr": attr,
                    "line": attrline,
                    "file": path,
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


def index_sites(files):
    """bare fn name -> [site]. `files` is {path: source}."""
    index = {}
    for path in sorted(files):
        for site in find_ignore_sites(path, files[path]):
            index.setdefault(site["name"], []).append(site)
    return index


def read_sources(roots=("src", "tests")):
    files = {}
    for root in roots:
        for dirpath, _, names in os.walk(root):
            for name in sorted(names):
                if name.endswith(".rs"):
                    p = os.path.join(dirpath, name).replace(os.sep, "/")
                    with open(p, encoding="utf-8") as fh:
                        files[p] = fh.read()
    return files


def find_reason(target, path, index):
    """-> (candidate sites, error). Exactly one of the two is falsy.

    More than one candidate is normal and not an error. `#[cfg(unix)] mod m`
    beside `#[cfg(not(unix))] mod m` is two honest declarations of one test,
    only one of which compiles, and this script deliberately does not evaluate
    `cfg` — that was the road to re-implementing rustc. So every candidate is
    validated and the caller complains only if they DISAGREE about what kind of
    declaration this is. Differing reason text between arms is fine; differing
    tags, or a malformed one, is not.
    """
    name = path.rsplit("::", 1)[-1]
    cands = index.get(name, [])
    if not cands:
        return [], ('no `#[ignore = "…"]` could be found for it. cargo '
                    "reports it as ignored, so it is an expected failure "
                    "with no declared reason — a bare #[ignore], a "
                    "#[cfg_attr(…, ignore)], or a definition this script "
                    "could not locate")
    # cargo's path ends with the module nesting written in the defining file.
    exact = [c for c in cands
             if path == c["inline_path"] or path.endswith("::" + c["inline_path"])]
    if not exact:
        exact = cands
    if len(exact) > 1:
        # Prefer a definition in the target's own root file; that is what
        # separates two same-named tests in two integration targets.
        own = [c for c in exact if c["file"] == target]
        if own:
            exact = own
    return exact, None


# --------------------------------------------------------------------------
# Reconciliation
# --------------------------------------------------------------------------

def classify(site):
    attr = site["attr"]
    if re.search(r'ignore\s*=\s*"XFAIL:', attr):
        return "XFAIL"
    if re.search(r'ignore\s*=\s*"SLOW:', attr):
        return "SLOW"
    if re.match(r"#\[\s*ignore\s*\]", attr):
        return "BARE"
    return "UNTAGGED"


def owner_of(site):
    """The milestone an XFAIL is owed to, or None. Same grammar as OWNER_RE."""
    m = OWNER_RE.search(site["attr"])
    return m.group(1) if m else None


def reconcile(listed, obs, index, forbid_owner=None):
    """listed: [(target, path)]; obs: [(target, path, outcome)].

    Keys are (target, module path) on both sides. -> (counts, problems)

    `forbid_owner` is the milestone-exit gate: when set to "M<n>", a declared
    XFAIL owned by that milestone which is STILL FAILING is reported as
    OWED_TO_M<n>. An XFAIL that now passes is not owed — it is an XPASS, which
    already fails the gate through its own path and asks for the `#[ignore]` to
    be deleted.
    """
    problems = []
    counts = {"xfail": 0, "xpass": 0, "slow_pass": 0,
              "declared_xfail": 0, "declared_slow": 0, "owed": 0}

    tags, sites = {}, {}
    for key in listed:
        target, path = key
        cands, err = find_reason(target, path, index)
        if err:
            problems.append(("TAG", "%s::%s: %s" % (target, path, err)))
            tags[key] = None
            continue
        kinds = {classify(c) for c in cands}
        if len(kinds) > 1:
            where = ", ".join("%s:%d [%s]" % (c["file"], c["line"], classify(c))
                              for c in cands)
            problems.append(("TAG", "%s::%s: its candidate declarations "
                                    "disagree about what it is (%s)"
                             % (target, path, where)))
            tags[key] = None
            continue
        site = cands[0]
        sites[key] = site
        tag = kinds.pop()
        tags[key] = tag
        loc = ", ".join("%s:%d" % (c["file"], c["line"]) for c in cands)
        if tag == "BARE":
            problems.append(("TAG", "%s: bare #[ignore] on %s::%s — every "
                             "expected failure needs a reason: #[ignore = "
                             '"XFAIL: <missing feature> (owned by M<n>)"] or '
                             '#[ignore = "SLOW: <why>"]' % (loc, target, path)))
        elif tag == "UNTAGGED":
            problems.append(("TAG", "%s: #[ignore] reason on %s::%s must start "
                             "with 'XFAIL: ' or 'SLOW: '" % (loc, target, path)))
        elif tag == "XFAIL":
            counts["declared_xfail"] += 1
            if not all(OWNER_RE.search(c["attr"]) for c in cands):
                problems.append(("TAG", "%s: XFAIL reason on %s::%s names no "
                                 "valid owner. Use '(owned by M<n>)' with n in "
                                 "1..9, or '(owned by unscheduled…)' for work "
                                 "under MILESTONES.md 'Not scheduled, and why'."
                                 % (loc, target, path)))
        elif tag == "SLOW":
            counts["declared_slow"] += 1
            if key not in SLOW_ALLOWLIST:
                problems.append(("TAG", "%s: %s::%s is tagged SLOW but is not "
                                 "on the reviewed allowlist in "
                                 "scripts/test-xfail.py. A passing test must "
                                 "not be retired from the suite by relabelling "
                                 "it." % (loc, target, path)))

    listed_set = set(listed)
    for key in sorted(k for k in listed_set if listed.count(k) > 1):
        problems.append(("DUPLICATE", "%s::%s listed more than once" % key))

    observed_keys = set()
    for target, path, outcome in obs:
        key = (target, path)
        observed_keys.add(key)
        if key not in listed_set:
            problems.append(("UNDECLARED", "%s::%s (%s) ran but `--list "
                             "--ignored` does not list it"
                             % (target, path, outcome)))
            continue
        tag = tags.get(key)
        if tag == "XFAIL":
            if outcome == "ok":
                counts["xpass"] += 1
                problems.append(("XPASS", "%s::%s\n      was: %s"
                                 % (target, path, sites[key]["attr"].strip())))
            else:
                counts["xfail"] += 1
                # Milestone gate. Deliberately worded like the conformance
                # runner's OWED_TO_ line so the two inventories read the same.
                if forbid_owner and owner_of(sites[key]) == forbid_owner:
                    counts["owed"] += 1
                    site = sites[key]
                    problems.append((
                        "OWED",
                        "%s::%s [OWED_TO_%s] class=xfail is still owed to %s\n"
                        "      declared at %s:%d\n"
                        "      reason: %s"
                        % (target, path, forbid_owner, forbid_owner,
                           site["file"], site["line"], site["attr"].strip())))
        elif tag == "SLOW":
            if outcome == "ok":
                counts["slow_pass"] += 1
            else:
                problems.append(("SLOWFAIL", "%s::%s" % (target, path)))

    for key in sorted(listed_set):
        if key not in observed_keys:
            problems.append(("STALE", "%s::%s" % key))

    counts["listed"] = len(listed_set)
    counts["observed"] = len(observed_keys)
    return counts, problems


# --------------------------------------------------------------------------
# Self-test: what is left to get wrong is reason lookup
# --------------------------------------------------------------------------

def self_test():
    failures = []

    def check(label, got, want):
        if got != want:
            failures.append("%s\n     got:  %r\n     want: %r"
                            % (label, got, want))

    XF = '#[ignore = "XFAIL: nope (owned by M1)"]'

    # 1. The same bare name in two modules of one file: cargo's module path
    #    selects the right reason.
    src = """
        mod a { #[test] #[ignore = "XFAIL: from a (owned by M1)"] fn test_dup() {} }
        mod b { #[test] #[ignore = "XFAIL: from b (owned by M2)"] fn test_dup() {} }
    """
    index = index_sites({"tests/x_test.rs": src})
    c, err = find_reason("tests/x_test.rs", "a::test_dup", index)
    check("two modules: picks a", (err, "from a" in c[0]["attr"]), (None, True))
    c, err = find_reason("tests/x_test.rs", "b::test_dup", index)
    check("two modules: picks b", (err, "from b" in c[0]["attr"]), (None, True))

    # 2. The same bare name in two integration targets: the target's own root
    #    file wins.
    index = index_sites({
        "tests/one_test.rs":
            '#[test] #[ignore = "XFAIL: one (owned by M1)"] fn test_same() {}',
        "tests/two_test.rs":
            '#[test] #[ignore = "XFAIL: two (owned by M2)"] fn test_same() {}',
    })
    c, err = find_reason("tests/two_test.rs", "test_same", index)
    check("two targets: picks its own root",
          (err, len(c), "two" in c[0]["attr"]), (None, 1, True))

    # 3. A shared module compiled into several targets is ONE site, so every
    #    target resolves to the same reason. This is tests/common/mod.rs, the
    #    shape that broke every filesystem-derived version of this gate.
    index = index_sites({
        "tests/common/mod.rs": "#[test] %s fn test_helper() {}" % XF,
        "tests/e2e_test.rs": "mod common;",
        "tests/integration_test.rs": "mod common;",
    })
    a, ea = find_reason("tests/e2e_test.rs", "common::test_helper", index)
    b, eb = find_reason("tests/integration_test.rs", "common::test_helper", index)
    check("shared module resolves for every target",
          (ea, eb, a[0] is b[0]), (None, None, True))

    # 4. A missing reason is reported, not guessed.
    _, err = find_reason("tests/x_test.rs", "test_absent", index_sites({}))
    check("missing reason reported", err is not None, True)

    # 5. Mutually exclusive `#[cfg]` arms are TWO honest declarations of ONE
    #    test. Only one compiles; this script does not evaluate cfg, so it must
    #    accept them as long as they agree about what kind of declaration it is.
    cfg_arms = """
        #[cfg(unix)]
        mod m { #[test] #[ignore = "XFAIL: unix arm (owned by M1)"] fn test_a() {} }
        #[cfg(not(unix))]
        mod m { #[test] #[ignore = "XFAIL: other arm (owned by M1)"] fn test_a() {} }
    """
    index = index_sites({"tests/p/main.rs": cfg_arms})
    cands, err = find_reason("tests/p/main.rs", "m::test_a", index)
    check("cfg arms are candidates, not an error", (err, len(cands)), (None, 2))
    _, problems = reconcile([("tests/p/main.rs", "m::test_a")],
                            [("tests/p/main.rs", "m::test_a", "FAILED")], index)
    check("cfg arms reconcile clean", problems, [])

    # 5b. ... but arms that disagree about the KIND of declaration are an error,
    #     because then the verdict would depend on which one was read.
    mixed = """
        #[cfg(unix)]
        mod m { #[test] #[ignore = "XFAIL: unix arm (owned by M1)"] fn test_b() {} }
        #[cfg(not(unix))]
        mod m { #[test] #[ignore = "SLOW: other arm"] fn test_b() {} }
    """
    index = index_sites({"tests/p/main.rs": mixed})
    _, problems = reconcile([("tests/p/main.rs", "m::test_b")],
                            [("tests/p/main.rs", "m::test_b", "FAILED")], index)
    check("disagreeing arms are an error",
          [k for k, _ in problems], ["TAG"])

    # 5c. A malformed owner in ANY arm is caught, not just the first.
    bad_arm = """
        #[cfg(unix)]
        mod m { #[test] #[ignore = "XFAIL: ok (owned by M1)"] fn test_c() {} }
        #[cfg(not(unix))]
        mod m { #[test] #[ignore = "XFAIL: bad (owned by someday)"] fn test_c() {} }
    """
    index = index_sites({"tests/p/main.rs": bad_arm})
    _, problems = reconcile([("tests/p/main.rs", "m::test_c")],
                            [("tests/p/main.rs", "m::test_c", "FAILED")], index)
    check("malformed owner in any arm is caught",
          [k for k, _ in problems], ["TAG"])

    # 6. Tag classification and the owner grammar.
    sites = find_ignore_sites("t.rs", """
        #[test] #[ignore] fn test_bare() {}
        #[test] #[ignore = "because"] fn test_untagged() {}
        #[test] #[ignore = "XFAIL: x (owned by M1)"] fn test_xfail() {}
        #[test] #[ignore = "SLOW: x"] fn test_slow() {}
    """)
    check("tags", [classify(s) for s in sites],
          ["BARE", "UNTAGGED", "XFAIL", "SLOW"])
    check("owner grammar rejects prose",
          bool(OWNER_RE.search('#[ignore = "XFAIL: x (owned by someday)"]')),
          False)

    # 7. Every literal and comment form must be inert to the scan: braces in
    #    ordinary and raw strings, byte strings, char literals versus
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
    check("literals and comments are inert",
          sorted(s["inline_path"] for s in find_ignore_sites("t.rs", tricky)),
          ["outer::test_b", "test_c"])

    # 8. --list and the run are keyed the same way, and a custom
    #    CARGO_TARGET_DIR (absolute artifact path) must still attribute.
    listed = parse_list(
        "     Running unittests src/lib.rs (target/release/deps/palladium-ab)\n"
        "optimizer::dead_code_test::tests::test_a: test\n"
        "1 test, 0 benchmarks\n"
        "     Running tests/x_test.rs (/abs/elsewhere/deps/x_test-cd)\n"
        "a::test_dup: test\n")
    check("--list keyed by (target, path)", listed,
          [("src/lib.rs", "optimizer::dead_code_test::tests::test_a"),
           ("tests/x_test.rs", "a::test_dup")])

    obs, unreported = parse_run(
        "     Running unittests src/lib.rs (target/release/deps/palladium-ab)\n"
        "test optimizer::dead_code_test::tests::test_a ... FAILED\n"
        "test result: FAILED. 0 passed; 1 failed\n"
        "     Running tests/x_test.rs (/abs/elsewhere/deps/x_test-cd)\n"
        "test a::test_dup ... ok\n"
        "test result: ok. 1 passed\n")
    check("run keyed the same way", obs,
          [("src/lib.rs", "optimizer::dead_code_test::tests::test_a", "FAILED"),
           ("tests/x_test.rs", "a::test_dup", "ok")])
    check("no unreported target", unreported, [])

    _, unreported = parse_run(
        "     Running tests/x_test.rs (target/release/deps/x_test-cd)\n"
        "test a::test_dup ... FAILED\n")
    check("unreported target detected", unreported, ["tests/x_test.rs"])

    # 9. The verdicts themselves.
    index = index_sites({
        "tests/x_test.rs":
            '#[test] #[ignore = "XFAIL: x (owned by M1)"] fn test_x() {}\n'
            '#[test] #[ignore = "SLOW: cheap"] fn test_s() {}'})
    listed = [("tests/x_test.rs", "test_x")]
    _, problems = reconcile(listed, [("tests/x_test.rs", "test_x", "FAILED")],
                            index)
    check("xfail is clean", problems, [])
    _, problems = reconcile(listed, [("tests/x_test.rs", "test_x", "ok")], index)
    check("xpass fails the gate", [k for k, _ in problems], ["XPASS"])
    _, problems = reconcile(listed, [], index)
    check("listed but never ran -> STALE", [k for k, _ in problems], ["STALE"])
    _, problems = reconcile([], [("tests/x_test.rs", "test_x", "FAILED")], index)
    check("ran but not listed -> UNDECLARED",
          [k for k, _ in problems], ["UNDECLARED"])
    _, problems = reconcile([("tests/x_test.rs", "test_s")],
                            [("tests/x_test.rs", "test_s", "ok")], index)
    check("SLOW off the allowlist fails", [k for k, _ in problems], ["TAG"])

    # 10. The milestone gate. This is the check the repo did not have: an
    #     `#[ignore]` owed to M1 and still failing must fail `make m1-exit`,
    #     while the ordinary `make test-xfail` (no owner set) stays green on the
    #     very same input. Both halves matter — a gate that fires always is as
    #     useless as one that never fires.
    owners = index_sites({
        "tests/x_test.rs":
            '#[test] #[ignore = "XFAIL: a (owned by M1)"] fn test_m1() {}\n'
            '#[test] #[ignore = "XFAIL: b (owned by M2)"] fn test_m2() {}'})
    both = [("tests/x_test.rs", "test_m1"), ("tests/x_test.rs", "test_m2")]
    both_failed = [("tests/x_test.rs", "test_m1", "FAILED"),
                   ("tests/x_test.rs", "test_m2", "FAILED")]

    c, problems = reconcile(both, both_failed, owners)
    check("no owner set: still-failing XFAILs are clean",
          ([k for k, _ in problems], c["owed"]), ([], 0))

    c, problems = reconcile(both, both_failed, owners, forbid_owner="M1")
    check("forbid M1: only the M1 row is owed",
          ([k for k, _ in problems], c["owed"]), (["OWED"], 1))
    check("the OWED line names the milestone and the declaration site",
          ("[OWED_TO_M1]" in problems[0][1]
           and "tests/x_test.rs:1" in problems[0][1]),
          True)

    c, problems = reconcile(both, both_failed, owners, forbid_owner="M2")
    check("forbid M2 selects a different row",
          ("test_m2" in problems[0][1], c["owed"]), (True, 1))

    # An XPASS is NOT owed: it passes, so the milestone does not owe it — what
    # it owes is the deletion of the `#[ignore]`, which XPASS already demands.
    # Counting it as owed too would report one defect as two.
    _, problems = reconcile(both,
                            [("tests/x_test.rs", "test_m1", "ok"),
                             ("tests/x_test.rs", "test_m2", "FAILED")],
                            owners, forbid_owner="M1")
    check("an XPASS is reported as XPASS, not as owed",
          sorted(k for k, _ in problems), ["XPASS"])

    if failures:
        print("self-test FAILED:", file=sys.stderr)
        for f in failures:
            print("  " + f, file=sys.stderr)
        return False
    print("self-test: 26 checks green (reason lookup incl. same name in two "
          "modules and in two targets, shared module, missing and ambiguous "
          "reasons, literal-safe scanning, cargo attribution, verdicts, and the "
          "milestone-owner gate incl. its off state)")
    return True


# --------------------------------------------------------------------------

def run_cargo(extra):
    """cargo test with the two streams merged BY THE OS.

    Cargo prints `Running <target>` to stderr and test lines to stdout, and it
    is their interleaving that says which target a test belongs to. Capturing
    the two separately and concatenating puts every target header after every
    result, and everything parses with target None.
    """
    proc = subprocess.run(
        ["cargo", "test", "--release", "--no-fail-fast", "--"] + extra,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    return proc.returncode, proc.stdout


def build_error(out):
    if re.search(r"^error\[", out, re.M) or re.search(
            r"^error: (could not compile|failed to|expected)", out, re.M):
        return "; ".join(re.findall(r"^error.*", out, re.M)[:3])
    return None


def main():
    os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

    if not self_test():
        print("\nthe gate's own reason lookup is broken; not running it",
              file=sys.stderr)
        return 1
    if "--self-test" in sys.argv:
        return 0

    problems = []

    _, out_list = run_cargo(["--list", "--ignored"])
    err = build_error(out_list)
    if err:
        print("error: the ignored set did not build: " + err, file=sys.stderr)
        return 1
    listed = parse_list(out_list)

    rc_run, out_run = run_cargo(["--ignored"])
    err = build_error(out_run)
    if err:
        problems.append(("BUILD", "the ignored set did not build: " + err))
    obs, unreported = parse_run(out_run)
    for t in unreported:
        problems.append(("NO_RESULT", "%s started and never reported a result "
                                      "— it did not run at all" % t))
    if rc_run != 0 and not any(o[2] == "FAILED" for o in obs):
        problems.append(("CARGO", "cargo exited %d with no failing test to "
                                  "explain it" % rc_run))

    forbid_owner = os.environ.get("TEST_XFAIL_FORBID_OWNER", "").strip() or None
    if forbid_owner and not re.fullmatch(r"M[1-9]|unscheduled", forbid_owner):
        # Fail closed. A typo'd milestone would match no owner and the gate
        # would report "nothing owed" — a green run that established nothing,
        # which is the failure mode this whole file exists to remove.
        print("error: TEST_XFAIL_FORBID_OWNER=%r is not a valid owner "
              "(M1..M9 or 'unscheduled')" % forbid_owner, file=sys.stderr)
        return 1

    counts, more = reconcile(listed, obs, index_sites(read_sources()),
                             forbid_owner=forbid_owner)
    problems += more

    print("==============================================")
    print("cargo lists %d ignored test(s); %d of them ran"
          % (counts["listed"], counts["observed"]))
    print("declared: xfail=%d slow=%d"
          % (counts["declared_xfail"], counts["declared_slow"]))
    print("ran:      xfail=%d xpass=%d slow_pass=%d"
          % (counts["xfail"], counts["xpass"], counts["slow_pass"]))
    print("  xfail      = declared missing-feature test, still failing — as expected")
    print("  xpass      = declared failing but PASSED — a stale expectation, fails the gate")
    print("  slow       = passes, excluded only for cost, on the reviewed allowlist")
    print("  stale      = cargo lists it as ignored, running it reported nothing")
    print("  undeclared = it ran as ignored but the listing does not have it")
    if forbid_owner:
        # Print the denominator even when it is zero: "0 owed" out of a stated
        # number of XFAILs is a measurement, "no output" is not.
        print("milestone gate: TEST_XFAIL_FORBID_OWNER=%s -> %d of %d still-failing "
              "XFAIL(s) owed to %s"
              % (forbid_owner, counts["owed"], counts["xfail"], forbid_owner))
    print("==============================================")

    titles = {
        "OWED": ("OWED — still failing and still owed to %s, so that milestone is "
                 "not finished:" % forbid_owner),
        "XPASS": "XPASS — these now pass; delete the #[ignore] so they join the regression net:",
        "STALE": "STALE — listed as ignored but never reported a result:",
        "UNDECLARED": "UNDECLARED — ran as ignored but is not in the listing:",
        "SLOWFAIL": "SLOW test failed — it is declared as passing-but-expensive, so this is a real regression:",
        "DUPLICATE": "DUPLICATE listings:",
        "TAG": "#[ignore] declaration errors:",
        "NO_RESULT": "Targets that produced no result:",
        "BUILD": "Build errors:",
        "CARGO": "Unexplained cargo status:",
    }
    for kind in ("OWED", "XPASS", "STALE", "UNDECLARED", "SLOWFAIL", "DUPLICATE",
                 "TAG", "NO_RESULT", "BUILD", "CARGO"):
        items = [m for k, m in problems if k == kind]
        if items:
            print()
            print(titles[kind])
            for m in items:
                print("  " + m)

    if not problems:
        print("✓ every ignored test cargo knows about has a declared reason, "
              "and every declared failure is still failing")
        return 0
    sys.stdout.flush()
    print("\n%d problem(s) above." % len(problems), file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
