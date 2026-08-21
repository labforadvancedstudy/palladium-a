# Palladium (palladium-a)

Systems language project: Rust로 짠 컴파일러(`pdc`)가 Palladium(.pd)을 C로 트랜스파일 → gcc 링크.
**현재 목표: 언어 스펙 정리 → 그 스펙대로 진짜 셀프컴파일(자기 소스를 자기가 컴파일) 달성.**

## User special commands

명령 받으면 "얍!" 하고 시작. Ultrathink what to do.

- `1`/`t`: build, lint, test 돌리고 성공 확인. warning까지 수정.
- `2`/`c`: TODO 확인하고 이어서 진행. 없으면 만들어서 진행.
- `3`/`r`: status report.
- `4`/`m`: 버드아이로 마일스톤 분석 + 다음 마일스톤 제안.
- `5`/`p`: changeset 확인, 커밋 만들고 push.

## Layout (verified 2026-08-21)

```
src/                # Rust 구현 컴파일러 (pdc/pdm/pls) — 현재 유일한 동작 컴파일러
docs/specification/ # 스펙 SSOT: language_specification.md, grammar.ebnf, semantics.md
docs/reference/     # LANGUAGE_REFERENCE.md 등 (스펙과 중복/드리프트 있음)
bootstrap/
  v1_archive/       # 초기 시도 아카이브 (역사 자료, 건드리지 않음)
  v2_full_compiler/ # .pd로 짠 "풀" 컴파일러 1,540줄 (lexer/parser/ast/codegen/pdc)
  v3_incremental/   # tiny_v1~v16 증분 컴파일러 50여 개 (대부분 실험 잔해)
stdlib/             # prelude.pd + std/
examples/           # tutorial/01_variables.pd ~ 06_arrays.pd, practical/
tests/              # Rust 테스트 + .pd 테스트
```

## Build & smoke

```bash
cargo build --release          # green (2026-08-21 실측, 29s)
./target/release/pdc compile examples/tutorial/01_variables.pd -o vars
```

## Current real status (2026-08-21 실측)

게이트: `make conformance` (언어 표면) · `make selfhost` (셀프호스팅 fixed point) · `make test-honest`.

현재 수치 (2026-08-21, 원본 커밋 f323cf1과 대조):

| 게이트 | 원본 | 현재 |
|---|---|---|
| selfhost | 불가 (링크조차 안 됨) | **green** (fixed point) |
| conformance | 실행 가능 0건 | 39 pass / 3 fail / 2 skip |
| lib 테스트 | 377 pass · 2 fail | 398 pass · 2 fail |
| 통합 테스트 (tests/*.rs) | 57 fail | 41 fail |

**게이트 맹점 주의**: `make test-rust`는 `cargo test --lib --bins`라 `tests/*.rs`를 **아예 실행하지 않는다**.
그래서 통합 테스트 41건 실패가 1년간 안 보였다. 진짜 상태는 `make test-honest`(`--no-fail-fast`)로만 보인다.
남은 41건은 전부 기존 결함 — 미구현 기능(closures·const generics·async) 테스트이거나,
`target/build/` vs 드라이버의 `build_output/` 경로 불일치다. 이번 세션 회귀는 0건.

**셀프호스팅 달성 (2026-08-21).** `bootstrap/pdc.pd`(~760줄, PBS-1)가 자기 소스를 컴파일하고
stage1·stage2 출력이 바이트 동일(`9b0cf24e…`). 데모가 아니라 fixed point다.
서브셋 스펙 = `docs/specification/bootstrap-subset.md`.

**고쳐진 것 (이 세션):**
- D1 링킹 — `runtime/palladium_runtime.c`가 레포에 **한 번도 존재한 적 없었다**(git 전 이력 확인).
  근본 원인은 `.gitignore`의 무차별 `*.c`. 런타임 작성 + negation 추가 → .pd가 처음으로 실행파일이 됨.
- D2 빌트인 드리프트 — typeck 36개 vs borrow checker 25개. `src/builtins.rs`를 SSOT로 만들고
  두 패스가 파생하도록 변경(중복 등록 400줄 삭제 + 드리프트 테스트).
- D3 tail return — `fn add(a,b) -> i64 { a + b }`가 **에러 없이 쓰레기값 반환**(생성 C에 return 누락).
  파서에서 `Stmt::Return`으로 lowering. 피해자는 일반 사용자 프로그램이었다.

  > **정정 1 (2026-08-22) — D3는 "완료"가 아니라 절반만 고쳐졌다.** 파서는 tail *expression*만
  > lowering하고 tail `if`는 하지 않는다. 실측:
  > `fn fib(n: i64) -> i64 { if n <= 1 { n } else { fib(n-1) + fib(n-2) } }` → 진단 없이 컴파일,
  > `fib(10)`이 55 대신 **8261746944** 반환, exit 0. 재귀 base case의 자연스러운 형태가 전부 해당된다.
  > 이 줄이 D3를 done으로 기록해 온 것이 이 결함이 숨어 있던 이유다.
  > 고정 = `tests/stdlib/stdlib_tail_if_defect.pd` + `make stdlib-gate`의 생성-C 구조 불변식
  > (`scripts/gate_probe.py generated-c`: 모든 non-void 함수는 **모든 경로에서** return해야 한다).
  > 파서 수정은 별도 작업 단위 — 이 브랜치는 결함을 고정만 하고 고치지 않는다.
  >
  > **정정 2 (2026-08-22)**: 이 줄은 원래 "stdlib 전체가 조용히 miscompile되고 있었다"였다 — 거짓.
  > `make stdlib-gate` 실측: `stdlib/` 21개 파일 중 **0개**만 컴파일된다(전부 lex/parse에서 거부되어
  > D3가 살던 codegen까지 도달조차 못 한다). 기본 설정에서 로드되지도 않는다. 컴파일된 적이 없으니
  > miscompile된 적도 없다. 참인 것은 반사실뿐: tail expression으로 끝나는 함수가 ~437개 있고,
  > tail `if`로 끝나는 함수는 그 수에 **포함되지 않은** 별도 집합이다 (즉 반사실 피해 범위는 437보다
  > 넓다). 측정 전문 = [`stdlib/STATUS.md`](stdlib/STATUS.md).

**남은 결함 (열림):**
- D4 `for`가 배열 **파라미터**를 순회할 때 decay된 포인터에 `sizeof` 사용 (`src/codegen/mod.rs:1553`).
- D5 `?` / `.await` — 정의되지 않은 C 타입/멤버를 참조하는 코드 생성 (에러 아님).
- D6 호출 인자 대여가 해제되지 않음 (`Lifetime::Named("fn")` vs `exit_scope`의 `Scope(n)`) →
  같은 값을 두 번 넘길 수 없고, 필드를 빌트인에 넘기면 이후 uninitialized 오판.

**허위였던 문서 주장 (교정 완료):** README "Bootstrap 100% Complete", FEATURES "Self-Hosting 100%",
`bootstrap/v3_incremental/BOOTSTRAP_ACHIEVED.md`. 어떤 Palladium 컴파일러도 자기 자신을 컴파일한
적이 없다. `v2_full_compiler/pdc.pd`는 자기 파서가 구현하지 않는 방언(if-expression·`matches!`·
`if let`)으로 쓰여 있어 원리적으로 불가능하다 (`parser.pd:178`).
`tests/07_traits_basic.pd`·`08_generics_basic.pd`는 "미구현"을 print만 하고 PASS하므로,
컨포먼스 green은 traits/generics의 증거가 **아니다**.

## Working rules

- 모든 상태 주장은 실행 증거(명령 출력, file:line)와 함께. 문서의 과거 주장을 근거로 쓰지 않는다.
- 스펙 SSOT는 `docs/specification/` — 구현과 충돌하면 스펙을 고치든 구현을 고치든 한쪽으로
  결정하고 기록한다. reference 문서는 스펙에서 파생.
- 셀프컴파일 게이트(순서대로): ① Rust pdc가 .pd→실행파일 end-to-end 복구
  ② 스펙의 "bootstrap subset" 확정 ③ 그 subset으로 짠 컴파일러가 Rust pdc로 컴파일됨
  ④ 그 산출물이 자기 소스를 다시 컴파일 (fixed point).
- 루트에 파일/디렉토리 20개 초과하면 정리 제안.

# TESTING

- 결과 보여주기 전에 항상 테스트: `cargo check` / `cargo test`, Makefile은 `make --dry-run`.
