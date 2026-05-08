use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mlocate::db::trigram;

fn bench_trigram_generation(c: &mut Criterion) {
    c.bench_function("trigram: generate (60 chars)", |b| {
        b.iter(|| {
            trigram::generate_trigrams(black_box(
                "/home/user/projects/mlocate/src/pipeline/extractor_worker.rs"
            ))
        })
    });

    c.bench_function("trigram: generate lowercase (60 chars)", |b| {
        b.iter(|| {
            trigram::generate_trigrams_lowercase(black_box(
                "/home/user/PROJECTS/MLOCATE/SRC/PIPELINE/EXTRACTOR_WORKER.RS"
            ))
        })
    });

    c.bench_function("trigram: short path (2 chars)", |b| {
        b.iter(|| {
            trigram::generate_trigrams(black_box("/a"))
        })
    });
}

fn bench_filter_parsing(c: &mut Criterion) {
    c.bench_function("filter: parse size '10MB+'", |b| {
        b.iter(|| {
            mlocate::filter::parse_size(black_box("10MB+"))
        })
    });

    c.bench_function("filter: parse modified '2d-'", |b| {
        b.iter(|| {
            mlocate::filter::parse_modified(black_box("2d-"))
        })
    });

    c.bench_function("filter: parse mime 'image/png'", |b| {
        b.iter(|| {
            mlocate::filter::parse_mime_type(black_box("image/png"))
        })
    });
}

criterion_group!(benches, bench_trigram_generation, bench_filter_parsing);
criterion_main!(benches);
