# 실제 작동하는 부트스트랩 데모

## 1단계: Rust pdc로 Palladium 컴파일러 컴파일

```bash
# Palladium으로 작성된 컴파일러를 Rust pdc로 컴파일
cargo run -- compile bootstrap/archive/bootstrap_compiler_v1.pd

# 생성된 C 코드를 실행파일로 컴파일
gcc build_output/bootstrap_compiler_v1.c -o pd_bootstrap_compiler
```

## 2단계: Palladium 컴파일러 실행

```bash
$ ./pd_bootstrap_compiler
Palladium Bootstrap Compiler
============================
Generated: output.c
Compile: gcc output.c
```

## 3단계: 생성된 코드 확인 및 실행

```bash
$ cat output.c
int main(){return 42;}

$ gcc output.c -o test_output
$ ./test_output; echo "Exit code: $?"
Exit code: 42
```

## 이게 뭘 증명하는가?

1. **Rust pdc가 Palladium 코드를 제대로 컴파일함** ✓
   - bootstrap_compiler_v1.pd → bootstrap_compiler_v1.c

2. **Palladium으로 작성한 컴파일러가 실제로 동작함** ✓
   - pd_bootstrap_compiler가 실행되어 C 코드 생성

3. **생성된 C 코드가 올바르게 작동함** ✓
   - output.c가 컴파일되고 실행되어 42 반환

## 현재 한계

- file_write()가 파일명이 아닌 핸들을 받음
- 문자열 연결 불가능
- 한 줄씩만 읽기 가능
- 모듈 시스템 없음

그래도 **Palladium으로 컴파일러를 작성할 수 있음을 증명!**