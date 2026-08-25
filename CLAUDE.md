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
  (21개 중 0개만 컴파일되므로 애초에 컴파일된 적이 없다).
- **D3b tail `if` — 닫힘 (2026-08-22).** 파서가 함수 본문 tail `if`의 각 분기와 tail `match`의 각
  arm까지 재귀적으로 lowering (`src/parser/mod.rs`, `lower_tail_to_return`). `fib(10)` = 55,
  양쪽 arm에 `return` 방출. 값이 있는 분기와 없는 분기가 섞이면(대표적으로 `else` 없는 tail `if`)
  조용한 fall-through 대신 **거부**한다 (`CompileError::tail_value_not_on_every_path`).
  `known_violation` 핀이 XPASS로 인계를 걸어줬고, fixture는 `tests/stdlib/stdlib_tail_if.pd`
  (`clean`)로 승격 — 이제 `main`이 함수들을 실제로 호출하고 값을 단언한다.

- D4 `for`가 배열 **파라미터**를 순회할 때 decay된 포인터에 `sizeof` 사용. 고쳐짐 — 경계가
  선언된 길이에서 온다. 코드젠이 증명 못 하는 길이는 잘못된 경계가 아니라 컴파일 에러.
- D5 `?` / `.await`가 정의되지 않은 C 타입/멤버를 참조하는 코드 생성. 고쳐짐 — 둘 다
  "is not implemented"로 거부되고, 결과와 함께 컴파일·실행되는 workaround를 제시한다.
  LLVM 백엔드는 더 나빴다(catch-all이 상수 `0` 반환) → `--llvm` 전면 거부.
- D9 `&[T; N]` / `&mut [T; N]` 파라미터를 코드젠이 거부. 고쳐짐 — 배열 파라미터에 C가 주는
  decay된 포인터로 lowering, `&`는 원소 슬롯을 const 한정. 공유/맨 배열 파라미터로의 쓰기와
  그것을 쓸 수 있는 파라미터로 넘기는 것은 컴파일 에러 (`language-spec` §A9.2).

- **missing-return 진단 — 닫힘 (2026-08-23).** `fn get_value() -> i64 { }`가 조용히 컴파일되고
  호출부가 리턴 레지스터를 읽던 결함. 이 항목이 "본문 전체 흐름 분석이 필요하다"고 적어둔 것은
  과대평가였다 — D3b가 넣은 `returns_on_every_path`(`src/parser/mod.rs`)가 이미 같은 질문에
  답하고 있었고, 호출부가 그 `false`에 **아무것도 하지 않았을 뿐이다.** 값이 tail 위치에 쓰이지
  않아 "단서가 없다"던 그 단서는 **선언된 리턴 타입**이다. 이제 유닛(`-> ()`·생략)이 아닌 함수가
  닫는 중괄호에 도달할 수 있으면 거부한다 (`CompileError::missing_return`). 받아들이는 쪽 리시트
  = `tests/m1_missing_return.rs` (tail 식·tail `if`·한쪽 분기만 early return·`while true`·
  `panic`·유닛 두 철자). 추적 .pd 242개 전수 재컴파일: 수용 집합 변화 0.
- **C 키워드 식별자 — 닫힘 (2026-08-23).** `fn double(x: i64)`이 `long long double(...)`을 방출.
  함수 이름만이 아니라 **모든 식별자 위치**가 당했다(struct 태그·필드·파라미터·지역·enum). 코드젠
  진입 직전 AST에서 예약어를 이스케이프한다 (`src/codegen/c_ident.rs`, `double` → `double_`).
  이스케이프는 단사다 — `double_` → `double__` — 두 소스 이름이 한 C 이름이 되면 gcc의 시끄러운
  에러가 조용한 중복 정의로 바뀌기 때문. 라이브러리 이름(`strlen`)은 **범위 밖이고 그 사실이
  측정돼 있다**: gcc가 conflicting declaration으로 시끄럽게 거부하므로 silent miscompile이 아니다
  (`tests/m1_c_keyword_idents.rs`). 기존 .pd 128개 중 126개의 생성 C는 바이트 동일 —
  나머지 2개는 아래 union 멤버 개명이 유일한 차이다.
  **리뷰가 잡은 두 구멍, 둘 다 "보호한 것과 방출한 것이 다르다"의 같은 모양:**
  ① **파생 이름** — union 멤버가 `variant.name.to_lowercase()`라, AST 이스케이프는 원본 철자
  `Register`를 예약어 목록과 대조해 통과시키고 파생이 `register`를 만들었다. 파생도 단사여야
  한다(케이스 폴딩은 아니다: `Register`/`register` 두 variant가 같은 멤버로 붕괴).
  이제 파생 이름은 전부 이름 붙은 함수를 거치고 그 함수가 **자기 결과에** `c_ident`를 적용한다
  (`c_enum_payload_member`).
  ② **모노모피제이션 템플릿** — 타입체커가 **이스케이프 안 된** AST를 받으므로
  (`src/driver/mod.rs:109`) 제네릭 템플릿이 escape를 통째로 우회했다. 이제
  `set_generic_instantiations`/`set_generic_struct_instantiations`도 입구에서 이스케이프한다.
  ③ **GNU 대체 철자** — 목록이 `asm`·`typeof`·`inline`·`restrict`로 이미 GNU까지 뻗어 있으면서
  `__asm__`·`__inline__`·`__const__`·`__restrict__` 앞에서 멈춰 있었다. 같은 인벤토리의 누락이지
  내가 그은 경계가 아니다(경계는 `strlen` 쪽 = 라이브러리 티어). 목록은 이제 117개이고,
  **소속 판정이 실행된다**: 후보 코퍼스를 실제 `cc`에 한 이름씩 물어보고 거부당한 것이 목록에
  없으면 red (`the_reserved_list_covers_every_keyword_this_toolchain_has`). 반대 방향도 측정:
  `__label__`은 키워드지만 `__label`·`__extension`·`__func`·`__foo__`는 아니라서 그대로 나가야
  하고, 그래서 `is_escaped_or_reserved`가 밑줄을 **한 개씩** 벗기며 매 단계 조회한다(예전엔 한
  번에 다 벗겨서 `__label`까지 과잉 예약했다).
  이 세 가지의 공통 교훈이 **"지도는 영토가 아니다"** — 그래서 코드젠 진입점 4개와 payload 멤버
  방출 지점 6개는 산문이 아니라 **테스트가 소스에서 매번 도출한다**
  (`every_codegen_ingress_escapes_what_it_is_given`,
  `every_payload_member_emission_uses_the_one_derivation`,
  `code_generation_never_case_folds_an_identifier`).

**남은 결함 (열림):**
- **제네릭 본문에서 타입 파라미터가 치환되지 않는다** — `type_to_c`가 `Type::TypeParam`/
  `Type::Generic`을 `void*`로 지워서, `fn f<T>(x: T) -> T { let y: T = x; return y; }`는
  `void* y = x;`를, `let b: Box<i64> = …`는 `void* b = (struct Box_i64){…}`를 방출하고 gcc가
  거부한다. **식별자 이스케이프와 무관하며 `main`에서 평범한 이름으로 동일 재현**된다.
  이것 때문에 제네릭 struct 필드 리시트는 링크·실행이 아니라 C 텍스트 단언이다
  (`a_keyword_named_generic_struct_field_is_escaped`).
- **`match`에 final `else`가 없다** — codegen이 `match`를 `if`/`else if` 체인으로 낮추는데 마지막
  `else`가 없어서, 모든 arm이 `return`해도 생성 C는 "모든 경로에서 return"을 증명하지 못한다
  (gcc도 동의: `-Wreturn-type`). D3b와 **별개** 결함. 핀 = `tests/stdlib/stdlib_tail_match.pd`
  (`known_violation:area_code,sides`).
- **`-Werror=return-type`가 아직 없다** — 위 두 항목이 닫혀도 링커 플래그는 **못 넣는다.** 막고
  있는 것은 바로 위의 final `else` 결함이고, 그것은 M1 소유가 아니다(`unscheduled`). 의무는
  `tests/rust-debt-manifest.txt`의
  `the_linker_will_ask_gcc_to_reject_a_function_that_falls_off_its_end` 행이 계속 들고 있다.
- **컴파일 불가능한 빌트인 6개** — `file_flush`·`file_seek`·`file_open_ex`·`file_close_ex`·
  `file_read_ex`·`file_write_ex`. 핸들 표현이 둘로 갈렸다(레거시=인덱스, 확장=`FileHandle`=`void*`).
  경계에서 캐스팅하면 `file_seek(file_open(p), 0, 0)`이 컴파일되고 정수 `1`을 `FILE*`로 역참조하므로
  **gcc 에러가 지금 segfault를 막는 유일한 장치다.** typeck이 `Support::Unsupported`로 먼저 거부 중.
  결정 필요: 확장 계열을 인덱스 테이블로 재기반할 것인가, `BUILTINS`에서 지울 것인가.
- `self`/제네릭 `impl`이 정의되지 않은 C. `Type::method(args)`가 codegen이 emit한 적 없는
  `Type_method__new`로 조용히 lowering.
- 중첩 배열이 로컬·파라미터 양쪽에서 불가 (`type_to_c`가 선언자가 아니라 타입으로 `T[M][N]` 구성).

**D6은 결함이 아니었다 (철회).** `CLAUDE.md`가 열린 결함으로 올려뒀으나 `191f8c1`에서 이미 고쳐졌고,
이 파일의 베이스보다 12커밋 앞선다. 명시된 다섯 프로그램 전부 재실행 — 하나도 재현되지 않는다
(`t(s); t(s)` → `5 5`, `take2(s,s)` → `10`, `bump(&mut p)` 연속 → `2`, 필드→빌트인 후 재사용 → `abc 3 1`).
호출 경로 `src/ownership/borrow_checker.rs:954-961`이 per-call lifetime을 만들고 인자 검사 후 끝낸다
(`new_lifetime()` :891 → `check_call_args` :894 → `end_borrows(&call_lifetime)` :897).
당시 인용됐던 줄은 대여된 **반환값**의 소유권 분류이지 인자 lifetime이 아니다 — green 핀이 붙은 채로
거짓 주장을 뒷받침하고 있었다. (옛 줄번호는 일부러 적지 않는다: 이 트리에 없는 리비전을 가리키는
`path:line`은 핀을 붙일 수 없고, 핀을 붙일 수 없는 인용은 조용히 드리프트한 인용과 구별되지 않는다 —
[`language-spec.md` A9.4](docs/specification/language-spec.md#a94-defect-d6-retracted)와 같은 이유.)

*이 줄의 인용 자체가 같은 병이었다.* 위 범위는 원래 `check_stmt`를 가리키고 있었다 — 주장(호출 경로)과
무관한 함수다. 언제부터 틀렸는지는 알 수 없다: **`CLAUDE.md`는 doc-evidence 코퍼스 밖이라 핀이 0개이고,
이 파일의 어떤 인용도 게이트가 검사한 적이 없다.** "핀이 0개"라는 사실은 드리프트가 살아남은 *이유*를
설명할 뿐 정당화하지 않는다 — 이 브랜치가 더 작은 파일들에 대해 계속 해온 주장("아무도 눈치채지
못한다")이 가장 크게 적용되는 표면이 바로 이 파일이다. 코퍼스 편입 가능 여부는
[`docs/contributing/claude-md-coverage.md`](docs/contributing/claude-md-coverage.md)에 측정과 함께 있다.

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
