#!/usr/bin/env bash
# The evidence gate for stable diagnostic codes — GI-12 spec D4/R2.
#
# WHAT THIS TARGET IS NOT. It is NOT a second runner over the 122 reject rows.
# A gate that re-executed the corpus its own way would be a second authority, and
# two authorities over one question means the weaker one decides every
# disagreement — in practice the one that says PASS. Execution belongs to
# `scripts/conformance.sh`, which this file INVOKES (R2) so that its verdict is
# folded in: registry-green with the corpus RED is RED here.
#
# WHAT IT OWNS.
#   1. REGISTRY COHERENCE. Uniqueness of code and symbolic name, the column
#      grammar, status values, tombstones un-pinnable and undeletable,
#      first_witness is a real refusal-witness row, introduced_commit is `-` or a
#      real commit.
#   2. COMPILER INVENTORY. Every code the BINARY can emit
#      (`pdc --dump-diagnostic-codes`) is an active registry row. Asked of the
#      binary, not of a grep over the source: a grep reads a code named in a
#      comment as emitted and a code built by `format!` as absent.
#      THE REVERSE DIRECTION is two checks, both below: every active code is
#      PINNED by at least one manifest row (3), and EMITTED by its first witness
#      (5). Neither could hold before the cutover; both are required after it.
#   3. MANIFEST PINS — the manifest is CODE-AUTHORITY since the GI-12 cutover.
#      Every reject row (stage compile) and skip row MUST pin
#      `code=PD####[;msg~<fragment>]`, exactly: no bare `PD####`, no spaces, no
#      phrase, no tombstoned or unregistered code. The SET of refusal rows is
#      pinned by digest (R5, over path, class AND stage), so a compensated retype
#      goes red; its SIZE is a committed constant beside the digest, and both the
#      manifest's code-pinned rows and the counts conformance prints are held to
#      it — a REQUIREMENT here, not an observation. Every pin must be exactly the
#      pin docs/contributing/diagnostic-pin-map.tsv dictates (a textual check,
#      no compile), and the pin grammar is one literal in three files.
#   4. PARSER SELF-TESTS. The shared parser (scripts/lib/diag-parse.sh) is
#      handed planted mutants and must report each one correctly. The mutants run
#      in a temp dir against temp manifests. THE LIVE CORPUS IS NEVER MUTATED —
#      a gate that edits the thing it certifies can leave it edited.
#   5. FIRST-WITNESS EMISSION. Each active code's first_witness is compiled for
#      real, and its stderr must carry that code. This is the only check that
#      proves the registry describes THIS BINARY and not a past one.
#   6. WIRING. The mutants call the check FUNCTIONS, so they stay green if the
#      LIVE call of a check is deleted. Every live check therefore runs through
#      `live`, which records that it reached a verdict, and the run ends by
#      requiring every label in LIVE_REQUIRED: a missing one is RED, by name.
#
# THREE-VALUED EXIT, and the aggregation may not swallow the third.
#   0 = every check passed.
#   1 = a check failed. A statement about the codes.
#   2 = a check COULD NOT BE MADE (a missing binary, an unreadable file, a
#       conformance run that did not produce a verdict). NOT a pass and NOT a
#       fail: the previous milestone's lesson was a gate that reported success
#       because it never managed to look.
#
# Usage: bash scripts/check-diagnostic-codes.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

PDC=./target/release/pdc
REGISTRY=docs/contributing/diagnostic-codes.tsv
MANIFEST=${CONFORMANCE_MANIFEST:-tests/conformance-manifest.txt}
PIN_MAP=docs/contributing/diagnostic-pin-map.tsv
GENERATOR=scripts/gen-code-pins.py

# THE PIN GRAMMAR — column 4 of a reject (stage compile) or skip row. The same
# literal is written in scripts/conformance.sh (`PIN_RE`, which refuses to READ any
# other spelling) and scripts/gen-code-pins.py (`PIN_GRAMMAR`, which refuses to
# WRITE one). Three copies of one regex are three programs the day one is edited,
# so `check_pin_grammar_copies` holds all three to this one, character for
# character.
PIN_RE='^code=PD[0-9]{4}(;msg~.+)?$'

GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; YELLOW=$'\033[0;33m'; NC=$'\033[0m'

# Environment overrides are refused on the evidence path. A gate that can be
# pointed at another manifest, another compiler or a blessing mode is a gate
# whose verdict is a function of the caller's environment.
for var in CONFORMANCE_BLESS CONFORMANCE_MANIFEST CONFORMANCE_FORBID_OWNER PDC_OVERRIDE; do
  if [ -n "${!var:-}" ]; then
    echo "error: $var is set. This gate refuses environment overrides on the" >&2
    echo "       evidence path — its verdict may not depend on the caller." >&2
    exit 2
  fi
done

[ -x "$PDC" ]      || { echo "error: $PDC not built. Run: cargo build --release" >&2; exit 2; }
# Resolved ONCE, and absolute: the witness compiles run from a scratch directory,
# so a repo-relative spelling would have to be rebuilt against `$OLDPWD` at every
# call site. One variable also gives the mutants below a seam — a stub compiler
# can be substituted for exactly one run without touching anything else.
PDC_ABS="$(cd "$(dirname "$PDC")" && pwd)/$(basename "$PDC")"
ROOT="$PWD"
[ -r "$REGISTRY" ] || { echo "error: registry $REGISTRY not readable" >&2; exit 2; }
[ -r "$MANIFEST" ] || { echo "error: manifest $MANIFEST not readable" >&2; exit 2; }
[ -r scripts/lib/diag-parse.sh ] || { echo "error: shared parser missing" >&2; exit 2; }
[ -r "$PIN_MAP" ]   || { echo "error: pin map $PIN_MAP not readable" >&2; exit 2; }
[ -r "$GENERATOR" ] || { echo "error: pin generator $GENERATOR not readable" >&2; exit 2; }

. scripts/lib/diag-parse.sh

TMPROOT=$(mktemp -d) || exit 2
trap 'rm -rf "$TMPROOT"' EXIT INT TERM

fails=0        # MEASURED defects. A statement about the codes.
abstained=0    # measurements that COULD NOT BE MADE. Not a defect and not a pass.
unrun=0        # controls this repository cannot host. Named, counted, never silent.
ok()   { printf '  %sok%s   %s\n' "$GREEN" "$NC" "$1"; }
bad()  { printf '  %sRED%s  %s\n' "$RED" "$NC" "$1"; fails=$((fails+1)); }
absta(){ printf '  %sNO VERDICT%s  %s\n' "$YELLOW" "$NC" "$1"; abstained=$((abstained+1)); }
note() { printf '  %s--%s   %s\n' "$YELLOW" "$NC" "$1"; }

# THE LIVE CALLS, AND THE PROOF THAT EACH ONE HAPPENED.
#
# The planted mutants below call `check_codes_pinned`, `check_membership`,
# `check_code_counts` and the rest DIRECTLY, on temp copies — which proves each
# function can go red and proves nothing about whether this run ever asks it
# about the live pair. Deleting a live call left every mutant green (suF-a review
# round 1). So a live check is ONE statement — `live <label> <ok-text> <check>
# [args...]` — that runs the check, prints its verdict, and records the label;
# deleting the call deletes the record, and `check_live_reached` at the end of the
# run names every label of LIVE_REQUIRED that was never recorded. A label is
# recorded whatever the verdict, because a check that ran and went RED has
# already said so: what this catches is the check that never ran.
#
# <ok-text> is printed as is, or — when it names a function — that function's
# output AFTER the check ran, for an ok line that reports what the check produced.
# An empty <ok-text> prints no summary line, for a check whose own lines say it.
#
# THREE-VALUED, LIKE THE GATE. A check returns 0 clean, 1 a measured defect, or
# 2 COULD NOT LOOK, and `live` spends each as what it is: 2 is a NO VERDICT, never
# a RED — `live` used to print every nonzero return as RED, which turns an
# unreadable file into a finding about the codes. Any other status is not a
# verdict at all and is a NO VERDICT too. Output lines are routed by their TAG
# when they carry one (`OK `, `RED `, `NOVERDICT `, the dialect
# `check_first_witness_emission` speaks, so that check goes through here like the
# others); an untagged line takes the return code's meaning. A return of 1 that
# names no defect, or of 2 that says nothing, is still spent as 1 or 2: the code
# is the verdict and the lines only explain it. Pinned by M30 below.
LIVE_REQUIRED=("registry" "inventory" "pin grammar" "every active code pinned"
               "R5 membership" "refusal-set size" "map = manifest" "pin grammar copies"
               "first-witness emission" "counts requirement")
LIVE_REACHED=()
live() {                     # label, ok-text | ok-function | "", check, args...
  local label=$1 text=$2 out rc l f0=$fails a0=$abstained; shift 2
  out=$("$@"); rc=$?
  case "$rc" in
    0|1|2) ;;
    *) absta "$label: the check returned $rc, which is none of 0 (clean), 1 (a measured defect) or 2 (could not look) — no verdict"
       LIVE_REACHED+=("$label"); return ;;
  esac
  while IFS= read -r l; do
    [ -n "$l" ] || continue
    case "$l" in
      OK\ *)        ok "${l#OK }" ;;
      RED\ *)       bad "$label: ${l#RED }" ;;
      NOVERDICT\ *) absta "$label: ${l#NOVERDICT }" ;;
      *) case "$rc" in
           1) bad "$label: $l" ;;
           2) absta "$label: $l" ;;
         esac ;;
    esac
  done <<<"$out"
  if [ "$rc" -eq 1 ] && [ "$fails" -eq "$f0" ]; then
    bad "$label: the check returned 1 and named no defect"
  elif [ "$rc" -eq 2 ] && [ "$fails" -eq "$f0" ] && [ "$abstained" -eq "$a0" ]; then
    absta "$label: the check returned 2 (could not look) and said nothing"
  elif [ "$rc" -eq 0 ] && [ -n "$text" ]; then
    declare -F "$text" >/dev/null && text=$("$text")
    ok "$text"
  fi
  LIVE_REACHED+=("$label")
}
check_live_reached() {       # -> the unreached labels, one per line; 0 none / 1 some
  local want got hit n=0
  for want in "${LIVE_REQUIRED[@]}"; do
    hit=0
    for got in ${LIVE_REACHED[@]+"${LIVE_REACHED[@]}"}; do
      [ "$got" = "$want" ] && hit=1
    done
    [ "$hit" -eq 1 ] || { echo "$want"; n=$((n+1)); }
  done
  [ "$n" -eq 0 ]
}

# THE AGGREGATION RULE, WRITTEN DOWN BECAUSE TWO NON-ZERO STATES CAN COEXIST.
# A run can hold both a measured defect and a measurement that failed, and the
# gate has one exit code to say it with. MEASURED RED WINS: 1 beats 2. If some
# check found a real defect, that is the most useful true thing the gate knows,
# and reporting `could not measure` instead would bury it behind whichever
# unrelated capture happened to be unreadable. 2 is reserved for the case where
# NOTHING was found wrong and something could not be looked at — the only case in
# which "green" would be a claim the run did not earn.
#
#   $1 = measured failures, $2 = abstentions -> 0 | 1 | 2 on stdout.
#   A pure function, so the priority is a checked case below and not a comment.
final_exit_code() {
  if [ "$1" -gt 0 ]; then printf '1\n'
  elif [ "$2" -gt 0 ]; then printf '2\n'
  else printf '0\n'; fi
}

# THE ONLY WAY OUT once measuring has begun.
#
# A rule that lives in one function and a terminal that does not call it is not a
# rule, it is a comment. This gate had exactly that: the corpus fold's NO_VERDICT
# arm ran its own `exit 2`, so a registry check that had ALREADY found a real
# defect was buried the moment an unrelated conformance run failed to reach a
# verdict — the gate would report "could not measure" while holding a measured
# RED it had printed four lines earlier. Two harness bails did the same thing.
# So every post-measurement terminal is this function, and the priority rule is
# stated in exactly one place.
#
# WHAT IS DELIBERATELY NOT ROUTED HERE: the preconditions above (no compiler, no
# registry, no manifest, no temp dir, a refused environment override). Those fire
# BEFORE any measurement exists, so there is no verdict to outrank and `exit 2`
# is the whole truth about the run.
finish() {
  echo
  [ "$unrun" -eq 0 ] || printf '%s%d control(s) NOT RUN, named above%s\n' "$YELLOW" "$unrun" "$NC"
  case "$(final_exit_code "$fails" "$abstained")" in
    0) printf '%s✓ diagnostic codes: every check green%s\n' "$GREEN" "$NC"
       exit 0 ;;
    1) printf '%s✗ diagnostic codes: %d check(s) RED%s' "$RED" "$fails" "$NC"
       [ "$abstained" -eq 0 ] \
         && printf '\n' \
         || printf '%s (and %d abstention(s), reported but outranked)%s\n' "$YELLOW" "$abstained" "$NC"
       exit 1 ;;
    2) printf '%s? diagnostic codes: nothing was found wrong, and %d check(s) could not be\n' "$YELLOW" "$abstained"
       printf '  made. This is not a pass — the gate has not been shown to hold.%s\n' "$NC"
       exit 2 ;;
  esac
}

# The corpus verdict, ROUTED rather than acted on inline, so the whole-gate
# control-flow mutants below can walk this exact arm with a planted prior RED.
#   $1 = verdict, $2 = conformance's exit status, $3 = its summary, $4 = its log
apply_conformance_verdict() {
  case "$1" in
    GREEN)
      ok "conformance green — $3"
      [ -r "$4" ] && ok "$(grep -m1 '^diagnostic-codes' "$4")" ;;
    RED)
      bad "conformance exited $2 — $3" ;;
    NO_VERDICT)
      # An ABSTENTION, counted, not a terminal. The corpus could not be measured,
      # which says nothing about anything this gate measured for itself.
      absta "conformance exited $2${3:+ — $3} — the corpus verdict could not be measured"
      echo "note: exit 2 is not a pass and not a failure. If some check above found" >&2
      echo "      a real defect, that outranks this and the gate exits 1." >&2 ;;
  esac
}

# ---------------------------------------------------------------------------
# The checks, as FUNCTIONS OVER (registry, manifest), so the mutants below can
# re-run exactly the code that certifies the live pair. A self-test that
# exercised a re-implementation would prove nothing about the check that runs.
# Each prints its complaints on stdout, one per line, and returns a BOOLEAN
# status: 0 clean, 1 complained. NOT the complaint COUNT — a shell status is
# taken modulo 256, so a function returning `n` reports 256 complaints as
# SUCCESS. The count is the number of lines the caller reads, which cannot wrap.
# ---------------------------------------------------------------------------

registry_rows() { grep -v '^#' "$1" | tail -n +2; }

# THREE-VALUED, AND THE THIRD VALUE IS NOT ALLOWED TO BECOME THE SECOND. The
# previous shape was `if [ "$conf_rc" -ne 0 ]; then bad`, which reported a
# conformance run that COULD NOT MEASURE as a measured failure and left this gate
# exiting 1. That is the GI-08 lesson verbatim, and it is reachable rather than
# theoretical: `scripts/conformance.sh:1063` exits 2 under `CONFORMANCE_BLESS`
# AFTER printing the `verified=` summary at :1006, so the "summary is present"
# branch above did not protect against it either.
#
#   $1 = conformance's exit status, $2 = its summary line (may be empty)
#   -> GREEN | RED | NO_VERDICT, on stdout. A pure decision, so the self-test
#      below can hand it statuses this run did not produce.
fold_conformance_verdict() {
  local rc=$1 summary=$2
  if [ -z "$summary" ]; then printf 'NO_VERDICT\n'; return; fi
  case "$rc" in
    0) printf 'GREEN\n' ;;
    1) printf 'RED\n' ;;
    *) printf 'NO_VERDICT\n' ;;
  esac
}

check_registry() {           # $1 = registry -> 0 clean / 1 complained
  local reg=$1 n=0 line code name status cond just wit commit
  local -a seen_codes=() seen_names=()

  # THE HEADER IS THE FIRST NON-COMMENT LINE, derived exactly the way
  # `registry_rows` derives the data (it drops this line), so the two cannot
  # disagree about where the table starts. This used to be `head -50`, a fixed
  # window over a preamble that grows: adding a paragraph of documentation to the
  # registry pushed the header to line 52 and the gate reported the columns
  # missing. A window is not a derivation.
  if [ "$(grep -v '^#' "$reg" | head -1)" != \
       'code	symbolic_name	status	semantic_condition	justification	first_witness	introduced_commit' ]; then
    echo "registry header row is missing or has the wrong columns"; n=$((n+1))
  fi

  while IFS=$'\t' read -r code name status cond just wit commit; do
    [ -n "${code:-}" ] || continue
    [ -n "${commit:-}" ] || { echo "$code: row does not have 7 tab-separated columns"; n=$((n+1)); continue; }

    [[ $code =~ ^PD[0-9]{4}$ ]] || { echo "$code: not spelled PD then exactly four digits"; n=$((n+1)); }
    [[ $name =~ ^[a-z][a-z0-9_]*$ ]] || { echo "$code: symbolic_name '$name' is not snake_case"; n=$((n+1)); }
    case "$status" in active|tombstone) ;; *) echo "$code: status '$status' is not active|tombstone"; n=$((n+1)) ;; esac
    [ -n "$cond" ] && [ "$cond" != "-" ] || { echo "$code: semantic_condition is empty — a code with no rule is a number"; n=$((n+1)); }
    [ -n "$just" ] || { echo "$code: justification is empty"; n=$((n+1)); }

    for c in ${seen_codes[@]+"${seen_codes[@]}"}; do
      [ "$c" = "$code" ] && { echo "$code: duplicate code row"; n=$((n+1)); }
    done
    for c in ${seen_names[@]+"${seen_names[@]}"}; do
      [ "$c" = "$name" ] && { echo "$code: symbolic_name '$name' is already used"; n=$((n+1)); }
    done
    seen_codes+=("$code"); seen_names+=("$name")

    if [ "$status" = active ]; then
      # R7: the witness class is REFUSAL-witness — a reject row, or one of the
      # two skip rows, whose refusal is their non-program proof.
      if [ "$wit" = "-" ]; then
        echo "$code: active row has no first_witness"; n=$((n+1))
      elif [ ! -f "$wit" ]; then
        echo "$code: first_witness $wit is not a file"; n=$((n+1))
      else
        local cls
        cls=$(awk -F'\t' -v p="$wit" '$1==p{print $2}' "$MANIFEST")
        case "$cls" in
          reject|skip) ;;
          "") echo "$code: first_witness $wit is not declared in the manifest"; n=$((n+1)) ;;
          *)  echo "$code: first_witness $wit is class '$cls', not a refusal witness"; n=$((n+1)) ;;
        esac
      fi
    else
      [ "$wit" = "-" ] || { echo "$code: a tombstone may not claim a witness (its witnesses belong to the survivor)"; n=$((n+1)); }
    fi

    # `-` for a tombstone or a row still in a working tree; otherwise a commit REACHABLE
    # FROM HEAD. `cat-file -e` was the wrong predicate and this is the difference
    # it missed: every commit on every other branch, and every commit of an
    # abandoned line of work, is an object in this repository. Measured here, 17
    # such commits exist right now (`git rev-list --all --not HEAD`), so the old
    # check would have accepted any of them as the provenance of a code that this
    # history never introduced. The claim the column makes is "this tree's history
    # contains the commit that first emitted this code", and that is ancestry.
    if [ "$commit" != "-" ]; then
      if ! git cat-file -e "$commit^{commit}" 2>/dev/null; then
        echo "$code: introduced_commit '$commit' is not a commit in this repository"; n=$((n+1))
      elif ! git merge-base --is-ancestor "$commit" HEAD 2>/dev/null; then
        echo "$code: introduced_commit '$commit' is a commit but is NOT an ancestor of HEAD — this history did not introduce it"; n=$((n+1))
      fi
    fi
  done < <(registry_rows "$reg")

  # D7's no-reuse promise is NOT checked here, deliberately. Deriving the
  # tombstone list from the file under test makes the check self-defeating: the
  # mutation that revives PD0025 also removes it from the list the check would
  # read, and the check passes by having nothing to look at. It lives in
  # `check_compiler_inventory`, where the binary's own `TOMBSTONES` is the second
  # authority, and in `src/errors/codes.rs`'s
  # `no_active_code_reuses_a_tombstoned_number`.
  [ "$n" -eq 0 ]
}

# A DUMP THAT COULD NOT BE TAKEN IS NOT A DISAGREEMENT. If the binary is missing,
# cannot run, or fails the dump, it was never asked, and nothing is known about
# its codes: that is a NO VERDICT (2), tagged so `live` spends it as one. It used
# to return 1, which reported a broken binary as a finding about the registry.
# A dump that SUCCEEDS and lists nothing stays a RED: the binary answered, an
# inventory with no code in it contradicts the registry it is held to, and the
# loop below would otherwise pass by having nothing to read. M31 pins both.
check_compiler_inventory() { # $1 = registry -> 0 clean / 1 complained / 2 could not ask
  local reg=$1 n=0 code status name
  local dump="$TMPROOT/dump"
  if ! "$PDC" --dump-diagnostic-codes >"$dump" 2>/dev/null; then
    echo "NOVERDICT pdc --dump-diagnostic-codes did not succeed, so what the binary can emit is unknown"
    return 2
  fi
  [ -s "$dump" ] || { echo "RED pdc --dump-diagnostic-codes succeeded and listed no code at all"; return 1; }
  while IFS=$'\t' read -r code status name; do
    [ -n "$code" ] || continue
    local rstatus rname
    rstatus=$(awk -F'\t' -v c="$code" '$1==c{print $3}' <(registry_rows "$reg"))
    rname=$(awk -F'\t' -v c="$code" '$1==c{print $2}' <(registry_rows "$reg"))
    if [ -z "$rstatus" ]; then
      echo "$code: the binary knows it and the registry does not list it"; n=$((n+1)); continue
    fi
    # THE CROSS-AUTHORITY CHECK. The compiler's own tombstone list and the
    # registry must agree about status, and neither derives from the other: a
    # retired number turned back on in the TSV is caught here even though the
    # TSV then no longer calls it a tombstone.
    if [ "$rstatus" != "$status" ]; then
      echo "$code: the binary says $status, the registry says $rstatus — a retired number may not be revived"
      n=$((n+1)); continue
    fi
    if [ "$status" = active ] && [ "$rname" != "$name" ]; then
      echo "$code: symbolic_name disagrees — binary says '$name', registry says '$rname'"; n=$((n+1))
    fi
  done <"$dump"
  [ "$n" -eq 0 ]
}

# EVERY refusal row, not every row that happens to look like a pin. Before the
# cutover this read only observables already spelled `code=…`, which was right
# while phrases were the authority and is a hole after: a reject row that kept a
# phrase would simply not be looked at. A reject row at stage `run` pins
# `exit=<N>` — a rule the cutover left alone — and is the one exemption.
check_manifest_pins() {      # $1 = registry, $2 = manifest -> 0 clean / 1 complained
  local reg=$1 man=$2 n=0 path cls stage obs rest code
  while IFS=$'\t' read -r path cls stage obs rest; do
    case "$cls" in reject|skip) ;; *) continue ;; esac
    [ "$cls" = reject ] && [ "$stage" = run ] && continue
    if [[ ! $obs =~ $PIN_RE ]]; then
      echo "$path: class=$cls observable '$obs' is not exactly code=PD####[;msg~<fragment>]"; n=$((n+1)); continue
    fi
    code=${obs:5:6}
    local status
    status=$(awk -F'\t' -v c="$code" '$1==c{print $3}' <(registry_rows "$reg"))
    case "$status" in
      active) ;;
      tombstone) echo "$path: pins $code, which is a tombstone — a retired code may not be pinned"; n=$((n+1)) ;;
      *) echo "$path: pins $code, which is not in the registry"; n=$((n+1)) ;;
    esac
  done < <(grep -v '^#' "$man")
  [ "$n" -eq 0 ]
}

# The reverse of check_compiler_inventory's direction, on the manifest side: an
# ACTIVE code that no refusal row pins is a number with no corpus witness to it
# being enforced — the registry would describe a rule the gate never exercises.
check_codes_pinned() {       # $1 = registry, $2 = manifest -> 0 clean / 1 complained
  local reg=$1 man=$2 n=0 code
  local pinned
  pinned=$(awk -F'\t' '$1 !~ /^#/ && ($2=="reject" || $2=="skip") && $4 ~ /^code=PD[0-9][0-9][0-9][0-9]/ {print substr($4, 6, 6)}' "$man" | sort -u)
  while IFS= read -r code; do
    [ -n "$code" ] || continue
    printf '%s\n' "$pinned" | grep -qx -- "$code" \
      || { echo "$code: active in the registry, and no reject or skip row pins it"; n=$((n+1)); }
  done < <(awk -F'\t' '$3=="active"{print $1}' <(registry_rows "$reg"))
  [ "$n" -eq 0 ]
}

# R5 — THE MEMBERSHIP PIN. A digest of the EXACT SET of refusal rows, as sorted
# `path<TAB>class<TAB>stage` lines over every `reject` and `skip` row of the
# manifest. Not a count: retyping reject A to xfail and xfail B to reject keeps
# every count where it was and changes this. THE STAGE IS IN IT because a
# `reject|compile -> reject|run (exit=N)` retype keeps path and class, and moves
# the row out of the code-pinned set: it stops being judged by a code at all.
# SCOPED TO THE REFUSAL ROWS ON PURPOSE — those are the rows this gate certifies,
# and a digest over the whole manifest would turn every unrelated xfail-to-run
# payoff into a red here.
#
# RE-PINNING IS A DECISION. Adding, removing or retyping a refusal row changes
# the digest; the run prints the new one, and the edit below is where a reviewer
# reads which rows moved. `scripts/gen-code-pins.py` prints the same digest
# (`refusal-rows`) before and after it writes, computed the same way.
#   was 15378d68… at the GI-12 cutover, over `path<TAB>class`: 122 reject + 2 skip.
#   re-pinned e6554cd0… at suF-a review round 1: the SAME 124 rows (122
#   reject|compile + 2 skip|compile), now hashed with their stage.
REFUSAL_SET_SHA=e6554cd0fa978da4734b8216551a35c505c482fe9974b3b2434c27acdc4bb910
# THE SIZE OF THAT SET, pinned beside it and re-pinned by the same deliberate
# edit. Every row of the set is at stage compile and pins a code, so this is also
# the number of code-pinned refusal rows and the `coded=` conformance must report.
# It is a COMMITTED number on purpose: the counts requirement used to be held to a
# count taken from the live manifest, and deleting refusal rows lowered both sides
# together (suF-a review round 1).
REFUSAL_SET_SIZE=124
refusal_set_digest() {       # $1 = manifest -> sha256 on stdout
  awk -F'\t' 'NF>=2 && $1 !~ /^#/ && ($2=="reject" || $2=="skip") {print $1 "\t" $2 "\t" $3}' "$1" \
    | LC_ALL=C sort | shasum -a 256 | cut -d' ' -f1
}
check_membership() {         # $1 = manifest -> 0 clean / 1 complained
  local got
  got=$(refusal_set_digest "$1")
  if [ "$got" != "$REFUSAL_SET_SHA" ]; then
    echo "the set of refusal rows changed: digest $got, pinned $REFUSAL_SET_SHA ($(awk -F'\t' 'NF>=2 && $1 !~ /^#/ && ($2=="reject" || $2=="skip")' "$1" | wc -l | tr -d ' ') reject/skip rows now). A row was added, removed or retyped (class or stage); re-pin REFUSAL_SET_SHA and REFUSAL_SET_SIZE deliberately, naming the rows"
    return 1
  fi
  return 0
}

# The manifest's code-pinned refusal rows, counted, against the committed size.
# R5 already refuses a changed set; this is the half that makes REFUSAL_SET_SIZE a
# statement about the manifest and not only about conformance's tally.
code_pinned_rows() {         # $1 = manifest -> count on stdout
  awk -F'\t' '$1 !~ /^#/ && ($2=="reject" || $2=="skip") && $4 ~ /^code=PD/' "$1" | wc -l | tr -d ' '
}
check_refusal_size() {       # $1 = manifest -> 0 clean / 1 complained
  local got
  got=$(code_pinned_rows "$1")
  if [ "$got" != "$REFUSAL_SET_SIZE" ]; then
    echo "$got refusal row(s) pin a code, and the committed refusal-set size is $REFUSAL_SET_SIZE — rows were added or removed; re-pin REFUSAL_SET_SIZE with REFUSAL_SET_SHA, deliberately"
    return 1
  fi
  return 0
}

# THE COUNTS LINE IS A REQUIREMENT, held to a COMMITTED number. Exactly this:
# conformance's `diagnostic-codes:` line must parse, its `coded=` must EQUAL $2,
# and its `uncoded=`, `malformed=` and `unreadable=` must each be 0. The live call
# passes REFUSAL_SET_SIZE — not a count of the live manifest, which a deleted row
# lowers in step with the sweep. It does not say WHICH rows were coded; that every
# refusal row is in the set and pins a code is R5's and check_refusal_size's.
#   $1 = the `diagnostic-codes:` line (may be empty), $2 = the required coded count
check_code_counts() {
  local line=$1 want=$2 c u m r
  if [ -z "$line" ]; then
    echo "conformance printed no \`diagnostic-codes:\` line, so what its comparator saw is unknown"; return 1
  fi
  c=$(printf '%s' "$line" | sed -n 's/.* coded=\([0-9]*\).*/\1/p')
  u=$(printf '%s' "$line" | sed -n 's/.* uncoded=\([0-9]*\).*/\1/p')
  m=$(printf '%s' "$line" | sed -n 's/.* malformed=\([0-9]*\).*/\1/p')
  r=$(printf '%s' "$line" | sed -n 's/.* unreadable=\([0-9]*\).*/\1/p')
  if [ -z "$c" ] || [ -z "$u" ] || [ -z "$m" ] || [ -z "$r" ]; then
    echo "the counts line does not parse: $line"; return 1
  fi
  if [ "$c" -ne "$want" ] || [ "$u" -ne 0 ] || [ "$m" -ne 0 ] || [ "$r" -ne 0 ]; then
    echo "conformance adjudicated coded=$c uncoded=$u malformed=$m unreadable=$r; the committed refusal-set size is $want, so the requirement is coded=$want and the rest 0"
    return 1
  fi
  return 0
}

# B7 — THE MAP AND THE MANIFEST SAY THE SAME THING. TEXTUAL, no compile: the
# generator measured every payload when it wrote the pins, and this asks only that
# nothing has been written since by any other hand. Every refusal row's column 4
# must be EXACTLY the pin the map dictates — `code=<code>`, plus `;msg~<fragment>`
# where the map says `msg_tilde=yes` — and the map and the manifest must name the
# same refusal rows, with the same class. The domain is the generator's own: a
# reject row at stage `run` pins `exit=<N>` and is outside both. The map's header
# is its first line that is neither blank nor a `#` comment, as the generator
# reads it.
check_map_pins() {           # $1 = pin map, $2 = manifest -> 0 clean / 1 complained
  awk -F'\t' '
    FNR == NR {
      sub(/\r$/, "")
      if ($0 == "" || $0 ~ /^#/) next
      if (!hdr) { hdr = 1; next }
      if (NF != 11) { printf "map row %s has %d columns, not 11\n", $4, NF; bad++; next }
      if ($4 in want) { printf "%s: in the map twice\n", $4; bad++; next }
      if ($9 == "yes")     want[$4] = "code=" $1 ";msg~" $10
      else if ($9 == "no") want[$4] = "code=" $1
      else { printf "%s: the map says msg_tilde=%s, which dictates no pin\n", $4, $9; bad++; want[$4] = "" }
      cls[$4] = $5
      next
    }
    { sub(/\r$/, "") }
    $1 ~ /^#/ || NF < 4 { next }
    ($2 == "reject" || $2 == "skip") && !($2 == "reject" && $3 == "run") {
      seen[$1] = 1
      if (!($1 in want)) { printf "%s: a %s row of the manifest that the map does not name\n", $1, $2; bad++; next }
      if ($2 != cls[$1]) { printf "%s: the manifest says class %s, the map says %s\n", $1, $2, cls[$1]; bad++ }
      if (want[$1] != "" && $4 != want[$1]) {
        printf "%s: the manifest pins %s and the map dictates %s — a pin is changed in the map and re-derived by scripts/gen-code-pins.py, never by hand\n", $1, $4, want[$1]; bad++
      }
    }
    END {
      for (p in want) if (!(p in seen)) { printf "%s: in the map, but not a reject/skip row of the manifest\n", p; bad++ }
      exit (bad > 0)
    }' "$1" "$2"
}

# B6 — ONE PIN GRAMMAR. The literal in scripts/gen-code-pins.py (`PIN_GRAMMAR`) and
# the one in scripts/conformance.sh (`PIN_RE`) must each be exactly this file's
# PIN_RE: a writer that accepts a pin the reader refuses, or the reverse, is the
# drift two copies of one regex invite. Read as TEXT, each from its one defining
# line; a file with no such line, or two, is a complaint, not a skip.
check_pin_grammar_copies() { # $1 = gen-code-pins.py, $2 = conformance.sh -> 0 clean / 1 complained
  local gen conf n=0 k
  gen=$(sed -n 's/^PIN_GRAMMAR = re\.compile(r"\(.*\)")$/\1/p' "$1")
  conf=$(sed -n "s/^PIN_RE='\(.*\)'\$/\1/p" "$2")
  k=$(printf '%s' "$gen" | grep -c '^')
  if [ "$k" -ne 1 ]; then
    echo "$1: $k \`PIN_GRAMMAR = re.compile(r\"…\")\` line(s), want exactly 1"; n=$((n+1))
  elif [ "$gen" != "$PIN_RE" ]; then
    echo "$1's PIN_GRAMMAR is '$gen', and the grammar is '$PIN_RE' — the writer and the reader disagree"; n=$((n+1))
  fi
  k=$(printf '%s' "$conf" | grep -c '^')
  if [ "$k" -ne 1 ]; then
    echo "$2: $k \`PIN_RE='…'\` line(s), want exactly 1"; n=$((n+1))
  elif [ "$conf" != "$PIN_RE" ]; then
    echo "$2's PIN_RE is '$conf', and the grammar is '$PIN_RE' — the reader and this gate disagree"; n=$((n+1))
  fi
  [ "$n" -eq 0 ]
}

# ---------------------------------------------------------------------------
# 1..3 — the live pair
# ---------------------------------------------------------------------------
echo "=============================================="
echo "diagnostic codes: registry, inventory, parser"
echo "=============================================="

# Each live check is ONE `live` statement: the call and the record that it ran
# cannot be separated (see LIVE_REQUIRED above).
live "registry" "registry coherent ($(registry_rows "$REGISTRY" | wc -l | tr -d ' ') rows: $(awk -F'\t' '$3=="active"' <(registry_rows "$REGISTRY") | wc -l | tr -d ' ') active, $(awk -F'\t' '$3=="tombstone"' <(registry_rows "$REGISTRY") | wc -l | tr -d ' ') tombstone)" \
  check_registry "$REGISTRY"

# The counts come from the dump the check itself writes, so the ok line is a
# function `live` calls AFTER the check, not text computed before it.
inventory_ok() {
  printf 'binary and registry agree on every code the binary knows (%s active, %s tombstone)' \
    "$(awk -F'\t' '$2=="active"' "$TMPROOT/dump" | wc -l | tr -d ' ')" \
    "$(awk -F'\t' '$2=="tombstone"' "$TMPROOT/dump" | wc -l | tr -d ' ')"
}
live "inventory" inventory_ok check_compiler_inventory "$REGISTRY"

live "pin grammar" "every refusal row pins an active code, exactly ($(code_pinned_rows "$MANIFEST") row(s) pinned by code)" \
  check_manifest_pins "$REGISTRY" "$MANIFEST"

live "every active code pinned" "every active code is pinned by at least one refusal row" \
  check_codes_pinned "$REGISTRY" "$MANIFEST"

live "R5 membership" "the set of refusal rows is the pinned one (R5 digest ${REFUSAL_SET_SHA:0:12}… over path, class, stage)" \
  check_membership "$MANIFEST"

live "refusal-set size" "the manifest's code-pinned refusal rows are the committed $REFUSAL_SET_SIZE" \
  check_refusal_size "$MANIFEST"

live "map = manifest" "every refusal row's pin is exactly the one $PIN_MAP dictates, and the two name the same rows" \
  check_map_pins "$PIN_MAP" "$MANIFEST"

live "pin grammar copies" "the pin grammar is one literal: $GENERATOR PIN_GRAMMAR = scripts/conformance.sh PIN_RE = this gate's PIN_RE" \
  check_pin_grammar_copies "$GENERATOR" scripts/conformance.sh

# ---------------------------------------------------------------------------
# 4 — first-witness emission, against THIS binary
# ---------------------------------------------------------------------------

# A REFUSAL WITNESS HAS TO REFUSE, AND THE EXIT STATUS IS THE ONLY THING THAT
# SAYS SO. This loop used to run pdc in a subshell and DROP its status, then read
# the stderr capture as if the capture alone settled the question. Two ways that
# fails open, and neither is hypothetical: a witness fixture that becomes
# ACCEPTED (the corpus calls that REJECT_ACCEPTED and treats it as a failure) is
# a row whose refusal no longer exists, and a pdc killed by a signal produces a
# short capture that parses to NO_CODE for a reason that has nothing to do with
# codes. Both must be named, not read through.
#
# WHICH NONZERO STATUS COUNTS. `1` is the front-end refusal — the only failure a
# fixture may declare, and the class every code in this registry names. `3/4/5/6`
# are the structured BACKEND/toolchain verdicts (see `report_link` in
# src/main.rs): the front end ACCEPTED and something later failed, so the
# language rule was not enforced and this is not a witness to it. `>= 128` is a
# death by signal. `0` is acceptance.
#
# THREE-VALUED, LIKE THE CORPUS FOLD, AND FOR THE SAME REASON. The branches below
# split into two kinds and only one of them is a finding about diagnostic codes:
#
#   MEASURED (RED)  the witness was ACCEPTED, or exited with a backend verdict, or
#                   refused with the wrong code / no code / two coded headers.
#                   Each is an answer: the registry's claim about this code is false.
#   ABSTENTION      pdc was killed by a signal, exited a status with no defined
#                   meaning, or its stderr capture could not be read. NOTHING was
#                   learned about the code. Spending these as failures is the exact
#                   defect this file fixed one section down in the corpus fold, and
#                   it survived here: three branches printed "no verdict ... was
#                   reached" and then incremented the failure count.
#
# Every line is tagged so the caller can route it without re-deciding:
#   `OK <text>` passed, `RED <text>` measured defect, `NOVERDICT <text>` abstention.
#
# $1 = registry -> 0 all clean, 1 a measured defect, 2 an abstention (and no
# measured defect). Tagged lines on stdout, one per line.
check_first_witness_emission() {
  local reg=$1 n=0 v=0 code name status cond just wit commit rc state
  if ! mkdir -p "$TMPROOT/run"; then
    echo "NOVERDICT could not create a working directory to compile the witnesses in"
    return 2
  fi
  while IFS=$'\t' read -r code name status cond just wit commit; do
    [ "${status:-}" = active ] || continue
    [ -f "$wit" ] || continue

    ( cd "$TMPROOT/run" && "$PDC_ABS" compile "$ROOT/$wit" -o w >/dev/null 2>"$TMPROOT/wit_stderr" )
    rc=$?
    case "$rc" in
      0)   echo "RED $code: $wit was ACCEPTED (exit 0) — a refusal witness that does not refuse witnesses nothing"; n=$((n+1)); continue ;;
      1)   ;;
      3|4|5|6) echo "RED $code: $wit exited $rc, a backend/toolchain verdict — the front end accepted it, so it does not witness a language rule"; n=$((n+1)); continue ;;
      *)   if [ "$rc" -ge 128 ]; then
             echo "NOVERDICT $code: $wit killed by signal $((rc-128)) — no verdict about $code was reached"
           else
             echo "NOVERDICT $code: $wit exited $rc, which is neither a front-end refusal (1) nor a structured backend verdict (3/4/5/6) — this status has no defined meaning, so it says nothing about $code"
           fi
           v=$((v+1)); continue ;;
    esac

    if ! state=$(pd_diag_parse "$TMPROOT/wit_stderr"); then
      echo "NOVERDICT $code: could not read the stderr capture of $wit — the refusal may or may not have carried its code"
      v=$((v+1)); continue
    fi
    case "$(pd_diag_state "$state")" in
      CODED)
        if [ "$(pd_diag_code "$state")" = "$code" ]; then
          echo "OK $code emitted by $wit (refused, exit 1)"
        else
          echo "RED $code: $wit emitted $(pd_diag_code "$state") instead"; n=$((n+1))
        fi ;;
      NO_CODE)   echo "RED $code: $wit refused with no code at all"; n=$((n+1)) ;;
      MALFORMED) echo "RED $code: $wit printed $(pd_diag_code "$state") coded primary headers — cardinality-1 is broken"; n=$((n+1)) ;;
    esac
  done < <(registry_rows "$reg")
  [ "$n" -eq 0 ] || return 1
  [ "$v" -eq 0 ] || return 2
  return 0
}

echo
echo "first-witness emission (real compiles):"
# Through `live` like the other nine: its tagged lines are routed there, and the
# call and the record that it ran are one statement. Its own lines name every
# code, so it carries no summary text.
live "first-witness emission" "" check_first_witness_emission "$REGISTRY"

# ---------------------------------------------------------------------------
# 5 — planted mutants. Temp dir, temp manifests, temp registries. The live
#     corpus is READ and never written.
# ---------------------------------------------------------------------------
echo
echo "planted mutants (parser + registry + manifest):"
# `finish`, not `exit 2`: by here the registry, inventory, pin and witness checks
# have all run and may be holding a measured RED.
M="$TMPROOT/mutants"
mkdir -p "$M" || { absta "could not create a directory to plant mutants in"; finish; }

expect_state() {             # name, capture-file, expected state, [expected code]
  local name=$1 cap=$2 want=$3 wantcode=${4:-}
  local st
  if ! st=$(pd_diag_parse "$cap"); then bad "$name: parser could not read its capture"; return; fi
  local got; got=$(pd_diag_state "$st")
  if [ "$got" != "$want" ]; then bad "$name: parser said $got, expected $want"; return; fi
  if [ -n "$wantcode" ] && [ "$(pd_diag_code "$st")" != "$wantcode" ]; then
    bad "$name: parser read code $(pd_diag_code "$st"), expected $wantcode"; return
  fi
  ok "$name"
}

# M0 — HARNESS NO-OP META-CONTROL. A real, unmutated refusal must come back
# CODED with its own code. Without this, a harness that failed everything would
# look like a harness that caught everything.
( cd "$TMPROOT/run" && "$OLDPWD/$PDC" compile "$OLDPWD/tests/reject/bool_does_not_cast_to_char.pd" -o m0 >/dev/null 2>"$M/m0" )
expect_state "M0 meta-control: an unmutated coded refusal parses as itself" "$M/m0" CODED PD0003

# M1 — WRONG CODE. The parser reports what was printed; the comparison against
# the expected code is the caller's, and it must be able to fail.
sed 's/PD0003/PD0009/' "$M/m0" >"$M/m1"
if st=$(pd_diag_parse "$M/m1") && [ "$(pd_diag_code "$st")" = PD0009 ] && [ "$(pd_diag_code "$st")" != PD0003 ]; then
  ok "M1 wrong code: PD0009 is read as PD0009 and does not satisfy PD0003"
else
  bad "M1 wrong code: a swapped code was not distinguished"
fi

# M2 — THE CODE TEXT PLANTED IN THE SOURCE. This is the F12 shape itself: a
# fixture that satisfies its own pin by containing it. A REAL compile, and the
# fixture's text reaches the capture FOUR times — inside the primary message,
# in the echoed source line (`5 | `), in the `= help:` line and in the suggested
# fix — at four different columns, none of them 0. The col-0 anchor is what
# refuses all four, and the mutant is only informative if the text really
# arrives, so that is asserted before the state is.
cat >"$M/planted.pd" <<'PD'
fn main() {
    let s: String = "x";
    print(s);
}
fn "error[PD0003]: forged"() {}
PD
( cd "$TMPROOT/run" && "$OLDPWD/$PDC" compile "$M/planted.pd" -o m2 >/dev/null 2>"$M/m2" )
planted_hits=$(sed $'s/\033\\[[0-9;]*m//g' "$M/m2" | grep -c 'error\[PD0003\]' || true)
if [ "$planted_hits" -ge 2 ]; then
  expect_state "M2 planted code text (x$planted_hits in the capture) is invisible to the parser" "$M/m2" NO_CODE
else
  bad "M2 planted code: the fixture's text reached the capture $planted_hits time(s), so this mutant proved nothing"
fi

# M3 — UNCODED. A refusal from a rule nothing has judged says so; NO_CODE is a
# state, never a silent pass.
#
# THE CONTROL RAN OUT OF FIXTURES, and that is the slices finishing rather than
# the control rotting. It was `ref_parameter.pd` until su2b coded that refusal
# PD0030, then `at_binding_shadows_item.pd` until su3 coded it PD0004, then
# `mut_borrow_of_immutable.pd` until su4 coded it PD0012 — and su4 is the LAST
# emission slice, so the corpus no longer holds an uncoded reject row to point
# at. Rather than keep a pointer there is nothing to point at, the control now
# compiles its own subject: `scripts/lib/unjudged-refusal.pd`, whose refusal is
# the ASSIGNMENT arm of the type checker's `type_mismatch` helper — a rule the
# locked 72-condition map allocates no number to, and one that
# `tests/gi12_diagnostic_codes.rs` independently asserts stays uncoded in
# `the_assignment_arm_sharing_the_type_mismatch_helper_stays_uncoded`. ONE FILE,
# read by both: this gate and the Rust test used to carry a literal each, and two
# literals are two programs the day one of them is edited. A future slice that
# codes that arm cannot quietly turn this control into a tautology, because it
# goes red here AND there, and moving the program is one edit.
#
# A REAL COMPILE, not a synthesised capture. The state under test is a COMPILER
# state — that a refusal from an unjudged site carries no code — and a capture
# this gate wrote would only test the parser against this gate's own text, which
# M5 already does. The two premises are therefore asserted before the state is:
# the program must REFUSE (an accepted program prints nothing to stderr, and an
# empty capture also parses as NO_CODE, so the guard is what keeps this control
# from passing by having nothing to read), and it must refuse with the sentence
# the arm above is named for.
UNCODED_SUBJECT=scripts/lib/unjudged-refusal.pd
( cd "$TMPROOT/run" && "$OLDPWD/$PDC" compile "$OLDPWD/$UNCODED_SUBJECT" -o m3 >/dev/null 2>"$M/m3" )
m3_rc=$?
m3_plain=$(sed $'s/\033\\[[0-9;]*m//g' "$M/m3" | head -1)
if [ ! -f "$UNCODED_SUBJECT" ]; then
  bad "M3 uncoded control: its subject $UNCODED_SUBJECT is missing, so it says nothing about NO_CODE"
elif [ "$m3_rc" -ne 1 ]; then
  bad "M3 uncoded control: the subject program exited $m3_rc instead of refusing (1), so it says nothing about NO_CODE"
elif ! printf '%s' "$m3_plain" | grep -q 'Type mismatch: expected Int, found Char'; then
  bad "M3 uncoded control: the program reached a different refusal ($m3_plain), so this control is no longer about the assignment arm"
else
  expect_state "M3 an unjudged refusal reports NO_CODE" "$M/m3" NO_CODE
fi

# M4 — TWO CODED PRIMARY HEADERS. The state the choke-point refactor made
# unreachable; the parser must still name it rather than pick one.
cat "$M/m0" "$M/m0" >"$M/m4"
expect_state "M4 two coded primary headers are MALFORMED, not the first one" "$M/m4" MALFORMED

# M5 — BARE ERRORS ARE NOT MALFORMED (R1). `pdc` with no command, a link
# verdict: legitimately uncoded, and any number of them.
printf 'error: no command given\nerror: something else\n' >"$M/m5"
expect_state "M5 two bare error: lines are NO_CODE, not MALFORMED" "$M/m5" NO_CODE

# M6 — R6's STREAM CONTROL. A col-0 `error[`-shaped line on STDOUT must not
# satisfy the parser. Synthesised deliberately: no pdc stdout line can begin
# with `error[` (the compile-path prints are `Compiling …`, `🔨 …`, `   Found …`,
# `✅ …`), so the only way to plant this hazard is to write the stream by hand —
# and the check that matters is that the parser reads the stderr capture it is
# given rather than a merged stream.
printf 'Compiling x.pd...\nerror[PD9999]: forged on stdout\n' >"$M/m6_stdout"
cp "$M/m3" "$M/m6_stderr"
cat "$M/m6_stdout" "$M/m6_stderr" >"$M/m6_merged"
expect_state "M6 stdout-borne error[ ] does not reach the stderr parse" "$M/m6_stderr" NO_CODE
if st=$(pd_diag_parse "$M/m6_merged") && [ "$(pd_diag_state "$st")" = CODED ]; then
  ok "M6 control: the MERGED stream would have been fooled — the split is load-bearing"
else
  bad "M6 control: the merged stream was not fooled, so this mutant proves nothing about the split"
fi

# M7..M10 — registry and manifest mutants, on COPIES.
# awk and not sed: BSD sed does not read `\t` as a tab, and a mutant that
# silently fails to mutate is a mutant that reports the harness as healthy.
mutate_registry() {          # name, awk-body-on-the-matching-row, expected complaint
  local name=$1 body=$2 want=$3
  awk -F'\t' -v OFS='\t' "$body" "$REGISTRY" >"$M/reg.tsv"
  if cmp -s "$REGISTRY" "$M/reg.tsv"; then
    bad "$name: the mutation changed nothing, so this mutant proves nothing"; return
  fi
  local out; out=$(check_registry "$M/reg.tsv"); local rc=$?
  if [ "$rc" -gt 0 ] && printf '%s' "$out" | grep -q -- "$want"; then
    ok "$name"
  else
    bad "$name: expected a complaint containing '$want', got rc=$rc: $(printf '%s' "$out" | head -1)"
  fi
}
mutate_registry "M7 a duplicated code row is refused" \
  '$1=="PD0003"{$1="PD0002"} {print}' "duplicate code row"
mutate_registry "M8 an active row with no witness is refused" \
  '$1=="PD0002"{$6="-"} {print}' "no first_witness"
# M9 is an INVENTORY mutant, not a registry-shape one: reviving PD0025 in the
# TSV makes the TSV internally consistent, and only the binary's own tombstone
# list contradicts it.
awk -F'\t' -v OFS='\t' \
  '$1=="PD0025"{$3="active"; $6="tests/reject/const_divide_by_zero.pd"} {print}' \
  "$REGISTRY" >"$M/reg.tsv"
if cmp -s "$REGISTRY" "$M/reg.tsv"; then
  bad "M9: the mutation changed nothing, so this mutant proves nothing"
else
  out=$(check_compiler_inventory "$M/reg.tsv")
  if printf '%s' "$out" | grep -q "may not be revived"; then
    ok "M9 reviving a tombstoned number is refused by the binary's own list"
  else
    bad "M9 a revived tombstone was accepted: ${out:-<nothing>}"
  fi
fi

# The manifest mutants need a temp manifest, never the live one. `NF==6` and not
# `head`: the first non-comment line of the real manifest is BLANK, and a mutant
# planted on a blank line mutates nothing while looking like it did.
mkman() { awk -F'\t' 'NF==6 && $1 !~ /^#/ && ($2=="reject" || $2=="skip")' "$MANIFEST" | head -3 >"$M/man.txt"; }
mkman
[ "$(wc -l <"$M/man.txt" | tr -d ' ')" -eq 3 ] \
  || { absta "could not slice 3 manifest rows to mutate"; finish; }

mutate_manifest() {          # name, replacement observable, expected complaint
  local name=$1 obs=$2 want=$3
  awk -F'\t' -v OFS='\t' -v o="$obs" 'NR==1{$4=o} {print}' "$M/man.txt" >"$M/man_mut.txt"
  if cmp -s "$M/man.txt" "$M/man_mut.txt"; then
    bad "$name: the mutation changed nothing, so this mutant proves nothing"; return
  fi
  local out; out=$(check_manifest_pins "$REGISTRY" "$M/man_mut.txt")
  if printf '%s' "$out" | grep -q -- "$want"; then
    ok "$name"
  else
    bad "$name: expected a complaint containing '$want', got: ${out:-<nothing>}"
  fi
}
mutate_manifest "M10 a manifest row pinning a tombstone is refused" \
  "code=PD0025" "which is a tombstone"
mutate_manifest "M11 a malformed code= pin is refused" \
  "code=PD3" "is not exactly code=PD"
mutate_manifest "M12 a pin to an unregistered code is refused" \
  "code=PD0777" "not in the registry"
mutate_manifest "M12b a whitespace variant of a well-formed pin is refused" \
  "code= PD0003" "is not exactly code=PD"
# M12c — THE HOLE THE CUTOVER CLOSED IN THIS FUNCTION. It used to read only
# observables already spelled `code=…`, so a refusal row that kept a PHRASE was
# never looked at. Now every refusal row must pin a code.
mutate_manifest "M12c a refusal row that still pins a PHRASE is refused" \
  "No main function found" "is not exactly code=PD"
mutate_manifest "M12d a bare PD#### with no code= is refused" \
  "PD0003" "is not exactly code=PD"

# M22 — EVERY ACTIVE CODE IS PINNED. A registry with one more active row than any
# manifest row pins must go red; the live pair must not.
awk -F'\t' -v OFS='\t' '{print} $1=="PD0003"{$1="PD0998"; $2="planted_unpinned_rule"; print}' \
  "$REGISTRY" >"$M/reg.tsv"
if cmp -s "$REGISTRY" "$M/reg.tsv"; then
  bad "M22: the mutation changed nothing, so this mutant proves nothing"
else
  out=$(check_codes_pinned "$M/reg.tsv" "$MANIFEST")
  if printf '%s' "$out" | grep -q "PD0998: active in the registry, and no reject or skip row pins it"; then
    ok "M22 an active code that no refusal row pins is refused"
  else
    bad "M22 an unpinned active code was accepted: ${out:-<nothing>}"
  fi
fi

# M23 — R5, THE MEMBERSHIP PIN, over temp copies of the WHOLE live manifest. The
# compensated retype is the case a count cannot see, so the mutant first proves
# the counts really are unchanged — otherwise it would only be another single
# retype wearing a better name.
membership_case() {          # name, awk program over the manifest, [require-same-counts]
  local name=$1 prog=$2 same=${3:-}
  awk -F'\t' -v OFS='\t' "$prog" "$MANIFEST" >"$M/man_r5.txt"
  if cmp -s "$MANIFEST" "$M/man_r5.txt"; then
    bad "$name: the mutation changed nothing, so this mutant proves nothing"; return
  fi
  if [ -n "$same" ]; then
    local before after
    before=$(awk -F'\t' 'NF==6 && $1 !~ /^#/ {print $2}' "$MANIFEST" | sort | uniq -c | tr -s ' ')
    after=$(awk -F'\t' 'NF==6 && $1 !~ /^#/ {print $2}' "$M/man_r5.txt" | sort | uniq -c | tr -s ' ')
    if [ "$before" != "$after" ]; then
      bad "$name: the class counts moved, so this is not a COMPENSATED retype"; return
    fi
  fi
  local out; out=$(check_membership "$M/man_r5.txt")
  if [ $? -ne 0 ] && printf '%s' "$out" | grep -q "the set of refusal rows changed"; then
    ok "$name"
  else
    bad "$name: the membership pin accepted it: ${out:-<nothing>}"
  fi
}
r5_reject=$(awk -F'\t' 'NF==6 && $1 !~ /^#/ && $2=="reject"{print $1; exit}' "$MANIFEST")
r5_xfail=$(awk -F'\t' 'NF==6 && $1 !~ /^#/ && $2=="xfail"{print $1; exit}' "$MANIFEST")
r5_skip=$(awk -F'\t' 'NF==6 && $1 !~ /^#/ && $2=="skip"{print $1; exit}' "$MANIFEST")
if [ -z "$r5_reject" ] || [ -z "$r5_xfail" ] || [ -z "$r5_skip" ]; then
  bad "M23: the live manifest has no reject, xfail or skip row to retype; these mutants prove nothing"
else
  membership_case "M23a a single retype (reject -> xfail) changes the refusal set" \
    "\$1==\"$r5_reject\"{\$2=\"xfail\"} {print}"
  membership_case "M23b a COMPENSATED retype (reject A -> xfail, xfail B -> reject) keeps every count and is still refused" \
    "\$1==\"$r5_reject\"{\$2=\"xfail\"} \$1==\"$r5_xfail\"{\$2=\"reject\"} {print}" same
  membership_case "M23c a reject <-> skip swap keeps both counts and is still refused" \
    "\$1==\"$r5_reject\"{\$2=\"skip\"} \$1==\"$r5_skip\"{\$2=\"reject\"} {print}" same
  # The STAGE retype: path and class unchanged, so a `path<TAB>class` digest could
  # not see it — and the row has left the code-pinned set for an `exit=<N>` rule.
  membership_case "M23e a stage retype (reject|compile -> reject|run, exit=1) keeps path and class and is still refused" \
    "\$1==\"$r5_reject\"{\$3=\"run\"; \$4=\"exit=1\"} {print}" same
  out=$(check_membership "$MANIFEST") \
    && ok "M23d meta-control: the unmutated live manifest passes the same function" \
    || bad "M23d meta-control: the unmutated manifest failed the membership pin — M23a-c and M23e are uninformative: $out"
fi

# M24 — THE COUNTS REQUIREMENT, over lines this run did not produce. Each line is
# derived from the committed size and is wrong in EXACTLY ONE field, so each case
# is RED for its own clause alone: with `coded=` one short as well (as M24d and
# M24e once were), deleting the clause under test would have left the case red
# through `coded != want`, and the mutant would have proved nothing about it.
counts_case() {              # name, line, want, expected rc
  local out; out=$(check_code_counts "$2" "$3"); local rc=$?
  if [ "$rc" = "$4" ]; then ok "$1"; else bad "$1: rc=$rc, expected $4: ${out:-<nothing>}"; fi
}
N=$REFUSAL_SET_SIZE
counts_case "M24a every refusal row coded, nothing else, is green" \
  "diagnostic-codes: coded=$N uncoded=0 malformed=0 unreadable=0" "$N" 0
counts_case "M24b one row short of the committed size is RED" \
  "diagnostic-codes: coded=$((N-1)) uncoded=0 malformed=0 unreadable=0" "$N" 1
counts_case "M24c an uncoded refusal is RED even if the coded count is reached" \
  "diagnostic-codes: coded=$N uncoded=1 malformed=0 unreadable=0" "$N" 1
counts_case "M24d a malformed refusal is RED even if the coded count is reached" \
  "diagnostic-codes: coded=$N uncoded=0 malformed=1 unreadable=0" "$N" 1
counts_case "M24e an unreadable capture is RED even if the coded count is reached" \
  "diagnostic-codes: coded=$N uncoded=0 malformed=0 unreadable=1" "$N" 1
counts_case "M24f no counts line at all is RED, not skipped" "" "$N" 1

# M26 — THE SIZE IS A COMMITTED NUMBER (B1). A manifest with one refusal row
# deleted lowers the live count, and must not lower the requirement with it.
awk -F'\t' -v p="$r5_reject" '$1!=p' "$MANIFEST" >"$M/man_short.txt"
if cmp -s "$MANIFEST" "$M/man_short.txt"; then
  bad "M26: the deletion changed nothing, so this mutant proves nothing"
else
  out=$(check_refusal_size "$M/man_short.txt")
  if [ $? -ne 0 ] && printf '%s' "$out" | grep -q "the committed refusal-set size is $REFUSAL_SET_SIZE"; then
    ok "M26 a manifest one refusal row short is RED against the committed size ($(code_pinned_rows "$M/man_short.txt") vs $REFUSAL_SET_SIZE)"
  else
    bad "M26 a short manifest passed the size check: ${out:-<nothing>}"
  fi
fi
out=$(check_refusal_size "$MANIFEST") \
  && ok "M26b meta-control: the live manifest has the committed number of code-pinned refusal rows" \
  || bad "M26b meta-control: the live manifest failed the size check — M26 is uninformative: $out"

# M25 — THE GENERATOR FAILS CLOSED (A1). `--self-test` decides planted inputs with
# the generator's own `decide()`, compiling nothing: a two-group code pinned bare
# must be REFUSED, naming the group, and its paired controls must be written.
gen_out=$(python3 "$GENERATOR" --self-test 2>&1); gen_rc=$?
case "$gen_rc" in
  0) if printf '%s\n' "$gen_out" | grep -q 'ok   G1 a two-group code with a bare pin is REFUSED'; then
       ok "M25 the generator refuses a two-group code pinned bare ($(printf '%s\n' "$gen_out" | tail -1))"
     else
       bad "M25 the generator self-test exited 0 without deciding G1: $(printf '%s' "$gen_out" | tail -3)"
     fi ;;
  1) bad "M25 the generator decided a planted case wrongly: $(printf '%s\n' "$gen_out" | grep 'FAIL' | head -3)" ;;
  *) absta "M25 the generator self-test could not run (exit $gen_rc): $(printf '%s' "$gen_out" | tail -2)" ;;
esac

# M27 — ONE PIN GRAMMAR (B6). Each copy drifts once, on a temp copy of its file;
# the check must name the file that drifted. A copy whose defining line vanishes
# is a complaint too, not a skip.
grammar_case() {             # name, source file, sed program, which arg (gen|conf), expected
  local name=$1 src=$2 prog=$3 which=$4 want=$5 out
  sed "$prog" "$src" >"$M/grammar_copy"
  if cmp -s "$src" "$M/grammar_copy"; then
    bad "$name: the drift changed nothing, so this mutant proves nothing"; return
  fi
  if [ "$which" = gen ]; then out=$(check_pin_grammar_copies "$M/grammar_copy" scripts/conformance.sh)
  else out=$(check_pin_grammar_copies "$GENERATOR" "$M/grammar_copy"); fi
  if [ $? -ne 0 ] && printf '%s' "$out" | grep -q -- "$want"; then ok "$name"
  else bad "$name: expected a complaint containing '$want', got: ${out:-<nothing>}"; fi
}
grammar_case "M27a the generator's PIN_GRAMMAR drifting ({4} -> {3}) is RED" \
  "$GENERATOR" '/^PIN_GRAMMAR = /s/{4}/{3}/' gen "PIN_GRAMMAR is '^code=PD\[0-9\]{3}"
grammar_case "M27b conformance.sh's PIN_RE drifting (.+ -> .*) is RED" \
  scripts/conformance.sh "/^PIN_RE=/s/\.+/.*/" conf "PIN_RE is '^code=PD\[0-9\]{4}(;msg~\.\*)"
grammar_case "M27c a PIN_RE definition that is no longer one line is RED, not skipped" \
  scripts/conformance.sh "/^PIN_RE=/s/^/# /" conf "0 \`PIN_RE="
out=$(check_pin_grammar_copies "$GENERATOR" scripts/conformance.sh) \
  && ok "M27d meta-control: the live copies pass the same function" \
  || bad "M27d meta-control: the live copies failed — M27a-c are uninformative: $out"

# M28 — THE MAP HOLDS THE MANIFEST (B7). A pin edited by hand — here the A1 defect
# itself, a frobnicate row put back to bare `code=PD0006` — and a map that lost a
# row must each be RED; the live pair must pass the same function.
awk -F'\t' -v OFS='\t' '$1=="tests/reject/unknown_attribute.pd"{$4="code=PD0006"} {print}' \
  "$MANIFEST" >"$M/man_hand.txt"
if cmp -s "$MANIFEST" "$M/man_hand.txt"; then
  bad "M28a: the hand edit changed nothing, so this mutant proves nothing"
else
  out=$(check_map_pins "$PIN_MAP" "$M/man_hand.txt")
  if [ $? -ne 0 ] && printf '%s' "$out" | grep -q "tests/reject/unknown_attribute.pd: the manifest pins code=PD0006 and the map dictates code=PD0006;msg~"; then
    ok "M28a a manifest pin edited by hand (unknown_attribute.pd back to bare) is RED against the map"
  else
    bad "M28a a hand-edited pin passed the map check: ${out:-<nothing>}"
  fi
fi
awk -F'\t' '$4!="tests/reject/total_attribute.pd"' "$PIN_MAP" >"$M/map_short.tsv"
if cmp -s "$PIN_MAP" "$M/map_short.tsv"; then
  bad "M28b: the deletion changed nothing, so this mutant proves nothing"
else
  out=$(check_map_pins "$M/map_short.tsv" "$MANIFEST")
  if [ $? -ne 0 ] && printf '%s' "$out" | grep -q "tests/reject/total_attribute.pd: a reject row of the manifest that the map does not name"; then
    ok "M28b a refusal row the map does not name is RED"
  else
    bad "M28b a row missing from the map passed: ${out:-<nothing>}"
  fi
fi
out=$(check_map_pins "$PIN_MAP" "$MANIFEST") \
  && ok "M28c meta-control: the live map and manifest pass the same function" \
  || bad "M28c meta-control: the live pair failed — M28a-b are uninformative: $out"

# M29 — THE WIRING ASSERTION ITSELF. With one required label unrecorded it must
# name that label; with every one recorded it must pass. The live run's own
# assertion is at the end, after the counts requirement has run.
out=$( LIVE_REACHED=("${LIVE_REQUIRED[@]:1}"); check_live_reached ); rc=$?
if [ "$rc" -ne 0 ] && [ "$out" = "${LIVE_REQUIRED[0]}" ]; then
  ok "M29a a live check that never ran is named (${LIVE_REQUIRED[0]})"
else
  bad "M29a an unrecorded live check was not named: rc=$rc, said '${out:-<nothing>}'"
fi
out=$( LIVE_REACHED=("${LIVE_REQUIRED[@]}"); check_live_reached ) \
  && ok "M29b paired control: every label recorded passes" \
  || bad "M29b paired control: a complete record was refused: $out"

# M30 — `live` ITSELF, three-valued. Each case drives `live` in a subshell with
# fresh counters and a stub check, and reads back what it SPENT: failures,
# abstentions, and whether the label was recorded. Before this, `live` printed
# every nonzero return as RED, so a check that could not look was a finding.
live_stub() {                # rc, then the lines to print
  local rc=$1; shift
  [ "$#" -gt 0 ] && printf '%s\n' "$@"
  return "$rc"
}
live_spent() {               # live's arguments -> "<fails> <abstained> recorded|unrecorded"
  ( fails=0; abstained=0; LIVE_REACHED=()
    live "$@" >/dev/null
    if [ "${#LIVE_REACHED[@]}" -eq 1 ] && [ "${LIVE_REACHED[0]}" = "$1" ]; then r=recorded; else r=unrecorded; fi
    printf '%s %s %s\n' "$fails" "$abstained" "$r" )
}
live_case() {                # name, expected spend, live's arguments...
  local name=$1 want=$2 got; shift 2
  got=$(live_spent "$@")
  if [ "$got" = "$want" ]; then ok "$name"; else bad "$name: spent '$got', expected '$want'"; fi
}
live_case "M30a a check that returns 2 is a NO VERDICT, not a RED, and is recorded" \
  "0 1 recorded" probe "fine" live_stub 2 "could not read the file"
live_case "M30b a check that returns 2 having said nothing is still a NO VERDICT" \
  "0 1 recorded" probe "fine" live_stub 2
live_case "M30c a check that returns 1 is a RED" \
  "1 0 recorded" probe "fine" live_stub 1 "a defect"
live_case "M30d a check that returns 1 having named only an abstention is still a RED" \
  "1 1 recorded" probe "fine" live_stub 1 "NOVERDICT one capture was unreadable"
live_case "M30e tagged lines are routed by their tag, whatever the return code" \
  "1 1 recorded" probe "" live_stub 1 "OK PD0001 emitted" "RED PD0002 not emitted" "NOVERDICT PD0003 unreadable"
live_case "M30f a status that is none of 0, 1 and 2 is a NO VERDICT, not a pass" \
  "0 1 recorded" probe "fine" live_stub 7 "something"
live_case "M30g paired control: a check that returns 0 spends nothing and is recorded" \
  "0 0 recorded" probe "fine" live_stub 0 "an untagged line from a clean check"

# M31 — THE INVENTORY, through `live`, with a stub binary in place of pdc. The
# subshell carries its own PDC and its own TMPROOT, so the live run's dump is
# not overwritten. A dump that FAILS is a NO VERDICT; the paired controls are a
# dump that DISAGREES with the registry and one that lists nothing, both RED —
# without them M31a could pass because the stub seam made everything abstain.
inventory_spent() {          # stub pdc -> what `live` spent on the inventory check
  ( PDC=$1; TMPROOT="$M/inv"; mkdir -p "$TMPROOT" || exit 3
    live_spent "inventory" "" check_compiler_inventory "$REGISTRY" )
}
inventory_case() {           # name, stub pdc, expected spend
  local got; got=$(inventory_spent "$2")
  if [ "$got" = "$3" ]; then ok "$1"; else bad "$1: spent '${got:-<nothing>}', expected '$3'"; fi
}
cat >"$M/pdc-nodump" <<'STUB'
#!/bin/sh
exit 1
STUB
cat >"$M/pdc-unknowncode" <<'STUB'
#!/bin/sh
[ "$1" = --dump-diagnostic-codes ] || exit 1
printf 'PD9999\tactive\tnot_in_the_registry\n'
STUB
cat >"$M/pdc-emptydump" <<'STUB'
#!/bin/sh
[ "$1" = --dump-diagnostic-codes ] || exit 1
exit 0
STUB
chmod +x "$M/pdc-nodump" "$M/pdc-unknowncode" "$M/pdc-emptydump"
inventory_case "M31a a binary whose dump FAILS is a NO VERDICT for the inventory, not a RED" \
  "$M/pdc-nodump" "0 1 recorded"
inventory_case "M31b paired control: a dump that names a code the registry lacks is still a RED" \
  "$M/pdc-unknowncode" "1 0 recorded"
inventory_case "M31c paired control: a dump that succeeds and lists nothing is still a RED" \
  "$M/pdc-emptydump" "1 0 recorded"

# M15 — A WITNESS THAT DOES NOT REFUSE. The registry row is re-pointed at a
# fixture pdc ACCEPTS (a `run`-class corpus row), which is the shape a witness
# takes when the rule it names stops being enforced. Before the exit status was
# read, this arrived as a stderr capture and was judged on its contents alone.
# `NF==6 && $1 !~ /^#/`, for the reason `mkman` needs it too: the manifest's
# preamble documents its own columns, so a bare `$2=="run"` matches the COMMENT
# describing the `run` class and hands back a fragment of prose as a fixture path.
accept_witness=$(awk -F'\t' 'NF==6 && $1 !~ /^#/ && $2=="run"{print $1; exit}' "$MANIFEST")
if [ -z "$accept_witness" ] || [ ! -f "$accept_witness" ]; then
  bad "M15: no accepted fixture available to re-point a witness at; this mutant proves nothing"
else
  awk -F'\t' -v OFS='\t' -v w="$accept_witness" \
    '$1=="PD0003"{$6=w} {print}' "$REGISTRY" >"$M/reg.tsv"
  if cmp -s "$REGISTRY" "$M/reg.tsv"; then
    bad "M15: the mutation changed nothing, so this mutant proves nothing"
  else
    out=$(check_first_witness_emission "$M/reg.tsv")
    if printf '%s' "$out" | grep -q "was ACCEPTED (exit 0)"; then
      ok "M15 a witness that does not refuse is refused ($accept_witness)"
    else
      bad "M15 an accepting witness was taken as a refusal: ${out:-<nothing>}"
    fi
  fi
fi

# M16 — PROVENANCE THAT IS NOT THIS HISTORY. A real commit object that is not an
# ancestor of HEAD: every abandoned branch in the repository is one, which is
# exactly why `git cat-file -e` was the wrong predicate.
#
# The hash is DERIVED AT RUN TIME, never hard-coded — a literal would rot the
# moment the branch it names is deleted, and a mutant that silently stops
# mutating is worse than one that is absent. If a repository has no commit
# outside HEAD's history the control cannot be run, and it says so and is
# COUNTED rather than skipped, which is the seam convention this repo already
# uses (`gate-receipts` prints `gate=N, NONE validated`; conformance prints
# `N probe group(s) pinned as uncovered`).
foreign=$(git rev-list --all --not HEAD 2>/dev/null | head -1)
if [ -z "$foreign" ]; then
  note "M16 NOT RUN (counted, not skipped): this repository has no commit outside HEAD's history to plant as false provenance"
  unrun=$((unrun+1))
else
  awk -F'\t' -v OFS='\t' -v c="$foreign" '$1=="PD0003"{$7=c} {print}' "$REGISTRY" >"$M/reg.tsv"
  if cmp -s "$REGISTRY" "$M/reg.tsv"; then
    bad "M16: the mutation changed nothing, so this mutant proves nothing"
  else
    out=$(check_registry "$M/reg.tsv")
    if printf '%s' "$out" | grep -q "NOT an ancestor of HEAD"; then
      ok "M16 a real commit outside this history is refused as provenance (${foreign:0:7})"
    else
      bad "M16 a non-ancestor commit was accepted as provenance: ${out:-<nothing>}"
    fi
  fi
  # The paired control: the SAME predicate must still accept a real ancestor, or
  # M16 could be passing because the check refuses every hash.
  awk -F'\t' -v OFS='\t' -v c="$(git rev-parse HEAD)" '$1=="PD0003"{$7=c} {print}' "$REGISTRY" >"$M/reg.tsv"
  out=$(check_registry "$M/reg.tsv")
  if printf '%s' "$out" | grep -q "ancestor"; then
    bad "M16b control: HEAD itself was refused as provenance, so M16 proves nothing: $out"
  else
    ok "M16b control: an ancestor of HEAD is accepted as provenance"
  fi
fi

# M18/M19 — THE WITNESS EXECUTION PATH'S OWN THREE-VALUED CONTRACT.
#
# M17 below interrogates the CORPUS fold and would have found neither of these:
# they live in `check_first_witness_emission`, which had the same collapse and
# kept it after the fold was fixed. A gate that spends an abstention as a failure
# reports a defect it did not find, and the next person to see this red spends a
# day looking for a broken code that is not broken.
witness_expect() {           # name, expected line tag, expected substring
  local name=$1 tag=$2 want=$3 out=$4 rc=$5
  if ! printf '%s' "$out" | grep -q "^$tag .*$want"; then
    bad "$name: expected a '$tag' line matching '$want', got: ${out:-<nothing>}"; return
  fi
  # The RETURN CODE is half the contract: a caller routes on it, and a function
  # that printed NOVERDICT and returned 1 would still be spent as a failure.
  local wantrc=1; [ "$tag" = NOVERDICT ] && wantrc=2
  if [ "$rc" != "$wantrc" ]; then
    bad "$name: line was tagged $tag but the function returned $rc, not $wantrc"; return
  fi
  ok "$name"
}

# M18 — a witness compiler KILLED BY A SIGNAL. A stub that kills itself with
# SIGKILL is the smallest faithful reproduction: pdc dies, the capture is short
# and would parse as NO_CODE, and nothing whatever was learned about the code.
cat >"$M/pdc-signal" <<'STUB'
#!/bin/sh
kill -9 $$
STUB
chmod +x "$M/pdc-signal"
out=$( PDC_ABS="$M/pdc-signal"; check_first_witness_emission "$REGISTRY" ); rc=$?
witness_expect "M18 a witness killed by a signal is an abstention, not a failure" \
  NOVERDICT "killed by signal 9" "$out" "$rc"

# M18b — a status with NO DEFINED MEANING. Not 0, not 1, not 3/4/5/6: the gate
# cannot say whether the rule was enforced, so it must not say either.
cat >"$M/pdc-weird" <<'STUB'
#!/bin/sh
exit 42
STUB
chmod +x "$M/pdc-weird"
out=$( PDC_ABS="$M/pdc-weird"; check_first_witness_emission "$REGISTRY" ); rc=$?
witness_expect "M18b an undefined exit status is an abstention, not a failure" \
  NOVERDICT "exited 42" "$out" "$rc"

# M18c — THE PAIRED CONTROL, or M18/M18b could be passing because the stub seam
# turns everything into an abstention. A stub that REFUSES like the front end and
# prints the wrong code must still be a measured RED.
cat >"$M/pdc-wrongcode" <<'STUB'
#!/bin/sh
echo "error[PD0009]: a refusal wearing the wrong code" >&2
exit 1
STUB
chmod +x "$M/pdc-wrongcode"
out=$( PDC_ABS="$M/pdc-wrongcode"; check_first_witness_emission "$REGISTRY" ); rc=$?
witness_expect "M18c control: a refusal carrying the wrong code is still a measured RED" \
  RED "emitted PD0009 instead" "$out" "$rc"

# M19 — THE STDERR CAPTURE COULD NOT BE READ. Reproduced through the real path by
# making the capture path a DIRECTORY: the redirect fails, and the parser is then
# handed something it cannot read. `pd_diag_parse` answers 2 for this and always
# has; what was wrong was the caller spending that 2 as a defect.
rm -f "$TMPROOT/wit_stderr"; mkdir -p "$TMPROOT/wit_stderr"
if pd_diag_parse "$TMPROOT/wit_stderr" >/dev/null 2>&1; then
  bad "M19: the parser read a directory as a capture, so this mutant plants nothing"
fi
out=$(check_first_witness_emission "$REGISTRY"); rc=$?
witness_expect "M19 an unreadable stderr capture is an abstention, not a failure" \
  NOVERDICT "could not read the stderr capture" "$out" "$rc"
rmdir "$TMPROOT/wit_stderr" 2>/dev/null

# M19b — meta-control: with the capture readable again, the live registry is
# green through the same function. Without this, M19 could be passing because the
# function has been left broken.
out=$(check_first_witness_emission "$REGISTRY"); rc=$?
if [ "$rc" -eq 0 ] && ! printf '%s' "$out" | grep -q '^RED \|^NOVERDICT '; then
  ok "M19b meta-control: the unmutated witness set passes the same function"
else
  bad "M19b meta-control: the unmutated witness set failed (rc=$rc) — M18/M19 are uninformative: $out"
fi

# M20 — THE AGGREGATION PRIORITY, as a checked case rather than a comment. A run
# holding both a measured defect and an abstention has one exit code to report
# with, and burying the defect behind the abstention is the failure mode.
exit_case() {                # name, fails, abstentions, expected
  local got; got=$(final_exit_code "$2" "$3")
  if [ "$got" = "$4" ]; then ok "$1"; else bad "$1: said $got, expected $4"; fi
}
exit_case "M20a nothing wrong and nothing unmeasurable is 0" 0 0 0
exit_case "M20b a measured defect alone is 1"                2 0 1
exit_case "M20c an abstention alone is 2, never 0"           0 3 2
exit_case "M20d a defect AND an abstention is 1 — measured RED outranks NO VERDICT" 1 1 1

# M21 — THE WHOLE-GATE CONTROL FLOW, not the arithmetic.
#
# M20 proves `final_exit_code` computes the priority correctly and would have gone
# on proving it forever while the corpus fold's NO_VERDICT arm ran its own
# `exit 2` and never called it. A pure function cannot see a terminal that
# bypasses it. So these cases walk the REAL path — `apply_conformance_verdict`
# into `finish` — in a subshell, with a prior measured RED already planted in the
# counters, and read the exit code the gate would actually have produced.
gate_terminal_case() {       # name, planted fails, planted abstentions, verdict, expected
  local name=$1 f=$2 a=$3 v=$4 want=$5 got
  ( fails=$f; abstained=$a; unrun=0
    apply_conformance_verdict "$v" 2 "verified=85 untranscribed=0 failures=0" /dev/null
    finish ) >/dev/null 2>&1
  got=$?
  if [ "$got" = "$want" ]; then ok "$name"; else bad "$name: the gate exited $got, expected $want"; fi
}
gate_terminal_case "M21a a prior measured RED survives a conformance NO_VERDICT — gate exits 1" \
  1 0 NO_VERDICT 1
gate_terminal_case "M21b paired control: NO_VERDICT with nothing measured wrong still exits 2" \
  0 0 NO_VERDICT 2
gate_terminal_case "M21c paired control: a green corpus with nothing else wrong exits 0" \
  0 0 GREEN 0
gate_terminal_case "M21d paired control: a RED corpus exits 1 through the same terminal" \
  0 0 RED 1
gate_terminal_case "M21e a prior RED and a prior abstention and a NO_VERDICT corpus still exits 1" \
  2 3 NO_VERDICT 1

# M17 — THE THREE-VALUED FOLD, over statuses this run did not produce. The
# decision is a pure function precisely so it can be interrogated here; the live
# call below feeds it the real corpus status.
fold_case() {                # name, rc, summary, expected verdict
  local got; got=$(fold_conformance_verdict "$2" "$3")
  if [ "$got" = "$4" ]; then ok "$1"; else bad "$1: fold said $got, expected $4"; fi
}
fold_case "M17a conformance exit 0 with a summary folds GREEN" \
  0 "verified=85 failures=0" GREEN
fold_case "M17b conformance exit 1 with a summary folds RED" \
  1 "verified=85 failures=1" RED
fold_case "M17c conformance exit 2 WITH a summary folds NO_VERDICT, not RED" \
  2 "verified=85 failures=0" NO_VERDICT
fold_case "M17d conformance with no summary folds NO_VERDICT whatever the status" \
  0 "" NO_VERDICT
fold_case "M17e a signalled conformance folds NO_VERDICT" \
  139 "verified=85 failures=0" NO_VERDICT
# META-CONTROL for the mutant harness itself: the UNMUTATED copies must be green,
# or every "refused" above could be the harness refusing everything.
out=$(check_registry "$REGISTRY"); [ $? -eq 0 ] \
  && ok "M13 meta-control: the unmutated registry passes the same function" \
  || bad "M13 meta-control: the unmutated registry failed — every mutant above is uninformative"
mkman
out=$(check_manifest_pins "$REGISTRY" "$M/man.txt"); [ $? -eq 0 ] \
  && ok "M14 meta-control: the unmutated manifest slice passes the same function" \
  || bad "M14 meta-control: the unmutated manifest slice failed"

# ---------------------------------------------------------------------------
# 6 — R2: the corpus verdict is folded in. Delegation avoids REIMPLEMENTATION,
#     not EXECUTION: this target cannot be green while the corpus is red.
# ---------------------------------------------------------------------------
echo
echo "canonical conformance (R2 — the corpus verdict is part of this one):"
conf_log="$TMPROOT/conformance.log"
bash scripts/conformance.sh tests examples >"$conf_log" 2>&1
conf_rc=$?
summary=$(grep -m1 '^verified=' "$conf_log")
verdict=$(fold_conformance_verdict "$conf_rc" "$summary")
apply_conformance_verdict "$verdict" "$conf_rc" "$summary" "$conf_log"

# The counts line is held to the manifest, not read off. Skipped only when the
# corpus produced no verdict at all — that is already an abstention above, and a
# second complaint about the same missing run would be a double count.
if [ "$verdict" != NO_VERDICT ]; then
  live "counts requirement" "requirement: conformance adjudicated the committed $REFUSAL_SET_SIZE refusal rows by code, and nothing uncoded, malformed or unreadable" \
    check_code_counts "$(grep -m1 '^diagnostic-codes: ' "$conf_log")" "$REFUSAL_SET_SIZE"
else
  LIVE_REACHED+=("counts requirement")    # no corpus verdict: already an abstention
fi

# ---------------------------------------------------------------------------
# 7 — WIRING: every live check above reached a verdict on THIS run.
# ---------------------------------------------------------------------------
echo
echo "wiring (every live check ran):"
out=$(check_live_reached)
if [ $? -eq 0 ]; then
  ok "wiring: all ${#LIVE_REQUIRED[@]} live checks reached a verdict (${LIVE_REQUIRED[*]})"
else
  while IFS= read -r l; do
    bad "wiring: the live check '$l' never ran — its call is gone, and its mutants stay green without it"
  done <<<"$out"
fi

finish
