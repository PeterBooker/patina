use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_esc_html(c: &mut Criterion) {
    let medium = "Normal text with <b>bold</b> and &amp; entities and \"quotes\". ".repeat(10);
    let large_dirty = "<script>alert('xss')</script> & <div class=\"foo\">bar</div> ".repeat(200);
    let large_clean =
        "This is completely plain text without any special characters at all. ".repeat(200);
    let entities = "&amp; &lt; &gt; &quot; &#039; &amp; &lt; &gt; &quot; &#039; ".repeat(50);

    let inputs: Vec<(&str, &str)> = vec![
        ("tiny_clean", "Hello, world!"),
        ("tiny_dirty", "<b>hi</b> & \"you\""),
        ("medium_mixed", &medium),
        ("large_dirty", &large_dirty),
        ("large_clean", &large_clean),
        ("entities_heavy", &entities),
    ];

    let mut group = c.benchmark_group("esc_html");
    for (name, input) in &inputs {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::new("rust", name), input, |b, i| {
            b.iter(|| patina_core::escaping::esc_html(i))
        });
    }
    group.finish();
}

fn bench_esc_attr(c: &mut Criterion) {
    let mixed = "value with <tags> & \"quotes\" and 'apostrophes' ".repeat(20);

    let inputs: Vec<(&str, &str)> = vec![
        ("tiny", "simple value"),
        ("injection", "\" onclick=\"alert(1)\" class=\""),
        ("mixed", &mixed),
    ];

    let mut group = c.benchmark_group("esc_attr");
    for (name, input) in &inputs {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::new("rust", name), input, |b, i| {
            b.iter(|| patina_core::escaping::esc_attr(i))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_esc_html, bench_esc_attr);
criterion_main!(benches);
