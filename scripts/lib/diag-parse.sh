# The ONE implementation of the diagnostic-header parser — GI-12 spec D3/R4/R6.
#
# WHY ONE. Two comparators that agree today are two comparators that can disagree
# tomorrow, and the weaker one wins every disagreement in practice because it is
# the one that says PASS. `scripts/conformance.sh` and
# `scripts/check-diagnostic-codes.sh` both source THIS file; neither is allowed a
# private copy. Sourced, not exec'd, so a consumer gets the function rather than
# a subprocess whose exit code has to re-encode the answer.
#
# WHAT IT PARSES, AND WHAT IT REFUSES TO SEE.
#   * ANSI is stripped first, because the compiler colours the header.
#   * The anchor is COLUMN 0: `^error\[PD[0-9]{4}\]: `. Every other line the
#     compiler writes is indented — the source echo is `N | `, notes are
#     `  = note:`, the location is `  --> ` — so a fixture cannot plant a header
#     by containing one. Measured hazard, not a hypothetical: this corpus already
#     had a fixture whose own text was classified as compiler output by a
#     whole-log grep.
#   * The PAYLOAD is the text after `error[PD####]: ` on that ONE line. Not the
#     first diagnostic block: notes and the source echo live in the block, so a
#     wrong-parameter refusal carrying the right code could borrow a fragment
#     from the echo, which is the substring hole this work exists to close, one
#     layer in (spec R4).
#
# THE FOUR STATES, and the cardinality contract they encode.
#   CODED <PD####> <payload>   exactly one coded primary header
#   NO_CODE                    zero coded headers. THE HONEST STATE for a site
#                              that is not wired yet, and for the CLI errors that
#                              legitimately have no language rule behind them
#                              (`pdc` with no command, a link verdict). A bare
#                              `error:` is NOT malformed (spec R1).
#   MALFORMED <n>              n >= 2 coded primary headers. A measurement
#                              failure, distinct from "wrong code": nothing can
#                              be attributed when the producer said two things.
#   (exit 2)                   the capture could not be read. Never collapsed
#                              into NO_CODE — that is the three-valued-grep
#                              lesson this repo already paid for once.
#
# Usage:  . scripts/lib/diag-parse.sh
#         state=$(pd_diag_parse "$stderr_capture") || handle-could-not-measure
#         pd_diag_code "$state" ; pd_diag_payload "$state"

# Strip ANSI SGR sequences. Same expression as conformance.sh's `strip_ansi`,
# kept here so this file is self-contained for its second consumer.
pd_diag_strip_ansi() { sed $'s/\033\\[[0-9;]*m//g'; }

# Parse a captured stderr stream. Prints ONE line, one of the states above.
# Exit 0 = parsed, 2 = could not read the capture.
pd_diag_parse() {
  local capture=$1 stripped n line
  [ -r "$capture" ] || return 2
  stripped=$(pd_diag_strip_ansi <"$capture") || return 2

  # grep's three outcomes matter here too: 0 matched, 1 no match, >1 could not
  # look. `grep -c` on a here-string cannot fail to read, but the count is taken
  # once and reused so the two questions cannot be answered by two different reads.
  n=$(printf '%s\n' "$stripped" | grep -c '^error\[PD[0-9][0-9][0-9][0-9]\]: ') || n=0

  if [ "$n" -eq 0 ]; then
    printf 'NO_CODE\n'
    return 0
  fi
  if [ "$n" -ge 2 ]; then
    printf 'MALFORMED\t%s\n' "$n"
    return 0
  fi

  line=$(printf '%s\n' "$stripped" | grep -m1 '^error\[PD[0-9][0-9][0-9][0-9]\]: ')
  printf 'CODED\t%s\t%s\n' \
    "$(printf '%s' "$line" | sed -n 's/^error\[\(PD[0-9][0-9][0-9][0-9]\)\]: .*/\1/p')" \
    "$(printf '%s' "$line" | sed 's/^error\[PD[0-9][0-9][0-9][0-9]\]: //')"
}

# Accessors, so no consumer re-splits the state line with its own idea of the
# field order.
pd_diag_state()   { printf '%s' "$1" | cut -f1; }
pd_diag_code()    { printf '%s' "$1" | cut -f2; }
pd_diag_payload() { printf '%s' "$1" | cut -f3-; }
