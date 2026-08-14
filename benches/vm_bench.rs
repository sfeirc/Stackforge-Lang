use criterion::{criterion_group, criterion_main, Criterion};
use stackforge::{run_ast, run_vm};
use std::io;

const FIB_25: &str = r#"
fn fib(n) {
    if (n < 2) { return n; }
    return fib(n - 1) + fib(n - 2);
}
print fib(25);
"#;

const TIGHT_LOOP: &str = r#"
let sum = 0;
for (let i = 0; i < 1000000; i = i + 1) {
    sum = sum + i;
}
print sum;
"#;

fn bench_recursive_fib(c: &mut Criterion) {
    let mut group = c.benchmark_group("recursive_fib_25");
    group.bench_function("bytecode_vm", |b| {
        b.iter(|| run_vm(FIB_25, &mut io::sink()).unwrap())
    });
    group.bench_function("ast_interpreter", |b| {
        b.iter(|| run_ast(FIB_25, &mut io::sink()).unwrap())
    });
    group.finish();
}

fn bench_tight_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("tight_loop_1e6");
    group.bench_function("bytecode_vm", |b| {
        b.iter(|| run_vm(TIGHT_LOOP, &mut io::sink()).unwrap())
    });
    group.bench_function("ast_interpreter", |b| {
        b.iter(|| run_ast(TIGHT_LOOP, &mut io::sink()).unwrap())
    });
    group.finish();
}

criterion_group!(benches, bench_recursive_fib, bench_tight_loop);
criterion_main!(benches);
