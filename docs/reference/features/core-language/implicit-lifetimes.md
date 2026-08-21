> **NORMATIVE — this is what Palladium is defined to be.** It is not a description of what
> `pdc` implements today. What is implemented, partial, or unimplemented is recorded per
> specification section in the
> [implementation status annex](../../../specification/language-spec.md#part-ii-implementation-status-annex).
> Palladium blocks below are fenced `no-compile`: the syntax is normative, the compiler does not
> accept it yet, and `scripts/check-docs.sh` counts each fence rather than hiding it.

# Feature: Implicit Lifetimes

Normative specification section: [`language-spec.md` §N9 References and lifetimes](../../../specification/language-spec.md#n9-references-and-lifetimes).

## Overview

Palladium infers lifetimes, so the great majority of code carries no lifetime annotation at all,
with the memory-safety guarantee unchanged.

### Normative syntax

| Form | Meaning |
|---|---|
| `ref T` | A shared borrow of `T`. Replaces Rust's `&T`. |
| `ref mut T` | A mutable borrow of `T`. Replaces Rust's `&mut T`. |
| `ref<'a> T` | An explicitly named region, for the cases inference cannot resolve. |

There is no `'a` parameter list on functions, structs or impls. A region name appears only inside
a `ref<...>`, and only when the compiler has asked for one.

## Code Comparison

### Rust (Explicit Lifetimes Required)
```rust
// Simple function - still needs lifetime
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}

// Struct with references
struct Parser<'a> {
    input: &'a str,
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Parser<'a> {
        Parser { input, position: 0 }
    }
    
    fn parse_word(&mut self) -> Option<&'a str> {
        // Complex lifetime reasoning
        let start = self.position;
        while self.position < self.input.len() {
            self.position += 1;
        }
        Some(&self.input[start..self.position])
    }
}

// Multiple lifetimes
fn compare_and_get<'a, 'b>(x: &'a str, y: &'b str) -> &'a str 
where 'b: 'a {
    if x.len() > y.len() { x } else { x }
}
```

### Go (No Lifetimes - Uses GC)
```go
// Go doesn't track lifetimes - garbage collected
func longest(x, y string) string {
    if len(x) > len(y) {
        return x
    }
    return y
}

type Parser struct {
    input    string
    position int
}

func NewParser(input string) *Parser {
    return &Parser{input: input, position: 0}
}

func (p *Parser) ParseWord() string {
    start := p.position
    for p.position < len(p.input) {
        p.position++
    }
    return p.input[start:p.position]
}

// No lifetime complexity but:
// - Runtime GC overhead
// - No compile-time guarantees
// - Potential memory leaks
```

### Palladium (Implicit Lifetimes)
```palladium no-compile
// Lifetimes inferred automatically
fn longest(x: ref str, y: ref str) -> ref str {
    if x.len() > y.len() { x } else { y }
}

// Struct lifetimes inferred
struct Parser {
    input: ref str,
    position: usize,
}

impl Parser {
    fn new(input: ref str) -> Parser {
        Parser { input, position: 0 }
    }
    
    fn parse_word(ref mut self) -> Option<ref str> {
        // Compiler understands lifetime flow
        let start = self.position;
        while self.position < self.input.len() {
            self.position += 1;
        }
        Some(self.input[start..self.position])
    }
}

// Only explicit when truly ambiguous
fn compare_and_get(x: ref str, y: ref str) -> ref str {
    // Compiler infers this returns x's lifetime
    if x.len() > y.len() { x } else { x }
}
```

## Why This Feature Exists

### 1. Cognitive Load Reduction
Explicit lifetimes are a well-known barrier to learning Rust, and most annotations are mechanical:
they restate a data-flow relationship the compiler already has in front of it. Writing them down
spends the programmer's attention on bookkeeping rather than on the program.

### 2. Maintaining Safety
- Same memory safety guarantees as Rust
- Compiler errors when inference is ambiguous
- Can always fall back to explicit annotations

## How It Works

The block below is compiler-internal pseudocode rather than a Palladium program.

### Inference Algorithm
```palladium no-compile
// Compiler's internal lifetime inference
fn infer_lifetimes(ast: AST) -> Result<LifetimeMap> {
    let constraints = collect_constraints(ast);
    let regions = build_region_graph(constraints);
    
    match solve_regions(regions) {
        Ok(solution) => Ok(solution),
        Err(Ambiguous) => Err("Lifetime annotation required"),
    }
}
```

### Inference Rules
1. **Single Input**: Output lifetime = input lifetime
2. **Multiple Inputs**: Look for obvious data flow
3. **Self Methods**: Track self's lifetime through calls
4. **Struct Fields**: Infer from usage patterns

### When Explicit Annotation Is Needed
```palladium no-compile
// Ambiguous case - needs annotation
fn unclear(x: ref<'a> str, y: ref str) -> ref<'a> str {
    if rand() { x } else { y }  // Error: can't infer
}
```

Inference failing is a **compile error**, never a silently chosen answer. The definition is that
annotations become unnecessary, not that ambiguity becomes tolerable.

## Where the implementation currently diverges

Measured at commit `abeb665`.

**1. `ref` is not a keyword, so the normative reference syntax does not parse.**
`grep -n '"ref"' src/lexer/token.rs` returns nothing, and `grammar.ebnf:58-59` lists `ref` among
the words that "lex as ordinary identifiers". Compiling
`fn longest(x: ref String, y: ref String) -> ref String { return x; }` gives:

```
error: Unexpected token: expected ')' (Expected ')'), found identifier 'String'
```

— `ref` was consumed as the parameter's type and `String` was then unexpected.

**2. The implementation uses Rust's syntax, including the `'a` parameters this design removes.**
`reference = '&' [ "'" identifier ] [ "mut" ] type` (`grammar.ebnf:145`), and generic parameter
lists accept lifetimes (`generic_param` at `grammar.ebnf:130`, parsed at
`src/parser/mod.rs:36-98`). Measured: `fn f<'a>(x: &'a String) -> &'a String { return x; }`
compiles and links. So the annotation burden the design deletes is currently the only supported
spelling.

**3. Nothing consumes the lifetimes it parses.** `Function.lifetime_params` is populated
(`src/parser/mod.rs:553`) and, outside the parser, appears only as `vec![]` in test and LSP
fixtures — `grep -rn lifetime_params src/ --include='*.rs' | grep -v '^src/parser'` returns
nothing else. There is no region inference of any kind: `grep -rn 'region\|Region' src/
--include='*.rs'` returns nothing.

**4. References are not a type.** The type checker maps `Type::Reference { inner, .. }` to the
inner type — "For now, treat references as the inner type / TODO: Proper reference type handling"
(`src/typeck/mod.rs:121-125`). `&i64` and `i64` are indistinguishable to it, so no lifetime
relation could be inferred even if the machinery existed.

**5. What exists instead is a move/initialization discipline.**
`src/ownership/borrow_checker.rs` (1165 lines) tracks moves and conflicting borrows over a scope
counter (`Lifetime` enum at `src/ownership/mod.rs:27`, scope lifetimes at `:109`). It has a known
defect where a call argument is borrowed with `Lifetime::Named("fn")`
(`src/ownership/borrow_checker.rs:236`) but released against `Lifetime::Scope(n)`, so the borrow
is never released. That is a bug in a different mechanism, not partial delivery of this one.

## Design intent, not measurements

The earlier version of this document asserted that "Studies show that lifetime annotations account
for 30% of Rust learning curve difficulty, 15% of compilation errors for beginners, 5-10% of code
verbosity", and a "Performance Impact" of "+5-10% compilation time, zero runtime overhead,
identical binary size". No study is cited, none is in this repository, and there is no
implementation to have measured the compile-time figure against. All of those numbers are deleted.

The intent that survives:

- Inference is a compile-time analysis with no runtime representation, so a program's generated
  code is unaffected by whether a lifetime was written or inferred.
- Safety is not traded for ergonomics: the checker rejects what Rust's borrow checker rejects, and
  reports ambiguity rather than guessing.

## Migration Guide

### From Rust
```rust
// Before (Rust)
fn process<'a, 'b>(data: &'a mut Data, config: &'b Config) -> &'a str {
    data.process_with_config(config)
}
```

```palladium no-compile
// After (Palladium)  
fn process(data: ref mut Data, config: ref Config) -> ref str {
    data.process_with_config(config)
}
```

## Future Improvements

1. **Better Error Messages**: Show why inference failed
2. **Inference Hints**: Guide compiler with attributes
3. **Cross-Function Inference**: Infer across module boundaries
4. **IDE Support**: Show inferred lifetimes on hover

## Related

- [Palladium v1.0 feature definition](../PALLADIUM_V1_FEATURES.md) — where this sits among the rest
- [Async as effect](../async-system/async-as-effect.md)
- [Totality checking](../advanced/totality-checking.md)
- [Feature index](../status.yaml)
- [Language specification](../../../specification/language-spec.md)
