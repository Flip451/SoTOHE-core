<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckApprovalVerdict | enum | — | Approved, Blocked | 🟡 | 🔵 |
| DryCheckVerdict | enum | — | NotAViolation, Accepted, Violation | 🟡 | 🔵 |
| VerdictFilter | enum | — | All, NotAViolation, Accepted, Violation | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodeFragment | value_object | modify | — | 🟡 | 🔵 |
| DiffFileHunks | value_object | — | — | 🟡 | 🔵 |
| DiffHunkRange | value_object | — | — | 🟡 | 🔵 |
| DryCheckEntry | value_object | — | — | 🟡 | 🔵 |
| DryCheckFinding | value_object | — | — | 🟡 | 🔵 |
| DryCheckPairKey | value_object | — | — | 🟡 | 🔵 |
| DryCheckRecord | value_object | — | — | 🟡 | 🔵 |
| FragmentContentHash | value_object | — | — | 🟡 | 🔵 |
| FragmentRef | value_object | — | — | 🟡 | 🔵 |
| Rationale | value_object | — | — | 🟡 | 🔵 |
| RefactorProposal | value_object | — | — | 🟡 | 🔵 |
| Timestamp | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DiffFileHunksError | error_type | — | EmptyHunks | 🟡 | 🔵 |
| DiffHunkRangeError | error_type | — | StartExceedsEnd, ZeroLine | 🟡 | 🔵 |
| DryCheckEntryError | error_type | — | ChangedPathOutsidePair | 🟡 | 🔵 |
| DryCheckFindingError | error_type | — | EmptyProposal | 🟡 | 🔵 |
| DryCheckPairKeyError | error_type | — | SelfMatch | 🟡 | 🔵 |
| DryCheckReaderError | error_type | — | Io, SymlinkDetected, Codec, InvalidData, IncompatibleSchema | 🟡 | 🔵 |
| DryCheckRecordError | error_type | — | ChangedPathOutsidePair | 🟡 | 🔵 |
| DryCheckWriterError | error_type | — | Io, SymlinkDetected, Codec, IncompatibleSchema | 🟡 | 🔵 |
| FragmentContentHashError | error_type | — | InvalidFormat | 🟡 | 🔵 |
| RationaleError | error_type | — | Empty | 🟡 | 🔵 |
| RefactorProposalError | error_type | — | Empty | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckReader | secondary_port | — | fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> | 🟡 | 🔵 |
| DryCheckWriter | secondary_port | — | fn append_record(&self, entry: &DryCheckEntry) -> Result<(), DryCheckWriterError> | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::dry_check::fragments_overlapping_hunks | free_function | — | fn(fragments: &[domain::semantic_dup::CodeFragment], changed_hunks: &[DiffFileHunks]) -> Vec<domain::semantic_dup::CodeFragment> | 🟡 | 🔵 |

