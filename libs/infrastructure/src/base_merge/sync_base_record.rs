//! Codec for the versioned `.sync-base.json` persistence record.

use domain::branch_strategy::BaseBranchName;
use domain::{CommitHash, TrackId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Persistence schema supported for a sync-base record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncBaseRecordSchemaVersion {
    /// Initial schema containing the complete synchronization identity.
    V1,
}

/// Schema-versioned persistence DTO for an active track's synchronization state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncBaseRecord {
    /// Schema used to encode this record.
    pub schema_version: SyncBaseRecordSchemaVersion,
    /// Active track identity.
    pub track_id: TrackId,
    /// Base branch incorporated by the guarded merge.
    pub base_branch: BaseBranchName,
    /// Exact base commit incorporated by the guarded merge.
    pub base_commit: CommitHash,
}

/// Codec failures at the persistence boundary.
#[derive(Debug, Error)]
pub(crate) enum SyncBaseRecordCodecError {
    /// JSON serialization failed.
    #[error("cannot encode sync-base record: {0}")]
    Encode(serde_json::Error),
    /// JSON deserialization failed.
    #[error("cannot decode sync-base record: {0}")]
    Decode(serde_json::Error),
    /// The record names an unsupported schema version.
    #[error("unsupported sync-base record schema version")]
    UnsupportedSchemaVersion,
    /// Track identity did not pass domain validation.
    #[error("invalid sync-base record track id")]
    InvalidTrackId,
    /// Base branch did not pass domain validation.
    #[error("invalid sync-base record base branch")]
    InvalidBaseBranch,
    /// Base commit did not pass domain validation.
    #[error("invalid sync-base record base commit")]
    InvalidBaseCommit,
}

#[derive(Serialize)]
struct SyncBaseRecordEncodeDto<'a> {
    schema_version: SyncBaseRecordSchemaVersion,
    track_id: &'a str,
    base_branch: &'a str,
    base_commit: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncBaseRecordDecodeDto {
    schema_version: SyncBaseRecordSchemaVersion,
    track_id: String,
    base_branch: String,
    base_commit: String,
}

impl SyncBaseRecord {
    fn as_encode_dto(&self) -> SyncBaseRecordEncodeDto<'_> {
        SyncBaseRecordEncodeDto {
            schema_version: self.schema_version,
            track_id: self.track_id.as_ref(),
            base_branch: self.base_branch.as_str(),
            base_commit: self.base_commit.as_ref(),
        }
    }

    fn try_from_decode_dto(dto: SyncBaseRecordDecodeDto) -> Result<Self, SyncBaseRecordCodecError> {
        if dto.schema_version != SyncBaseRecordSchemaVersion::V1 {
            return Err(SyncBaseRecordCodecError::UnsupportedSchemaVersion);
        }
        let track_id =
            TrackId::try_new(dto.track_id).map_err(|_| SyncBaseRecordCodecError::InvalidTrackId)?;
        let base_branch = BaseBranchName::try_new(dto.base_branch)
            .map_err(|_| SyncBaseRecordCodecError::InvalidBaseBranch)?;
        let base_commit = CommitHash::try_new(dto.base_commit)
            .map_err(|_| SyncBaseRecordCodecError::InvalidBaseCommit)?;
        Ok(Self { schema_version: dto.schema_version, track_id, base_branch, base_commit })
    }
}

impl Serialize for SyncBaseRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.as_encode_dto().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SyncBaseRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let dto = SyncBaseRecordDecodeDto::deserialize(deserializer)?;
        Self::try_from_decode_dto(dto).map_err(serde::de::Error::custom)
    }
}

/// Encodes a validated sync-base record using its declared schema version.
pub(crate) fn encode(record: &SyncBaseRecord) -> Result<String, SyncBaseRecordCodecError> {
    serde_json::to_string(record).map_err(SyncBaseRecordCodecError::Encode)
}

/// Decodes a sync-base record, rejecting unsupported schemas and invalid fields.
pub(crate) fn decode(input: &str) -> Result<SyncBaseRecord, SyncBaseRecordCodecError> {
    let dto = serde_json::from_str(input).map_err(SyncBaseRecordCodecError::Decode)?;
    SyncBaseRecord::try_from_decode_dto(dto)
}
