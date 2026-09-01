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
#      ONE DIRECTION ONLY, and deliberately: the reverse — every registry row is
#      emittable — becomes true at the cutover, when the last emission slice
#      lands. Claiming it now would be a check that passes by being false.
#   3. MANIFEST PIN GRAMMAR. `code=PD####` optionally `;msg~<fragment>`, exact.
#      No bare `PD####`, no spaces, no tombstoned code. VACUOUS TODAY, and it
#      says so: the manifest is still phrase-authority until the cutover, so this
#      check is proven by planted mutants rather than by the live corpus.
#   4. PARSER SELF-TESTS. The shared parser (scripts/lib/diag-parse.sh) is
#      handed planted mutants and must report each one correctly. The mutants run
#      in a temp dir against temp manifests. THE LIVE CORPUS IS NEVER MUTATED —
#      a gate that edits the thing it certifies can leave it edited.
#   5. FIRST-WITNESS EMISSION. Each active code's first_witness is compiled for
#      real, and its stderr must carry that code. This is the only check that
#      proves the registry describes THIS BINARY and not a past one.
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
[ -r "$REGISTRY" ] || { echo "error: registry $REGISTRY not readable" >&2; exit 2; }
[ -r "$MANIFEST" ] || { echo "error: manifest $MANIFEST not readable" >&2; exit 2; }
[ -r scripts/lib/diag-parse.sh ] || { echo "error: shared parser missing" >&2; exit 2; }

. scripts/lib/diag-parse.sh

TMPROOT=$(mktemp -d) || exit 2
trap 'rm -rf "$TMPROOT"' EXIT INT TERM

fails=0
ok()   { printf '  %sok%s   %s\n' "$GREEN" "$NC" "$1"; }
bad()  { printf '  %sRED%s  %s\n' "$RED" "$NC" "$1"; fails=$((fails+1)); }
note() { printf '  %s--%s   %s\n' "$YELLOW" "$NC" "$1"; }

# ---------------------------------------------------------------------------
# The checks, as FUNCTIONS OVER (registry, manifest), so the mutants below can
# re-run exactly the code that certifies the live pair. A self-test that
# exercised a re-implementation would prove nothing about the check that runs.
# Each prints its complaints on stdout and returns the number of them.
# ---------------------------------------------------------------------------

registry_rows() { grep -v '^#' "$1" | tail -n +2; }

check_registry() {           # $1 = registry
  local reg=$1 n=0 line code name status cond just wit commit
  local -a seen_codes=() seen_names=()

  if ! head -50 "$reg" | grep -q '^code	symbolic_name	status	semantic_condition	justification	first_witness	introduced_commit$'; then
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

    # `-` while the row is still in a working tree; otherwise a real commit.
    if [ "$commit" != "-" ] && ! git cat-file -e "$commit^{commit}" 2>/dev/null; then
      echo "$code: introduced_commit '$commit' is not a commit in this repository"; n=$((n+1))
    fi
  done < <(registry_rows "$reg")

  # D7's no-reuse promise is NOT checked here, deliberately. Deriving the
  # tombstone list from the file under test makes the check self-defeating: the
  # mutation that revives PD0025 also removes it from the list the check would
  # read, and the check passes by having nothing to look at. It lives in
  # `check_compiler_inventory`, where the binary's own `TOMBSTONES` is the second
  # authority, and in `src/errors/codes.rs`'s
  # `no_active_code_reuses_a_tombstoned_number`.
  return $n
}

check_compiler_inventory() { # $1 = registry
  local reg=$1 n=0 code status name
  local dump="$TMPROOT/dump"
  if ! "$PDC" --dump-diagnostic-codes >"$dump" 2>/dev/null; then
    echo "pdc --dump-diagnostic-codes did not succeed"; return 1
  fi
  [ -s "$dump" ] || { echo "pdc --dump-diagnostic-codes printed nothing"; return 1; }
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
  return $n
}

check_manifest_pins() {      # $1 = registry, $2 = manifest
  local reg=$1 man=$2 n=0 path cls stage obs rest code
  while IFS=$'\t' read -r path cls stage obs rest; do
    case "$obs" in code=*|*';msg~'*|*'code ='*) ;; *) continue ;; esac
    if [[ ! $obs =~ ^code=PD[0-9]{4}(\;msg~.+)?$ ]]; then
      echo "$path: observable '$obs' is not exactly code=PD####[;msg~<fragment>]"; n=$((n+1)); continue
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
  return $n
}

# ---------------------------------------------------------------------------
# 1..3 — the live pair
# ---------------------------------------------------------------------------
echo "=============================================="
echo "diagnostic codes: registry, inventory, parser"
echo "=============================================="

out=$(check_registry "$REGISTRY"); rc=$?
if [ "$rc" -eq 0 ]; then
  ok "registry coherent ($(registry_rows "$REGISTRY" | wc -l | tr -d ' ') rows: $(awk -F'\t' '$3=="active"' <(registry_rows "$REGISTRY") | wc -l | tr -d ' ') active, $(awk -F'\t' '$3=="tombstone"' <(registry_rows "$REGISTRY") | wc -l | tr -d ' ') tombstone)"
else
  while IFS= read -r l; do bad "registry: $l"; done <<<"$out"
fi

out=$(check_compiler_inventory "$REGISTRY"); rc=$?
if [ "$rc" -eq 0 ]; then
  ok "binary and registry agree on every code the binary knows ($(awk -F'\t' '$2=="active"' "$TMPROOT/dump" | wc -l | tr -d ' ') active, $(awk -F'\t' '$2=="tombstone"' "$TMPROOT/dump" | wc -l | tr -d ' ') tombstone)"
  note "the reverse direction (every registry row is emittable) becomes checkable at the cutover"
else
  while IFS= read -r l; do bad "inventory: $l"; done <<<"$out"
fi

out=$(check_manifest_pins "$REGISTRY" "$MANIFEST"); rc=$?
pinned=$(grep -c $'\tcode=PD' "$MANIFEST" || true)
if [ "$rc" -eq 0 ]; then
  ok "manifest code= pins well-formed and active ($pinned row(s) pinned by code)"
  [ "$pinned" -eq 0 ] && note "VACUOUS on the live manifest — phrase authority holds until the cutover; the grammar is proven by the planted mutants below"
else
  while IFS= read -r l; do bad "manifest: $l"; done <<<"$out"
fi

# ---------------------------------------------------------------------------
# 4 — first-witness emission, against THIS binary
# ---------------------------------------------------------------------------
echo
echo "first-witness emission (real compiles):"
mkdir -p "$TMPROOT/run" || exit 2
while IFS=$'\t' read -r code name status cond just wit commit; do
  [ "${status:-}" = active ] || continue
  [ -f "$wit" ] || continue
  ( cd "$TMPROOT/run" && "$OLDPWD/$PDC" compile "$OLDPWD/$wit" -o w >/dev/null 2>"$TMPROOT/wit_stderr" )
  if ! state=$(pd_diag_parse "$TMPROOT/wit_stderr"); then
    bad "$code: could not read the stderr capture of $wit"; continue
  fi
  case "$(pd_diag_state "$state")" in
    CODED)
      if [ "$(pd_diag_code "$state")" = "$code" ]; then
        ok "$code emitted by $wit"
      else
        bad "$code: $wit emitted $(pd_diag_code "$state") instead"
      fi ;;
    NO_CODE)   bad "$code: $wit refused with no code at all" ;;
    MALFORMED) bad "$code: $wit printed $(pd_diag_code "$state") coded primary headers — cardinality-1 is broken" ;;
  esac
done < <(registry_rows "$REGISTRY")

# ---------------------------------------------------------------------------
# 5 — planted mutants. Temp dir, temp manifests, temp registries. The live
#     corpus is READ and never written.
# ---------------------------------------------------------------------------
echo
echo "planted mutants (parser + registry + manifest):"
M="$TMPROOT/mutants"; mkdir -p "$M" || exit 2

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

# M3 — UNCODED. A refusal from a site that is not wired yet says so; NO_CODE is
# a state, never a silent pass.
( cd "$TMPROOT/run" && "$OLDPWD/$PDC" compile "$OLDPWD/tests/reject/ref_parameter.pd" -o m3 >/dev/null 2>"$M/m3" )
expect_state "M3 an unwired refusal reports NO_CODE" "$M/m3" NO_CODE

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
mkman() { awk -F'\t' 'NF==6 && $1 !~ /^#/' "$MANIFEST" | head -3 >"$M/man.txt"; }
mkman
[ "$(wc -l <"$M/man.txt" | tr -d ' ')" -eq 3 ] || { echo "error: could not slice 3 manifest rows" >&2; exit 2; }

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
if [ -z "$summary" ]; then
  echo "error: the conformance run produced no summary line — no corpus verdict to fold" >&2
  printf '  %sNO VERDICT%s\n' "$YELLOW" "$NC"
  exit 2
fi
if [ "$conf_rc" -ne 0 ]; then
  bad "conformance exited $conf_rc — $summary"
else
  ok "conformance green — $summary"
  ok "$(grep -m1 '^diagnostic-codes' "$conf_log")"
fi

echo
if [ "$fails" -eq 0 ]; then
  printf '%s✓ diagnostic codes: every check green%s\n' "$GREEN" "$NC"
  exit 0
fi
printf '%s✗ diagnostic codes: %d check(s) RED%s\n' "$RED" "$fails" "$NC"
exit 1
