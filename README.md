### “Palladium α” **v0.4 – Final‑Lock Draft**

*(통합 리서치·PDF 화이트페이퍼  반영, 2025‑06‑14)*

---

## 1. 설계 원칙 – 최종 결의

| #  | 원칙               | 최종 결정                                                                   |
| -- | ---------------- | ----------------------------------------------------------------------- |
| 1  | **안전 ↔ 단순**      | Rust 1.31 (NLL) 수준의 소유권·빌림을 유지하되, *암시 수명*·*단일 String*으로 표면 복잡도 축소       |
| 2  | **선택적 런타임**      | 필수 런타임 0 byte. 옵션으로 ① 경량 스케줄러 ② 보수적 GC ③ JIT 모듈을 *필요 시* 링크              |
| 3  | **컴파일 속도**       | 언어 규칙을 ≤ Rust 1.31 복잡도로 고정, *Cranelift debug* + *LLVM release* 듀얼 파이프라인 |
| 4  | **기능 미니멀리즘**     | GAT·full const‑generics·proc‑macro *불포함* (실험은 plugin tier)              |
| 5  | **정형 증명**        | MIR‑Pd → ASM 경로를 Coq 검증; “컴파일된 프로그램이 의미론 보존” 증명                         |
| 6  | **다중 메모리 계층**    | `own T` (기본) · `Rc[T]` · `@T` (GC) 세 포인터 종류만 노출                         |
| 7  | **단일 동적 다형성 모델** | **Fat‑VTable** → 모든 trait 객체화 허용; 별도 “객체 안전 규칙” 제거                      |
| 8  | **모듈 = 파일시스템**   | `foo.pdm` == 모듈 foo ; `mod` 선언 폐지                                       |
| 9  | **거버넌스**         | 공개 RFC + **Chief Architect 1인 최종 merge**                                |
| 10 | **AI·웹 친화**      | WASM tier‑1, REPL·reflection 모듈 표준 제공                                   |

---

## 2. 언어 핵심 구문 (최종)

```palladium
// ── 타입 · 메모리 ──
struct Vec3 { x: f32, y: f32, z: f32 }   // 명목·구조 혼합
let p: own Vec3      = Vec3{0,0,0};      // 값 소유
let q: &Vec3         = &p;               // 불변 빌림
let r: Rc[Vec3]      = rc p.clone();     // 참조 계수
let g: @Vec3         = gc Vec3{1,2,3};   // GC 포인터

// ── 오류 처리 ──
fn read(path: &String) -> io::Result[String] ? {
    fs::open(path)?.read_to_string()
}

// ── 동시성 (스케줄러 선택) ──
task fetch(url: &String) -> Result[Bytes] {      // ‘task’ = 협력 코루틴
    let resp = http::get(url)?;                  // 내부에서 자동 yield
    resp.bytes()
}
spawn fetch("https://...");                      // 즉시 태스크 실행

// ── 매크로 2.0 ──
pub macro vec { [ $( $e:expr ),* ] => {
    let mut tmp = List[_]();
    $( tmp.push($e); )*
    tmp
}}
```

---

## 3. 메모리 모델 세부 (API 스텁)

| 표기      | 안전                       | 특징                       | 주 사용처        |
| ------- | ------------------------ | ------------------------ | ------------ |
| `own T` | ✔                        | 스택/힙 RAII, 이동·소유권 전송     | 핵심 루프·임베디드   |
| `Rc[T]` | ✔ (단일 스레드) / `Arc[T]` 멀티 | 참조 계수 + 약한 포인터           | GUI·서비스      |
| `@T`    | ✔                        | 보수적 mark‑sweep, cycle 허용 | 그래프·AST·스크립트 |

런타임 링크 전략

```
link own   → 0 KiB
+Rc/Arc    → libpd_rc.a  (≈12 KiB)
+@GC       → libpd_gc.a  (≈45 KiB, stop‑the‑world, generational)
```

---

## 4. 동시성·IO 선택지

| 모드            | 키워드                 | 런타임                 | 장점                 | 비용          |
| ------------- | ------------------- | ------------------- | ------------------ | ----------- |
| OS Thread     | `thread::spawn`     | 없음                  | C 수준 직관            | 스레드당 스택     |
| **Task** (기본) | `task`/`spawn`      | libpd\_sched (M\:N) | 함수 색깔 없음, 자동 yield | 소형 프레임+스케줄러 |
| Async Interop | `async fn` (plugin) | 외부 executor         | Tokio 등 재사용        | colored API |

> **정책**: 표준은 *task* 모델만 정의. `async` 는 외부 plugin tier에서 유지.

---

## 5. 매크로 2.0 규칙

1. 완전 위생, 기본 캡처 불가
2. `macro name { pattern => expansion }` 문법만 허용
3. 모듈 안 아이템으로 취급, `use mod::mac` 가능
4. 컴파일‑타임 순수 함수 (I/O 금지) – 빌드 결정성 확보

*proc‑macro* 는 plugin tier crate로 격리; core 언어에선 비활성.

---

## 6. 오류·패닉 정책

* **Result + `?`** : 모든 회복 가능 경로
* **panic!** : 항상 *abort* (unwind 불가) → 코드 경량화·분석 단순화
* 테스트 프레임워크가 `panic = abort` 환경에서 실패 케이스 격리 실행

---

## 7. ABI & Interop (확정)

| 언어                 | 호출 규약                   | 지원 범위       |
| ------------------ | ----------------------- | ----------- |
| C / Fortran / Zig  | `extern "c"`            | 전체          |
| C++ (Itanium/MSVC) | `extern "cpp"` + 헤더파서   | 클래스·가상함수 호출 |
| Rust               | via C‑shim or `repr(C)` | 모든 안정 item  |
| D                  | `extern "d"` (DMD/LDC)  | POD + 함수    |

자동 바인딩 툴 체계

```
cbindgen-pd   (C)  
cppbind-pd    (C++)  
rustbind-pd   (Rust reprC)  
```

---

## 8. 도구·아티팩트 체계

```
pdc          ── 컴파일러 ▸ LLVM / Cranelift
pdpkg        ── 패키지·빌드 (Cargo‑like)
pdtest       ── 단위·통합 테스트
pdrepl       ── JIT/인터프리터 (Cranelift)
pddoc        ── mdBook 규격 API 문서
```

CI 봇: `docs/spec ↔ compiler/tests ↔ benches` 해시 체크로 스펙·구현 불일치 자동 경고.

---

## 9. 로드맵 업데이트

| 분기      | 릴리스               | 하이라이트                                     |
| ------- | ----------------- | ----------------------------------------- |
| 2025 Q4 | **0.1‑bootstrap** | Rust → Pd self‑host, `own/Rc`, OS threads |
| 2026 Q2 | 0.2               | 파일‑모듈, 매크로 2.0, task 스케줄러                 |
| 2026 Q4 | 0.3               | @GC, Fat‑VTable, extern "cpp"             |
| 2027 Q2 | 0.4               | Coq 검증 β, WASM tier‑1, REPL               |
| 2027 Q4 | **1.0 LTS**       | 기능 동결, 3‑년 안정 지원                          |

---

## 10. 앞으로 바로 할 일 (개발 우선순위)

1. `/compiler/ir` Coq signature skeleton 확정 → 최소 검증 경로 통과
2. `/stdlib/string.pdm` : 단일 String + slice view API
3. `libpd_sched` : M\:N 런타임 프로토타입 (epoll + work‑stealing)
4. `macro 2.0` 파서 통합 및 vec! 이식
5. `cppbind-pd` CLI MVP (Clang libclang driver)

---

## 11. 결론

* **Rust 1.31**의 “단단한 심장”을 중심으로,
* **PDF 백서**가 지적한 과도 복잡성( async 색깔, GAT 폭증, 컴파일 타임) 을 제거 / 선택화하고,&#x20;
* **GC·스케줄러·JIT**를 “옵션 계층”으로 추가하여 **우아함 + 완전성**을 동시에 달성.

**Palladium α v0.4**는 이제 스펙·로드맵·거버넌스가 고정됐다.
다음 단계는 *코드*다. `git checkout -b 0.1-bootstrap` 후 첫 PR을 올려 달라. 🚀
