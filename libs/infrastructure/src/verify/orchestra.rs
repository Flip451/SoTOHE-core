//! Verify Claude orchestra hooks, permissions, and agent definitions.
//!
//! Rust port of `scripts/verify_orchestra_guardrails.py`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use domain::verify::{Finding, VerifyOutcome};

// ---------------------------------------------------------------------------
// File paths
// ---------------------------------------------------------------------------

const SETTINGS_PATH: &str = ".claude/settings.json";
const SETTINGS_LOCAL_PATH: &str = ".claude/settings.local.json";
const PERMISSION_EXTENSIONS_PATH: &str = ".claude/permission-extensions.json";
const AGENTS_DIR: &str = ".claude/agents";

const REQUIRED_AGENT_FILES: &[&str] = &["orchestrator.md", "rust-implementation-lead.md"];

// ---------------------------------------------------------------------------
// Static regexes (known-valid patterns compiled once)
// ---------------------------------------------------------------------------

// These three patterns are verified-valid at design time.
// We store them as `Option<Regex>` to avoid `expect()`/`unwrap()` in non-test code.
// Call sites treat `None` as "pattern unavailable — skip regex check".
static EXTRA_CARGO_MAKE_ALLOW_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"^Bash\(cargo make (?P<task>[A-Za-z0-9][A-Za-z0-9_-]*)(?::\*)?\)$").ok()
});

static EXTRA_GIT_ALLOW_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^Bash\(git (?P<subcommand>[a-z][a-z-]*)(?::\*)?\)$").ok());

static HARDCODED_CODEX_MODEL_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"gpt-\d+").ok());

// ---------------------------------------------------------------------------
// Constants: expected hook paths (path -> label)
// ---------------------------------------------------------------------------

const EXPECTED_HOOK_PATHS: &[(&str, &str)] = &[
    (".claude/hooks/check-codex-before-write.py", "codex-before-write hook"),
    (".claude/hooks/suggest-gemini-research.py", "gemini-research hook"),
    (".claude/hooks/error-to-codex.py", "error-to-codex hook"),
    (".claude/hooks/post-test-analysis.py", "post-test-analysis hook"),
    (".claude/hooks/check-codex-after-plan.py", "codex-after-plan hook"),
    (".claude/hooks/log-cli-tools.py", "log-cli-tools hook"),
    (".claude/hooks/lint-on-save.py", "lint-on-save hook"),
    (".claude/hooks/python-lint-on-save.py", "python-lint-on-save hook"),
    (".claude/hooks/post-implementation-review.py", "post-implementation-review hook"),
];

// ---------------------------------------------------------------------------
// Constants: expected hook commands (label -> required fragments)
// ---------------------------------------------------------------------------

const EXPECTED_HOOK_COMMANDS: &[(&str, &[&str])] = &[
    (
        "direct git ops block hook",
        &[
            "SOTP_CLI_BINARY:-",
            "$CLAUDE_PROJECT_DIR/bin/sotp",
            "hook dispatch block-direct-git-ops",
            "|| exit 2",
        ],
    ),
    (
        "skill compliance hook",
        &[
            "SOTP_CLI_BINARY:-",
            "$CLAUDE_PROJECT_DIR/bin/sotp",
            "hook dispatch skill-compliance",
            "|| exit 0",
        ],
    ),
];

// ---------------------------------------------------------------------------
// Constants: expected allow (entry -> label)
// ---------------------------------------------------------------------------

const EXPECTED_OTHER_ALLOW: &[(&str, &str)] = &[
    ("Read(./**)", "repo read permission"),
    ("Edit(./**)", "repo edit permission"),
    ("Write(./**)", "repo write permission"),
    ("Glob(./**)", "repo glob permission"),
    ("Grep(./**)", "repo grep permission"),
    ("Task(*)", "task permission"),
    ("Skill(*)", "skill permission"),
    ("TodoWrite(*)", "todo write permission"),
    ("Bash(codex:*)", "codex permission"),
    ("Bash(gemini:*)", "gemini permission"),
    ("Bash(tree:*)", "tree permission"),
    ("Bash(which:*)", "which permission"),
    ("Bash(true)", "true permission"),
    ("Bash(head :*)", "head read-only permission"),
    ("Bash(tail :*)", "tail read-only permission"),
    ("Bash(wc :*)", "wc read-only permission"),
];

const EXPECTED_GIT_ALLOW: &[(&str, &str)] = &[
    ("Bash(git status:*)", "git status permission"),
    ("Bash(git diff:*)", "git diff permission"),
    ("Bash(git log:*)", "git log permission"),
    ("Bash(git show:*)", "git show permission"),
    ("Bash(git branch --list:*)", "git branch --list permission"),
    ("Bash(git branch --show-current)", "git branch --show-current permission"),
    ("Bash(git rev-parse:*)", "git rev-parse permission"),
    ("Bash(git ls-files:*)", "git ls-files permission"),
    ("Bash(git notes show:*)", "git notes show permission"),
    ("Bash(git notes list:*)", "git notes list permission"),
];

const EXPECTED_CARGO_MAKE_ALLOW: &[(&str, &str)] = &[
    ("Bash(cargo make help)", "cargo make help permission"),
    ("Bash(cargo make bootstrap)", "cargo make bootstrap permission"),
    ("Bash(cargo make build-tools)", "cargo make build-tools permission"),
    ("Bash(cargo make build-dev)", "cargo make build-dev permission"),
    ("Bash(cargo make up)", "cargo make up permission"),
    ("Bash(cargo make down)", "cargo make down permission"),
    ("Bash(cargo make logs)", "cargo make logs permission"),
    ("Bash(cargo make ps)", "cargo make ps permission"),
    ("Bash(cargo make shell)", "cargo make shell permission"),
    ("Bash(cargo make tools-up)", "cargo make tools-up permission"),
    ("Bash(cargo make tools-down)", "cargo make tools-down permission"),
    ("Bash(cargo make fmt-exec)", "cargo make fmt-exec permission"),
    ("Bash(cargo make clippy-exec)", "cargo make clippy-exec permission"),
    ("Bash(cargo make test-exec)", "cargo make test-exec permission"),
    ("Bash(cargo make test-one-exec:*)", "cargo make test-one-exec permission"),
    ("Bash(cargo make check-exec)", "cargo make check-exec permission"),
    ("Bash(cargo make machete-exec)", "cargo make machete-exec permission"),
    ("Bash(cargo make deny-exec)", "cargo make deny-exec permission"),
    ("Bash(cargo make llvm-cov-exec)", "cargo make llvm-cov-exec permission"),
    ("Bash(cargo make fmt)", "cargo make fmt permission"),
    ("Bash(cargo make fmt-check)", "cargo make fmt-check permission"),
    ("Bash(cargo make clippy)", "cargo make clippy permission"),
    ("Bash(cargo make test)", "cargo make test permission"),
    ("Bash(cargo make test-doc)", "cargo make test-doc permission"),
    ("Bash(cargo make test-nocapture)", "cargo make test-nocapture permission"),
    ("Bash(cargo make bacon)", "cargo make bacon permission"),
    ("Bash(cargo make bacon-test)", "cargo make bacon-test permission"),
    ("Bash(cargo make check)", "cargo make check permission"),
    ("Bash(cargo make deny)", "cargo make deny permission"),
    ("Bash(cargo make machete)", "cargo make machete permission"),
    ("Bash(cargo make clippy-tests)", "cargo make clippy-tests permission"),
    ("Bash(cargo make llvm-cov)", "cargo make llvm-cov permission"),
    ("Bash(cargo make check-layers)", "cargo make check-layers permission"),
    ("Bash(cargo make architecture-rules-verify-sync)", "architecture rules wrapper permission"),
    ("Bash(cargo make python-lint)", "cargo make python-lint permission"),
    ("Bash(cargo make ci-rust)", "cargo make ci-rust permission"),
    ("Bash(cargo make ci)", "cargo make ci permission"),
    ("Bash(cargo make verify-arch-docs)", "cargo make verify-arch-docs permission"),
    ("Bash(cargo make verify-plan-progress)", "cargo make verify-plan-progress permission"),
    ("Bash(cargo make verify-track-metadata)", "cargo make verify-track-metadata permission"),
    ("Bash(cargo make verify-tech-stack)", "cargo make verify-tech-stack permission"),
    ("Bash(cargo make verify-orchestra)", "cargo make verify-orchestra permission"),
    ("Bash(cargo make verify-latest-track)", "cargo make verify-latest-track permission"),
    ("Bash(cargo make verify-track-registry)", "cargo make verify-track-registry permission"),
    ("Bash(cargo make scripts-selftest)", "cargo make scripts-selftest permission"),
    ("Bash(cargo make hooks-selftest)", "cargo make hooks-selftest permission"),
    ("Bash(cargo make guides-selftest)", "cargo make guides-selftest permission"),
    ("Bash(cargo make add-all)", "cargo make add-all permission"),
    ("Bash(cargo make track-add-paths)", "cargo make track-add-paths permission"),
    ("Bash(cargo make track-commit-message)", "cargo make track-commit-message permission"),
    ("Bash(cargo make track-note)", "cargo make track-note permission"),
    ("Bash(cargo make track-transition:*)", "cargo make track-transition permission"),
    ("Bash(cargo make track-sync-views:*)", "cargo make track-sync-views permission"),
    ("Bash(cargo make track-branch-create:*)", "cargo make track-branch-create permission"),
    ("Bash(cargo make track-branch-switch:*)", "cargo make track-branch-switch permission"),
    ("Bash(cargo make track-activate:*)", "cargo make track-activate permission"),
    ("Bash(cargo make track-plan-branch:*)", "cargo make track-plan-branch permission"),
    ("Bash(cargo make track-resolve:*)", "cargo make track-resolve permission"),
    (
        "Bash(cargo make architecture-rules-workspace-members)",
        "architecture rules workspace-members permission",
    ),
    ("Bash(cargo make workspace-tree)", "architecture rules workspace-tree permission"),
    ("Bash(cargo make workspace-tree-full)", "architecture rules workspace-tree-full permission"),
    (
        "Bash(cargo make architecture-rules-direct-checks)",
        "architecture rules direct-checks permission",
    ),
    ("Bash(cargo make conventions-add:*)", "conventions add permission"),
    ("Bash(cargo make conventions-update-index)", "conventions update-index permission"),
    ("Bash(cargo make conventions-verify-index)", "conventions verify-index permission"),
    ("Bash(cargo make guides-add:*)", "guides add permission"),
    ("Bash(cargo make guides-list)", "cargo make guides-list permission"),
    ("Bash(cargo make guides-fetch:*)", "guides fetch permission"),
    ("Bash(cargo make guides-clean:*)", "guides clean permission"),
    ("Bash(cargo make guides-usage)", "cargo make guides-usage permission"),
    ("Bash(cargo make guides-setup)", "cargo make guides-setup permission"),
    ("Bash(cargo make track-pr)", "cargo make track-pr permission"),
    ("Bash(cargo make track-pr:*)", "cargo make track-pr wildcard permission"),
    ("Bash(cargo make track-pr-push)", "cargo make track-pr-push permission"),
    ("Bash(cargo make track-pr-ensure)", "cargo make track-pr-ensure permission"),
    ("Bash(cargo make track-pr-review)", "cargo make track-pr-review permission"),
    ("Bash(cargo make track-pr-merge:*)", "cargo make track-pr-merge permission"),
    ("Bash(cargo make track-pr-status:*)", "cargo make track-pr-status permission"),
    ("Bash(cargo make track-local-review:*)", "cargo make track-local-review permission"),
    ("Bash(cargo make track-switch-main)", "cargo make track-switch-main permission"),
];

const FORBIDDEN_ALLOW: &[&str] = &[
    "Bash(git:*)",
    "Bash(git add:*)",
    "Bash(git commit:*)",
    "Bash(git fetch:*)",
    "Bash(git remote:*)",
    "Bash(git tag:*)",
    "Bash(cat:*)",
    "Bash(ls:*)",
    "Bash(find:*)",
    "Bash(grep:*)",
    // head, tail, wc are read-only — moved to allow (WF-35)
    "Bash(sort:*)",
    "Bash(uniq:*)",
    "Bash(diff:*)",
    "Bash(echo:*)",
    "Bash(pwd:*)",
    "Bash(cd:*)",
    "Bash(mkdir:*)",
    "Bash(touch:*)",
    "Bash(cp:*)",
    "Bash(mv:*)",
    "Bash(chmod:*)",
    "Bash(chown:*)",
    "Bash(install:*)",
    "Bash(cargo:*)",
    "Bash(docker:*)",
    "Bash(docker-compose:*)",
    "Bash(rustup:*)",
    "Bash(rustfmt:*)",
    "Bash(jq:*)",
    "Bash(sed:*)",
    "Bash(awk:*)",
    "Bash(env:*)",
    "Bash(xargs:*)",
    "Bash(python3:*)",
    "Bash(pytest:*)",
    "Bash(python3 scripts/architecture_rules.py:*)",
    "Bash(python3 scripts/convention_docs.py:*)",
    "Bash(python3 scripts/external_guides.py:*)",
    "Bash(cargo make add:*)",
    "Bash(cargo make commit:*)",
    "Bash(cargo make note:*)",
    "Bash(cargo make clean)",
    "Bash(cargo make architecture-rules-verify-sync:*)",
];

// ---------------------------------------------------------------------------
// Constants: expected deny (entry -> label)
// ---------------------------------------------------------------------------

const EXPECTED_DENY: &[(&str, &str)] = &[
    ("Read(./.env)", "env file read deny rule"),
    ("Read(./.env.*)", "env wildcard read deny rule"),
    ("Read(./.cache/cargo/**)", "cargo cache read deny rule"),
    ("Grep(./.cache/cargo/**)", "cargo cache grep deny rule"),
    ("Read(./.cache/home/**)", "home cache read deny rule"),
    ("Grep(./.cache/home/**)", "home cache grep deny rule"),
    ("Read(./.cache/pytest/**)", "pytest cache read deny rule"),
    ("Grep(./.cache/pytest/**)", "pytest cache grep deny rule"),
    ("Read(./.cache/sccache/**)", "sccache read deny rule"),
    ("Grep(./.cache/sccache/**)", "sccache grep deny rule"),
    ("Read(./.cache/uv/**)", "uv cache read deny rule"),
    ("Grep(./.cache/uv/**)", "uv cache grep deny rule"),
    ("Read(./**/*.db)", "db file read deny rule"),
    ("Grep(./**/*.db)", "db file grep deny rule"),
    ("Read(./**/*.sqlite)", "sqlite file read deny rule"),
    ("Grep(./**/*.sqlite)", "sqlite file grep deny rule"),
    ("Read(./**/*.pem)", "pem read deny rule"),
    ("Read(./**/*.key)", "key read deny rule"),
    ("Read(./**/credentials*)", "credentials read deny rule"),
    ("Read(./**/*secret*)", "secret-pattern read deny rule"),
    ("Read(./private/**)", "private dir read deny rule"),
    ("Grep(./private/**)", "private dir grep deny rule"),
    ("Read(./config/secrets/**)", "config secrets read deny rule"),
    ("Grep(./config/secrets/**)", "config secrets grep deny rule"),
    ("Read(~/.ssh/**)", "ssh read deny rule"),
    ("Read(~/.aws/**)", "aws read deny rule"),
    ("Bash(rm -rf /)", "rm root deny rule"),
    ("Bash(rm -rf ~)", "rm home deny rule"),
    ("Bash(cargo make --allow-private:*)", "host allow-private deny rule"),
    ("Bash(touch :*)", "file-write touch deny rule"),
    ("Bash(cp :*)", "file-write cp deny rule"),
    ("Bash(mv :*)", "file-write mv deny rule"),
    ("Bash(install :*)", "file-write install deny rule"),
    ("Bash(chmod :*)", "file-write chmod deny rule"),
    ("Bash(chown :*)", "file-write chown deny rule"),
];

// ---------------------------------------------------------------------------
// Constants: subagent model allowlist
// ---------------------------------------------------------------------------

const SUBAGENT_MODEL_ALLOWLIST: &[&str] =
    &["claude-sonnet-4-6", "claude-opus-4-6", "claude-haiku-4-5-20251001"];

// ---------------------------------------------------------------------------
// Constants: allowed extra git subcommands in permission-extensions.json
// ---------------------------------------------------------------------------

const ALLOWED_EXTRA_GIT_SUBCOMMANDS: &[&str] =
    &["cat-file", "describe", "for-each-ref", "merge-base", "name-rev", "rev-list", "show-ref"];

// ---------------------------------------------------------------------------
// Constants: model resolution targets
// (path, label, required_snippets, forbidden_snippets)
// ---------------------------------------------------------------------------

const MODEL_RESOLUTION_TARGETS: &[(&str, &str, &[&str], &[&str])] = &[
    (
        ".claude/skills/codex-system/SKILL.md",
        "codex-system override-first resolution",
        &[
            "profiles.<active_profile>.provider_model_overrides.codex  \u{2192}  {model}",
            "fallback: providers.codex.default_model  \u{2192}  {model}",
        ],
        &[
            "read `providers.codex.default_model` from `.claude/agent-profiles.json` and pass as `--model {model}`",
        ],
    ),
    (
        ".claude/skills/track-plan/SKILL.md",
        "track-plan provider-specific invocation",
        &["Agent tool", "cargo make track-local-plan"],
        &["codex exec --model gpt-5.3-codex --sandbox read-only --full-auto \""],
    ),
    (
        ".claude/commands/track/review.md",
        "track review override-first resolution",
        &[
            "provider_model_overrides",
            "providers.<reviewer_provider>.fast_model",
            "providers.<reviewer_provider>.default_model",
        ],
        &["Read the provider's `default_model` to get `{model}`."],
    ),
];

// ---------------------------------------------------------------------------
// Constants: reviewer wrapper guidance
// (path, label, required_snippets, forbidden_snippets)
// ---------------------------------------------------------------------------

const REVIEW_WRAPPER_TARGETS: &[(&str, &str, &[&str], &[&str])] = &[
    (
        ".claude/agent-profiles.json",
        "agent profile reviewer wrapper path",
        &["cargo make track-local-review -- --model {model} --prompt \\\"{task}\\\""],
        &["codex exec review --uncommitted --json --model {model} --full-auto"],
    ),
    (
        ".claude/commands/track/review.md",
        "track review wrapper path",
        &[
            "cargo make track-local-review -- --model {fast_model} --round-type fast --group",
            "--track-id",
            "--briefing-file tmp/reviewer-runtime/briefing-",
            "{\"verdict\":\"zero_findings\",\"findings\":[]}",
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"describe the bug\",\"severity\":\"P1\",\"file\":\"path/to/file.rs\",\"line\":123}]}",
            "Every object field is required by the output schema.",
            "use `null` for that field instead of omitting it.",
        ],
        &[
            "timeout 180 codex exec --model {model} --sandbox read-only --full-auto",
            "timeout 600 codex exec --model {model} --sandbox read-only --full-auto",
        ],
    ),
    (
        ".claude/skills/codex-system/SKILL.md",
        "codex-system reviewer wrapper path",
        &[
            "cargo make track-local-review -- --model {model} --briefing-file tmp/codex-briefing.md",
            "cargo make track-local-review -- --model {model} --prompt \"",
            "{\"verdict\":\"zero_findings\",\"findings\":[]}",
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"describe the bug\",\"severity\":\"P1\",\"file\":\"path/to/file.rs\",\"line\":123}]}",
            "Every object field is required by the output schema.",
            "use `null` for that field instead of omitting it.",
        ],
        &[
            "timeout 180 codex exec --model {model} --sandbox read-only --full-auto \\\n  \"Review this Rust implementation: {description}\"",
            "timeout 600 codex exec --model {model} --sandbox read-only --full-auto \\\n  \"Review this Rust implementation: {description}\"",
            "codex exec review --uncommitted --json --model {model} --full-auto",
        ],
    ),
    (
        ".claude/rules/02-codex-delegation.md",
        "codex delegation reviewer wrapper path",
        &[
            "cargo make track-local-review -- --model {model} --prompt \\",
            "{\"verdict\":\"zero_findings\",\"findings\":[]}",
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"describe the bug\",\"severity\":\"P1\",\"file\":\"path/to/file.rs\",\"line\":123}]}",
            "field \u{81ea}\u{4f53}\u{306f}\u{7701}\u{7565}\u{305b}\u{305a} `null` \u{3092}\u{4f7f}\u{3046}\u{3002}",
        ],
        &[
            "timeout 180 codex exec --model {model} --sandbox read-only --full-auto \\\n  \"Review this Rust implementation: {description}\"",
            "timeout 600 codex exec --model {model} --sandbox read-only --full-auto \\\n  \"Review this Rust implementation: {description}\"",
            "codex exec review --uncommitted --json --model {model} --full-auto",
        ],
    ),
];

// ---------------------------------------------------------------------------
// Constants: TeammateIdle markers
// ---------------------------------------------------------------------------

const TEAMMATE_IDLE_MARKERS: &[(&str, &str)] = &[
    (
        "parent directory",
        "TeammateIdle feedback instructs creating parent directory before writing",
    ),
    ("agent-teams", "TeammateIdle feedback references agent-teams log directory"),
];

// ---------------------------------------------------------------------------
// Helper: build expected allow map (BTreeMap for determinism)
// ---------------------------------------------------------------------------

fn expected_allow_map() -> BTreeMap<&'static str, &'static str> {
    let mut map = BTreeMap::new();
    for (k, v) in
        EXPECTED_OTHER_ALLOW.iter().chain(EXPECTED_GIT_ALLOW).chain(EXPECTED_CARGO_MAKE_ALLOW)
    {
        map.insert(*k, *v);
    }
    map
}

fn forbidden_allow_set() -> BTreeSet<&'static str> {
    FORBIDDEN_ALLOW.iter().copied().collect()
}

// ---------------------------------------------------------------------------
// Helper: extract cargo make task name / git subcommand from allow entries
// ---------------------------------------------------------------------------

/// Extract the task name from `Bash(cargo make <task>)` or `Bash(cargo make <task>:*)`.
///
/// Returns `None` when the entry does not match the pattern.
fn cargo_make_task_name(entry: &str) -> Option<String> {
    let re = EXTRA_CARGO_MAKE_ALLOW_RE.as_ref()?;
    let caps = re.captures(entry)?;
    let m = caps.name("task")?;
    let start = m.start();
    let end = m.end();
    entry.get(start..end).map(ToOwned::to_owned)
}

/// Extract the subcommand from `Bash(git <subcommand>)` or `Bash(git <subcommand>:*)`.
///
/// Returns `None` when the entry does not match the pattern.
fn git_subcommand_name(entry: &str) -> Option<String> {
    let re = EXTRA_GIT_ALLOW_RE.as_ref()?;
    let caps = re.captures(entry)?;
    let m = caps.name("subcommand")?;
    let start = m.start();
    let end = m.end();
    entry.get(start..end).map(ToOwned::to_owned)
}

/// All cargo make task names that are already reserved by baseline expected or forbidden lists.
fn known_cargo_make_tasks() -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for entry in
        EXPECTED_CARGO_MAKE_ALLOW.iter().map(|(k, _)| *k).chain(FORBIDDEN_ALLOW.iter().copied())
    {
        if let Some(task) = cargo_make_task_name(entry) {
            set.insert(task);
        }
    }
    set
}

/// All git subcommands that are already reserved by baseline expected or forbidden lists.
fn known_git_subcommands() -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for entry in EXPECTED_GIT_ALLOW.iter().map(|(k, _)| *k).chain(FORBIDDEN_ALLOW.iter().copied()) {
        if let Some(sub) = git_subcommand_name(entry) {
            set.insert(sub);
        }
    }
    set
}

// ---------------------------------------------------------------------------
// JSON loading helpers
// ---------------------------------------------------------------------------

/// Load `.claude/settings.json` from `root`.
///
/// # Errors
///
/// Returns `Err` with a descriptive message when the file is missing,
/// contains invalid JSON, or does not decode to a JSON object.
fn load_settings(root: &Path) -> Result<serde_json::Value, String> {
    let path = root.join(SETTINGS_PATH);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("Missing settings file {SETTINGS_PATH}: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid JSON in {SETTINGS_PATH}: {e}"))?;
    if !value.is_object() {
        return Err(format!("{SETTINGS_PATH} must decode to a JSON object"));
    }
    Ok(value)
}

/// Load `extra_allow` string array from `.claude/permission-extensions.json`.
///
/// # Errors
///
/// Returns `Err` with a descriptive message when the file exists but is
/// invalid JSON or has the wrong shape.
fn load_permission_extensions(root: &Path) -> Result<Vec<String>, String> {
    let path = root.join(PERMISSION_EXTENSIONS_PATH);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read {PERMISSION_EXTENSIONS_PATH}: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Invalid JSON in {PERMISSION_EXTENSIONS_PATH}: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| format!("{PERMISSION_EXTENSIONS_PATH} must decode to a JSON object"))?;
    let entries = match obj.get("extra_allow") {
        Some(v) => v.as_array().ok_or_else(|| {
            format!("{PERMISSION_EXTENSIONS_PATH} field 'extra_allow' must be an array of strings")
        })?,
        None => return Ok(Vec::new()), // Python: data.get("extra_allow", [])
    };
    let mut result = Vec::with_capacity(entries.len());
    for item in entries {
        let s = item.as_str().ok_or_else(|| {
            format!("{PERMISSION_EXTENSIONS_PATH} field 'extra_allow' entries must be strings")
        })?;
        result.push(s.to_owned());
    }
    Ok(result)
}

/// Extract all hook `command` strings from `settings["hooks"]`.
///
/// # Errors
///
/// Returns `Err` when the hooks field is missing or has an unexpected shape.
fn hook_commands(settings: &serde_json::Value) -> Result<Vec<String>, String> {
    let hooks = settings
        .get("hooks")
        .and_then(|v| v.as_object())
        .ok_or_else(|| format!("{SETTINGS_PATH} missing object field 'hooks'"))?;
    let mut commands = Vec::new();
    for (_event, bindings_val) in hooks {
        let bindings = bindings_val
            .as_array()
            .ok_or_else(|| "Each hooks event binding list must be an array".to_owned())?;
        for binding in bindings {
            let binding_obj = binding
                .as_object()
                .ok_or_else(|| "Each hooks event binding must be an object".to_owned())?;
            if let Some(hooks_val) = binding_obj.get("hooks") {
                let nested = hooks_val
                    .as_array()
                    .ok_or_else(|| "Each hooks binding must contain a hooks array".to_owned())?;
                for hook in nested {
                    let hook_obj = hook
                        .as_object()
                        .ok_or_else(|| "Each hook entry must be an object".to_owned())?;
                    if let Some(cmd) = hook_obj.get("command").and_then(|v| v.as_str()) {
                        commands.push(cmd.to_owned());
                    }
                }
            }
        }
    }
    Ok(commands)
}

/// Extract `settings["permissions"][key]` as a sorted set of strings.
///
/// # Errors
///
/// Returns `Err` when the field is missing or entries are not strings.
fn permission_set(settings: &serde_json::Value, key: &str) -> Result<BTreeSet<String>, String> {
    let permissions = settings
        .get("permissions")
        .and_then(|v| v.as_object())
        .ok_or_else(|| format!("{SETTINGS_PATH} missing object field 'permissions'"))?;
    let values = permissions
        .get(key)
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("{SETTINGS_PATH} permissions.{key} must be an array"))?;
    let mut set = BTreeSet::new();
    for item in values {
        let s = item
            .as_str()
            .ok_or_else(|| format!("{SETTINGS_PATH} permissions.{key} entries must be strings"))?;
        set.insert(s.to_owned());
    }
    Ok(set)
}

// ---------------------------------------------------------------------------
// Verification sub-functions
// ---------------------------------------------------------------------------

/// Verify hook paths are present in commands and hook files exist on disk.
fn verify_hook_paths(commands: &[String], root: &Path, outcome: &mut VerifyOutcome) {
    for (hook_path, label) in EXPECTED_HOOK_PATHS {
        if !commands.iter().any(|c| c.contains(hook_path)) {
            outcome.add(Finding::error(format!("Missing in {SETTINGS_PATH}: {label}")));
        }
        if !root.join(hook_path).is_file() {
            outcome.add(Finding::error(format!("Missing hook file: {hook_path}")));
        }
    }

    for (label, fragments) in EXPECTED_HOOK_COMMANDS {
        let found = commands.iter().any(|c| fragments.iter().all(|f| c.contains(*f)));
        if !found {
            let frags = fragments.join(", ");
            outcome.add(Finding::error(format!(
                "Missing in {SETTINGS_PATH}: {label} (expected fragments: {frags})"
            )));
        }
    }
}

/// Verify the allow list: expected present, forbidden absent, no unexpected entries.
fn verify_allowlist(allow: &BTreeSet<String>, extra_allow: &[String], outcome: &mut VerifyOutcome) {
    let expected = expected_allow_map();
    let forbidden = forbidden_allow_set();
    let extra_set: BTreeSet<&str> = extra_allow.iter().map(String::as_str).collect();

    for (entry, label) in &expected {
        if !allow.contains(*entry) {
            outcome.add(Finding::error(format!("Missing in {SETTINGS_PATH}: {label}")));
        }
    }

    for entry in &forbidden {
        if allow.contains(*entry) {
            outcome.add(Finding::error(format!(
                "{SETTINGS_PATH} contains {entry} - direct access would be silently allowed"
            )));
        }
    }

    for entry in allow {
        let s = entry.as_str();
        if forbidden.contains(s) {
            continue; // already reported above
        }
        if entry.starts_with("Bash(") && entry.contains("scripts/") {
            outcome.add(Finding::error(format!(
                "{SETTINGS_PATH} contains {entry} - direct repo scripts must be routed \
                through cargo make wrappers"
            )));
            continue;
        }
        if !expected.contains_key(s) && !extra_set.contains(s) {
            outcome.add(Finding::error(format!(
                "{SETTINGS_PATH} contains unexpected allow entry: {entry} - \
                add it to {PERMISSION_EXTENSIONS_PATH} if this project intentionally extends \
                the baseline"
            )));
        }
    }
}

/// Validate entries in `permission-extensions.json` `extra_allow`.
fn validate_permission_extensions(
    extra_allow: &[String],
    allow: &BTreeSet<String>,
    outcome: &mut VerifyOutcome,
) {
    let expected = expected_allow_map();
    let forbidden = forbidden_allow_set();
    let known_tasks = known_cargo_make_tasks();
    let known_git = known_git_subcommands();
    let allowed_git_subs: BTreeSet<&str> = ALLOWED_EXTRA_GIT_SUBCOMMANDS.iter().copied().collect();

    for entry in extra_allow {
        let s = entry.as_str();

        if !allow.contains(s) {
            outcome.add(Finding::error(format!(
                "{PERMISSION_EXTENSIONS_PATH} contains latent extra_allow entry not present in \
                {SETTINGS_PATH} permissions.allow: {entry}"
            )));
            continue;
        }

        if expected.contains_key(s) {
            outcome.add(Finding::error(format!(
                "{PERMISSION_EXTENSIONS_PATH} contains baseline allow entry: {entry} - \
                extra_allow is only for project-specific additions"
            )));
            continue;
        }

        if forbidden.contains(s) {
            outcome.add(Finding::error(format!(
                "{PERMISSION_EXTENSIONS_PATH} contains forbidden extra_allow entry: {entry} - \
                direct access would be silently allowed"
            )));
            continue;
        }

        if entry.starts_with("Bash(") && entry.contains("scripts/") {
            outcome.add(Finding::error(format!(
                "{PERMISSION_EXTENSIONS_PATH} contains {entry} - \
                direct repo scripts must be routed through cargo make wrappers"
            )));
            continue;
        }

        if let Some(task) = cargo_make_task_name(s) {
            if known_tasks.contains(&task) {
                outcome.add(Finding::error(format!(
                    "{PERMISSION_EXTENSIONS_PATH} contains extension for guarded cargo make \
                    task: {entry} - baseline or approval-gated cargo make task names cannot be \
                    widened via extra_allow"
                )));
            }
            // else: valid project cargo make extension
            continue;
        }

        if let Some(sub) = git_subcommand_name(s) {
            if allowed_git_subs.contains(sub.as_str()) {
                if known_git.contains(&sub) {
                    outcome.add(Finding::error(format!(
                        "{PERMISSION_EXTENSIONS_PATH} contains extension for guarded git \
                        subcommand: {entry} - baseline git permissions cannot be widened via \
                        extra_allow"
                    )));
                }
                // else: valid read-only git extension
                continue;
            }
        }

        outcome.add(Finding::error(format!(
            "{PERMISSION_EXTENSIONS_PATH} contains unsupported extra_allow entry: {entry} - \
            only project-specific Bash(cargo make <task>) / Bash(cargo make <task>:*) and \
            whitelisted read-only Bash(git <subcommand>) / Bash(git <subcommand>:*) are allowed"
        )));
    }
}

/// Verify deny list entries are present.
fn verify_denylist(deny: &BTreeSet<String>, outcome: &mut VerifyOutcome) {
    for (entry, label) in EXPECTED_DENY {
        if !deny.contains(*entry) {
            outcome.add(Finding::error(format!("Missing in {SETTINGS_PATH}: {label}")));
        }
    }
}

/// Verify env configuration: agent teams flag and subagent model.
fn verify_env(settings: &serde_json::Value, outcome: &mut VerifyOutcome) {
    let env = match settings.get("env").and_then(|v| v.as_object()) {
        Some(e) => e,
        None => {
            outcome.add(Finding::error(format!("{SETTINGS_PATH} is missing env configuration")));
            return;
        }
    };

    match env.get("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS").and_then(|v| v.as_str()) {
        Some("1") => {}
        _ => {
            outcome.add(Finding::error(format!("Missing in {SETTINGS_PATH}: agent teams enabled")));
        }
    }

    let model = env.get("CLAUDE_CODE_SUBAGENT_MODEL").and_then(|v| v.as_str());
    match model {
        Some(m) if SUBAGENT_MODEL_ALLOWLIST.contains(&m) => {}
        other => {
            let allowlist: Vec<&str> = SUBAGENT_MODEL_ALLOWLIST.to_vec();
            outcome.add(Finding::error(format!(
                "{SETTINGS_PATH}: CLAUDE_CODE_SUBAGENT_MODEL must be one of {allowlist:?}, \
                got {other:?}"
            )));
        }
    }
}

/// Verify TeammateIdle hooks contain required marker phrases.
fn verify_teammate_idle_feedback(settings: &serde_json::Value, outcome: &mut VerifyOutcome) {
    let hooks = match settings.get("hooks").and_then(|v| v.as_object()) {
        Some(h) => h,
        None => return,
    };

    let mut feedback_text = String::new();
    if let Some(idle_bindings) = hooks.get("TeammateIdle").and_then(|v| v.as_array()) {
        for binding in idle_bindings {
            if let Some(nested) = binding.get("hooks").and_then(|v| v.as_array()) {
                for hook in nested {
                    if let Some(cmd) = hook.get("command").and_then(|v| v.as_str()) {
                        feedback_text.push_str(cmd);
                    }
                }
            }
        }
    }

    for (marker, label) in TEAMMATE_IDLE_MARKERS {
        if !feedback_text.contains(marker) {
            outcome.add(Finding::error(format!(
                "Missing in {SETTINGS_PATH} TeammateIdle feedback: {label:?}"
            )));
        }
    }
}

/// Verify required agent definition files exist.
fn verify_agent_definitions(root: &Path, outcome: &mut VerifyOutcome) {
    for required in REQUIRED_AGENT_FILES {
        if !root.join(AGENTS_DIR).join(required).is_file() {
            outcome.add(Finding::error(format!(
                "Missing required agent definition: {AGENTS_DIR}/{required}"
            )));
        }
    }
}

/// Verify no hardcoded Codex model literals in `.claude/skills/` and `.claude/commands/`.
fn verify_no_hardcoded_codex_model_literals(root: &Path, outcome: &mut VerifyOutcome) {
    for dir in &[".claude/skills", ".claude/commands"] {
        let base = root.join(dir);
        if !base.is_dir() {
            continue;
        }
        if let Err(e) = scan_dir_for_gpt_pattern(&base, outcome) {
            outcome.add(Finding::error(format!("Error scanning {dir}: {e}")));
        }
    }
}

fn scan_dir_for_gpt_pattern(dir: &Path, outcome: &mut VerifyOutcome) -> Result<(), String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("Cannot read dir {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    paths.sort();

    for path in paths {
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "__pycache__") {
                continue;
            }
            scan_dir_for_gpt_pattern(&path, outcome)?;
        } else if path.is_file() {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
            if HARDCODED_CODEX_MODEL_RE.as_ref().is_some_and(|re| re.is_match(&text)) {
                outcome.add(Finding::error(format!(
                    "{} contains hardcoded Codex model literal matching {}",
                    path.display(),
                    r"gpt-\d+"
                )));
            }
        }
    }
    Ok(())
}

/// Verify override-first model resolution guidance in target files.
fn verify_override_first_model_resolution(root: &Path, outcome: &mut VerifyOutcome) {
    for (rel_path, label, required_snippets, forbidden_snippets) in MODEL_RESOLUTION_TARGETS {
        let path = root.join(rel_path);
        if !path.is_file() {
            outcome.add(Finding::error(format!("Missing model resolution target: {rel_path}")));
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                outcome.add(Finding::error(format!("Cannot read {rel_path}: {e}")));
                continue;
            }
        };

        let missing: Vec<&&str> =
            required_snippets.iter().filter(|s| !content.contains(**s)).collect();
        if !missing.is_empty() {
            let joined: Vec<&str> = missing.iter().map(|s| **s).collect();
            outcome.add(Finding::error(format!(
                "{rel_path} is missing canonical override-first guidance for {label}: {}",
                joined.join("; ")
            )));
        }

        for forbidden in *forbidden_snippets {
            if content.contains(*forbidden) {
                outcome.add(Finding::error(format!(
                    "{rel_path} still contains stale default_model-only guidance: {forbidden}"
                )));
            }
        }
    }
}

/// Verify reviewer wrapper guidance in target files.
fn verify_reviewer_wrapper_guidance(root: &Path, outcome: &mut VerifyOutcome) {
    for (rel_path, label, required_snippets, forbidden_snippets) in REVIEW_WRAPPER_TARGETS {
        let path = root.join(rel_path);
        if !path.is_file() {
            outcome.add(Finding::error(format!("Missing reviewer wrapper target: {rel_path}")));
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                outcome.add(Finding::error(format!("Cannot read {rel_path}: {e}")));
                continue;
            }
        };

        let missing: Vec<&&str> =
            required_snippets.iter().filter(|s| !content.contains(**s)).collect();
        if !missing.is_empty() {
            let joined: Vec<&str> = missing.iter().map(|s| **s).collect();
            outcome.add(Finding::error(format!(
                "{rel_path} is missing reviewer wrapper guidance for {label}: {}",
                joined.join("; ")
            )));
        }

        for forbidden in *forbidden_snippets {
            if content.contains(*forbidden) {
                outcome.add(Finding::error(format!(
                    "{rel_path} still contains stale reviewer command guidance: {forbidden}"
                )));
            }
        }
    }
}

/// Verify `.claude/settings.local.json` is NOT tracked by git.
fn verify_no_local_settings_committed(root: &Path, outcome: &mut VerifyOutcome) {
    let result = std::process::Command::new("git")
        .args(["ls-files", "--error-unmatch", SETTINGS_LOCAL_PATH])
        .current_dir(root)
        .output();

    match result {
        Err(e) => {
            outcome.add(Finding::error(format!(
                "Cannot run git ls-files to check {SETTINGS_LOCAL_PATH}: {e}"
            )));
        }
        Ok(output) => {
            if output.status.success() {
                outcome.add(Finding::error(format!(
                    "{SETTINGS_LOCAL_PATH} is tracked by git. \
                    Local overrides must not be committed — add to .gitignore and run: \
                    git rm --cached .claude/settings.local.json"
                )));
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let code = output.status.code().unwrap_or(-1);

                if code == 128 && stderr.to_lowercase().contains("not a git repository") {
                    // Not a git repo — that's fine
                } else if code != 1 {
                    outcome.add(Finding::error(format!(
                        "git ls-files failed (exit {code}): {}",
                        stderr.trim()
                    )));
                }
                // exit code 1 = file not tracked = expected state
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Verify Claude orchestra hooks, permissions, and agent definitions.
///
/// # Errors
///
/// Returns findings for every structural violation found in
/// `.claude/settings.json`, `.claude/permission-extensions.json`,
/// and the `.claude/agents/` directory.
pub fn verify(root: &Path) -> VerifyOutcome {
    let mut outcome = VerifyOutcome::pass();

    let settings = match load_settings(root) {
        Ok(s) => s,
        Err(e) => {
            outcome.add(Finding::error(e));
            return outcome;
        }
    };

    let extra_allow = match load_permission_extensions(root) {
        Ok(e) => e,
        Err(e) => {
            outcome.add(Finding::error(e));
            return outcome;
        }
    };

    let commands = match hook_commands(&settings) {
        Ok(c) => c,
        Err(e) => {
            outcome.add(Finding::error(e));
            return outcome;
        }
    };

    let allow = match permission_set(&settings, "allow") {
        Ok(a) => a,
        Err(e) => {
            outcome.add(Finding::error(e));
            return outcome;
        }
    };

    let deny = match permission_set(&settings, "deny") {
        Ok(d) => d,
        Err(e) => {
            outcome.add(Finding::error(e));
            return outcome;
        }
    };

    verify_hook_paths(&commands, root, &mut outcome);
    validate_permission_extensions(&extra_allow, &allow, &mut outcome);
    verify_allowlist(&allow, &extra_allow, &mut outcome);
    verify_denylist(&deny, &mut outcome);
    verify_env(&settings, &mut outcome);
    verify_teammate_idle_feedback(&settings, &mut outcome);
    verify_agent_definitions(root, &mut outcome);
    verify_no_hardcoded_codex_model_literals(root, &mut outcome);
    verify_override_first_model_resolution(root, &mut outcome);
    verify_reviewer_wrapper_guidance(root, &mut outcome);
    verify_no_local_settings_committed(root, &mut outcome);

    outcome
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn write_settings(root: &Path, value: &serde_json::Value) {
        let dir = root.join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), serde_json::to_string(value).unwrap()).unwrap();
    }

    fn all_expected_allow_entries() -> Vec<&'static str> {
        EXPECTED_OTHER_ALLOW
            .iter()
            .chain(EXPECTED_GIT_ALLOW)
            .chain(EXPECTED_CARGO_MAKE_ALLOW)
            .map(|(k, _)| *k)
            .collect()
    }

    // -----------------------------------------------------------------------
    // load_settings
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_settings_missing_file_returns_error() {
        let tmp = TempDir::new().unwrap();
        let result = load_settings(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing settings file"));
    }

    #[test]
    fn test_load_settings_invalid_json_returns_error() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), "not json").unwrap();
        let result = load_settings(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid JSON"));
    }

    #[test]
    fn test_load_settings_non_object_returns_error() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), "[1,2,3]").unwrap();
        let result = load_settings(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("JSON object"));
    }

    #[test]
    fn test_load_settings_valid_returns_ok() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), r#"{"foo":"bar"}"#).unwrap();
        let result = load_settings(tmp.path());
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // load_permission_extensions
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_permission_extensions_missing_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let result = load_permission_extensions(tmp.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_load_permission_extensions_valid_returns_entries() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        let data = json!({"extra_allow": ["Bash(cargo make my-custom-task)"]});
        std::fs::write(
            dir.join("permission-extensions.json"),
            serde_json::to_string(&data).unwrap(),
        )
        .unwrap();
        let result = load_permission_extensions(tmp.path()).unwrap();
        assert_eq!(result, vec!["Bash(cargo make my-custom-task)"]);
    }

    // -----------------------------------------------------------------------
    // hook_commands
    // -----------------------------------------------------------------------

    #[test]
    fn test_hook_commands_extracts_commands() {
        let settings = json!({
            "hooks": {
                "PreToolUse": [{
                    "hooks": [
                        {"command": "echo hello"},
                        {"command": "echo world"}
                    ]
                }]
            }
        });
        let cmds = hook_commands(&settings).unwrap();
        assert_eq!(cmds.len(), 2);
        assert!(cmds.contains(&"echo hello".to_owned()));
        assert!(cmds.contains(&"echo world".to_owned()));
    }

    #[test]
    fn test_hook_commands_missing_hooks_field_returns_error() {
        let settings = json!({"permissions": {}});
        let result = hook_commands(&settings);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // permission_set
    // -----------------------------------------------------------------------

    #[test]
    fn test_permission_set_returns_correct_entries() {
        let settings = json!({
            "permissions": {
                "allow": ["Bash(true)", "Read(./**)"],
                "deny": []
            }
        });
        let allow = permission_set(&settings, "allow").unwrap();
        assert!(allow.contains("Bash(true)"));
        assert!(allow.contains("Read(./**)"));
    }

    // -----------------------------------------------------------------------
    // cargo_make_task_name / git_subcommand_name
    // -----------------------------------------------------------------------

    #[test]
    fn test_cargo_make_task_name_matches_plain_task() {
        assert_eq!(cargo_make_task_name("Bash(cargo make ci)"), Some("ci".to_owned()));
    }

    #[test]
    fn test_cargo_make_task_name_matches_wildcard_task() {
        assert_eq!(
            cargo_make_task_name("Bash(cargo make track-transition:*)"),
            Some("track-transition".to_owned())
        );
    }

    #[test]
    fn test_cargo_make_task_name_returns_none_for_non_matching() {
        assert_eq!(cargo_make_task_name("Bash(git status:*)"), None);
        assert_eq!(cargo_make_task_name("Read(./**)"), None);
    }

    #[test]
    fn test_git_subcommand_name_matches() {
        assert_eq!(git_subcommand_name("Bash(git show:*)"), Some("show".to_owned()));
        assert_eq!(git_subcommand_name("Bash(git rev-parse:*)"), Some("rev-parse".to_owned()));
    }

    #[test]
    fn test_git_subcommand_name_returns_none_for_non_matching() {
        assert_eq!(git_subcommand_name("Bash(cargo make ci)"), None);
    }

    // -----------------------------------------------------------------------
    // verify_env
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_env_passes_with_valid_config() {
        let settings = json!({
            "env": {
                "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1",
                "CLAUDE_CODE_SUBAGENT_MODEL": "claude-sonnet-4-6"
            }
        });
        let mut outcome = VerifyOutcome::pass();
        verify_env(&settings, &mut outcome);
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_verify_env_fails_with_missing_agent_teams() {
        let settings = json!({
            "env": {
                "CLAUDE_CODE_SUBAGENT_MODEL": "claude-sonnet-4-6"
            }
        });
        let mut outcome = VerifyOutcome::pass();
        verify_env(&settings, &mut outcome);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_verify_env_fails_with_unknown_subagent_model() {
        let settings = json!({
            "env": {
                "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1",
                "CLAUDE_CODE_SUBAGENT_MODEL": "gpt-99"
            }
        });
        let mut outcome = VerifyOutcome::pass();
        verify_env(&settings, &mut outcome);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_verify_env_fails_when_env_missing() {
        let settings = json!({});
        let mut outcome = VerifyOutcome::pass();
        verify_env(&settings, &mut outcome);
        assert!(outcome.has_errors());
    }

    // -----------------------------------------------------------------------
    // verify_denylist
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_denylist_passes_when_all_present() {
        let deny: BTreeSet<String> = EXPECTED_DENY.iter().map(|(k, _)| k.to_string()).collect();
        let mut outcome = VerifyOutcome::pass();
        verify_denylist(&deny, &mut outcome);
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_verify_denylist_fails_when_entry_missing() {
        let mut deny: BTreeSet<String> = EXPECTED_DENY.iter().map(|(k, _)| k.to_string()).collect();
        deny.remove("Read(./.env)");
        let mut outcome = VerifyOutcome::pass();
        verify_denylist(&deny, &mut outcome);
        assert!(outcome.has_errors());
    }

    // -----------------------------------------------------------------------
    // verify_allowlist
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_allowlist_passes_with_all_expected_entries() {
        let allow: BTreeSet<String> =
            all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
        let mut outcome = VerifyOutcome::pass();
        verify_allowlist(&allow, &[], &mut outcome);
        assert!(outcome.is_ok(), "errors: {:?}", outcome.findings());
    }

    #[test]
    fn test_verify_allowlist_fails_when_expected_entry_missing() {
        let mut allow: BTreeSet<String> =
            all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
        allow.remove("Bash(true)");
        let mut outcome = VerifyOutcome::pass();
        verify_allowlist(&allow, &[], &mut outcome);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_verify_allowlist_fails_when_forbidden_entry_present() {
        let mut allow: BTreeSet<String> =
            all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
        allow.insert("Bash(git add:*)".to_owned());
        let mut outcome = VerifyOutcome::pass();
        verify_allowlist(&allow, &[], &mut outcome);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_verify_allowlist_fails_on_unexpected_entry() {
        let mut allow: BTreeSet<String> =
            all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
        allow.insert("Bash(some-unknown-tool)".to_owned());
        let mut outcome = VerifyOutcome::pass();
        verify_allowlist(&allow, &[], &mut outcome);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_verify_allowlist_accepts_extra_allow_entries() {
        let mut allow: BTreeSet<String> =
            all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
        allow.insert("Bash(cargo make my-project-task)".to_owned());
        let extra_allow = vec!["Bash(cargo make my-project-task)".to_owned()];
        let mut outcome = VerifyOutcome::pass();
        verify_allowlist(&allow, &extra_allow, &mut outcome);
        assert!(outcome.is_ok(), "errors: {:?}", outcome.findings());
    }

    // -----------------------------------------------------------------------
    // verify_agent_definitions
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_agent_definitions_passes_when_files_exist() {
        let tmp = TempDir::new().unwrap();
        let agents = tmp.path().join(".claude").join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        for f in REQUIRED_AGENT_FILES {
            std::fs::write(agents.join(f), "# agent").unwrap();
        }
        let mut outcome = VerifyOutcome::pass();
        verify_agent_definitions(tmp.path(), &mut outcome);
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_verify_agent_definitions_fails_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let agents = tmp.path().join(".claude").join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        if let Some(f) = REQUIRED_AGENT_FILES.first() {
            std::fs::write(agents.join(f), "# agent").unwrap();
        }
        let mut outcome = VerifyOutcome::pass();
        verify_agent_definitions(tmp.path(), &mut outcome);
        if REQUIRED_AGENT_FILES.len() > 1 {
            assert!(outcome.has_errors());
        }
    }

    // -----------------------------------------------------------------------
    // verify_hook_paths
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_hook_paths_reports_missing_command_fragment() {
        let commands = vec!["echo hello".to_owned()];
        let tmp = TempDir::new().unwrap();
        let mut outcome = VerifyOutcome::pass();
        verify_hook_paths(&commands, tmp.path(), &mut outcome);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_verify_hook_paths_passes_when_all_fragments_present() {
        let tmp = TempDir::new().unwrap();
        // Write all hook files to disk so file-exists check passes
        let hooks_dir = tmp.path().join(".claude").join("hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        for (hook_path, _) in EXPECTED_HOOK_PATHS {
            let full = tmp.path().join(hook_path);
            if let Some(p) = full.parent() {
                std::fs::create_dir_all(p).unwrap();
            }
            std::fs::write(&full, "#!/usr/bin/env python3\n").unwrap();
        }

        // Build a single giant command that contains all needed path fragments and fragments
        let mut big_cmd = String::new();
        for (hook_path, _) in EXPECTED_HOOK_PATHS {
            big_cmd.push_str(hook_path);
            big_cmd.push(' ');
        }
        // Add all EXPECTED_HOOK_COMMANDS fragments
        for (_, fragments) in EXPECTED_HOOK_COMMANDS {
            for f in *fragments {
                big_cmd.push_str(f);
                big_cmd.push(' ');
            }
        }

        let commands = vec![big_cmd];
        let mut outcome = VerifyOutcome::pass();
        verify_hook_paths(&commands, tmp.path(), &mut outcome);
        assert!(outcome.is_ok(), "errors: {:?}", outcome.findings());
    }

    // -----------------------------------------------------------------------
    // verify_teammate_idle_feedback
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_teammate_idle_feedback_passes_when_markers_present() {
        let feedback = "parent directory and agent-teams logs here";
        let settings = json!({
            "hooks": {
                "TeammateIdle": [{
                    "hooks": [{"command": feedback}]
                }]
            }
        });
        let mut outcome = VerifyOutcome::pass();
        verify_teammate_idle_feedback(&settings, &mut outcome);
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_verify_teammate_idle_feedback_fails_when_marker_absent() {
        let settings = json!({
            "hooks": {
                "TeammateIdle": [{
                    "hooks": [{"command": "some other feedback"}]
                }]
            }
        });
        let mut outcome = VerifyOutcome::pass();
        verify_teammate_idle_feedback(&settings, &mut outcome);
        assert!(outcome.has_errors());
    }

    // -----------------------------------------------------------------------
    // verify_no_hardcoded_codex_model_literals
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_no_hardcoded_codex_model_literals_passes_for_clean_files() {
        let tmp = TempDir::new().unwrap();
        let skills = tmp.path().join(".claude").join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(skills.join("clean.md"), "Use `{model}` from profiles.").unwrap();
        let mut outcome = VerifyOutcome::pass();
        verify_no_hardcoded_codex_model_literals(tmp.path(), &mut outcome);
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_verify_no_hardcoded_codex_model_literals_fails_on_gpt_literal() {
        let tmp = TempDir::new().unwrap();
        let skills = tmp.path().join(".claude").join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(skills.join("bad.md"), "Use gpt-4 for this task.").unwrap();
        let mut outcome = VerifyOutcome::pass();
        verify_no_hardcoded_codex_model_literals(tmp.path(), &mut outcome);
        assert!(outcome.has_errors());
    }

    // -----------------------------------------------------------------------
    // validate_permission_extensions
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_extensions_accepts_valid_project_task() {
        let mut allow: BTreeSet<String> =
            all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
        let extra_entry = "Bash(cargo make my-project-task)".to_owned();
        allow.insert(extra_entry.clone());
        let extra_allow = vec![extra_entry];
        let mut outcome = VerifyOutcome::pass();
        validate_permission_extensions(&extra_allow, &allow, &mut outcome);
        assert!(outcome.is_ok(), "errors: {:?}", outcome.findings());
    }

    #[test]
    fn test_validate_extensions_rejects_forbidden_entry() {
        let mut allow: BTreeSet<String> =
            all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
        let forbidden_entry = "Bash(git add:*)".to_owned();
        allow.insert(forbidden_entry.clone());
        let extra_allow = vec![forbidden_entry];
        let mut outcome = VerifyOutcome::pass();
        validate_permission_extensions(&extra_allow, &allow, &mut outcome);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_validate_extensions_rejects_baseline_entry() {
        let all_expected: BTreeSet<String> =
            all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
        let extra_allow = vec!["Bash(true)".to_owned()];
        let mut outcome = VerifyOutcome::pass();
        validate_permission_extensions(&extra_allow, &all_expected, &mut outcome);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_validate_extensions_rejects_scripts_direct_access() {
        let mut allow: BTreeSet<String> =
            all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
        let scripts_entry = "Bash(python3 scripts/my_script.py:*)".to_owned();
        allow.insert(scripts_entry.clone());
        let extra_allow = vec![scripts_entry];
        let mut outcome = VerifyOutcome::pass();
        validate_permission_extensions(&extra_allow, &allow, &mut outcome);
        assert!(outcome.has_errors());
    }

    // -----------------------------------------------------------------------
    // verify_no_local_settings_committed
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_no_local_settings_committed_passes_in_non_git_dir() {
        let tmp = TempDir::new().unwrap();
        let mut outcome = VerifyOutcome::pass();
        verify_no_local_settings_committed(tmp.path(), &mut outcome);
        // In a non-git dir, git exits 128 -- treated as ok
        assert!(outcome.is_ok());
    }

    // -----------------------------------------------------------------------
    // known_cargo_make_tasks / known_git_subcommands
    // -----------------------------------------------------------------------

    #[test]
    fn test_known_cargo_make_tasks_contains_baseline_tasks() {
        let tasks = known_cargo_make_tasks();
        assert!(tasks.contains("ci"));
        assert!(tasks.contains("test"));
        assert!(tasks.contains("clippy"));
    }

    #[test]
    fn test_known_git_subcommands_contains_baseline_subcommands() {
        let subs = known_git_subcommands();
        assert!(subs.contains("status"));
        assert!(subs.contains("diff"));
        assert!(subs.contains("log"));
    }

    // -----------------------------------------------------------------------
    // expected_allow_map completeness
    // -----------------------------------------------------------------------

    #[test]
    fn test_expected_allow_map_has_all_three_sections() {
        let map = expected_allow_map();
        // Other allow
        assert!(map.contains_key("Read(./**)"));
        assert!(map.contains_key("Bash(true)"));
        // Git allow
        assert!(map.contains_key("Bash(git status:*)"));
        // Cargo make allow
        assert!(map.contains_key("Bash(cargo make ci)"));
        assert!(map.contains_key("Bash(cargo make track-switch-main)"));
    }

    // -----------------------------------------------------------------------
    // Integration: verify() with the real project root (smoke test)
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_does_not_panic_on_project_root() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(std::path::Path::new("."));
        let outcome = verify(root);
        // Just confirm it returns without panicking.
        let _ = outcome.findings().len();
    }

    // -----------------------------------------------------------------------
    // verify() early-exit on missing settings
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_returns_error_outcome_when_settings_missing() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        let msgs: Vec<&str> = outcome.findings().iter().map(|f| f.message()).collect();
        assert!(msgs.iter().any(|m| m.contains("Missing settings file")));
    }

    // -----------------------------------------------------------------------
    // write_settings helper (used in integration sub-tests below)
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_settings_helper_creates_readable_file() {
        let tmp = TempDir::new().unwrap();
        let settings = json!({"hooks": {}, "permissions": {"allow": [], "deny": []}});
        write_settings(tmp.path(), &settings);
        let loaded = load_settings(tmp.path());
        assert!(loaded.is_ok());
    }
}
