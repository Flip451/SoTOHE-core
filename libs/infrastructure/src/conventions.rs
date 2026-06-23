//! Convention document management — Rust port of `scripts/convention_docs.py`.
//!
//! Provides add / update-index / verify-index operations for
//! `knowledge/conventions/`.

use std::fmt;
use std::path::Path;

use domain::verify::{VerifyFinding, VerifyOutcome};
use regex::Regex;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const INDEX_START: &str = "<!-- convention-docs:start -->";
const INDEX_END: &str = "<!-- convention-docs:end -->";

/// File ordering for convention docs index rendering.
///
/// Stems not listed here receive order 100 and sort alphabetically after
/// the explicitly ordered entries.
static FILE_ORDER: &[(&str, u32)] = &[
    ("architecture", 10),
    ("domain-model", 20),
    ("data-model", 30),
    ("api-design", 40),
    ("error-handling", 50),
    ("instrumentation", 60),
    ("testing", 70),
    ("naming", 80),
    ("generated-code", 90),
    ("security", 100),
];

/// Acronyms/abbreviations that should be uppercased in generated titles.
static UPPERCASE_WORDS: &[&str] = &[
    "api", "cli", "cpu", "css", "db", "gpu", "grpc", "html", "http", "https", "id", "io", "json",
    "jwt", "oauth", "sdk", "sql", "ui", "uri", "url", "ux",
];

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from `sotp conventions add` and `update-index` operations.
#[derive(Debug)]
pub enum ConventionDocsError {
    /// An I/O error occurred while reading or writing a file.
    Io { path: String, source: std::io::Error },
    /// The conventions README.md file does not exist.
    MissingReadme(String),
    /// The README.md file is missing the index marker comments.
    MissingMarkers(String),
    /// A convention document with the derived slug already exists.
    AlreadyExists(String),
    /// The provided slug is not valid kebab-case ASCII.
    InvalidSlug(String),
}

impl fmt::Display for ConventionDocsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "I/O error on {path}: {source}"),
            Self::MissingReadme(msg) => write!(f, "README index target is missing: {msg}"),
            Self::MissingMarkers(msg) => {
                write!(f, "README index markers not found in {msg}")
            }
            Self::AlreadyExists(msg) => write!(f, "Convention document already exists: {msg}"),
            Self::InvalidSlug(msg) => write!(f, "Invalid slug: {msg}"),
        }
    }
}

impl std::error::Error for ConventionDocsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Create a new convention document from template and update the README index.
///
/// Implements `sotp conventions add`.
///
/// # Errors
///
/// Returns `ConventionDocsError` when:
/// - `slug` is provided but is not valid kebab-case ASCII
/// - `name` cannot be slugified to a non-empty ASCII string
/// - `README.md` is missing or lacks index markers
/// - A document with the resolved slug already exists
/// - Any I/O operation fails
pub fn add_convention_doc(
    root: &Path,
    name: &str,
    slug: Option<&str>,
    title: Option<&str>,
    summary: Option<&str>,
) -> Result<(), ConventionDocsError> {
    guard_convention_root(root)?;
    let conventions_dir = root.join("knowledge").join("conventions");
    let readme_path = conventions_dir.join("README.md");

    let resolved_slug = resolve_slug(name, slug)?;
    let resolved_title = resolve_title(name, &resolved_slug, title);

    // Verify README and markers before touching the filesystem.
    let content = read_readme(root, &readme_path)?;
    ensure_readme_markers(&content, &readme_path)?;

    let target = conventions_dir.join(format!("{resolved_slug}.md"));
    guard_convention_path(root, &target, format!("knowledge/conventions/{resolved_slug}.md"))?;
    if target.is_file() {
        return Err(ConventionDocsError::AlreadyExists(format!(
            "knowledge/conventions/{resolved_slug}.md"
        )));
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ConventionDocsError::Io {
            path: "knowledge/conventions".to_owned(),
            source: e,
        })?;
    }

    let template = build_template(&resolved_title, summary);
    guard_convention_path(root, &target, format!("knowledge/conventions/{resolved_slug}.md"))?;
    std::fs::write(&target, template).map_err(|e| ConventionDocsError::Io {
        path: format!("knowledge/conventions/{resolved_slug}.md"),
        source: e,
    })?;

    update_convention_index(root)
}

/// Regenerate the README.md index from current convention documents.
///
/// Implements `sotp conventions update-index`.
///
/// # Errors
///
/// Returns `ConventionDocsError` when:
/// - `README.md` is missing or cannot be read
/// - Index markers are absent from `README.md`
/// - Any I/O operation fails while scanning or writing
pub fn update_convention_index(root: &Path) -> Result<(), ConventionDocsError> {
    guard_convention_root(root)?;
    let conventions_dir = root.join("knowledge").join("conventions");
    let readme_path = conventions_dir.join("README.md");

    let content = read_readme(root, &readme_path)?;
    ensure_readme_markers(&content, &readme_path)?;

    let new_block =
        render_index_block(root, &conventions_dir).map_err(|e| ConventionDocsError::Io {
            path: "knowledge/conventions".to_owned(),
            source: std::io::Error::other(e),
        })?;

    let updated = replace_marker_block(&content, &new_block);
    guard_convention_path(root, &readme_path, "knowledge/conventions/README.md")?;
    std::fs::write(&readme_path, updated).map_err(|e| ConventionDocsError::Io {
        path: "knowledge/conventions/README.md".to_owned(),
        source: e,
    })?;

    Ok(())
}

/// Verify the convention README index is in sync.
///
/// Logic relocated from `verify::convention_docs::verify` (D3/CN-06).
///
/// Returns a passing `VerifyOutcome` when no convention documents exist yet
/// (bootstrapping) or when the index is in sync with the actual files.
pub fn verify_convention_index(root: &Path) -> VerifyOutcome {
    if let Err(e) = guard_convention_root(root) {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(e.to_string())]);
    }

    let conventions_dir = root.join("knowledge").join("conventions");
    let readme_path = conventions_dir.join("README.md");

    if let Err(e) = guard_convention_path(root, &conventions_dir, "knowledge/conventions") {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(e.to_string())]);
    }

    // If the conventions directory does not exist at all, there is nothing to
    // verify — return pass (bootstrapping case).
    if !conventions_dir.is_dir() {
        return VerifyOutcome::pass();
    }

    // README.md is only required once convention documents exist.
    // An empty conventions directory is the bootstrapping case — pass.
    if let Err(e) = guard_convention_path(root, &readme_path, "knowledge/conventions/README.md") {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(e.to_string())]);
    }
    if !readme_path.is_file() {
        let has_convention_docs = std::fs::read_dir(&conventions_dir).is_ok_and(|entries| {
            entries.flatten().any(|e| {
                let name = e.file_name();
                let name_str = name.to_string_lossy();
                name_str.ends_with(".md") && name_str != "README.md"
            })
        });
        if has_convention_docs {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(
                "knowledge/conventions contains convention documents but is missing README.md"
                    .to_owned(),
            )]);
        }
        // No conventions bootstrapped yet — skip.
        return VerifyOutcome::pass();
    }

    let content = match std::fs::read_to_string(&readme_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot read knowledge/conventions/README.md: {e}"
            ))]);
        }
    };

    // Check markers exist.
    let marker_re = match Regex::new(&format!(
        "(?s){}.*?{}",
        regex::escape(INDEX_START),
        regex::escape(INDEX_END)
    )) {
        Ok(re) => re,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Internal regex error: {e}"
            ))]);
        }
    };

    let actual_block = match marker_re.find(&content) {
        Some(m) => m.as_str().to_owned(),
        None => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(
                "README index markers not found in knowledge/conventions/README.md".to_owned(),
            )]);
        }
    };

    let expected = match render_index_block(root, &conventions_dir) {
        Ok(block) => block,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(e)]);
        }
    };

    if actual_block != expected {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(
            "Convention README index is out of sync. To fix: run `cargo make conventions-update-index`."
                .to_owned(),
        )]);
    }

    VerifyOutcome::pass()
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn guard_convention_root(root: &Path) -> Result<(), ConventionDocsError> {
    match root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Err(ConventionDocsError::Io {
            path: root.display().to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("refusing to use symlinked root: {}", root.display()),
            ),
        }),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ConventionDocsError::Io { path: root.display().to_string(), source }),
    }
}

fn guard_convention_path(
    root: &Path,
    path: &Path,
    display_path: impl Into<String>,
) -> Result<(), ConventionDocsError> {
    crate::track::symlink_guard::reject_symlinks_below(path, root)
        .map(|_| ())
        .map_err(|source| ConventionDocsError::Io { path: display_path.into(), source })
}

fn read_readme(root: &Path, readme_path: &Path) -> Result<String, ConventionDocsError> {
    guard_convention_path(root, readme_path, "knowledge/conventions/README.md")?;
    if !readme_path.is_file() {
        return Err(ConventionDocsError::MissingReadme(
            "knowledge/conventions/README.md".to_owned(),
        ));
    }
    std::fs::read_to_string(readme_path).map_err(|e| ConventionDocsError::Io {
        path: "knowledge/conventions/README.md".to_owned(),
        source: e,
    })
}

fn ensure_readme_markers(content: &str, _readme_path: &Path) -> Result<(), ConventionDocsError> {
    let Ok(re) =
        Regex::new(&format!("(?s){}.*?{}", regex::escape(INDEX_START), regex::escape(INDEX_END)))
    else {
        return Err(ConventionDocsError::MissingMarkers(
            "knowledge/conventions/README.md".to_owned(),
        ));
    };
    if re.find(content).is_none() {
        return Err(ConventionDocsError::MissingMarkers(
            "knowledge/conventions/README.md".to_owned(),
        ));
    }
    Ok(())
}

fn replace_marker_block(content: &str, new_block: &str) -> String {
    // Build a regex that matches the existing marker block (DOTALL).
    let pattern = format!("(?s){}.*?{}", regex::escape(INDEX_START), regex::escape(INDEX_END));
    // Infallible: the pattern is constructed from known-valid strings.
    if let Ok(re) = Regex::new(&pattern) {
        re.replacen(content, 1, new_block).into_owned()
    } else {
        content.to_owned()
    }
}

/// Convert a free-form string to a kebab-case ASCII slug.
fn slugify(value: &str) -> String {
    // Replace any run of non-alphanumeric characters with a single "-".
    let Ok(re) = Regex::new(r"[^a-z0-9]+") else {
        return String::new();
    };
    let lowered = value.trim().to_lowercase();
    let slug = re.replace_all(&lowered, "-");
    // Strip leading/trailing "-".
    slug.trim_matches('-').to_owned()
}

/// Validate that `value` is already a valid kebab-case ASCII slug.
///
/// # Errors
///
/// Returns `ConventionDocsError::InvalidSlug` if the value is empty or is not
/// already in canonical kebab-case form.
fn validate_slug(value: &str) -> Result<String, ConventionDocsError> {
    let slug = slugify(value);
    if slug.is_empty() {
        return Err(ConventionDocsError::InvalidSlug(
            "slug must contain at least one ASCII letter or digit".to_owned(),
        ));
    }
    if slug != value {
        return Err(ConventionDocsError::InvalidSlug("slug must be kebab-case ASCII".to_owned()));
    }
    Ok(slug)
}

/// Derive the slug from `name` or validate the explicitly provided one.
fn resolve_slug(name: &str, provided: Option<&str>) -> Result<String, ConventionDocsError> {
    if let Some(s) = provided {
        return validate_slug(s);
    }
    let derived = slugify(name);
    if derived.is_empty() {
        return Err(ConventionDocsError::InvalidSlug(
            "non-ASCII or free-form names require --slug with a kebab-case ASCII file name"
                .to_owned(),
        ));
    }
    Ok(derived)
}

/// Derive the document title from `name`, `slug`, and an optional explicit title.
fn resolve_title(name: &str, slug: &str, provided: Option<&str>) -> String {
    if let Some(t) = provided {
        return t.to_owned();
    }
    if slug == name {
        return default_title(slug);
    }
    name.trim().to_owned()
}

/// Convert a kebab-case slug to a human-readable title.
///
/// Known abbreviations (listed in `UPPERCASE_WORDS`) are uppercased;
/// all other words are capitalized (first letter upper, rest lower).
fn default_title(slug: &str) -> String {
    let words: Vec<String> = slug
        .split('-')
        .map(|part| {
            if UPPERCASE_WORDS.contains(&part) {
                part.to_uppercase()
            } else {
                let mut chars = part.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
        })
        .collect();
    words.join(" ")
}

/// Build the Japanese template for a new convention document.
fn build_template(title: &str, summary: Option<&str>) -> String {
    let summary_text = summary.unwrap_or("この規約の目的と適用範囲をここに書く。");
    format!(
        "# {title}\n\n\
         ## Purpose\n\n\
         {summary_text}\n\n\
         ## Scope\n\n\
         - Applies to: `TODO:` この規約が適用されるレイヤ、機能、ファイル、状況を書く\n\
         - Does not apply to: `TODO:` 適用外や境界条件を書く\n\n\
         ## Rules\n\n\
         - `TODO:` 守るべきルールを書く\n\
         - `TODO:` 禁止事項や避ける実装を書く\n\
         - `TODO:` 境界での変換、命名、エラー処理など具体条件を書く\n\n\
         ## Examples\n\n\
         - Good: `TODO:` 推奨される実装例を書く\n\
         - Bad: `TODO:` 避けるべき実装例を書く\n\n\
         ## Exceptions\n\n\
         - `TODO:` 例外を認める条件、承認方法、記録方法を書く\n\n\
         ## Review Checklist\n\n\
         - `TODO:` レビュー時に確認する観点を書く\n\n\
         ## Related Documents\n\n\
         - `TODO:` 関連する spec / plan / rule を書く\n"
    )
}

/// Render the full marker block (including `INDEX_START` / `INDEX_END`) from
/// the current files in `conventions_dir`.
///
/// # Errors
///
/// Returns an error string when `conventions_dir` cannot be read or a
/// convention document file cannot be read.
fn render_index_block(root: &Path, conventions_dir: &Path) -> Result<String, String> {
    let mut entries: Vec<(String, String)> = Vec::new();

    crate::track::symlink_guard::reject_symlinks_below(conventions_dir, root)
        .map_err(|e| format!("Cannot validate directory knowledge/conventions: {e}"))?;
    if conventions_dir.is_dir() {
        let read_dir = std::fs::read_dir(conventions_dir)
            .map_err(|e| format!("Cannot read directory knowledge/conventions: {e}"))?;

        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        for entry_result in read_dir {
            let entry = entry_result.map_err(|e| {
                format!("Cannot read directory entry in knowledge/conventions: {e}")
            })?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".md") && name_str != "README.md" {
                paths.push(entry.path());
            }
        }

        paths.sort_by_key(|p| sort_key(p.as_path()));

        for path in &paths {
            let file_name =
                path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            crate::track::symlink_guard::reject_symlinks_below(path, root).map_err(|e| {
                format!("Cannot validate convention doc knowledge/conventions/{file_name}: {e}")
            })?;
            let heading = extract_heading(path).map_err(|e| {
                format!("Cannot read convention doc knowledge/conventions/{file_name}: {e}")
            })?;
            entries.push((file_name, heading));
        }
    }

    let body = if entries.is_empty() {
        "- No convention documents yet. Add one with `/conventions:add <name>`.".to_owned()
    } else {
        entries
            .iter()
            .map(|(name, heading)| format!("- `{name}`: {heading}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(format!("{INDEX_START}\n{body}\n{INDEX_END}"))
}

/// Extract the first `# Heading` line from a markdown file.
fn extract_heading(path: &Path) -> Result<String, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("# ") {
            return Ok(heading.trim().to_owned());
        }
    }
    Ok(path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default())
}

/// Determine the sort order for a convention document path.
fn sort_key(path: &Path) -> (u32, String) {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let order =
        FILE_ORDER.iter().find(|(name, _)| *name == stem).map(|(_, ord)| *ord).unwrap_or(100);
    (order, stem)
}

// ---------------------------------------------------------------------------
// Port adapter (T023)
// ---------------------------------------------------------------------------

/// Filesystem adapter that implements [`usecase::conventions::ConventionsPort`].
///
/// Delegates to the module-level `add_convention_doc`, `update_convention_index`,
/// and `verify_convention_index` free functions.
pub struct FsConventionsAdapter;

impl FsConventionsAdapter {
    /// Create a new `FsConventionsAdapter`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsConventionsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl usecase::conventions::ConventionsPort for FsConventionsAdapter {
    fn add_convention(
        &self,
        root: &std::path::Path,
        name: &str,
        slug: Option<&str>,
        title: Option<&str>,
        summary: Option<&str>,
    ) -> Result<String, usecase::conventions::ConventionsPortError> {
        add_convention_doc(root, name, slug, title, summary)
            .map(|()| "[OK] Convention document added.".to_owned())
            .map_err(|e| usecase::conventions::ConventionsPortError::Unavailable(e.to_string()))
    }

    fn update_index(
        &self,
        root: &std::path::Path,
    ) -> Result<String, usecase::conventions::ConventionsPortError> {
        update_convention_index(root)
            .map(|()| "[OK] Convention README index updated.".to_owned())
            .map_err(|e| usecase::conventions::ConventionsPortError::Unavailable(e.to_string()))
    }

    fn verify_index(
        &self,
        root: &std::path::Path,
    ) -> Result<usecase::conventions::VerifyIndexResult, usecase::conventions::ConventionsPortError>
    {
        let outcome = verify_convention_index(root);
        Ok(usecase::conventions::VerifyIndexResult {
            ok: outcome.is_ok(),
            findings: outcome.findings().iter().map(|f| f.to_string()).collect(),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn setup_conventions(root: &Path, files: &[(&str, &str)], readme_content: &str) {
        let dir = root.join("knowledge").join("conventions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("README.md"), readme_content).unwrap();
        for (name, content) in files {
            std::fs::write(dir.join(name), content).unwrap();
        }
    }

    fn make_readme_with_block(root: &Path, dir: &Path) -> String {
        let block = render_index_block(root, dir).unwrap();
        format!("# Conventions\n\n{block}\n")
    }

    // -----------------------------------------------------------------------
    // verify_convention_index (relocated from verify::convention_docs)
    // -----------------------------------------------------------------------

    #[test]
    fn test_no_conventions_dir_passes() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify_convention_index(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_synced_index_passes() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("knowledge").join("conventions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("security.md"), "# Security\nRules here.\n").unwrap();

        let readme = make_readme_with_block(tmp.path(), &dir);
        std::fs::write(dir.join("README.md"), &readme).unwrap();

        let outcome = verify_convention_index(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_out_of_sync_index_fails() {
        let tmp = TempDir::new().unwrap();
        setup_conventions(
            tmp.path(),
            &[("security.md", "# Security\n")],
            &format!("# Conventions\n\n{INDEX_START}\n- stale entry\n{INDEX_END}\n"),
        );
        let outcome = verify_convention_index(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_missing_markers_fails() {
        let tmp = TempDir::new().unwrap();
        setup_conventions(
            tmp.path(),
            &[("security.md", "# Security\n")],
            "# Conventions\nNo markers here.\n",
        );
        let outcome = verify_convention_index(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_convention_docs_without_readme_fails() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("knowledge").join("conventions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("security.md"), "# Security\n").unwrap();
        let outcome = verify_convention_index(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_empty_conventions_dir_without_readme_passes() {
        // An empty conventions directory (no docs, no README) is the bootstrapping
        // case — matches Python verify_index behaviour which returns pass.
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("knowledge").join("conventions");
        std::fs::create_dir_all(&dir).unwrap();
        // No README.md, no convention docs — dir exists but is empty.
        let outcome = verify_convention_index(tmp.path());
        assert!(outcome.is_ok());
    }

    // -----------------------------------------------------------------------
    // slugify / validate_slug / default_title
    // -----------------------------------------------------------------------

    #[test]
    fn test_slugify_simple_name() {
        assert_eq!(slugify("hello world"), "hello-world");
    }

    #[test]
    fn test_slugify_strips_non_ascii() {
        assert_eq!(slugify("Foo Bar!"), "foo-bar");
    }

    #[test]
    fn test_slugify_collapses_multiple_separators() {
        assert_eq!(slugify("foo---bar"), "foo-bar");
    }

    #[test]
    fn test_validate_slug_valid_kebab() {
        assert_eq!(validate_slug("error-handling").unwrap(), "error-handling");
    }

    #[test]
    fn test_validate_slug_uppercase_rejected() {
        assert!(matches!(validate_slug("ErrorHandling"), Err(ConventionDocsError::InvalidSlug(_))));
    }

    #[test]
    fn test_validate_slug_empty_rejected() {
        assert!(matches!(validate_slug(""), Err(ConventionDocsError::InvalidSlug(_))));
    }

    #[test]
    fn test_default_title_known_acronym() {
        assert_eq!(default_title("api-design"), "API Design");
    }

    #[test]
    fn test_default_title_plain_words() {
        assert_eq!(default_title("error-handling"), "Error Handling");
    }

    #[test]
    fn test_default_title_mixed() {
        assert_eq!(default_title("grpc-api"), "GRPC API");
    }

    // -----------------------------------------------------------------------
    // build_template
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_template_starts_with_title() {
        let t = build_template("My Title", None);
        assert!(t.starts_with("# My Title\n"));
    }

    #[test]
    fn test_build_template_with_summary() {
        let t = build_template("Title", Some("Custom summary."));
        assert!(t.contains("Custom summary."));
    }

    #[test]
    fn test_build_template_default_summary_japanese() {
        let t = build_template("Title", None);
        assert!(t.contains("この規約の目的と適用範囲をここに書く。"));
    }

    // -----------------------------------------------------------------------
    // update_convention_index
    // -----------------------------------------------------------------------

    #[test]
    fn test_update_convention_index_writes_correct_entry() {
        let tmp = TempDir::new().unwrap();
        setup_conventions(
            tmp.path(),
            &[("security.md", "# Security Guide\n")],
            &format!("# Conventions\n\n{INDEX_START}\n- old\n{INDEX_END}\n"),
        );
        update_convention_index(tmp.path()).unwrap();

        let readme = std::fs::read_to_string(
            tmp.path().join("knowledge").join("conventions").join("README.md"),
        )
        .unwrap();
        assert!(readme.contains("- `security.md`: Security Guide"));
    }

    #[test]
    fn test_update_convention_index_idempotent() {
        let tmp = TempDir::new().unwrap();
        setup_conventions(
            tmp.path(),
            &[("security.md", "# Security\n")],
            &format!("# Conventions\n\n{INDEX_START}\n- old\n{INDEX_END}\n"),
        );

        update_convention_index(tmp.path()).unwrap();
        let first = std::fs::read_to_string(
            tmp.path().join("knowledge").join("conventions").join("README.md"),
        )
        .unwrap();

        update_convention_index(tmp.path()).unwrap();
        let second = std::fs::read_to_string(
            tmp.path().join("knowledge").join("conventions").join("README.md"),
        )
        .unwrap();

        assert_eq!(first, second, "update_convention_index must be idempotent");
    }

    #[test]
    fn test_update_convention_index_missing_readme_errors() {
        let tmp = TempDir::new().unwrap();
        // No conventions dir at all.
        let result = update_convention_index(tmp.path());
        assert!(
            matches!(result, Err(ConventionDocsError::MissingReadme(_))),
            "expected MissingReadme, got: {result:?}"
        );
    }

    #[test]
    fn test_update_convention_index_missing_markers_errors() {
        let tmp = TempDir::new().unwrap();
        setup_conventions(tmp.path(), &[], "# Conventions\nNo markers here.\n");
        let result = update_convention_index(tmp.path());
        assert!(
            matches!(result, Err(ConventionDocsError::MissingMarkers(_))),
            "expected MissingMarkers, got: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_update_convention_index_symlinked_conventions_dir_errors() {
        let tmp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let knowledge_dir = tmp.path().join("knowledge");
        std::fs::create_dir_all(&knowledge_dir).unwrap();
        std::fs::create_dir_all(outside.path()).unwrap();
        std::os::unix::fs::symlink(outside.path(), knowledge_dir.join("conventions")).unwrap();

        let result = update_convention_index(tmp.path());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConventionDocsError::Io { .. }), "{err:?}");
        assert!(err.to_string().contains("refusing to follow symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn test_update_convention_index_symlinked_root_errors() {
        let real_root = TempDir::new().unwrap();
        make_synced_conventions_dir(&real_root, &[]);
        let link_parent = TempDir::new().unwrap();
        let root_link = link_parent.path().join("workspace-link");
        std::os::unix::fs::symlink(real_root.path(), &root_link).unwrap();

        let result = update_convention_index(&root_link);
        let err = result.unwrap_err();
        assert!(matches!(&err, ConventionDocsError::Io { .. }), "{err:?}");
        assert!(err.to_string().contains("refusing to use symlinked root"));
    }

    // -----------------------------------------------------------------------
    // add_convention_doc
    // -----------------------------------------------------------------------

    fn make_synced_conventions_dir(tmp: &TempDir, docs: &[(&str, &str)]) {
        let dir = tmp.path().join("knowledge").join("conventions");
        std::fs::create_dir_all(&dir).unwrap();
        for (name, content) in docs {
            std::fs::write(dir.join(name), content).unwrap();
        }
        let readme = make_readme_with_block(tmp.path(), &dir);
        std::fs::write(dir.join("README.md"), readme).unwrap();
    }

    #[test]
    fn test_add_convention_doc_happy_path_creates_file_and_updates_index() {
        let tmp = TempDir::new().unwrap();
        make_synced_conventions_dir(&tmp, &[]);
        add_convention_doc(tmp.path(), "testing", None, None, None).unwrap();

        let dir = tmp.path().join("knowledge").join("conventions");
        assert!(dir.join("testing.md").is_file(), "convention file should exist");

        let readme = std::fs::read_to_string(dir.join("README.md")).unwrap();
        assert!(readme.contains("testing.md"), "README index should reference new file");
    }

    #[test]
    fn test_add_convention_doc_with_explicit_slug() {
        let tmp = TempDir::new().unwrap();
        make_synced_conventions_dir(&tmp, &[]);
        add_convention_doc(tmp.path(), "Error Handling", Some("error-handling"), None, None)
            .unwrap();

        let dir = tmp.path().join("knowledge").join("conventions");
        assert!(dir.join("error-handling.md").is_file());
    }

    #[test]
    fn test_add_convention_doc_already_exists_error() {
        let tmp = TempDir::new().unwrap();
        make_synced_conventions_dir(&tmp, &[("testing.md", "# Testing\n")]);
        // Re-sync README after adding the existing file.
        update_convention_index(tmp.path()).unwrap();

        let result = add_convention_doc(tmp.path(), "testing", None, None, None);
        assert!(
            matches!(result, Err(ConventionDocsError::AlreadyExists(_))),
            "expected AlreadyExists, got: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_add_convention_doc_symlinked_target_errors_before_write() {
        let tmp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        make_synced_conventions_dir(&tmp, &[]);
        let target = tmp.path().join("knowledge").join("conventions").join("testing.md");
        let outside_target = outside.path().join("testing.md");
        std::fs::write(&outside_target, "# Outside\n").unwrap();
        std::os::unix::fs::symlink(&outside_target, &target).unwrap();

        let result = add_convention_doc(tmp.path(), "testing", None, None, None);
        let err = result.unwrap_err();
        assert!(matches!(&err, ConventionDocsError::Io { .. }), "{err:?}");
        assert!(err.to_string().contains("refusing to follow symlink"));
    }

    #[test]
    fn test_add_convention_doc_invalid_slug_error() {
        let tmp = TempDir::new().unwrap();
        make_synced_conventions_dir(&tmp, &[]);
        // Slug with uppercase should fail.
        let result = add_convention_doc(tmp.path(), "foo", Some("BadSlug"), None, None);
        assert!(
            matches!(result, Err(ConventionDocsError::InvalidSlug(_))),
            "expected InvalidSlug, got: {result:?}"
        );
    }

    #[test]
    fn test_add_convention_doc_verify_index_passes_after_add() {
        let tmp = TempDir::new().unwrap();
        make_synced_conventions_dir(&tmp, &[]);
        add_convention_doc(tmp.path(), "security", None, None, None).unwrap();

        let outcome = verify_convention_index(tmp.path());
        assert!(outcome.is_ok(), "index should be in sync after add");
    }

    #[test]
    fn test_add_convention_doc_with_summary() {
        let tmp = TempDir::new().unwrap();
        make_synced_conventions_dir(&tmp, &[]);
        add_convention_doc(
            tmp.path(),
            "naming",
            None,
            None,
            Some("Naming rules for this project."),
        )
        .unwrap();

        let dir = tmp.path().join("knowledge").join("conventions");
        let content = std::fs::read_to_string(dir.join("naming.md")).unwrap();
        assert!(content.contains("Naming rules for this project."));
    }
}
