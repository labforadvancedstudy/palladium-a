#!/usr/bin/env bash
# Regenerate runtime/pd_prelude.h from what the Rust pdc emits.
#
# The Rust compiler inlines its C runtime (print, string ops, file wrappers,
# argv capture) into every generated file. The Palladium-written bootstrap
# compiler cannot carry ~290 lines of C in string literals, so it emits
# `#include "pd_prelude.h"` instead. This script keeps that header in sync.
#
# Run after any change to the prelude in src/codegen/mod.rs.

set -euo pipefail
cd "$(dirname "$0")/.."

PDC=./target/release/pdc
PROBE=/tmp/pd_prelude_probe.pd

[ -x "$PDC" ] || { echo "error: $PDC not built (cargo build --release)" >&2; exit 2; }

# A probe that forces the whole prelude to be emitted.
cat > "$PROBE" <<'EOF'
fn main() {
    let s: String = "x";
    print(s);
    print_int(string_len(s));
    print_int(arg_count());
}
EOF

"$PDC" compile "$PROBE" -o pd_prelude_probe >/dev/null 2>&1 || {
  echo "error: probe failed to compile" >&2; exit 1; }

GEN=build_output/pd_prelude_probe.c
[ -f "$GEN" ] || { echo "error: $GEN not produced" >&2; exit 1; }

# The prelude ends at the last line before the first user-level declaration.
# User code starts at the generated `int main(`.
END=$(grep -n '^int main(' "$GEN" | head -1 | cut -d: -f1)
[ -n "$END" ] || { echo "error: cannot locate user code boundary" >&2; exit 1; }
END=$((END - 1))

{
  echo "// Palladium C prelude — runtime helpers emitted by the Rust pdc, extracted so that"
  echo "// the Palladium-written bootstrap compiler can emit '#include \"pd_prelude.h\"'"
  echo "// instead of re-emitting the whole runtime from string literals."
  echo "// GENERATED — do not edit by hand. Regenerate with: scripts/gen-prelude.sh"
  echo "#ifndef PD_PRELUDE_H"
  echo "#define PD_PRELUDE_H"
  sed -n "1,${END}p" "$GEN"
  echo "#endif // PD_PRELUDE_H"
} > runtime/pd_prelude.h

echo "wrote runtime/pd_prelude.h ($(wc -l < runtime/pd_prelude.h) lines, from $GEN:1-$END)"

# Prove it is usable on its own.
cat > /tmp/pd_prelude_check.c <<'EOF'
#include "pd_prelude.h"
int main(int argc, char** argv) {
    __pd_argc = argc; __pd_argv = argv;
    __pd_print("prelude ok");
    __pd_print_int(__pd_string_len("abcd"));
    return 0;
}
EOF
gcc -Iruntime /tmp/pd_prelude_check.c runtime/palladium_runtime.c -o /tmp/pd_prelude_check
/tmp/pd_prelude_check
