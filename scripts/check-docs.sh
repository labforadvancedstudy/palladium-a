#!/usr/bin/env bash
# Extract every ```palladium code block from the documentation, compile it, and
# report which ones the compiler actually accepts.
#
# This exists because this repository's documentation spent a year describing a
# language the compiler did not implement. A snippet that does not compile is a
# false claim, and this script is what makes that mechanical instead of a matter
# of good intentions.
#
# A block is checked as a whole program when it contains `fn main`. A block
# without `fn main` is wrapped in one, so fragments are still type-checked.
# To exempt a block that is deliberately showing something broken or aspirational,
# put the word "no-compile" on the fence:  ```palladium no-compile
#
# Usage: scripts/check-docs.sh [path ...]        (default: docs README.md)

set -uo pipefail
cd "$(dirname "$0")/.."

PDC=./target/release/pdc
TARGETS=("${@:-docs README.md}")

# Directories whose snippets are deliberately not expected to compile.
# They are printed, never silently dropped — a hidden exclusion is how
# documentation drifts away from the compiler in the first place.
EXCLUDE_DIRS=(
  "docs/design"                 # PROPOSAL documents: designs that were never built
  "docs/internals/bootstrap"    # historical narrative of earlier bootstrap attempts
)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

[ -x "$PDC" ] || { echo "error: $PDC not built (cargo build --release)" >&2; exit 2; }

pass=0; fail=0; skipped=0
declare -a FAILED

extract_and_check() {
  local file="$1"
  awk -v out="$WORK" -v base="$(echo "$file" | tr '/.' '__')" '
    /^```palladium[[:space:]]*no-compile/ { skipping=1; next }
    skipping && /^```[[:space:]]*$/       { skipping=0; next }
    skipping                              { next }
    /^```palladium[[:space:]]*$/ {
      inblock=1; n++;
      fname = out "/" base "_" n ".pd";
      startline = NR;
      print startline > (fname ".line");
      close(fname ".line");
      next
    }
    inblock && /^```[[:space:]]*$/ { inblock=0; close(fname); next }
    inblock { print > fname }
  ' "$file"
}

is_excluded() {
  local f="$1"
  for d in "${EXCLUDE_DIRS[@]}"; do
    case "$f" in "$d"/*) return 0 ;; esac
  done
  return 1
}

excluded_files=0
for target in ${TARGETS[@]}; do
  while IFS= read -r f; do
    if is_excluded "$f"; then
      excluded_files=$((excluded_files+1))
      continue
    fi
    extract_and_check "$f"
    # grep -c exits 1 with a count of 0 when nothing matches, so capture first.
    nc=$(grep -c '^```palladium[[:space:]]*no-compile' "$f" 2>/dev/null) || nc=0
    skipped=$((skipped + nc))
  done < <(find "$target" -name '*.md' 2>/dev/null | sort)
done

if [ "$excluded_files" -gt 0 ]; then
  echo "excluded $excluded_files file(s) under: ${EXCLUDE_DIRS[*]}"
  echo
fi

shopt -s nullglob
for snippet in "$WORK"/*.pd; do
  name=$(basename "$snippet" .pd)
  line=$(cat "$snippet.line" 2>/dev/null || echo "?")
  src=$(echo "$name" | sed 's/_[0-9]*$//' | tr '_' '/')

  # A fragment without a main is still worth type-checking: wrap it.
  if ! grep -qE '^[[:space:]]*fn[[:space:]]+main[[:space:]]*\(' "$snippet"; then
    { cat "$snippet"; printf '\nfn main() {\n}\n'; } > "$snippet.wrapped"
    mv "$snippet.wrapped" "$snippet"
  fi

  if out=$("$PDC" compile "$snippet" -o "doccheck_$name" 2>&1); then
    printf '%-56s %s\n' "$src (block near line $line)" "OK"
    pass=$((pass+1))
  else
    reason=$(echo "$out" | grep -oE 'error[^\n]{0,90}' | head -1)
    printf '%-56s %s\n' "$src (block near line $line)" "FAIL"
    FAILED+=("$src near line $line :: $reason")
    fail=$((fail+1))
  fi
done

echo
echo "=========================================="
echo "doc snippets: pass=$pass fail=$fail skipped(no-compile)=$skipped"
echo "=========================================="

if [ ${#FAILED[@]} -gt 0 ]; then
  echo
  echo "Snippets the compiler rejects — each one is a documentation claim that is not true:"
  printf '  %s\n' "${FAILED[@]}"
fi

[ "$fail" -eq 0 ]
