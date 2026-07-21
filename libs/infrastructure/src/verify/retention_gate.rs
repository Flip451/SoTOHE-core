//! Retention verification for retired gate/document identifiers.
//!
//! The check scans the configured live surface and fails when retired
//! identifiers or same-line state-marker expressions reappear. Missing live
//! roots are allowed; I/O failures on existing roots are reported as errors.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};

pub(crate) const LIVE_SURFACE: &[&str] = &[
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

const GATE_WORDS_JP: &[&str] = &["実装を開始", "準備完了", "ブロック"];
const MAX_SCAN_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// Verify the retention live surface under `root`.
pub fn verify(root: &Path) -> VerifyOutcome {
    let findings =
        scan_live_surface(root).into_iter().map(VerifyFinding::error).collect::<Vec<_>>();
    VerifyOutcome::from_findings(findings)
}

fn m_token() -> String {
    ["TO", "DO", ":"].concat()
}

fn forbidden_tokens() -> Vec<String> {
    let kebab = ["tech", "stack"].join("-");
    let snake = ["tech", "stack"].join("_");
    let camel = ["Tech", "Stack"].concat();
    vec![
        format!("verify-{kebab}"),
        format!("verify-{kebab}-local"),
        format!("verify {kebab}"),
        format!("VerifyCommand::{camel}"),
        format!("VerifyInput::{camel}"),
        format!("verify_{snake}"),
        ["TECH", "STACK", "FILE"].join("_"),
        format!("verify::{snake}"),
        format!("pub mod {snake};"),
        format!("track/{kebab}.md"),
        ["track/", "product", ".md"].concat(),
        ["track/", "product-guidelines", ".md"].concat(),
    ]
}

fn scan_live_surface(root: &Path) -> Vec<String> {
    let tokens = forbidden_tokens();
    let marker = m_token();
    let mut findings = Vec::new();

    for rel in LIVE_SURFACE {
        let path = root.join(rel);
        match path.symlink_metadata() {
            Ok(metadata) => {
                walk_with_metadata(&path, metadata, &tokens, marker.as_str(), &mut findings)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => findings
                .push(format!("{}: I/O error checking live-surface root: {e}", path.display())),
        }
    }

    findings
}

fn walk(path: &Path, tokens: &[String], marker: &str, findings: &mut Vec<String>) {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(e) => {
            findings.push(format!("{}: I/O error reading metadata: {e}", path.display()));
            return;
        }
    };

    walk_with_metadata(path, metadata, tokens, marker, findings);
}

fn walk_with_metadata(
    path: &Path,
    metadata: std::fs::Metadata,
    tokens: &[String],
    marker: &str,
    findings: &mut Vec<String>,
) {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        findings.push(format!("{}: symlink guard: refusing to follow symlink", path.display()));
    } else if file_type.is_dir() {
        walk_dir(path, tokens, marker, findings);
    } else if file_type.is_file() {
        scan_file(path, tokens, marker, findings);
    } else {
        findings.push(format!("{}: unsupported live-surface file type", path.display()));
    }
}

fn walk_dir(path: &Path, tokens: &[String], marker: &str, findings: &mut Vec<String>) {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            findings.push(format!("{}: I/O error listing directory: {e}", path.display()));
            return;
        }
    };

    let mut children = Vec::<PathBuf>::new();
    for entry in entries {
        match entry {
            Ok(entry) => children.push(entry.path()),
            Err(e) => {
                findings.push(format!("{}: I/O error reading directory entry: {e}", path.display()))
            }
        }
    }

    children.sort();
    for child in children {
        walk(child.as_path(), tokens, marker, findings);
    }
}

fn scan_file(path: &Path, tokens: &[String], marker: &str, findings: &mut Vec<String>) {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) => {
            findings.push(format!("{}: I/O error reading file: {e}", path.display()));
            return;
        }
    };
    let mut bytes = Vec::new();
    if let Err(e) = file.take(MAX_SCAN_FILE_BYTES + 1).read_to_end(&mut bytes) {
        findings.push(format!("{}: I/O error reading file: {e}", path.display()));
        return;
    }
    if bytes.len() as u64 > MAX_SCAN_FILE_BYTES {
        findings.push(format!(
            "{}: file exceeds retention scan size limit of {MAX_SCAN_FILE_BYTES} bytes",
            path.display()
        ));
        return;
    }

    let content = String::from_utf8_lossy(&bytes);
    for (idx, line) in content.lines().enumerate() {
        let line_number = idx + 1;
        for token in tokens {
            if line.contains(token.as_str()) {
                findings.push(format!(
                    "{}:{line_number}: retired identifier `{token}` reappeared",
                    path.display()
                ));
            }
        }
        if line.contains(marker) && has_gate_word(line) {
            findings.push(format!(
                "{}:{line_number}: state-marker expression reappeared",
                path.display()
            ));
        }
    }
}

fn has_gate_word(line: &str) -> bool {
    let lower = line.to_lowercase();
    GATE_WORDS_ASCII.iter().any(|word| lower.contains(word))
        || GATE_WORDS_JP.iter().any(|word| line.contains(word))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    fn representative_file(rel: &str) -> String {
        match rel {
            "CLAUDE.md" | "README.md" | "Makefile.toml" => rel.to_owned(),
            "libs" => "libs/domain/src/lib.rs".to_owned(),
            "apps" => "apps/cli/src/main.rs".to_owned(),
            "knowledge/conventions" => "knowledge/conventions/example.md".to_owned(),
            ".harness/config" => ".harness/config/example.json".to_owned(),
            _ => format!("{rel}/example.md"),
        }
    }

    fn synthetic_layout(root: &Path) {
        for rel in LIVE_SURFACE {
            write(root, representative_file(rel).as_str(), "clean content\n");
        }
    }

    #[test]
    fn test_retention_gate_live_surface_roots_match_contract() {
        assert_eq!(
            LIVE_SURFACE,
            &[
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
            ]
        );
    }

    #[test]
    fn test_retention_gate_clean_synthetic_layout_passes() {
        let tmp = tempfile::tempdir().unwrap();
        synthetic_layout(tmp.path());

        let outcome = verify(tmp.path());

        assert!(outcome.is_ok(), "clean layout must pass");
    }

    #[test]
    fn test_retention_gate_each_retired_token_fails() {
        for token in forbidden_tokens() {
            let tmp = tempfile::tempdir().unwrap();
            synthetic_layout(tmp.path());
            write(tmp.path(), ".claude/rules/example.md", &format!("see {token}\n"));

            let outcome = verify(tmp.path());

            assert!(outcome.has_errors(), "token must be detected: {token}");
        }
    }

    #[test]
    fn test_retention_gate_each_live_surface_root_is_scanned() {
        let token = forbidden_tokens().into_iter().next().expect("token list");
        for rel in LIVE_SURFACE {
            let tmp = tempfile::tempdir().unwrap();
            synthetic_layout(tmp.path());
            write(tmp.path(), representative_file(rel).as_str(), &format!("{token}\n"));

            let outcome = verify(tmp.path());

            assert!(outcome.has_errors(), "root must be scanned: {rel}");
        }
    }

    #[test]
    fn test_retention_gate_gate_word_before_marker_fails() {
        let tmp = tempfile::tempdir().unwrap();
        synthetic_layout(tmp.path());
        let line = format!("Implementation is blocked while docs contain {}\n", m_token());
        write(tmp.path(), "knowledge/conventions/example.md", line.as_str());

        let outcome = verify(tmp.path());

        assert!(outcome.has_errors(), "gate-word-first state marker must fail");
    }

    #[test]
    fn test_retention_gate_marker_before_gate_word_fails() {
        let tmp = tempfile::tempdir().unwrap();
        synthetic_layout(tmp.path());
        let line = format!("{} resolve this before implementation can start\n", m_token());
        write(tmp.path(), "CLAUDE.md", line.as_str());

        let outcome = verify(tmp.path());

        assert!(outcome.has_errors(), "marker-first state marker must fail");
    }

    #[test]
    fn test_retention_gate_marker_without_gate_word_passes() {
        let tmp = tempfile::tempdir().unwrap();
        synthetic_layout(tmp.path());
        let line = format!("{} fix the typo in the second column\n", m_token());
        write(tmp.path(), "README.md", line.as_str());

        let outcome = verify(tmp.path());

        assert!(outcome.is_ok(), "plain marker line must pass");
    }

    #[test]
    fn test_retention_gate_oversized_file_fails_closed() {
        let tmp = tempfile::tempdir().unwrap();
        synthetic_layout(tmp.path());
        let path = tmp.path().join("README.md");
        let oversized = vec![b'a'; (MAX_SCAN_FILE_BYTES + 1) as usize];
        std::fs::write(path, oversized).unwrap();

        let outcome = verify(tmp.path());

        assert!(outcome.has_errors(), "oversized file must fail closed");
        let messages =
            outcome.findings().iter().map(|finding| finding.message()).collect::<Vec<_>>();
        assert!(
            messages.iter().any(|message| message.contains("scan size limit")),
            "expected scan size limit finding, got: {messages:?}"
        );
    }

    #[test]
    fn test_retention_gate_file_open_error_fails_closed() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("missing.md");
        let mut findings = Vec::new();

        scan_file(path.as_path(), &forbidden_tokens(), m_token().as_str(), &mut findings);

        assert_eq!(findings.len(), 1);
        assert!(
            findings.iter().any(|finding| finding.contains("I/O error reading file")),
            "expected read error finding, got: {findings:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_retention_gate_symlinked_live_surface_child_fails_closed() {
        let tmp = tempfile::tempdir().unwrap();
        synthetic_layout(tmp.path());
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("escaped.md"), "clean content\n").unwrap();
        let link = tmp.path().join(".claude/rules/escaped.md");
        std::os::unix::fs::symlink(outside.path().join("escaped.md"), link.as_path()).unwrap();

        let outcome = verify(tmp.path());

        assert!(outcome.has_errors(), "symlinked child must fail closed");
        let messages =
            outcome.findings().iter().map(|finding| finding.message()).collect::<Vec<_>>();
        assert!(
            messages.iter().any(|message| message.contains("refusing to follow symlink")),
            "expected symlink rejection finding, got: {messages:?}"
        );
    }
}
