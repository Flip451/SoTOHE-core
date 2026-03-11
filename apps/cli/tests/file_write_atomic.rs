//! Integration tests for `sotp file write-atomic`.

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::{Command, Stdio};

fn sotp_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sotp"))
}

#[test]
fn test_write_atomic_creates_file_with_correct_content() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("output.txt");

    let mut child = sotp_bin()
        .args(["file", "write-atomic", "--path", target.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"hello atomic world").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello atomic world");
}

#[test]
fn test_write_atomic_overwrites_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("existing.txt");
    std::fs::write(&target, "old content").unwrap();

    let mut child = sotp_bin()
        .args(["file", "write-atomic", "--path", target.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"new content").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "new content");
}

#[test]
fn test_write_atomic_no_temp_files_remain() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("clean.txt");

    let mut child = sotp_bin()
        .args(["file", "write-atomic", "--path", target.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"content").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());

    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().filter_map(|e| e.ok()).collect();
    assert_eq!(entries.len(), 1, "expected only target file, found: {entries:?}");
    assert_eq!(entries[0].file_name(), "clean.txt");
}

#[test]
fn test_write_atomic_fails_for_nonexistent_parent() {
    let mut child = sotp_bin()
        .args(["file", "write-atomic", "--path", "/nonexistent/dir/file.txt"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"data").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("atomic write failed"), "stderr: {stderr}");
}
