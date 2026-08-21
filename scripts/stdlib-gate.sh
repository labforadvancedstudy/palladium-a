#!/usr/bin/env bash
# The stdlib gate.
#
# WHAT THIS GATE IS FOR
# ---------------------
# `stdlib/` does not compile. Not "partially" — 0 of its 21 .pd files are
# accepted by pdc (see stdlib/STATUS.md for the measurement). A conformance-style
# gate "over stdlib" would pass because it tests nothing, which is the failure
# mode this repository exists to stamp out. So this gate answers the questions
# that ARE answerable about a directory of uncompilable library modules, and
# hands the rest to the runner that owns it.
#
#   Phase 0  NEGATIVE CONTROL. Prove this harness can fail before believing any
#            "ok" it prints.
#   Phase 1  PIN THE MEASUREMENT. Recompute every stdlib/ verdict and blocker and
#            compare against stdlib/MANIFEST.tsv.
#   Phase 2  DRIVER INVENTORY + STRUCTURAL INVARIANT ON THE GENERATED C.
#   Phase 3  BUILTIN ACCOUNTING against tests/stdlib/BUILTINS.tsv.
#
# SEAM WITH `make conformance`
# ----------------------------
# tests/stdlib/*.pd are driver programs with `fn main`, so they are conformance
# fixtures and `make conformance` runs and transcript-diffs them — its runner has
# an expected-output verdict class and its own closed inventory over tests/.
# This gate does NOT execute or diff them; duplicating that would ship two
# semantic standards for one question. What it does instead is check the
# *generated C*, which is a different question and needs no execution at all.
# stdlib/ itself stays here: those are library modules with no `main`, where the
# only pinnable thing is a compile verdict plus its blocker.
#
# Usage: scripts/stdlib-gate.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

PDC=./target/release/pdc
OUT_DIR=build_output
MANIFEST=stdlib/MANIFEST.tsv
BUILTIN_MANIFEST=tests/stdlib/BUILTINS.tsv
DRIVER_MANIFEST=tests/stdlib/DRIVERS.tsv
DRIVER_DIR=tests/stdlib
SCRATCH=$OUT_DIR/stdlib_gate_scratch

GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; NC=$'\033[0m'

if [ ! -x "$PDC" ]; then
  echo "error: $PDC not built. Run: cargo build --release" >&2
  exit 2
fi
for f in "$MANIFEST" "$BUILTIN_MANIFEST" "$DRIVER_MANIFEST"; do
  [ -f "$f" ] || { echo "error: missing $f" >&2; exit 2; }
done

rm -rf "$SCRATCH"; mkdir -p "$SCRATCH"
failures=0
note() { printf '  %sFAIL%s %s\n' "$RED" "$NC" "$1"; failures=$((failures+1)); }
ok()   { printf '  %sok%s   %s\n' "$GREEN" "$NC" "$1"; }

strip_ansi() { sed $'s/\033\\[[0-9;]*m//g'; }

# Distinct `error` lines in a compile log. Used to require that a verdict is
# justified by the WHOLE diagnostic set, not merely by one line appearing in it.
error_set() { strip_ansi <"$1" | grep -a 'error' | sed 's/^[[:space:]]*//' | sort -u; }

# Classify WHY a file was rejected, from the diagnostic plus the source line the
# compiler pointed at. The manifest pins the CATEGORY, not the wording, so
# rephrasing a diagnostic does not fail the gate but failing for a genuinely
# different reason does.
classify_blocker() {
  local file="$1" log="$2" msg loc line src
  msg=$(strip_ansi <"$log" | grep -m1 -a 'error' || true)
  loc=$(strip_ansi <"$log" | grep -m1 -aoE -- '--> [^ ]+:[0-9]+:[0-9]+' || true)
  line=$(printf '%s' "$loc" | sed -E 's/.*:([0-9]+):[0-9]+$/\1/')
  src=""
  if [ -n "$line" ] && [ "$line" -eq "$line" ] 2>/dev/null; then
    src=$(sed -n "${line}p" "$file" 2>/dev/null || true)
  fi
  local hash_pat="Unexpected character '#'"
  local esc_pat="Unexpected character '\\'"
  case "$msg" in
    *"$hash_pat"*) echo "ATTRIBUTE";   return;;
    *"$esc_pat"*)  echo "CHAR_ESCAPE"; return;;
  esac
  printf '%s' "$src" | grep -qE '[0-9]+\.[0-9]+'                                 && { echo "FLOAT_LITERAL";   return; }
  printf '%s' "$src" | grep -qE '^[[:space:]]*(pub[[:space:]]+)?use[[:space:]]'  && { echo "USE_DECL";        return; }
  printf '%s' "$src" | grep -qE '^[[:space:]]*(pub[[:space:]]+)?mod[[:space:]]'  && { echo "MOD_DECL";        return; }
  printf '%s' "$src" | grep -qE '^[[:space:]]*pub[[:space:]]+fn'                 && { echo "PUB_FN_IN_IMPL";  return; }
  case "$msg" in *"found 'type'"*) echo "ASSOC_TYPE"; return;; esac
  printf '%s' "$src" | grep -qE '<[A-Za-z_]+[[:space:]]*='                       && { echo "GENERIC_DEFAULT"; return; }
  case "$msg" in *"Expected '=' after variable name"*) echo "UNINIT_LET"; return;; esac
  echo "OTHER"
}

# ACCEPTED_NO_MAIN is load-bearing: `pdc compile` refuses any file without a
# `fn main`, and every stdlib file is a library module, so COMPILE_OK is
# unreachable for all of them and an XPASS check against it would be dead code.
#
# Requiring the no-main diagnostic to be the ONLY distinct error matters. Merely
# grepping for its presence would let a file that ALSO fails to parse be filed as
# "the language accepts this", which is the opposite of the truth.
verdict_of() {
  local file="$1" log="$2" base others
  base="stdlibgate_$(echo "$file" | tr '/.' '__')"
  if "$PDC" compile "$file" -o "$base" >"$log" 2>&1; then
    echo "COMPILE_OK"; return
  fi
  if grep -qa "No main function found" "$log"; then
    others=$(error_set "$log" | grep -cva 'No main function found')
    if [ "$others" -eq 0 ]; then echo "ACCEPTED_NO_MAIN"; else echo "COMPILE_FAIL"; fi
    return
  fi
  if grep -qa "gcc compilation failed" "$log"; then echo "LINK_FAIL"; else echo "COMPILE_FAIL"; fi
}

is_accepted() { [ "$1" = "COMPILE_OK" ] || [ "$1" = "ACCEPTED_NO_MAIN" ]; }

# ---------------------------------------------------------------------------
echo "== Phase 0: negative control (can this harness fail at all?) =="
# The control must REACH the point of comparison before its result means
# anything. Previously a compile failure, a run failure and the intended
# mismatch were all reported as "mismatch detection works" — i.e. the control
# passed loudest exactly when it had stopped controlling anything. Each step is
# now required to SUCCEED, and only the final diff is required to FAIL.
cat >"$SCRATCH/negctl.pd" <<'EOF'
fn main() {
    print("expected_line");
    print_int(41);
}
EOF
printf 'expected_line\n42\n' >"$SCRATCH/negctl.expected"
if ! "$PDC" compile "$SCRATCH/negctl.pd" -o stdlibgate_negctl >"$SCRATCH/negctl.log" 2>&1; then
  note "negative control did NOT COMPILE — the control is broken, not passing"
  strip_ansi <"$SCRATCH/negctl.log" | grep -m1 -a 'error' | sed 's/^/        /'
elif [ ! -x "$OUT_DIR/stdlibgate_negctl" ]; then
  note "negative control compiled but produced no executable — the control is broken"
elif ! "$OUT_DIR/stdlibgate_negctl" >"$SCRATCH/negctl.actual" 2>/dev/null; then
  note "negative control did NOT RUN cleanly — the control is broken, not passing"
elif diff -q "$SCRATCH/negctl.expected" "$SCRATCH/negctl.actual" >/dev/null 2>&1; then
  note "negative control ran, but the planted mismatch was NOT detected — transcript comparison is vacuous"
else
  ok "control compiled, ran, and its planted mismatch was detected"
fi

# panic() must ABORT — not merely "fail somehow".
#
# The previous version accepted ANY non-zero exit as proof that panic() worked:
# a missing binary (127), a loader failure, a wrong-architecture exec would all
# have been read as "panic aborts correctly". That is the same defect this very
# phase exists to catch, reproduced one control down. Three things are now
# required, and each is checked separately:
#   1. the executable exists,
#   2. it died from SIGABRT specifically — abort() raises signal 6, which a
#      POSIX shell reports as 128+6 = 134. A generic failure exit (1, 127, …) is
#      NOT an abort and must not be accepted,
#   3. the panic message reached stderr, proving the runtime's panic path ran
#      rather than the process dying on the way in.
# Run via a child shell (with a trailing `exit $?`, or bash exec()s the binary
# and reports the abort itself) so no "Abort trap" job message reaches the gate.
PANIC_MSG="negative control panic reached the runtime"
cat >"$SCRATCH/negpanic.pd" <<EOF
fn main() { if 1 != 2 { panic("$PANIC_MSG"); } }
EOF
if ! "$PDC" compile "$SCRATCH/negpanic.pd" -o stdlibgate_negpanic >"$SCRATCH/negpanic.log" 2>&1; then
  note "negative control for panic() failed to compile — the control is broken, not passing"
  strip_ansi <"$SCRATCH/negpanic.log" | grep -m1 -a 'error' | sed 's/^/        /'
elif [ ! -x "$OUT_DIR/stdlibgate_negpanic" ]; then
  note "negative control for panic() produced no executable — the control is broken, not passing"
else
  bash -c "'$OUT_DIR/stdlibgate_negpanic' >/dev/null 2>\"$SCRATCH/negpanic.err\"; exit \$?" 2>/dev/null
  panic_rc=$?
  if [ "$panic_rc" -eq 0 ]; then
    note "panic() exited 0 — assertions are vacuous"
  elif [ "$panic_rc" -ne 134 ]; then
    note "panic() exited $panic_rc, which is not SIGABRT (134) — the program failed for some OTHER reason, so this proves nothing about panic()"
    sed 's/^/        /' "$SCRATCH/negpanic.err" 2>/dev/null | head -3
  elif ! grep -qaF "$PANIC_MSG" "$SCRATCH/negpanic.err" 2>/dev/null; then
    note "panic() aborted but its message never reached stderr — the runtime panic path did not run"
  else
    ok "panic() aborts with SIGABRT and its message reaches stderr"
  fi
fi

# And the generated-C checker must itself be able to fail.
cat >"$SCRATCH/negc.c" <<'EOF'
long long falls_off(long long a, long long b) {
    (a + b);
}
int main(void) { return 0; }
EOF
# Same discipline: the checker must REJECT this file (exit 1), not merely fail.
# It exits 2 when its own harness is broken — a missing analyser, no python3, no
# C compiler — and accepting that as "rejection works" would report success
# precisely when the checker was checking nothing. Verified: deleting
# scripts/check-c-returns.py made the old form print "ok".
bash scripts/check-generated-c.sh "$SCRATCH/negc.c" >"$SCRATCH/negc.log" 2>&1
negc_rc=$?
if [ "$negc_rc" -eq 0 ]; then
  note "the generated-C checker ACCEPTED a function with no return — Phase 2 is vacuous"
elif [ "$negc_rc" -eq 2 ]; then
  note "the generated-C checker MALFUNCTIONED (exit 2) instead of rejecting — Phase 2 proves nothing"
  sed 's/^/        /' "$SCRATCH/negc.log" | head -4
elif [ "$negc_rc" -ne 1 ]; then
  note "the generated-C checker exited $negc_rc, which is not a rejection — Phase 2 proves nothing"
  sed 's/^/        /' "$SCRATCH/negc.log" | head -4
elif ! grep -qa 'FINDING ' "$SCRATCH/negc.log"; then
  # Exit 1 alone is not proof: it must be corroborated by a well-formed finding.
  note "the generated-C checker exited 1 but produced no well-formed FINDING — cannot tell a rejection from arbitrary output"
  sed 's/^/        /' "$SCRATCH/negc.log" | head -4
else
  ok "generated-C checker rejects a non-void function that never returns"
fi

# ---------------------------------------------------------------------------
echo
echo "== Phase 1: stdlib/ measurement is pinned to $MANIFEST =="
# No -type f: a symlinked fixture must be seen, not silently dropped. (The
# conformance corpus has one such symlink, tests/integration/test.pd.) stdlib/
# has none today; if one appears it is flagged rather than quietly included,
# because a symlink makes the manifest key ambiguous.
declare -a ON_DISK
while IFS= read -r f; do ON_DISK+=("$f"); done < <(find stdlib -name '*.pd' | sort)
while IFS= read -r l; do
  [ -n "$l" ] && note "symlink in stdlib/: $l — decide deliberately whether the manifest keys the link or its target"
done < <(find stdlib -name '*.pd' -type l)

manifest_paths=$(grep -vE '^[[:space:]]*(#|$)' "$MANIFEST" | cut -f1 | sort)
disk_paths=$(printf '%s\n' "${ON_DISK[@]}" | sort)
if [ "$manifest_paths" != "$disk_paths" ]; then
  note "stdlib/ file set drifted from the manifest:"
  diff <(printf '%s\n' "$manifest_paths") <(printf '%s\n' "$disk_paths") | sed 's/^/      /' || true
  echo "      (< only in manifest, > only on disk) — update $MANIFEST"
else
  ok "file set matches (${#ON_DISK[@]} files)"
fi

accepted=0; rejected=0
printf '\n  %-42s %-16s %s\n' "FILE" "VERDICT" "BLOCKER"
printf '  %-42s %-16s %s\n' "------------------------------------------" "----------------" "---------------"
while IFS=$'\t' read -r path want_verdict want_blocker; do
  case "$path" in ''|\#*) continue;; esac
  if [ ! -f "$path" ]; then note "$path is in the manifest but missing on disk"; continue; fi
  log="$SCRATCH/$(echo "$path" | tr '/.' '__').log"
  got_verdict=$(verdict_of "$path" "$log")
  if is_accepted "$got_verdict"; then
    got_blocker="-"; accepted=$((accepted+1))
  else
    got_blocker=$(classify_blocker "$path" "$log"); rejected=$((rejected+1))
  fi
  printf '  %-42s %-16s %s\n' "$path" "$got_verdict" "$got_blocker"
  if [ "$got_verdict" != "$want_verdict" ]; then
    if is_accepted "$want_verdict"; then
      note "REGRESSION: $path was $want_verdict, now $got_verdict"
      strip_ansi <"$log" | grep -m1 -a 'error' | sed 's/^/        /'
    elif is_accepted "$got_verdict"; then
      note "XPASS: $path is recorded $want_verdict but the language now accepts it ($got_verdict) — update $MANIFEST"
    else
      note "VERDICT_CHANGED: $path was $want_verdict, now $got_verdict — update $MANIFEST"
      strip_ansi <"$log" | grep -m1 -a 'error' | sed 's/^/        /'
    fi
  elif ! is_accepted "$got_verdict" && [ "$got_blocker" != "$want_blocker" ]; then
    note "BLOCKER_CHANGED: $path was blocked on $want_blocker, now $got_blocker — update $MANIFEST"
    strip_ansi <"$log" | grep -m1 -a 'error' | sed 's/^/        /'
  fi
done < "$MANIFEST"
printf '\n  stdlib/: %d accepted by the language, %d rejected\n' "$accepted" "$rejected"

# Reachability. `stdlib/` is on no default search path, but $PALLADIUM_PATH is
# user-configurable (src/resolver/mod.rs:52) and the resolver IS live via the
# `import` keyword — so "unreachable" holds only because the modules do not
# parse. Pin that: force stdlib/std onto the search path and require the import
# to fail with the SAME blocker the manifest records for that file.
mkdir -p "$SCRATCH/reach"
printf 'import option;\nfn main() { print_int(1); }\n' >"$SCRATCH/reach/reach.pd"
reach_log="$SCRATCH/reach.log"
REPO_ROOT=$PWD
( cd "$SCRATCH/reach" && PALLADIUM_PATH="$REPO_ROOT/stdlib/std" \
    "$REPO_ROOT/$PDC" compile reach.pd -o stdlibgate_reach ) >"$reach_log" 2>&1
reach_rc=$?
if [ "$reach_rc" -eq 0 ]; then
  note "XPASS: 'import option' with PALLADIUM_PATH=stdlib/std now SUCCEEDS — stdlib is reachable; update stdlib/STATUS.md"
elif grep -qa "Expected 'fn' for method, but found 'pub'" "$reach_log"; then
  ok "forced onto PALLADIUM_PATH, stdlib/std/option.pd still fails with its recorded blocker"
else
  note "forced-import probe failed for an UNRECORDED reason — the reachability claim in stdlib/STATUS.md is unverified:"
  strip_ansi <"$reach_log" | grep -m1 -a 'error' | sed 's/^/        /'
fi

# ---------------------------------------------------------------------------
echo
echo "== Phase 2: driver inventory + structural invariant on the generated C =="
declared=$(grep -vE '^[[:space:]]*(#|$)' "$DRIVER_MANIFEST" | cut -f1 | sort)
on_disk_pd=$(find "$DRIVER_DIR" -name '*.pd' -exec basename {} .pd \; | sort)
on_disk_gold=$(find "$DRIVER_DIR" -name '*.expected' -exec basename {} .expected \; | sort)

if [ "$declared" != "$on_disk_pd" ]; then
  note "driver set drifted: $DRIVER_MANIFEST vs *.pd on disk"
  diff <(printf '%s\n' "$declared") <(printf '%s\n' "$on_disk_pd") | sed 's/^/      /' || true
  echo "      (< declared but absent — deleted or renamed; > present but undeclared)"
else
  ok "declared drivers match *.pd on disk ($(printf '%s\n' "$declared" | wc -l | tr -d ' ') files)"
fi
if [ "$on_disk_pd" != "$on_disk_gold" ]; then
  note "driver/golden set mismatch — an orphaned golden proves nothing and a driver without one is unverifiable"
  diff <(printf '%s\n' "$on_disk_pd") <(printf '%s\n' "$on_disk_gold") | sed 's/^/      /' || true
  echo "      (< .pd without .expected, > .expected without .pd)"
else
  ok "every driver has a golden and every golden has a driver"
fi

# Compile each driver to C and check the emitted code. This does NOT run them:
# `make conformance` owns execution and transcript diffing. The invariant here is
# structural and therefore optimisation-independent, which matters because the
# defect it targets is undefined behaviour with no stable runtime manifestation
# (measured: garbage and exit 0 at BOTH -O0 and -O2).
#
# Each driver declares the verdict its C must receive. `known_violation:<fns>`
# pins an OPEN compiler defect to the exact functions it corrupts, so the
# expectation cannot go stale in either direction: if the violation spreads, or
# moves, or disappears, the gate goes red and someone updates DRIVERS.tsv.
clean_n=0; known_n=0
phase2_before=$failures
while IFS=$'\t' read -r base golden cverdict purpose; do
  case "$base" in ''|\#*) continue;; esac
  drv="$DRIVER_DIR/$base.pd"
  [ -f "$drv" ] || continue          # set-equality check above already reported it
  log="$SCRATCH/$base.log"
  if ! "$PDC" compile "$drv" -o "$base" >"$log" 2>&1; then
    note "$drv failed to compile"
    strip_ansi <"$log" | grep -m1 -a 'error' | sed 's/^/        /'
    continue
  fi
  cfile="$OUT_DIR/$base.c"
  if [ ! -f "$cfile" ]; then
    note "$drv compiled but produced no $cfile to inspect"
    continue
  fi

  cc_log="$SCRATCH/$base.cc.log"
  bash scripts/check-generated-c.sh "$cfile" >"$cc_log" 2>&1
  cc_rc=$?

  case "$cverdict" in
    clean)
      clean_n=$((clean_n+1))
      # Only exit 1 is a finding. Everything else non-zero — 2, or a signal such
      # as 137 — is a malfunction. Measured: a checker killed with 137 used to be
      # reported here as "generated C violates the structural invariant".
      if [ "$cc_rc" -eq 0 ]; then
        :
      elif [ "$cc_rc" -eq 1 ]; then
        note "$drv: generated C violates the structural invariant (declared 'clean')"
        strip_ansi <"$cc_log" | sed 's/^/      /'
      else
        note "$drv: the generated-C check MALFUNCTIONED (exit $cc_rc) — this is not evidence the C is bad OR good"
        strip_ansi <"$cc_log" | sed 's/^/      /'
      fi
      ;;
    known_violation:*)
      known_n=$((known_n+1))
      want_fns=${cverdict#known_violation:}
      # A partial FINDING set from a malfunctioning checker must never be allowed
      # to "match" the pinned set. Require exactly exit 1 before reading findings.
      if [ "$cc_rc" -ne 0 ] && [ "$cc_rc" -ne 1 ]; then
        note "$drv: the generated-C check MALFUNCTIONED (exit $cc_rc) — the pinned violation could not be confirmed"
        strip_ansi <"$cc_log" | sed 's/^/      /'
        continue
      fi
      if [ "$cc_rc" -eq 0 ]; then
        note "XPASS: $drv is recorded known_violation:$want_fns but its C is now CLEAN — the compiler defect is fixed; promote it to 'clean' in $DRIVER_MANIFEST"
        continue
      fi
      # Every declared function must be flagged, and nothing else may be.
      got_fns=$(strip_ansi <"$cc_log" | grep -a '^ *FINDING .*may fall off its end' \
                | sed -E 's/.*: ([A-Za-z_][A-Za-z_0-9 *]*[ *])([A-Za-z_][A-Za-z_0-9]*)\(.*/\2/' | sort -u | paste -sd, -)
      want_sorted=$(printf '%s' "$want_fns" | tr ',' '\n' | sort -u | paste -sd, -)
      if [ "$got_fns" != "$want_sorted" ]; then
        note "known_violation set changed for $drv: declared [$want_sorted], actual [$got_fns] — update $DRIVER_MANIFEST"
        strip_ansi <"$cc_log" | grep -a 'may fall off its end' | sed 's/^/      /'
      fi
      ;;
    *)
      note "$drv has unknown cverdict '$cverdict' in $DRIVER_MANIFEST"
      ;;
  esac
done < "$DRIVER_MANIFEST"
if [ "$failures" -eq "$phase2_before" ]; then
  ok "generated C: $clean_n clean, $known_n pinned to an open defect (2 independent nets)"
else
  printf '  %s--%s   generated C: %d clean, %d pinned — see failures above\n' "$RED" "$NC" "$clean_n" "$known_n"
fi

# ---------------------------------------------------------------------------
echo
echo "== Phase 3: every builtin in src/builtins.rs is accounted for =="
# Baseline BEFORE the first check in this phase. It used to be captured after the
# canonical-vs-recorded set check, so that one failure could still be followed by
# a green phase line.
phase3_before=$failures
canonical=$(grep -oE '^[[:space:]]+name: "[a-z_0-9]+"' src/builtins.rs | sed -E 's/.*"(.*)"/\1/' | sort)
recorded=$(grep -vE '^[[:space:]]*(#|$)' "$BUILTIN_MANIFEST" | cut -f1 | sort)
if [ "$canonical" != "$recorded" ]; then
  note "builtin set drifted from $BUILTIN_MANIFEST:"
  diff <(printf '%s\n' "$recorded") <(printf '%s\n' "$canonical") | sed 's/^/      /' || true
  echo "      (< only in manifest, > only in src/builtins.rs) — update $BUILTIN_MANIFEST"
else
  ok "all $(printf '%s\n' "$canonical" | wc -l | tr -d ' ') builtins are recorded"
fi

# Counters record entries that VERIFIED, not entries that were merely declared.
# Incrementing before the checks made "31 exercised" true by construction.
covered=0; partial=0; unusable=0
declared=0
while IFS=$'\t' read -r name status stage fp detail note; do
  case "$name" in ''|\#*) continue;; esac
  declared=$((declared+1))
  entry_before=$failures
  case "$status" in
    COVERED|PARTIAL|COVERED_BY_EFFECT)
      # The manifest NAMES the driver. Both pieces of evidence must come from
      # THAT driver, not from anywhere in the directory: searching all sources
      # for the call and all goldens for the marker independently let a call in
      # one file be vouched for by a marker in another.
      src="$DRIVER_DIR/$detail"
      gold="$DRIVER_DIR/${detail%.pd}.expected"
      if [ ! -f "$src" ]; then
        note "builtin '$name' names driver '$detail', which does not exist"
      elif [ ! -f "$gold" ]; then
        note "builtin '$name' names driver '$detail', which has no golden transcript"
      else
        if ! grep -qE "(^|[^a-z_0-9])${name}\(" "$src"; then
          note "builtin '$name' is $status but $detail does not call it"
        fi
        if [ "$status" = "COVERED_BY_EFFECT" ]; then
          # No return value to observe; the marker is a plain name. Justified
          # per-builtin in BUILTINS.tsv.
          grep -qxF "@builtin $name" "$gold" || \
            note "builtin '$name' is COVERED_BY_EFFECT but $gold has no '@builtin $name' line"
        else
          # The marker must carry the builtin's OBSERVED RESULT, computed by
          # calling it. A bare name would only prove that a print ran.
          grep -qE "^@builtin ${name} -> .+$" "$gold" || \
            note "builtin '$name' is $status but $gold has no '@builtin $name -> <result>' line — a marker without an observed result is not evidence the call ran"
        fi
      fi
      ;;
    NEGATIVE_CONTROL)
      : ;;   # proved in Phase 0
    UNUSABLE)
      probe="$SCRATCH/probe_$name.pd"
      plog="$SCRATCH/probe_$name.log"
      printf 'fn main() { %s }\n' "$detail" >"$probe"
      if "$PDC" compile "$probe" -o "stdlibgate_probe_$name" >"$plog" 2>&1; then
        note "XPASS: builtin '$name' is recorded UNUSABLE but now compiles — update $BUILTIN_MANIFEST"
      else
        # Pin the STAGE and the DIAGNOSTIC. Without this, a future parser or
        # typechecker regression would "re-prove" a gcc signature mismatch while
        # actually failing for an entirely unrelated reason.
        if grep -qa "gcc compilation failed" "$plog"; then act_stage=link; else act_stage=compile; fi
        if [ "$act_stage" != "$stage" ]; then
          note "STAGE_CHANGED: '$name' is recorded failing at '$stage' but failed at '$act_stage' — that is not the recorded defect"
          strip_ansi <"$plog" | grep -m1 -a 'error' | sed 's/^/        /'
        elif ! strip_ansi <"$plog" | grep -qaF -- "$fp"; then
          note "FINGERPRINT_CHANGED: '$name' failed at '$stage' but not with '$fp' — that is not the recorded defect"
          strip_ansi <"$plog" | grep -m1 -a 'error:' | sed 's/^/        /'
        fi
      fi
      ;;
    *) note "builtin '$name' has unknown status '$status' in $BUILTIN_MANIFEST" ;;
  esac
  # Count it only if nothing was recorded against it.
  if [ "$failures" -eq "$entry_before" ]; then
    case "$status" in
      PARTIAL)  partial=$((partial+1)) ;;
      UNUSABLE) unusable=$((unusable+1)) ;;
      *)        covered=$((covered+1)) ;;
    esac
  fi
done < "$BUILTIN_MANIFEST"
# MERGE-TIME RECONCILIATION.
# fix/m1-builtin-registry enumerates the C-seam defects per DIMENSION; this file
# can only say UNUSABLE or not. Once that branch lands, every builtin it marks
# unsupported must still be recorded UNUSABLE here — otherwise one table has been
# promoted without the other, which is the drift both exist to stop.
#
# Keyed on `Support::Unsupported`, a structured field on each Builtin in that
# branch, rather than on the prose of the PRELUDE_TYPE_MISMATCHES string array:
# an enum variant survives reformatting, and the array's wording does not.
# `PRELUDE_TYPE_MISMATCHES` is accepted as a second activation marker only.
#
# FAILS CLOSED. Earlier this silently disarmed itself if the constant were
# renamed or reformatted: extraction returned zero names and it still printed
# "reconciled". Now, once ANY marker is present, an empty extraction is a
# failure, because "I found nothing to reconcile" and "I could not read it" must
# not look the same.
if grep -q -e 'Support::Unsupported' -e 'PRELUDE_TYPE_MISMATCHES' src/builtins.rs 2>/dev/null; then
  recon_names=$(python3 - src/builtins.rs <<'RECON'
import re, sys
src = open(sys.argv[1], errors="replace").read()
names = set()
# Each `Builtin { ... }` block that carries Support::Unsupported.
for block in re.findall(r"Builtin\s*\{.*?\n    \}", src, re.S):
    if "Support::Unsupported" in block:
        m = re.search(r'name:\s*"([a-z_0-9]+)"', block)
        if m:
            names.add(m.group(1))
# Fallback: the PRELUDE_TYPE_MISMATCHES array, if that is all that exists.
if not names:
    arr = re.search(r"PRELUDE_TYPE_MISMATCHES[^=]*=\s*&\[(.*?)\];", src, re.S)
    if arr:
        names.update(re.findall(r'"([a-z_0-9]+) (?:param|return)', arr.group(1)))
print("\n".join(sorted(names)))
RECON
)
  recon_rc=$?
  recon_count=$(printf '%s' "$recon_names" | grep -c . || true)
  if [ "$recon_rc" -ne 0 ]; then
    note "RECONCILE: could not parse src/builtins.rs for unsupported builtins (extractor exit $recon_rc) — failing closed rather than reporting 'reconciled'"
  elif [ "$recon_count" -eq 0 ]; then
    note "RECONCILE: src/builtins.rs carries a seam marker but zero builtin names could be extracted — the parsing contract broke; failing closed"
  else
    recon_missing=0
    while IFS= read -r bname; do
      [ -n "$bname" ] || continue
      if ! grep -qE "^${bname}\tUNUSABLE\t" "$BUILTIN_MANIFEST"; then
        note "RECONCILE: src/builtins.rs marks '$bname' unsupported at the C seam, but $BUILTIN_MANIFEST does not record it UNUSABLE — resolve both tables together"
        recon_missing=$((recon_missing+1))
      fi
    done <<RECONEOF
$recon_names
RECONEOF
    [ "$recon_missing" -eq 0 ] && ok "reconciled with src/builtins.rs ($recon_count unsupported builtin(s))"
  fi
else
  printf '  %s..%s   no C-seam marker in src/builtins.rs yet; reconciliation arms itself when fix/m1-builtin-registry lands\n' "$GREEN" "$NC"
fi

if [ "$failures" -eq "$phase3_before" ]; then
  ok "$covered exercised, $partial partial, $unusable unusable and re-proved at a pinned stage+diagnostic"
else
  printf '  %s--%s   %d of %d builtin entries verified — see failures above\n' \
    "$RED" "$NC" "$((covered+partial+unusable))" "$declared"
fi

# ---------------------------------------------------------------------------
echo
echo "=============================================="
if [ "$failures" -eq 0 ]; then
  printf '%s✓ stdlib gate green%s — %d stdlib files pinned, %d drivers inventoried, %d builtins accounted for\n' \
    "$GREEN" "$NC" "${#ON_DISK[@]}" "$((clean_n+known_n))" "$((covered+partial+unusable))"
  echo "  transcript verification of tests/stdlib/ belongs to \`make conformance\`"
  echo "=============================================="
  exit 0
fi
printf '%s✗ stdlib gate red%s — %d failure(s)\n' "$RED" "$NC" "$failures"
echo "=============================================="
exit 1
