# 02 — 아키텍처: 지금 있는 컴파일러

Status: shipped
Measured: 2026-11-10, main `f2d1dd3`

이 문서는 구현된 계약을 현재형으로 적는다. 미래 목표가 아니다. 목표 쪽 구조(effects·totality·
region)는 아직 코드가 없고 `01-spec.md`와 MILESTONES.md M5–M7이 소유한다.

## 파이프라인

`src/driver/mod.rs`의 `compile_string`이 아래 순서로 한 번 지나간다.

```text
.pd 소스
  → Lexer            src/lexer        (952줄)
  → Parser           src/parser       (5,921줄)   tail 식을 return으로 낮춘다
  → MacroExpander    src/macros       (1,652줄)   토큰 템플릿 치환, hygiene 없음
  → TypeChecker      src/typeck       (11,384줄)  제네릭 인스턴스 기록을 만든다
  → BorrowChecker    src/ownership    (3,997줄)
  → EffectAnalyzer   src/effects      (490줄)     effect를 출력만 한다 (거부 규칙 없음)
  → UnsafeChecker    src/unsafe_ops   (389줄)
  → Optimizer        src/optimizer    (3,793줄)
  → CodeGenerator    src/codegen      (15,089줄)  C 텍스트를 문자열로 이어 붙인다
  → write_output → gcc(-O2) + runtime/palladium_runtime.c  src/linker.rs
```

- 동작하는 백엔드는 C 하나다. `--llvm`은 `LLVMTextBackend`로 가고, 그 백엔드는 프로그램을
  방출하지 않고 거부한다 (`src/codegen/llvm_text_backend.rs`).
- 런타임은 C 파일 하나(853줄)다. 문자열은 arena에 쌓고 `atexit`까지 해제하지 않는다.
- GC 없음, RTTI 없음. 배열 경계 검사를 방출하지 않는다.
- `bootstrap/pdc.pd`(1,016줄)는 Palladium으로 쓴 컴파일러이고, PBS-1 서브셋만 구현한다
  (`docs/specification/bootstrap-subset.md`). stage1·stage2 출력이 바이트 동일하다.

## 크기 (추적 파일, 줄 수)

| 범주 | 줄 | 내용 |
|---|---:|---|
| 코드 | 69,621 | `src/**/*.rs` (LSP 6,613 · package 2,986 · async_runtime 498 포함) |
| 테스트 | 25,970 + 11,708 | `tests/**/*.rs` + `.pd` fixture 221개 |
| 문서 | 21,942 | `docs/**/*.md`, 루트 `*.md` |
| 운영 | 29,550 | `scripts/*`, `Makefile` — 게이트와 게이트의 자기 시험 |

게이트 기계(운영)가 컴파일러 코드의 42% 크기다. 이 저장소가 "문서가 거짓말을 했다"는 사건을
겪은 뒤 만든 방어선이고, 변경 한 건마다 인용 줄번호·핀·수치 문구를 다시 맞추는 비용을 만든다.

## 게이트

`make gates` = 17개 타깃. 로컬이 정본이다(CI는 결제 잠금으로 돌지 않는다).

```text
conformance · test-conformance-runner · check-diagnostic-codes · check-docs · gate-receipts ·
test-doc-evidence · selfhost · stdlib-gate · test-gate-probe · version-gate · test-version-gate ·
version-source-gate · check-retracted-claims · test-thesis-runner · test-xfail ·
test-requirements-runner · test-honest
```

- 한 번 도는 데 부하가 없으면 약 30분, 부하 평균 30–50에서는 60–75분이다 (2026-11-10 실측).
- 문서 게이트가 읽는 범위는 루트 `*.md`, `docs/**/*.md`, `docs/**/*.toml`, `src/**/*.rs`,
  `tests/**/*.rs`다. `.prd/`는 이 범위 밖이다 (`scripts/check_doc_evidence.py`의 glob).
- 진단 코드는 `PD####`이고 `scripts/check-diagnostic-codes.sh`가 등록부·핀·fixture를 대조한다.
  main의 마지막 코드는 PD0081이다.

## 경계에서 실측된 사실

이슈 시리즈 #84–#93(`issue-series-84-93/`)이 전부 같은 경계에서 나왔다. 자세한 진단은
`03-typeck-codegen-seam.md`.

1. **타입 체커와 코드젠 사이에 타입이 붙은 중간 표현이 없다.** 코드젠은 AST를 다시 걸으며
   C 타입을 스스로 추론한다. 체커가 본 타입과 코드젠이 방출한 타입이 다를 수 있다.
2. **체커는 모든 정수를 `i64`로 기록한다** (issue #82). 폭(`u32`, `u64`)은 코드젠이 C 철자에서
   되찾는다.
3. **체커에서 코드젠으로 가는 정보 일부가 평문 문자열이다** — 제네릭 인스턴스의 타입 인자,
   struct 리터럴 기록.
4. **C 이름은 소스 이름을 이어 붙여 만든다.** main에서는 `id__S`, `E_V`, `Box_i64` 꼴이고,
   소스가 같은 이름을 정의하면 충돌한다.
5. **잘못된 C의 최종 판정자는 gcc다.** 체커가 통과시킨 프로그램이 gcc에서 실패하거나
   ICE 트랩("the C backend emitted ill-typed C")에 걸리는 경로가 남아 있다.
