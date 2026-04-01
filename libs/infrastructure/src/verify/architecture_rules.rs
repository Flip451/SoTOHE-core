//! Verify architecture-rules.json is in sync with Cargo.toml and deny.toml.
//!
//! Rust port of `architecture_rules.verify_sync()` plus the workspace-member
//! cross-reference checks from `verify_architecture_docs.py`.

use std::path::Path;

use domain::verify::{Finding, VerifyOutcome};

/// Verify architecture rules synchronization.
///
/// Checks:
/// 1. `architecture-rules.json` matches `Cargo.toml` workspace members.
/// 2. `architecture-rules.json` matches `deny.toml` deny rules.
/// 3. Each workspace member path appears in `Cargo.toml` and `track/tech-stack.md`.
///
/// # Errors
///
/// Returns error findings for mismatches or missing files.
pub fn verify(root: &Path) -> VerifyOutcome {
    let mut outcome = VerifyOutcome::pass();

    let rules = match load_rules(root) {
        Ok(r) => r,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Failed to load architecture rules: {e}"
            ))]);
        }
    };

    let layers = match parse_layers(&rules) {
        Ok(l) => l,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Failed to parse architecture rules: {e}"
            ))]);
        }
    };

    // Verify Cargo.toml workspace members match.
    outcome.merge(verify_cargo_members(root, &layers));

    // Verify deny.toml deny rules match.
    outcome.merge(verify_deny_rules(root, &rules, &layers));

    // Verify each workspace member is referenced in Cargo.toml and tech-stack.md.
    outcome.merge(verify_member_references(root, &layers));

    outcome
}

#[derive(Debug, Clone)]
struct LayerRule {
    crate_name: String,
    path: String,
    may_depend_on: Vec<String>,
    deny_reason: String,
}

fn load_rules(root: &Path) -> Result<serde_json::Value, String> {
    let path = root.join("architecture-rules.json");
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&content).map_err(|e| format!("Invalid JSON in {}: {e}", path.display()))
}

fn parse_layers(rules: &serde_json::Value) -> Result<Vec<LayerRule>, String> {
    let layers = rules
        .get("layers")
        .and_then(|v| v.as_array())
        .ok_or("architecture rules must define a non-empty 'layers' array")?;

    if layers.is_empty() {
        return Err("architecture rules 'layers' array is empty".to_owned());
    }

    let mut result = Vec::new();
    let mut seen_crates = std::collections::BTreeSet::new();
    let mut seen_paths = std::collections::BTreeSet::new();

    for layer in layers {
        let crate_name = layer
            .get("crate")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or("layer 'crate' must be a non-empty string")?
            .to_owned();
        let path = layer
            .get("path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or(format!("layer '{crate_name}' must define a non-empty 'path'"))?
            .to_owned();
        let may_depend_on_arr = match layer.get("may_depend_on") {
            Some(v) => {
                v.as_array().ok_or(format!("layer '{crate_name}' has invalid 'may_depend_on'"))?
            }
            None => &Vec::new(), // Python defaults missing may_depend_on to []
        };
        let mut may_depend_on = Vec::with_capacity(may_depend_on_arr.len());
        for item in may_depend_on_arr {
            let s = item
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or(format!("layer '{crate_name}' has invalid 'may_depend_on' entries"))?;
            may_depend_on.push(s.to_owned());
        }
        let deny_reason = match layer.get("deny_reason") {
            Some(v) => v
                .as_str()
                .ok_or(format!(
                    "layer '{crate_name}' has invalid 'deny_reason' (must be a string)"
                ))?
                .to_owned(),
            None => String::new(),
        };

        if !seen_crates.insert(crate_name.clone()) {
            return Err(format!("duplicate crate in architecture rules: {crate_name}"));
        }
        if !seen_paths.insert(path.clone()) {
            return Err(format!("duplicate path in architecture rules: {path}"));
        }

        result.push(LayerRule { crate_name, path, may_depend_on, deny_reason });
    }

    // Validate dependency references.
    let known_crates: std::collections::BTreeSet<&str> =
        result.iter().map(|l| l.crate_name.as_str()).collect();
    for layer in &result {
        for dep in &layer.may_depend_on {
            if !known_crates.contains(dep.as_str()) {
                return Err(format!(
                    "layer '{}' references unknown dependency: {dep}",
                    layer.crate_name
                ));
            }
            if dep == &layer.crate_name {
                return Err(format!("layer '{}' cannot depend on itself", layer.crate_name));
            }
        }
    }

    Ok(result)
}

fn verify_cargo_members(root: &Path, layers: &[LayerRule]) -> VerifyOutcome {
    let cargo_path = root.join("Cargo.toml");
    let content = match std::fs::read_to_string(&cargo_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Failed to read Cargo.toml: {e}"
            ))]);
        }
    };

    let cargo_data: toml::Table = match toml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Failed to parse Cargo.toml: {e}"
            ))]);
        }
    };

    let members_arr = cargo_data
        .get("workspace")
        .and_then(|w| w.as_table())
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array());
    let actual_members: Vec<String> = match members_arr {
        Some(arr) => {
            let mut members = Vec::with_capacity(arr.len());
            for (i, v) in arr.iter().enumerate() {
                match v.as_str() {
                    Some(s) if !s.is_empty() => members.push(s.to_owned()),
                    _ => {
                        return VerifyOutcome::from_findings(vec![Finding::error(format!(
                            "Cargo.toml workspace.members[{i}] must be a non-empty string"
                        ))]);
                    }
                }
            }
            members
        }
        None => Vec::new(),
    };

    let expected_members: Vec<String> = layers.iter().map(|l| l.path.clone()).collect();

    let mut actual_sorted = actual_members.clone();
    actual_sorted.sort();
    let mut expected_sorted = expected_members.clone();
    expected_sorted.sort();

    if actual_sorted != expected_sorted {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "Cargo.toml workspace members mismatch: expected {expected_members:?}, got {actual_members:?}"
        ))]);
    }

    VerifyOutcome::pass()
}

fn verify_deny_rules(
    root: &Path,
    rules: &serde_json::Value,
    layers: &[LayerRule],
) -> VerifyOutcome {
    let deny_path = root.join("deny.toml");
    let content = match std::fs::read_to_string(&deny_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Failed to read deny.toml: {e}"
            ))]);
        }
    };

    let deny_data: toml::Table = match toml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Failed to parse deny.toml: {e}"
            ))]);
        }
    };

    let actual_deny = match parse_deny_entries(&toml::Value::Table(deny_data)) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Failed to parse deny.toml: {e}"
            ))]);
        }
    };
    let expected_deny = match expected_deny_rules(rules, layers) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Failed to compute expected deny rules: {e}"
            ))]);
        }
    };

    if actual_deny != expected_deny {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "deny.toml layer policy mismatch: expected {expected_deny:?}, got {actual_deny:?}"
        ))]);
    }

    VerifyOutcome::pass()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DenyEntry {
    crate_name: String,
    wrappers: Vec<String>,
    reason: String,
}

fn parse_deny_entries(deny_data: &toml::Value) -> Result<Vec<DenyEntry>, String> {
    // Locate deny entries: top-level deny first, then [bans].deny (Python parity).
    // Python only inspects [bans] when top-level deny is absent.
    let deny_entries_val = if let Some(top_level) = deny_data.get("deny") {
        Some(top_level)
    } else if let Some(bans) = deny_data.get("bans") {
        if !bans.is_table() {
            return Err("deny.toml [bans] must be a TOML table".to_owned());
        }
        bans.get("deny")
    } else {
        None
    };

    let Some(deny_val) = deny_entries_val else {
        return Ok(Vec::new());
    };
    let entries = deny_val.as_array().ok_or("deny.toml deny must be an array of inline tables")?;

    let mut result = Vec::with_capacity(entries.len());
    for (i, entry) in entries.iter().enumerate() {
        let crate_name = entry
            .get("crate")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or(format!(
                "deny.toml deny entry[{i}] is missing required non-empty string field 'crate'"
            ))?
            .to_owned();
        let wrappers_arr = entry
            .get("wrappers")
            .and_then(|v| v.as_array())
            .ok_or(format!("deny.toml deny entry '{crate_name}' has invalid 'wrappers'"))?;
        let mut wrappers = Vec::with_capacity(wrappers_arr.len());
        for w in wrappers_arr {
            let s = w.as_str().filter(|s| !s.is_empty()).ok_or(format!(
                "deny.toml deny entry '{crate_name}' has empty or non-string wrapper"
            ))?;
            wrappers.push(s.to_owned());
        }
        wrappers.sort();
        let reason = entry
            .get("reason")
            .and_then(|r| r.as_str())
            .ok_or(format!("deny.toml deny entry '{crate_name}' has invalid 'reason'"))?
            .to_owned();
        result.push(DenyEntry { crate_name, wrappers, reason });
    }
    result.sort_by(|a, b| a.crate_name.cmp(&b.crate_name));
    Ok(result)
}

fn expected_deny_rules(
    _rules: &serde_json::Value,
    layers: &[LayerRule],
) -> Result<Vec<DenyEntry>, String> {
    // Build dependents map: for each crate, which crates depend on it?
    let mut dependents: std::collections::BTreeMap<String, Vec<String>> =
        layers.iter().map(|l| (l.crate_name.clone(), Vec::new())).collect();

    for layer in layers {
        for dep in &layer.may_depend_on {
            if let Some(deps) = dependents.get_mut(dep) {
                deps.push(layer.crate_name.clone());
            }
        }
    }

    let mut result = Vec::new();
    for layer in layers {
        if let Some(wrappers) = dependents.get(&layer.crate_name) {
            if !wrappers.is_empty() {
                if layer.deny_reason.trim().is_empty() {
                    return Err(format!(
                        "layer '{}' must define a non-empty 'deny_reason' when it has dependents",
                        layer.crate_name
                    ));
                }
                let mut sorted_wrappers = wrappers.clone();
                sorted_wrappers.sort();
                result.push(DenyEntry {
                    crate_name: layer.crate_name.clone(),
                    wrappers: sorted_wrappers,
                    reason: layer.deny_reason.clone(),
                });
            }
        }
    }
    result.sort_by(|a, b| a.crate_name.cmp(&b.crate_name));
    Ok(result)
}

fn verify_member_references(root: &Path, layers: &[LayerRule]) -> VerifyOutcome {
    let mut outcome = VerifyOutcome::pass();

    let cargo_content = std::fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    let tech_stack_content =
        std::fs::read_to_string(root.join("track").join("tech-stack.md")).unwrap_or_default();

    for layer in layers {
        let member = &layer.path;

        // Check Cargo.toml contains the member path (quoted).
        let quoted = format!("\"{member}\"");
        if !cargo_content.contains(&quoted) {
            outcome
                .add(Finding::error(format!("Missing in Cargo.toml: workspace member {member}")));
        }

        // Check tech-stack.md contains the member path.
        if !tech_stack_content.contains(member.as_str()) {
            outcome.add(Finding::error(format!(
                "Missing in track/tech-stack.md: tech-stack workspace map {member}"
            )));
        }
    }

    outcome
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn write_file(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    fn setup_rules(root: &Path) {
        write_file(
            root,
            "architecture-rules.json",
            r#"{
  "layers": [
    {"crate": "domain", "path": "libs/domain", "may_depend_on": [], "deny_reason": "controlled"},
    {"crate": "cli", "path": "apps/cli", "may_depend_on": ["domain"], "deny_reason": ""}
  ]
}"#,
        );
    }

    fn setup_cargo(root: &Path) {
        write_file(
            root,
            "Cargo.toml",
            r#"[workspace]
members = ["libs/domain", "apps/cli"]
"#,
        );
    }

    fn setup_deny(root: &Path) {
        write_file(
            root,
            "deny.toml",
            r#"[bans]
deny = [
  { crate = "domain", wrappers = ["cli"], reason = "controlled" },
]
"#,
        );
    }

    fn setup_tech_stack(root: &Path) {
        write_file(root, "track/tech-stack.md", "# Tech Stack\n- libs/domain\n- apps/cli\n");
    }

    #[test]
    fn test_synced_rules_pass() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path());
        setup_cargo(tmp.path());
        setup_deny(tmp.path());
        setup_tech_stack(tmp.path());
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "findings: {:?}", outcome.findings());
    }

    #[test]
    fn test_cargo_member_mismatch_fails() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path());
        write_file(tmp.path(), "Cargo.toml", "[workspace]\nmembers = [\"libs/domain\"]\n");
        setup_deny(tmp.path());
        setup_tech_stack(tmp.path());
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_missing_rules_file_fails() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_deny_mismatch_fails() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path());
        setup_cargo(tmp.path());
        write_file(tmp.path(), "deny.toml", "[bans]\ndeny = []\n");
        setup_tech_stack(tmp.path());
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_missing_tech_stack_reference_fails() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path());
        setup_cargo(tmp.path());
        setup_deny(tmp.path());
        write_file(tmp.path(), "track/tech-stack.md", "# Tech Stack\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }
}
