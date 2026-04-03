# Verification: BRIDGE-01 sotp export-schema

## Scope Verified

- [ ] Domain types match spec domain_states
- [ ] public trait definitions and trait method signatures are exported
- [ ] pub(crate) exclusion confirmed
- [ ] trait impl methods remain excluded from export output
- [ ] Multi-path / --layer resolution works
- [ ] text/json output formats correct
- [ ] domain_scanner.rs behavior unchanged
- [ ] vision.md / TODO-PLAN.md command names updated to `sotp export-schema`, and TODO.md Phase 3 roadmap is aligned with this strategy update (T007)

## Manual Verification Steps

1. Run `sotp export-schema libs/domain/src` and verify text output (module hierarchy preserved)
2. Run `sotp export-schema --format json libs/domain/src` and verify JSON schema (stable ordering)
3. Verify a public `trait` definition and its method signatures appear in the exported output
4. Run an input containing both inherent impl methods and trait impl methods, and verify only the inherent impl methods are exported
5. Run `sotp export-schema --layer domain` with architecture-rules.json
6. Run `sotp export-schema libs/domain/src libs/usecase/src` and verify multi-path aggregation
7. Run `sotp export-schema --project-root /path/to/project --layer domain` and verify custom root
8. Run `sotp export-schema` with no args and verify error message
9. Verify `vision.md` and `TODO-PLAN.md` contain `sotp export-schema` and do not contain `domain export-schema`, then verify `TODO.md` Phase 3 roadmap and `最終更新` date reflect this strategy update
10. Verify `cargo make ci` passes

## Result

(pending)

## Open Issues

(none)

## Verified At

(pending)
