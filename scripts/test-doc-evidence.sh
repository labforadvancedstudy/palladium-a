#!/usr/bin/env bash
# Regression tests for the `cmd:` and `conformance:` evidence checks.
#
# WHY THIS EXISTS
# `cmd:` evidence was a SHAPE check for as long as it existed: the string had to look
# like `cmd: X -> Y`, and X was never run. It passed every day, over nine false items,
# because passing cost nothing. A gate is worth its exit code and nothing else, so the
# only way to know this one has one is to hand it a lie and require it to say so.
#
# HOW A NEGATIVE CONTROL FAILS VACUOUSLY, AND WHAT IS DONE ABOUT IT
# This repository has already caught a control that counted its own breakage as a
# detection. Three rules here, all mechanical:
#
#   1. THE GREEN CONTROL RUNS FIRST AND IS FATAL. Case 1 is a TRUE index that must pass.
#      If it does not, the harness itself is broken and every later "detected!" would be
#      the harness failing, not the gate working. The script aborts rather than counting
#      those as passes.
#   2. EVERY RED CASE ASSERTS A SPECIFIC MESSAGE, not merely a non-zero exit. Red for the
#      wrong reason is a failure here.
#   3. EVERY INDEX HOLDS EXACTLY ONE ROW, named uniquely, and the assertion names it. A
#      failure that leaked in from another row cannot satisfy it.
#
# Usage: bash scripts/test-doc-evidence.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

TMP=$(mktemp -d) || exit 2
# A scratch directory INSIDE the repository. Two cases below are about what a path inside
# the repository may resolve to, and a symlink under /tmp cannot test that. Removed on
# every exit path, including the failing ones.
INREPO=.test-doc-evidence-tmp
rm -rf "$INREPO"
trap 'rm -rf "$TMP" "$INREPO"' EXIT
mkdir -p "$INREPO/empty" || exit 2
ln -sfn /etc "$INREPO/escape" || exit 2
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; NC=$'\033[0m'
pass=0; fail=0

# index <name> <implementation> <evidence-item>...  -> writes $TMP/<name>.toml
index() {
  local name=$1 impl=$2; shift 2
  {
    printf '[probe.%s]\n' "$name"
    printf 'description = "throwaway row for scripts/test-doc-evidence.sh"\n'
    printf 'spec = "../../specification/language-spec.md"\n'
    printf 'implementation = "%s"\n' "$impl"
    printf 'evidence = [\n'
    local e
    for e in "$@"; do printf '  "%s",\n' "$e"; done
    printf ']\n'
  } > "$TMP/$name.toml"
}

# run <name> -> sets RC and OUT
run() {
  OUT=$(python3 scripts/check_doc_evidence.py --index-only --index "$TMP/$1.toml" 2>&1)
  RC=$?
}

# expect_green <name> <case>
expect_green() {
  run "$1"
  if [ "$RC" -eq 0 ]; then
    printf '  %sok%s   %s\n' "$GREEN" "$NC" "$2"; pass=$((pass+1))
  else
    printf '  %sFAIL%s %s\n' "$RED" "$NC" "$2"
    printf '         (expected exit 0, got %s)\n' "$RC"
    printf '%s\n' "$OUT" | sed 's/^/         | /' | head -12
    fail=$((fail+1))
  fi
}

# expect_red <name> <case> <substring the message MUST contain>...
expect_red() {
  local name=$1 case=$2; shift 2
  run "$name"
  if [ "$RC" -eq 0 ]; then
    printf '  %sFAIL%s %s\n' "$RED" "$NC" "$case"
    printf '         (gate stayed GREEN on a deliberately false item)\n'
    fail=$((fail+1)); return
  fi
  # Red for the RIGHT reason, and about THIS row.
  local want missing=0
  if ! printf '%s\n' "$OUT" | grep -qF -- "probe.$name"; then
    printf '  %sFAIL%s %s\n' "$RED" "$NC" "$case"
    printf '         (went red without naming the row probe.%s)\n' "$name"
    printf '%s\n' "$OUT" | sed 's/^/         | /' | head -12
    fail=$((fail+1)); return
  fi
  for want in "$@"; do
    if ! printf '%s\n' "$OUT" | grep -qF -- "$want"; then
      printf '  %sFAIL%s %s\n' "$RED" "$NC" "$case"
      printf '         (message lacks: %s)\n' "$want"
      missing=1
    fi
  done
  if [ "$missing" -eq 1 ]; then
    printf '%s\n' "$OUT" | sed 's/^/         | /' | head -14
    fail=$((fail+1)); return
  fi
  printf '  %sok%s   %s\n' "$GREEN" "$NC" "$case"; pass=$((pass+1))
}

echo "== the harness must be able to say YES before any NO is worth anything =="

# CASE 1 (FATAL IF RED). True items covering every shape the corpus uses: a positive
# count, an ABSENCE PROOF, a real two-segment PIPELINE, a find with an expression, and a
# conformance class. The absence proof is here on purpose -- `exit 1, 0 lines` is the
# success case for most of this corpus, and a checker written around `check_returncode()`
# would invert exactly these, passing the liars and failing the honest rows. The pipeline
# is here because there was no green pipeline control at all in the first version, so
# nothing established that the new per-segment status checks pass a working pipeline.
#
# THE SCOPE IS runtime/ AND NOT scripts/, AND THAT MATTERS. It used to count scripts/, and
# the count is taken when this index is built -- before the gate runs. The gate's first
# Python import writes scripts/__pycache__, so on a FRESH CHECKOUT the directory gained an
# entry between the claim and the measurement and this control failed. It is the fatal
# green control, so the whole probe aborted; and it failed only on the first run in a new
# tree, which is to say only on the certifying path. Measured in a shallow clone:
# `ls scripts/` was 31 before the first import and 32 after. runtime/ holds two checked-in
# files and nothing generates into it.
index truth implemented \
  "cmd: ls runtime/ -> exit 0, $(ls runtime/ | wc -l | tr -d ' ') lines" \
  "cmd: grep -rn zzz_no_such_identifier_anywhere src/ --include='*.rs' -> exit 1, 0 lines -- the shape most of this corpus uses" \
  "cmd: grep -rn 'pub fn' src/lexer/ --include='*.rs' | grep -v zzz_nothing -> exit 0, $(/usr/bin/grep -rn 'pub fn' src/lexer/ --include='*.rs' | /usr/bin/grep -v zzz_nothing | wc -l | tr -d ' ') lines" \
  "cmd: find src -name '*.zzz' -> exit 0, 0 lines -- find's expression is an expression, not more paths" \
  "conformance: tests/reject/try_block.pd reject"
expect_green truth "a TRUE index passes (count, absence, PIPELINE, find expr, conformance)"
if [ "$fail" -ne 0 ]; then
  echo
  echo "${RED}ABORT${NC}: the green control failed, so this harness cannot distinguish"
  echo "       'the gate detected the lie' from 'the harness is broken'. Every case"
  echo "       below would be meaningless. Fix the control first."
  exit 1
fi

echo
echo "== a false result must fail, loudly, with claimed AND actual =="

# CASE 2. The defect that started this: a claim of one line over a command producing many.
# This is the shape of the real item `grep -rn 'effects::' ... -> 1 line`, which produced 8.
index wrong_count unimplemented \
  "cmd: grep -rn 'pub fn' src/lexer/ --include='*.rs' -> exit 0, 1 lines -- deliberately false"
expect_red wrong_count "a false LINE COUNT is rejected, showing claimed and actual" \
  "claimed: exit 0, 1 line(s)" "actual:" "output:"

# CASE 3. Count right, exit status wrong. Checking only one of the two numbers would let
# this through, and the exit status is the half that carries an absence.
index wrong_exit unimplemented \
  "cmd: grep -rn zzz_no_such_identifier_anywhere src/ --include='*.rs' -> exit 0, 0 lines"
expect_red wrong_exit "a false EXIT STATUS is rejected even when the count matches" \
  "claimed: exit 0, 0 line(s)" "actual:  exit 1, 0 line(s)"

# CASE 4. The absence proof must not be invertible. A command that CANNOT LOOK exits 2 and
# prints to stderr; read as a boolean that is "no match", which is how a probe for the LLVM
# backend's CLI flag sat recorded as `exit 1, 0 lines` having never searched anything.
index could_not_look unimplemented \
  "cmd: grep -rn 'x' src/no_such_directory_here/ -> exit 1, 0 lines"
expect_red could_not_look "a command that COULD NOT LOOK is not a proof of absence" \
  "does not exist"

# CASE 4b. THE SAME COMMAND WITH --include, WHICH IS THE DANGEROUS ONE. Measured on BSD
# grep 2.6.0, a missing directory plus --include exits 1, prints nothing, and writes no
# stderr: byte for byte a true absence proof. Twenty-odd real items have this exact shape,
# so a directory rename would leave all of them green over nothing. Found by this harness,
# not by review -- the first version of the executor passed this case.
index missing_dir_include unimplemented \
  "cmd: grep -rn 'x' src/no_such_directory_here/ --include='*.rs' -> exit 1, 0 lines"
expect_red missing_dir_include "an absence over a MISSING PATH is rejected (--include hides it)" \
  "does not exist" "not an absence"

# CASE 4c. And the same shape over a path that DOES exist must still pass, so 4b is a
# check on the path and not a blanket refusal of the corpus's dominant form.
index missing_dir_ok unimplemented \
  "cmd: grep -rn zzz_no_such_identifier_anywhere src/lexer/ --include='*.rs' -> exit 1, 0 lines"
expect_green missing_dir_ok "the same shape over an EXISTING path still passes"

# CASE 4d. Inversion was stripped from the L3 probe only as a whole token, so a legitimate
# item written with a short cluster kept `-v` in the probe, matched nothing, and was
# REJECTED. Fail-closed rather than a false green, but it made an honest observation
# unwritable -- which is its own kind of pressure to write a worse one.
index clustered_invert unimplemented \
  "cmd: grep -rvn zzz_no_such_identifier_anywhere src/lexer/ --include='*.rs' -> exit 0, $(/usr/bin/grep -rvn zzz_no_such_identifier_anywhere src/lexer/ --include='*.rs' | wc -l | tr -d ' ') lines"
expect_green clustered_invert "a clustered -rvn item is not falsely rejected by its own probe"

# CASE 5. Prose in the result position. This is the pre-2026-08-22 spelling: it is exactly
# what the old shape check accepted, so it must now be named as unrunnable.
index prose_result unimplemented \
  "cmd: grep -rn 'Region' src/ --include='*.rs' -> 1 line, src/driver/mod.rs:147"
expect_red prose_result "a prose result is rejected: there is nothing to compare" \
  "must be" "exit <N>, <M> lines"

# CASE 6. A quoted claim the output does not contain. Same rule, and same reason, as the
# `src:` check: a citation whose excerpt lacks the thing being claimed.
index bad_quote unimplemented \
  "cmd: ls runtime/ -> exit 0, $(ls runtime/ | wc -l | tr -d ' ') lines -- including \`no_such_file_at_all.py\`"
expect_red bad_quote "a quote absent from the output is rejected" \
  "but the command's output does not contain it"

echo
echo "== a cmd: may not become a second engine for a question a gate already owns =="

# CASE 7-9. The migration is enforced, not merely performed. Without these, the sixteen
# `pdc compile '<program>'` items could walk straight back in.
index inline_program unimplemented \
  "cmd: pdc compile 'fn main() { try { } }' -> exit 1, 1 lines"
expect_red inline_program "an inline pdc program is refused and sent to conformance" \
  "not an observation" "conformance:"

index builds unimplemented \
  "cmd: cargo build --release -> exit 0, 0 lines"
expect_red builds "a build is refused and sent to gate:" "gate: cargo"

index artifact unimplemented \
  "cmd: grep -c '#line' build_output/01_lexical_comments.c -> exit 1, 0 lines"
expect_red artifact "a build artifact is refused: not reproducible from a checkout" \
  "build artifact"

# CASE 10. No shell. A `cmd:` is argv, and an operator is not a hermetic observation.
index operator unimplemented \
  "cmd: grep -rn 'x' src/ ; ls -> exit 0, 0 lines"
expect_red operator "a shell operator is refused" "shell operator"

# CASE 11. Not on the allowlist.
index not_allowed unimplemented \
  "cmd: python3 -c pass -> exit 0, 0 lines"
expect_red not_allowed "a command outside the allowlist is refused" "allowlist"

echo
echo "== an absence proof must be shown CAPABLE of producing output =="
#
# Review found two more doors into the room Case 4b opens, and confirming them found two
# more again. Every one produces `exit 1, 0 lines` -- a perfect absence proof -- from a
# command that read nothing. They are listed as separate cases because each defeats a
# DIFFERENT part of the parse, and a single fix that misses one is worth knowing about.

# CASE 14. No path operand at all. grep falls back to stdin, is handed the gate's empty
# stdin, and reports the canonical absence over nothing. This is the plainest form of the
# whole class and the first version accepted it.
index no_path unimplemented \
  "cmd: grep -rn zzz_no_such_identifier_anywhere -> exit 1, 0 lines"
expect_red no_path "a command naming NO path is refused" "names no path to read"

# CASE 15. The pattern arrives through an option, so the one real path is what a
# "drop the first operand, it is the pattern" rule throws away. Zero paths were checked.
index pattern_opt unimplemented \
  "cmd: grep -r --regexp=zzz_no_such_identifier_anywhere src/ -> exit 1, 0 lines"
expect_red pattern_opt "an option-supplied pattern (--regexp=) is refused" \
  "supplies its pattern through"

# CASE 16. The same, clustered into a short-option bundle. A check that looked at the
# whole token saw `-rne`, matched nothing in its table, and let the `e` through.
index pattern_short unimplemented \
  "cmd: grep -rne zzz_no_such_identifier_anywhere src/ -> exit 1, 0 lines"
expect_red pattern_short "a clustered -e is refused" "clusters -e"

# CASE 17. An option whose argument is separate would be counted as a path.
index sep_arg unimplemented \
  "cmd: grep -rn --include '*.rs' zzz_nothing src/ -> exit 1, 0 lines"
expect_red sep_arg "an option taking a separate argument is refused" "separate argument"

# CASE 18. THE MEASUREMENT, not the parse. Every case above is a door; this is the room.
# The scope exists, is named, and is inside the repo -- and holds nothing, so the absence
# is over an empty stream. No argv inspection can see this; only running it can.
index empty_scope unimplemented \
  "cmd: grep -rn zzz_no_such_identifier_anywhere $INREPO/empty/ -> exit 1, 0 lines"
expect_red empty_scope "an absence over an EMPTY scope is refused (measured, not parsed)" \
  "produces NOTHING even when asked to match everything"

# CASE 19. And the same for a live directory filtered down to nothing by --include, which
# is a scope census's blind spot: the path exists and has files, just none it will open.
index filtered_to_nothing unimplemented \
  "cmd: grep -rn zzz_nothing src/ --include='*.no_such_extension' -> exit 1, 0 lines"
expect_red filtered_to_nothing "an absence filtered to an empty file set is refused" \
  "produces NOTHING even when asked to match everything"

# CASE 20. A downstream segment naming a file reads the file, not the pipe, so the
# pipeline's number is not what it appears to be.
index downstream_path unimplemented \
  "cmd: grep -rn 'pub fn' src/lexer/ --include='*.rs' | grep -v zzz Cargo.toml -> exit 0, 1 lines"
expect_red downstream_path "a downstream segment naming a path is refused" \
  "downstream of a pipe but also names the path"

# CASE 20b. find's L3 probe used to delete the whole expression and run `find <paths>`,
# which prints the directory itself. So `find empty-dir -type f` returned nothing while
# its probe printed `empty-dir`, and the empty scope passed: L3 was a proof for grep and
# NOT for find, under a claim that said "if that finds nothing, the command reads
# nothing". The probe now keeps the traversal and neutralises only the matching predicate.
index find_empty_scope unimplemented \
  "cmd: find $INREPO/empty -type f -> exit 0, 0 lines"
expect_red find_empty_scope "an absence over an empty scope is refused for FIND too" \
  "produces NOTHING even when asked to match everything"

echo
echo "== a find expression is forwarded to a real process, so it is enumerated =="

# CASES 21a-c. The expression was passed through unchecked, so a declared hermetic
# OBSERVATION could execute a program of the document's choosing or alter the checkout.
# Neither is caught by the tool checks, because the program being run really is find.
index find_exec unimplemented \
  "cmd: find src -name '*.rs' -exec /bin/sh -c id {} + -> exit 0, 0 lines"
expect_red find_exec "find -exec is refused: it runs a program of the document's choosing" \
  "-exec" "observation set"

index find_delete unimplemented \
  "cmd: find $INREPO/empty -delete -> exit 0, 0 lines"
expect_red find_delete "find -delete is refused: an observation may not alter the tree" \
  "-delete"

index find_negation unimplemented \
  "cmd: find src -not -name zzz -> exit 0, 0 lines"
expect_red find_negation "find negation is refused: it would invert the L3 probe" "-not"

# THE COMBINATIONS, not just the simple shapes. The previous round's find controls covered
# one name predicate, an empty `-type f`, and forbidden predicates -- and a claim that the
# grammar was total. It was not: `-o` between a traversal and a matching predicate reads
# as (type f) OR (name), so one vacuous branch removes the -type bound and the probe
# measures a scope the command never searched.
index find_type_or unimplemented \
  "cmd: find $INREPO/empty -type f -o -name '*.zzz' -> exit 0, 0 lines"
expect_red find_type_or "a find disjunction is refused outright, not reduced" \
  "-o" "one \`cmd:\` item per pattern"

index find_match_then_type unimplemented \
  "cmd: find src -name '*.zzz' -type f -> exit 0, 0 lines"
expect_red find_match_then_type "a traversal predicate AFTER a match is refused" \
  "must come first"

index find_bad_type unimplemented \
  "cmd: find src -type ff -name zzz -> exit 0, 0 lines"
expect_red find_bad_type "an invalid multi-letter -type argument is refused" "-type"

index find_dangling_o unimplemented \
  "cmd: find src -name zzz -o -type f -> exit 0, 0 lines"
expect_red find_dangling_o "a disjunction mixing match and traversal is refused" "-o"

# THE THIRD DEFECT IN THIS CONSTRUCT IN THREE ROUNDS, and why the grammar shrank instead
# of getting cleverer. `-a` binds tighter than `-o`, so this reads as name(x) OR (name(y)
# AND print): measured, the command printed 0 lines and its probe printed 3, so L3 passed
# an absence that had read nothing relevant. Rounds one and two were the same construct.
index find_action_under_o unimplemented \
  "cmd: find src -name '*.x' -o -name '*.y' -print -> exit 0, 0 lines"
expect_red find_action_under_o "an action under a disjunction is refused (it bound to one branch)" \
  "not in the observation set"

index find_two_matches unimplemented \
  "cmd: find src -name a -name b -> exit 0, 0 lines"
expect_red find_two_matches "two matching predicates are refused: one item per pattern" \
  "at most one"

index find_action unimplemented \
  "cmd: find src -name '*.zzz' -print -> exit 0, 0 lines"
expect_red find_action "an action is refused; the default action already prints" "-print"

# And the honest shapes must still be writable, or the grammar is just a wall.
index find_plain unimplemented \
  "cmd: find src stdlib -name '*.zzz' -> exit 0, 0 lines"
expect_green find_plain "one matching predicate over a real scope passes"

index find_maxdepth unimplemented \
  "cmd: find src -maxdepth 0 -name '*.zzz' -> exit 0, 0 lines"
expect_green find_maxdepth "-maxdepth 0 is preserved by the probe, not neutralised"

index find_type_and_name unimplemented \
  "cmd: find src -type f -name '*.zzz' -> exit 0, 0 lines"
expect_green find_type_and_name "traversal AND match (no -o) keeps the -type bound"

index find_regular_file unimplemented \
  "cmd: find Cargo.toml -name '*.zzz' -> exit 0, 0 lines"
expect_green find_regular_file "a regular file as the starting path is a valid scope"

echo
echo "== 'no shell' must be a property of the process, not a claim in a comment =="

# CASE 21. The allowlist compared basename(argv[0]), so any executable named grep was
# accepted -- including one checked into this repository, whose interpreter can be a shell.
index exe_by_path unimplemented \
  "cmd: scripts/grep -rn zzz src/ -> exit 1, 0 lines"
expect_red exe_by_path "an executable named by PATH is refused" "names the executable by path"

# CASE 22. Operand paths were checked lexically and with exists(), neither of which
# follows a link. A symlink committed inside the repo can point anywhere, and the gate
# would be measuring unversioned content while reporting on this tree.
index symlink_escape unimplemented \
  "cmd: grep -rn zzz_nothing $INREPO/escape/ -> exit 1, 0 lines"
expect_red symlink_escape "a symlink resolving outside the repository is refused" \
  "outside"

# CASE 23. PATH was inherited, so which binary answered `grep` was decided by whoever
# invoked make. Here a saboteur `grep` is put first on PATH; the gate must still be green,
# because it resolves tools on its own pinned PATH and runs them by absolute path.
mkdir -p "$TMP/hijack"
printf '#!/bin/sh\necho "HIJACKED"\nexit 0\n' > "$TMP/hijack/grep"
printf '#!/bin/sh\necho "HIJACKED"\nexit 0\n' > "$TMP/hijack/ls"
chmod +x "$TMP/hijack/grep" "$TMP/hijack/ls"
OUT=$(PATH="$TMP/hijack:$PATH" python3 scripts/check_doc_evidence.py \
        --index-only --index "$TMP/truth.toml" 2>&1); RC=$?
if [ "$RC" -eq 0 ] && ! printf '%s\n' "$OUT" | grep -qF HIJACKED; then
  printf '  %sok%s   %s\n' "$GREEN" "$NC" "a hijacked PATH does not change WHICH BINARY runs"
  pass=$((pass+1))
else
  printf '  %sFAIL%s %s\n' "$RED" "$NC" "a hijacked PATH does not change WHICH BINARY runs"
  printf '         (exit %s)\n' "$RC"
  printf '%s\n' "$OUT" | sed 's/^/         | /' | head -10
  fail=$((fail+1))
fi

# CASE 23b. And the child's ENVIRONMENT is pinned too, which is a different mechanism
# from resolving the binary and needs its own control — the coverage script found this
# gap by reverting the env pinning and watching every control stay green. `GREP_OPTIONS`
# is honoured by the grep on this platform, so an inherited environment would let whoever
# invoked make change what the gate measures: with it set to -c, the true item below
# returns 3 counted lines instead of 6 matched ones.
OUT=$(GREP_OPTIONS=-c python3 scripts/check_doc_evidence.py \
        --index-only --index "$TMP/truth.toml" 2>&1); RC=$?
if [ "$RC" -eq 0 ]; then
  printf '  %sok%s   %s\n' "$GREEN" "$NC" \
    "an inherited GREP_OPTIONS does not change WHAT THE TOOL DOES"
  pass=$((pass+1))
else
  printf '  %sFAIL%s %s\n' "$RED" "$NC" \
    "an inherited GREP_OPTIONS does not change WHAT THE TOOL DOES"
  printf '         (exit %s)\n' "$RC"
  printf '%s\n' "$OUT" | sed 's/^/         | /' | head -10
  fail=$((fail+1))
fi

echo
echo "== every segment's status is a verdict, or the run establishes nothing =="
#
# WHITE-BOX, deliberately. On this five-command allowlist an upstream segment cannot
# easily be made to fail without also writing to stderr (which is caught earlier), so
# these drive run_pipeline directly with a saboteur argv -- the same fault-injection shape
# scripts/test-gate-probe.sh uses on the stdlib gate's producers. The rule under test is
# that a pipeline whose FIRST segment died still hands the last command an empty stream,
# and the last command's exit code cannot tell that from a true absence.
seg_case() {  # seg_case <name> <saboteur sh -c body> <expected fragment>
  local name=$1 body=$2 want=$3
  OUT=$(python3 - "$body" <<'PYEOF' 2>&1
import sys, pathlib
sys.path.insert(0, "scripts")
import check_doc_evidence as C
first = {"argv": ["/bin/sh", "-c", sys.argv[1]], "parsed": {"head": "grep"}}
last = {"argv": [C.shutil.which("grep", path=C.SAFE_PATH), "-v", "zzz"], "parsed": {"head": "grep"}}
rc, out, err = C.run_pipeline([first, last])
print(f"rc={rc} lines={len(out.splitlines())} err={err}")
PYEOF
)
  if printf '%s\n' "$OUT" | grep -qF "$want"; then
    printf '  %sok%s   %s\n' "$GREEN" "$NC" "$name"; pass=$((pass+1))
  else
    printf '  %sFAIL%s %s\n' "$RED" "$NC" "$name"
    printf '         (wanted %s)\n         | %s\n' "$want" "$OUT"
    fail=$((fail+1))
  fi
}
seg_case "an upstream segment exiting 3 is a MALFUNCTION, not a result" \
  'exit 3' 'MALFUNCTIONED'
seg_case "an upstream segment killed by a signal is a MALFUNCTION" \
  'kill -9 $$' 'MALFUNCTIONED'
seg_case "an upstream segment exiting 137 (shell convention) is a MALFUNCTION" \
  'exit 137' 'MALFUNCTIONED'
seg_case "an upstream segment exiting 1 (grep: no match) still concludes" \
  'exit 1' 'err=None'

# INTEGRATION, through the real allowlist. I argued in the previous round that a
# non-final failure was unreachable with only grep/ls/find/sort/wc downstream. That was
# WRONG, and review said so: `grep -q` terminates at its first match, so a sufficiently
# productive upstream is killed by SIGPIPE. Measured here, end to end, with two ordinary
# allowlisted greps -- no injection. The last segment exits 0 and prints nothing, which is
# indistinguishable from a true absence unless the upstream's status is also read.
index sigpipe unimplemented \
  "cmd: grep -rn e src/ --include='*.rs' | grep -q fn -> exit 0, 0 lines"
expect_red sigpipe "a real SIGPIPE from a downstream grep -q is caught (not white-box)" \
  "MALFUNCTIONED" "signal 13"

echo
echo "== gate: outcomes must come from a run, not from a memory =="

# CASE 28. A `gate:` result with nothing a machine can disagree with is prose, and prose
# is how `-> total=42 pass=39` outlived the output format that produced it.
index gate_prose implemented \
  "gate: make conformance -> everything looked fine to me"
expect_red gate_prose "a gate: result with no checkable token is refused" \
  "nothing checkable"

# CASES 29-33. Receipt validation, tested against receipts THIS SCRIPT WRITES.
#
# The first version read build_output/gate-receipts, i.e. whatever a previous target had
# left on disk. That is the defect this repository is named for, inside the gate built to
# close it: on a clean checkout there are no receipts, the case failed, and `make gates`
# could not have passed — it passed for me only because a manual run had left its output
# behind. Now the control mints its own receipt directory, so it is order-independent,
# clean-tree-safe, and unable to be satisfied by state from another run.
RCPT=$TMP/receipts
mkdir -p "$RCPT"
printf 'fixtures=70 evaluated=70 verified=46 vacuous=7 reject=14 failures=0\nall gates green\n' \
  > "$RCPT/fake.out"
printf 'make conformance\t0\tfake.out\n' > "$RCPT/index.tsv"

# gate_receipt <index-name> <run-id> -> sets RC and OUT
gate_receipt() {
  OUT=$(python3 scripts/check_doc_evidence.py --index-only --index "$TMP/$1.toml" \
          --gate-receipts "$RCPT" 2>&1); RC=$?
}
gate_case() {  # gate_case <index-name> <expect green|red> <case> [fragment]
  local name=$1 want=$2 case=$3 frag=${4:-}
  gate_receipt "$name"
  if [ "$want" = green ]; then
    if [ "$RC" -eq 0 ]; then
      printf '  %sok%s   %s\n' "$GREEN" "$NC" "$case"; pass=$((pass+1)); return
    fi
  elif [ "$RC" -ne 0 ] && printf '%s\n' "$OUT" | grep -qF -- "$frag"; then
    printf '  %sok%s   %s\n' "$GREEN" "$NC" "$case"; pass=$((pass+1)); return
  fi
  printf '  %sFAIL%s %s\n' "$RED" "$NC" "$case"
  printf '         (exit %s)\n' "$RC"
  printf '%s\n' "$OUT" | sed 's/^/         | /' | head -10
  fail=$((fail+1))
}

# The green side first, so the four reds below cannot be the harness misfiring.
index gate_true implemented "gate: make conformance -> verified=46 fixtures=70"
gate_case gate_true green "a gate: result the run DID print validates"

# THE TRUNCATION HOLE. Tokens were compared by containment, so `verified=4` was found
# inside `verified=46`. A number could drift downward and still validate, in the one
# mechanism whose whole purpose is that a number cannot drift.
index gate_truncated implemented "gate: make conformance -> verified=4 fixtures=7"
gate_case gate_truncated red \
  "a TRUNCATED number (verified=4 vs verified=46) is refused" "the run printed verified=46"

index gate_absent implemented "gate: make conformance -> verified=99999"
gate_case gate_absent red \
  "a number the gate did not print is refused" "the run printed verified=46"

# MEMBERSHIP IS NOT AGREEMENT. `seen[key]` was a set and a claim validated if it was ANY
# member, so a run printing both verified=46 and verified=4 endorsed either number.
printf 'verified=46\nlater, verified=4\n' > "$RCPT/fake.out"
gate_case gate_true red \
  "a self-contradicting run endorses NEITHER value" "more than one value"
printf 'fixtures=70 evaluated=70 verified=46 vacuous=7 reject=14 failures=0\nall gates green\n' \
  > "$RCPT/fake.out"


echo
echo "== the receipt RUNNER, and the order the certifying target runs it in =="
#
# These cover scripts/gate-receipts.sh, the Makefile and the CI workflow. Until now every
# control lived over scripts/check_doc_evidence.py, so fixes in those three files could be
# reverted with nothing going red -- while the coverage runner printed "every fix has a
# control". scripts/doc-evidence-fixes.tsv is now the reconciled denominator, and these
# are the controls that make its `mutated` rows mean something.

# A cited gate command is a string from a document. The grammar is enumerated, not a
# prefix: `make `* accepted `make conformance CC=clang`, `-j 8` and `--always-make`, each
# of which changes what the cited gate MEANS while being spelled like the gate it names.
gr_allowed() {   # -> 0 if scripts/gate-receipts.sh would run this command
  ( eval "$(sed -n '/^allowed() {/,/^}$/p' scripts/gate-receipts.sh)"; allowed "$1" )
}
gr_case() {      # gr_case <allow|refuse> <command>
  local want=$1 cmd=$2 got=refuse
  gr_allowed "$cmd" && got=allow
  if [ "$got" = "$want" ]; then
    printf '  %sok%s   gate command %-40s -> %s\n' "$GREEN" "$NC" "'$cmd'" "$want"
    pass=$((pass+1))
  else
    printf '  %sFAIL%s gate command %-40s -> %s\n' "$RED" "$NC" "'$cmd'" "$want"
    printf '         (it was %s)\n' "$got"
    fail=$((fail+1))
  fi
}
gr_case allow  "make conformance"
gr_case allow  "cargo build --release"
gr_case allow  "cargo test --release --lib lsp::"
gr_case refuse "make conformance CC=clang"
gr_case refuse "make conformance -j 8"
gr_case refuse "make conformance --always-make"
gr_case refuse "cargo build --release --features anything"
gr_case refuse "make conformance; rm -rf /"

# Receipts must not be DISCOVERABLE by or REUSABLE by the certifying path -- not "must not
# outlive the run", which is false under SIGKILL or host failure and which the runner's own
# documentation now disclaims; this comment said it anyway, which is a retraction applied in
# one file and not the other. The previous design wrote a run id NEXT TO the bytes it
# authenticated, so `--gate-run-id "$(cat .../RUN_ID)"` replayed an old run -- measured, it
# validated 10/10. Text-level on purpose: proving it by running the real thing costs ~31s
# and `make gate-receipts` in the same target already does. What is asserted is that no
# shared path is used, so no later run can find an earlier one's bytes, and that the
# private path is trapped for removal.
CASE="receipts are invocation-private and removed on exit"
if grep -q 'OUT=\$(mktemp -d' scripts/gate-receipts.sh \
   && grep -q "trap 'rm -rf \"\$OUT\"' EXIT" scripts/gate-receipts.sh \
   && ! grep -q 'OUT=build_output/gate-receipts' scripts/gate-receipts.sh; then
  printf '  %sok%s   %s\n' "$GREEN" "$NC" "$CASE"; pass=$((pass+1))
else
  printf '  %sFAIL%s %s\n' "$RED" "$NC" "$CASE"
  printf '         (a shared, surviving directory is replayable, and two runs race)\n'; fail=$((fail+1))
fi

# ... and the checker refuses a receipts directory that is repository content, so one
# cannot be committed and then pointed at.
mkdir -p "$INREPO/receipts"
printf 'make conformance\t0\tfake.out\n' > "$INREPO/receipts/index.tsv"
printf 'verified=46\n' > "$INREPO/receipts/fake.out"
index gate_inrepo implemented "gate: make conformance -> verified=46"
OUT=$(python3 scripts/check_doc_evidence.py --index-only --index "$TMP/gate_inrepo.toml" \
        --gate-receipts "$INREPO/receipts" 2>&1); RC=$?
CASE="a receipts directory inside the repository is refused"
if [ "$RC" -ne 0 ] && printf '%s\n' "$OUT" | grep -qF "inside the repository"; then
  printf '  %sok%s   %s\n' "$GREEN" "$NC" "$CASE"; pass=$((pass+1))
else
  printf '  %sFAIL%s %s\n' "$RED" "$NC" "$CASE"
  printf '         (it was accepted, exit %s)\n' "$RC"
  fail=$((fail+1))
fi

# CI WIRING. Not the order -- with the probe minting its own receipts the order fixes
# nothing, and a textual prerequisite check does not measure execution order under
# `make -j` anyway; that fix and its control are deleted rather than kept as a measurement
# of the wrong thing. What DOES matter is that the workflow invokes the evidence gate at
# all: before this branch it ran scripts/check-docs.sh directly and check-doc-evidence
# appeared in no workflow, so every cmd: item and every gate: receipt went unrun in CI.
for step in check-doc-evidence.sh gate-receipts.sh test-doc-evidence.sh; do
  if grep -qF "scripts/$step" .github/workflows/preview.yml; then
    printf '  %sok%s   CI runs scripts/%s\n' "$GREEN" "$NC" "$step"; pass=$((pass+1))
  else
    printf '  %sFAIL%s CI runs scripts/%s\n' "$RED" "$NC" "$step"
    printf '         (it does NOT, so that gate never runs on a push)\n'; fail=$((fail+1))
  fi
done

echo
echo "== a claim about what the compiler DOES needs evidence from a run =="

# CASE 30. The durable form of the pdc/cargo/make blocklist. That list stops one spelling;
# it does not stop a future author from deleting a compiler experiment, replacing it with
# a source grep, and satisfying the schema. Which is how this index came to say a program
# "compiles, links, prints 99, no diagnostic" about a program the compiler refuses.
index dyn_claim partial \
  "src: src/parser/mod.rs:553 lifetime_params, populated by the parser" \
  "cmd: grep -rn zzz_no_such_identifier_anywhere src/ --include='*.rs' -> exit 1, 0 lines"
expect_red dyn_claim "a partial/implemented row with only STATIC evidence is refused" \
  "claim about what the compiler" "every item here is static"

# CASE 31. And the same row passes the moment it carries something that came from a run,
# so the rule is a demand for evidence rather than a demand for more items.
index dyn_claim_ok partial \
  "src: src/parser/mod.rs:553 lifetime_params, populated by the parser" \
  "conformance: tests/reject/try_block.pd reject"
expect_green dyn_claim_ok "the same row passes once it cites a fixture"

# CASE 32. `unimplemented` is exempt on purpose: an absence is exactly what a `cmd:`
# absence proof is for, and 16 real rows legitimately rest on one. If this went red the
# rule would be demanding fixtures for features that do not exist.
index dyn_absence unimplemented \
  "cmd: grep -rn zzz_no_such_identifier_anywhere src/ --include='*.rs' -> exit 1, 0 lines"
expect_green dyn_absence "an unimplemented row may still rest on an absence proof"

echo
echo "== conformance: evidence must agree with the manifest that runs it =="

# CASE 12. The class must be what the manifest declares. `PASS` over a `vacuous` fixture
# is how a placeholder that only prints "unimplemented" got counted as coverage; the
# manifest separates those two columns precisely so that cannot happen.
index wrong_class unimplemented \
  "conformance: tests/07_traits_basic.pd run"
expect_red wrong_class "a class the manifest disagrees with is rejected" \
  "declares it 'vacuous'"

# CASE 13. A fixture nobody declared is run by no gate.
index undeclared unimplemented \
  "conformance: tests/reject/../reject/try_block.pd reject"
expect_red undeclared "an undeclared fixture is rejected" "not declared in"

echo
echo "=============================================="
if [ "$fail" -eq 0 ]; then
  echo "${GREEN}doc-evidence gate probe green${NC} -- $pass case(s): the gate passes a true"
  echo "index and goes red, for the stated reason, on every falsehood above."
  echo "=============================================="
  exit 0
fi
echo "${RED}doc-evidence gate probe FAILED${NC} -- $fail of $((pass+fail)) case(s)"
echo "=============================================="
exit 1
