<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| VerifyCommand | enum | modify | LatestTrack, RetentionGate, SotpVersionTag, MachinePaths, TemplateRefs, ArchDocs, Layers, HooksPath, SpecAttribution, SpecFrontmatter, CanonicalModules, ModuleSize, DomainPurity, DomainStrings, UsecasePurity, DocLinks, ViewFreshness, SpecSignals, PlanArtifactRefs, CatalogueSpecRefs | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueSpecRefsArgs | dto | modify | — | 🔵 | 🔵 |
| VerifyArgs | dto | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::verify::parse_track_id | free_function | add | fn(value: &str) -> Result<cli_driver::verify::TrackId, clap::Error> | 🔵 | 🔵 |

