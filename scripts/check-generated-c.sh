#!/usr/bin/env bash
# Structural invariant on the C that pdc emits.
#
# WHY THIS EXISTS
# ---------------
# Defect D3: a tail expression in a value-returning function was lowered to a
# bare expression statement, so codegen emitted `(a + b);` and the function fell
# off its end with no `return`. That is undefined behaviour in C, and UB has no
# stable manifestation you can test for:
#
#   measured with the D3 fix reverted, `fn add(a,b) -> i64 { a + b }` returned
#     -O2 -> 8261746944, exit 0
#     -O0 -> 8264595040, exit 0
#
# Garbage at both levels, exit 0 at both. So neither an exit-code gate nor a
# pinned optimisation level can be relied on: the garbage value could equal the
# expected value by accident on another machine, another libc, another compiler,
# and then a transcript diff would pass too.
#
# The only stable statement is a STRUCTURAL one about the emitted code, which is
# what this script checks. It is optimisation-independent by construction: it
# never runs anything.
#
# TWO INDEPENDENT NETS
# --------------------
#   Net A (own analysis)  every non-void function's body must DEFINITELY RETURN
#                         on every path (scripts/check-c-returns.py). Phrased
#                         over the EMITTED BODY, not over the source construct:
#                         "a tail expression lowers to a return" would only
#                         catch what the parser already handles, whereas this
#                         catches a tail `if`, a tail `match`, and anything else
#                         lowering forgets — including defects nobody has found.
#                         Compiler-independent: it needs no C compiler to have
#                         an opinion.
#   Net B (C compiler)    `-Werror=return-type`: the same question answered by a
#                         real compiler's control-flow graph, over the real
#                         grammar. It is a FRONTEND diagnostic — verified
#                         identical at -O0, -O2 and -O3.
#
# Neither net subsumes the other, and they fail in different directions. Net A
# is a line-oriented reader of the shape pdc happens to emit, so unusual
# formatting, `switch`, or `goto` could defeat it — but it survives a C compiler
# that stops diagnosing, or a switch to one that never did. Net B understands
# the language properly but only exists while the compiler cooperates. A defect
# has to get past both.
#
# EXIT TAXONOMY — a finding and a malfunction must not share an exit code.
#   0  every file analysed, invariant holds
#   1  at least one genuine FINDING, and nothing malfunctioned
#   2  a HARNESS error: input missing/unreadable, an analyser that raised, or a
#      C compiler that failed for a reason unrelated to return types. Harness
#      errors DOMINATE, because a partial analysis cannot support a verdict.
#
# The distinction is load-bearing for callers: scripts/stdlib-gate.sh uses this
# script as a negative control, and "it exited non-zero" is not evidence that it
# rejected anything. Measured before this taxonomy existed: a Python
# RecursionError was printed as "FAIL Net A (falls off the end)".
#
# Usage: scripts/check-generated-c.sh <file.c> [file.c ...]

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

RUNTIME_DIR=runtime
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; NC=$'\033[0m'

if [ "$#" -eq 0 ]; then
  echo "usage: scripts/check-generated-c.sh <file.c> [...]" >&2
  exit 2
fi

strip_ansi() { sed $'s/\033\\[[0-9;]*m//g'; }

CC=${CC:-gcc}
if ! command -v "$CC" >/dev/null 2>&1; then
  echo "error: C compiler '$CC' not found; Net B cannot run" >&2
  exit 2
fi

violations=0
harness=0
checked=0

# --- Net A ------------------------------------------------------------------
# Delegated to scripts/check-c-returns.py: a real terminator analysis, because
# "the last line is a return" would wrongly flag legitimate code such as
#     if (c) { return 1; } else { return 2; }
# whose last line is `}`. An if/else terminates iff BOTH arms do; an `if` with
# no `else` never does.
NET_A=scripts/check-c-returns.py
if [ ! -f "$NET_A" ]; then
  echo "error: $NET_A missing; Net A cannot run" >&2
  exit 2
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 not found; Net A cannot run" >&2
  exit 2
fi
# Exit 0 = clean, 1 = violations found, anything else = the analyser itself
# broke. Conflating the third with the second would report a crashed analyser as
# a structural defect (and, in a negative control, as proof the net works).
net_a() { python3 "$NET_A" "$1" 2>&1; }

for c in "$@"; do
  if [ ! -f "$c" ]; then
    printf '  %sHARNESS%s %s does not exist — nothing was analysed\n' "$RED" "$NC" "$c"
    harness=$((harness+1))
    continue
  fi
  if [ ! -r "$c" ]; then
    printf '  %sHARNESS%s %s is not readable — nothing was analysed\n' "$RED" "$NC" "$c"
    harness=$((harness+1))
    continue
  fi
  checked=$((checked+1))
  file_violation=0
  file_harness=0

  a_out=$(net_a "$c"); a_rc=$?
  if [ "$a_rc" -eq 1 ]; then
    # Exit 1 must be corroborated by a well-formed FINDING line. Trusting the
    # code alone would let arbitrary output (a traceback, a usage message) be
    # presented as a structural defect.
    if printf '%s\n' "$a_out" | grep -q '^FINDING '; then
      printf '  %sFAIL%s Net A (falls off the end) in %s\n' "$RED" "$NC" "$c"
      printf '%s\n' "$a_out" | sed 's/^/        /'
      file_violation=1
    else
      printf '  %sHARNESS%s Net A exited 1 on %s with no well-formed FINDING — treating as a malfunction, not a defect\n' "$RED" "$NC" "$c"
      printf '%s\n' "$a_out" | sed 's/^/        /'
      file_harness=1
    fi
  elif [ "$a_rc" -ne 0 ]; then
    printf '  %sHARNESS%s Net A exit %d on %s — the net did not run\n' "$RED" "$NC" "$a_rc" "$c"
    printf '%s\n' "$a_out" | sed 's/^/        /'
    file_harness=1
  fi

  # Net B. Redirect to a file rather than piping: a `| head` here would SIGPIPE
  # the compiler and could be mistaken for a diagnostic.
  # NET B CLASSIFICATION. A non-zero exit from the C compiler is not by itself a
  # return-type finding, and neither is the mere SIGHTING of return-type wording
  # somewhere in the log: `#error no return statement` contains that text and is
  # a completely unrelated defect (measured — it used to be reported as a Net B
  # finding). So the COMPLETE diagnostic set is validated:
  #   * the compiler must have exited 1, a normal "I found errors" exit. Anything
  #     above 128 is a signal — a killed compiler proves nothing, however much
  #     text it managed to buffer;
  #   * there must be at least one `error:` line;
  #   * EVERY error line must be a return-type diagnostic. One unrelated error
  #     means this file failed for another reason and Net B cannot speak to it.
  # Findings are emitted as structured `FINDING` lines, carrying file:line, so
  # Net B is subject to the same corroboration rule as Net A rather than sitting
  # outside it.
  b_log=$(mktemp)
  "$CC" -fsyntax-only -Werror=return-type -I "$RUNTIME_DIR" "$c" >"$b_log" 2>&1
  b_rc=$?
  if [ "$b_rc" -ne 0 ]; then
    b_errors=$(strip_ansi <"$b_log" | grep -a 'error:' || true)
    b_total=$(printf '%s' "$b_errors" | grep -c . || true)
    b_rettype=$(printf '%s\n' "$b_errors" \
      | grep -aE 'error: .*(non-void function does not return a value|control reaches end of non-void function|\[-Werror?=?return-type\])' || true)
    b_rt_n=$(printf '%s' "$b_rettype" | grep -c . || true)

    if [ "$b_rc" -gt 128 ]; then
      printf '  %sHARNESS%s Net B: %s was killed by a signal (exit %d) on %s — a killed compiler proves nothing\n' \
        "$RED" "$NC" "$CC" "$b_rc" "$c"
      file_harness=1
    elif [ "$b_rc" -ne 1 ]; then
      printf '  %sHARNESS%s Net B: %s exited %d on %s, which is not its "errors found" exit\n' \
        "$RED" "$NC" "$CC" "$b_rc" "$c"
      file_harness=1
    elif [ "$b_total" -eq 0 ]; then
      printf '  %sHARNESS%s Net B: %s failed on %s but emitted no error diagnostic to classify\n' \
        "$RED" "$NC" "$CC" "$c"
      sed 's/^/        /' "$b_log" | head -3
      file_harness=1
    elif [ "$b_rt_n" -ne "$b_total" ]; then
      printf '  %sHARNESS%s Net B could not run on %s — %d of %d diagnostics are NOT return-type errors, so %s failed for another reason\n' \
        "$RED" "$NC" "$c" "$((b_total - b_rt_n))" "$b_total" "$CC"
      printf '%s\n' "$b_errors" | grep -avE 'non-void function does not return a value|control reaches end of non-void function|\[-Werror?=?return-type\]' | head -3 | sed 's/^/        /'
      file_harness=1
    else
      printf '  %sFAIL%s Net B (%s -Werror=return-type) in %s\n' "$RED" "$NC" "$CC" "$c"
      printf '%s\n' "$b_rettype" | head -5 | sed 's/^/        FINDING /'
      file_violation=1
    fi
  fi
  rm -f "$b_log"

  if [ "$file_harness" -ne 0 ]; then
    harness=$((harness+1))
  elif [ "$file_violation" -ne 0 ]; then
    violations=$((violations+1))
  else
    # Report the denominator: "ok" over zero analysed functions would be a
    # vacuous pass, and Net A now refuses that case outright.
    printf '  %sok%s   %s (%s)\n' "$GREEN" "$NC" "$c" \
      "$(printf '%s\n' "$a_out" | grep -a '^ANALYSED' | head -1 | sed 's/^ANALYSED //; s/ in [0-9]* file(s)$//')"
  fi
done

# Harness errors dominate: a run that malfunctioned cannot assert "these are the
# defects", nor can it assert that there are none.
if [ "$harness" -gt 0 ]; then
  printf '%s✗ generated-C check MALFUNCTIONED on %d input(s) (%d genuine finding(s) also seen)%s\n' \
    "$RED" "$harness" "$violations" "$NC"
  exit 2
fi
if [ "$violations" -gt 0 ]; then
  printf '%s✗ generated C failed the structural invariant in %d of %d file(s)%s\n' \
    "$RED" "$violations" "$checked" "$NC"
  exit 1
fi
exit 0
