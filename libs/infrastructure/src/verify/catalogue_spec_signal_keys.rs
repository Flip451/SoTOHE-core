//! Raw catalogue keys and resolved tombstone identities for catalogue-spec checks.

/// A catalogue entry's raw JSON section key and its resolved display identity.
///
/// Delete tombstones keep both forms: hashes and signal coverage address the
/// raw section key, while diagnostics and identity-aware comparisons use the
/// fully-qualified `name`.
pub(super) struct CatalogueEntryKey {
    pub(super) section: String,
    pub(super) entry_key: String,
    pub(super) name: String,
}

impl CatalogueEntryKey {
    pub(super) fn new(
        section: impl Into<String>,
        entry_key: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self { section: section.into(), entry_key: entry_key.into(), name: name.into() }
    }
}

/// Return the resolved identity of a type/trait tombstone while leaving its
/// raw section key available for JSON lookup and entry-hash computation.
pub(super) fn normalized_delete_identity(
    raw_catalogue: &serde_json::Value,
    section: &str,
    entry_key: &str,
    crate_name: &str,
) -> String {
    let segments: Vec<&str> = entry_key.split("::").collect();
    if segments.len() > 1 && segments.first().copied() == Some(crate_name) {
        return entry_key.to_owned();
    }

    let item_name = segments.last().copied().unwrap_or(entry_key);
    let module_path = raw_catalogue
        .get(section)
        .and_then(serde_json::Value::as_object)
        .and_then(|entries| entries.get(entry_key))
        .and_then(serde_json::Value::as_object)
        .and_then(|entry| entry.get("module_path"))
        .and_then(serde_json::Value::as_str)
        .filter(|path| !path.is_empty());

    let mut path = vec![crate_name.to_owned()];
    if let Some(module_path) = module_path {
        path.extend(module_path.split("::").map(str::to_owned));
    } else {
        path.extend(
            segments
                .iter()
                .take(segments.len().saturating_sub(1))
                .map(|segment| (*segment).to_owned()),
        );
    }
    path.push(item_name.to_owned());
    path.join("::")
}

/// Resolve the JSON section key for a decoded catalogue entry.
pub(super) fn raw_catalogue_entry_key(
    raw_catalogue: &serde_json::Value,
    section: &str,
    resolved_key: &str,
    is_delete: bool,
) -> Result<String, String> {
    let section_object = raw_catalogue
        .get(section)
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| format!("catalogue section '{section}' is missing or not an object"))?;

    if section_object.contains_key(resolved_key) {
        return Ok(resolved_key.to_owned());
    }

    if is_delete
        && let Some(short_key) = resolved_key.rsplit("::").next()
        && short_key != resolved_key
        && section_object.contains_key(short_key)
    {
        return Ok(short_key.to_owned());
    }

    Err(format!("catalogue entry '{resolved_key}' not found in section '{section}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalized_delete_identity_repeated_item_segment_preserves_prefix() {
        let catalogue = serde_json::json!({"types": {"Thing::nested::Thing": {}}});

        assert_eq!(
            normalized_delete_identity(&catalogue, "types", "Thing::nested::Thing", "crate"),
            "crate::Thing::nested::Thing"
        );
    }

    #[test]
    fn test_normalized_delete_identity_bare_crate_name_is_not_treated_as_qualified() {
        let catalogue = serde_json::json!({"types": {"crate": {}}});

        assert_eq!(
            normalized_delete_identity(&catalogue, "types", "crate", "crate"),
            "crate::crate"
        );
    }
}
