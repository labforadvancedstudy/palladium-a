# ❓ Palladium Bootstrap FAQ

> Common questions about Palladium's self-hosting journey

## General Questions

### Q: What does "self-hosting" mean?
**A**: Self-hosting means the Palladium compiler is written in Palladium itself. It can compile any Palladium program, including its own source code.

### Q: Why is self-hosting important?
**A**: It proves the language is:
- Complete enough for serious software
- Practical for real-world use
- Independent of other languages
- Ready for production

### Q: How long did it take to achieve self-hosting?
**A**: The focused bootstrap effort took approximately 4-5 days of intensive development, building on several weeks of language design and initial implementation.

## Technical Questions

### Q: What's the minimum feature set needed for self-hosting?
**A**: Essential features include:
```palladium
struct MinimalFeatures {
    // Types
    integers: bool,      // ✅
    strings: bool,       // ✅
    booleans: bool,      // ✅
    arrays: bool,        // ✅
    structs: bool,       // ✅
    
    // Control
    functions: bool,     // ✅
    if_else: bool,       // ✅
    loops: bool,         // ✅
    
    // I/O
    file_read: bool,     // ✅
    file_write: bool,    // ✅
}
```

### Q: How big is the self-hosted compiler?
**A**: The Palladium compiler consists of:
- Lexer: ~1000 lines
- Parser: ~1300 lines
- Type Checker: ~400 lines
- Code Generator: ~300 lines
- **Total: ~3000+ lines of Palladium code**

### Q: What language was the first compiler written in?
**A**: The initial Palladium compiler was written in Rust. This is called the "bootstrap compiler" - it exists only to compile the first version of the Palladium compiler written in Palladium.

### Q: Does Palladium still depend on Rust?
**A**: No! After the initial bootstrap, Palladium is completely independent. The Rust compiler is no longer needed.

## Process Questions

### Q: How do you verify self-hosting correctness?
**A**: Three-stage verification:
```bash
# Stage 1: Rust compiler compiles Palladium compiler
rust_compiler palladium.pd -> palladium_1

# Stage 2: First Palladium compiler compiles itself  
palladium_1 palladium.pd -> palladium_2

# Stage 3: Second compiler compiles itself
palladium_2 palladium.pd -> palladium_3

# Verification: Stage 2 and 3 must be identical
diff palladium_2 palladium_3  # Should show no differences
```

### Q: What's the compilation pipeline?
**A**: 
```
.pd file → Lexer → Tokens → Parser → AST → Type Checker → Typed AST → Code Gen → C code → GCC → Executable
```

### Q: Why generate C code instead of assembly?
**A**: C provides:
- Portability across platforms
- Easier debugging
- Access to C libraries
- Simpler implementation
- Good performance

## Design Questions

### Q: What were the biggest challenges?
**A**: 
1. **Memory Management** - No garbage collector
2. **String Handling** - Needed StringBuilder pattern
3. **Fixed Arrays** - No dynamic allocation initially
4. **Missing Features** - Discovered needs during implementation

### Q: What features were added specifically for bootstrap?
**A**: 
- Mutable parameters (for efficient algorithms)
- Unary operators (-, !)
- String manipulation functions
- File I/O
- Better array handling

### Q: Any design decisions you regret?
**A**: Minor ones:
- Should have implemented Vec earlier
- Module system would help organization
- Better error types (Result<T,E>) from start

## Usage Questions

### Q: How do I compile Palladium programs?
**A**: 
```bash
# Compile a Palladium program
$ pdc program.pd -o program

# Run the compiled program
$ ./program
```

### Q: Can I use the Palladium compiler today?
**A**: Yes! The compiler is functional and can compile real programs. It's still being improved, but it works.

### Q: Where's the source code?
**A**: The complete compiler source is in the `examples/bootstrap/` directory:
- `lexer_complete.pd`
- `parser_complete.pd`
- `typechecker_simple.pd`
- `codegen_simple.pd`
- `pdc.pd` (main compiler)

## Future Questions

### Q: What's next after self-hosting?
**A**: 
1. **Optimization** - Make compiled code faster
2. **Error Messages** - Better diagnostics
3. **Standard Library** - More built-in functionality
4. **Tooling** - Package manager, build system
5. **Backends** - LLVM, WebAssembly

### Q: Can I contribute?
**A**: Absolutely! Now that it's self-hosted, you can improve the compiler using Palladium itself. Check CONTRIBUTING.md for guidelines.

### Q: Will Palladium replace Rust/C++/Go?
**A**: Palladium aims to be another option in the systems programming space. Each language has its strengths - Palladium focuses on simplicity, safety without GC, and proven correctness.

## Philosophy Questions

### Q: Why create another programming language?
**A**: Palladium explores the intersection of:
- Formal correctness (Turing)
- Raw performance (von Neumann)
- Modern ergonomics
- Simplicity

### Q: What makes Palladium special?
**A**: 
- No garbage collector, but memory safe
- Simple enough to learn quickly
- Powerful enough to write compilers
- Designed for correctness proofs

### Q: Is Palladium production-ready?
**A**: It's ready for experimentation and small projects. The core is solid (it compiles itself!), but ecosystem and tooling are still growing.

## Troubleshooting

### Q: Compiler crashes on my program
**A**: Check:
1. Syntax follows examples
2. All types are declared
3. Array bounds are respected
4. File paths are correct

### Q: Generated C code won't compile
**A**: Ensure:
- GCC is installed
- Include paths are set
- No name conflicts with C keywords

### Q: How do I debug compiler issues?
**A**: 
```palladium
// Add debug prints in compiler
fn parse_expression(tokens: Tokens) -> Expr {
    print("Parsing expression at token: ");
    print_token(tokens.current());
    // ... rest of parsing
}
```

## Meta Questions

### Q: How does it feel to achieve self-hosting?
**A**: It's an incredible milestone! Like teaching a child to walk, then watching them run. The language is now truly independent and alive.

### Q: Any advice for others attempting self-hosting?
**A**: 
1. Start simple - minimal features first
2. Test constantly - catch bugs early
3. Stay motivated - it's challenging
4. Celebrate milestones - enjoy the journey!

### Q: What's the most satisfying part?
**A**: Running `pdc pdc.pd -o pdc_new` and seeing it work. The compiler compiling itself - it's programming magic! ✨

---

Have more questions? Join our Discord or open an issue on GitHub!

*"The best documentation is a working compiler that documents itself by existing."*