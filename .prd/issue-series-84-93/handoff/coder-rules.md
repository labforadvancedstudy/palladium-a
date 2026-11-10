# Common rules — palladium-a issue series #84–#93 (read with your issue brief)

Repo: labforadvancedstudy/palladium-a, a C-transpiling self-hosting language; Rust compiler `pdc` (src/). Gates: `make gates` is the
local source of truth (CI is billing-locked). main = the parent of your worktree.

- **No commit, no push, no cargo fmt.** Work ONLY in your worktree. Scratch = your work dir (given in the brief). Keep a ledger
  <work>/LEDGER.md (3-6 lines per step).
- **RED first**: a test that pins the defect, quoted RED on the unmodified tree, before any fix.
- **Fix the CLASS, not the instance**: the brief names a defect class. Find and measure every site of the class; fix every reachable
  one in this unit; give a one-line MEASURED justification for each site you leave. A defect of a DIFFERENT class you find: write a
  repro to <work>/found/ and report it with file:line — do not fix it.
- **No new silent wrong output and no lost runs**: a program that compiled AND ran correctly on main must still do so (same stdout),
  unless the brief says the unit deliberately refuses that construct. Prefer a named refusal to a guess. Measure this with old-vs-new
  runs over your probe set (main's compiler: build it from `git -C <worktree> stash`-free means — use `git worktree add` of main into
  your work dir, or `git archive`, never touch the main checkout).
- **Corpus witness**: the defect's program as a conformance fixture: a `run` row with an `.expected` transcript for a construct that
  now works, or a `reject` row pinned `code=PD####[;msg~<fragment>]` for a construct that is now refused by name — in the manifest's
  established form (read neighbouring rows of tests/conformance-manifest.txt). untranscribed must stay 0. If you add a diagnostic code,
  follow scripts/check-diagnostic-codes.sh's rules (run it).
- **Rust tests**: add focused tests in an existing or new tests/*.rs file, in the file's established style.
- **Docs that count things are gated**: conformance counts in prose (`scripts/check-doc-evidence.sh` checks them), MILESTONES ledger
  (`python3 scripts/requirements.py --check-ledger`). Citation pins: only `bash scripts/check-doc-evidence.sh --update --allow-repin`
  (then a plain run), never hand-edit docs/citation-pins.tsv, and do it LAST after every other edit; if a re-pinned citation's claim is
  no longer true, fix the claim. Do not edit requirement acceptance text that is SHA-pinned (thesis_exit.py PINNED_ACCEPTANCE_*).
- Every probe `pdc compile`/`pdc run` runs under a timeout: `perl -e 'alarm 20; exec @ARGV' <pdc> run x.pd`
  (PALLADIUM_RUNTIME=<worktree>/runtime). rc 142 = hang, a finding.
- **Before you report**, run on your final tree: `cargo build --release`; `cargo test --release` for the test files you touched plus
  `--lib`; `bash scripts/conformance.sh` (or `make conformance`); `python3 scripts/requirements.py --check-ledger`;
  `bash scripts/check-doc-evidence.sh`; `bash scripts/check-diagnostic-codes.sh` if you touched diagnostics; `make selfhost`
  (fingerprint must stay e8bd8cda unless bootstrap/ changed), and `make gate-receipts` (docs/reference/features/feature-index.toml
  `gate:` result strings cite conformance counts like verified=/fixtures=/evaluated= and must be re-derived when you add a fixture).
  Best: run the full `make gates` on your final tree (~30 min) and quote its last line; the dispatcher re-runs it on a clean clone.
- Damage control: if a task-guard rejects tool calls, stop and return your state as text.
- You cannot write report files outside your work dir? You CAN write in your work dir. Your FINAL MESSAGE is the report, ≤ 600 words:
  RED then GREEN (quoted); the class table (site → fixed / left + measured reason); old-vs-new probe summary (counts: same output,
  newly refused by name, newly running, output changed — the last must be 0 or justified); checks run and their result lines; files
  changed; anything found (found/). Then create <work>/done (empty file).

NOTE (load): `alarm 20` kills pdc but NOT the program it spawned — a hanging compiled program (e.g. target/build/recursive_generic) keeps running as an orphan and burned 2 cores for 90 min. After a probe hits the alarm (rc 142), kill any leftover target/build/<name> process from your probe dirs (`pkill -f "target/build/<name>"`).
