# 01 — Palladium: 무엇을, 왜, 어디까지

Status: in-progress
Measured: 2026-11-10, main `f2d1dd3` (v0.5.0 이후 3개 유닛 머지 시점)

> 이 문서는 요약과 포인터다. 정본은 따로 있고 여기서 복제하지 않는다.
> - 순서·마일스톤: `docs/contributing/MILESTONES.md`
> - 1.0 요구사항 202행: `docs/contributing/1.0-requirements.tsv`
> - 언어 정의(Part I)와 구현 현황(Part II): `docs/specification/language-spec.md`

## 문제

Rust는 메모리 안전을 주지만 세 가지 비용을 사용자에게 넘긴다: 함수 색깔(`async`/`.await`),
종료 여부를 말하지 못하는 타입 시스템, 수명 표기(`'a`). Palladium은 이 셋을 컴파일러의 일로
옮기려는 시스템 언어다 (language-spec N1).

## 목표 — 세 가지 차별점

| # | 차별점 | 정의 | 소유 마일스톤 | 2026-11-10 상태 |
|---|---|---|---|---|
| 1 | 비동기는 색깔이 아니라 effect | `async`/`.await` 없음. effect는 추론되고 호출자에게 전파 (N7) | M5 | 미구현 |
| 2 | 종료 증명 | `#[total]`을 컴파일러가 방출 전에 증명 (N8) | M6 | 미구현 (속성이 렉싱되지 않음) |
| 3 | 수명 추론 | `ref`/`ref mut`, `'a` 파라미터 목록 없음, 모호하면 이름 붙여 거부 (N9) | M7 | 미구현 (`ref`는 키워드가 아님) |

## 끝 상태 — 1.0의 정의

1.0 = `make thesis-exit` green. 네 조건(MILESTONES.md "What 1.0 is"):

1. `make selfhost` green **이면서** `bootstrap/pdc.pd`가 세 차별점의 방언으로 쓰여 있다.
2. 차별점마다 transcript가 있는 non-vacuous conformance fixture 1개.
3. 차별점마다 reject twin 1개 (증명 실패, 모호한 region, 게이트 없는 effect 탈출).
4. 두 번째 증인 프로그램(JSON 파서)이 같은 세 조건을 만족.

## 비목표 (1.0 밖, `post-1.0`로 기록됨)

패키지 레지스트리 · WASM · LSP 서버 · 디버거 · 포매터 · Rust FFI · Lean/Coq 증명 export ·
refinement types · side-channel bounds · parallel-by-default. C FFI는 1.0 안이다.

## 인수 시나리오 (실행 → 기대 관측치)

| 명령 | 기대 | 2026-11-10 실측 (`f2d1dd3`) |
|---|---|---|
| `make gates` | 마지막 줄 `✓ all gates green` | green (`make gates exit=0`, 유닛 #84 랜딩 게이트) |
| `make selfhost` | stage1·stage2 C 바이트 동일 | fixed point `e8bd8cda…` |
| `make conformance` | `failures=0`, `untranscribed=0` | `verified=92 vacuous=6 xfail=6 reject=125 skip=2 failures=0` (231 fixtures) |
| `make thesis-exit` | exit 0 | exit 2 `NO_VERDICT` (issue #48 뒤에 GI-11) |
| `make stdlib-gate` | stdlib 파일 컴파일 | 21개 중 0개 컴파일 |

## 현재 위치

| 항목 | 값 | 출처 |
|---|---|---|
| 릴리스 | v0.3.0 (2026-08-22) · v0.4.0 (2026-08-23) · v0.5.0 (2026-11-09) | `git tag` |
| 닫힌 마일스톤 | M1, M2 | MILESTONES.md "Completed" |
| 남은 마일스톤 | M3 traits/generics · M4 modules · M5 effects · M6 totality · M7 regions · M8 stdlib+FFI · M9 thesis | MILESTONES.md |
| 1.0 요구사항 | 충족 82 · 남음 112 · 막힘 8 (202행) | `1.0-requirements.tsv` |
| 남은 행의 소유 | M3 25 · M4 8 · M5 16 · M6 12 · M7 19 · M8 18 · M9 16 · P1 6 | 같은 파일, 열 2·7 |
| GitHub 이슈 | 56개 중 열림 42 · 닫힘 14 | `gh issue list --state all` |

M1·M2는 "있는 것을 정직하게 만드는" 마일스톤이었다. 세 차별점은 전부 M5 이후에 있고,
그 앞에 M3(traits·generics)와 M4(modules)가 선행 조건으로 놓여 있다.

## 워크스트림

| 폴더 | 내용 | 상태 |
|---|---|---|
| `issue-series-84-93/` | v0.5.0 직후 hand-off 이슈 10개(#84–#93) 순차 처리 | 일시 중지 (2026-11-10, 4개 이슈 머지·6개 진행 중) |

판단 근거 문서: `03-typeck-codegen-seam.md` (이 시리즈가 드러낸 구조 문제와 결정 선택지).
재현 프로그램: `research/found-defects/`.
