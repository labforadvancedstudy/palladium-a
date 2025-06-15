# Palladium Programming Language

## 프로젝트 소개

Palladium은 안전성과 성능을 모두 갖춘 차세대 시스템 프로그래밍 언어입니다. Rust의 메모리 안전성과 함수형 프로그래밍의 수학적 엄밀함을 결합하여, 검증 가능하고 효율적인 소프트웨어 개발을 가능하게 합니다.

## 주요 특징

- **메모리 안전성**: 컴파일 시점에 메모리 오류를 방지
- **타입 안전성**: 강력한 정적 타입 시스템
- **간결한 문법**: 개발자 친화적인 직관적 구문
- **LLVM 기반**: 최적화된 네이티브 코드 생성
- **점진적 검증**: 필요에 따라 형식적 검증 추가 가능
- **표현력 높은 제어 구조**: 변수, 산술 연산, 조건문 지원

## 빠른 시작 가이드

### 요구사항

- Rust 1.70 이상
- LLVM 14 이상
- Cargo

### 설치

```bash
# 저장소 클론
git clone https://github.com/palladium-lang/palladium.git
cd palladium

# 빌드
cargo build --release

# 설치 (선택사항)
cargo install --path .
```

### 첫 번째 프로그램

```palladium
// calculator.pd
fn main() -> i32 {
    let a = 42;
    let b = 8;
    let result = a + b;
    
    print("The answer is: ");
    print_int(result);
    
    if result == 50 {
        print("Correct!");
    } else {
        print("Something went wrong!");
    }
    
    return 0;
}
```

```bash
# 컴파일하고 실행
palladium run calculator.pd
```

## 예제 코드

### 변수와 산술 연산

```palladium
// variables.pd
fn main() -> i32 {
    let x = 10;
    let y = 20;
    let sum = x + y;
    
    print("x = ");
    print_int(x);
    print("y = ");
    print_int(y);
    print("sum = ");
    print_int(sum);
    
    return sum;
}
```

### 조건문과 비교 연산

```palladium
// conditions.pd
fn main() -> i32 {
    let age = 25;
    
    if age >= 18 {
        print("Adult");
        if age >= 65 {
            print("Senior");
        } else {
            print("Not a senior");
        }
    } else {
        print("Minor");
    }
    
    return 0;
}
```

### 복잡한 연산과 표현식

```palladium
// complex_calc.pd
fn main() -> i32 {
    let score1 = 85;
    let score2 = 92;
    let score3 = 78;
    
    // 평균 계산 (정수 나눗셈)
    let average = (score1 + score2 + score3) / 3;
    
    print("Average score: ");
    print_int(average);
    
    // 등급 판정
    if average >= 90 {
        print("Grade: A");
    } else if average >= 80 {
        print("Grade: B");
    } else if average >= 70 {
        print("Grade: C");
    } else {
        print("Grade: F");
    }
    
    return average;
}
```

## 빌드 및 설치 방법

### 개발 환경 설정

1. Rust 툴체인 설치:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. LLVM 설치:
```bash
# macOS
brew install llvm@14

# Ubuntu/Debian
sudo apt-get install llvm-14 llvm-14-dev

# Arch Linux
sudo pacman -S llvm
```

3. 프로젝트 빌드:
```bash
cargo build
```

## 사용법

### 컴파일러 실행

```bash
# 파일 컴파일
palladium compile examples/hello.pd

# 컴파일 후 실행
palladium run examples/hello.pd

# 도움말
palladium --help
```

### 컴파일러 옵션

- `compile <file>`: Palladium 파일을 실행 파일로 컴파일
- `run <file>`: 컴파일 후 즉시 실행
- `check <file>`: 문법 검사만 수행
- `--version`: 버전 정보 표시
- `--help`: 도움말 표시

## 현재 지원되는 기능 (v0.2-alpha)

### 기본 기능
- ✅ 함수 정의 (`fn`)
- ✅ main 함수
- ✅ 문자열 리터럴
- ✅ print 함수
- ✅ print_int 함수 (정수 출력)
- ✅ 반환문 (`return`)
- ✅ 단일 행 주석 (`//`)
- ✅ 변수 선언 (`let`)
- ✅ 조건문 (`if`/`else`)

### 연산자
- ✅ 산술 연산자: `+`, `-`, `*`, `/`
- ✅ 비교 연산자: `<`, `>`, `<=`, `>=`, `==`, `!=`
- ✅ 복잡한 표현식 지원 (괄호 포함)

### 타입 시스템
- ✅ 기본 타입: `i32`, `String`
- ✅ 함수 타입
- ✅ 반환 타입 명시
- ✅ 타입 추론 (변수 선언 시)

### 제한사항
- ❌ 반복문 (v0.3 예정)
- ❌ 사용자 정의 타입 (v0.3 예정)
- ❌ 배열과 컬렉션 (v0.3 예정)
- ❌ 모듈 시스템 (v0.3 예정)

## 로드맵

### v0.1 (완료)
- [x] 기본 컴파일러 구조
- [x] 간단한 함수 컴파일
- [x] LLVM 코드 생성
- [x] 기본 타입 시스템

### v0.2 (현재 - alpha)
- [x] 변수와 바인딩
- [x] 기본 연산자 (산술, 비교)
- [x] 조건문 (if/else)
- [ ] 반복문 (while/for)

### v0.3 (2025 Q2)
- [ ] 구조체와 열거형
- [ ] 패턴 매칭
- [ ] 모듈 시스템
- [ ] 표준 라이브러리 기초

### v0.4 (2025 Q3)
- [ ] 트레이트 시스템
- [ ] 제네릭
- [ ] 소유권 시스템
- [ ] 빌림 검사기

### v0.5 (2025 Q4)
- [ ] 비동기 프로그래밍
- [ ] 매크로 시스템
- [ ] 최적화 개선

### v1.0 (2026)
- [ ] 자체 호스팅 컴파일러
- [ ] 형식적 검증 통합
- [ ] 프로덕션 준비 완료

## 기여하기

Palladium 프로젝트에 기여를 환영합니다! [CONTRIBUTING.md](CONTRIBUTING.md)를 참고해주세요.

## 라이선스

MIT 라이선스 - 자세한 내용은 [LICENSE](LICENSE) 파일을 참고하세요.

## 문의

- GitHub Issues: [github.com/palladium-lang/palladium/issues](https://github.com/palladium-lang/palladium/issues)
- Email: palladium-lang@example.com