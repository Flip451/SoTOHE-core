//! Retention gate for the retired tech-stack verify gate and the retired
//! track direct-docs identifiers (IN-09, CN-01, CN-05, AC-11).
//!
//! Existence-based scan of the CN-01 live surface: fails when a retired
//! identifier, a retired file path, or a TODO-marker state expression
//! (M token + readiness / blocking gate word on the same line, either order)
//! reappears. No state fields are read or written — presence in file content
//! is the only signal. Fail-closed: any I/O error panics.
//!
//! Forbidden tokens are assembled by string concatenation and this file is
//! additionally excluded from the scan via [`SELF_REL`] so the definition
//! list cannot flag itself.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

/// This test file, relative to the workspace root — excluded from the scan
/// (explicit allow-list) so the token definitions below do not flag themselves.
const SELF_REL: &str = "apps/cli/tests/retention_gate.rs";

/// CN-01 live-surface roots, relative to the workspace root.
///
/// Entries may be single files or directories (walked recursively). A missing
/// root contributes nothing: the check is existence-based, so absence of a
/// path trivially means absence of forbidden content under it.
const LIVE_SURFACE: &[&str] = &[
    "CLAUDE.md",
    "README.md",
    ".claude/rules",
    ".claude/commands",
    ".claude/agents",
    ".claude/skills",
    ".harness/workflows",
    ".harness/config",
    ".harness/capabilities",
    "Makefile.toml",
    "libs",
    "apps",
    "knowledge/conventions",
];

/// Readiness / blocking gate words (CN-05), ASCII set.
///
/// Matched case-insensitively so capitalized prose (e.g. "Implementation")
/// still counts as the same gate word.
const GATE_WORDS_ASCII: &[&str] = &[
    "block",
    "blocked",
    "unresolved",
    "ready",
    "readiness",
    "implementation",
    "start",
    "gate",
    "proceed",
    "pass",
    "fail",
];

/// Readiness / blocking gate words (CN-05), Japanese set. Matched verbatim.
const GATE_WORDS_JP: &[&str] = &["実装を開始", "準備完了", "ブロック"];

/// The M token (CN-05): ASCII `TODO` immediately followed by a colon (U+003A).
///
/// Assembled by concatenation so this definition is itself not an M token.
fn m_token() -> String {
    ["TO", "DO", ":"].concat()
}

/// Retired identifiers and file paths (IN-09), assembled by concatenation.
fn forbidden_tokens() -> Vec<String> {
    let kebab = ["tech", "stack"].join("-"); // tech-stack
    let snake = ["tech", "stack"].join("_"); // tech_stack
    let camel = ["Tech", "Stack"].concat(); // TechStack
    vec![
        // Makefile task names. `verify-<kebab>` also matches the `-local`
        // variant; both are listed so a violation names the exact task.
        format!("verify-{kebab}"),
        format!("verify-{kebab}-local"),
        // CLI subcommand path.
        format!("verify {kebab}"),
        // Rust identifiers.
        format!("VerifyCommand::{camel}"),
        format!("VerifyInput::{camel}"),
        format!("verify_{snake}"),
        ["TECH", "STACK", "FILE"].join("_"),
        // Module references.
        format!("verify::{snake}"),
        format!("pub mod {snake};"),
        // Retired file paths.
        format!("track/{kebab}.md"),
        ["track/", "product", ".md"].concat(),
        ["track/", "product-guidelines", ".md"].concat(),
    ]
}

/// Scan the CN-01 live surface under `root` and return all violations as
/// human-readable `path:line: reason` strings. Empty result means the gate
/// passes.
fn scan_live_surface(root: &Path) -> Vec<String> {
    let tokens = forbidden_tokens();
    let m = m_token();
    let mut violations = Vec::new();
    for rel in LIVE_SURFACE {
        let path = root.join(rel);
        if path.exists() {
            walk(&path, &tokens, &m, &mut violations);
        }
    }
    violations
}

fn walk(path: &Path, tokens: &[String], m: &str, out: &mut Vec<String>) {
    if path.is_dir() {
        let entries = std::fs::read_dir(path)
            .unwrap_or_else(|e| panic!("I/O error listing {}: {e}", path.display()));
        let mut children: Vec<PathBuf> = entries
            .map(|entry| {
                entry.unwrap_or_else(|e| panic!("I/O error listing {}: {e}", path.display())).path()
            })
            .collect();
        children.sort();
        for child in children {
            walk(&child, tokens, m, out);
        }
    } else {
        scan_file(path, tokens, m, out);
    }
}

fn scan_file(path: &Path, tokens: &[String], m: &str, out: &mut Vec<String>) {
    if path.ends_with(SELF_REL) {
        return;
    }
    let bytes =
        std::fs::read(path).unwrap_or_else(|e| panic!("I/O error reading {}: {e}", path.display()));
    let content = String::from_utf8_lossy(&bytes);
    for (idx, line) in content.lines().enumerate() {
        for token in tokens {
            if line.contains(token.as_str()) {
                out.push(format!(
                    "{}:{}: retired identifier `{token}` reappeared",
                    path.display(),
                    idx + 1
                ));
            }
        }
        if line.contains(m) && has_gate_word(line) {
            out.push(format!(
                "{}:{}: TODO-marker state expression (M token + readiness/blocking gate word)",
                path.display(),
                idx + 1
            ));
        }
    }
}

/// True when `line` contains any CN-05 readiness / blocking gate word.
///
/// Same-line co-existence with the M token is checked by the caller; a
/// contains-based check covers both orders (M token first or gate word first).
fn has_gate_word(line: &str) -> bool {
    let lower = line.to_lowercase();
    GATE_WORDS_ASCII.iter().any(|w| lower.contains(w))
        || GATE_WORDS_JP.iter().any(|w| line.contains(w))
}

/// Workspace root, resolved from this crate's manifest dir (`apps/cli`).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("apps/cli must live two levels below the workspace root")
        .to_path_buf()
}

// ── positive scan: real workspace ────────────────────────────────────────────

/// AC-11: the live surface of the real workspace must contain no retired
/// identifier, retired file path, or TODO-marker state expression.
#[test]
fn test_retention_gate_real_workspace_live_surface_is_clean() {
    let violations = scan_live_surface(&workspace_root());
    assert!(
        violations.is_empty(),
        "retention gate: retired tokens reappeared on the live surface:\n{}",
        violations.join("\n")
    );
}

// ── negative scan: synthetic live-surface layout ─────────────────────────────

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
}

/// Create a minimal clean live-surface layout covering file and directory
/// roots.
fn synthetic_layout(root: &Path) {
    write(root, "CLAUDE.md", "# Maintainer index\n");
    write(root, "README.md", "# Readme\n");
    write(root, ".claude/rules/01-language.md", "Think in English.\n");
    write(root, ".harness/config/signal-gates.json", "{}\n");
    write(root, "Makefile.toml", "[tasks.ci]\n");
    write(root, "libs/domain/src/lib.rs", "pub struct Marker;\n");
    write(root, "apps/cli/src/main.rs", "fn main() {}\n");
    write(root, "knowledge/conventions/example.md", "A live rule.\n");
}

#[test]
fn test_retention_gate_clean_synthetic_layout_passes() {
    let tmp = tempfile::tempdir().unwrap();
    synthetic_layout(tmp.path());
    let violations = scan_live_surface(tmp.path());
    assert!(violations.is_empty(), "clean layout must pass, got:\n{}", violations.join("\n"));
}

/// AC-11 (a): injecting any retired identifier into one live-surface file
/// must fail the scan.
#[test]
fn test_retention_gate_detects_each_injected_forbidden_token() {
    for token in forbidden_tokens() {
        let tmp = tempfile::tempdir().unwrap();
        synthetic_layout(tmp.path());
        write(tmp.path(), ".claude/rules/01-language.md", &format!("see {token} for details\n"));
        let violations = scan_live_surface(tmp.path());
        assert!(
            !violations.is_empty(),
            "scanner must detect injected retired identifier `{token}`"
        );
    }
}

/// AC-11 (b): the exact English sentence `Implementation is blocked while
/// docs contain ` followed by one space and the M token (line ends with the
/// M token) must fail the scan.
#[test]
fn test_retention_gate_detects_todo_marker_state_expression_gate_word_first() {
    let tmp = tempfile::tempdir().unwrap();
    synthetic_layout(tmp.path());
    let line = format!("Implementation is blocked while docs contain {}\n", m_token());
    write(tmp.path(), "knowledge/conventions/example.md", &line);
    let violations = scan_live_surface(tmp.path());
    assert!(
        !violations.is_empty(),
        "scanner must detect gate-word-first TODO-marker state expression"
    );
}

/// CN-05 / AC-11: the reversed order (M token first, gate word after) on the
/// same line must also fail the scan.
#[test]
fn test_retention_gate_detects_todo_marker_state_expression_marker_first() {
    let tmp = tempfile::tempdir().unwrap();
    synthetic_layout(tmp.path());
    let line = format!("{} resolve this before implementation can start\n", m_token());
    write(tmp.path(), "CLAUDE.md", &line);
    let violations = scan_live_surface(tmp.path());
    assert!(
        !violations.is_empty(),
        "scanner must detect marker-first TODO-marker state expression"
    );
}

/// An M token without any readiness / blocking gate word on the same line is
/// not a state expression (CN-05 forbids the co-existence pattern, not plain
/// task notes).
#[test]
fn test_retention_gate_allows_m_token_without_gate_word() {
    let tmp = tempfile::tempdir().unwrap();
    synthetic_layout(tmp.path());
    let line = format!("{} fix the typo in the second column\n", m_token());
    write(tmp.path(), "knowledge/conventions/example.md", &line);
    let violations = scan_live_surface(tmp.path());
    assert!(
        violations.is_empty(),
        "an M token without a gate word must not fail the gate, got:\n{}",
        violations.join("\n")
    );
}
