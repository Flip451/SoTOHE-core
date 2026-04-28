//! Integration test for the `verify adr-signals` composition path.
//!
//! Test 1 from impl-plan T006: exercises the full
//! `FsAdrFileAdapter` → `Arc<dyn AdrFilePort>` → `VerifyAdrSignalsInteractor`
//! pipeline against the committed fixture directory at
//! `apps/cli/tests/fixtures/adr/`. The fixtures cover all 5 typestate
//! variants (`proposed` / `accepted` / `implemented` / `superseded` /
//! `deprecated`) plus a `grandfathered: true` decision; the 5 typestate
//! entries land in the Blue band via `user_decision_ref`, while the
//! grandfathered entry is excluded from signal evaluation per ADR D4 and
//! counted in `grandfathered_count` instead. Verifies `red_count == 0`.

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
        "fixtures must not produce any Red signal (got blue={} yellow={} red={} grandfathered={})",
        report.blue_count(),
        report.yellow_count(),
        report.red_count(),
        report.grandfathered_count(),
    );
    // 5 typestate fixtures with user_decision_ref → Blue.
    // 1 grandfathered fixture → grandfathered band (excluded from signals per D4).
    assert_eq!(
        report.blue_count(),
        5,
        "expected 5 Blue (typestate fixtures), got blue={} yellow={} red={} grandfathered={}",
        report.blue_count(),
        report.yellow_count(),
        report.red_count(),
        report.grandfathered_count(),
    );
    assert_eq!(report.yellow_count(), 0);
    assert_eq!(report.grandfathered_count(), 1);
}
