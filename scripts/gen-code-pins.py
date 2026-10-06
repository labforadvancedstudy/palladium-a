#!/usr/bin/env python3
"""Write the `code=` pins of every refusal-witness manifest row — GI-12 cutover, spec R9/R11.

WHAT IT DOES. Reads docs/contributing/diagnostic-pin-map.tsv (the locked semantic map) and
rewrites column 4 of every `reject` and `skip` row of tests/conformance-manifest.txt to
`code=PD####` or `code=PD####;msg~<fragment>`. Every other byte of the manifest is left as
it was, so the diff of a run is exactly the pins and nothing else.

WHY A GENERATOR AND NOT 124 HAND EDITS. A hand-edited pin is a claim about a payload the
editor read once. Here every claim is re-measured from the compiler on every run, and the
run REFUSES TO WRITE ANYTHING unless all of the following are true of the binary in
target/release/pdc right now:

  * the manifest's refusal rows and the map's rows are the SAME SET of (path, class), in
    both directions — a row in one and not the other is never silently skipped. (A
    `reject` row at stage `run` pins `exit=<N>`, a rule this tool does not own: it is
    outside the set and its column 4 is never touched. None exists today.);
  * no map row is PROVISIONAL (R11): a fragment verified only against a header the
    compiler no longer prints is not a fragment;
  * `discrimination_required` and `msg_tilde` agree, row by row;
  * each fixture is REFUSED by the front end (exit 1) with exactly one coded primary
    header, and that header's code is the code the map assigns it;
  * each code's witnesses partition into PAYLOAD GROUPS — rows whose primary payloads
    are character-identical — and a bare pin (`msg_tilde=no`) is written only for a
    code with ONE group: a single witness, or twins and nothing else. A bare pin is
    decided on the code alone, so on a code with two groups it accepts the other
    group's refusal (suF-a review round 1: a `total` refusal satisfied the three
    `frobnicate` rows of PD0006);
  * on a code with two or more groups, EVERY row carries a `msg~` fragment that occurs
    in its own group's PAYLOAD and in NO other group's (a fragment that selects
    another group too is an accepting pin wearing a discriminating fragment's
    clothes), and twins share one fragment. An error names the group;
  * a row tagged IDENTICAL really has a twin: some same-code sibling prints its
    payload character for character;
  * every pin it would write matches the grammar conformance.sh enforces.

THE PAYLOADS COME FROM THE SHARED PARSER, not from a re-implementation of it: each
fixture's stderr is captured and handed to `pd_diag_parse` in scripts/lib/diag-parse.sh,
the one parser conformance.sh adjudicates with. A generator that read payloads its own
way could certify a fragment the gate then fails to find.

THE MAP IS INPUT, AND IT IS HELD TO THE MANIFEST. conformance.sh reads the manifest and
this tool reads the map. `scripts/check-diagnostic-codes.sh` compares the two on every
run, TEXTUALLY and without compiling: every refusal row's manifest pin must be exactly
the pin the map dictates (`code=<code>`, plus `;msg~<fragment>` where the map says
`msg_tilde=yes`), and both must name the same refusal rows. So a hand edit of a manifest
pin goes red there; a pin is changed in the map and re-derived here, where every check
above is re-made. Running this again re-derives every pin wholesale and is idempotent:
on an unchanged compiler and map, the second run writes the same bytes as the first.

RECEIPT. Before and after writing, it prints two sha256 digests of the manifest's
sorted `path<TAB>class<TAB>stage` lines — over every row, and over the refusal rows
alone (the set `scripts/check-diagnostic-codes.sh` pins as REFUSAL_SET_SHA, computed
the same way). The two pairs must be equal: this tool rewrites observables, never
membership, class or stage.

SELF-TEST. `--self-test` runs `decide()` — a pure function of (map rows, manifest,
measured payloads) — over planted inputs, and compiles nothing: a two-group code with a
bare pin, a fragment that also selects another group, twins with two fragments, and
an IDENTICAL tag with no twin must each be REFUSED, naming what is wrong, and a
one-group code's twins pinned bare must be written. `make check-diagnostic-codes` runs it.

Usage:  python3 scripts/gen-code-pins.py [--map PATH] [--manifest PATH]
        python3 scripts/gen-code-pins.py --self-test
Exit:   0 pins written (or already current)   1 refused: the map is not true of this
        compiler, nothing written             2 could not measure
        (--self-test: 0 every planted case decided as required, 1 otherwise)
"""
import argparse
import hashlib
import re
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PDC = ROOT / "target/release/pdc"
PARSER = ROOT / "scripts/lib/diag-parse.sh"

MAP_COLUMNS = ["code", "condition", "witnesses", "row", "class", "param_dep", "witness_tag",
               "discrimination_required", "msg_tilde", "candidate_fragment",
               "single_witness_shape"]
REFUSAL_CLASSES = ("reject", "skip")
# The grammar scripts/conformance.sh validates column 4 of a reject/skip row against.
# Restated here only to refuse to WRITE a pin the runner would refuse to READ.
PIN_GRAMMAR = re.compile(r"^code=PD[0-9]{4}(;msg~.+)?$")


class CannotMeasure(Exception):
    """Exit 2: something this run needed could not be read or run."""


def rel_or_abs(p: str) -> Path:
    q = Path(p)
    return q if q.is_absolute() else ROOT / q


def load_map(path: Path):
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise CannotMeasure(f"cannot read the map {path}: {exc}") from exc
    lines = [l for l in text.split("\n") if l and not l.startswith("#")]
    if not lines:
        raise CannotMeasure(f"the map {path} has no header row")
    header = lines[0].split("\t")
    if header != MAP_COLUMNS:
        raise CannotMeasure(f"the map's header is {header}, not {MAP_COLUMNS}")
    rows = []
    for n, line in enumerate(lines[1:], 2):
        cols = line.split("\t")
        if len(cols) != len(MAP_COLUMNS):
            raise CannotMeasure(f"map data row {n} has {len(cols)} columns, want "
                                f"{len(MAP_COLUMNS)}: {line[:80]}")
        rows.append(dict(zip(MAP_COLUMNS, cols)))
    return rows


def manifest_rows(text: str):
    """-> [(line_index, cols)] for every declared row, comments and blanks skipped exactly
    the way scripts/conformance.sh skips them."""
    out = []
    for i, line in enumerate(text.split("\n")):
        body = line.rstrip("\r")
        if not body.strip() or body.startswith("#"):
            continue
        out.append((i, body.split("\t")))
    return out


def membership_digests(text: str):
    """(sha256 over every row's `path<TAB>class<TAB>stage`, sha256 over the refusal
    rows', count). The stage is in both since suF-a review round 1: a retype from
    `reject|compile` to `reject|run` keeps path and class and moves the row out of
    the code-pinned set."""
    rows = [(c[0], c[1], c[2] if len(c) >= 3 else "")
            for _, c in manifest_rows(text) if len(c) >= 2]
    every = "".join(f"{p}\t{c}\t{s}\n" for p, c, s in sorted(rows))
    refusal = [r for r in rows if r[1] in REFUSAL_CLASSES]
    ref = "".join(f"{p}\t{c}\t{s}\n" for p, c, s in sorted(refusal))
    return (hashlib.sha256(every.encode()).hexdigest(),
            hashlib.sha256(ref.encode()).hexdigest(), len(refusal))


def measure(paths):
    """Compile each fixture and parse its stderr with THE shared parser.

    -> {path: (exit_status, state_line)}. The compiles run in a scratch directory, since
    pdc writes `build_output/` relative to its working directory; the tree is never
    written. All captures go to ONE bash process that sources the parser, so the parser
    is the same function conformance.sh calls and it is loaded once, not 124 times.
    """
    if not PDC.is_file():
        raise CannotMeasure(f"{PDC} is not built. Run: cargo build --release")
    if not PARSER.is_file():
        raise CannotMeasure(f"the shared parser {PARSER} is missing")
    out = {}
    with tempfile.TemporaryDirectory() as d:
        work = Path(d)
        caps = []
        for n, path in enumerate(paths):
            cap = work / f"stderr_{n}"
            with open(cap, "wb") as err:
                res = subprocess.run([str(PDC), "compile", str(ROOT / path), "-o", f"pin_{n}"],
                                     cwd=work, stdout=subprocess.DEVNULL, stderr=err)
            caps.append(str(cap))
            out[path] = [res.returncode, None]
        script = ('. "$1"; shift\n'
                  'for f in "$@"; do pd_diag_parse "$f" || printf "UNREADABLE\\n"; done\n')
        res = subprocess.run(["bash", "-c", script, "gen-code-pins", str(PARSER), *caps],
                             capture_output=True, text=True)
        if res.returncode != 0:
            raise CannotMeasure(f"the shared parser exited {res.returncode}: {res.stderr[:200]}")
        states = res.stdout.split("\n")
        if states and states[-1] == "":
            states.pop()
        if len(states) != len(paths):
            raise CannotMeasure(f"the shared parser printed {len(states)} state(s) for "
                                f"{len(paths)} capture(s)")
        for path, state in zip(paths, states):
            if state == "UNREADABLE":
                raise CannotMeasure(f"the shared parser could not read the capture for {path}")
            out[path][1] = state
    return out


def decide(map_rows, man_text, measured):
    """-> (errors, {path: pin}). Pure over its inputs, so a planted map can be judged."""
    errors = []
    by_path = {}
    for r in map_rows:
        if r["row"] in by_path:
            errors.append(f"{r['row']}: appears in the map twice")
        by_path[r["row"]] = r

    # A reject row at stage `run` pins `exit=<N>`, not a code — a rule this unit leaves
    # exactly as it was. Such a row is outside this tool's domain in BOTH directions: it is
    # not required to be in the map, and its column 4 is never written. Zero exist today.
    declared = {c[0]: c[1] for _, c in manifest_rows(man_text)
                if len(c) >= 3 and c[1] in REFUSAL_CLASSES
                and not (c[1] == "reject" and c[2] == "run")}
    for p in sorted(set(declared) - set(by_path)):
        errors.append(f"{p}: a {declared[p]} row in the manifest that the map does not cover")
    for p in sorted(set(by_path) - set(declared)):
        errors.append(f"{p}: in the map, but not a reject/skip row of the manifest")
    for p in sorted(set(declared) & set(by_path)):
        if declared[p] != by_path[p]["class"]:
            errors.append(f"{p}: the manifest says class {declared[p]}, the map says "
                          f"{by_path[p]['class']}")

    siblings = defaultdict(list)
    for r in map_rows:
        siblings[r["code"]].append(r["row"])
    for code, rows in siblings.items():
        stated = {r["witnesses"] for r in map_rows if r["code"] == code}
        if stated != {str(len(rows))}:
            errors.append(f"{code}: the map states {sorted(stated)} witness(es), and lists "
                          f"{len(rows)}")

    payload = {}
    for r in map_rows:
        p = r["row"]
        if p not in measured:
            continue
        rc, state = measured[p]
        cols = state.split("\t")
        if rc != 1:
            errors.append(f"{p}: pdc exited {rc}, not 1 — the front end did not refuse it, so "
                          "there is no refusal to pin")
            continue
        if cols[0] != "CODED":
            errors.append(f"{p}: the shared parser says {state!r}, not one coded primary header")
            continue
        if cols[1] != r["code"]:
            errors.append(f"{p}: the compiler emits {cols[1]}, the map assigns {r['code']}")
            continue
        payload[p] = "\t".join(cols[2:])

    # PAYLOAD-EQUIVALENCE GROUPS, per code: the rows whose primary payloads are
    # character-identical. The comparator matches a bare `code=` on the code alone, so a
    # bare pin accepts EVERY refusal that code can print; it is only as selective as the
    # row needs when the code prints one payload across all its witnesses. Two groups
    # means a refusal from one group satisfies a bare pin on the other — the PD0006 shape
    # suF-a review round 1 found (a `total` refusal satisfied the `frobnicate` rows).
    groups = defaultdict(lambda: defaultdict(list))
    for r in map_rows:
        if r["row"] in payload:
            groups[r["code"]][payload[r["row"]]].append(r["row"])

    def group_rows(code, text):
        return "{" + ", ".join(sorted(groups[code][text])) + "}"

    def group_name(code, text):
        shown = text if len(text) <= 72 else text[:71] + "…"
        return f"{code} group {group_rows(code, text)} payload {shown!r}"

    pins = {}
    for r in map_rows:
        p, code, msg, frag = r["row"], r["code"], r["msg_tilde"], r["candidate_fragment"]
        if msg not in ("yes", "no"):
            errors.append(f"{p}: msg_tilde={msg!r}. Only `yes` and `no` are writable — a "
                          "PROVISIONAL fragment was verified against a header the compiler no "
                          "longer prints (R11)")
            continue
        want_disc = "yes" if msg == "yes" else "no"
        if r["discrimination_required"] != want_disc:
            errors.append(f"{p}: discrimination_required={r['discrimination_required']} but "
                          f"msg_tilde={msg}")
            continue
        if p not in payload:
            continue                      # its measurement error is already reported
        sibs = [s for s in siblings[code] if s != p]
        own = payload[p]
        twins = [s for s in groups[code][own] if s != p]
        others = [t for t in groups[code] if t != own]
        # IDENTICAL says "this row need not be told apart from its twins", so it is a claim
        # that a twin EXISTS, checked whatever the row's pin turns out to be.
        if r["witness_tag"] == "IDENTICAL" and not twins:
            errors.append(f"{p}: tagged IDENTICAL, but no same-code sibling prints its "
                          f"payload {own!r}")
            continue
        if msg == "yes":
            if frag in ("", "-"):
                errors.append(f"{p}: msg_tilde=yes with no fragment")
                continue
            if "\t" in frag or "\n" in frag or "\r" in frag:
                errors.append(f"{p}: the fragment holds a tab or a line break, which a TSV "
                              "cell cannot carry")
                continue
            # In EVERY payload of its own group — which is one payload, character for
            # character, so the own-payload test is the whole of it.
            if frag not in own:
                errors.append(f"{p}: fragment {frag!r} is not in its own primary payload "
                              f"{own!r}")
                continue
            # In NO payload of another group of the code. Twins are not "another group":
            # the fragment is in a twin's payload by construction, and must be.
            also = [group_name(code, t) for t in others if frag in t]
            if also:
                errors.append(f"{p}: fragment {frag!r} also selects another payload group of "
                              f"{code}: {'; '.join(also)} — an accepting pin")
                continue
            # Twins share ONE fragment, so the group is pinned by one statement, not by
            # several that happen to agree today.
            differ = sorted({by_path[s]["candidate_fragment"] for s in twins
                             if by_path[s]["msg_tilde"] == "yes"} - {frag})
            unpinned = sorted(s for s in twins if by_path[s]["msg_tilde"] != "yes")
            if differ or unpinned:
                errors.append(f"{p}: twins share one fragment, and "
                              f"{group_name(code, own)} does not: "
                              + (f"other fragment(s) {differ}" if differ else "")
                              + ("; " if differ and unpinned else "")
                              + (f"bare twin(s) {unpinned}" if unpinned else ""))
                continue
            pin = f"code={code};msg~{frag}"
        else:
            if frag != "-":
                errors.append(f"{p}: msg_tilde=no but the map carries fragment {frag!r}")
                continue
            # A bare pin is allowed only when the code prints ONE payload across all its
            # measured witnesses: a single witness, or twins and nothing else.
            if others:
                errors.append(f"{p}: a bare pin on {group_name(code, own)}, but {code} has "
                              f"{len(others) + 1} payload groups, and a refusal from any of "
                              f"the others would satisfy it: "
                              f"{'; '.join(group_rows(code, t) for t in others)}. Every row of "
                              "a code with two or more groups needs a `;msg~` that is in its "
                              "own group's payload and in no other group's")
                continue
            if sibs and r["witness_tag"] != "IDENTICAL":
                errors.append(f"{p}: {code} has {len(sibs) + 1} witnesses and this row has "
                              "no fragment and no IDENTICAL tag — an accepting pin")
                continue
            pin = f"code={code}"
        if not PIN_GRAMMAR.match(pin):
            errors.append(f"{p}: would write {pin!r}, which is not code=PD####[;msg~<fragment>]")
            continue
        pins[p] = pin
    return errors, pins


def rewrite(man_text: str, pins):
    lines = man_text.split("\n")
    for i, cols in manifest_rows(man_text):
        if cols[0] in pins and len(cols) >= 4:
            cr = lines[i].endswith("\r")
            cols = lines[i].rstrip("\r").split("\t")
            cols[3] = pins[cols[0]]
            lines[i] = "\t".join(cols) + ("\r" if cr else "")
    return "\n".join(lines)


def self_test() -> int:
    """Planted inputs through `decide()`, the function a real run uses. Nothing compiles:
    `measured` is written here, so each case is exactly the payloads it names."""
    attr_f, attr_t = "unknown attribute `frobnicate`", "unknown attribute `total`"

    def run(rows):
        """rows: [(code, path, tag, msg, fragment, payload)] -> decide()'s (errors, pins)."""
        n = Counter(r[0] for r in rows)
        map_rows = [dict(zip(MAP_COLUMNS, [c, "planted", str(n[c]), p, "reject", "no", tag,
                                           m, m, f, "-"])) for c, p, tag, m, f, _ in rows]
        man = "".join(f"{p}\treject\tcompile\tcode=PD0000\t-\tplanted\n"
                      for _, p, _, _, _, _ in rows)
        measured = {p: (1, f"CODED\t{c}\t{text}") for c, p, _, _, _, text in rows}
        return decide(map_rows, man, measured)

    a, b, c = "tests/reject/a.pd", "tests/reject/b.pd", "tests/reject/c.pd"
    fails = 0

    def case(name, rows, refused=(), message="", written=None):
        nonlocal fails
        errors, pins = run(rows)
        said = [e for e in errors if any(e.startswith(f"{p}: ") for p in refused)]
        ok = (all(any(e.startswith(f"{p}: ") for e in errors) for p in refused)
              and not any(p in pins for p in refused)
              and all(message in e for e in said)
              and (written is None or (pins == written and not errors)))
        print(f"  {'ok  ' if ok else 'FAIL'} {name}")
        if not ok:
            fails += 1
            print(f"       errors: {errors}\n       pins:   {pins}")

    # G1 — THE A1 SHAPE, as it stood in the map: three twins pinned bare, and a fourth
    # witness of the same code printing another payload.
    case("G1 a two-group code with a bare pin is REFUSED, naming the group",
         [("PD9001", a, "IDENTICAL", "no", "-", attr_f),
          ("PD9001", b, "IDENTICAL", "no", "-", attr_f),
          ("PD9001", c, "-", "yes", "`total`", attr_t)],
         refused=(a, b),
         message=f"a bare pin on PD9001 group {{{a}, {b}}}")
    case("G2 paired control: the same code with one shared fragment per group is written",
         [("PD9001", a, "IDENTICAL", "yes", "`frobnicate`", attr_f),
          ("PD9001", b, "IDENTICAL", "yes", "`frobnicate`", attr_f),
          ("PD9001", c, "-", "yes", "`total`", attr_t)],
         written={a: "code=PD9001;msg~`frobnicate`", b: "code=PD9001;msg~`frobnicate`",
                  c: "code=PD9001;msg~`total`"})
    case("G3 a fragment that also selects ANOTHER group is refused, naming that group",
         [("PD9001", a, "IDENTICAL", "yes", "unknown attribute", attr_f),
          ("PD9001", b, "IDENTICAL", "yes", "unknown attribute", attr_f),
          ("PD9001", c, "-", "yes", "`total`", attr_t)],
         refused=(a, b), message=f"also selects another payload group of PD9001: "
                                 f"PD9001 group {{{c}}}")
    case("G4 twins with two different fragments are refused (twins share ONE)",
         [("PD9001", a, "IDENTICAL", "yes", "`frobnicate`", attr_f),
          ("PD9001", b, "IDENTICAL", "yes", "frobnicate", attr_f),
          ("PD9001", c, "-", "yes", "`total`", attr_t)],
         refused=(a, b), message="twins share one fragment")
    case("G5 an IDENTICAL tag with no twin is refused",
         [("PD9002", a, "IDENTICAL", "yes", "`frobnicate`", attr_f),
          ("PD9002", c, "-", "yes", "`total`", attr_t)],
         refused=(a,), message="tagged IDENTICAL, but no same-code sibling prints")
    case("G6 a ONE-group code (twins and nothing else) pinned bare is written",
         [("PD9003", a, "IDENTICAL", "no", "-", attr_f),
          ("PD9003", b, "IDENTICAL", "no", "-", attr_f)],
         written={a: "code=PD9003", b: "code=PD9003"})
    print(f"gen-code-pins self-test: {6 - fails}/6 decided as required")
    return 1 if fails else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--map", default="docs/contributing/diagnostic-pin-map.tsv")
    ap.add_argument("--manifest", default="tests/conformance-manifest.txt")
    ap.add_argument("--self-test", action="store_true",
                    help="decide planted inputs, compile nothing, write nothing")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    map_path, man_path = rel_or_abs(a.map), rel_or_abs(a.manifest)
    try:
        map_rows = load_map(map_path)
        try:
            # newline="" so a `\r\n` row, if one ever exists, is carried as written
            # rather than normalised: the run may change column 4 and nothing else.
            with open(man_path, encoding="utf-8", newline="") as fh:
                before = fh.read()
        except OSError as exc:
            raise CannotMeasure(f"cannot read the manifest {man_path}: {exc}") from exc
        measured = measure(sorted({r["row"] for r in map_rows}))
    except CannotMeasure as exc:
        print(f"gen-code-pins: could not measure — {exc}", file=sys.stderr)
        return 2

    errors, pins = decide(map_rows, before, measured)
    states = Counter("msg~" if ";msg~" in v else "bare" for v in pins.values())
    print(f"map rows {len(map_rows)}  conditions {len({r['code'] for r in map_rows})}  "
          f"measured {len(measured)}  pins {len(pins)} "
          f"(with msg~ {states['msg~']}, bare {states['bare']})")
    if errors:
        print(f"REFUSED — {len(errors)} row(s) are not true of this compiler; the manifest "
              "was NOT written:")
        for e in errors:
            print(f"  {e}")
        return 1

    after = rewrite(before, pins)
    pre, post = membership_digests(before), membership_digests(after)
    changed = sum(1 for x, y in zip(before.split("\n"), after.split("\n")) if x != y)
    print(f"membership before: all-rows {pre[0]}  refusal-rows {pre[1]} (n={pre[2]})")
    print(f"membership after:  all-rows {post[0]}  refusal-rows {post[1]} (n={post[2]})")
    if pre != post:
        print("could not measure: the rewrite changed membership, which it never may",
              file=sys.stderr)
        return 2
    if after == before:
        print(f"unchanged — every pin was already current "
              f"(manifest sha256 {hashlib.sha256(before.encode()).hexdigest()})")
        return 0
    with open(man_path, "w", encoding="utf-8", newline="") as fh:
        fh.write(after)
    print(f"wrote {changed} row(s) of {man_path}  manifest sha256 "
          f"{hashlib.sha256(before.encode()).hexdigest()} -> "
          f"{hashlib.sha256(after.encode()).hexdigest()}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
