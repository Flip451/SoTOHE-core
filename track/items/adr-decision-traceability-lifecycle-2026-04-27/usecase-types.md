<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| VerifyAdrSignalsError | error_type | — | AdrFileListing, AdrFileRead | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| VerifyAdrSignals | application_service | — | fn verify(&self, command: VerifyAdrSignalsCommand) -> Result<AdrVerifyReport, VerifyAdrSignalsError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| VerifyAdrSignalsInteractor | interactor | — | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| VerifyAdrSignalsCommand | command | — | — | 🔵 | 🔵 |

