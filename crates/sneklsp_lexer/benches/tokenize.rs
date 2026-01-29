use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};

const SIMPLE_EXPR: &str = include_str!("../../../testdata/simple/expressions.py");
const SIMPLE_FUNCTION: &str = include_str!("../../../testdata/simple/functions.py");
const HELLO_WORLD: &str = include_str!("../../../testdata/simple/hello.py");

fn bench_tokenize(c: &mut Criterion) {
    let mut group = c.benchmark_group("tokenize");

    group.throughput(Throughput::Bytes(SIMPLE_EXPR.len() as u64));
    group.bench_function("simple_expr", |b| {
        b.iter(|| sneklsp_lexer::tokenize(black_box(SIMPLE_EXPR)));
    });

    group.throughput(Throughput::Bytes(SIMPLE_FUNCTION.len() as u64));
    group.bench_function("simple_function", |b| {
        b.iter(|| sneklsp_lexer::tokenize(black_box(SIMPLE_FUNCTION)));
    });

    group.throughput(Throughput::Bytes(HELLO_WORLD.len() as u64));
    group.bench_function("hello_world", |b| {
        b.iter(|| sneklsp_lexer::tokenize(black_box(HELLO_WORLD)));
    });

    group.finish();
}

criterion_group!(benches, bench_tokenize);
criterion_main!(benches);
