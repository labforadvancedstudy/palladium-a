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
# Usage: scripts/check-generated-c.sh <file.c> [file.c ...]

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

RUNTIME_DIR=runtime
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; NC=$'\033[0m'

if [ "$#" -eq 0 ]; then
  echo "usage: scripts/check-generated-c.sh <file.c> [...]" >&2
  exit 2
fi

CC=${CC:-gcc}
if ! command -v "$CC" >/dev/null 2>&1; then
  echo "error: C compiler '$CC' not found; Net B cannot run" >&2
  exit 2
fi

failures=0
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
net_a() { python3 "$NET_A" "$1" 2>&1; }

for c in "$@"; do
  if [ ! -f "$c" ]; then
    printf '  %sFAIL%s %s does not exist\n' "$RED" "$NC" "$c"
    failures=$((failures+1))
    continue
  fi
  checked=$((checked+1))
  file_bad=0

  a_out=$(net_a "$c")
  if [ -n "$a_out" ]; then
    printf '  %sFAIL%s Net A (missing return) in %s\n' "$RED" "$NC" "$c"
    printf '%s\n' "$a_out" | sed 's/^/        /'
    file_bad=1
  fi

  # Net B. Redirect to a file rather than piping: a `| head` here would SIGPIPE
  # the compiler and could be mistaken for a diagnostic.
  b_log=$(mktemp)
  if ! "$CC" -fsyntax-only -Werror=return-type -I "$RUNTIME_DIR" "$c" >"$b_log" 2>&1; then
    printf '  %sFAIL%s Net B (%s -Werror=return-type) in %s\n' "$RED" "$NC" "$CC" "$c"
    grep -a "error:" "$b_log" | head -5 | sed 's/^/        /'
    file_bad=1
  fi
  rm -f "$b_log"

  if [ "$file_bad" -eq 0 ]; then
    printf '  %sok%s   %s\n' "$GREEN" "$NC" "$c"
  else
    failures=$((failures+1))
  fi
done

if [ "$failures" -gt 0 ]; then
  printf '%s✗ generated C failed the structural invariant in %d of %d file(s)%s\n' \
    "$RED" "$failures" "$checked" "$NC"
  exit 1
fi
exit 0
