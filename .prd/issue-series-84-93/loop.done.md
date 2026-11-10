# issue-series-84-93 — done (append-only)

Loop: ./loop.md · Each round close appends one block at the bottom; nothing above it ever changes (this skill's `scripts/loop-lint.py` C2 compares against git). Rows here are snapshots (Last Measured frozen at move time). A later pass is a `re-verified` line in a loop.md round section; a regression reopens as a new loop.md row with Source `regression:<ID>`.

## R1 — closed 2026-11-09

- measured 1✅ 0🟡 0🔴 0🚪 0⚫ · result: 유닛 #85 머지 · closed rows: U85 · commit/PR: main `d5166e8` → `903ba02` (fast-forward, 2커밋)

### Closed rows

| ID | Source | Universe Item | Observable Acceptance | AS-IS Evidence | Status | Gap | Dispatch | Verification Evidence | Last Measured |
|---|---|---|---|---|---|---|---|---|---|
| U85 | issue #85 | 매크로는 free function이 확장되는 모든 자리(메서드, trait 기본 구현, import)에서 확장된다. 파싱되지 않는 확장은 호출 자리에서 PD0079로 거부 | 리뷰 3명 승인 · `make gates` exit=0 (exact tree) · issue CLOSED | main `d5166e8`: impl 메서드 본문의 매크로가 확장되지 않음 | ✅ | 없음 | 코더 1 + 리뷰 2라운드 | `903ba02` 게이트 로그 마지막 줄 `✓ all gates green` / `make gates exit=0` · issue #85 CLOSED 2026-11-09T11:44:31Z | 2026-11-09 |

### Round section (as it was in loop.md)

- 계획: #85를 첫 유닛으로. 원인 = 확장기가 최상위 함수만 방문.
- 로그: 구현 `32b1efd` → 리뷰 1차 반려(파싱되지 않는 확장이 이름 없이 실패) → `903ba02` → 2차 만장일치 → clean clone 게이트 green → main ff.
- 변경량: 코드 +214/−37 · 테스트 +768/−42 · 문서 +5/−5 · 운영 +17/−14.
- 새 진단 코드: PD0079.

## R2 — closed 2026-11-10

- measured 1✅ 0🟡 0🔴 0🚪 0⚫ · result: 유닛 #93+#91(+#79) 머지 · closed rows: U93 · commit/PR: main `903ba02` → `2ca5eb5` (fast-forward, 5커밋)

### Closed rows

| ID | Source | Universe Item | Observable Acceptance | AS-IS Evidence | Status | Gap | Dispatch | Verification Evidence | Last Measured |
|---|---|---|---|---|---|---|---|---|---|
| U93 | issue #93 #91 #79 | 제네릭 인자는 스코프 안의 const 파라미터를 가리킬 때만 const다(철자로 판정하지 않는다). 별칭 순환은 쓰인 자리에서 PD0080, 한 파라미터를 두 번 이름 붙인 제네릭 목록은 PD0081 | 리뷰 3명 승인 · `make gates` exit=0 (exact tree) · issue 3개 CLOSED | main `903ba02`: `Wrap<K>`의 `K`를 const로 읽음 · 진단에 `const A` 표시 · 순환 별칭에서 컴파일러 멈춤 | ✅ | 없음 | 코더 1 + 리뷰 5라운드 | `2ca5eb5` 게이트 로그 `✓ all gates green` / `make gates exit=0` · #93 CLOSED 2026-11-09T19:47:01Z · #91 19:47:03Z · #79 19:47:04Z | 2026-11-10 |

### Round section (as it was in loop.md)

- 계획: #93과 #91은 원인이 같다(제네릭 인자의 종류를 철자로 판정). #79(순환 별칭 멈춤)는 같은 별칭 확장 코드를 지나므로 같이 닫는다.
- 로그: `37e1195` → 리뷰 1–4차 반려(별칭 순환, 실제 별칭 확장의 구조 가드, 제네릭 별칭 순환의 경로, 중복 바인더) → `2ca5eb5` → 5차 만장일치.
- 게이트 1회 red: m2-exit inventory가 `pub fn compile` 진입점 변경을 잡음 → 진입점을 제자리에 복원 → green.
- 변경량: 코드 +642/−155 · 테스트 +935/−41 · 문서 +113/−109 · 운영 +236/−226.
- 새 진단 코드: PD0080, PD0081.

## R3 — closed 2026-11-10

- measured 1✅ 0🟡 0🔴 0🚪 0⚫ · result: 유닛 #84 머지 · closed rows: U84 · commit/PR: main `2ca5eb5` → `f2d1dd3` (fast-forward, 8커밋)

### Closed rows

| ID | Source | Universe Item | Observable Acceptance | AS-IS Evidence | Status | Gap | Dispatch | Verification Evidence | Last Measured |
|---|---|---|---|---|---|---|---|---|---|
| U84 | issue #84 | 튜플 참조 파라미터(`&(A, B)`, `&mut (A, B)`, 튜플 별칭의 참조)는 튜플 C struct의 포인터다. 공유 튜플 참조를 쓰는 쪽에 빌려주면 거부. 자기 이름을 읽는 `let`은 임시 변수를 거쳐 바깥 바인딩을 읽는다 | 리뷰 3명 승인 · `make gates` exit=0 (exact tree) · issue CLOSED | main `2ca5eb5`: 호출되지 않는 `&TupleAlias` 함수가 "Unsupported type in reference parameter"로 거부됨 · `let x = x + 1`이 −16 출력 | ✅ | 없음 | 코더 1 + 리뷰 8라운드 | `f2d1dd3` 게이트 로그 `✓ all gates green` / `make gates exit=0` · issue #84 CLOSED 2026-11-10T01:03:29Z | 2026-11-10 |

### Round section (as it was in loop.md)

- 계획: 튜플 참조 파라미터를 포인터로 낮춘다. 낮추면서 생기는 쓰기 권한 문제를 같이 닫는다.
- 로그: `eabed4b` → 리뷰 1–7차 반려 → `f2d1dd3` → 8차 만장일치 → 게이트 green(부하 평균 30–70에서 약 85분) → main ff.
  - 1–4차: 공유 참조를 쓰는 파라미터에 빌려주는 경로들(철자, 지역 참조, 바인딩 단위 판정).
  - 5–6차: 자기 이름을 읽는 `let`. main에 있던 결함을 원인에서 고침(초기화식이 새 바인딩을 읽어 쓰레기값·크래시).
  - 7차: 임시 변수 이름이 enum 생성자 `__pd_shadow0`와 충돌 → 방출된 C 전체에 없는 이름으로 사후 명명.
- main에서 돌다가 이제 이름 붙여 거부되는 프로그램: 26개(main에서 틀린 값 11, 의도된 거부 15).
- 변경량: 코드 +1,356/−43 · 테스트 +1,061/−20 · 문서 +58/−48 · 운영 +108/−107.
- 새 진단 코드: 없음.
