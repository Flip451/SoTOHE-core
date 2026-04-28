//! Integration test for the `verify adr-signals` composition path.
//!
//! Test 1 from impl-plan T006: exercises the full
//! `FsAdrFileAdapter` → `Arc<dyn AdrFilePort>` → `VerifyAdrSignalsInteractor`
//! pipeline against the committed fixture directory at
//! `apps/cli/tests/fixtures/adr/`. The fixtures cover all 5 typestate
//! variants (`proposed` / `accepted` / `implemented` / `superseded` /
//! `deprecated`) plus a `grandfathered: true` decision; every entry is
//! constructed to land in the Blue band (`user_decision_ref` set, or
//! grandfathered exemption per ADR D4). Verifies `red_count == 0`.

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;

use domain::AdrFilePort;
use infrastructure::adr_decision::FsAdrFileAdapter;
use usecase::verify_adr_signals::{
    VerifyAdrSignals, VerifyAdrSignalsCommand, VerifyAdrSignalsInteractor,
};

fn fixture_adr_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/adr")
}

#[test]
fn test_interactor_against_committed_fixtures_yields_zero_red_count() {
    let adr_dir = fixture_adr_dir();
    assert!(adr_dir.is_dir(), "fixture directory missing: {}", adr_dir.display());

    let adapter = FsAdrFileAdapter::new(adr_dir);
    let port: Arc<dyn AdrFilePort> = Arc::new(adapter);
    let interactor = VerifyAdrSignalsInteractor::new(port);

    let report = interactor
        .verify(VerifyAdrSignalsCommand)
        .expect("verify must succeed against committed fixtures");

    assert_eq!(
        report.red_count(),
        0,
        "fixtures must not produce any Red signal (got blue={} yellow={} red={})",
        report.blue_count(),
        report.yellow_count(),
        report.red_count(),
    );
    // 5 typestate fixtures with user_decision_ref + 1 grandfathered = 6 Blue.
    assert_eq!(
        report.blue_count(),
        6,
        "expected 6 Blue (5 typestate + 1 grandfathered), got blue={} yellow={} red={}",
        report.blue_count(),
        report.yellow_count(),
        report.red_count(),
    );
    assert_eq!(report.yellow_count(), 0);
}
