//! Tests for the convention front-matter codec.
//!
//! Kept in a sibling module so that only the production half adds to the
//! parent module's length.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use usecase::conventions_resolve::{
    ConventionCapabilityId, ConventionDocumentPath, ConventionResolveError,
};

use super::{CapabilityIdField, ConventionFrontMatterDto, parse_convention_front_matter};

fn document() -> ConventionDocumentPath {
    ConventionDocumentPath::try_new(PathBuf::from("knowledge/conventions/testing.md")).unwrap()
}

fn capability(id: &str) -> ConventionCapabilityId {
    ConventionCapabilityId::try_new(id).unwrap()
}

/// Reads back the elements a decoded view carries, in the order it holds them.
///
/// The view's own contents are read rather than the requirement built from it,
/// so what a test using this observes is the codec's output itself and not what
/// a later comparison makes of it.
fn declared(dto: &ConventionFrontMatterDto) -> Vec<&str> {
    dto.required_for.iter().map(CapabilityIdField::as_str).collect()
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

    let dto = parse_convention_front_matter(&document(), content).unwrap();

    assert_eq!(
        declared(&dto),
        ["implementer", "reviewer"],
        "decoding hands back a view carrying the elements the document declared, as it spelled \
         them and in the order it listed them"
    );
    assert_eq!(
        requirement_from(content).unwrap(),
        ["implementer", "reviewer"],
        "and those decoded elements are the ones a requirement built from the view answers on"
    );
}

#[test]
fn test_parse_convention_front_matter_tolerates_keys_it_does_not_read() {
    let content = "---\nname: testing\ndescription: how to test\nrequired_for:\n  - implementer\nnested:\n  key: value\n---\n";

    let dto = parse_convention_front_matter(&document(), content).unwrap();

    assert_eq!(
        declared(&dto),
        ["implementer"],
        "consumer documents own their own metadata, so unread keys are neither carried into the \
         decoded view nor rejected while producing it"
    );
    assert_eq!(requirement_from(content).unwrap(), ["implementer"]);
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
fn test_parse_convention_front_matter_with_a_comment_only_block_decodes_to_the_default_value() {
    // Comments contribute no YAML node, so a block holding only them declared
    // nothing — the same empty state as a blank block, and not a scalar the
    // mapping check should refuse.
    let content = "---\n# just a note\n\n#: and another\n---\n\n# Testing\n";

    assert_eq!(
        requirement_from(content).unwrap(),
        Vec::<String>::new(),
        "a block carrying only comments declares nothing, which `AC-08` makes a normal empty \
         state rather than malformed metadata"
    );
}

#[test]
fn test_parse_convention_front_matter_with_crlf_line_endings_inside_an_empty_block() {
    // A carriage return inside the block is a line break rather than content,
    // whether the delimiters around it carry one or not, and a blank CRLF line
    // is still a blank line.
    let content = "---\n\r\n# just a note\r\n---\n\n# Testing\n";

    assert_eq!(
        requirement_from(content).unwrap(),
        Vec::<String>::new(),
        "the carriage return of a CRLF break is not content, so a block holding only blank and \
         comment lines still declares nothing"
    );
}

#[test]
fn test_parse_convention_front_matter_with_non_yaml_whitespace_before_a_hash_fails_closed() {
    // YAML's separation whitespace is space and tab and nothing else, so none
    // of these leading characters ends a line's indentation: the `#` after one
    // is not preceded by whitespace and so starts no comment, leaving the line
    // a plain scalar. Deciding emptiness with a Unicode-aware trim would strip
    // them, read the line as a comment, and hand back an empty declaration for
    // front matter that is malformed — narrower than reading an explicit null
    // that way, and the same fail-open.
    for space in ['\u{A0}', '\u{3000}', '\u{2009}'] {
        let content = format!("---\n{space}#not-a-comment\n---\n");

        let result = parse_convention_front_matter(&document(), &content);

        assert!(
            matches!(result, Err(ConventionResolveError::FrontMatterUnparseable { .. })),
            "U+{:04X} is not YAML whitespace, so this block holds a scalar and is refused with \
             every other non-mapping rather than read as declaring nothing",
            space as u32
        );
    }
}

#[test]
fn test_parse_convention_front_matter_with_an_explicit_null_block_fails_closed() {
    // `serde_yaml` decodes all three of these to the same `Value::Null` an
    // empty block produces, so a codec testing the parsed value would read them
    // as an absent declaration. They are not absent: the document wrote a
    // scalar, and a scalar cannot present `required_for`.
    for block in ["null", "~", "Null"] {
        let content = format!("---\n{block}\n---\n");

        let result = parse_convention_front_matter(&document(), &content);

        assert!(
            matches!(result, Err(ConventionResolveError::FrontMatterUnparseable { .. })),
            "'{block}' is front matter the document declared and is not a mapping, so it fails \
             closed rather than being decoded as declaring nothing"
        );
    }
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
fn test_parse_convention_front_matter_carries_a_blank_element_to_its_deciding_site() {
    for (element, decoded) in [("''", ""), ("'   '", "   "), ("\"\\t\"", "\t")] {
        let content = format!("---\nrequired_for:\n  - {element}\n---\n");

        let dto = parse_convention_front_matter(&document(), &content)
            .expect("a blank element is a well-shaped string, so decoding cannot fail on it");

        assert_eq!(
            declared(&dto),
            [decoded],
            "the element reaches the decoded view exactly as written, neither trimmed away nor \
             rejected here: rejecting it would make the empty-identifier condition a decision \
             this codec takes as well as the identifier constructor"
        );
        assert!(
            matches!(
                dto.into_requirement(document()),
                Err(ConventionResolveError::EmptyCapabilityId { .. })
            ),
            "and carrying it intact is what leaves the one site that does decide it able to fail \
             closed on '{element}'"
        );
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

#[test]
fn test_parse_convention_front_matter_reads_a_declaration_after_every_parser_line_break() {
    // The parser breaks on more than `\n` and `\r`: `libyaml`'s `IS_BREAK` also
    // admits NEL, LS, and PS, which was established by running the parser rather
    // than by reading the specification — the two differ here. Each block below
    // is a comment followed by a declaration and the parser reads every one as a
    // mapping, so a comment-detection test that does not break on the same
    // characters sees one line beginning with `#`, calls the block comment-only,
    // and discards the declaration with no diagnostic.
    for (name, separator) in
        [("LF", "\n"), ("CR", "\r"), ("NEL", "\u{85}"), ("LS", "\u{2028}"), ("PS", "\u{2029}")]
    {
        // The declaration is a flow mapping, so the whole block occupies one
        // `\n`-delimited line and the separator under test is the *only* thing
        // that can end the comment. Written as a block mapping it would spill
        // onto a second `\n` line, and that line alone would make the block
        // look non-empty however the first was split — the assertion would then
        // hold for every separator regardless of which breaks are recognised,
        // which is no test of the break set at all.
        let content = format!("---\n# note{separator}required_for: [implementer]\n---\n");

        let dto = parse_convention_front_matter(&document(), &content)
            .unwrap_or_else(|error| panic!("the {name} block should decode: {error}"));

        assert_eq!(
            declared(&dto),
            ["implementer"],
            "the declaration after a {name}-separated comment survives, because this codec ends \
             the comment where the parser does"
        );
    }
}

#[test]
fn test_parse_convention_front_matter_reads_a_declaration_however_the_document_is_delimited() {
    // The codec's end of the same question the reader answers: a document whose
    // block this repository cannot locate is not reported as malformed, it is
    // reported as declaring nothing, and the consumer's convention silently
    // stops applying. Each spelling below is an ordinary file — a marked one, a
    // CR-broken one, and one that is both — carrying one real declaration.
    for (name, content) in [
        ("byte-order mark", "\u{FEFF}---\nrequired_for: [implementer]\n---\n"),
        ("lone CR", "---\rrequired_for: [implementer]\r---\r"),
        ("mark and CR", "\u{FEFF}---\rrequired_for: [implementer]\r---\r"),
    ] {
        let dto = parse_convention_front_matter(&document(), content)
            .unwrap_or_else(|error| panic!("the {name} document should decode: {error}"));

        assert_eq!(
            declared(&dto),
            ["implementer"],
            "a {name} document declares what it says it declares, rather than resolving as a \
             document that declares nothing"
        );
    }
}

#[test]
fn test_parse_convention_front_matter_reads_a_declaration_from_a_crlf_document() {
    // A document written by a Windows editor: every break in the block is CRLF.
    // The block reader hands breaks back as the file spelled them, so this is
    // the one path where the hand-written `RequiredForElements` visitor and the
    // `CapabilityIdField` mirror meet CRLF-separated sequence elements.
    let content = "---\r\nrequired_for:\r\n  - implementer\r\n  - reviewer\r\n---\r\n";

    let dto = parse_convention_front_matter(&document(), content)
        .expect("CRLF is an ordinary line ending, not a malformed document");

    let declared: Vec<&str> = dto.required_for.iter().map(CapabilityIdField::as_str).collect();
    assert_eq!(
        declared,
        ["implementer", "reviewer"],
        "every declaration survives a CRLF block, spelled as the document wrote it"
    );
}

#[test]
fn test_parse_convention_front_matter_rejects_a_block_the_parser_refuses_but_a_lexical_test_calls_empty()
 {
    // Every one of these looks blank-or-comment to a line-by-line reading, and
    // every one is refused by the parser: a tab may not indent, and a control
    // character may not appear even inside a comment. Deciding emptiness before
    // parsing let each of them return the default DTO, so a document whose
    // metadata is malformed lost its `required_for` enforcement instead of
    // failing closed. Parsing first is what makes that unreachable, and this
    // test fails the moment the fast path is reintroduced.
    for (name, block) in
        [("tab indentation", "\t# comment"), ("control character in a comment", "# comm\u{1}ent")]
    {
        let content = format!("---\n{block}\n---\n");

        let Err(error) = parse_convention_front_matter(&document(), &content) else {
            panic!(
                "the parser refuses {name}, so this codec must fail closed rather than \
                    report an empty declaration"
            );
        };

        assert!(
            matches!(error, ConventionResolveError::FrontMatterUnparseable { .. }),
            "{name} is a decode failure, reported as one: {error}"
        );
    }
}
