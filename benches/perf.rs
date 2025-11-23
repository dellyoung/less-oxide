use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use less_oxide::{compile, CompileOptions};

struct Case {
    name: &'static str,
    source: &'static str,
    minify: bool,
}

fn compile_benchmarks(c: &mut Criterion) {
    let cases = [
        Case {
            name: "baseline_pretty",
            source: include_str!("../fixtures/benchmark.less"),
            minify: false,
        },
        Case {
            name: "baseline_minified",
            source: include_str!("../fixtures/benchmark.less"),
            minify: true,
        },
        Case {
            name: "import_pretty",
            source: include_str!("../fixtures/import.less"),
            minify: false,
        },
        Case {
            name: "import_minified",
            source: include_str!("../fixtures/import.less"),
            minify: true,
        },
        Case {
            name: "mixins_pretty",
            source: include_str!("../fixtures/mixins.less"),
            minify: false,
        },
        Case {
            name: "mixins_minified",
            source: include_str!("../fixtures/mixins.less"),
            minify: true,
        },
        Case {
            name: "arithmetic_pretty",
            source: include_str!("../fixtures/arithmetic.less"),
            minify: false,
        },
        Case {
            name: "arithmetic_minified",
            source: include_str!("../fixtures/arithmetic.less"),
            minify: true,
        },
    ];

    for case in cases {
        bench_case(c, &case);
    }
}

fn bench_case(c: &mut Criterion, case: &Case) {
    let mut group = c.benchmark_group(format!("less_compile/{}", case.name));
    group.throughput(Throughput::Bytes(case.source.len() as u64));

    let id = BenchmarkId::new(case.name, if case.minify { "min" } else { "pretty" });
    group.bench_with_input(id, &case.minify, |b, &minify| {
        b.iter(|| {
            compile(
                case.source,
                CompileOptions {
                    minify,
                    ..CompileOptions::default()
                },
            )
            .unwrap()
        });
    });

    group.finish();
}

criterion_group!(benches, compile_benchmarks);
criterion_main!(benches);
