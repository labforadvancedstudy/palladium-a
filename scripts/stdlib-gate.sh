#!/usr/bin/env bash
# The stdlib gate.
#
# WHAT THIS GATE IS FOR
# ---------------------
# `stdlib/` does not compile. Not "partially" — 0 of its 21 .pd files are
# accepted by pdc, and nothing in src/ ever loads them (see stdlib/STATUS.md
# for the measurement and the evidence). Writing a conformance-style gate
# "over stdlib" would therefore be a gate that passes because it tests nothing,
# which is the exact failure mode this repository exists to stamp out.
#
# So this gate does three separate, honest jobs:
#
#   Phase 1 — PIN THE MEASUREMENT.
#     Recompute the verdict for every file under stdlib/ and compare it against
#     stdlib/MANIFEST.tsv. Fails when a file that used to compile stops
#     compiling (REGRESSION), when a file recorded as uncompilable starts
#     compiling (XPASS — the manifest is stale and must be updated), when the
#     reason a file fails changes (BLOCKER_CHANGED), or when the file set on
#     disk drifts from the manifest.
#
#   Phase 2 — COVER WHAT stdlib WOULD REST ON.
#     Compile, link and run every driver in tests/stdlib/ and diff its complete
#     stdout against a golden transcript. This is the coverage the M1 item was
#     actually asking for: the language surface and the builtins that a real
#     standard library would be built out of, including the tail-expression
#     return that miscompiled unnoticed.
#
#     Transcript diffing is not decoration. The tail-return defect makes the
#     generated C undefined, and at -O2 gcc is entitled to delete the very
#     assertion that would catch it: with the fix reverted,
#     tests/stdlib/stdlib_tail_return.pd printed 8261746944 instead of 42 and
#     still exited 0. An exit-code-only gate — which is what
#     scripts/conformance.sh is — cannot see that. Comparing the transcript can.
#
#   Phase 3 — PIN BUILTIN COVERAGE.
#     Every builtin in src/builtins.rs must be accounted for in
#     tests/stdlib/BUILTINS.tsv: either COVERED (and then it must actually be
#     called by a driver) or UNUSABLE (and then a probe must confirm it still
#     does not compile). A builtin cannot be silently dropped from coverage,
#     and an UNUSABLE one that starts working goes XPASS.
#
# Phase 0 is a negative control: it proves this harness can fail at all before
# any of its passes are believed.
#
# Usage: scripts/stdlib-gate.sh

set -uo pipefail
cd "$(dirname "$0")/.."

PDC=./target/release/pdc
OUT_DIR=build_output
MANIFEST=stdlib/MANIFEST.tsv
BUILTIN_MANIFEST=tests/stdlib/BUILTINS.tsv
DRIVER_DIR=tests/stdlib
SCRATCH=$OUT_DIR/stdlib_gate_scratch

GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; YEL=$'\033[0;33m'; NC=$'\033[0m'

if [ ! -x "$PDC" ]; then
  echo "error: $PDC not built. Run: cargo build --release" >&2
  exit 2
fi
for f in "$MANIFEST" "$BUILTIN_MANIFEST"; do
  [ -f "$f" ] || { echo "error: missing $f" >&2; exit 2; }
done

rm -rf "$SCRATCH"; mkdir -p "$SCRATCH"
failures=0
note() { printf '  %s%s%s %s\n' "$RED" "FAIL" "$NC" "$1"; failures=$((failures+1)); }
ok()   { printf '  %sok%s   %s\n' "$GREEN" "$NC" "$1"; }

# Strip ANSI so error text can be matched.
strip_ansi() { sed $'s/\033\\[[0-9;]*m//g'; }

# Classify WHY a file was rejected, from the error message plus the source line
# the compiler pointed at. The category — not the exact wording — is what the
# manifest pins, so that rephrasing a diagnostic does not fail the gate but
# failing for a genuinely different reason does.
classify_blocker() {
  local file="$1" log="$2"
  local msg loc line src
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
    *"$hash_pat"*) echo "ATTRIBUTE";        return;;
    *"$esc_pat"*)  echo "CHAR_ESCAPE";      return;;
  esac
  if printf '%s' "$src" | grep -qE '[0-9]+\.[0-9]+'; then
    echo "FLOAT_LITERAL"; return
  fi
  if printf '%s' "$src" | grep -qE '^[[:space:]]*(pub[[:space:]]+)?use[[:space:]]'; then
    echo "USE_DECL"; return
  fi
  if printf '%s' "$src" | grep -qE '^[[:space:]]*(pub[[:space:]]+)?mod[[:space:]]'; then
    echo "MOD_DECL"; return
  fi
  if printf '%s' "$src" | grep -qE '^[[:space:]]*pub[[:space:]]+fn'; then
    echo "PUB_FN_IN_IMPL"; return
  fi
  case "$msg" in
    *"found 'type'"*) echo "ASSOC_TYPE"; return;;
  esac
  if printf '%s' "$src" | grep -qE '<[A-Za-z_]+[[:space:]]*='; then
    echo "GENERIC_DEFAULT"; return
  fi
  case "$msg" in
    *"Expected '=' after variable name"*) echo "UNINIT_LET"; return;;
  esac
  echo "OTHER"
}

# Compile one .pd and classify the outcome.
#
# ACCEPTED_NO_MAIN is load-bearing, not a nicety. Every file under stdlib/ is a
# library module, and `pdc compile` refuses any file without a `fn main`
# ("No main function found"). Without this verdict, a stdlib module could never
# reach COMPILE_OK however much the language grew, so the XPASS check below —
# "a file recorded as uncompilable started compiling" — would be unreachable and
# the gate would be quietly incapable of reporting the one thing it promises.
# ACCEPTED_NO_MAIN means: the language accepted this file; only the harness's
# main-function requirement stands between it and a real module.
verdict_of() {
  local file="$1" log="$2" base
  base="stdlibgate_$(echo "$file" | tr '/.' '__')"
  if "$PDC" compile "$file" -o "$base" >"$log" 2>&1; then
    echo "COMPILE_OK"
  elif grep -qa "No main function found" "$log"; then
    echo "ACCEPTED_NO_MAIN"
  elif grep -qa "gcc compilation failed" "$log"; then
    echo "LINK_FAIL"
  else
    echo "COMPILE_FAIL"
  fi
}

# Does this verdict mean the language accepted the file?
is_accepted() {
  [ "$1" = "COMPILE_OK" ] || [ "$1" = "ACCEPTED_NO_MAIN" ]
}

# ---------------------------------------------------------------------------
echo "== Phase 0: negative control (can this harness fail at all?) =="
# A program whose transcript deliberately disagrees with its golden file. If
# this is NOT detected, every "ok" printed below is worthless.
cat >"$SCRATCH/negctl.pd" <<'EOF'
fn main() {
    print("expected_line");
    print_int(41);
}
EOF
printf 'expected_line\n42\n' >"$SCRATCH/negctl.expected"
if "$PDC" compile "$SCRATCH/negctl.pd" -o stdlibgate_negctl >"$SCRATCH/negctl.log" 2>&1 \
   && "$OUT_DIR/stdlibgate_negctl" >"$SCRATCH/negctl.actual" 2>/dev/null \
   && diff -q "$SCRATCH/negctl.expected" "$SCRATCH/negctl.actual" >/dev/null; then
  note "negative control was NOT detected — the transcript check is vacuous"
else
  ok "transcript mismatch is detected"
fi
# And that a failed assertion really does exit non-zero.
cat >"$SCRATCH/negpanic.pd" <<'EOF'
fn main() { if 1 != 2 { panic("negative control"); } }
EOF
if "$PDC" compile "$SCRATCH/negpanic.pd" -o stdlibgate_negpanic >"$SCRATCH/negpanic.log" 2>&1; then
  # Run via a child shell: panic() calls abort(), and this shell would
  # otherwise print its own "Abort trap: 6" job message to the gate's output.
  if bash -c "'$OUT_DIR/stdlibgate_negpanic'" >/dev/null 2>&1; then
    note "panic() did not produce a non-zero exit — assertions are vacuous"
  else
    ok "panic() exits non-zero (covers the panic builtin)"
  fi
else
  note "negative control for panic() failed to compile"
fi

# ---------------------------------------------------------------------------
echo
echo "== Phase 1: stdlib/ measurement is pinned to $MANIFEST =="
declare -a ON_DISK
while IFS= read -r f; do ON_DISK+=("$f"); done < <(find stdlib -name '*.pd' | sort)

manifest_paths=$(grep -vE '^\s*(#|$)' "$MANIFEST" | cut -f1 | sort)
disk_paths=$(printf '%s\n' "${ON_DISK[@]}" | sort)

if [ "$manifest_paths" != "$disk_paths" ]; then
  note "stdlib/ file set drifted from the manifest:"
  diff <(printf '%s\n' "$manifest_paths") <(printf '%s\n' "$disk_paths") \
    | sed 's/^/      /' || true
  echo "      (< only in manifest, > only on disk) — update $MANIFEST"
else
  ok "file set matches (${#ON_DISK[@]} files)"
fi

compile_ok=0; compile_fail=0
printf '\n  %-42s %-16s %s\n' "FILE" "VERDICT" "BLOCKER"
printf '  %-42s %-16s %s\n' "------------------------------------------" "----------------" "---------------"
while IFS=$'\t' read -r path want_verdict want_blocker; do
  case "$path" in ''|\#*) continue;; esac
  if [ ! -f "$path" ]; then
    note "$path is in the manifest but missing on disk"
    continue
  fi
  log="$SCRATCH/$(echo "$path" | tr '/.' '__').log"
  got_verdict=$(verdict_of "$path" "$log")
  if is_accepted "$got_verdict"; then
    got_blocker="-"
    compile_ok=$((compile_ok+1))
  else
    got_blocker=$(classify_blocker "$path" "$log")
    compile_fail=$((compile_fail+1))
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
printf '\n  stdlib/: %d accepted by the language, %d rejected\n' "$compile_ok" "$compile_fail"

# ---------------------------------------------------------------------------
echo
echo "== Phase 2: tests/stdlib drivers must match their golden transcripts =="
drivers=0
while IFS= read -r drv; do
  base=$(basename "$drv" .pd)
  golden="$DRIVER_DIR/$base.expected"
  drivers=$((drivers+1))
  if [ ! -f "$golden" ]; then
    note "$drv has no $base.expected — a driver without a golden transcript proves nothing"
    continue
  fi
  log="$SCRATCH/$base.log"
  if ! "$PDC" compile "$drv" -o "$base" >"$log" 2>&1; then
    note "$drv failed to compile"
    strip_ansi <"$log" | grep -m1 -a 'error' | sed 's/^/        /'
    continue
  fi
  actual="$SCRATCH/$base.actual"
  "$OUT_DIR/$base" >"$actual" 2>/dev/null
  rc=$?
  if [ $rc -ne 0 ]; then
    note "$drv exited $rc (an assertion fired)"
    diff "$golden" "$actual" | head -12 | sed 's/^/        /'
    continue
  fi
  if ! diff -q "$golden" "$actual" >/dev/null; then
    note "$drv transcript differs from $base.expected"
    diff "$golden" "$actual" | head -20 | sed 's/^/        /'
    continue
  fi
  ok "$drv ($(wc -l <"$golden" | tr -d ' ') lines verified)"
done < <(find "$DRIVER_DIR" -name '*.pd' | sort)
[ "$drivers" -gt 0 ] || note "no drivers found in $DRIVER_DIR"

# ---------------------------------------------------------------------------
echo
echo "== Phase 3: every builtin in src/builtins.rs is accounted for =="
canonical=$(grep -oE '^\s+name: "[a-z_0-9]+"' src/builtins.rs | sed -E 's/.*"(.*)"/\1/' | sort)
recorded=$(grep -vE '^\s*(#|$)' "$BUILTIN_MANIFEST" | cut -f1 | sort)
if [ "$canonical" != "$recorded" ]; then
  note "builtin set drifted from $BUILTIN_MANIFEST:"
  diff <(printf '%s\n' "$recorded") <(printf '%s\n' "$canonical") | sed 's/^/      /' || true
  echo "      (< only in manifest, > only in src/builtins.rs) — update $BUILTIN_MANIFEST"
else
  ok "all $(printf '%s\n' "$canonical" | wc -l | tr -d ' ') builtins are recorded"
fi

covered=0; unusable=0
while IFS=$'\t' read -r name status detail; do
  case "$name" in ''|\#*) continue;; esac
  case "$status" in
    COVERED)
      covered=$((covered+1))
      if ! grep -qhE "(^|[^a-z_])${name}\(" "$DRIVER_DIR"/*.pd; then
        note "builtin '$name' is marked COVERED but no driver in $DRIVER_DIR calls it"
      fi
      ;;
    UNUSABLE)
      unusable=$((unusable+1))
      # Re-prove it: an UNUSABLE builtin that quietly started working would
      # leave the manifest lying, so the gate re-runs the probe every time.
      probe="$SCRATCH/probe_$name.pd"
      printf 'fn main() { %s }\n' "$detail" >"$probe"
      if "$PDC" compile "$probe" -o "stdlibgate_probe_$name" >"$SCRATCH/probe_$name.log" 2>&1; then
        note "XPASS: builtin '$name' is recorded UNUSABLE but now compiles — update $BUILTIN_MANIFEST"
      fi
      ;;
    NEGATIVE_CONTROL)
      # Covered by Phase 0 (it aborts, so no passing program can call it).
      covered=$((covered+1))
      ;;
    *)
      note "builtin '$name' has unknown status '$status' in $BUILTIN_MANIFEST"
      ;;
  esac
done < "$BUILTIN_MANIFEST"
ok "$covered builtins exercised, $unusable recorded unusable and re-proved"

# ---------------------------------------------------------------------------
echo
echo "=============================================="
if [ "$failures" -eq 0 ]; then
  printf '%s✓ stdlib gate green%s — %d stdlib files pinned, %d drivers verified, %d builtins accounted for\n' \
    "$GREEN" "$NC" "${#ON_DISK[@]}" "$drivers" "$((covered+unusable))"
  echo "=============================================="
  exit 0
fi
printf '%s✗ stdlib gate red%s — %d failure(s)\n' "$RED" "$NC" "$failures"
echo "=============================================="
exit 1
