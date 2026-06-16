<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrFrontMatterCodecError | error_type | reference | YamlParse, MissingAdrId, InvalidDecisionField | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsAdrFileAdapter | secondary_adapter | reference | impl AdrFilePort | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::adr_decision::parse::parse_adr_frontmatter | free_function | modify | fn(content: &str) -> Result<domain::AdrFrontMatter, AdrFrontMatterCodecError> | 🔵 | 🔵 |

