//! Verify that Rust source files do not exceed configured line limits.
//!
//! Configuration is read from `architecture-rules.json` → `module_limits`.

use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};

use super::syn_helpers::has_cfg_test_attr;

const ARCH_RULES_FILE: &str = "architecture-rules.json";

/// Check `.rs` file sizes against `module_limits` in architecture-rules.json.
///
/// # Errors
///
/// Returns findings when files exceed the configured thresholds.
pub fn verify(root: &Path) -> VerifyOutcome {
    let rules_path = root.join(ARCH_RULES_FILE);
    let rules_content = match std::fs::read_to_string(&rules_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot read {ARCH_RULES_FILE}: {e}"
            ))]);
        }
    };

    let rules: serde_json::Value = match serde_json::from_str(&rules_content) {
        Ok(v) => v,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot parse {ARCH_RULES_FILE}: {e}"
            ))]);
        }
    };

    let limits = match rules.get("module_limits") {
        Some(v) => v,
        None => return VerifyOutcome::pass(),
    };

    let max_lines = limits.get("max_lines").and_then(|v| v.as_u64()).unwrap_or(700) as usize;
    let warn_lines = limits.get("warn_lines").and_then(|v| v.as_u64()).unwrap_or(400) as usize;
    let excludes: Vec<&str> = limits
        .get("exclude")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut findings = Vec::new();
    let mut file_sizes = Vec::new();
    collect_rs_files(root, root, &excludes, &mut findings, &mut file_sizes);

    for (rel_path, line_count) in file_sizes {
        if line_count > max_lines {
            findings.push(VerifyFinding::error(format!(
                "{rel_path}: {line_count} lines (max {max_lines})"
            )));
        } else if line_count > warn_lines {
            findings.push(VerifyFinding::warning(format!(
                "{rel_path}: {line_count} lines (warn threshold {warn_lines})"
            )));
        }
    }

    VerifyOutcome::from_findings(findings)
}

/// Count production lines in a Rust source file, excluding `#[cfg(test)]` mod blocks.
///
/// Parses the source with `syn` and handles two exclusion patterns:
///
/// 1. **File-level `#![cfg(test)]` inner attribute** — the entire file is test-only;
///    returns 0 so the file is never flagged regardless of size.
/// 2. **`#[cfg(test)] mod` blocks** — collects the line ranges of any inline or
///    file-backed `mod` item (at any nesting depth) that carries `#[cfg(test)]`. All
///    lines that fall within those ranges are excluded from the count.
///
/// The closing-brace line of an inline block is included in the excluded range. In
/// rustfmt-formatted code (required by `cargo make fmt`) the closing `}` always
/// occupies its own line, so no production tokens share that line.
///
/// If parsing fails (e.g. the file contains syntax errors), the function falls back
/// to counting every line in the file. This is conservative: test lines are counted
/// too, which can only make the module-size check stricter, never looser.
fn count_production_lines(content: &str) -> usize {
    let total_lines = content.lines().count();

    let file = match syn::parse_file(content) {
        Ok(f) => f,
        Err(_) => return total_lines,
    };

    // If the file starts with `#![cfg(test)]`, the entire file is test-only.
    if has_cfg_test_attr(&file.attrs) {
        return 0;
    }

    // Collect 1-based [start, end] line ranges of cfg(test) mod blocks at any depth.
    let mut collector = CfgTestModCollector { ranges: Vec::new() };
    syn::visit::visit_file(&mut collector, &file);
    let test_ranges = collector.ranges;

    if test_ranges.is_empty() {
        return total_lines;
    }

    // Count lines that are NOT inside any test range (lines are 1-based).
    content
        .lines()
        .enumerate()
        .filter(|(idx, _)| {
            let line_no = idx + 1; // 1-based
            !test_ranges.iter().any(|(s, e)| line_no >= *s && line_no <= *e)
        })
        .count()
}

/// Visitor that collects the line ranges of `#[cfg(test)] mod` declarations.
///
/// Two forms are handled:
///
/// - **Inline block** (`mod foo { ... }`): the range covers from the first attribute
///   line through the closing brace line (both inclusive, 1-based). The closing brace
///   always occupies its own line in rustfmt-formatted code (required by
///   `cargo make fmt`), so no production tokens share that line.
///
/// - **File-backed declaration** (`mod foo;`): the range covers only the attribute
///   and `mod foo;` lines. The content of the referenced file is excluded separately:
///   files matching `*_tests.rs` / `tests/` are skipped by the path filter, and
///   module files with file-level or parent `#[cfg(test)]` gating are skipped by
///   `is_test_only_module_file`.
struct CfgTestModCollector {
    ranges: Vec<(usize, usize)>,
}

impl<'ast> syn::visit::Visit<'ast> for CfgTestModCollector {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if has_cfg_test_attr(&node.attrs) {
            // First attribute line (or mod keyword line if no attributes).
            let start = node
                .attrs
                .first()
                .map(|a| a.pound_token.spans[0].start().line)
                .unwrap_or_else(|| node.mod_token.span.start().line);

            if let Some((brace, _)) = &node.content {
                // Inline block: exclude from first attribute line through closing brace.
                let end = brace.span.close().start().line;
                self.ranges.push((start, end));
                // Do NOT recurse into this mod's items: the entire range is excluded.
                return;
            } else {
                // File-backed declaration: exclude only the attribute + mod lines.
                // `semi` is `Some` for `mod foo;` and `None` should not occur when
                // `content` is also `None`, but fall back to the mod keyword line.
                let end =
                    node.semi.map_or(node.mod_token.span.start().line, |s| s.spans[0].start().line);
                self.ranges.push((start, end));
                return;
            }
        }
        // Recurse into non-test mods so we catch nested cfg(test) blocks.
        syn::visit::visit_item_mod(self, node);
    }
}

/// Return `true` if the file at `path` is a test-only module that should be excluded
/// from production line counting.
///
/// A file is considered test-only when any of the following holds:
/// - Its content opens with `#![cfg(test)]` (explicit test-only file marker).
/// - Its parent module file declares the file-backed module with `#[cfg(test)]`,
///   for example `#[cfg(test)] mod fixtures;`.
///
/// If reading or parsing any file fails the function returns `false` (conservative:
/// the file is treated as production code and counted normally).
fn is_test_only_module_file(root: &Path, path: &Path) -> bool {
    // Check file-level #![cfg(test)] inner attribute.
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(file) = syn::parse_file(&content) {
            if has_cfg_test_attr(&file.attrs) {
                return true;
            }
        }
    }

    for (candidate, module_path) in file_backed_module_source_probes(root, path) {
        if file_declares_cfg_test_module_path(&candidate, &module_path) {
            return true;
        }
    }

    false
}

fn file_backed_module_source_probes(root: &Path, path: &Path) -> Vec<(PathBuf, Vec<String>)> {
    let mut probes = Vec::new();
    let Some(mut base_dir) = path.parent().map(Path::to_path_buf) else {
        return probes;
    };

    loop {
        for source in parent_module_file_candidates(root, &base_dir) {
            if source.as_path() == path {
                continue;
            }
            if let Some(module_path) = module_path_for_file_from_base(&base_dir, path) {
                probes.push((source, module_path));
            }
        }

        if base_dir == root {
            break;
        }
        let Some(parent) = base_dir.parent() else {
            break;
        };
        base_dir = parent.to_path_buf();
    }

    probes
}

fn parent_module_file_candidates(root: &Path, parent_dir: &Path) -> Vec<PathBuf> {
    let mut candidates =
        vec![parent_dir.join("mod.rs"), parent_dir.join("lib.rs"), parent_dir.join("main.rs")];

    if let Some(grandparent) = parent_dir.parent() {
        if let Some(dir_name) = parent_dir.file_name() {
            candidates.push(grandparent.join(format!("{}.rs", dir_name.to_string_lossy())));
        }
    }

    candidates.into_iter().filter(|candidate| candidate.strip_prefix(root).is_ok()).collect()
}

fn module_path_for_file_from_base(base_dir: &Path, path: &Path) -> Option<Vec<String>> {
    let file_name = path.file_name()?.to_string_lossy();
    let module_path = if file_name == "mod.rs" {
        let module_dir = path.parent()?;
        normal_component_strings(module_dir.strip_prefix(base_dir).ok()?)?
    } else {
        let parent_dir = path.parent()?;
        let mut components = normal_component_strings(parent_dir.strip_prefix(base_dir).ok()?)?;
        components.push(path.file_stem()?.to_string_lossy().into_owned());
        components
    };

    if module_path.is_empty() { None } else { Some(module_path) }
}

fn normal_component_strings(path: &Path) -> Option<Vec<String>> {
    let mut names = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(name) = component else {
            return None;
        };
        names.push(name.to_string_lossy().into_owned());
    }
    Some(names)
}

fn file_declares_cfg_test_module_path(path: &Path, module_path: &[String]) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return false,
    };

    let file = match syn::parse_file(&content) {
        Ok(file) => file,
        Err(_) => return false,
    };

    items_declare_cfg_test_module_path(&file.items, module_path, false)
}

fn items_declare_cfg_test_module_path(
    items: &[syn::Item],
    module_path: &[String],
    inherited_cfg_test: bool,
) -> bool {
    let Some((head, tail)) = module_path.split_first() else {
        return false;
    };

    items.iter().any(|item| {
        if let syn::Item::Mod(module) = item {
            if module.ident != head.as_str() {
                return false;
            }
            let cfg_test = inherited_cfg_test || has_cfg_test_attr(&module.attrs);
            if tail.is_empty() {
                return module.content.is_none() && cfg_test;
            }
            if cfg_test && module.content.is_none() {
                return true;
            }
            if let Some((_, nested_items)) = &module.content {
                return items_declare_cfg_test_module_path(nested_items, tail, cfg_test);
            }
        }
        false
    })
}

fn collect_rs_files(
    root: &Path,
    dir: &Path,
    excludes: &[&str],
    findings: &mut Vec<VerifyFinding>,
    results: &mut Vec<(String, usize)>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            let rel = dir.strip_prefix(root).unwrap_or(dir);
            findings.push(VerifyFinding::error(format!(
                "{}: cannot read directory: {e}",
                rel.to_string_lossy()
            )));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                let rel = dir.strip_prefix(root).unwrap_or(dir);
                findings.push(VerifyFinding::error(format!(
                    "{}: cannot read entry: {e}",
                    rel.to_string_lossy()
                )));
                continue;
            }
        };
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rel_str = rel.to_string_lossy();

        if excludes.iter().any(|exc| {
            let normalized = exc.trim_end_matches('/');
            rel_str.starts_with(&format!("{normalized}/")) || *rel_str == *normalized
        }) {
            continue;
        }

        if path.is_dir() {
            if is_integration_tests_dir(root, &path) {
                continue;
            }
            collect_rs_files(root, &path, excludes, findings, results);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let file_name = path.file_name().map(|n| n.to_string_lossy());
            if file_name.as_deref().is_some_and(|n| n.ends_with("_tests.rs")) {
                continue;
            }
            if is_test_only_module_file(root, &path) {
                continue;
            }

            let line_count = match std::fs::read_to_string(&path) {
                Ok(content) => count_production_lines(&content),
                Err(e) => {
                    findings
                        .push(VerifyFinding::error(format!("{rel_str}: cannot read file: {e}")));
                    continue;
                }
            };
            results.push((rel_str.into_owned(), line_count));
        }
    }
}

fn is_integration_tests_dir(root: &Path, path: &Path) -> bool {
    if path.file_name().and_then(|n| n.to_str()) != Some("tests") {
        return false;
    }
    if path.parent() == Some(root) {
        return true;
    }
    path.parent().is_some_and(|parent| parent.join("Cargo.toml").is_file())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn setup_rules(root: &Path, max: usize, warn: usize, excludes: &[&str]) {
        let rules = serde_json::json!({
            "version": 2,
            "module_limits": {
                "max_lines": max,
                "warn_lines": warn,
                "exclude": excludes
            }
        });
        std::fs::write(root.join(ARCH_RULES_FILE), rules.to_string()).unwrap();
    }

    fn write_rs_file(root: &Path, rel: &str, lines: usize) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let content: String = (0..lines).map(|i| format!("// line {i}\n")).collect();
        std::fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_module_size_passes_for_small_files() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        write_rs_file(tmp.path(), "src/small.rs", 100);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_module_size_warns_for_files_above_warn_threshold() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        write_rs_file(tmp.path(), "src/medium.rs", 450);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok()); // warnings don't fail
        assert_eq!(outcome.findings().len(), 1);
        assert!(outcome.findings()[0].to_string().contains("450 lines"));
    }

    #[test]
    fn test_module_size_warns_for_files_above_max_threshold() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        write_rs_file(tmp.path(), "src/large.rs", 750);
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(outcome.findings()[0].to_string().contains("750 lines"));
    }

    #[test]
    fn test_module_size_excludes_vendor_directory() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &["vendor/"]);
        write_rs_file(tmp.path(), "vendor/big-crate/src/lib.rs", 2000);
        write_rs_file(tmp.path(), "src/small.rs", 50);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_module_size_missing_rules_file_errors() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_module_size_no_module_limits_section_passes() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(ARCH_RULES_FILE), r#"{"version": 2}"#).unwrap();
        write_rs_file(tmp.path(), "src/huge.rs", 9999);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_module_size_skips_tests_rs_files() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        write_rs_file(tmp.path(), "src/user_tests.rs", 750);
        write_rs_file(tmp.path(), "src/user.rs", 50);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_module_size_skips_tests_directory() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        write_rs_file(tmp.path(), "tests/integration.rs", 750);
        write_rs_file(tmp.path(), "src/small.rs", 50);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_module_size_skips_package_integration_tests_directory() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 20, 10, &[]);
        std::fs::create_dir_all(tmp.path().join("crate")).unwrap();
        std::fs::write(tmp.path().join("crate/Cargo.toml"), "[package]\nname = \"crate\"\n")
            .unwrap();
        write_rs_file(tmp.path(), "crate/tests/integration.rs", 30);
        write_rs_file(tmp.path(), "src/small.rs", 5);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_module_size_counts_src_tests_directory_if_not_cfg_gated() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 20, 10, &[]);
        write_rs_file(tmp.path(), "src/tests/mod.rs", 30);
        write_rs_file(tmp.path(), "src/small.rs", 5);
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "ungated src/tests modules must be counted");
    }

    #[test]
    fn test_module_size_skips_tests_rs_file_declared_with_cfg_test() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        // `tests.rs` is test-only when the parent mod.rs declares it via #[cfg(test)] mod tests.
        let mod_dir = tmp.path().join("src/my_module");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(mod_dir.join("mod.rs"), "#[cfg(test)]\nmod tests;\n").unwrap();
        write_rs_file(tmp.path(), "src/my_module/tests.rs", 750);
        write_rs_file(tmp.path(), "src/small.rs", 50);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "unexpected findings: {:?}", outcome.findings());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_module_size_skips_file_backed_cfg_test_module() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("lib.rs"), "#[cfg(test)]\nmod fixtures;\n").unwrap();
        write_rs_file(tmp.path(), "src/fixtures.rs", 750);
        write_rs_file(tmp.path(), "src/small.rs", 50);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "unexpected findings: {:?}", outcome.findings());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_module_size_skips_file_backed_cfg_test_mod_rs() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("lib.rs"), "#[cfg(test)]\nmod fixtures;\n").unwrap();
        write_rs_file(tmp.path(), "src/fixtures/mod.rs", 750);
        write_rs_file(tmp.path(), "src/small.rs", 50);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "unexpected findings: {:?}", outcome.findings());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_module_size_skips_nested_inline_file_backed_cfg_test_module() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("lib.rs"),
            "mod outer {\n    #[cfg(test)]\n    mod fixtures;\n}\n",
        )
        .unwrap();
        write_rs_file(tmp.path(), "src/outer/fixtures.rs", 750);
        write_rs_file(tmp.path(), "src/small.rs", 50);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "unexpected findings: {:?}", outcome.findings());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_module_size_counts_tests_rs_file_if_not_cfg_gated() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        // `tests.rs` is NOT excluded if the parent module declares it without #[cfg(test)].
        let mod_dir = tmp.path().join("src/my_module");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(mod_dir.join("mod.rs"), "mod tests;\n").unwrap();
        write_rs_file(tmp.path(), "src/my_module/tests.rs", 750);
        write_rs_file(tmp.path(), "src/small.rs", 50);
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for production tests.rs");
    }

    #[test]
    fn test_count_production_lines_skips_cfg_test_file_attribute() {
        // A file with `#![cfg(test)]` as a file-level inner attribute is entirely
        // test-only. `count_production_lines` must return 0.
        let mut content = String::from("#![cfg(test)]\n");
        for i in 0..500 {
            content.push_str(&format!("// test line {i}\n"));
        }
        let prod_lines = count_production_lines(&content);
        assert_eq!(
            prod_lines, 0,
            "expected 0 production lines for cfg(test) file, got {prod_lines}"
        );
    }

    #[test]
    fn test_count_production_lines_counts_cfg_not_test_file_attribute() {
        let content = "#![cfg(not(test))]\nfn production_only() {}\n";
        let prod_lines = count_production_lines(content);
        assert_eq!(
            prod_lines,
            content.lines().count(),
            "cfg(not(test)) files are production code and must be counted"
        );
    }

    #[test]
    fn test_count_production_lines_counts_mixed_cfg_file_attribute() {
        let content =
            "#![cfg(any(test, feature = \"test-helpers\"))]\nfn feature_enabled_helper() {}\n";
        let prod_lines = count_production_lines(content);
        assert_eq!(
            prod_lines,
            content.lines().count(),
            "mixed test/feature cfg files can compile in production and must be counted"
        );
    }

    #[test]
    fn test_count_production_lines_counts_mixed_cfg_module_block() {
        let content = concat!(
            "fn before() {}\n",
            "#[cfg(any(test, feature = \"test-helpers\"))]\n",
            "mod helpers {\n",
            "    fn feature_enabled_helper() {}\n",
            "}\n",
        );
        let prod_lines = count_production_lines(content);
        assert_eq!(
            prod_lines,
            content.lines().count(),
            "mixed test/feature cfg modules can compile in production and must be counted"
        );
    }

    #[test]
    fn test_module_size_counts_file_backed_cfg_not_test_module() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 20, 10, &[]);
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("lib.rs"), "#[cfg(not(test))]\nmod fixtures;\n").unwrap();
        write_rs_file(tmp.path(), "src/fixtures.rs", 30);

        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "cfg(not(test)) module files must be counted");
    }

    #[test]
    fn test_module_size_counts_file_backed_mixed_cfg_module() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 20, 10, &[]);
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("lib.rs"),
            "#[cfg(any(test, feature = \"test-helpers\"))]\nmod fixtures;\n",
        )
        .unwrap();
        write_rs_file(tmp.path(), "src/fixtures.rs", 30);

        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "mixed test/feature module files must be counted");
    }

    #[test]
    fn test_count_production_lines_excludes_cfg_test_block() {
        let mut content = String::new();
        for i in 0..500 {
            content.push_str(&format!("// prod line {i}\n"));
        }
        content.push_str("#[cfg(test)]\n");
        content.push_str("mod tests {\n");
        for i in 0..298 {
            content.push_str(&format!("    // test line {i}\n"));
        }
        content.push_str("}\n");

        let prod_lines = count_production_lines(&content);
        assert_eq!(prod_lines, 500, "expected 500 production lines, got {prod_lines}");

        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        let path = tmp.path().join("src/mixed.rs");
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(&path, &content).unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }
}
