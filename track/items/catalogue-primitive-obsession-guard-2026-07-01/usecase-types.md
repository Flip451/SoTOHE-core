<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| LintRuleKind | enum | modify | FieldEmpty, FieldNonEmpty, KindLayerConstraint, ReferencedRoleConstraint, TraitImplRequired, NoRoleInMethodSignature, MethodReferenceSignature, AccessorSignatureRequired, FieldElementUniqueAcrossEntries, NoExternalReferenceInMethods, NoPublicField, ForbiddenMethodReceiver, ForbidPrimitiveInTypes | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| LintConfigLoader | secondary_port | reference | fn load(&self) -> Result<LintConfig, LintConfigLoaderError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunCatalogueLint | application_service | reference | fn execute(&self, command: RunCatalogueLintCommand) -> Result<Vec<CatalogueLintViolation>, RunCatalogueLintError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunCatalogueLintInteractor | interactor | modify | — | 🟡 | 🔵 |

