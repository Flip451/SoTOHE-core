//! Filesystem-backed implementation of the domain
//! [`domain::AdrFilePort`] secondary port.

use std::path::PathBuf;

use domain::{AdrFilePort, AdrFilePortError, AdrFrontMatter};

use super::parse::parse_adr_frontmatter;

/// Adapter that walks a real filesystem directory of ADR markdown files and
/// hands the usecase layer parsed [`AdrFrontMatter`] aggregates.
///
/// Construction takes the ADR directory path so production code can wire
/// `knowledge/adr` and tests can wire a temporary fixture directory without
/// depending on the real workspace layout. Both the directory walk and the
/// YAML parse are encapsulated here so the usecase layer never touches
/// `serde` types or filesystem APIs directly (CN-05).
#[derive(Debug, Clone)]
pub struct FsAdrFileAdapter {
    adr_dir: PathBuf,
}

impl FsAdrFileAdapter {
    /// Create a new adapter rooted at `adr_dir`.
    ///
    /// `adr_dir` is the directory containing ADR markdown files (typically
    /// `knowledge/adr/`). The directory's existence is not checked at
    /// construction; failures surface lazily from
    /// [`FsAdrFileAdapter::list_adr_paths`] / [`FsAdrFileAdapter::read_adr_frontmatter`].
    #[must_use]
    pub fn new(adr_dir: PathBuf) -> Self {
        Self { adr_dir }
    }
}

impl AdrFilePort for FsAdrFileAdapter {
    fn list_adr_paths(&self) -> Result<Vec<PathBuf>, AdrFilePortError> {
        let entries = std::fs::read_dir(&self.adr_dir)
            .map_err(|e| AdrFilePortError::ListPaths(format!("{}: {e}", self.adr_dir.display())))?;

        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| {
                AdrFilePortError::ListPaths(format!("{}: {e}", self.adr_dir.display()))
            })?;
            let file_type = entry.file_type().map_err(|e| {
                AdrFilePortError::ListPaths(format!("{}: {e}", self.adr_dir.display()))
            })?;
            if !file_type.is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("md")) {
                paths.push(path);
            }
        }
        paths.sort();
        Ok(paths)
    }

    fn read_adr_frontmatter(&self, path: PathBuf) -> Result<AdrFrontMatter, AdrFilePortError> {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| AdrFilePortError::ReadFile(format!("{}: {e}", path.display())))?;
        parse_adr_frontmatter(&content)
            .map_err(|e| AdrFilePortError::ReadFile(format!("{}: {e}", path.display())))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::fs;

    use domain::AdrDecisionEntry;

    use super::*;

    fn write_adr(dir: &std::path::Path, name: &str, body: &str) {
        fs::write(dir.join(name), body).unwrap();
    }

    fn make_tempdir(label: &str) -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "sotohe-fs-adr-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_list_adr_paths_returns_sorted_md_files_only() {
        let dir = make_tempdir("list-md-only");
        write_adr(&dir, "2026-04-27-0001-foo.md", "---\nadr_id: foo\n---\n");
        write_adr(&dir, "2026-04-27-0002-bar.md", "---\nadr_id: bar\n---\n");
        write_adr(&dir, "README.txt", "ignored");
        write_adr(&dir, "notes.json", "{}");

        let adapter = FsAdrFileAdapter::new(dir.clone());
        let paths = adapter.list_adr_paths().unwrap();
        let names: Vec<String> =
            paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["2026-04-27-0001-foo.md", "2026-04-27-0002-bar.md"]);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_list_adr_paths_with_missing_dir_returns_list_paths_error() {
        let dir = make_tempdir("missing");
        fs::remove_dir_all(&dir).unwrap();

        let adapter = FsAdrFileAdapter::new(dir);
        let err = adapter.list_adr_paths().unwrap_err();
        assert!(matches!(err, AdrFilePortError::ListPaths(_)));
    }

    #[test]
    fn test_list_adr_paths_with_empty_dir_returns_empty_vec() {
        let dir = make_tempdir("empty");
        let adapter = FsAdrFileAdapter::new(dir.clone());
        let paths = adapter.list_adr_paths().unwrap();
        assert!(paths.is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_read_adr_frontmatter_decodes_all_five_typestate_variants() {
        let dir = make_tempdir("five-variants");

        write_adr(
            &dir,
            "p.md",
            "---\nadr_id: p\ndecisions:\n  - id: D1\n    status: proposed\n    user_decision_ref: chat\n---\n",
        );
        write_adr(
            &dir,
            "a.md",
            "---\nadr_id: a\ndecisions:\n  - id: D1\n    status: accepted\n    user_decision_ref: chat\n---\n",
        );
        write_adr(
            &dir,
            "i.md",
            "---\nadr_id: i\ndecisions:\n  - id: D1\n    status: implemented\n    user_decision_ref: chat\n    implemented_in: abc1234\n---\n",
        );
        write_adr(
            &dir,
            "s.md",
            "---\nadr_id: s\ndecisions:\n  - id: D1\n    status: superseded\n    user_decision_ref: chat\n    superseded_by: 2026-05-01-other.md#D7\n---\n",
        );
        write_adr(
            &dir,
            "d.md",
            "---\nadr_id: d\ndecisions:\n  - id: D1\n    status: deprecated\n    user_decision_ref: chat\n---\n",
        );

        let adapter = FsAdrFileAdapter::new(dir.clone());

        let p = adapter.read_adr_frontmatter(dir.join("p.md")).unwrap();
        assert!(matches!(&p.decisions()[0], AdrDecisionEntry::ProposedDecision(_)));
        let a = adapter.read_adr_frontmatter(dir.join("a.md")).unwrap();
        assert!(matches!(&a.decisions()[0], AdrDecisionEntry::AcceptedDecision(_)));
        let i = adapter.read_adr_frontmatter(dir.join("i.md")).unwrap();
        assert!(matches!(&i.decisions()[0], AdrDecisionEntry::ImplementedDecision(_)));
        let s = adapter.read_adr_frontmatter(dir.join("s.md")).unwrap();
        assert!(matches!(&s.decisions()[0], AdrDecisionEntry::SupersededDecision(_)));
        let d = adapter.read_adr_frontmatter(dir.join("d.md")).unwrap();
        assert!(matches!(&d.decisions()[0], AdrDecisionEntry::DeprecatedDecision(_)));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_read_adr_frontmatter_with_missing_file_returns_read_file_error() {
        let dir = make_tempdir("missing-file");
        let adapter = FsAdrFileAdapter::new(dir.clone());
        let err = adapter.read_adr_frontmatter(dir.join("nonexistent.md")).unwrap_err();
        assert!(matches!(err, AdrFilePortError::ReadFile(_)));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_read_adr_frontmatter_with_no_frontmatter_returns_read_file_error() {
        let dir = make_tempdir("no-fm");
        write_adr(&dir, "bare.md", "# No front-matter here\n");
        let adapter = FsAdrFileAdapter::new(dir.clone());
        let err = adapter.read_adr_frontmatter(dir.join("bare.md")).unwrap_err();
        assert!(matches!(err, AdrFilePortError::ReadFile(_)));
        fs::remove_dir_all(dir).unwrap();
    }
}
