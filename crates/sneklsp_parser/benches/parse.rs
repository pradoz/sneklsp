use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use sneklsp_ast::AstArena;

const SIMPLE_EXPR: &str = include_str!("../../../testdata/simple/expressions.py");
const SIMPLE_FUNCTION: &str = include_str!("../../../testdata/simple/functions.py");
const HELLO_WORLD: &str = include_str!("../../../testdata/simple/hello.py");

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");

    group.throughput(Throughput::Bytes(SIMPLE_EXPR.len() as u64));
    group.bench_function("simple_expr", |b| {
        let arena = AstArena::new();
        b.iter(|| sneklsp_parser::parse(black_box(SIMPLE_EXPR), &arena));
    });

    group.throughput(Throughput::Bytes(SIMPLE_FUNCTION.len() as u64));
    group.bench_function("simple_function", |b| {
        let arena = AstArena::new();
        b.iter(|| sneklsp_parser::parse(black_box(SIMPLE_FUNCTION), &arena));
    });

    group.throughput(Throughput::Bytes(HELLO_WORLD.len() as u64));
    group.bench_function("hello_world", |b| {
        let arena = AstArena::new();
        b.iter(|| sneklsp_parser::parse(black_box(HELLO_WORLD), &arena));
    });

    group.finish();
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
