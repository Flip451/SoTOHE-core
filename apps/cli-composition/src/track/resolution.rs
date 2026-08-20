//! Compatibility resolution tests for the `track` command family.

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use cli_driver::track_resolution::{
        TrackItemsDirectoryInput, TrackResolutionInput, TrackResolutionOutcome,
    };

    fn resolve_active_from_items(items_dir: std::path::PathBuf) -> String {
        let items_dir = TrackItemsDirectoryInput::try_new(items_dir).unwrap();
        match crate::TrackCompositionRoot::new()
            .track_resolution_driver()
            .resolve(TrackResolutionInput::ReadFromItems { track_id: None, items_dir })
        {
            TrackResolutionOutcome::Resolved(track_id) => track_id.to_string(),
            other => panic!("expected resolved track, got {other:?}"),
        }
    }

    #[test]
    fn test_track_resolve_id_date_prefixed_branch_returns_opaque_id() {
        let root = tempfile::tempdir().unwrap();
        let track_id = "2026-07-31-date-prefixed-track";
        crate::test_support::seed_repo(root.path(), &format!("track/{track_id}"));

        let resolved = resolve_active_from_items(root.path().join("track").join("items"));
        assert_eq!(resolved, track_id);
    }

    #[test]
    fn test_track_resolve_id_suffix_form_branch_returns_opaque_id() {
        let root = tempfile::tempdir().unwrap();
        let track_id = "legacy-suffix-track-2026-07-31";
        crate::test_support::seed_repo(root.path(), &format!("track/{track_id}"));

        let resolved = resolve_active_from_items(root.path().join("track").join("items"));
        assert_eq!(resolved, track_id);
    }
}
