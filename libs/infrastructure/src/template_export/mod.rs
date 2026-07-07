//! Infrastructure adapters for the template extraction boundary (spec IN-02,
//! AC-02, CN-02).
//!
//! [`codec`] provides the fail-closed serde decoder for the boundary manifest
//! (the machine-readable boundary SSoT; ADR D4). Filesystem adapters that read
//! the manifest and drive export are added alongside this module in a later
//! task and re-exported here.

pub mod codec;

pub use codec::{
    MANIFEST_SCHEMA_VERSION, TemplateBoundaryManifestCodecError, TemplateBoundaryManifestDto,
    TemplatePathClassificationDto, TemplatePathEntryDto, decode_manifest,
};
