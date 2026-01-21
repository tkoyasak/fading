use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use fading_ls::diagnostics::diagnose;
use fading_ls::document::Document;
use tower_lsp_server::ls_types::TextDocumentContentChangeEvent;

fn bench_diagnose_medium(c: &mut Criterion) {
    let content = include_str!("../fixtures/medium.md");
    let doc = Document::new(None, false, content.to_string());

    c.bench_function("diagnose_medium", |b| b.iter(|| diagnose(black_box(&doc))));
}

fn bench_diagnose_with_changes(c: &mut Criterion) {
    let content = include_str!("../fixtures/medium.md");

    let mut group = c.benchmark_group("diagnose_with_changes");

    // Initial document
    group.bench_function("initial", |b| {
        let doc = Document::new(None, false, content.to_string());
        b.iter(|| diagnose(black_box(&doc)))
    });

    // After small edit (simulates incremental parsing)
    group.bench_function("after_small_edit", |b| {
        let mut doc = Document::new(None, false, content.to_string());
        // Simulate a small change
        doc.update(
            2,
            &[TextDocumentContentChangeEvent {
                range: Some(tower_lsp_server::ls_types::Range::new(
                    tower_lsp_server::ls_types::Position::new(10, 0),
                    tower_lsp_server::ls_types::Position::new(10, 5),
                )),
                range_length: None,
                text: "Hello".to_string(),
            }],
        );

        b.iter(|| diagnose(black_box(&doc)))
    });

    group.finish();
}

fn bench_document_operations(c: &mut Criterion) {
    let content = include_str!("../fixtures/medium.md");

    let mut group = c.benchmark_group("document_operations");

    // Document creation (includes initial parse)
    group.bench_function("new", |b| {
        b.iter(|| Document::new(None, false, black_box(content.to_string())))
    });

    // Document clone (Arc overhead)
    group.bench_function("clone", |b| {
        let doc = Document::new(None, false, content.to_string());
        b.iter(|| black_box(&doc).clone())
    });

    // Apply change + update (incremental parse)
    group.bench_function("apply_change_and_update", |b| {
        b.iter_batched(
            || Document::new(None, false, content.to_string()),
            |mut doc| {
                doc.update(
                    2,
                    &[TextDocumentContentChangeEvent {
                        range: Some(tower_lsp_server::ls_types::Range::new(
                            tower_lsp_server::ls_types::Position::new(10, 0),
                            tower_lsp_server::ls_types::Position::new(10, 5),
                        )),
                        range_length: None,
                        text: "World".to_string(),
                    }],
                );
                doc
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_diagnose_medium,
    bench_diagnose_with_changes,
    bench_document_operations
);
criterion_main!(benches);
