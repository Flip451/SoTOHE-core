// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `file` command family — primary adapter driver.
//!
//! `FileDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The logic here mirrors
//! `apps/cli-composition/src/file.rs`; T021 removes the `cli_composition`
//! duplicate when the live path is flipped.

// TODO(T021): add infrastructure imports once Cargo.toml is materialized.
// use std::path::PathBuf;
// use infrastructure::track::atomic_write::atomic_write_file;

use std::path::PathBuf;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `file` command family.
pub enum FileInput {
    /// Atomically write `content` to `path` (tmp + fsync + rename).
    WriteAtomic {
        /// Destination file path.
        path: PathBuf,
        /// Bytes to write.
        content: Vec<u8>,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `file` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct FileDriver {
    // TODO(T021): inject use-case interactors here (currently this family has
    // no injectable adapter dependencies — infrastructure functions are called
    // inline, same as cli_composition::FileCompositionRoot).
}

impl FileDriver {
    /// Create a new `FileDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a file command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: FileInput) -> CommandOutcome {
        match input {
            FileInput::WriteAtomic { path, content } => self.file_write_atomic(path, content),
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/file.rs;
    // T021 removes the cli_composition copy).
    // -----------------------------------------------------------------------

    fn file_write_atomic(&self, _path: PathBuf, _content: Vec<u8>) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::track::atomic_write::atomic_write_file here.
        // Mirrors cli_composition/src/file.rs FileCompositionRoot::file_write_atomic.
        CommandOutcome::success(None)
    }
}

impl Default for FileDriver {
    fn default() -> Self {
        Self::new()
    }
}
