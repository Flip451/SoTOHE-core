//! Stateless free function decoding an ADR file's YAML front-matter into the
//! domain [`AdrFrontMatter`] aggregate.
//!
//! The parser performs a **two-pass** decode:
//!
//! 1. **Raw `serde_yaml::Value` pass** — inspect each `decisions[]` mapping
//!    for **explicit key presence** of `implemented_in` / `superseded_by`.
//!    `serde` cannot distinguish `key: null` from `key absent` for
//!    `Option<T>` fields, so the typed DTO check alone would silently
//!    accept e.g. `implemented_in: null` on a `proposed` decision.
//! 2. **Typed DTO pass** — deserialize into the private `AdrFrontMatterDto`
//!    / `AdrDecisionDto` (with `deny_unknown_fields`), then dispatch each
//!    decision on its `status` string to the correct domain typestate
//!    variant of [`AdrDecisionEntry`].
//!
//! Fail-closed (CN-04): an ADR file with no front-matter at all surfaces as
//! [`AdrFrontMatterCodecError::MissingAdrId`] rather than a silent default.

use domain::{
    AcceptedDecision, AdrDecisionCommon, AdrDecisionEntry, AdrFrontMatter, DeprecatedDecision,
    ImplementedDecision, ProposedDecision, SupersededDecision,
};

use super::dto::{AdrDecisionDto, AdrFrontMatterDto};
use super::error::AdrFrontMatterCodecError;
use crate::verify::frontmatter::parse_yaml_frontmatter;

/// Decode an ADR file's full text into a domain [`AdrFrontMatter`] aggregate.
///
/// `content` is the raw markdown including the leading `---` YAML
/// front-matter block. The body after the closing `---` is ignored.
///
/// # Errors
///
/// - [`AdrFrontMatterCodecError::MissingAdrId`] — no front-matter block, or
///   `adr_id` parsed as an empty string.
/// - [`AdrFrontMatterCodecError::YamlParse`] — `serde_yaml` raw-parse
///   failure (syntax error, missing required field, type mismatch, schema
///   key rejected by `deny_unknown_fields`).
/// - [`AdrFrontMatterCodecError::InvalidDecisionField`] — per-decision
///   schema invariant violated (unknown status, forbidden / missing
///   typestate-specific field, empty domain identifier).
pub fn parse_adr_frontmatter(content: &str) -> Result<AdrFrontMatter, AdrFrontMatterCodecError> {
    let frontmatter =
        parse_yaml_frontmatter(content).ok_or(AdrFrontMatterCodecError::MissingAdrId)?;
    let yaml_text = &frontmatter.frontmatter;

    let raw: serde_yaml::Value = serde_yaml::from_str(yaml_text)?;
    check_decision_key_presence(&raw)?;

    let dto: AdrFrontMatterDto = serde_yaml::from_str(yaml_text)?;
    let AdrFrontMatterDto { adr_id, decisions } = dto;

    check_decision_ids_unique(&decisions)?;

    let entries =
        decisions.into_iter().map(decision_dto_to_entry).collect::<Result<Vec<_>, _>>()?;

    AdrFrontMatter::new(adr_id, entries).map_err(|_| AdrFrontMatterCodecError::MissingAdrId)
}

/// Reject duplicate decision IDs within a single ADR.
///
/// `knowledge/conventions/adr.md` constrains `decisions[].id` to be unique
/// within one ADR; without this check, references such as
/// `superseded_by: <adr>.md#D1` are ambiguous and lifecycle traceability
/// breaks while `verify adr-signals` still passes.
fn check_decision_ids_unique(decisions: &[AdrDecisionDto]) -> Result<(), AdrFrontMatterCodecError> {
    let mut seen = std::collections::HashSet::with_capacity(decisions.len());
    for decision in decisions {
        if !seen.insert(decision.id.as_str()) {
            return Err(AdrFrontMatterCodecError::InvalidDecisionField(format!(
                "duplicate decision id: '{}'",
                decision.id
            )));
        }
    }
    Ok(())
}

/// First-pass key-presence check: forbid typestate-specific keys on the
/// wrong status. `serde` cannot see the difference between `key: null` and
/// `key absent` after deserialization, so the check inspects the raw value.
fn check_decision_key_presence(raw: &serde_yaml::Value) -> Result<(), AdrFrontMatterCodecError> {
    let Some(decisions) = raw.get("decisions") else {
        return Ok(());
    };
    let Some(decisions) = decisions.as_sequence() else {
        return Ok(());
    };

    for decision in decisions {
        let Some(map) = decision.as_mapping() else { continue };
        let status = map
            .get(serde_yaml::Value::String("status".to_owned()))
            .and_then(serde_yaml::Value::as_str)
            .unwrap_or("");

        let id = map
            .get(serde_yaml::Value::String("id".to_owned()))
            .and_then(serde_yaml::Value::as_str)
            .unwrap_or("?");

        let has_implemented_in =
            map.contains_key(serde_yaml::Value::String("implemented_in".to_owned()));
        let has_superseded_by =
            map.contains_key(serde_yaml::Value::String("superseded_by".to_owned()));

        if has_implemented_in && status != "implemented" {
            return Err(AdrFrontMatterCodecError::InvalidDecisionField(format!(
                "decision '{id}': `implemented_in` key is only allowed when status is 'implemented' (got '{status}')"
            )));
        }
        if has_superseded_by && status != "superseded" {
            return Err(AdrFrontMatterCodecError::InvalidDecisionField(format!(
                "decision '{id}': `superseded_by` key is only allowed when status is 'superseded' (got '{status}')"
            )));
        }
    }

    Ok(())
}

/// Convert a typed [`AdrDecisionDto`] into the domain
/// [`AdrDecisionEntry`] variant that matches its `status` string.
///
/// Validates required-when-value invariants (e.g. `status: "implemented"`
/// requires `implemented_in: Some(_)`) and dispatches to the correct
/// typestate constructor.
fn decision_dto_to_entry(
    dto: AdrDecisionDto,
) -> Result<AdrDecisionEntry, AdrFrontMatterCodecError> {
    let AdrDecisionDto {
        id,
        user_decision_ref,
        review_finding_ref,
        candidate_selection,
        status,
        superseded_by,
        implemented_in,
        grandfathered,
    } = dto;

    let common = AdrDecisionCommon::new(
        id.clone(),
        user_decision_ref,
        review_finding_ref,
        candidate_selection,
        grandfathered.unwrap_or(false),
    )
    .map_err(|e| AdrFrontMatterCodecError::InvalidDecisionField(format!("decision '{id}': {e}")))?;

    match status.as_str() {
        "proposed" => Ok(AdrDecisionEntry::ProposedDecision(ProposedDecision::new(common))),
        "accepted" => Ok(AdrDecisionEntry::AcceptedDecision(AcceptedDecision::new(common))),
        "implemented" => {
            let implemented_in = implemented_in.ok_or_else(|| {
                AdrFrontMatterCodecError::InvalidDecisionField(format!(
                    "decision '{id}': status 'implemented' requires `implemented_in`"
                ))
            })?;
            ImplementedDecision::new(common, implemented_in)
                .map(AdrDecisionEntry::ImplementedDecision)
                .map_err(|e| {
                    AdrFrontMatterCodecError::InvalidDecisionField(format!("decision '{id}': {e}"))
                })
        }
        "superseded" => {
            let superseded_by = superseded_by.ok_or_else(|| {
                AdrFrontMatterCodecError::InvalidDecisionField(format!(
                    "decision '{id}': status 'superseded' requires `superseded_by`"
                ))
            })?;
            SupersededDecision::new(common, superseded_by)
                .map(AdrDecisionEntry::SupersededDecision)
                .map_err(|e| {
                    AdrFrontMatterCodecError::InvalidDecisionField(format!("decision '{id}': {e}"))
                })
        }
        "deprecated" => Ok(AdrDecisionEntry::DeprecatedDecision(DeprecatedDecision::new(common))),
        other => Err(AdrFrontMatterCodecError::InvalidDecisionField(format!(
            "decision '{id}': unknown status '{other}'"
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn wrap(yaml: &str) -> String {
        format!("---\n{yaml}\n---\n# body\n")
    }

    // ── Happy paths: each of the 5 status strings ─────────────────────────────

    #[test]
    fn test_parse_adr_frontmatter_valid_proposed_decision_succeeds() {
        let content = wrap(
            r#"adr_id: 2026-04-27-1234-foo
decisions:
  - id: D1
    status: proposed
    user_decision_ref: chat:2026-04-25"#,
        );
        let fm = parse_adr_frontmatter(&content).unwrap();
        assert_eq!(fm.adr_id(), "2026-04-27-1234-foo");
        assert_eq!(fm.decisions().len(), 1);
        match &fm.decisions()[0] {
            AdrDecisionEntry::ProposedDecision(d) => {
                assert_eq!(d.common.id(), "D1");
                assert_eq!(d.common.user_decision_ref(), Some("chat:2026-04-25"));
            }
            other => panic!("expected ProposedDecision, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_adr_frontmatter_valid_accepted_decision_succeeds() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D2
    status: accepted
    review_finding_ref: RF-12"#,
        );
        let fm = parse_adr_frontmatter(&content).unwrap();
        match &fm.decisions()[0] {
            AdrDecisionEntry::AcceptedDecision(d) => {
                assert_eq!(d.common.id(), "D2");
                assert_eq!(d.common.review_finding_ref(), Some("RF-12"));
            }
            other => panic!("expected AcceptedDecision, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_adr_frontmatter_valid_implemented_decision_succeeds() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D3
    status: implemented
    user_decision_ref: chat:2026-04-25
    implemented_in: abc1234"#,
        );
        let fm = parse_adr_frontmatter(&content).unwrap();
        match &fm.decisions()[0] {
            AdrDecisionEntry::ImplementedDecision(d) => {
                assert_eq!(d.common.id(), "D3");
                assert_eq!(d.implemented_in(), "abc1234");
            }
            other => panic!("expected ImplementedDecision, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_adr_frontmatter_valid_superseded_decision_succeeds() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D4
    status: superseded
    user_decision_ref: chat:2026-04-25
    superseded_by: 2026-05-01-other.md#D7"#,
        );
        let fm = parse_adr_frontmatter(&content).unwrap();
        match &fm.decisions()[0] {
            AdrDecisionEntry::SupersededDecision(d) => {
                assert_eq!(d.common.id(), "D4");
                assert_eq!(d.superseded_by(), "2026-05-01-other.md#D7");
            }
            other => panic!("expected SupersededDecision, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_adr_frontmatter_valid_deprecated_decision_succeeds() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D5
    status: deprecated
    user_decision_ref: chat:2026-04-25"#,
        );
        let fm = parse_adr_frontmatter(&content).unwrap();
        match &fm.decisions()[0] {
            AdrDecisionEntry::DeprecatedDecision(d) => assert_eq!(d.common.id(), "D5"),
            other => panic!("expected DeprecatedDecision, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_adr_frontmatter_grandfathered_true_passes_through() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D1
    status: proposed
    grandfathered: true"#,
        );
        let fm = parse_adr_frontmatter(&content).unwrap();
        match &fm.decisions()[0] {
            AdrDecisionEntry::ProposedDecision(d) => assert!(d.common.grandfathered()),
            other => panic!("unexpected variant {other:?}"),
        }
    }

    #[test]
    fn test_parse_adr_frontmatter_multiple_decisions_preserves_order() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D1
    status: proposed
    user_decision_ref: chat
  - id: D2
    status: accepted
    user_decision_ref: chat"#,
        );
        let fm = parse_adr_frontmatter(&content).unwrap();
        assert_eq!(fm.decisions().len(), 2);
        assert!(matches!(&fm.decisions()[0], AdrDecisionEntry::ProposedDecision(_)));
        assert!(matches!(&fm.decisions()[1], AdrDecisionEntry::AcceptedDecision(_)));
    }

    // ── Error paths ───────────────────────────────────────────────────────────

    #[test]
    fn test_parse_adr_frontmatter_no_frontmatter_returns_missing_adr_id() {
        let content = "# Just a body, no front-matter\n";
        let err = parse_adr_frontmatter(content).unwrap_err();
        assert!(matches!(err, AdrFrontMatterCodecError::MissingAdrId));
    }

    #[test]
    fn test_parse_adr_frontmatter_empty_adr_id_returns_missing_adr_id() {
        let content = wrap(
            r#"adr_id: ""
decisions: []"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        assert!(matches!(err, AdrFrontMatterCodecError::MissingAdrId));
    }

    #[test]
    fn test_parse_adr_frontmatter_malformed_yaml_returns_yaml_parse() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D1
   status: proposed"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        assert!(matches!(err, AdrFrontMatterCodecError::YamlParse(_)));
    }

    #[test]
    fn test_parse_adr_frontmatter_unknown_top_level_field_returns_yaml_parse() {
        let content = wrap(
            r#"adr_id: foo
unexpected_key: bar"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        assert!(matches!(err, AdrFrontMatterCodecError::YamlParse(_)));
    }

    #[test]
    fn test_parse_adr_frontmatter_unknown_decision_field_returns_yaml_parse() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D1
    status: proposed
    bogus_field: 42"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        assert!(matches!(err, AdrFrontMatterCodecError::YamlParse(_)));
    }

    #[test]
    fn test_parse_adr_frontmatter_unknown_status_returns_invalid_decision_field() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D1
    status: weirdstate"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        match err {
            AdrFrontMatterCodecError::InvalidDecisionField(msg) => {
                assert!(msg.contains("unknown status"), "got: {msg}");
                assert!(msg.contains("weirdstate"), "got: {msg}");
            }
            other => panic!("expected InvalidDecisionField, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_adr_frontmatter_implemented_without_implemented_in_returns_invalid_decision_field()
     {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D1
    status: implemented
    user_decision_ref: chat"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        assert!(matches!(err, AdrFrontMatterCodecError::InvalidDecisionField(_)));
    }

    #[test]
    fn test_parse_adr_frontmatter_superseded_without_superseded_by_returns_invalid_decision_field()
    {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D1
    status: superseded
    user_decision_ref: chat"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        assert!(matches!(err, AdrFrontMatterCodecError::InvalidDecisionField(_)));
    }

    // ── Key-presence detection (raw Value pass) ───────────────────────────────

    #[test]
    fn test_parse_adr_frontmatter_implemented_in_null_on_proposed_returns_invalid_decision_field() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D1
    status: proposed
    implemented_in: null"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        match err {
            AdrFrontMatterCodecError::InvalidDecisionField(msg) => {
                assert!(msg.contains("implemented_in"), "got: {msg}");
            }
            other => panic!("expected InvalidDecisionField, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_adr_frontmatter_implemented_in_value_on_proposed_returns_invalid_decision_field()
    {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D1
    status: proposed
    implemented_in: abc1234"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        assert!(matches!(err, AdrFrontMatterCodecError::InvalidDecisionField(_)));
    }

    #[test]
    fn test_parse_adr_frontmatter_superseded_by_null_on_accepted_returns_invalid_decision_field() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D1
    status: accepted
    superseded_by: null"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        match err {
            AdrFrontMatterCodecError::InvalidDecisionField(msg) => {
                assert!(msg.contains("superseded_by"), "got: {msg}");
            }
            other => panic!("expected InvalidDecisionField, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_adr_frontmatter_empty_decision_id_returns_invalid_decision_field() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: ""
    status: proposed"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        assert!(matches!(err, AdrFrontMatterCodecError::InvalidDecisionField(_)));
    }

    #[test]
    fn test_parse_adr_frontmatter_empty_decisions_array_succeeds() {
        let content = wrap(
            r#"adr_id: foo
decisions: []"#,
        );
        let fm = parse_adr_frontmatter(&content).unwrap();
        assert_eq!(fm.adr_id(), "foo");
        assert!(fm.decisions().is_empty());
    }

    #[test]
    fn test_parse_adr_frontmatter_decisions_omitted_succeeds() {
        let content = wrap(r#"adr_id: foo"#);
        let fm = parse_adr_frontmatter(&content).unwrap();
        assert_eq!(fm.adr_id(), "foo");
        assert!(fm.decisions().is_empty());
    }

    #[test]
    fn test_parse_adr_frontmatter_duplicate_decision_ids_returns_invalid_decision_field() {
        let content = wrap(
            r#"adr_id: foo
decisions:
  - id: D1
    status: proposed
    user_decision_ref: chat:2026-04-25
  - id: D1
    status: accepted
    user_decision_ref: chat:2026-04-25"#,
        );
        let err = parse_adr_frontmatter(&content).unwrap_err();
        match err {
            AdrFrontMatterCodecError::InvalidDecisionField(msg) => {
                assert!(msg.contains("duplicate decision id"), "got: {msg}");
                assert!(msg.contains("D1"), "got: {msg}");
            }
            other => panic!("expected InvalidDecisionField, got {other:?}"),
        }
    }
}
