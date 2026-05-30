//! Workspace Rust-file scanner and code-fragment extractor.
//!
//! Provides a single public function, [`extract_code_fragments`], that walks a
//! workspace root recursively, finds all `*.rs` source files, and splits each
//! file into item-level [`domain::semantic_dup::CodeFragment`] values.
//!
//! **Granularity:** one fragment per function or top-level `impl` block,
//! extracted by locating lines that start (after optional whitespace) with `fn `
//! or `impl ` and treating each such line as the start of a new fragment.
//! This item-level granularity captures the semantic unit most likely to be
//! re-implemented without exploding the index size with sub-expression noise.
//!
//! Empty fragments (e.g. from consecutive boundary lines) are silently dropped
//! because [`domain::semantic_dup::CodeFragment::new`] rejects empty content.

use std::path::Path;

use domain::semantic_dup::CodeFragment;

// ── ExtractError ──────────────────────────────────────────────────────────────

/// Errors that can occur while scanning the workspace and extracting fragments.
#[derive(Debug)]
pub enum ExtractError {
    /// A filesystem I/O error (reading a directory entry or file content).
    Io {
        /// Human-readable description of the operation that failed, with the
        /// underlying OS error appended.
        source: String,
    },
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { source } => write!(f, "extractor I/O error: {source}"),
        }
    }
}

impl std::error::Error for ExtractError {}

// ── Public API ────────────────────────────────────────────────────────────────

/// Walk `workspace_root` recursively, collect every `*.rs` file, and split each
/// file into item-level [`CodeFragment`] values (one per `fn ` or `impl ` block).
///
/// The extractor is called by the CLI composition root before constructing
/// [`usecase::semantic_dup::BuildIndexCommand`] or
/// [`usecase::semantic_dup::MeasureQualityCommand`].  IO errors from this
/// function are propagated by the CLI, which may wrap them in
/// `BuildIndexError::Io` / `MeasureQualityError::Io`.
///
/// Empty fragments that arise from consecutive boundary lines are silently
/// dropped (`CodeFragment::new` rejects empty content and this function skips
/// such rejections without returning an error).
///
/// # Errors
///
/// Returns [`ExtractError::Io`] when a directory entry cannot be read or when
/// a source file cannot be read as UTF-8 text.
pub fn extract_code_fragments(workspace_root: &Path) -> Result<Vec<CodeFragment>, ExtractError> {
    let mut fragments = Vec::new();
    collect_rs_fragments(workspace_root, &mut fragments)?;
    Ok(fragments)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Recursively walk `dir`, appending fragments from every `*.rs` file found.
fn collect_rs_fragments(dir: &Path, out: &mut Vec<CodeFragment>) -> Result<(), ExtractError> {
    let read_dir = std::fs::read_dir(dir).map_err(|e| ExtractError::Io {
        source: format!("cannot read directory '{}': {e}", dir.display()),
    })?;

    for entry_result in read_dir {
        let entry = entry_result.map_err(|e| ExtractError::Io {
            source: format!("cannot read directory entry in '{}': {e}", dir.display()),
        })?;

        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| ExtractError::Io {
            source: format!("cannot determine file type of '{}': {e}", path.display()),
        })?;

        if file_type.is_dir() {
            collect_rs_fragments(&path, out)?;
        } else if file_type.is_file() && has_rs_extension(&path) {
            let file_fragments = extract_from_file(&path)?;
            out.extend(file_fragments);
        }
        // symlinks and other file types are silently skipped
    }

    Ok(())
}

/// Return `true` when `path` has the `.rs` extension (case-sensitive).
fn has_rs_extension(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "rs")
}

/// Read `file_path` and split its content into item-level [`CodeFragment`]s.
///
/// Splitting heuristic: a line that starts with `fn ` or `impl ` (ignoring
/// leading ASCII whitespace) begins a new fragment.  The previous fragment is
/// closed at that boundary.  This is intentionally simple — it handles the
/// common case of module-level and `impl`-level function definitions without
/// the complexity of a full Rust parser.
fn extract_from_file(file_path: &Path) -> Result<Vec<CodeFragment>, ExtractError> {
    let source = std::fs::read_to_string(file_path).map_err(|e| ExtractError::Io {
        source: format!("cannot read '{}': {e}", file_path.display()),
    })?;

    let fragments = split_into_fragments(&source);
    let mut result = Vec::with_capacity(fragments.len());

    for text in fragments {
        // CodeFragment::new rejects empty content; skip silently.
        if let Ok(fragment) = CodeFragment::new(file_path.to_path_buf(), text) {
            result.push(fragment);
        }
    }

    Ok(result)
}

/// Split `source` text into item-level fragment strings.
///
/// Each element in the returned `Vec` corresponds to the lines from one
/// `fn `- or `impl `-headed block up to (but not including) the next such
/// boundary line.  Lines before the first boundary form a leading fragment
/// (module-level attributes, `use` declarations, etc.) that is included only
/// if it is non-empty.
///
/// The split is line-oriented and does not parse Rust syntax, so nested
/// functions (inside `impl` blocks or closures) are included in the outer
/// fragment's text rather than being extracted separately.  This is intentional:
/// item-level granularity captures the semantic unit most likely to be
/// duplicated.
fn split_into_fragments(source: &str) -> Vec<String> {
    let mut fragments: Vec<String> = Vec::new();
    let mut current = String::new();

    for line in source.lines() {
        if is_item_boundary(line) {
            // Flush the accumulated lines as a fragment.
            let trimmed = current.trim().to_owned();
            if !trimmed.is_empty() {
                fragments.push(trimmed);
            }
            current = String::new();
        }
        // Append the line (including newline) to the current fragment.
        current.push_str(line);
        current.push('\n');
    }

    // Flush the last fragment.
    let trimmed = current.trim().to_owned();
    if !trimmed.is_empty() {
        fragments.push(trimmed);
    }

    fragments
}

/// Return `true` when `line` is an item-boundary line: it starts (after
/// stripping leading ASCII whitespace) with `fn ` or `impl `.
///
/// The space after `fn`/`impl` is required to avoid matching identifiers like
/// `fn_name` or `implement` that merely start with those letters.
fn is_item_boundary(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("fn ") || trimmed.starts_with("impl ")
}
