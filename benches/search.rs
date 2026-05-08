use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mlocate::index::trigram;
use mlocate::index::format::{IndexWriter, IndexConfig, IndexReader};
use mlocate::index::search::{search, SearchOptions, SearchFilters};

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

fn bench_bitmap_intersection(c: &mut Criterion) {
    let mut writer = IndexWriter::new();
    for i in 0..100_000u32 {
        let path = format!("/home/user/file_{}.txt", i);
        writer.add_file(&path, i as u64 * 100, 1746720000, 0o644, "text/plain");
        let tris = trigram::generate_trigrams_lowercase(&path);
        for tri in &tris {
            let key = trigram::trigram_to_bytes(tri);
            writer.add_trigram_doc(key, i);
        }
    }
    writer.prune_empty();
    let config = IndexConfig {
        indexed_paths: vec![],
        pruned_paths: vec![],
        timestamp: 0,
        hostname: "test".into(),
        total_bytes_indexed: 0,
        mlocate_version: "0.1.0".into(),
    };
    let data = writer.into_bytes(&config).unwrap();
    let data_static: &'static [u8] = Box::leak(data.into_boxed_slice());
    let reader = IndexReader::from_bytes(data_static).unwrap();

    c.bench_function("search: trigram-accelerated 'file_'", |b| {
        b.iter(|| {
            let results = search(
                &reader,
                &["file_".to_string()],
                &SearchOptions::default(),
                &SearchFilters::default(),
            ).unwrap();
            black_box(results.count())
        })
    });

    c.bench_function("search: short pattern 'fi' (linear scan)", |b| {
        b.iter(|| {
            let results = search(
                &reader,
                &["fi".to_string()],
                &SearchOptions::default(),
                &SearchFilters::default(),
            ).unwrap();
            black_box(results.count())
        })
    });
}

fn bench_serialization(c: &mut Criterion) {
    let mut writer = IndexWriter::new();
    for i in 0..10_000u32 {
        let path = format!("/home/user/bench_file_{}.txt", i);
        writer.add_file(&path, i as u64 * 100, 1746720000, 0o644, "text/plain");
        let tris = trigram::generate_trigrams_lowercase(&path);
        for tri in &tris {
            writer.add_trigram_doc(trigram::trigram_to_bytes(tri), i);
        }
    }
    writer.prune_empty();
    let config = IndexConfig {
        indexed_paths: vec![],
        pruned_paths: vec![],
        timestamp: 0,
        hostname: "test".into(),
        total_bytes_indexed: 0,
        mlocate_version: "0.1.0".into(),
    };

    c.bench_function("serialization: into_bytes 10K files", |b| {
        b.iter(|| writer.into_bytes(&config).unwrap())
    });
}

fn bench_deserialization(c: &mut Criterion) {
    let mut writer = IndexWriter::new();
    for i in 0..10_000u32 {
        let path = format!("/home/user/bench_file_{}.txt", i);
        writer.add_file(&path, i as u64 * 100, 1746720000, 0o644, "text/plain");
        let tris = trigram::generate_trigrams_lowercase(&path);
        for tri in &tris {
            writer.add_trigram_doc(trigram::trigram_to_bytes(tri), i);
        }
    }
    writer.prune_empty();
    let config = IndexConfig {
        indexed_paths: vec![],
        pruned_paths: vec![],
        timestamp: 0,
        hostname: "test".into(),
        total_bytes_indexed: 0,
        mlocate_version: "0.1.0".into(),
    };
    let data = writer.into_bytes(&config).unwrap();
    let data_static: &'static [u8] = Box::leak(data.into_boxed_slice());

    c.bench_function("deserialization: IndexReader::from_bytes 10K files", |b| {
        b.iter(|| IndexReader::from_bytes(data_static).unwrap())
    });
}

criterion_group!(benches,
    bench_trigram_generation,
    bench_filter_parsing,
    bench_bitmap_intersection,
    bench_serialization,
    bench_deserialization,
);
criterion_main!(benches);
