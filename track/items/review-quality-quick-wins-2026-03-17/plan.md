<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Review quality quick wins: shared frontmatter parser + fail-closed convention

Extract duplicated frontmatter parsing into a shared module and replace hand-rolled JSON parsing with typed serde deserialization

## Shared Frontmatter Parser

Extract common YAML frontmatter parsing logic from spec_attribution.rs and spec_frontmatter.rs into a shared frontmatter.rs module.

- [x] Extract shared frontmatter parser from spec_attribution and spec_frontmatter into verify/frontmatter.rs

## Typed Deserialization Convention

Replace hand-rolled serde_json::Value parsing in canonical_modules.rs with typed #[derive(Deserialize)] structs, and add convention doc for typed deserialization over manual JSON walking.

- [x] Replace hand-rolled serde_json::Value parsing in canonical_modules.rs with typed deserialization (#[derive(Deserialize)]), add typed-deserialization and prefer-type-safe-abstractions convention docs, and update conventions/README.md index
