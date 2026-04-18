use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_sanitize_title_with_dashes(c: &mut Criterion) {
    let medium = "A Long Title With  Multiple  Spaces and <em>tags</em> ".repeat(4);
    let dirty = "<script>alert('xss')</script> 50% off &amp; stuff ".repeat(20);
    let unicode = "Café résumé ".repeat(10);
    let cjk = "日本語のタイトル ".repeat(10);
    let truncate = "a".repeat(400);

    let inputs: Vec<(&str, &str)> = vec![
        ("tiny_ascii", "Hello World"),
        ("tiny_html", "<em>Title</em>"),
        ("medium_mixed", &medium),
        ("dirty", &dirty),
        ("unicode", &unicode),
        ("cjk", &cjk),
        ("truncate_200", &truncate),
    ];

    let mut group = c.benchmark_group("sanitize_title_with_dashes/display");
    for (name, input) in &inputs {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::new("rust", name), input, |b, i| {
            b.iter(|| patina_core::sanitize::title::sanitize_title_with_dashes(i, "", "display"))
        });
    }
    group.finish();

    // The `save` context fires the extra replacement chain (six str_replace
    // passes against fixed needles). Benched separately so the delta is
    // visible against the display path.
    let save_inputs: Vec<(&str, &str)> = vec![
        ("tiny_ascii", "Hello World"),
        ("with_slash", "foo/bar/baz"),
        ("with_em_dash", "hello\u{2014}world"),
        ("with_nbsp", "non\u{00A0}breaking"),
    ];
    let mut group = c.benchmark_group("sanitize_title_with_dashes/save");
    for (name, input) in &save_inputs {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::new("rust", name), input, |b, i| {
            b.iter(|| patina_core::sanitize::title::sanitize_title_with_dashes(i, "", "save"))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_sanitize_title_with_dashes);
criterion_main!(benches);
