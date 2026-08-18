<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PhaseCommandService | application_service | reference | fn validate(&self, command: PhaseValidateCommand) -> Result<(), CommandConfigLoadError>, fn explain(&self, query: PhaseExplainQuery) -> Result<PhaseCommandExplanation, PhaseCommandExplainError>, fn enter(&self, command: PhaseEnterCommand) -> Result<PhaseCommandEnterOutcome, PhaseCommandEnterError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DeriveTestObligationsInteractor | interactor | reference | — | 🔵 | 🔵 |
| EvaluateTestObligationsInteractor | interactor | reference | — | 🔵 | 🔵 |

