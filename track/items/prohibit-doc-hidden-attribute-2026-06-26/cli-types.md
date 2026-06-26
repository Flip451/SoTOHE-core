<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| VerifyCommand | enum | modify | TechStack, LatestTrack, ArchDocs, Layers, HooksPath, SpecAttribution, SpecFrontmatter, CanonicalModules, ModuleSize, DomainPurity, DomainStrings, UsecasePurity, DocLinks, ViewFreshness, SpecSignals, PlanArtifactRefs, CatalogueSpecRefs, DocHidden | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| VerifyArgs | dto | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::verify_command_gate_name | free_function | modify | fn(cmd: &commands::verify::VerifyCommand) -> &'static str | 🔵 | 🔵 |

