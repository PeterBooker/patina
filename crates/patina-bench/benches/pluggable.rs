use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_sanitize_redirect(c: &mut Criterion) {
    let long_url = format!("http://example.com/{}", "a".repeat(10000));

    let inputs: Vec<(&str, &str)> = vec![
        ("simple", "http://example.com/page"),
        ("with_spaces", "http://example.com/my page/here"),
        ("unicode", "http://example.com/日本語/ページ?q=テスト"),
        (
            "dirty",
            "http://example.com/<script>alert(1)</script>?foo=bar&baz=qux",
        ),
        ("empty", ""),
        ("long", &long_url),
    ];

    let mut group = c.benchmark_group("wp_sanitize_redirect");
    for (name, input) in &inputs {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::new("rust", name), input, |b, i| {
            b.iter(|| patina_core::pluggable::sanitize_redirect(i))
        });
    }
    group.finish();
}

fn bench_validate_redirect(c: &mut Criterion) {
    let inputs = vec![
        ("same_host", "http://example.com/page"),
        ("relative", "/path/to/page"),
        ("different_host", "http://evil.com/phish"),
        ("javascript", "javascript:alert(1)"),
        ("protocol_relative", "//example.com/page"),
    ];

    let allowed = ["example.com"];

    let mut group = c.benchmark_group("wp_validate_redirect");
    for (name, input) in &inputs {
        group.bench_with_input(BenchmarkId::new("rust", name), input, |b, i| {
            b.iter(|| patina_core::pluggable::validate_redirect(i, "example.com", &allowed, None))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_sanitize_redirect, bench_validate_redirect);
criterion_main!(benches);
