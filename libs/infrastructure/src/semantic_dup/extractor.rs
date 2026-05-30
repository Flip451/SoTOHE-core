//! Workspace Rust-file scanner and code-fragment extractor.
//!
//! Provides a single public function, [`extract_code_fragments`], that walks a
//! workspace root recursively, finds all `*.rs` source files, and splits each
//! file into item-level [`domain::semantic_dup::CodeFragment`] values.
//!
//! **Granularity:** one fragment per function or top-level `impl` block,
//! extracted by locating lines that begin a Rust item declaration — optional
//! visibility (`pub`, `pub(crate)`, …) and modifiers (`async`, `unsafe`,
//! `const`, `default`, `extern`) followed by `fn` or `impl` — and treating
//! each such line as the start of a new fragment.  This item-level granularity
//! captures the semantic unit most likely to be re-implemented without
//! exploding the index size with sub-expression noise.
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
/// file into item-level [`CodeFragment`] values (one per `fn` or `impl` item).
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
            if is_excluded_dir(&path) {
                continue;
            }
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

/// Return `true` when `path` is a directory that should be skipped during
/// recursive scanning.
///
/// Two categories are excluded:
/// - `target`: Cargo's build-output directory.  Generated `.rs` files there
///   (e.g. proc-macro expansions, build-script outputs) are not workspace
///   sources and would skew similarity results.
/// - Dot-prefixed (hidden) directories: covers `.git` (VCS internals),
///   `.fastembed_cache` (model-download cache), and temporary rebuild
///   siblings such as `.{name}.tmp-build` / `.{name}.old`.  None of these
///   contain workspace source code.
///
/// The LanceDB index directory (`--db-path`) holds only LanceDB data files,
/// not `.rs` source, so it contributes nothing even if descended.  Because
/// its path is user-configurable, name-based exclusion of `target` and
/// dot-prefixed dirs is the pragmatic, dependency-free solution — no
/// `.gitignore` parsing required.
fn is_excluded_dir(path: &Path) -> bool {
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name == "target" || name.starts_with('.'),
        None => false,
    }
}

/// Read `file_path` and split its content into item-level [`CodeFragment`]s.
///
/// Splitting heuristic: a line that begins a Rust item declaration (as
/// determined by [`is_item_boundary`]) starts a new fragment.  The previous
/// fragment is closed at that boundary.  This is intentionally heuristic —
/// it handles module-level and `impl`-level function definitions, including
/// visibility and modifier prefixes, without the complexity of a full Rust
/// parser.
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
/// item-headed block (identified by [`is_item_boundary`]) up to (but not
/// including) the next boundary line.  Lines before the first boundary form
/// a leading fragment (module-level attributes, `use` declarations, etc.)
/// that is included only if it is non-empty.
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

/// Return `true` when `line` starts a Rust item boundary.
///
/// Handles optional leading visibility qualifiers (`pub`, `pub(crate)`,
/// `pub(super)`, `pub(in ...)`) and function/impl modifiers (`async`,
/// `unsafe`, `const`, `default`, `extern`) in any valid order, followed
/// by the `fn` or `impl` keyword.
///
/// Examples that are recognised:
/// - `fn foo()`
/// - `pub fn foo()`
/// - `pub(crate) async fn foo()`
/// - `pub unsafe fn foo()`
/// - `const fn foo()`
/// - `impl Foo`
/// - `impl<T> Foo`
/// - `pub(super) default fn foo()`
///
/// Uses only safe string operations — no panicking index/slice.
fn is_item_boundary(line: &str) -> bool {
    /// Consume one leading token from `s` (splitting on ASCII whitespace),
    /// returning `(token, rest)`.  Returns `("", s)` if nothing is left.
    fn next_token(s: &str) -> (&str, &str) {
        let s = s.trim_start();
        if s.is_empty() {
            return ("", s);
        }
        // `pub(...)` is a single logical token; consume until the matching `)`.
        if s.starts_with("pub(") {
            if let Some(end) = s.find(')') {
                let tok = s.get(..=end).unwrap_or(s);
                let rest = s.get(end + 1..).unwrap_or("").trim_start();
                return (tok, rest);
            }
        }
        // Otherwise split at the first whitespace character.
        match s.find(|c: char| c.is_ascii_whitespace()) {
            Some(pos) => {
                let tok = s.get(..pos).unwrap_or(s);
                let rest = s.get(pos..).unwrap_or("").trim_start();
                (tok, rest)
            }
            None => (s, ""),
        }
    }

    let trimmed = line.trim_start();
    let mut rest = trimmed;

    // Consume optional visibility qualifier: `pub` / `pub(...)`.
    {
        let (tok, after) = next_token(rest);
        if tok == "pub" || tok.starts_with("pub(") {
            rest = after;
        }
    }

    // Consume zero or more modifiers that may legally precede `fn` or `impl`:
    // `async`, `unsafe`, `const`, `default`, `extern`.
    //
    // Special case for `extern`: it may be followed by an ABI string such as
    // `"C"` or `"system"` (e.g. `extern "C" fn foo()`).  Consume the ABI
    // string token as well so the subsequent keyword check sees `fn` / `impl`.
    loop {
        let (tok, after) = next_token(rest);
        match tok {
            "async" | "unsafe" | "const" | "default" => {
                rest = after;
            }
            "extern" => {
                rest = after;
                // Optionally consume the ABI string (e.g. `"C"`, `"system"`).
                let (maybe_abi, after_abi) = next_token(rest);
                if maybe_abi.starts_with('"') {
                    rest = after_abi;
                }
            }
            _ => break,
        }
    }

    // The next token must be the item keyword.
    //
    // Special case: `impl<T>` has no whitespace between `impl` and `<`, so
    // `next_token` returns `("impl<T>", ...)` instead of `("impl", "<T>...")`.
    // We handle this by also checking whether `rest` starts with `"impl<"`.
    if rest.starts_with("impl<") {
        // `impl<...>` generic form: unconditionally a boundary.
        return true;
    }

    let (keyword, after_keyword) = next_token(rest);
    match keyword {
        "fn" => {
            // Require at least one character after `fn` (function name or whitespace),
            // to avoid matching a bare identifier named `fn` without a space.
            !after_keyword.is_empty()
        }
        "impl" => {
            // `impl` followed by whitespace + name.  The `impl<` case is handled
            // above, so here `after_keyword` starts with a name or is empty.
            true
        }
        _ => false,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    // ── is_item_boundary ──────────────────────────────────────────────────────

    #[test]
    fn test_is_item_boundary_plain_fn() {
        assert!(is_item_boundary("fn foo() {}"));
    }

    #[test]
    fn test_is_item_boundary_pub_fn() {
        assert!(is_item_boundary("pub fn foo() {}"));
    }

    #[test]
    fn test_is_item_boundary_pub_crate_fn() {
        assert!(is_item_boundary("pub(crate) fn foo() {}"));
    }

    #[test]
    fn test_is_item_boundary_pub_super_fn() {
        assert!(is_item_boundary("pub(super) fn foo() {}"));
    }

    #[test]
    fn test_is_item_boundary_pub_in_path_fn() {
        assert!(is_item_boundary("pub(in crate::foo) fn foo() {}"));
    }

    #[test]
    fn test_is_item_boundary_async_fn() {
        assert!(is_item_boundary("async fn handle() {}"));
    }

    #[test]
    fn test_is_item_boundary_pub_async_fn() {
        assert!(is_item_boundary("pub async fn handle() {}"));
    }

    #[test]
    fn test_is_item_boundary_unsafe_fn() {
        assert!(is_item_boundary("unsafe fn raw_op() {}"));
    }

    #[test]
    fn test_is_item_boundary_pub_unsafe_fn() {
        assert!(is_item_boundary("pub unsafe fn raw_op() {}"));
    }

    #[test]
    fn test_is_item_boundary_const_fn() {
        assert!(is_item_boundary("const fn compute() -> u32 { 0 }"));
    }

    #[test]
    fn test_is_item_boundary_pub_const_fn() {
        assert!(is_item_boundary("pub const fn compute() -> u32 { 0 }"));
    }

    #[test]
    fn test_is_item_boundary_pub_crate_async_fn() {
        assert!(is_item_boundary("pub(crate) async fn do_work() {}"));
    }

    #[test]
    fn test_is_item_boundary_plain_impl() {
        assert!(is_item_boundary("impl Foo {"));
    }

    #[test]
    fn test_is_item_boundary_impl_with_generics_no_space() {
        // `impl<T>` — no space between `impl` and `<`
        assert!(is_item_boundary("impl<T> Foo<T> {"));
    }

    #[test]
    fn test_is_item_boundary_impl_trait() {
        assert!(is_item_boundary("impl Bar for Baz {"));
    }

    #[test]
    fn test_is_item_boundary_indented_fn() {
        assert!(is_item_boundary("    fn inner() {}"));
    }

    #[test]
    fn test_is_item_boundary_indented_pub_async_fn() {
        assert!(is_item_boundary("    pub async fn handler() {}"));
    }

    #[test]
    fn test_is_item_boundary_extern_c_fn() {
        assert!(is_item_boundary("extern \"C\" fn raw_c() {}"));
    }

    #[test]
    fn test_is_item_boundary_pub_extern_c_fn() {
        assert!(is_item_boundary("pub extern \"C\" fn raw_c() {}"));
    }

    #[test]
    fn test_is_item_boundary_extern_system_fn() {
        assert!(is_item_boundary("extern \"system\" fn raw_sys() {}"));
    }

    #[test]
    fn test_is_item_boundary_extern_fn_no_abi() {
        // `extern fn` without an ABI string is also valid Rust (defaults to "C").
        assert!(is_item_boundary("extern fn ffi_fn() {}"));
    }

    #[test]
    fn test_is_item_boundary_unsafe_extern_c_fn() {
        assert!(is_item_boundary("unsafe extern \"C\" fn raw_unsafe() {}"));
    }

    // Non-boundary lines must NOT be matched.

    #[test]
    fn test_is_item_boundary_let_binding_not_boundary() {
        assert!(!is_item_boundary("let x = fn_name();"));
    }

    #[test]
    fn test_is_item_boundary_fn_name_prefix_not_boundary() {
        assert!(!is_item_boundary("fn_name()"));
    }

    #[test]
    fn test_is_item_boundary_implement_not_boundary() {
        assert!(!is_item_boundary("implement this trait"));
    }

    #[test]
    fn test_is_item_boundary_comment_not_boundary() {
        assert!(!is_item_boundary("// fn foo() {}"));
    }

    #[test]
    fn test_is_item_boundary_use_decl_not_boundary() {
        assert!(!is_item_boundary("use std::path::Path;"));
    }

    #[test]
    fn test_is_item_boundary_struct_not_boundary() {
        assert!(!is_item_boundary("pub struct Foo {}"));
    }

    #[test]
    fn test_is_item_boundary_empty_line_not_boundary() {
        assert!(!is_item_boundary(""));
        assert!(!is_item_boundary("   "));
    }

    // ── is_excluded_dir ───────────────────────────────────────────────────────

    #[test]
    fn test_is_excluded_dir_target() {
        assert!(is_excluded_dir(std::path::Path::new("/workspace/target")));
    }

    #[test]
    fn test_is_excluded_dir_hidden_git() {
        assert!(is_excluded_dir(std::path::Path::new("/workspace/.git")));
    }

    #[test]
    fn test_is_excluded_dir_hidden_fastembed_cache() {
        assert!(is_excluded_dir(std::path::Path::new("/workspace/.fastembed_cache")));
    }

    #[test]
    fn test_is_excluded_dir_hidden_tmp_build() {
        assert!(is_excluded_dir(std::path::Path::new("/workspace/.mylib.tmp-build")));
    }

    #[test]
    fn test_is_excluded_dir_regular_src_not_excluded() {
        assert!(!is_excluded_dir(std::path::Path::new("/workspace/src")));
    }

    #[test]
    fn test_is_excluded_dir_regular_libs_not_excluded() {
        assert!(!is_excluded_dir(std::path::Path::new("/workspace/libs")));
    }

    // ── collect_rs_fragments exclusion (integration) ──────────────────────────

    /// Verify that `extract_code_fragments` only picks up sources from real
    /// source directories and skips `target/`, `.git/`, and other hidden dirs.
    #[test]
    fn test_extract_code_fragments_skips_excluded_dirs() {
        use std::fs;
        use tempfile::tempdir;

        let root = tempdir().unwrap();
        let root_path = root.path();

        // A real source file at the workspace root — must be extracted.
        let src_content = "pub fn real_fn() {}\n";
        fs::write(root_path.join("real_source.rs"), src_content).unwrap();

        // target/generated.rs — must be skipped.
        let target_dir = root_path.join("target");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(target_dir.join("generated.rs"), "pub fn generated() {}\n").unwrap();

        // .git/hooks.rs — must be skipped.
        let git_dir = root_path.join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("hooks.rs"), "pub fn git_hook() {}\n").unwrap();

        // .hidden_dir/foo.rs — must be skipped.
        let hidden_dir = root_path.join(".hidden_dir");
        fs::create_dir_all(&hidden_dir).unwrap();
        fs::write(hidden_dir.join("foo.rs"), "pub fn hidden_fn() {}\n").unwrap();

        let fragments = extract_code_fragments(root_path).unwrap();

        // All returned fragments must come from real_source.rs only.
        assert!(!fragments.is_empty(), "expected at least one fragment from real_source.rs");
        for fragment in &fragments {
            assert_eq!(
                fragment.source_path,
                root_path.join("real_source.rs"),
                "unexpected fragment from excluded path: {}",
                fragment.source_path.display()
            );
        }
    }
}
