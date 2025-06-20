use criterion::{black_box, criterion_group, criterion_main, Criterion};
use palladium::driver::Driver;
use palladium::lexer::Lexer;
use palladium::parser::Parser;

fn bench_lexer(c: &mut Criterion) {
    let source = r#"
        fn fibonacci(n: i64) -> i64 {
            if n <= 1 {
                return n;
            }
            return fibonacci(n - 1) + fibonacci(n - 2);
        }
        
        fn main() {
            let result = fibonacci(10);
            print_int(result);
        }
    "#;

    c.bench_function("lexer_fibonacci", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(source));
            let _ = lexer.collect_tokens().unwrap();
        })
    });
}

fn bench_parser(c: &mut Criterion) {
    let source = r#"
        fn fibonacci(n: i64) -> i64 {
            if n <= 1 {
                return n;
            }
            return fibonacci(n - 1) + fibonacci(n - 2);
        }
        
        fn main() {
            let result = fibonacci(10);
            print_int(result);
        }
    "#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.collect_tokens().unwrap();

    c.bench_function("parser_fibonacci", |b| {
        b.iter(|| {
            let mut parser = Parser::new(black_box(tokens.clone()));
            let _ = parser.parse().unwrap();
        })
    });
}

fn bench_full_compilation(c: &mut Criterion) {
    let source = r#"
        fn fibonacci(n: i64) -> i64 {
            if n <= 1 {
                return n;
            }
            return fibonacci(n - 1) + fibonacci(n - 2);
        }
        
        fn main() {
            let result = fibonacci(10);
            print_int(result);
        }
    "#;

    c.bench_function("full_compilation_fibonacci", |b| {
        b.iter(|| {
            let driver = Driver::new();
            let _ = driver
                .compile_string(black_box(source), "fibonacci.pd")
                .unwrap();
        })
    });
}

fn bench_large_program(c: &mut Criterion) {
    // Generate a large program with many functions
    let mut source = String::new();
    for i in 0..100 {
        source.push_str(&format!(
            "fn func_{}(x: i64) -> i64 {{ return x + {}; }}\n",
            i, i
        ));
    }
    source.push_str("fn main() { let x = func_0(42); }\n");

    c.bench_function("compile_100_functions", |b| {
        b.iter(|| {
            let driver = Driver::new();
            let _ = driver
                .compile_string(black_box(&source), "large.pd")
                .unwrap();
        })
    });
}

criterion_group!(
    benches,
    bench_lexer,
    bench_parser,
    bench_full_compilation,
    bench_large_program
);
criterion_main!(benches);
