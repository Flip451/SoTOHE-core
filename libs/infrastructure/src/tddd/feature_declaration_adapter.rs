//! Filesystem implementation of the client-specific TDDD feature declaration ports.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use domain::tddd::catalogue_v2::TdddLayerBinding;
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::{CargoFeatureName, LayerId, TdddFeatureDeclaration};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use usecase::tddd_feature_declaration::{
    TdddActualFeatureDeclarationPort, TdddActualFeatureDeclarationPortError,
    TdddBaselineFeatureDeclarationPort, TdddBaselineFeatureDeclarationPortError,
    TdddFeatureDeclarationReadError,
};

use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

const DECLARATION_FILE: &str = "tddd-features.json";
const SNAPSHOT_FILE: &str = "tddd-features-baseline.json";
const MAX_READ_BYTES: usize = 1024 * 1024;
static SNAPSHOT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Filesystem adapter shared by baseline and actual feature-declaration clients.
#[derive(Debug, Clone, Default)]
pub struct FsTdddFeatureDeclarationAdapter;

impl FsTdddFeatureDeclarationAdapter {
    /// Creates a filesystem-backed feature declaration adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl TdddBaselineFeatureDeclarationPort for FsTdddFeatureDeclarationAdapter {
    fn load_for_baseline(
        &self,
        track_dir: &Path,
        workspace_root: &Path,
        layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddBaselineFeatureDeclarationPortError> {
        validate_trusted_roots(track_dir, workspace_root)
            .map_err(TdddBaselineFeatureDeclarationPortError::Read)?;
        let (declaration, bytes) = load_declaration(track_dir, workspace_root, layers)
            .map_err(TdddBaselineFeatureDeclarationPortError::Read)?;
        let snapshot_path = track_dir.join(SNAPSHOT_FILE);
        match read_bytes(&snapshot_path, track_dir) {
            Ok(Some(snapshot)) if snapshot == bytes => Ok(declaration),
            Ok(Some(_)) => Err(TdddBaselineFeatureDeclarationPortError::BaselineSnapshotMismatch),
            Ok(None) => match write_first_snapshot(&snapshot_path, track_dir, &bytes) {
                Ok(()) => Ok(declaration),
                Err(SnapshotPublicationError::Mismatch) => {
                    Err(TdddBaselineFeatureDeclarationPortError::BaselineSnapshotMismatch)
                }
                Err(SnapshotPublicationError::Read(reason)) => {
                    Err(TdddBaselineFeatureDeclarationPortError::Read(read_error(
                        &snapshot_path,
                        reason.as_str().to_owned(),
                    )))
                }
                Err(SnapshotPublicationError::Write(reason)) => {
                    Err(TdddBaselineFeatureDeclarationPortError::SnapshotWrite {
                        path: snapshot_path,
                        reason,
                    })
                }
            },
            Err(reason) => Err(TdddBaselineFeatureDeclarationPortError::Read(read_error(
                &snapshot_path,
                reason.as_str().to_owned(),
            ))),
        }
    }
}

impl TdddActualFeatureDeclarationPort for FsTdddFeatureDeclarationAdapter {
    fn load_for_actual(
        &self,
        track_dir: &Path,
        workspace_root: &Path,
        layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError> {
        validate_trusted_roots(track_dir, workspace_root)
            .map_err(TdddActualFeatureDeclarationPortError::Read)?;
        let (declaration, bytes) = load_declaration(track_dir, workspace_root, layers)
            .map_err(TdddActualFeatureDeclarationPortError::Read)?;
        let snapshot_path = track_dir.join(SNAPSHOT_FILE);
        let snapshot = read_bytes(&snapshot_path, track_dir).map_err(|reason| {
            TdddActualFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::ReadDeclaration {
                    path: snapshot_path.clone(),
                    reason,
                },
            )
        })?;
        let snapshot =
            snapshot.ok_or(TdddActualFeatureDeclarationPortError::MissingBaselineSnapshot {
                path: snapshot_path,
            })?;
        if snapshot == bytes {
            Ok(declaration)
        } else {
            Err(TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch)
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureDeclarationDto {
    schema_version: u32,
    #[serde(deserialize_with = "deserialize_layer_map")]
    layers: BTreeMap<String, Vec<String>>,
}

/// Deserializes layer declarations while rejecting duplicate layer keys.
///
/// Serde's standard map implementation uses last-wins semantics, which would make an ambiguous
/// declaration silently select one layer configuration. The visitor follows the strict map pattern
/// used by the other infrastructure codecs.
fn deserialize_layer_map<'de, D>(deserializer: D) -> Result<BTreeMap<String, Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StrictLayerMapVisitor;

    impl<'de> Visitor<'de> for StrictLayerMapVisitor {
        type Value = BTreeMap<String, Vec<String>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map from layer name to Cargo feature list")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut layers = BTreeMap::new();
            while let Some(layer) = map.next_key::<String>()? {
                let features = map.next_value()?;
                if layers.insert(layer.clone(), features).is_some() {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate key '{layer}' in feature declaration layers"
                    )));
                }
            }
            Ok(layers)
        }
    }

    deserializer.deserialize_map(StrictLayerMapVisitor)
}

fn validate_trusted_roots(
    track_dir: &Path,
    workspace_root: &Path,
) -> Result<(), TdddFeatureDeclarationReadError> {
    validate_trusted_directory(track_dir)?;
    validate_trusted_directory(workspace_root)
}

fn validate_trusted_directory(path: &Path) -> Result<(), TdddFeatureDeclarationReadError> {
    reject_symlinks_up_to_root(path).map_err(|error| read_error(path, error.to_string()))?;
    let metadata = path.symlink_metadata().map_err(|error| read_error(path, error.to_string()))?;
    if !metadata.is_dir() {
        return Err(read_error(path, "trusted root must be an existing directory".to_owned()));
    }
    Ok(())
}

fn load_declaration(
    track_dir: &Path,
    workspace_root: &Path,
    bindings: &[TdddLayerBinding],
) -> Result<(TdddFeatureDeclaration, Vec<u8>), TdddFeatureDeclarationReadError> {
    let declaration_path = track_dir.join(DECLARATION_FILE);
    let bytes = read_bytes(&declaration_path, track_dir).map_err(|reason| {
        TdddFeatureDeclarationReadError::ReadDeclaration { path: declaration_path.clone(), reason }
    })?;
    let bytes = bytes.ok_or_else(|| TdddFeatureDeclarationReadError::MissingDeclaration {
        path: declaration_path.clone(),
    })?;
    let dto: FeatureDeclarationDto = serde_json::from_slice(&bytes).map_err(|error| {
        TdddFeatureDeclarationReadError::DecodeDeclaration {
            path: declaration_path.clone(),
            reason: diagnostic(error.to_string()),
        }
    })?;
    if dto.schema_version != 1 {
        return Err(TdddFeatureDeclarationReadError::DecodeDeclaration {
            path: declaration_path,
            reason: diagnostic(format!(
                "unsupported tddd feature declaration schema version {}",
                dto.schema_version
            )),
        });
    }

    let required_layers = bindings
        .iter()
        .map(|binding| parse_layer(&binding.layer_id, &declaration_path))
        .collect::<Result<Vec<_>, _>>()?;
    let mut declaration_layers = BTreeMap::new();
    for (layer, features) in dto.layers {
        let layer = parse_layer(&layer, &declaration_path)?;
        let features = features
            .into_iter()
            .map(|feature| {
                CargoFeatureName::try_new(feature).map_err(|error| {
                    TdddFeatureDeclarationReadError::DecodeDeclaration {
                        path: declaration_path.clone(),
                        reason: diagnostic(error.to_string()),
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        declaration_layers.insert(layer, features);
    }
    let declaration = TdddFeatureDeclaration::try_new(declaration_layers, &required_layers)
        .map_err(|error| TdddFeatureDeclarationReadError::DecodeDeclaration {
            path: declaration_path,
            reason: diagnostic(error.to_string()),
        })?;
    validate_cargo_features(&declaration, workspace_root, bindings)?;
    Ok((declaration, bytes))
}

fn parse_layer(value: &str, path: &Path) -> Result<LayerId, TdddFeatureDeclarationReadError> {
    LayerId::try_new(value.to_owned()).map_err(|error| {
        TdddFeatureDeclarationReadError::DecodeDeclaration {
            path: path.to_path_buf(),
            reason: diagnostic(error.to_string()),
        }
    })
}

fn validate_cargo_features(
    declaration: &TdddFeatureDeclaration,
    workspace_root: &Path,
    bindings: &[TdddLayerBinding],
) -> Result<(), TdddFeatureDeclarationReadError> {
    for binding in bindings {
        let layer = parse_layer(&binding.layer_id, workspace_root)?;
        let features = declaration.features_for(&layer).map_err(|error| {
            TdddFeatureDeclarationReadError::DecodeDeclaration {
                path: workspace_root.to_path_buf(),
                reason: diagnostic(error.to_string()),
            }
        })?;
        let defined_features = manifest_features(workspace_root, binding)?;
        for feature in features {
            if !defined_features.contains(feature.as_str()) {
                return Err(TdddFeatureDeclarationReadError::UnknownCargoFeature {
                    layer: layer.clone(),
                    feature: feature.clone(),
                });
            }
        }
    }
    Ok(())
}

fn manifest_features(
    workspace_root: &Path,
    binding: &TdddLayerBinding,
) -> Result<BTreeSet<String>, TdddFeatureDeclarationReadError> {
    let target = binding.targets.first().ok_or_else(|| {
        TdddFeatureDeclarationReadError::ReadDeclaration {
            path: workspace_root.to_path_buf(),
            reason: diagnostic(format!("layer '{}' has no rustdoc target", binding.layer_id)),
        }
    })?;
    if binding.targets.len() != 1 {
        return Err(TdddFeatureDeclarationReadError::ReadDeclaration {
            path: workspace_root.to_path_buf(),
            reason: diagnostic(format!(
                "layer '{}' has multiple rustdoc targets",
                binding.layer_id
            )),
        });
    }
    let workspace_manifest = workspace_root.join("Cargo.toml");
    let workspace_text = read_text(&workspace_manifest, workspace_root)?;
    let workspace: toml::Value = toml::from_str(&workspace_text).map_err(|error| {
        TdddFeatureDeclarationReadError::ReadDeclaration {
            path: workspace_manifest.clone(),
            reason: diagnostic(error.to_string()),
        }
    })?;
    let members = workspace
        .get("workspace")
        .and_then(|value| value.get("members"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| TdddFeatureDeclarationReadError::ReadDeclaration {
            path: workspace_manifest.clone(),
            reason: diagnostic("workspace Cargo.toml has no members array".to_owned()),
        })?;
    let member_paths = members
        .iter()
        .map(|member| {
            let member = member.as_str().ok_or_else(|| {
                read_error(
                    &workspace_manifest,
                    "workspace Cargo.toml members entries must be strings".to_owned(),
                )
            })?;
            normalize_workspace_member(member, &workspace_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for member_path in member_paths {
        let manifest_path = workspace_root.join(member_path).join("Cargo.toml");
        if !manifest_path.starts_with(workspace_root) {
            return Err(read_error(
                &workspace_manifest,
                "workspace member resolves outside trusted workspace root".to_owned(),
            ));
        }
        let manifest_text = read_text(&manifest_path, workspace_root)?;
        let manifest: toml::Value = toml::from_str(&manifest_text).map_err(|error| {
            TdddFeatureDeclarationReadError::ReadDeclaration {
                path: manifest_path.clone(),
                reason: diagnostic(error.to_string()),
            }
        })?;
        let package_name = manifest
            .get("package")
            .and_then(|value| value.get("name"))
            .and_then(toml::Value::as_str);
        if package_name == Some(target) {
            return match manifest.get("features") {
                None => Ok(BTreeSet::new()),
                Some(features) => validate_manifest_features(features, &manifest_path),
            };
        }
    }
    Err(TdddFeatureDeclarationReadError::ReadDeclaration {
        path: workspace_manifest,
        reason: diagnostic(format!("could not resolve Cargo manifest for target '{target}'")),
    })
}

fn validate_manifest_features(
    features: &toml::Value,
    manifest_path: &Path,
) -> Result<BTreeSet<String>, TdddFeatureDeclarationReadError> {
    let table = features.as_table().ok_or_else(|| {
        read_error(manifest_path, "Cargo.toml features value must be a table".to_owned())
    })?;
    for (feature, definition) in table {
        let Some(entries) = definition.as_array() else {
            return Err(read_error(
                manifest_path,
                format!("Cargo feature '{feature}' must be an array of strings"),
            ));
        };
        if entries.iter().any(|entry| entry.as_str().is_none()) {
            return Err(read_error(
                manifest_path,
                format!("Cargo feature '{feature}' must be an array of strings"),
            ));
        }
    }
    Ok(table.keys().cloned().collect())
}

fn normalize_workspace_member(
    member: &str,
    workspace_manifest: &Path,
) -> Result<PathBuf, TdddFeatureDeclarationReadError> {
    let member_path = Path::new(member);
    let mut normalized = PathBuf::new();
    for component in member_path.components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(read_error(
                    workspace_manifest,
                    format!("workspace member '{member}' is not a safe relative path"),
                ));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(read_error(
            workspace_manifest,
            "workspace member path must not be empty".to_owned(),
        ));
    }
    Ok(normalized)
}

fn read_text(path: &Path, trusted_root: &Path) -> Result<String, TdddFeatureDeclarationReadError> {
    let bytes = read_bytes(path, trusted_root).map_err(|reason| {
        TdddFeatureDeclarationReadError::ReadDeclaration { path: path.to_path_buf(), reason }
    })?;
    let bytes = bytes.ok_or_else(|| TdddFeatureDeclarationReadError::ReadDeclaration {
        path: path.to_path_buf(),
        reason: diagnostic("file not found".to_owned()),
    })?;
    String::from_utf8(bytes).map_err(|error| TdddFeatureDeclarationReadError::ReadDeclaration {
        path: path.to_path_buf(),
        reason: diagnostic(error.to_string()),
    })
}

fn read_bytes(path: &Path, trusted_root: &Path) -> Result<Option<Vec<u8>>, DiagnosticMessage> {
    match reject_symlinks_below(path, trusted_root) {
        Ok(true) => read_limited_file(path).map(Some),
        Ok(false) => Ok(None),
        Err(error) => Err(diagnostic(error.to_string())),
    }
}

fn read_limited_file(path: &Path) -> Result<Vec<u8>, DiagnosticMessage> {
    let metadata = std::fs::metadata(path).map_err(|error| diagnostic(error.to_string()))?;
    if !metadata.is_file() {
        return Err(diagnostic(format!("expected regular file: {}", path.display())));
    }
    if metadata.len() > MAX_READ_BYTES as u64 {
        return Err(file_size_limit_error(path));
    }

    let mut file = File::open(path).map_err(|error| diagnostic(error.to_string()))?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take((MAX_READ_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| diagnostic(error.to_string()))?;
    if bytes.len() > MAX_READ_BYTES {
        return Err(file_size_limit_error(path));
    }
    Ok(bytes)
}

fn file_size_limit_error(path: &Path) -> DiagnosticMessage {
    diagnostic(format!(
        "file exceeds maximum permitted size of {MAX_READ_BYTES} bytes: {}",
        path.display()
    ))
}

fn read_error(path: &Path, reason: String) -> TdddFeatureDeclarationReadError {
    TdddFeatureDeclarationReadError::ReadDeclaration {
        path: path.to_path_buf(),
        reason: diagnostic(reason),
    }
}

#[derive(Debug)]
enum SnapshotPublicationError {
    Mismatch,
    Read(DiagnosticMessage),
    Write(DiagnosticMessage),
}

fn write_first_snapshot(
    path: &Path,
    trusted_root: &Path,
    bytes: &[u8],
) -> Result<(), SnapshotPublicationError> {
    write_first_snapshot_after_temporary_write(path, trusted_root, bytes, |_| {})
}

/// Creates the immutable baseline snapshot without ever publishing partial bytes.
///
/// The temporary file is created below the trusted root, fully synced, then published with a
/// hard link. `hard_link` has no replacement behavior, so a concurrent writer cannot replace
/// the snapshot selected by the first successful publisher.
fn write_first_snapshot_after_temporary_write(
    path: &Path,
    trusted_root: &Path,
    bytes: &[u8],
    after_temporary_write: impl FnOnce(&Path),
) -> Result<(), SnapshotPublicationError> {
    match reject_symlinks_below(path, trusted_root) {
        Ok(true) => return compare_snapshot(path, trusted_root, bytes),
        Ok(false) => {}
        Err(error) => return Err(SnapshotPublicationError::Write(diagnostic(error.to_string()))),
    }
    let (mut file, temporary) = create_snapshot_temporary_file(path, trusted_root)?;
    if let Err(error) = file.write_all(bytes) {
        drop(file);
        return Err(clean_up_temporary(
            &temporary,
            SnapshotPublicationError::Write(diagnostic(error.to_string())),
        ));
    }
    if let Err(error) = file.sync_all() {
        drop(file);
        return Err(clean_up_temporary(
            &temporary,
            SnapshotPublicationError::Write(diagnostic(error.to_string())),
        ));
    }
    drop(file);

    after_temporary_write(&temporary);

    let publication = match reject_symlinks_below(path, trusted_root) {
        Ok(false) => match std::fs::hard_link(&temporary, path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                compare_snapshot(path, trusted_root, bytes)
            }
            Err(error) => Err(SnapshotPublicationError::Write(diagnostic(error.to_string()))),
        },
        Ok(true) => compare_snapshot(path, trusted_root, bytes),
        Err(error) => Err(SnapshotPublicationError::Write(diagnostic(error.to_string()))),
    };
    if let Err(error) = publication {
        return Err(clean_up_temporary(&temporary, error));
    }
    if let Err(error) = std::fs::remove_file(&temporary) {
        return Err(clean_up_temporary(
            &temporary,
            SnapshotPublicationError::Write(diagnostic(error.to_string())),
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        SnapshotPublicationError::Write(diagnostic("baseline snapshot has no parent".to_owned()))
    })?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| SnapshotPublicationError::Write(diagnostic(error.to_string())))
}

fn create_snapshot_temporary_file(
    path: &Path,
    trusted_root: &Path,
) -> Result<(File, PathBuf), SnapshotPublicationError> {
    let parent = path.parent().ok_or_else(|| {
        SnapshotPublicationError::Write(diagnostic("baseline snapshot has no parent".to_owned()))
    })?;
    for _ in 0..1024 {
        let sequence = SNAPSHOT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary =
            parent.join(format!(".{SNAPSHOT_FILE}.{}.{sequence}.tmp", std::process::id()));
        match reject_symlinks_below(&temporary, trusted_root) {
            Ok(false) => {}
            Ok(true) => continue,
            Err(error) => {
                return Err(SnapshotPublicationError::Write(diagnostic(error.to_string())));
            }
        }
        match OpenOptions::new().write(true).create_new(true).open(&temporary) {
            Ok(file) => return Ok((file, temporary)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(SnapshotPublicationError::Write(diagnostic(error.to_string())));
            }
        }
    }
    Err(SnapshotPublicationError::Write(diagnostic(
        "unable to allocate baseline snapshot temporary file".to_owned(),
    )))
}

fn clean_up_temporary(
    temporary: &Path,
    error: SnapshotPublicationError,
) -> SnapshotPublicationError {
    if let Err(cleanup_error) = std::fs::remove_file(temporary)
        && cleanup_error.kind() != std::io::ErrorKind::NotFound
    {
        return SnapshotPublicationError::Write(diagnostic(format!(
            "{}; additionally unable to remove temporary baseline snapshot: {cleanup_error}",
            snapshot_publication_error_description(&error)
        )));
    }
    error
}

fn snapshot_publication_error_description(error: &SnapshotPublicationError) -> String {
    match error {
        SnapshotPublicationError::Mismatch => {
            "baseline snapshot contains different declaration bytes".to_owned()
        }
        SnapshotPublicationError::Read(reason) | SnapshotPublicationError::Write(reason) => {
            reason.as_str().to_owned()
        }
    }
}

fn compare_snapshot(
    path: &Path,
    trusted_root: &Path,
    bytes: &[u8],
) -> Result<(), SnapshotPublicationError> {
    let Some(snapshot) = read_bytes(path, trusted_root).map_err(SnapshotPublicationError::Read)?
    else {
        return Err(SnapshotPublicationError::Read(diagnostic(
            "baseline snapshot disappeared during write".to_owned(),
        )));
    };
    if snapshot == bytes { Ok(()) } else { Err(SnapshotPublicationError::Mismatch) }
}

fn diagnostic(message: String) -> DiagnosticMessage {
    let mut text = if message.trim().is_empty() {
        "feature declaration operation failed".to_owned()
    } else {
        message
    };
    loop {
        match DiagnosticMessage::try_new(text) {
            Ok(message) => return message,
            Err(_) => text = "feature declaration operation failed".to_owned(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn binding(layer_id: &str, target: &str) -> TdddLayerBinding {
        TdddLayerBinding {
            layer_id: layer_id.to_owned(),
            catalogue_file: format!("{layer_id}-types.json"),
            baseline_file: format!("{layer_id}-types-baseline.json"),
            targets: vec![target.to_owned()],
        }
    }

    fn setup_workspace() -> tempfile::TempDir {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(workspace.path().join("libs/domain")).unwrap();
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/domain\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        std::fs::write(
            workspace.path().join("libs/domain/Cargo.toml"),
            "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[features]\nsemantic-dup = []\n",
        )
        .unwrap();
        workspace
    }

    fn write_declaration(track_dir: &Path, contents: &str) {
        std::fs::write(track_dir.join(DECLARATION_FILE), contents).unwrap();
    }

    #[test]
    fn test_baseline_port_with_valid_declaration_creates_snapshot() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        write_declaration(
            track.path(),
            "{\"schema_version\":1,\"layers\":{\"domain\":[\"semantic-dup\"]}}",
        );
        let declaration = FsTdddFeatureDeclarationAdapter::new()
            .load_for_baseline(track.path(), workspace.path(), &[binding("domain", "domain")])
            .unwrap();
        let features = declaration.features_for(&LayerId::try_new("domain").unwrap()).unwrap();
        assert_eq!(features.first().unwrap().as_str(), "semantic-dup");
        assert!(track.path().join(SNAPSHOT_FILE).exists());
    }

    #[test]
    fn test_baseline_port_with_featureless_layer_preserves_empty_feature_list() {
        let workspace = setup_workspace();
        std::fs::create_dir_all(workspace.path().join("apps/cli")).unwrap();
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/domain\", \"apps/cli\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        std::fs::write(
            workspace.path().join("apps/cli/Cargo.toml"),
            "[package]\nname = \"cli\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        let track = tempfile::tempdir().unwrap();
        write_declaration(
            track.path(),
            "{\"schema_version\":1,\"layers\":{\"cli\":[],\"domain\":[\"semantic-dup\"]}}",
        );

        let declaration = FsTdddFeatureDeclarationAdapter::new()
            .load_for_baseline(
                track.path(),
                workspace.path(),
                &[binding("domain", "domain"), binding("cli", "cli")],
            )
            .unwrap();

        let domain = LayerId::try_new("domain").unwrap();
        let cli = LayerId::try_new("cli").unwrap();
        assert_eq!(
            declaration
                .features_for(&domain)
                .unwrap()
                .iter()
                .map(CargoFeatureName::as_str)
                .collect::<Vec<_>>(),
            ["semantic-dup"]
        );
        assert!(declaration.features_for(&cli).unwrap().is_empty());
    }

    #[test]
    fn test_actual_port_without_baseline_snapshot_returns_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        write_declaration(track.path(), "{\"schema_version\":1,\"layers\":{\"domain\":[]}}");
        let result = FsTdddFeatureDeclarationAdapter::new().load_for_actual(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );
        assert!(matches!(
            result,
            Err(TdddActualFeatureDeclarationPortError::MissingBaselineSnapshot { .. })
        ));
    }

    #[test]
    fn test_actual_port_without_declaration_returns_missing_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        let result = FsTdddFeatureDeclarationAdapter::new().load_for_actual(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );
        assert!(matches!(
            result,
            Err(TdddActualFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::MissingDeclaration { .. }
            ))
        ));
    }

    #[test]
    fn test_actual_port_with_matching_baseline_snapshot_returns_declaration() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        let adapter = FsTdddFeatureDeclarationAdapter::new();
        write_declaration(
            track.path(),
            "{\"schema_version\":1,\"layers\":{\"domain\":[\"semantic-dup\"]}}",
        );
        adapter
            .load_for_baseline(track.path(), workspace.path(), &[binding("domain", "domain")])
            .unwrap();

        let declaration = adapter
            .load_for_actual(track.path(), workspace.path(), &[binding("domain", "domain")])
            .unwrap();

        let layer = LayerId::try_new("domain").unwrap();
        assert_eq!(
            declaration.features_for(&layer).unwrap().first().unwrap().as_str(),
            "semantic-dup"
        );
    }

    #[test]
    fn test_baseline_port_without_declaration_returns_missing_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );
        assert!(matches!(
            result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::MissingDeclaration { .. }
            ))
        ));
    }

    #[test]
    fn test_baseline_port_with_invalid_declaration_returns_decode_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        write_declaration(track.path(), "not JSON");
        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );
        assert!(matches!(
            result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::DecodeDeclaration { .. }
            ))
        ));
    }

    #[test]
    fn test_baseline_port_with_unsupported_schema_returns_decode_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        write_declaration(track.path(), "{\"schema_version\":2,\"layers\":{\"domain\":[]}}");

        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );

        assert!(matches!(
            &result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::DecodeDeclaration { .. }
            ))
        ));
        let Err(TdddBaselineFeatureDeclarationPortError::Read(
            TdddFeatureDeclarationReadError::DecodeDeclaration { reason, .. },
        )) = result
        else {
            return;
        };
        assert!(reason.as_str().contains("unsupported"));
    }

    #[test]
    fn test_baseline_port_with_unknown_declaration_field_returns_decode_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        write_declaration(
            track.path(),
            "{\"schema_version\":1,\"layers\":{\"domain\":[]},\"future_field\":true}",
        );

        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );

        assert!(matches!(
            &result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::DecodeDeclaration { .. }
            ))
        ));
        let Err(TdddBaselineFeatureDeclarationPortError::Read(
            TdddFeatureDeclarationReadError::DecodeDeclaration { reason, .. },
        )) = result
        else {
            return;
        };
        assert!(reason.as_str().contains("unknown field"));
    }

    #[test]
    fn test_baseline_port_with_duplicate_layer_key_returns_decode_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        write_declaration(
            track.path(),
            "{\"schema_version\":1,\"layers\":{\"domain\":[],\"domain\":[\"semantic-dup\"]}}",
        );

        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );

        assert!(matches!(
            &result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::DecodeDeclaration { .. }
            ))
        ));
        let Err(TdddBaselineFeatureDeclarationPortError::Read(
            TdddFeatureDeclarationReadError::DecodeDeclaration { reason, .. },
        )) = result
        else {
            return;
        };
        assert!(reason.as_str().contains("duplicate key"));
    }

    #[test]
    fn test_baseline_port_with_large_declaration_returns_read_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        std::fs::write(track.path().join(DECLARATION_FILE), vec![b'x'; MAX_READ_BYTES + 1])
            .unwrap();

        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );

        assert!(matches!(
            &result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::ReadDeclaration { .. }
            ))
        ));
        let Err(TdddBaselineFeatureDeclarationPortError::Read(
            TdddFeatureDeclarationReadError::ReadDeclaration { reason, .. },
        )) = result
        else {
            return;
        };
        assert!(reason.as_str().contains("maximum permitted size"));
    }

    #[test]
    fn test_baseline_port_with_malformed_workspace_manifest_returns_read_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("Cargo.toml"), "[workspace\nmembers = [").unwrap();
        write_declaration(track.path(), "{\"schema_version\":1,\"layers\":{\"domain\":[]}}");

        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );

        assert!(matches!(
            result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::ReadDeclaration { .. }
            ))
        ));
    }

    #[test]
    fn test_baseline_port_with_non_string_workspace_member_returns_read_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/domain\", 1]\nresolver = \"2\"\n",
        )
        .unwrap();
        write_declaration(track.path(), "{\"schema_version\":1,\"layers\":{\"domain\":[]}}");

        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );

        assert!(matches!(
            result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::ReadDeclaration { .. }
            ))
        ));
    }

    #[test]
    fn test_baseline_port_with_non_table_features_returns_read_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        std::fs::write(
            workspace.path().join("libs/domain/Cargo.toml"),
            "features = []\n[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        write_declaration(track.path(), "{\"schema_version\":1,\"layers\":{\"domain\":[]}}");

        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );

        assert!(matches!(
            result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::ReadDeclaration { .. }
            ))
        ));
    }

    #[test]
    fn test_baseline_port_with_non_string_feature_entry_returns_read_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        std::fs::write(
            workspace.path().join("libs/domain/Cargo.toml"),
            "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[features]\nsemantic-dup = [1]\n",
        )
        .unwrap();
        write_declaration(
            track.path(),
            "{\"schema_version\":1,\"layers\":{\"domain\":[\"semantic-dup\"]}}",
        );

        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );

        assert!(matches!(
            result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::ReadDeclaration { .. }
            ))
        ));
    }

    #[test]
    fn test_baseline_port_with_out_of_root_workspace_member_returns_read_error() {
        let workspace = setup_workspace();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(
            outside.path().join("Cargo.toml"),
            "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"../outside\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        let track = tempfile::tempdir().unwrap();
        write_declaration(track.path(), "{\"schema_version\":1,\"layers\":{\"domain\":[]}}");

        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );

        assert!(matches!(
            &result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::ReadDeclaration { .. }
            ))
        ));
        let Err(TdddBaselineFeatureDeclarationPortError::Read(
            TdddFeatureDeclarationReadError::ReadDeclaration { reason, .. },
        )) = result
        else {
            return;
        };
        assert!(reason.as_str().contains("safe relative path"));
    }

    #[test]
    fn test_baseline_port_with_unknown_cargo_feature_returns_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        write_declaration(
            track.path(),
            "{\"schema_version\":1,\"layers\":{\"domain\":[\"unknown\"]}}",
        );
        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            workspace.path(),
            &[binding("domain", "domain")],
        );
        assert!(matches!(
            result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::UnknownCargoFeature { .. }
            ))
        ));
    }

    #[test]
    fn test_actual_port_with_unknown_cargo_feature_returns_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        let adapter = FsTdddFeatureDeclarationAdapter::new();
        write_declaration(track.path(), "{\"schema_version\":1,\"layers\":{\"domain\":[]}}");
        adapter
            .load_for_baseline(track.path(), workspace.path(), &[binding("domain", "domain")])
            .unwrap();
        write_declaration(
            track.path(),
            "{\"schema_version\":1,\"layers\":{\"domain\":[\"unknown\"]}}",
        );

        let result =
            adapter.load_for_actual(track.path(), workspace.path(), &[binding("domain", "domain")]);

        assert!(matches!(
            result,
            Err(TdddActualFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::UnknownCargoFeature { .. }
            ))
        ));
    }

    #[test]
    fn test_actual_port_with_changed_declaration_returns_snapshot_mismatch() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        write_declaration(track.path(), "{\"schema_version\":1,\"layers\":{\"domain\":[]}}");
        let adapter = FsTdddFeatureDeclarationAdapter::new();
        adapter
            .load_for_baseline(track.path(), workspace.path(), &[binding("domain", "domain")])
            .unwrap();
        write_declaration(
            track.path(),
            "{\"schema_version\":1,\"layers\":{\"domain\":[\"semantic-dup\"]}}",
        );
        let result =
            adapter.load_for_actual(track.path(), workspace.path(), &[binding("domain", "domain")]);
        assert!(matches!(
            result,
            Err(TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch)
        ));
    }

    #[test]
    fn test_snapshot_write_does_not_expose_partial_file() {
        let track = tempfile::tempdir().unwrap();
        let snapshot = track.path().join(SNAPSHOT_FILE);
        let bytes = vec![b'x'; 64 * 1024];
        let (temporary_ready_sender, temporary_ready_receiver) = std::sync::mpsc::channel();
        let (publish_sender, publish_receiver) = std::sync::mpsc::channel();
        let writer_path = snapshot.clone();
        let writer_bytes = bytes.clone();
        let trusted_root = track.path().to_path_buf();

        std::thread::scope(|scope| {
            let writer = scope.spawn(move || {
                write_first_snapshot_after_temporary_write(
                    &writer_path,
                    &trusted_root,
                    &writer_bytes,
                    |_| {
                        temporary_ready_sender.send(()).unwrap();
                        publish_receiver.recv().unwrap();
                    },
                )
            });

            temporary_ready_receiver.recv().unwrap();
            assert_eq!(read_bytes(&snapshot, track.path()).unwrap(), None);
            assert!(!snapshot.exists());
            publish_sender.send(()).unwrap();
            assert!(writer.join().unwrap().is_ok());
        });

        assert_eq!(std::fs::read(&snapshot).unwrap(), bytes);
    }

    #[test]
    fn test_snapshot_conflicting_concurrent_publisher_returns_mismatch() {
        let track = tempfile::tempdir().unwrap();
        let snapshot = track.path().join(SNAPSHOT_FILE);
        let result = write_first_snapshot_after_temporary_write(
            &snapshot,
            track.path(),
            b"first declaration",
            |_| std::fs::write(&snapshot, b"second declaration").unwrap(),
        );

        assert!(matches!(result, Err(SnapshotPublicationError::Mismatch)));
    }

    #[test]
    fn test_concurrent_baseline_publishers_accept_the_same_declaration() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        write_declaration(track.path(), "{\"schema_version\":1,\"layers\":{\"domain\":[]}}");
        let barrier = std::sync::Barrier::new(2);

        std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                barrier.wait();
                FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
                    track.path(),
                    workspace.path(),
                    &[binding("domain", "domain")],
                )
            });
            let second = scope.spawn(|| {
                barrier.wait();
                FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
                    track.path(),
                    workspace.path(),
                    &[binding("domain", "domain")],
                )
            });
            assert!(first.join().unwrap().is_ok());
            assert!(second.join().unwrap().is_ok());
        });

        assert!(track.path().join(SNAPSHOT_FILE).is_file());
    }

    #[cfg(unix)]
    #[test]
    fn test_baseline_port_with_symlinked_track_root_returns_read_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        write_declaration(track.path(), "{\"schema_version\":1,\"layers\":{\"domain\":[]}}");
        let links = tempfile::tempdir().unwrap();
        let symlinked_track = links.path().join("track");
        std::os::unix::fs::symlink(track.path(), &symlinked_track).unwrap();

        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            &symlinked_track,
            workspace.path(),
            &[binding("domain", "domain")],
        );

        assert!(matches!(
            result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::ReadDeclaration { .. }
            ))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_baseline_port_with_symlinked_workspace_root_returns_read_error() {
        let workspace = setup_workspace();
        let track = tempfile::tempdir().unwrap();
        write_declaration(track.path(), "{\"schema_version\":1,\"layers\":{\"domain\":[]}}");
        let links = tempfile::tempdir().unwrap();
        let symlinked_workspace = links.path().join("workspace");
        std::os::unix::fs::symlink(workspace.path(), &symlinked_workspace).unwrap();

        let result = FsTdddFeatureDeclarationAdapter::new().load_for_baseline(
            track.path(),
            &symlinked_workspace,
            &[binding("domain", "domain")],
        );

        assert!(matches!(
            result,
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                TdddFeatureDeclarationReadError::ReadDeclaration { .. }
            ))
        ));
    }
}
