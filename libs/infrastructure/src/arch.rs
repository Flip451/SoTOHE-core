//! Architecture rules rendering — Rust port of `scripts/architecture_rules.py`.
//!
//! Reads `architecture-rules.json` from the workspace root and renders
//! workspace tree / members / direct-check matrices as strings.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

pub(crate) const ARCH_RULES_FILE: &str = "architecture-rules.json";

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when loading or rendering architecture rules.
#[derive(Debug, thiserror::Error)]
pub enum ArchRulesError {
    /// The rules file could not be read.
    #[error("failed to read {path}: {source}")]
    Io { path: String, source: std::io::Error },

    /// The rules file contains invalid JSON.
    #[error("failed to parse architecture rules: {0}")]
    Parse(#[from] serde_json::Error),

    /// The rules file is structurally invalid.
    #[error("invalid architecture rules: {0}")]
    InvalidRules(String),
}

// ---------------------------------------------------------------------------
// JSON schema types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(crate) struct ArchRules {
    layers: Vec<LayerEntry>,
    extra_dirs: Option<Value>,
}

impl ArchRules {
    pub(crate) fn layers(&self) -> &[LayerEntry] {
        &self.layers
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayerEntry {
    pub(crate) crate_name: String,
    pub(crate) path: String,
    pub(crate) may_depend_on: Vec<String>,
    pub(crate) deny_reason: String,
}

#[derive(Debug)]
struct ExtraDirEntry {
    path: String,
    label: String,
}

// ---------------------------------------------------------------------------
// Internal loading + validation
// ---------------------------------------------------------------------------

pub(crate) fn load_rules(root: &Path) -> Result<ArchRules, ArchRulesError> {
    ensure_trusted_root(root)?;
    let rules_path = root.join(ARCH_RULES_FILE);
    crate::track::symlink_guard::reject_symlinks_below(&rules_path, root)
        .map_err(|source| ArchRulesError::Io { path: ARCH_RULES_FILE.to_owned(), source })?;
    let content = std::fs::read_to_string(&rules_path)
        .map_err(|e| ArchRulesError::Io { path: ARCH_RULES_FILE.to_owned(), source: e })?;
    let value: Value = serde_json::from_str(&content)?;
    parse_rules(&value)
}

fn ensure_trusted_root(root: &Path) -> Result<(), ArchRulesError> {
    match root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Err(ArchRulesError::Io {
            path: root.display().to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("refusing to use symlinked root: {}", root.display()),
            ),
        }),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ArchRulesError::Io { path: root.display().to_string(), source }),
    }
}

fn invalid_rules(message: impl Into<String>) -> ArchRulesError {
    ArchRulesError::InvalidRules(message.into())
}

fn required_non_empty_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
    message: impl Into<String>,
) -> Result<String, ArchRulesError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid_rules(message))
}

pub(crate) fn parse_rules(value: &Value) -> Result<ArchRules, ArchRulesError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_rules("architecture rules must be a JSON object"))?;
    let layers_value = object
        .get("layers")
        .ok_or_else(|| invalid_rules("architecture rules must define a non-empty 'layers' list"))?;
    let layer_values = layers_value
        .as_array()
        .filter(|layers| !layers.is_empty())
        .ok_or_else(|| invalid_rules("architecture rules must define a non-empty 'layers' list"))?;

    let mut layers = Vec::with_capacity(layer_values.len());
    let mut seen_crates: BTreeSet<String> = BTreeSet::new();
    let mut seen_paths: BTreeSet<String> = BTreeSet::new();
    for layer_value in layer_values {
        let layer_object = layer_value
            .as_object()
            .ok_or_else(|| invalid_rules("each layer entry must be an object"))?;
        let crate_name = required_non_empty_string(
            layer_object,
            "crate",
            "layer 'crate' must be a non-empty string",
        )?;
        let path = required_non_empty_string(
            layer_object,
            "path",
            format!("layer '{crate_name}' must define a non-empty 'path'"),
        )?;
        let may_depend_on = match layer_object.get("may_depend_on") {
            Some(value) => {
                let values = value.as_array().ok_or_else(|| {
                    invalid_rules(format!(
                        "layer '{crate_name}' has invalid 'may_depend_on' entries"
                    ))
                })?;
                let mut deps = Vec::with_capacity(values.len());
                for item in values {
                    let dep = item.as_str().filter(|value| !value.is_empty()).ok_or_else(|| {
                        invalid_rules(format!(
                            "layer '{crate_name}' has invalid 'may_depend_on' entries"
                        ))
                    })?;
                    deps.push(dep.to_owned());
                }
                deps
            }
            None => Vec::new(),
        };
        let deny_reason = match layer_object.get("deny_reason") {
            Some(value) => value
                .as_str()
                .ok_or_else(|| {
                    invalid_rules(format!("layer '{crate_name}' has invalid 'deny_reason'"))
                })?
                .to_owned(),
            None => String::new(),
        };

        if !seen_crates.insert(crate_name.clone()) {
            return Err(invalid_rules(format!(
                "duplicate crate in architecture rules: {crate_name}"
            )));
        }
        if !seen_paths.insert(path.clone()) {
            return Err(invalid_rules(format!("duplicate path in architecture rules: {path}")));
        }

        layers.push(LayerEntry { crate_name, path, may_depend_on, deny_reason });
    }

    let known_crates: BTreeSet<&str> = layers.iter().map(|l| l.crate_name.as_str()).collect();
    for layer in &layers {
        let mut unknown: Vec<&str> = layer
            .may_depend_on
            .iter()
            .map(String::as_str)
            .filter(|dep| !known_crates.contains(dep))
            .collect();
        if !unknown.is_empty() {
            unknown.sort_unstable();
            return Err(invalid_rules(format!(
                "layer '{}' references unknown dependencies: {}",
                layer.crate_name,
                unknown.join(", ")
            )));
        }
        if layer.may_depend_on.iter().any(|dep| dep == &layer.crate_name) {
            return Err(invalid_rules(format!(
                "layer '{}' cannot depend on itself",
                layer.crate_name
            )));
        }
    }

    let extra_dirs = object.get("extra_dirs").cloned();
    Ok(ArchRules { layers, extra_dirs })
}

fn parse_extra_dirs(
    value: Option<&Value>,
    layer_paths: &BTreeSet<String>,
) -> Result<Vec<ExtraDirEntry>, ArchRulesError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let entries = value
        .as_array()
        .ok_or_else(|| invalid_rules("architecture rules 'extra_dirs' must be an array"))?;
    let mut extra_dirs = Vec::with_capacity(entries.len());
    let mut seen_paths = BTreeSet::new();
    for entry in entries {
        let entry_object = entry
            .as_object()
            .ok_or_else(|| invalid_rules("each extra_dirs entry must be an object"))?;
        let path = required_non_empty_string(
            entry_object,
            "path",
            "extra_dirs entry must define a non-empty 'path'",
        )?;
        let label = match entry_object.get("label") {
            Some(value) => value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                invalid_rules(format!("extra_dirs entry '{path}' has invalid 'label'"))
            })?,
            None => String::new(),
        };
        if layer_paths.contains(&path) {
            return Err(invalid_rules(format!("extra_dirs path duplicates layer path: {path}")));
        }
        if !seen_paths.insert(path.clone()) {
            return Err(invalid_rules(format!(
                "duplicate extra_dirs path in architecture rules: {path}"
            )));
        }
        extra_dirs.push(ExtraDirEntry { path, label });
    }
    Ok(extra_dirs)
}

// ---------------------------------------------------------------------------
// Tree rendering helpers
// ---------------------------------------------------------------------------

fn path_depth(path: &str) -> usize {
    path.split('/').count()
}

fn parent_path(path: &str) -> Option<String> {
    path.rsplit_once('/').map(|(parent, _)| parent.to_owned())
}

fn render_line(
    path: &str,
    label: &str,
    depth: usize,
    parent_has_next: &[bool],
    is_last: bool,
) -> String {
    let name = path.split('/').next_back().unwrap_or(path);
    let mut line = if depth == 0 {
        format!("{name}/")
    } else {
        let prefix: String = parent_has_next
            .iter()
            .map(|&has_next| if has_next { "│   " } else { "    " })
            .collect();
        let branch = if is_last { "└── " } else { "├── " };
        format!("{prefix}{branch}{name}/")
    };
    if !label.is_empty() {
        let line_len = line.chars().count();
        let padding = if 24 > line_len { 24 - line_len } else { 1 };
        line.push_str(&" ".repeat(padding));
        line.push_str("# ");
        line.push_str(label);
    }
    line
}

fn build_tree_lines(entries: &[(&str, &str)], // (path, label)
) -> Vec<String> {
    let labels: BTreeMap<&str, &str> = entries.iter().map(|&(p, l)| (p, l)).collect();

    // Collect all ancestor paths
    let mut all_paths: BTreeSet<String> = BTreeSet::new();
    for &(path, _) in entries {
        let parts: Vec<&str> = path.split('/').collect();
        for i in 1..=parts.len() {
            if let Some(prefix) = parts.get(..i) {
                all_paths.insert(prefix.join("/"));
            }
        }
    }

    // Sort by (depth, path) — matches Python's `sorted(all_paths, key=lambda item: (_path_depth(item), item))`
    let mut sorted_paths: Vec<String> = all_paths.into_iter().collect();
    sorted_paths.sort_by(|a, b| {
        let da = path_depth(a);
        let db = path_depth(b);
        da.cmp(&db).then_with(|| a.cmp(b))
    });

    // Build children map: parent -> sorted child list
    // Use Option<&str> as key (None = root level)
    let mut children: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
    for path in &sorted_paths {
        let parent = parent_path(path);
        children.entry(parent).or_default().push(path.clone());
        children.entry(Some(path.clone())).or_default();
    }

    let mut lines = vec!["Cargo.toml                # workspace definition".to_owned()];

    fn visit(
        path: &str,
        parent_flags: &[bool],
        children: &BTreeMap<Option<String>, Vec<String>>,
        labels: &BTreeMap<&str, &str>,
        lines: &mut Vec<String>,
    ) {
        // Determine siblings list
        let parent_key = parent_path(path);
        let empty_vec: Vec<String> = vec![];
        let siblings = children.get(&parent_key).unwrap_or(&empty_vec);
        let is_last = siblings.last().map(|s| s.as_str()) == Some(path);
        let depth = path_depth(path) - 1;
        let label = labels.get(path).copied().unwrap_or("");

        lines.push(render_line(path, label, depth, parent_flags, is_last));

        let child_paths = children.get(&Some(path.to_owned())).cloned().unwrap_or_default();
        for child in &child_paths {
            let child_parent_flags: Vec<bool> = if depth == 0 {
                vec![]
            } else {
                let mut flags = parent_flags.to_vec();
                flags.push(!is_last);
                flags
            };
            visit(child, &child_parent_flags, children, labels, lines);
        }
    }

    let top_level = children.get(&None).cloned().unwrap_or_default();
    for top in &top_level {
        visit(top, &[], &children, &labels, &mut lines);
    }

    lines
}

fn render_tree_inner(
    rules: &ArchRules,
    include_extra_dirs: bool,
) -> Result<String, ArchRulesError> {
    let mut entries: Vec<(String, String)> =
        rules.layers.iter().map(|l| (l.path.clone(), format!("{} crate", l.crate_name))).collect();
    if include_extra_dirs {
        let layer_paths: BTreeSet<String> =
            rules.layers.iter().map(|layer| layer.path.clone()).collect();
        entries.extend(
            parse_extra_dirs(rules.extra_dirs.as_ref(), &layer_paths)?
                .into_iter()
                .map(|dir| (dir.path, dir.label)),
        );
    }
    let entry_refs: Vec<(&str, &str)> =
        entries.iter().map(|(p, l)| (p.as_str(), l.as_str())).collect();
    Ok(build_tree_lines(&entry_refs).join("\n"))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Renders a workspace tree containing only crate paths (layers).
///
/// # Errors
///
/// Returns `ArchRulesError` if the rules file cannot be read, parsed, or is structurally invalid.
pub fn render_workspace_tree(root: &Path) -> Result<String, ArchRulesError> {
    let rules = load_rules(root)?;
    render_tree_inner(&rules, false)
}

/// Renders a workspace tree including extra_dirs entries.
///
/// # Errors
///
/// Returns `ArchRulesError` if the rules file cannot be read, parsed, or is structurally invalid.
pub fn render_workspace_tree_full(root: &Path) -> Result<String, ArchRulesError> {
    let rules = load_rules(root)?;
    render_tree_inner(&rules, true)
}

/// Renders workspace member paths, one per line.
///
/// # Errors
///
/// Returns `ArchRulesError` if the rules file cannot be read, parsed, or is structurally invalid.
pub fn render_workspace_members(root: &Path) -> Result<String, ArchRulesError> {
    let rules = load_rules(root)?;
    let members: Vec<String> = rules.layers.iter().map(|l| l.path.clone()).collect();
    Ok(members.join("\n"))
}

/// Renders the direct-check matrix as `{crate}\t{forbidden1|forbidden2|...}` per line.
///
/// # Errors
///
/// Returns `ArchRulesError` if the rules file cannot be read, parsed, or is structurally invalid.
pub fn render_direct_checks(root: &Path) -> Result<String, ArchRulesError> {
    let rules = load_rules(root)?;
    let crates: Vec<&str> = rules.layers.iter().map(|l| l.crate_name.as_str()).collect();

    let mut lines: Vec<String> = Vec::new();
    for layer in &rules.layers {
        let mut forbidden: Vec<&str> = crates
            .iter()
            .copied()
            .filter(|&c| c != layer.crate_name && !layer.may_depend_on.iter().any(|d| d == c))
            .collect();
        forbidden.sort_unstable();
        lines.push(format!("{}\t{}", layer.crate_name, forbidden.join("|")));
    }
    Ok(lines.join("\n"))
}

// ---------------------------------------------------------------------------
// Port adapter (T023)
// ---------------------------------------------------------------------------

/// Filesystem adapter that implements [`usecase::arch::ArchPort`].
///
/// Delegates to the module-level `render_workspace_*` / `render_direct_checks`
/// free functions and converts `ArchRulesError` to `String`.
pub struct FsArchAdapter;

impl FsArchAdapter {
    /// Create a new `FsArchAdapter`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsArchAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl usecase::arch::ArchPort for FsArchAdapter {
    fn render_tree(
        &self,
        project_root: &std::path::Path,
    ) -> Result<String, usecase::arch::ArchPortError> {
        render_workspace_tree(project_root)
            .map_err(|e| usecase::arch::ArchPortError::Unavailable(e.to_string()))
    }

    fn render_tree_full(
        &self,
        project_root: &std::path::Path,
    ) -> Result<String, usecase::arch::ArchPortError> {
        render_workspace_tree_full(project_root)
            .map_err(|e| usecase::arch::ArchPortError::Unavailable(e.to_string()))
    }

    fn render_members(
        &self,
        project_root: &std::path::Path,
    ) -> Result<String, usecase::arch::ArchPortError> {
        render_workspace_members(project_root)
            .map_err(|e| usecase::arch::ArchPortError::Unavailable(e.to_string()))
    }

    fn render_direct_checks(
        &self,
        project_root: &std::path::Path,
    ) -> Result<String, usecase::arch::ArchPortError> {
        render_direct_checks(project_root)
            .map_err(|e| usecase::arch::ArchPortError::Unavailable(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    const MINIMAL_RULES: &str = r#"{
  "layers": [
    { "crate": "domain",  "path": "libs/domain",  "may_depend_on": [] },
    { "crate": "usecase", "path": "libs/usecase", "may_depend_on": ["domain"] },
    { "crate": "infra",   "path": "libs/infra",   "may_depend_on": ["domain", "usecase"] }
  ],
  "extra_dirs": [
    { "path": "docs", "label": "documentation" }
  ]
}"#;

    fn setup_dir(rules_json: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();
        dir
    }

    // -----------------------------------------------------------------------
    // workspace_members
    // -----------------------------------------------------------------------

    #[test]
    fn render_workspace_members_returns_one_path_per_line() {
        let dir = setup_dir(MINIMAL_RULES);
        let out = render_workspace_members(dir.path()).unwrap();
        assert_eq!(out, "libs/domain\nlibs/usecase\nlibs/infra");
    }

    // -----------------------------------------------------------------------
    // direct_checks
    // -----------------------------------------------------------------------

    #[test]
    fn render_direct_checks_format_matches_python_reference() {
        let dir = setup_dir(MINIMAL_RULES);
        let out = render_direct_checks(dir.path()).unwrap();
        // domain may NOT depend on usecase or infra
        // usecase may NOT depend on infra
        // infra may depend on domain + usecase, so only forbidden = nothing else
        let mut lines = out.lines();
        assert_eq!(lines.next().unwrap(), "domain\tinfra|usecase");
        assert_eq!(lines.next().unwrap(), "usecase\tinfra");
        assert_eq!(lines.next().unwrap(), "infra\t");
        assert!(lines.next().is_none());
    }

    #[test]
    fn render_direct_checks_uses_tab_separator() {
        let dir = setup_dir(MINIMAL_RULES);
        let out = render_direct_checks(dir.path()).unwrap();
        for line in out.lines() {
            assert!(line.contains('\t'), "line should contain TAB: {line:?}");
        }
    }

    #[test]
    fn render_direct_checks_forbidden_sorted_alphabetically() {
        let dir = setup_dir(MINIMAL_RULES);
        let out = render_direct_checks(dir.path()).unwrap();
        let first_line = out.lines().next().unwrap();
        let forbidden_part = first_line.split('\t').nth(1).unwrap();
        // "infra|usecase" — alphabetical
        assert_eq!(forbidden_part, "infra|usecase");
    }

    // -----------------------------------------------------------------------
    // workspace_tree
    // -----------------------------------------------------------------------

    #[test]
    fn render_workspace_tree_starts_with_cargo_toml_line() {
        let dir = setup_dir(MINIMAL_RULES);
        let out = render_workspace_tree(dir.path()).unwrap();
        assert!(
            out.starts_with("Cargo.toml                # workspace definition"),
            "unexpected first line: {out:?}"
        );
    }

    #[test]
    fn render_workspace_tree_contains_crate_labels() {
        let dir = setup_dir(MINIMAL_RULES);
        let out = render_workspace_tree(dir.path()).unwrap();
        assert!(out.contains("# domain crate"), "missing domain crate label");
        assert!(out.contains("# usecase crate"), "missing usecase crate label");
        assert!(out.contains("# infra crate"), "missing infra crate label");
    }

    #[test]
    fn render_workspace_tree_excludes_extra_dirs() {
        let dir = setup_dir(MINIMAL_RULES);
        let out = render_workspace_tree(dir.path()).unwrap();
        assert!(!out.contains("# documentation"), "tree-only should not include extra_dirs");
    }

    #[test]
    fn render_workspace_tree_full_includes_extra_dirs() {
        let dir = setup_dir(MINIMAL_RULES);
        let out = render_workspace_tree_full(dir.path()).unwrap();
        assert!(out.contains("# documentation"), "full tree should include extra_dirs label");
    }

    #[test]
    fn render_workspace_tree_full_accepts_extra_dir_without_label() {
        let rules = r#"{
  "layers": [
    { "crate": "domain", "path": "libs/domain", "may_depend_on": [] }
  ],
  "extra_dirs": [
    { "path": "track/items/<id>" }
  ]
}"#;
        let dir = setup_dir(rules);
        let out = render_workspace_tree_full(dir.path()).unwrap();
        assert!(out.contains("track/"), "missing track dir in:\n{out}");
        assert!(out.contains("<id>/"), "missing unlabeled extra dir in:\n{out}");
    }

    #[test]
    fn malformed_extra_dirs_only_break_tree_full() {
        let rules = r#"{
  "layers": [
    { "crate": "domain", "path": "libs/domain", "may_depend_on": [] },
    { "crate": "usecase", "path": "libs/usecase", "may_depend_on": ["domain"] }
  ],
  "extra_dirs": "not an array"
}"#;
        let dir = setup_dir(rules);

        assert_eq!(render_workspace_members(dir.path()).unwrap(), "libs/domain\nlibs/usecase");
        assert_eq!(render_direct_checks(dir.path()).unwrap(), "domain\tusecase\nusecase\t");
        assert!(render_workspace_tree(dir.path()).unwrap().contains("# domain crate"));

        let err = render_workspace_tree_full(dir.path()).unwrap_err();
        assert!(matches!(err, ArchRulesError::InvalidRules(_)), "unexpected: {err:?}");
    }

    #[test]
    fn render_workspace_tree_uses_tree_drawing_chars() {
        let dir = setup_dir(MINIMAL_RULES);
        let out = render_workspace_tree(dir.path()).unwrap();
        // At least one tree drawing sequence should appear
        let has_tree_chars = out.contains("├── ") || out.contains("└── ");
        assert!(has_tree_chars, "expected tree drawing chars in:\n{out}");
    }

    // -----------------------------------------------------------------------
    // Error cases
    // -----------------------------------------------------------------------

    #[test]
    fn missing_file_returns_io_error() {
        let dir = TempDir::new().unwrap();
        let err = render_workspace_members(dir.path()).unwrap_err();
        assert!(matches!(err, ArchRulesError::Io { .. }), "unexpected: {err:?}");
    }

    #[test]
    fn missing_file_error_display_does_not_include_workspace_path() {
        let dir = TempDir::new().unwrap();
        let err = render_workspace_members(dir.path()).unwrap_err();
        assert!(!err.to_string().contains(&dir.path().display().to_string()));
        assert!(err.to_string().contains("architecture-rules.json"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_architecture_rules_returns_io_error() {
        let dir = TempDir::new().unwrap();
        let real = dir.path().join("real-rules.json");
        let link = dir.path().join("architecture-rules.json");
        fs::write(&real, MINIMAL_RULES).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let err = render_workspace_members(dir.path()).unwrap_err();
        assert!(matches!(&err, ArchRulesError::Io { .. }), "unexpected: {err:?}");
        assert!(err.to_string().contains("refusing to follow symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_root_returns_io_error() {
        let real_root = setup_dir(MINIMAL_RULES);
        let link_parent = TempDir::new().unwrap();
        let root_link = link_parent.path().join("workspace-link");
        std::os::unix::fs::symlink(real_root.path(), &root_link).unwrap();

        let err = render_workspace_members(&root_link).unwrap_err();
        assert!(matches!(&err, ArchRulesError::Io { .. }), "unexpected: {err:?}");
        assert!(err.to_string().contains("refusing to use symlinked root"));
    }

    #[test]
    fn invalid_json_returns_parse_error() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("architecture-rules.json"), "not json").unwrap();
        let err = render_workspace_members(dir.path()).unwrap_err();
        assert!(matches!(err, ArchRulesError::Parse(_)), "unexpected: {err:?}");
    }

    #[test]
    fn empty_layers_returns_invalid_rules_error() {
        let dir = setup_dir(r#"{ "layers": [] }"#);
        let err = render_workspace_members(dir.path()).unwrap_err();
        assert!(matches!(err, ArchRulesError::InvalidRules(_)), "unexpected: {err:?}");
    }

    #[test]
    fn duplicate_crate_returns_invalid_rules_error() {
        let rules = r#"{
  "layers": [
    { "crate": "domain", "path": "libs/domain",  "may_depend_on": [] },
    { "crate": "domain", "path": "libs/domain2", "may_depend_on": [] }
  ]
}"#;
        let dir = setup_dir(rules);
        let err = render_workspace_members(dir.path()).unwrap_err();
        assert!(matches!(err, ArchRulesError::InvalidRules(_)), "unexpected: {err:?}");
    }

    #[test]
    fn unknown_dependency_returns_invalid_rules_error() {
        let rules = r#"{
  "layers": [
    { "crate": "domain",  "path": "libs/domain",  "may_depend_on": ["nonexistent"] }
  ]
}"#;
        let dir = setup_dir(rules);
        let err = render_workspace_members(dir.path()).unwrap_err();
        assert!(matches!(err, ArchRulesError::InvalidRules(_)), "unexpected: {err:?}");
    }

    #[test]
    fn empty_may_depend_on_item_returns_invalid_rules_error() {
        let rules = r#"{
  "layers": [
    { "crate": "domain", "path": "libs/domain", "may_depend_on": [""] }
  ]
}"#;
        let dir = setup_dir(rules);
        let err = render_workspace_members(dir.path()).unwrap_err();
        assert!(matches!(err, ArchRulesError::InvalidRules(_)), "unexpected: {err:?}");
    }

    #[test]
    fn extra_dirs_duplicate_layer_path_returns_invalid_rules_error() {
        let rules = r#"{
  "layers": [
    { "crate": "domain", "path": "libs/domain", "may_depend_on": [] }
  ],
  "extra_dirs": [
    { "path": "libs/domain", "label": "duplicate" }
  ]
}"#;
        let dir = setup_dir(rules);
        let err = render_workspace_tree_full(dir.path()).unwrap_err();
        assert!(matches!(err, ArchRulesError::InvalidRules(_)), "unexpected: {err:?}");
    }

    #[test]
    fn duplicate_extra_dirs_path_returns_invalid_rules_error() {
        let rules = r#"{
  "layers": [
    { "crate": "domain", "path": "libs/domain", "may_depend_on": [] }
  ],
  "extra_dirs": [
    { "path": "track", "label": "state" },
    { "path": "track", "label": "duplicate" }
  ]
}"#;
        let dir = setup_dir(rules);
        let err = render_workspace_tree_full(dir.path()).unwrap_err();
        assert!(matches!(err, ArchRulesError::InvalidRules(_)), "unexpected: {err:?}");
    }
}
