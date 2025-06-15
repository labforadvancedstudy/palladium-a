# Palladium 시작하기

Palladium 프로그래밍 언어에 오신 것을 환영합니다! 이 가이드는 Palladium을 처음 시작하는 개발자를 위한 상세한 안내서입니다.

## 목차

1. [설치 가이드](#설치-가이드)
2. [첫 번째 프로그램 작성하기](#첫-번째-프로그램-작성하기)
3. [컴파일러 사용법](#컴파일러-사용법)
4. [예제 설명](#예제-설명)
5. [문제 해결 팁](#문제-해결-팁)

## 설치 가이드

### 사전 요구사항

Palladium을 설치하기 전에 다음 도구들이 필요합니다:

#### 1. Rust 설치

Palladium 컴파일러는 Rust로 작성되었으므로 Rust 툴체인이 필요합니다.

```bash
# Rust 설치 (macOS/Linux)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 설치 확인
rustc --version
cargo --version
```

Windows 사용자는 [rustup.rs](https://rustup.rs/)에서 설치 프로그램을 다운로드하세요.

#### 2. LLVM 설치

Palladium은 LLVM을 백엔드로 사용합니다.

**macOS:**
```bash
# Homebrew를 사용한 설치
brew install llvm@14

# 환경 변수 설정 (zsh)
echo 'export PATH="/opt/homebrew/opt/llvm@14/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

**Ubuntu/Debian:**
```bash
# LLVM 14 설치
sudo apt-get update
sudo apt-get install llvm-14 llvm-14-dev clang-14

# 기본 버전으로 설정
sudo update-alternatives --install /usr/bin/llvm-config llvm-config /usr/bin/llvm-config-14 100
```

**Arch Linux:**
```bash
sudo pacman -S llvm clang
```

#### 3. Git 설치

소스 코드를 클론하기 위해 Git이 필요합니다.

```bash
# macOS
brew install git

# Ubuntu/Debian
sudo apt-get install git

# Arch Linux
sudo pacman -S git
```

### Palladium 설치

#### 소스에서 빌드하기

1. 저장소 클론:
```bash
git clone https://github.com/palladium-lang/palladium.git
cd palladium
```

2. 컴파일러 빌드:
```bash
cargo build --release
```

3. 바이너리 설치 (선택사항):
```bash
cargo install --path .
```

4. 설치 확인:
```bash
palladium --version
```

#### 환경 변수 설정

바이너리를 직접 실행하려면:

```bash
# 현재 세션에서만
export PATH="$PATH:$(pwd)/target/release"

# 영구적으로 설정 (zsh)
echo 'export PATH="$PATH:'$(pwd)'/target/release"' >> ~/.zshrc
source ~/.zshrc

# 영구적으로 설정 (bash)
echo 'export PATH="$PATH:'$(pwd)'/target/release"' >> ~/.bashrc
source ~/.bashrc
```

## 첫 번째 프로그램 작성하기

### 1. Hello World

파일 생성: `hello.pd`

```palladium
// 첫 번째 Palladium 프로그램
fn main() -> i32 {
    print("Hello, World!");
    return 0;
}
```

### 2. 컴파일 및 실행

```bash
# 컴파일
palladium compile hello.pd

# 실행
./hello

# 또는 한 번에 컴파일하고 실행
palladium run hello.pd
```

### 3. 프로그램 분석

- `fn main() -> i32`: 프로그램의 진입점. 정수를 반환
- `print("Hello, World!")`: 문자열을 콘솔에 출력
- `return 0`: 성공적으로 종료됨을 나타내는 0 반환

## 컴파일러 사용법

### 기본 명령어

```bash
# 도움말 표시
palladium --help

# 버전 정보
palladium --version

# 파일 컴파일
palladium compile <file.pd>

# 컴파일 후 실행
palladium run <file.pd>

# 문법 검사만 수행
palladium check <file.pd>
```

### 컴파일 옵션

#### 출력 파일 지정

```bash
palladium compile hello.pd -o my_program
```

#### 디버그 정보 포함

```bash
palladium compile hello.pd --debug
```

#### 최적화 레벨

```bash
# 최적화 없음 (빠른 컴파일)
palladium compile hello.pd -O0

# 기본 최적화
palladium compile hello.pd -O1

# 공격적 최적화
palladium compile hello.pd -O2
```

### 에러 메시지 읽기

Palladium은 친절한 에러 메시지를 제공합니다:

```
error[E0001]: expected ';' at end of statement
  --> hello.pd:3:24
   |
3  |     print("Hello, World!")
   |                          ^ expected ';' here
   |
help: add a semicolon at the end of the statement
```

## 예제 설명

### 예제 1: 기본 출력

```palladium
// basic_print.pd
fn main() -> i32 {
    print("첫 번째 줄");
    print("두 번째 줄");
    return 0;
}
```

이 프로그램은 두 줄의 텍스트를 출력합니다.

### 예제 2: 여러 함수 (향후 지원)

```palladium
// multi_function.pd
fn greet() {
    print("안녕하세요!");
}

fn main() -> i32 {
    greet();
    print("Palladium에 오신 것을 환영합니다!");
    return 0;
}
```

*참고: v0.1에서는 main 함수만 지원됩니다.*

### 예제 3: 주석 사용

```palladium
// comments.pd
// 이것은 한 줄 주석입니다

fn main() -> i32 {
    // 함수 내부의 주석
    print("주석은 컴파일되지 않습니다"); // 줄 끝 주석
    return 0; // 0을 반환
}
```

## 문제 해결 팁

### 일반적인 오류와 해결 방법

#### 1. "palladium: command not found"

**원인**: 실행 파일이 PATH에 없음

**해결책**:
```bash
# 전체 경로로 실행
./target/release/palladium --version

# 또는 PATH에 추가
export PATH="$PATH:$(pwd)/target/release"
```

#### 2. "error: linker 'cc' not found"

**원인**: C 컴파일러가 설치되지 않음

**해결책**:
```bash
# macOS
xcode-select --install

# Ubuntu/Debian
sudo apt-get install build-essential

# Arch Linux
sudo pacman -S base-devel
```

#### 3. "LLVM not found"

**원인**: LLVM이 설치되지 않았거나 경로가 잘못됨

**해결책**:
```bash
# LLVM 설치 확인
llvm-config --version

# 환경 변수 설정
export LLVM_SYS_140_PREFIX=/usr/lib/llvm-14
```

#### 4. 컴파일 에러: "unexpected token"

**원인**: 문법 오류

**해결책**:
- 세미콜론 확인
- 괄호 쌍 확인
- 함수 시그니처 확인

### 디버깅 팁

1. **컴파일러 출력 자세히 보기**:
```bash
palladium compile hello.pd --verbose
```

2. **중간 결과물 확인**:
```bash
# LLVM IR 출력
palladium compile hello.pd --emit-llvm
```

3. **문법 검사만 수행**:
```bash
palladium check hello.pd
```

### 성능 최적화 팁

1. **릴리스 모드로 컴파일**:
```bash
palladium compile hello.pd -O2
```

2. **프로파일링** (향후 지원):
```bash
palladium compile hello.pd --profile
```

## 다음 단계

이제 Palladium의 기본을 익혔습니다! 다음을 시도해보세요:

1. **더 많은 예제 살펴보기**: `examples/` 디렉토리 확인
2. **언어 레퍼런스 읽기**: 상세한 문법과 기능 설명
3. **컴파일러 소스 코드 탐험**: Rust를 알고 있다면 컴파일러 구조 이해
4. **커뮤니티 참여**: GitHub Issues에서 질문하고 피드백 제공

## 도움 받기

문제가 발생하면:

1. **GitHub Issues**: https://github.com/palladium-lang/palladium/issues
2. **FAQ 문서**: docs/FAQ.md
3. **소스 코드**: 컴파일러 동작 이해를 위해 소스 코드 참고

Happy coding with Palladium!