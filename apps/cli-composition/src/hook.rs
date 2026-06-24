//! `hook` command family — per-context composition root.
//!
//! The composition root constructs the wired `HookDriver` with process
//! environment values injected at construction time (CN-02).

use std::path::{Path, PathBuf};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `hook` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct HookCompositionRoot;

impl HookCompositionRoot {
    /// Create a new `HookCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for HookCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

fn hooks_path_configured() -> bool {
    infrastructure::verify::hooks_path::verify(Path::new(".")).is_ok()
}

impl HookCompositionRoot {
    /// Build a wired [`cli_driver::hook::HookDriver`] for the hook family.
    ///
    /// Reads process environment values here (composition root responsibility per CN-02)
    /// and passes them to the use-case interactor.
    pub fn hook_driver(&self) -> cli_driver::hook::HookDriver {
        use infrastructure::shell::ConchShellParser;
        use usecase::hook_dispatch::HookDispatchInteractor;

        let guarded_git_token_present = std::env::var("SOTP_GUARDED_GIT").is_ok();
        let hooks_path_configured = hooks_path_configured();
        let project_dir = std::env::var("CLAUDE_PROJECT_DIR").ok().map(PathBuf::from);

        let parser_port = Arc::new(ConchShellParser);
        let service = Arc::new(HookDispatchInteractor::new(
            parser_port,
            project_dir,
            guarded_git_token_present,
            hooks_path_configured,
        ));

        cli_driver::hook::HookDriver::new(service)
    }
}
