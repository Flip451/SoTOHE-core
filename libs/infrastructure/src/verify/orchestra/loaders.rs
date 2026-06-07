//! JSON loading helpers for orchestra verification.

use std::collections::BTreeSet;
use std::path::Path;

use super::constants::{PERMISSION_EXTENSIONS_PATH, SETTINGS_PATH};

/// Load `.claude/settings.json` from `root`.
///
/// # Errors
///
/// Returns `Err` with a descriptive message when the file is missing,
/// contains invalid JSON, or does not decode to a JSON object.
pub(crate) fn load_settings(root: &Path) -> Result<serde_json::Value, String> {
    let path = root.join(SETTINGS_PATH);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("Missing settings file {SETTINGS_PATH}: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid JSON in {SETTINGS_PATH}: {e}"))?;
    if !value.is_object() {
        return Err(format!("{SETTINGS_PATH} must decode to a JSON object"));
    }
    Ok(value)
}

/// Load `extra_allow` string array from `.claude/permission-extensions.json`.
///
/// # Errors
///
/// Returns `Err` with a descriptive message when the file exists but is
/// invalid JSON or has the wrong shape.
pub(crate) fn load_permission_extensions(root: &Path) -> Result<Vec<String>, String> {
    let path = root.join(PERMISSION_EXTENSIONS_PATH);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read {PERMISSION_EXTENSIONS_PATH}: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Invalid JSON in {PERMISSION_EXTENSIONS_PATH}: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| format!("{PERMISSION_EXTENSIONS_PATH} must decode to a JSON object"))?;
    let entries = match obj.get("extra_allow") {
        Some(v) => v.as_array().ok_or_else(|| {
            format!("{PERMISSION_EXTENSIONS_PATH} field 'extra_allow' must be an array of strings")
        })?,
        None => return Ok(Vec::new()), // Python: data.get("extra_allow", [])
    };
    let mut result = Vec::with_capacity(entries.len());
    for item in entries {
        let s = item.as_str().ok_or_else(|| {
            format!("{PERMISSION_EXTENSIONS_PATH} field 'extra_allow' entries must be strings")
        })?;
        result.push(s.to_owned());
    }
    Ok(result)
}

/// Extract all hook `command` strings from `settings["hooks"]`.
///
/// # Errors
///
/// Returns `Err` when the hooks field is missing or has an unexpected shape.
pub(crate) fn hook_commands(settings: &serde_json::Value) -> Result<Vec<String>, String> {
    let hooks = settings
        .get("hooks")
        .and_then(|v| v.as_object())
        .ok_or_else(|| format!("{SETTINGS_PATH} missing object field 'hooks'"))?;
    let mut commands = Vec::new();
    for (_event, bindings_val) in hooks {
        let bindings = bindings_val
            .as_array()
            .ok_or_else(|| "Each hooks event binding list must be an array".to_owned())?;
        for binding in bindings {
            let binding_obj = binding
                .as_object()
                .ok_or_else(|| "Each hooks event binding must be an object".to_owned())?;
            if let Some(hooks_val) = binding_obj.get("hooks") {
                let nested = hooks_val
                    .as_array()
                    .ok_or_else(|| "Each hooks binding must contain a hooks array".to_owned())?;
                for hook in nested {
                    let hook_obj = hook
                        .as_object()
                        .ok_or_else(|| "Each hook entry must be an object".to_owned())?;
                    if let Some(cmd) = hook_obj.get("command").and_then(|v| v.as_str()) {
                        commands.push(cmd.to_owned());
                    }
                }
            }
        }
    }
    Ok(commands)
}

/// Extract `settings["permissions"][key]` as a sorted set of strings.
///
/// # Errors
///
/// Returns `Err` when the field is missing or entries are not strings.
pub(crate) fn permission_set(
    settings: &serde_json::Value,
    key: &str,
) -> Result<BTreeSet<String>, String> {
    let permissions = settings
        .get("permissions")
        .and_then(|v| v.as_object())
        .ok_or_else(|| format!("{SETTINGS_PATH} missing object field 'permissions'"))?;
    let values = permissions
        .get(key)
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("{SETTINGS_PATH} permissions.{key} must be an array"))?;
    let mut set = BTreeSet::new();
    for item in values {
        let s = item
            .as_str()
            .ok_or_else(|| format!("{SETTINGS_PATH} permissions.{key} entries must be strings"))?;
        set.insert(s.to_owned());
    }
    Ok(set)
}
