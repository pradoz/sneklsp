use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};

const SIMPLE_EXPR: &str = include_str!("../../../testdata/simple/expressions.py");
const SIMPLE_FUNCTION: &str = include_str!("../../../testdata/simple/functions.py");
const SIMPLE_500_LINES: &str = include_str!("../../../testdata/simple/500lines.py");

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

    group.throughput(Throughput::Bytes(SIMPLE_500_LINES.len() as u64));
    group.bench_function("500_lines", |b| {
        b.iter(|| sneklsp_lexer::tokenize(black_box(SIMPLE_500_LINES)));
    });

    group.finish();
}

criterion_group!(benches, bench_tokenize);
criterion_main!(benches);
