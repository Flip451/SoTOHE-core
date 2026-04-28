<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrFrontMatterCodecError | error_type | — | YamlParse, MissingAdrId, InvalidDecisionField | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrFrontMatterDto | dto | — | — | 🟡 | 🔵 |
| AdrDecisionDto | dto | — | — | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsAdrFileAdapter | secondary_adapter | — | impl AdrFilePort | 🟡 | 🟡 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| parse_adr_frontmatter | free_function | — | — | 🟡 | 🟡 |

