//! Integration tests for [`LanceDbSemanticIndexAdapter`].
//!
//! Confirms CN-01 (local-only operation): the adapter stores and retrieves
//! fragments via a temporary directory — no network access, no real embedding
//! model.  Synthetic embedding vectors are used throughout.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::PathBuf;

use domain::semantic_dup::{CodeFragment, TopK};
use infrastructure::semantic_dup::index::LanceDbSemanticIndexAdapter;
use usecase::semantic_dup::SemanticIndexPort as _;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_fragment(path: &str, content: &str) -> CodeFragment {
    CodeFragment::new(PathBuf::from(path), content.to_owned(), 1, 1).unwrap()
}

/// Build a synthetic unit vector with a `1.0` at `hot_dim` and `0.0` elsewhere.
fn unit_vec(dim: usize, hot_dim: usize) -> Vec<f32> {
    let mut v = vec![0.0_f32; dim];
    v[hot_dim] = 1.0;
    v
}

// ── Round-trip tests ──────────────────────────────────────────────────────────

/// Insert two fragments with linearly independent embedding vectors and verify
/// that searching with each vector returns the matching fragment as the top
/// result (cosine similarity = 1.0 for the exact match).
///
/// This confirms CN-01: the adapter operates entirely on local storage (the
/// temp directory) without any network call or real embedding model.
#[test]
fn test_lance_db_adapter_insert_and_search_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = LanceDbSemanticIndexAdapter::new(dir.path().to_path_buf()).unwrap();

    let frag_a = make_fragment("src/a.rs", "fn a() {}");
    let frag_b = make_fragment("src/b.rs", "fn b() {}");

    // Orthogonal 4-dimensional unit vectors.
    let emb_a = unit_vec(4, 0); // [1, 0, 0, 0]
    let emb_b = unit_vec(4, 1); // [0, 1, 0, 0]

    adapter.insert(&frag_a, &emb_a).unwrap();
    adapter.insert(&frag_b, &emb_b).unwrap();

    // Search with emb_a — frag_a should come first.
    let top_k = TopK::new(2).unwrap();
    let results_a = adapter.search(&emb_a, top_k).unwrap();

    assert_eq!(results_a.len(), 2, "expected both fragments in the results");
    // The result closest to emb_a is frag_a (score ≈ 1.0).
    assert_eq!(
        results_a[0].fragment.source_path,
        PathBuf::from("src/a.rs"),
        "frag_a should be the top result when queried with emb_a"
    );
    // frag_b is orthogonal to emb_a — its cosine similarity = 0.0.
    assert!(
        results_a[0].score.value() > results_a[1].score.value(),
        "top result should have higher score than second result"
    );

    // Search with emb_b — frag_b should come first (symmetric verification).
    let top_k = TopK::new(2).unwrap();
    let results_b = adapter.search(&emb_b, top_k).unwrap();

    assert_eq!(results_b.len(), 2, "expected both fragments in the results");
    // The result closest to emb_b is frag_b (score ≈ 1.0).
    assert_eq!(
        results_b[0].fragment.source_path,
        PathBuf::from("src/b.rs"),
        "frag_b should be the top result when queried with emb_b"
    );
    // frag_a is orthogonal to emb_b — its cosine similarity = 0.0.
    assert!(
        results_b[0].score.value() > results_b[1].score.value(),
        "top result should have higher score than second result"
    );
}

/// Search on an empty database returns an empty result set without error.
///
/// LanceDB's graceful "table not found" path (see the `search` implementation's
/// `is_table_not_found` check) should be exercised here.
#[test]
fn test_lance_db_adapter_search_on_empty_db_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = LanceDbSemanticIndexAdapter::new(dir.path().to_path_buf()).unwrap();

    let top_k = TopK::new(5).unwrap();
    let query = unit_vec(4, 0);
    let results = adapter.search(&query, top_k).unwrap();

    assert!(results.is_empty(), "search on empty DB should return empty results");
}

/// Insert a single fragment; searching with its exact embedding vector should
/// return a single result with a high similarity score (≈ 1.0 for cosine).
#[test]
fn test_lance_db_adapter_single_insert_top1_search_returns_the_fragment() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = LanceDbSemanticIndexAdapter::new(dir.path().to_path_buf()).unwrap();

    let frag = make_fragment("src/only.rs", "fn only() {}");
    let emb = unit_vec(8, 3); // 8-dim, hot at index 3

    adapter.insert(&frag, &emb).unwrap();

    let top_k = TopK::new(1).unwrap();
    let results = adapter.search(&emb, top_k).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].fragment.source_path, PathBuf::from("src/only.rs"));
    assert_eq!(results[0].fragment.content(), "fn only() {}");
    // Exact-match cosine similarity: LanceDB distance ≈ 0.0, score ≈ 1.0.
    assert!(
        results[0].score.value() > 0.9,
        "expected similarity > 0.9 for exact vector match, got {}",
        results[0].score.value()
    );
}

/// Insert three fragments and verify that the result ordering reflects cosine
/// similarity ranking (most similar first).
///
/// - Query vector: [1, 0, 0, 0]
/// - frag_a: [1, 0, 0, 0]  → similarity = 1.0 (exact match)
/// - frag_b: [0.8, 0.6, 0, 0] → similarity ≈ 0.8
/// - frag_c: [0, 1, 0, 0]  → similarity = 0.0 (orthogonal)
#[test]
fn test_lance_db_adapter_search_results_ordered_by_descending_similarity() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = LanceDbSemanticIndexAdapter::new(dir.path().to_path_buf()).unwrap();

    let frag_a = make_fragment("src/a.rs", "fn a() {}");
    let frag_b = make_fragment("src/b.rs", "fn b() {}");
    let frag_c = make_fragment("src/c.rs", "fn c() {}");

    let emb_a = vec![1.0_f32, 0.0, 0.0, 0.0];
    let emb_b = vec![0.8_f32, 0.6, 0.0, 0.0]; // partially aligned with a
    let emb_c = vec![0.0_f32, 1.0, 0.0, 0.0]; // orthogonal to a

    adapter.insert(&frag_a, &emb_a).unwrap();
    adapter.insert(&frag_b, &emb_b).unwrap();
    adapter.insert(&frag_c, &emb_c).unwrap();

    let query = vec![1.0_f32, 0.0, 0.0, 0.0];
    let top_k = TopK::new(3).unwrap();
    let results = adapter.search(&query, top_k).unwrap();

    assert_eq!(results.len(), 3);
    // Results should be in descending similarity order.
    assert!(
        results[0].score.value() >= results[1].score.value(),
        "first result ({}) should have score >= second result ({})",
        results[0].score.value(),
        results[1].score.value()
    );
    assert!(
        results[1].score.value() >= results[2].score.value(),
        "second result ({}) should have score >= third result ({})",
        results[1].score.value(),
        results[2].score.value()
    );
    // frag_a (exact match) should be first.
    assert_eq!(
        results[0].fragment.source_path,
        PathBuf::from("src/a.rs"),
        "exact-match fragment should be ranked first"
    );
}
