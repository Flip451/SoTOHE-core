//! Codec for `knowledge/external/guides.json` — deserializes into domain `GuideEntry`.

use std::path::Path;

use domain::skill_compliance::GuideEntry;

/// Serde representation of a single guide entry in `guides.json`.
#[derive(Debug, serde::Deserialize)]
struct GuideRaw {
    id: String,
    #[serde(default)]
    trigger_keywords: Vec<String>,
    #[serde(default)]
    summary: Vec<String>,
    #[serde(default)]
    project_usage: Vec<String>,
    #[serde(default)]
    cache_path: String,
}

/// Serde representation of the top-level `guides.json`.
#[derive(Debug, serde::Deserialize)]
struct GuidesRegistry {
    #[serde(default)]
    guides: Vec<GuideRaw>,
}

/// Loads guide entries from a `guides.json` file path.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
pub fn load_guides(path: &Path) -> Result<Vec<GuideEntry>, GuidesCodecError> {
    let content = std::fs::read_to_string(path).map_err(GuidesCodecError::Io)?;
    let registry: GuidesRegistry =
        serde_json::from_str(&content).map_err(GuidesCodecError::Json)?;
    Ok(registry.guides.into_iter().map(|raw| raw.into()).collect())
}

impl From<GuideRaw> for GuideEntry {
    fn from(raw: GuideRaw) -> Self {
        Self {
            id: raw.id,
            trigger_keywords: raw.trigger_keywords,
            summary: raw.summary,
            project_usage: raw.project_usage,
            cache_path: raw.cache_path,
        }
    }
}

/// Errors from guides.json loading.
#[derive(Debug, thiserror::Error)]
pub enum GuidesCodecError {
    /// File I/O error.
    #[error("failed to read guides.json: {0}")]
    Io(#[source] std::io::Error),
    /// JSON parse error.
    #[error("failed to parse guides.json: {0}")]
    Json(#[source] serde_json::Error),
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_load_guides_from_real_file() {
        let path = Path::new("knowledge/external/guides.json");
        if !path.exists() {
            return; // Skip if not in repo root
        }
        let guides = load_guides(path).unwrap();
        assert!(!guides.is_empty());
        assert!(!guides[0].id.is_empty());
        assert!(!guides[0].trigger_keywords.is_empty());
    }

    #[test]
    fn test_load_guides_minimal_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guides.json");
        std::fs::write(
            &path,
            r#"{"version":1,"guides":[{"id":"test","trigger_keywords":["foo"]}]}"#,
        )
        .unwrap();
        let guides = load_guides(&path).unwrap();
        assert_eq!(guides.len(), 1);
        assert_eq!(guides[0].id, "test");
        assert_eq!(guides[0].trigger_keywords, vec!["foo"]);
        assert!(guides[0].summary.is_empty());
    }

    #[test]
    fn test_load_guides_empty_guides_array() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guides.json");
        std::fs::write(&path, r#"{"version":1,"guides":[]}"#).unwrap();
        let guides = load_guides(&path).unwrap();
        assert!(guides.is_empty());
    }

    #[test]
    fn test_load_guides_missing_file_returns_error() {
        let result = load_guides(Path::new("/nonexistent/guides.json"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GuidesCodecError::Io(_)));
    }
}
