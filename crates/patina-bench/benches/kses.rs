use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_wp_kses_post(c: &mut Criterion) {
    let small =
        "<p>Simple <b>paragraph</b> with <a href=\"http://example.com\">link</a>.</p>".to_string();
    let medium =
        "<p>Paragraph <b>bold</b> and <a href=\"http://example.com\">link</a>.</p>".repeat(10);
    let with_script = "<p>Safe <script>alert('xss')</script> text</p>".repeat(5);
    let large = "<div class=\"container\"><p>Content with <strong>formatting</strong>, \
        <a href=\"http://example.com\" title=\"Link\">links</a>, and &amp; entities.</p></div>"
        .repeat(20);

    let inputs: Vec<(&str, &str)> = vec![
        ("small_76B", &small),
        ("medium_740B", &medium),
        ("with_script", &with_script),
        ("large_3KB", &large),
    ];

    let mut group = c.benchmark_group("wp_kses_post");
    for (name, input) in &inputs {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::new("rust", name), input, |b, i| {
            b.iter(|| patina_core::kses::wp_kses_post(i))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_wp_kses_post);
criterion_main!(benches);
