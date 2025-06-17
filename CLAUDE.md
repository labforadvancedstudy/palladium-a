READ and RESEPCT './CLAUDE.LOCAL.md'

---

1. 너는 ADHD야. 항상 정신이 들면 첫번째 루틴은 "TODO를 확인"해야해. 
1.1 어디까지 했고 무엇을 진행하고 있었고 다음에 무엇을 해야할지 항상 확인해.
1.2 모르겠다면 당장 중단해. 소리쳐! "나 뭘해야할지 모르겠어요!" 그리고 사용자에게 도움을 요청해.
2. 루트 폴더에 파일과 디렉토리가 20개가 넘으면 유저에게 파일 정리 계획을 이야기해주고 정리 제안을 해줘.

---

# TESTING
- Always test your code before showing results to the user
- Run make --dry-run for Makefiles, cargo check for Rust, syntax validation for configs

---

# Palladium Project Structure & Analysis

## Project Overview

Palladium is a systems programming language with the goal of combining Turing's correctness with von Neumann's performance. The project has achieved 100% bootstrap capability as of June 17, 2025.

## Directory Structure

```
palladium-a/
├── src/                    # Rust implementation of the compiler
│   ├── ast/               # Abstract Syntax Tree definitions
│   ├── codegen/           # Code generation (C backend)
│   ├── driver/            # Compiler driver/main entry
│   ├── errors/            # Error handling and reporting
│   ├── lexer/             # Lexical analysis/tokenization
│   ├── parser/            # Syntax parsing
│   ├── resolver/          # Module resolution
│   ├── runtime/           # Runtime library functions
│   ├── tests/             # Compiler tests
│   └── typeck/            # Type checking
│
├── bootstrap/             # First bootstrap attempt (archived)
│   ├── archive/          # Old bootstrap attempts
│   ├── core/             # Core bootstrap files
│   ├── demos/            # Demo programs
│   └── utilities/        # Helper utilities
│
├── bootstrap2/            # Second bootstrap attempt (successful)
│   ├── pdc.pd            # Full compiler (1,220 lines)
│   └── various .pd files # Bootstrap components
│
├── bootstrap3/            # Third bootstrap (incremental approach)
│   ├── tiny_v1-v16.pd    # Incremental tiny compilers
│   ├── tiny_self.pd      # Self-hosting test
│   ├── BOOTSTRAP_ACHIEVED.md
│   └── build_output/     # Generated C files
│
├── stdlib/                # Standard library
│   ├── std/
│   │   ├── collections/
│   │   ├── io/
│   │   ├── math.pd
│   │   └── string.pd
│   └── prelude.pd
│
├── examples/              # Example programs
│   ├── algorithms/       # Algorithm implementations
│   ├── basic/            # Basic language features
│   ├── bootstrap/        # Bootstrap examples
│   ├── data_structures/  # Data structure examples
│   └── testing/          # Test programs
│
├── docs/                  # Documentation
│   ├── design/           # Language design docs
│   ├── marketing/        # Marketing materials
│   ├── planning/         # Project planning
│   └── release/          # Release notes
│
├── reports/               # Status reports
├── scripts/               # Build/utility scripts
├── archive/               # Archived materials
└── build_output/          # Compiler output files
```

## Key Files

### Rust Compiler (src/)
- `main.rs` - Entry point
- `lexer/scanner.rs` - Tokenizer using logos
- `parser/parser.rs` - Recursive descent parser
- `typeck/mod.rs` - Type checker
- `codegen/c_backend.rs` - C code generator

### Bootstrap Compilers
- `bootstrap2/pdc.pd` - Full-featured compiler (1,220 lines)
- `bootstrap3/tiny_v16.pd` - Final tiny compiler with arrays (760 lines)
- `bootstrap3/BOOTSTRAP_ACHIEVED.md` - Bootstrap documentation

### Standard Library
- `stdlib/prelude.pd` - Core types and functions
- `stdlib/std/string.pd` - String utilities
- `stdlib/std/math.pd` - Math functions

## Architecture Analysis

### Strengths ✅

1. **Clear Separation of Concerns**
   - Rust implementation separate from Palladium bootstrap
   - Incremental bootstrap approach in bootstrap3/
   - Well-organized standard library

2. **Multiple Bootstrap Paths**
   - bootstrap2/ has full compiler
   - bootstrap3/ has incremental tiny compilers
   - Demonstrates different approaches to self-hosting

3. **Comprehensive Examples**
   - Examples cover all language features
   - Good test coverage
   - Bootstrap demos included

### Weaknesses ❌

1. **Scattered Bootstrap Attempts**
   - Three bootstrap directories (bootstrap/, bootstrap2/, bootstrap3/)
   - Could be consolidated or better documented
   - Some duplicate/abandoned code

2. **Build System**
   - No unified build system (Makefile, build.pd, etc.)
   - Manual compilation steps required
   - Output files mixed with source in some directories

3. **Documentation Gaps**
   - No top-level architecture document
   - Bootstrap process not clearly documented
   - Missing user guide for the language

4. **Testing Infrastructure**
   - Tests scattered across directories
   - No automated test runner
   - No CI/CD setup

5. **Module System**
   - Resolver implementation incomplete
   - Import paths inconsistent
   - No clear module naming convention

## Recommendations

### Immediate Actions

1. **Consolidate Bootstrap**
   ```
   bootstrap/
   ├── v1_archived/     # Move old attempts here
   ├── v2_full/         # Full compiler approach
   ├── v3_incremental/  # Tiny compiler approach
   └── README.md        # Explain each approach
   ```

2. **Create Build System**
   ```palladium
   // build.pd - Palladium build script
   fn build_compiler() {
       compile("bootstrap/v3_incremental/tiny_v16.pd");
       compile("src/main.pd"); // Future: Palladium version
   }
   ```

3. **Standardize Project Layout**
   ```
   src/          # Palladium source (future)
   rust_src/     # Current Rust implementation
   bootstrap/    # Bootstrap compilers
   stdlib/       # Standard library
   tests/        # All tests
   docs/         # All documentation
   ```

### Long-term Improvements

1. **Self-hosting Transition**
   - Port Rust compiler to Palladium
   - Use tiny_v16 as starting point
   - Gradually expand features

2. **Testing Framework**
   - Create test runner in Palladium
   - Automated regression tests
   - Performance benchmarks

3. **Documentation**
   - Language specification
   - User guide
   - Contributor guide
   - Architecture document

## Project Health Score: 85/100

### Positive Factors (+)
- ✅ 100% bootstrap achieved (+30)
- ✅ Working compilers (+20)
- ✅ Good code organization (+10)
- ✅ Comprehensive examples (+10)
- ✅ Active development (+10)
- ✅ Clear vision/philosophy (+5)

### Negative Factors (-)
- ❌ Scattered bootstrap code (-5)
- ❌ No build automation (-5)
- ❌ Documentation gaps (-5)

## Conclusion

The Palladium project has achieved its primary goal of bootstrap capability. The code is well-written and the language design is solid. The main areas for improvement are:

1. **Organization** - Consolidate bootstrap attempts
2. **Automation** - Build system and testing
3. **Documentation** - User and developer guides

The project is ready for the next phase: transitioning from Rust implementation to full self-hosting using the bootstrap compilers.