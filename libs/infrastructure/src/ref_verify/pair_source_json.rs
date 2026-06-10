//! JSON parsing and prompt-template rendering helpers for the ref-verify adapter.

/// Render a prompt template, substituting `{{claim}}`, `{{evidence}}`, and `{{tier}}`
/// placeholders without rescanning already-inserted values.
pub(super) fn render_prompt_template(
    template: &str,
    claim: &str,
    evidence: &str,
    tier: &str,
) -> String {
    let mut out = String::with_capacity(template.len() + claim.len() + evidence.len());
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let candidate = &rest[start..];
        if let Some(next) = candidate.strip_prefix("{{claim}}") {
            out.push_str(claim);
            rest = next;
        } else if let Some(next) = candidate.strip_prefix("{{evidence}}") {
            out.push_str(evidence);
            rest = next;
        } else if let Some(next) = candidate.strip_prefix("{{tier}}") {
            out.push_str(tier);
            rest = next;
        } else {
            out.push_str("{{");
            rest = &candidate[2..];
        }
    }

    out.push_str(rest);
    out
}

/// Parse a verdict JSON object of type `T` from raw model output.
///
/// The response must be exactly one JSON object after trimming leading/trailing
/// whitespace. Prose, examples, and trailing brace blocks fail closed instead of
/// being searched for a parseable verdict somewhere in the output.
pub(super) fn extract_json_object_parsed<T: serde::de::DeserializeOwned>(
    raw: &str,
) -> Result<T, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("no JSON object found in output".to_owned());
    }

    serde_json::from_str::<T>(trimmed)
        .map_err(|e| format!("response must be exactly one verdict JSON object: {e}"))
}
