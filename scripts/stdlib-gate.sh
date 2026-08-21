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

# panic() must abort. Run via a child shell so this shell's own "Abort trap" job
# message does not land in the gate output. The trailing `exit $?` is required:
# with a single command, bash exec()s it and the abort is reported by THIS shell
# anyway.
cat >"$SCRATCH/negpanic.pd" <<'EOF'
fn main() { if 1 != 2 { panic("negative control"); } }
EOF
if ! "$PDC" compile "$SCRATCH/negpanic.pd" -o stdlibgate_negpanic >"$SCRATCH/negpanic.log" 2>&1; then
  note "negative control for panic() failed to compile — the control is broken"
elif bash -c "'$OUT_DIR/stdlibgate_negpanic' >/dev/null 2>&1; exit \$?" 2>/dev/null; then
  note "panic() did not produce a non-zero exit — assertions are vacuous"
else
  ok "panic() exits non-zero (covers the panic builtin)"
fi

# And the generated-C checker must itself be able to fail.
cat >"$SCRATCH/negc.c" <<'EOF'
long long falls_off(long long a, long long b) {
    (a + b);
}
int main(void) { return 0; }
EOF
if bash scripts/check-generated-c.sh "$SCRATCH/negc.c" >/dev/null 2>&1; then
  note "the generated-C checker accepted a function with no return — Phase 2 is vacuous"
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
      if [ "$cc_rc" -ne 0 ]; then
        note "$drv: generated C violates the structural invariant (declared 'clean')"
        strip_ansi <"$cc_log" | sed 's/^/      /'
      fi
      ;;
    known_violation:*)
      known_n=$((known_n+1))
      want_fns=${cverdict#known_violation:}
      if [ "$cc_rc" -eq 0 ]; then
        note "XPASS: $drv is recorded known_violation:$want_fns but its C is now CLEAN — the compiler defect is fixed; promote it to 'clean' in $DRIVER_MANIFEST"
        continue
      fi
      # Every declared function must be flagged, and nothing else may be.
      got_fns=$(strip_ansi <"$cc_log" | grep -a 'may fall off its end' \
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
ok "generated C: $clean_n clean, $known_n pinned to an open defect (2 independent nets)"

# ---------------------------------------------------------------------------
echo
echo "== Phase 3: every builtin in src/builtins.rs is accounted for =="
canonical=$(grep -oE '^[[:space:]]+name: "[a-z_0-9]+"' src/builtins.rs | sed -E 's/.*"(.*)"/\1/' | sort)
recorded=$(grep -vE '^[[:space:]]*(#|$)' "$BUILTIN_MANIFEST" | cut -f1 | sort)
if [ "$canonical" != "$recorded" ]; then
  note "builtin set drifted from $BUILTIN_MANIFEST:"
  diff <(printf '%s\n' "$recorded") <(printf '%s\n' "$canonical") | sed 's/^/      /' || true
  echo "      (< only in manifest, > only in src/builtins.rs) — update $BUILTIN_MANIFEST"
else
  ok "all $(printf '%s\n' "$canonical" | wc -l | tr -d ' ') builtins are recorded"
fi

covered=0; partial=0; unusable=0
while IFS=$'\t' read -r name status stage fp detail; do
  case "$name" in ''|\#*) continue;; esac
  case "$status" in
    COVERED|PARTIAL)
      if [ "$status" = "PARTIAL" ]; then partial=$((partial+1)); else covered=$((covered+1)); fi
      # (a) statically called somewhere in the drivers, AND
      # (b) its marker reached a GOLDEN transcript, i.e. the path really ran.
      # Either alone is defeatable: (a) by dead code, (b) by a stray print.
      if ! grep -qhE "(^|[^a-z_0-9])${name}\(" "$DRIVER_DIR"/*.pd; then
        note "builtin '$name' is $status but no driver in $DRIVER_DIR calls it"
      fi
      if ! grep -qhxF "@builtin $name" "$DRIVER_DIR"/*.expected; then
        note "builtin '$name' is $status but no golden contains '@builtin $name' — no runtime evidence it was exercised"
      fi
      ;;
    NEGATIVE_CONTROL)
      covered=$((covered+1))   # proved in Phase 0
      ;;
    UNUSABLE)
      unusable=$((unusable+1))
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
done < "$BUILTIN_MANIFEST"
ok "$covered exercised, $partial partial, $unusable unusable and re-proved at a pinned stage+diagnostic"

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
