use std::collections::{HashMap, HashSet};

use globset::{GlobBuilder, GlobMatcher};

use super::error::ScopeConfigError;
use super::types::{FilePath, MainScopeName, ScopeName};
use crate::TrackId;

/// Scope classification configuration for review.
///
/// Maps named scopes to glob patterns and provides pure classification logic.
/// The `other` scope is implicit — files matching no named scope go to `Other`.
///
/// ADR: `ReviewScopeConfig` is a domain type (pure classification, no I/O).
#[derive(Debug)]
pub struct ReviewScopeConfig {
    scopes: HashMap<MainScopeName, ScopeEntry>,
    operational: Vec<GlobMatcher>,
    /// Broad glob matchers for other-track exclusion.
    /// `<other-track>` is expanded to `*` (any segment).
    other_track_matchers: Vec<GlobMatcher>,
    /// Prefix of the current track's items dir (e.g. `track/items/my-track/`).
    /// Used to post-filter: a path matches other_track only if it does NOT
    /// start with this prefix.
    current_track_prefix: String,
    /// Global default for per-scope diff ceiling (lines), used by
    /// [`Self::diff_ceiling_for_scope`] when a configured scope has no per-scope
    /// override. `None` means no global default — scopes without an override
    /// return `None` (unconstrained).
    default_diff_ceiling: Option<u32>,
}

/// One named scope's classification matchers, optional briefing file, and
/// optional per-scope diff ceiling override.
///
/// Crate-private. Access the briefing path externally via
/// [`ReviewScopeConfig::briefing_file_for_scope`] and the effective ceiling via
/// [`ReviewScopeConfig::diff_ceiling_for_scope`].
#[derive(Debug)]
struct ScopeEntry {
    matchers: Vec<GlobMatcher>,
    /// Workspace-relative path to a scope-specific briefing markdown file.
    /// `None` means no scope-specific briefing (the reviewer uses the main briefing only).
    briefing_file: Option<String>,
    /// Per-scope override for diff ceiling (lines). `None` means inherit the
    /// global default (`ReviewScopeConfig::default_diff_ceiling`).
    diff_ceiling: Option<u32>,
}

impl ReviewScopeConfig {
    /// Builds a scope config from review-scope.json data.
    ///
    /// Expands `<track-id>` placeholder in group patterns, `operational`
    /// patterns, and `other_track` patterns before compiling each glob.
    /// `<other-track>` is expanded to `*` (broad wildcard); post-filtered to
    /// exclude the current track.
    ///
    /// Each entry in `entries` is `(name, patterns, briefing_file, diff_ceiling)`.
    /// `briefing_file` is a workspace-relative path to a scope-specific
    /// severity policy markdown file; the loader does not read the file — it
    /// is fetched by the reviewer's own Read tool at review time (ADR
    /// 2026-04-18-1354 §D4). `diff_ceiling` is the optional per-scope batch
    /// sizing ceiling (lines) used by [`Self::diff_ceiling_for_scope`]; `None`
    /// means inherit `default_diff_ceiling`.
    ///
    /// `default_diff_ceiling` is the global fallback used when a configured
    /// scope has no per-scope override. `None` means no global default (the
    /// resulting `diff_ceiling_for_scope` is unconstrained for scopes without
    /// an override). The implicit `ScopeName::Other` never inherits this
    /// default (see `diff_ceiling_for_scope`).
    ///
    /// # Errors
    /// Returns `ScopeConfigError` on invalid scope names or glob patterns.
    #[allow(clippy::type_complexity)] // entries tuple is the loader↔domain seam; a newtype would force a public schema migration disproportionate to its value here.
    pub fn new(
        track_id: &TrackId,
        entries: Vec<(String, Vec<String>, Option<String>, Option<u32>)>,
        operational: Vec<String>,
        other_track: Vec<String>,
        default_diff_ceiling: Option<u32>,
    ) -> Result<Self, ScopeConfigError> {
        // Build named scopes
        let mut scopes = HashMap::new();
        for (name, patterns, briefing_file, diff_ceiling) in entries {
            let scope_name = MainScopeName::new(name)?;
            let matchers = patterns
                .iter()
                .map(|pat| {
                    let expanded = expand_track_id(pat, track_id);
                    compile_glob(&expanded).map_err(|source| ScopeConfigError::InvalidPattern {
                        pattern: expanded,
                        source,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            scopes.insert(scope_name, ScopeEntry { matchers, briefing_file, diff_ceiling });
        }

        // Compile operational matchers with placeholder expansion
        let operational = operational
            .iter()
            .map(|pat| {
                let expanded = expand_track_id(pat, track_id);
                compile_glob(&expanded).map_err(|source| {
                    ScopeConfigError::InvalidOperationalPattern { pattern: expanded, source }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Compile other_track matchers: <other-track> → * (broad), post-filtered by prefix
        let other_track_matchers = other_track
            .iter()
            .map(|pat| {
                let expanded = expand_other_track(pat, track_id);
                compile_glob(&expanded).map_err(|source| {
                    ScopeConfigError::InvalidOtherTrackPattern { pattern: expanded, source }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let current_track_prefix = format!("track/items/{}/", track_id.as_ref());

        Ok(Self {
            scopes,
            operational,
            other_track_matchers,
            current_track_prefix,
            default_diff_ceiling,
        })
    }

    /// Returns the effective diff ceiling (lines) for the given scope.
    ///
    /// Returns the per-scope override if set; otherwise returns the global
    /// default supplied to [`Self::new`]. Returns `None` if neither is
    /// configured (unconstrained batch sizing for that scope).
    ///
    /// `ScopeName::Other` always returns `None`: the implicit Other scope is
    /// not a configured review scope and cannot carry a per-scope ceiling, nor
    /// does it inherit the global default. Callers that need a ceiling for
    /// `Other` should treat its absence as "no ceiling applied" (D3 / IN-04 /
    /// IN-05 / AC-03).
    #[must_use]
    pub fn diff_ceiling_for_scope(&self, scope: &ScopeName) -> Option<u32> {
        match scope {
            ScopeName::Other => None,
            ScopeName::Main(name) => self
                .scopes
                .get(name)
                .and_then(|entry| entry.diff_ceiling.or(self.default_diff_ceiling)),
        }
    }

    /// Classifies files into scopes.
    ///
    /// - Files matching `operational` or `other_track` patterns are excluded first.
    /// - Each remaining file is matched against named scopes.
    /// - A file matching multiple named scopes is included in **both** (ADR: independent review).
    /// - Files matching no named scope go to `Other`.
    #[must_use]
    pub fn classify(&self, files: &[FilePath]) -> HashMap<ScopeName, Vec<FilePath>> {
        let mut result: HashMap<ScopeName, Vec<FilePath>> = HashMap::new();

        for file in files {
            let s = file.as_str();

            // Exclude operational files
            if self.operational.iter().any(|m| m.is_match(s)) {
                continue;
            }
            // Exclude other-track files: glob matches AND path is NOT the current track
            if self.is_other_track(s) {
                continue;
            }

            // Match against named scopes
            let mut matched = false;
            for (name, entry) in &self.scopes {
                if entry.matchers.iter().any(|m| m.is_match(s)) {
                    result.entry(ScopeName::Main(name.clone())).or_default().push(file.clone());
                    matched = true;
                }
            }

            // Unmatched → Other
            if !matched {
                result.entry(ScopeName::Other).or_default().push(file.clone());
            }
        }

        result
    }

    /// Returns the set of scope names that have files in the given list.
    #[must_use]
    pub fn get_scope_names(&self, files: &[FilePath]) -> HashSet<ScopeName> {
        self.classify(files).into_keys().collect()
    }

    /// Returns `true` if the given scope name is defined in this config.
    ///
    /// `Other` always returns `true` (implicit scope).
    #[must_use]
    pub fn contains_scope(&self, scope: &ScopeName) -> bool {
        match scope {
            ScopeName::Other => true,
            ScopeName::Main(name) => self.scopes.contains_key(name),
        }
    }

    /// Returns all scope names defined in this config, including `Other`.
    #[must_use]
    pub fn all_scope_names(&self) -> HashSet<ScopeName> {
        let mut names: HashSet<ScopeName> =
            self.scopes.keys().map(|k| ScopeName::Main(k.clone())).collect();
        names.insert(ScopeName::Other);
        names
    }

    /// Returns the workspace-relative path to the scope-specific briefing file,
    /// or `None` if no briefing is configured for this scope.
    ///
    /// Always returns `None` for `ScopeName::Other` — the reserved scope has no
    /// briefing by design (ADR 2026-04-18-1354 §D5).
    ///
    /// The returned path is a raw string; no file I/O or existence check is
    /// performed here. The reviewer's sandbox Read tool fetches the file at
    /// review time (ADR §D4).
    #[must_use]
    pub fn briefing_file_for_scope(&self, scope: &ScopeName) -> Option<&str> {
        match scope {
            ScopeName::Other => None,
            ScopeName::Main(name) => {
                self.scopes.get(name).and_then(|entry| entry.briefing_file.as_deref())
            }
        }
    }

    /// Checks if a path matches other_track patterns AND is not the current track.
    fn is_other_track(&self, path: &str) -> bool {
        if self.other_track_matchers.is_empty() {
            return false;
        }
        // Only exclude if the path is NOT under the current track's items dir
        if path.starts_with(&self.current_track_prefix) {
            return false;
        }
        self.other_track_matchers.iter().any(|m| m.is_match(path))
    }
}

/// Compiles a glob pattern with literal separator disabled
/// (allows `**` to match across `/`).
fn compile_glob(pattern: &str) -> Result<GlobMatcher, globset::Error> {
    let glob = GlobBuilder::new(pattern).literal_separator(false).build()?;
    Ok(glob.compile_matcher())
}

/// Expands `<track-id>` placeholder with the current track ID.
fn expand_track_id(pattern: &str, track_id: &TrackId) -> String {
    pattern.replace("<track-id>", track_id.as_ref())
}

/// Expands `<other-track>` to `*` (broad wildcard) and `<track-id>` to the current ID.
/// Post-filtering in `is_other_track()` ensures the current track is not excluded.
fn expand_other_track(pattern: &str, track_id: &TrackId) -> String {
    pattern.replace("<other-track>", "*").replace("<track-id>", track_id.as_ref())
}
