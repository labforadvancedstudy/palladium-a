#!/usr/bin/env bash
# THE DEFINITION OF PALLADIUM 1.0, AS A COMMAND.
#
# 1.0 is not "the inventory has no unmet rows". That is a completeness criterion,
# and completeness criteria are the generator of every fiction this repository has
# had to retract: `progress: 85%`, "Bootstrap 100% Complete", "Self-Hosting 100%",
# "v0.6: Self-hosting achieved". Draw the line on an inventory and the disease
# returns under a new name.
#
# 1.0 is the thesis, proven on the one artifact here that structurally cannot lie:
#
#     bootstrap/pdc.pd, rewritten in the differentiated dialect, still reaching a
#     byte-identical stage1/stage2 fixed point.
#
# A vacuous conformance fixture can print "not yet implemented" and PASS — seven of
# them did, for a year, and defect D5 survived behind one. A compiler cannot compile
# itself vacuously. Scope follows from what the compiler actually uses, not from
# argument about what belongs in a release.
#
# THIS GATE IS EXPECTED TO BE RED UNTIL M9. It is committed red on purpose: the
# definition of 1.0 has to live in the repository as a command, because prose drifts
# and commands do not. Every RED line below names the milestone that owes it.
#
# The four conditions (docs/contributing/MILESTONES.md, and the `thesis` rows of
# docs/contributing/1.0-requirements.tsv):
#
#   1. make selfhost is green, AND bootstrap/pdc.pd is written in the dialect:
#      no `async`, no `await`, no lifetime parameter list; at least one `ref`/
#      `ref mut` parameter; at least one `#[total]` the compiler DISCHARGES; and a
#      file-IO function whose inferred effect reaches its caller.
#   2. one NON-VACUOUS conformance fixture with a transcript, per differentiator.
#   3. per differentiator, a REJECT TWIN. This half is load-bearing and must never
#      be dropped: for an inference feature the rejection IS the product. A region
#      inferencer that accepts everything is a no-op, and a no-op is indistinguish-
#      able from a working one if you only look at green fixtures.
#   4. a SECOND witness program, so that one program's accidental shape does not
#      become the definition of the language.
#
# Usage:  scripts/thesis-exit.sh          # exit 0 only when 1.0 is real
#         THESIS_VERBOSE=1 scripts/…      # show every probe

set -uo pipefail
cd "$(dirname "$0")/.."

BOOT=bootstrap/pdc.pd
MANIFEST=docs/contributing/1.0-requirements.tsv
CONF=tests/conformance-manifest.txt
WITNESS2=tests/witness/json_parser.pd

red=0
green=0
say()  { printf '  %s\n' "$*"; }
ok()   { green=$((green+1)); printf '  \033[0;32mok  \033[0m %s\n' "$*"; }
bad()  { red=$((red+1));     printf '  \033[0;31mRED \033[0m %-58s %s\n' "$1" "owed by ${2:-?}"; }

# ---------------------------------------------------------------------------
# A lexer, not a grep. `'` is legal in Palladium in two places the naive scan
# gets wrong, and both would make this gate lie:
#   - a char literal ('a', and in a compiler's own source, '<'), which a raw
#     `grep "'"` flags as a lifetime;
#   - `ref<'a> T`, which N9 EXPLICITLY PERMITS where inference cannot resolve.
#     A gate that forbade every `'` would reject a conforming program and push
#     the implementation toward a narrower language than the normative text.
# So: strip comments and string/char literals, then look for a lifetime in
# PARAMETER-LIST position only, which is `<'` not preceded by `ref`.
# ---------------------------------------------------------------------------
strip_literals() {
  python3 - "$1" <<'PY'
import re, sys
src = open(sys.argv[1], encoding='utf-8', errors='replace').read()

# `'` is ambiguous: it opens a char literal, and it also introduces a lifetime.
# Treating every `'` as a quote is not a small inaccuracy — it consumes from the
# tick to end of file, and TH-02 can then NEVER fire. A gate that cannot go red
# on the thing it checks is the exact defect this repository keeps retracting, so
# the disambiguation is explicit: a char literal is `'x'` or `'\x'` and nothing
# else. Anything else beginning with `'` is a lifetime tick and is KEPT.
CHAR = re.compile(r"'(?:\\.|[^\\'])'")

out, i, n = [], 0, len(src)
while i < n:
    c = src[i]
    if c == '"':
        i += 1
        while i < n and src[i] != '"':
            i += 2 if src[i] == '\\' else 1
        i += 1
        out.append(' ')
    elif c == "'":
        m = CHAR.match(src, i)
        if m:
            i = m.end(); out.append(' ')     # a char literal: erased
        else:
            out.append(c); i += 1            # a lifetime tick: kept
    elif src.startswith('//', i):
        while i < n and src[i] != '\n': i += 1
    elif src.startswith('/*', i):
        depth, i = 1, i + 2
        while i < n and depth:
            if src.startswith('/*', i): depth, i = depth + 1, i + 2
            elif src.startswith('*/', i): depth, i = depth - 1, i + 2
            else: i += 1
    else:
        out.append(c); i += 1
print(''.join(out))
PY
}

has_lifetime_param_list() {  # <stripped source> -> 0 if a `'a` LIST is present
  # N9 permits a region name in exactly one place, `ref<…>`. Blank those, and any
  # surviving `<'` is a parameter list of the kind N9 removes.
  printf '%s' "$1" | sed "s/ref<'[A-Za-z_0-9]*>/ref/g" | grep -qE "<'"
}

has_async_token() { printf '%s' "$1" | grep -qE '(^|[^A-Za-z_])(async|await)([^A-Za-z_0-9]|$)'; }

# --self-test: prove the two source probes can still go RED. The repository's own
# rule — a gate whose ability to fail is untested is not a gate (see
# `make test-conformance-runner`, `make test-gate-probe`). The first version of
# the stripper here treated every `'` as a quote, consumed from the tick to end of
# file, and TH-02 could never fire; these four cases are why that was caught.
if [ "${1:-}" = "--self-test" ]; then
  T=$(mktemp -d); fails=0
  probe() {  # <name> <source> <want-lifetime 0|1> <want-async 0|1>
    printf '%s\n' "$2" > "$T/p.pd"; s=$(strip_literals "$T/p.pd")
    has_lifetime_param_list "$s" && gl=1 || gl=0
    has_async_token "$s"          && ga=1 || ga=0
    if [ "$gl" = "$3" ] && [ "$ga" = "$4" ]; then
      printf '  \033[0;32mok  \033[0m %s\n' "$1"
    else
      printf '  \033[0;31mFAIL\033[0m %s (lifetime %s want %s, async %s want %s)\n' "$1" "$gl" "$3" "$ga" "$4"
      fails=$((fails+1))
    fi
  }
  echo "thesis-exit self-test — can the source probes still go RED?"
  probe "a char literal '<' is not a lifetime"        "fn f() { let x = '<'; }"              0 0
  probe "ref<'a> T is PERMITTED by N9"                "fn f(x: ref<'a> String) -> i64 { }"   0 0
  probe "fn f<'a>(…) is a lifetime parameter list"    "fn f<'a>(x: ref String) -> i64 { }"   1 0
  probe "async/await in a comment is not a token"     "fn f() { /* async */ } // await"      0 0
  probe "a real async fn IS a token"                  "async fn f() { }"                     0 1
  probe "an .await IS a token"                        "fn f() { let x = g().await; }"        0 1
  rm -rf "$T"
  [ "$fails" -eq 0 ] && { echo "  self-test green"; exit 0; } || { echo "  self-test RED"; exit 1; }
fi

# A conformance row must exist with the declared class, and the corpus run must
# be green. Class `run` additionally requires a sibling .expected transcript —
# there is no exit-code-only spelling, because a tail-return miscompile prints
# garbage and still exits 0.
row_is() {  # row_is <path> <class>
  awk -F'\t' -v p="$1" -v c="$2" '$1==p && $2==c {found=1} END {exit !found}' "$CONF"
}

echo "=============================================================="
echo "  make thesis-exit — the definition of Palladium 1.0"
echo "=============================================================="
echo
echo "Condition 1 — the self-hosting compiler is written in the dialect"

if [ ! -f "$BOOT" ]; then
  bad "$BOOT exists" M9
else
  SRC=$(strip_literals "$BOOT")

  # TH-01  no async / await token
  if has_async_token "$SRC"; then
    bad "TH-01 no \`async\` / \`await\` token in $BOOT" M5
  else
    ok "TH-01 no \`async\` / \`await\` token in $BOOT"
  fi

  # TH-02  no lifetime PARAMETER LIST — `ref<'a> T` stays legal, see N9.
  if has_lifetime_param_list "$SRC"; then
    bad "TH-02 no lifetime parameter list in $BOOT" M7
  else
    ok "TH-02 no lifetime parameter list in $BOOT"
  fi

  # TH-03  at least one ref / ref mut parameter
  if printf '%s' "$SRC" | grep -qE ':[[:space:]]*ref([[:space:]]+mut)?[[:space:]]'; then
    ok "TH-03 at least one \`ref\`/\`ref mut\` parameter"
  else
    bad "TH-03 at least one \`ref\`/\`ref mut\` parameter" M7
  fi

  # TH-04  at least one #[total] — and it must be DISCHARGED, which is what
  # compiling the file with the checker on proves, since an unproven obligation
  # is a compile error with no downgrade (requirement N8-12).
  if printf '%s' "$SRC" | grep -qE '#\[total'; then
    if ./target/release/pdc compile "$BOOT" -o /dev/null >/dev/null 2>&1; then
      ok "TH-04 a #[total] the compiler discharges"
    else
      bad "TH-04 a #[total] the compiler discharges (present, not discharged)" M6
    fi
  else
    bad "TH-04 a #[total] the compiler discharges" M6
  fi

  # TH-05  a file-IO function's inferred effect reaches its caller.
  # The driver already reports per-function effects; what is missing is that the
  # report is derived from a FIXED POINT and gates anything (requirements
  # N7-05, N7-08).
  if ./target/release/pdc compile "$BOOT" -o /dev/null 2>&1 \
       | grep -qE "has effects.*(IO|Io|io)"; then
    ok "TH-05 a file-IO effect propagates to its caller"
  else
    bad "TH-05 a file-IO effect propagates to its caller" M5
  fi
fi

if make -s selfhost >/dev/null 2>&1; then
  ok "SH-01 make selfhost — stage1 and stage2 C are byte-identical"
else
  bad "SH-01 make selfhost" M9
fi
if grep -qE '^selfhost-corpus:' Makefile 2>/dev/null && make -s selfhost-corpus >/dev/null 2>&1; then
  ok "SH-02..04 the bootstrap compiler compiles the corpus, refusals included"
else
  bad "SH-02..04 make selfhost-corpus (bootstrap compiles the LANGUAGE)" M9
fi

echo
echo "Condition 2 — one non-vacuous fixture per differentiator"
for pair in \
  "tests/09_effects_system.pd|effects are inferred and propagate|M5" \
  "tests/13_total_attribute.pd|#[total] is discharged|M6" \
  "tests/05_ref_shared.pd|ref / ref mut with inferred regions|M7"
do
  IFS='|' read -r path what owner <<<"$pair"
  if row_is "$path" run; then ok "C2 $what — $path"; else bad "C2 $what — $path" "$owner"; fi
done

echo
echo "Condition 3 — the reject twin per differentiator (the rejection IS the product)"
for pair in \
  "tests/reject/pure_function_calls_io.pd|an ungated effect escape is refused|M5" \
  "tests/reject/total_unproven.pd|a #[total] whose proof fails is refused|M6" \
  "tests/reject/ambiguous_region.pd|an ambiguous region is refused, BY NAME|M7"
do
  IFS='|' read -r path what owner <<<"$pair"
  if row_is "$path" reject; then ok "C3 $what — $path"; else bad "C3 $what — $path" "$owner"; fi
done

echo
echo "Condition 4 — a second witness, so one program's shape is not the language"
if row_is "$WITNESS2" run; then
  ok "TH-06/WT-02 $WITNESS2 is in the corpus"
  W=$(strip_literals "$WITNESS2" 2>/dev/null || echo '')
  printf '%s' "$W" | grep -qE ':[[:space:]]*ref([[:space:]]+mut)?[[:space:]]' \
    && ok "TH-06 witness 2 uses \`ref\`" || bad "TH-06 witness 2 uses \`ref\`" M9
  printf '%s' "$W" | grep -qE '#\[total' \
    && ok "TH-06 witness 2 carries a #[total]" || bad "TH-06 witness 2 carries a #[total]" M9
  printf '%s' "$W" | grep -qE '(^|[^A-Za-z_])(async|await)([^A-Za-z_0-9]|$)' \
    && bad "TH-06 witness 2 is free of async/await" M9 || ok "TH-06 witness 2 is free of async/await"
else
  bad "TH-06/WT-02 $WITNESS2 is a \`run\` row in the corpus" M2
fi

echo
echo "=============================================================="
printf '  thesis: %d green, \033[0;31m%d RED\033[0m\n' "$green" "$red"
if [ "$red" -eq 0 ]; then
  echo "  Palladium 1.0: the thesis holds."
  echo "=============================================================="
  exit 0
fi
cat <<'EOF'
  1.0 is NOT reached. Every RED line names the milestone that owes it.
  This gate is committed red on purpose — see the header of this file and
  docs/contributing/MILESTONES.md. Do not make it pass by weakening it:
  dropping condition 3 would let a no-op inferencer look finished.
EOF
echo "=============================================================="
exit 1
