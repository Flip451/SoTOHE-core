//! Tests for the convention front-matter codec.
//!
//! Kept in a sibling module so that only the production half adds to the
//! parent module's length.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use usecase::conventions_resolve::{
    ConventionCapabilityId, ConventionDocumentPath, ConventionResolveError,
};

use super::{ConventionFrontMatterDto, parse_convention_front_matter};

fn document() -> ConventionDocumentPath {
    ConventionDocumentPath::try_new(PathBuf::from("knowledge/conventions/testing.md")).unwrap()
}

fn capability(id: &str) -> ConventionCapabilityId {
    ConventionCapabilityId::try_new(id).unwrap()
}

/// Decodes `content` and pairs it with the fixture document.
fn requirement_from(content: &str) -> Result<Vec<String>, ConventionResolveError> {
    let dto = parse_convention_front_matter(&document(), content)?;
    let requirement = dto.into_requirement(document())?;
    Ok(["implementer", "reviewer", "consumer-house-style"]
        .into_iter()
        .filter(|id| requirement.requires(&capability(id)))
        .map(str::to_owned)
        .collect())
}

#[test]
fn test_parse_convention_front_matter_decodes_a_required_for_string_array() {
    let content = "---\nrequired_for:\n  - implementer\n  - reviewer\n---\n\n# Testing\n";

    let matched = requirement_from(content).unwrap();

    assert_eq!(matched, ["implementer", "reviewer"]);
}

#[test]
fn test_parse_convention_front_matter_tolerates_keys_it_does_not_read() {
    let content = "---\nname: testing\ndescription: how to test\nrequired_for:\n  - implementer\nnested:\n  key: value\n---\n";

    let matched = requirement_from(content).unwrap();

    assert_eq!(
        matched,
        ["implementer"],
        "consumer documents own their own metadata, so unread keys are not this codec's business"
    );
}

/// Keys YAML admits that no field-identifier resolution would accept.
const NON_STRING_KEYS: [&str; 5] = ["true", "42", "3.14", "null", "[a, b]"];

#[test]
fn test_parse_convention_front_matter_ignores_a_non_string_key_beside_required_for() {
    for key in NON_STRING_KEYS {
        let content = format!("---\n{key}: metadata\nrequired_for:\n  - implementer\n---\n");

        assert_eq!(
            requirement_from(&content).unwrap(),
            ["implementer"],
            "the entry keyed '{key}' is one this codec does not read, so it is ignored rather \
             than resolved as a field identifier"
        );
    }
}

#[test]
fn test_parse_convention_front_matter_with_only_a_non_string_key_decodes_to_the_default_value() {
    for key in NON_STRING_KEYS {
        let content = format!("---\n{key}: metadata\n---\n");

        assert_eq!(
            requirement_from(&content).unwrap(),
            Vec::<String>::new(),
            "'{key}: metadata' is well-formed YAML declaring no `required_for`, which AC-08 makes \
             a normal empty state rather than a shape failure"
        );
    }
}

#[test]
fn test_parse_convention_front_matter_reports_required_for_shape_beside_a_non_string_key() {
    let content = "---\ntrue: metadata\nrequired_for: implementer\n---\n";

    let Err(error) = parse_convention_front_matter(&document(), content) else {
        panic!("expected a RequiredForNotStringArray rejection");
    };

    assert!(
        matches!(error, ConventionResolveError::RequiredForNotStringArray { .. }),
        "the shape failure is `required_for`'s own, and the neighbouring non-string key neither \
         masks it nor is reported in its place: {error}"
    );
}

#[test]
fn test_parse_convention_front_matter_with_an_unregistered_capability_id_decodes_it_unchanged() {
    // `consumer-house-style` names no entry in `.harness/capabilities/` and
    // none in `agent-profiles.json`. The codec consults neither registry, so
    // there is no state in which decoding fails for want of a registration.
    let content = "---\nrequired_for:\n  - consumer-house-style\n---\n";

    let matched = requirement_from(content).unwrap();

    assert_eq!(matched, ["consumer-house-style"]);
}

#[test]
fn test_parse_convention_front_matter_preserves_element_text_verbatim() {
    let content = "---\nrequired_for:\n  - ' implementer '\n  - Reviewer\n---\n";

    let dto = parse_convention_front_matter(&document(), content).unwrap();
    let requirement = dto.into_requirement(document()).unwrap();

    assert!(
        !requirement.requires(&capability("implementer")),
        "a padded element is a different identifier, so trimming it here would decide a match \
         the usecase comparison is supposed to decide"
    );
    assert!(requirement.requires(&capability(" implementer ")));
    assert!(
        !requirement.requires(&capability("reviewer")),
        "case is not folded either, for the same reason"
    );
    assert!(requirement.requires(&capability("Reviewer")));
}

#[test]
fn test_capability_id_field_borrows_the_wire_text_verbatim() {
    let field: super::CapabilityIdField = serde_yaml::from_str("' implementer '").unwrap();

    assert_eq!(field.as_str(), " implementer ");
}

#[test]
fn test_parse_convention_front_matter_without_a_block_decodes_to_the_default_value() {
    let content = "# Testing\n\nA document that carries no front matter at all.\n";

    let requirement = parse_convention_front_matter(&document(), content)
        .unwrap()
        .into_requirement(document())
        .unwrap();

    assert_eq!(requirement.document(), &document());
    assert!(
        !requirement.requires(&capability("implementer")),
        "a document without front matter requires nothing, which is a normal empty state"
    );
}

#[test]
fn test_parse_convention_front_matter_without_required_for_decodes_to_the_default_value() {
    let content = "---\nname: testing\ndescription: how to test\n---\n";

    assert_eq!(
        requirement_from(content).unwrap(),
        Vec::<String>::new(),
        "an absent `required_for` is the same empty state as an absent block"
    );
}

#[test]
fn test_parse_convention_front_matter_with_an_empty_block_decodes_to_the_default_value() {
    // The shared block reader wants the closing delimiter on its own line, so
    // an empty block holds one blank line.
    let content = "---\n\n---\n\n# Testing\n";

    assert_eq!(requirement_from(content).unwrap(), Vec::<String>::new());
}

#[test]
fn test_default_convention_front_matter_dto_requires_nothing() {
    let requirement = ConventionFrontMatterDto::default().into_requirement(document()).unwrap();

    assert_eq!(requirement.document(), &document());
    assert!(!requirement.requires(&capability("implementer")));
}

#[test]
fn test_parse_convention_front_matter_with_an_unclosed_block_fails_closed() {
    let content = "---\nrequired_for:\n  - implementer\n";

    let result = parse_convention_front_matter(&document(), content);

    assert!(
        matches!(result, Err(ConventionResolveError::FrontMatterUnparseable { .. })),
        "an unclosed block yields no front matter to parse, so it is not read as an absent one"
    );
}

#[test]
fn test_parse_convention_front_matter_with_unparseable_yaml_fails_closed() {
    let content = "---\nrequired_for: [implementer\n---\n";

    let Err(error) = parse_convention_front_matter(&document(), content) else {
        panic!("expected a FrontMatterUnparseable rejection");
    };

    assert!(matches!(error, ConventionResolveError::FrontMatterUnparseable { .. }));
    assert!(
        error.to_string().contains("knowledge/conventions/testing.md"),
        "the failure names the document, which the parser itself is never told: {error}"
    );
}

#[test]
fn test_parse_convention_front_matter_with_a_non_mapping_block_fails_closed() {
    for block in ["- implementer\n- reviewer", "just a scalar", "42"] {
        let content = format!("---\n{block}\n---\n");

        let result = parse_convention_front_matter(&document(), &content);

        assert!(
            matches!(result, Err(ConventionResolveError::FrontMatterUnparseable { .. })),
            "'{block}' is valid YAML but cannot present `required_for`, so it is not decoded as \
             an absent declaration"
        );
    }
}

#[test]
fn test_parse_convention_front_matter_with_a_non_array_required_for_fails_closed() {
    for value in ["implementer", "{ implementer: true }", "42", "true"] {
        let content = format!("---\nrequired_for: {value}\n---\n");

        let result = parse_convention_front_matter(&document(), &content);

        assert!(
            matches!(result, Err(ConventionResolveError::RequiredForNotStringArray { .. })),
            "`required_for: {value}` is not an array of strings"
        );
    }
}

#[test]
fn test_parse_convention_front_matter_with_a_null_required_for_fails_closed() {
    for value in ["", " null", " ~", " Null"] {
        let content = format!("---\nrequired_for:{value}\n---\n");

        let Err(error) = parse_convention_front_matter(&document(), &content) else {
            panic!("expected a RequiredForNotStringArray rejection for 'required_for:{value}'");
        };

        assert!(
            matches!(error, ConventionResolveError::RequiredForNotStringArray { .. }),
            "a null `required_for` is not an array of strings, so it fails closed rather than \
             being coerced into an empty declaration list: {error}"
        );
    }
}

#[test]
fn test_parse_convention_front_matter_distinguishes_an_absent_required_for_from_a_null_one() {
    let absent = parse_convention_front_matter(&document(), "---\nname: testing\n---\n");
    let null = parse_convention_front_matter(&document(), "---\nrequired_for:\n---\n");

    assert!(
        absent.is_ok(),
        "a document declaring no `required_for` is a normal empty state (AC-08)"
    );
    assert!(
        matches!(null, Err(ConventionResolveError::RequiredForNotStringArray { .. })),
        "a `required_for` present with a null value is a shape failure (AC-07), and the two \
         must not collapse into the same outcome"
    );
}

#[test]
fn test_parse_convention_front_matter_with_an_empty_required_for_array_declares_nothing() {
    let content = "---\nrequired_for: []\n---\n";

    assert_eq!(
        requirement_from(content).unwrap(),
        Vec::<String>::new(),
        "an empty array is a well-shaped declaration of nothing, which is the one spelling the \
         empty list is reachable by"
    );
}

#[test]
fn test_parse_convention_front_matter_with_a_non_string_element_fails_closed() {
    for element in ["42", "true", "{ id: implementer }", "[implementer]"] {
        let content = format!("---\nrequired_for:\n  - {element}\n---\n");

        let Err(error) = parse_convention_front_matter(&document(), &content) else {
            panic!("expected a RequiredForNotStringArray rejection for element '{element}'");
        };

        assert!(
            matches!(error, ConventionResolveError::RequiredForNotStringArray { .. }),
            "a non-string element is a shape error decided while decoding, distinct from the \
             empty-identifier condition: {error}"
        );
        assert!(error.to_string().contains("knowledge/conventions/testing.md"), "{error}");
    }
}

#[test]
fn test_convention_front_matter_dto_translates_a_blank_element_into_empty_capability_id() {
    for element in ["''", "'   '", "\"\\t\""] {
        let content = format!("---\nrequired_for:\n  - {element}\n---\n");

        let dto = parse_convention_front_matter(&document(), &content)
            .expect("a blank element is a well-shaped string, so decoding succeeds");
        let Err(error) = dto.into_requirement(document()) else {
            panic!("expected an EmptyCapabilityId rejection for element '{element}'");
        };

        assert!(
            matches!(error, ConventionResolveError::EmptyCapabilityId { .. }),
            "the identifier constructor's rejection is translated here, not decided again: \
             {error}"
        );
        assert!(
            error.to_string().contains("knowledge/conventions/testing.md"),
            "the translation supplies the document the constructor is never handed: {error}"
        );
    }
}

#[test]
fn test_convention_front_matter_dto_rejects_a_blank_element_among_accepted_ones() {
    let content = "---\nrequired_for:\n  - implementer\n  - ''\n  - reviewer\n---\n";

    let dto = parse_convention_front_matter(&document(), content).unwrap();

    assert!(
        matches!(
            dto.into_requirement(document()),
            Err(ConventionResolveError::EmptyCapabilityId { .. })
        ),
        "one blank identifier fails the whole document rather than being dropped from the list"
    );
}
