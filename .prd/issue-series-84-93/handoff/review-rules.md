너는 trinity 패널리스트 1인이다. 다른 에이전트·패널·스킬을 스폰하지 마라. 합의는 dispatcher가 라운드로 만든다. 주어진 증거만으로 단독 판정을 내라.

# Common review rules — palladium-a issue series #84–#93

Owner's instruction (verbatim, 2026-11-09): "여기서 생성한 모든 이슈 각각 순차적으로 모두 처리하고 머지해줘" (the ten follow-up issues #84–#93).
Each unit: a branch from main, a commit by the orchestrator closing its issue(s), a dispatcher `make gates` on a clean clone of the exact
tip, unanimous trinity APPROVE, then fast-forward to main.

Merge criteria (main = the branch's parent):
- The issue's defect is fixed for its whole CLASS, or each remaining site has a measured reason; the issue's own repro behaves as the
  unit claims.
- No program that compiled and ran on main changes output, and none stops running, unless the unit deliberately refuses that
  construct BY NAME and says so (a refusal of something that only ever failed in gcc or ran wrong is not a lost run).
- No new silent wrong output, no new ICE, no hang. A named refusal beats a guess.
- Tests: a corpus fixture (run/reject) and Rust tests that fail on main and pass on the tip.
- Docs, counts, pins and ledgers stay true (gates check most; read the prose the diff touches).
Probe yourself; do not trust the coder's report. Every pdc probe under `perl -e 'alarm 20; exec @ARGV' <pdc> run x.pd` with
PALLADIUM_RUNTIME=<tree>/runtime. Build your own compiler from a `git archive` of the tip into your probe dir; main's compiler is
$W/gates-rel/target/release/pdc (read-only). No builds/edits/commits in the
unit's worktree. No rm -r.

Keep your final message SHORT (≤ 350 words before the contract): findings with file:line and measured evidence only.
Answer contract — end with exactly:
VERDICT: <APPROVE | REJECT>
MUST-FIX: <blocking items with file + evidence line, or "none">
근거: <≤3 key reasons, each with evidence location and a grade [실측|추론]>

NOTE (load): `alarm 20` kills pdc but NOT the program it spawned — a hanging compiled program (e.g. target/build/recursive_generic) keeps running as an orphan and burned 2 cores for 90 min. After a probe hits the alarm (rc 142), kill any leftover target/build/<name> process from your probe dirs (`pkill -f "target/build/<name>"`).
