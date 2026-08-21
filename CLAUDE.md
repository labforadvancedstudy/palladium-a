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
  파서에서 `Stmt::Return`으로 lowering. 피해자는 일반 사용자 프로그램이었다 — `stdlib/`가 아니다
  (21개 중 0개만 컴파일되므로 애초에 컴파일된 적이 없다). **tail `if`는 아직 미수정**:
  `fib(10)`이 55 대신 8261746944 반환. 고정 = `make stdlib-gate`의 생성-C 구조 불변식.

- D4 `for`가 배열 **파라미터**를 순회할 때 decay된 포인터에 `sizeof` 사용. 고쳐짐 — 경계가
  선언된 길이에서 온다. 코드젠이 증명 못 하는 길이는 잘못된 경계가 아니라 컴파일 에러.
- D5 `?` / `.await`가 정의되지 않은 C 타입/멤버를 참조하는 코드 생성. 고쳐짐 — 둘 다
  "is not implemented"로 거부되고, 결과와 함께 컴파일·실행되는 workaround를 제시한다.
  LLVM 백엔드는 더 나빴다(catch-all이 상수 `0` 반환) → `--llvm` 전면 거부.
- D9 `&[T; N]` / `&mut [T; N]` 파라미터를 코드젠이 거부. 고쳐짐 — 배열 파라미터에 C가 주는
  decay된 포인터로 lowering, `&`는 원소 슬롯을 const 한정. 공유/맨 배열 파라미터로의 쓰기와
  그것을 쓸 수 있는 파라미터로 넘기는 것은 컴파일 에러 (`language-spec` §A9.2).

**남은 결함 (열림):**
- **D3b tail `if`** — 파서가 tail *expression*은 `Stmt::Return`으로 낮추지만 tail `if`는 안 한다.
  `fn fib(n) -> i64 { if n <= 1 { n } else { … } }`가 깨끗이 컴파일되고 `fib(10)`이 55 대신
  8261746944를 반환. `make stdlib-gate`의 생성-C 구조 불변식이 `known_violation`으로 고정 중.
- **컴파일 불가능한 빌트인 6개** — `file_flush`·`file_seek`·`file_open_ex`·`file_close_ex`·
  `file_read_ex`·`file_write_ex`. 핸들 표현이 둘로 갈렸다(레거시=인덱스, 확장=`FileHandle`=`void*`).
  경계에서 캐스팅하면 `file_seek(file_open(p), 0, 0)`이 컴파일되고 정수 `1`을 `FILE*`로 역참조하므로
  **gcc 에러가 지금 segfault를 막는 유일한 장치다.** typeck이 `Support::Unsupported`로 먼저 거부 중.
  결정 필요: 확장 계열을 인덱스 테이블로 재기반할 것인가, `BUILTINS`에서 지울 것인가.
- C 키워드 식별자(`fn double`)가 유효하지 않은 C를 생성. `self`/제네릭 `impl`이 정의되지 않은 C.
  `Type::method(args)`가 codegen이 emit한 적 없는 `Type_method__new`로 조용히 lowering.
- 중첩 배열이 로컬·파라미터 양쪽에서 불가 (`type_to_c`가 선언자가 아니라 타입으로 `T[M][N]` 구성).

**D6은 결함이 아니었다 (철회).** `CLAUDE.md`가 열린 결함으로 올려뒀으나 `191f8c1`에서 이미 고쳐졌고,
이 파일의 베이스보다 12커밋 앞선다. 명시된 다섯 프로그램 전부 재실행 — 하나도 재현되지 않는다
(`t(s); t(s)` → `5 5`, `take2(s,s)` → `10`, `bump(&mut p)` 연속 → `2`, 필드→빌트인 후 재사용 → `abc 3 1`).
호출 경로 `src/ownership/borrow_checker.rs:505-521`이 per-call lifetime을 만들고 인자 검사 후 끝낸다.
인용됐던 `:236`은 대여된 **반환값**의 소유권 분류이지 인자 lifetime이 아니다 — green 핀이 붙은 채로
거짓 주장을 뒷받침하고 있었다.

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
