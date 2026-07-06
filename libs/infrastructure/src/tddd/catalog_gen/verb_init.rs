//! `sotp catalog init` (D8 / IN-02 / AC-02): generate empty per-layer catalogue
//! skeletons for every TDDD layer, all-or-nothing.

use std::path::{Path, PathBuf};

use usecase::catalog_gen::{CatalogError, CatalogInitReport};

use super::fs_access::{catalogue_present, load_bindings, port_error, track_dir, write_catalogue};
use super::json_build::empty_catalogue_value;
use crate::tddd::catalogue_document_codec::derive_filename_stem;
use crate::verify::tddd_layers::TdddLayerBinding;

/// A single catalogue file to create.
struct Target {
    path: PathBuf,
    crate_name: String,
    layer: String,
}

/// Generate the empty catalogue skeleton for every TDDD layer of the track.
///
/// # Errors
///
/// Returns [`CatalogError::FileExists`] if any target file already exists
/// (no partial generation), or [`CatalogError::Port`] on filesystem failure.
pub(super) fn run(track_id: &str, items_dir: &Path) -> Result<CatalogInitReport, CatalogError> {
    let bindings = load_bindings(items_dir)?;
    let dir = track_dir(items_dir, track_id)?;
    let targets = plan_targets(&bindings, &dir);
    create_skeletons(&dir, items_dir, &targets)
}

/// Compute the target catalogue files from the layer bindings.
fn plan_targets(bindings: &[TdddLayerBinding], dir: &Path) -> Vec<Target> {
    bindings
        .iter()
        .map(|binding| {
            let file = binding.catalogue_file();
            Target {
                path: dir.join(file),
                crate_name: derive_filename_stem(Path::new(file)),
                layer: binding.layer_id().to_owned(),
            }
        })
        .collect()
}

/// Create every target skeleton, failing closed if any already exists.
fn create_skeletons(
    dir: &Path,
    trusted_root: &Path,
    targets: &[Target],
) -> Result<CatalogInitReport, CatalogError> {
    for target in targets {
        if catalogue_present(&target.path, trusted_root)? {
            return Err(CatalogError::FileExists { path: target.path.clone() });
        }
    }
    if !dir.exists() {
        std::fs::create_dir_all(dir)
            .map_err(|err| port_error(format!("failed to create {}: {err}", dir.display())))?;
    }
    let mut created = Vec::new();
    for target in targets {
        let value = match empty_catalogue_value(&target.crate_name, &target.layer) {
            Ok(value) => value,
            Err(err) => return fail_after_partial_create(&created, trusted_root, err),
        };
        if let Err(err) = write_catalogue(&target.path, trusted_root, &value) {
            return fail_after_write_error(&created, &target.path, trusted_root, err);
        }
        created.push(target.path.clone());
    }
    let created = created.into_iter().map(|path| path.display().to_string()).collect();
    Ok(CatalogInitReport { created_files: created })
}

fn fail_after_partial_create(
    created_paths: &[PathBuf],
    trusted_root: &Path,
    err: CatalogError,
) -> Result<CatalogInitReport, CatalogError> {
    if let Err(rollback_err) = rollback_created_files(created_paths, trusted_root) {
        return Err(port_error(format!(
            "catalog init failed: {err}; rollback failed: {rollback_err}"
        )));
    }
    Err(err)
}

fn fail_after_write_error(
    created_paths: &[PathBuf],
    failed_path: &Path,
    trusted_root: &Path,
    err: CatalogError,
) -> Result<CatalogInitReport, CatalogError> {
    let mut rollback_paths = created_paths.to_vec();
    rollback_paths.push(failed_path.to_path_buf());
    fail_after_partial_create(&rollback_paths, trusted_root, err)
}

fn rollback_created_files(
    created_paths: &[PathBuf],
    trusted_root: &Path,
) -> Result<(), CatalogError> {
    for path in created_paths.iter().rev() {
        if catalogue_present(path, trusted_root)? {
            std::fs::remove_file(path).map_err(|err| {
                port_error(format!("failed to remove partial catalogue {}: {err}", path.display()))
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_create_skeletons_fresh_then_existing() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("track/items/my-track");
        let targets = vec![
            Target {
                path: dir.join("domain-types.json"),
                crate_name: "domain".to_owned(),
                layer: "domain".to_owned(),
            },
            Target {
                path: dir.join("usecase-types.json"),
                crate_name: "usecase".to_owned(),
                layer: "usecase".to_owned(),
            },
        ];

        let report = create_skeletons(&dir, temp.path(), &targets).unwrap();
        assert_eq!(report.created_files.len(), 2);
        assert!(dir.join("domain-types.json").exists());
        assert!(dir.join("usecase-types.json").exists());

        // Re-running with an existing file is rejected (all-or-nothing).
        let err = create_skeletons(&dir, temp.path(), &targets).unwrap_err();
        assert!(matches!(err, CatalogError::FileExists { .. }));
    }

    #[test]
    fn test_create_skeletons_no_partial_when_one_exists() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("track");
        std::fs::create_dir_all(&dir).unwrap();
        // Pre-create the second target only.
        std::fs::write(dir.join("usecase-types.json"), "{}").unwrap();
        let targets = vec![
            Target {
                path: dir.join("domain-types.json"),
                crate_name: "domain".to_owned(),
                layer: "domain".to_owned(),
            },
            Target {
                path: dir.join("usecase-types.json"),
                crate_name: "usecase".to_owned(),
                layer: "usecase".to_owned(),
            },
        ];

        let err = create_skeletons(&dir, temp.path(), &targets).unwrap_err();
        assert!(matches!(err, CatalogError::FileExists { .. }));
        // The first target must NOT have been written (no partial generation).
        assert!(!dir.join("domain-types.json").exists());
    }

    #[test]
    fn test_create_skeletons_rolls_back_after_write_failure() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("track");
        let first = dir.join("domain-types.json");
        let second = dir.join("missing-parent/usecase-types.json");
        let targets = vec![
            Target {
                path: first.clone(),
                crate_name: "domain".to_owned(),
                layer: "domain".to_owned(),
            },
            Target {
                path: second.clone(),
                crate_name: "usecase".to_owned(),
                layer: "usecase".to_owned(),
            },
        ];

        let err = create_skeletons(&dir, temp.path(), &targets).unwrap_err();
        assert!(matches!(err, CatalogError::Port { .. }));
        assert!(!first.exists());
        assert!(!second.exists());
    }

    #[test]
    fn test_fail_after_write_error_rolls_back_failed_target() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("track");
        std::fs::create_dir_all(&dir).unwrap();
        let first = dir.join("domain-types.json");
        let failed = dir.join("usecase-types.json");
        std::fs::write(&first, "{}").unwrap();
        std::fs::write(&failed, "partial").unwrap();

        let err = fail_after_write_error(
            std::slice::from_ref(&first),
            &failed,
            temp.path(),
            port_error("simulated partial write failure"),
        )
        .unwrap_err();

        assert!(matches!(err, CatalogError::Port { .. }));
        assert!(!first.exists());
        assert!(!failed.exists());
    }
}
