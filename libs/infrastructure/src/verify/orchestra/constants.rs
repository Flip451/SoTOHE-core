//! Compile-time constants and static regexes for orchestra verification.

use std::sync::LazyLock;

use regex::Regex;

// ---------------------------------------------------------------------------
// File paths
// ---------------------------------------------------------------------------

pub(crate) const SETTINGS_PATH: &str = ".claude/settings.json";
pub(crate) const SETTINGS_LOCAL_PATH: &str = ".claude/settings.local.json";
pub(crate) const PERMISSION_EXTENSIONS_PATH: &str = ".claude/permission-extensions.json";
pub(crate) const AGENTS_DIR: &str = ".claude/agents";

pub(crate) const REQUIRED_AGENT_FILES: &[&str] = &[
    "review-fix-lead.md",
    "dry-fix-lead.md",
    "spec-designer.md",
    "impl-planner.md",
    "type-designer.md",
    "adr-editor.md",
];

// ---------------------------------------------------------------------------
// Static regexes (known-valid patterns compiled once)
// ---------------------------------------------------------------------------

// These three patterns are verified-valid at design time.
// We store them as `Option<Regex>` to avoid `expect()`/`unwrap()` in non-test code.
// Call sites treat `None` as "pattern unavailable — skip regex check".
pub(crate) static EXTRA_CARGO_MAKE_ALLOW_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"^Bash\(cargo make (?P<task>[A-Za-z0-9][A-Za-z0-9_-]*)(?::\*)?\)$").ok()
});

pub(crate) static EXTRA_GIT_ALLOW_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^Bash\(git (?P<subcommand>[a-z][a-z-]*)(?::\*)?\)$").ok());

pub(crate) static HARDCODED_CODEX_MODEL_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"gpt-\d+").ok());

// ---------------------------------------------------------------------------
// Constants: expected hook paths (path -> label)
// ---------------------------------------------------------------------------

// All Python hooks were removed by RV2-17 (ADR 2026-04-09-2323).
// Only Rust hooks (block-direct-git-ops, skill-compliance, block-test-file-deletion)
// remain, and they are validated via EXPECTED_HOOK_COMMANDS by command-fragment match.
// EXPECTED_HOOK_PATHS is intentionally empty: there is no on-disk script file to verify
// because Rust hooks dispatch through the `sotp hook dispatch ...` binary entry point.
pub(crate) const EXPECTED_HOOK_PATHS: &[(&str, &str)] = &[];

// ---------------------------------------------------------------------------
// Constants: expected hook commands (label -> required fragments)
// ---------------------------------------------------------------------------

pub(crate) const EXPECTED_HOOK_COMMANDS: &[(&str, &[&str])] = &[
    (
        "hooksPath setup preflight hook",
        &[
            "SOTP_CLI_BINARY:-",
            "$CLAUDE_PROJECT_DIR/bin/sotp",
            "command -v sotp",
            "hook dispatch hooks-path-setup",
            "else echo '[Git Policy] sotp CLI is not available. Install sotp or set SOTP_CLI_BINARY, then configure hooks with `git config --local core.hooksPath .githooks`.' >&2; exit 2; fi",
            "|| exit 2",
        ],
    ),
    (
        "direct git ops block hook",
        &[
            "SOTP_CLI_BINARY:-",
            "$CLAUDE_PROJECT_DIR/bin/sotp",
            "command -v sotp",
            "hook dispatch block-direct-git-ops",
            "else echo '[Git Policy] sotp CLI is not available. Install sotp or set SOTP_CLI_BINARY before running Bash commands.' >&2; exit 2; fi",
            "|| exit 2",
        ],
    ),
    (
        "test file deletion block hook",
        &[
            "SOTP_CLI_BINARY:-",
            "$CLAUDE_PROJECT_DIR/bin/sotp",
            "command -v sotp",
            "hook dispatch block-test-file-deletion",
            "else echo '[Git Policy] sotp CLI is not available. Install sotp or set SOTP_CLI_BINARY before running Bash or Write guard checks.' >&2; exit 2; fi",
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

pub(crate) const FORBIDDEN_HOOK_COMMAND_FRAGMENTS: &[(&str, &str)] = &[(
    "source checkout cargo fallback in distributed hook command",
    "cargo run --quiet -p cli --",
)];

// ---------------------------------------------------------------------------
// Constants: expected allow (entry -> label)
// ---------------------------------------------------------------------------

pub(crate) const EXPECTED_OTHER_ALLOW: &[(&str, &str)] = &[
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
    ("Bash(grep:*)", "grep read-only permission"),
    ("Bash(uniq:*)", "uniq write-capable but exec-incapable permission"),
    ("Bash(diff:*)", "diff read-only permission"),
    ("Bash(jq:*)", "jq read-only permission"),
    ("Bash(pwd:*)", "pwd read-only permission"),
    ("Bash(bin/sotp arch:*)", "sotp arch subcommand permission"),
    ("Bash(bin/sotp conventions:*)", "sotp conventions subcommand permission"),
];

pub(crate) const EXPECTED_GIT_ALLOW: &[(&str, &str)] = &[
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

pub(crate) const EXPECTED_CARGO_MAKE_ALLOW: &[(&str, &str)] = &[
    ("Bash(cargo make help)", "cargo make help permission"),
    ("Bash(cargo make bootstrap)", "cargo make bootstrap permission"),
    ("Bash(cargo make build-tools)", "cargo make build-tools permission"),
    ("Bash(cargo make build-dev)", "cargo make build-dev permission"),
    ("Bash(cargo make up)", "cargo make up permission"),
    ("Bash(cargo make down)", "cargo make down permission"),
    ("Bash(cargo make logs)", "cargo make logs permission"),
    ("Bash(cargo make ps)", "cargo make ps permission"),
    ("Bash(cargo make shell)", "cargo make shell permission"),
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
    ("Bash(cargo make ci-rust)", "cargo make ci-rust permission"),
    ("Bash(cargo make ci)", "cargo make ci permission"),
    ("Bash(cargo make verify-arch-docs)", "cargo make verify-arch-docs permission"),
    ("Bash(cargo make verify-plan-progress)", "cargo make verify-plan-progress permission"),
    ("Bash(cargo make verify-track-metadata)", "cargo make verify-track-metadata permission"),
    ("Bash(cargo make verify-tech-stack)", "cargo make verify-tech-stack permission"),
    ("Bash(cargo make verify-orchestra)", "cargo make verify-orchestra permission"),
    ("Bash(cargo make verify-latest-track)", "cargo make verify-latest-track permission"),
    ("Bash(cargo make verify-track-registry)", "cargo make verify-track-registry permission"),
    ("Bash(cargo make add-all)", "cargo make add-all permission"),
    ("Bash(cargo make track-add-paths)", "cargo make track-add-paths permission"),
    ("Bash(cargo make track-commit-message)", "cargo make track-commit-message permission"),
    ("Bash(cargo make track-note)", "cargo make track-note permission"),
    ("Bash(cargo make track-branch-create:*)", "cargo make track-branch-create permission"),
    ("Bash(cargo make track-branch-switch:*)", "cargo make track-branch-switch permission"),
    ("Bash(cargo make track-pr)", "cargo make track-pr permission"),
    ("Bash(cargo make track-pr:*)", "cargo make track-pr wildcard permission"),
    ("Bash(cargo make track-pr-push)", "cargo make track-pr-push permission"),
    ("Bash(cargo make track-pr-review)", "cargo make track-pr-review permission"),
    ("Bash(cargo make track-set-commit-hash)", "cargo make track-set-commit-hash bare permission"),
    (
        "Bash(cargo make track-set-commit-hash:*)",
        "cargo make track-set-commit-hash wildcard permission",
    ),
    ("Bash(cargo make track-switch-main)", "cargo make track-switch-main permission"),
    (
        "Bash(cargo make track-local-review-fix-codex:*)",
        "cargo make track-local-review-fix-codex permission",
    ),
    ("Bash(cargo make track-local-dry-fix:*)", "cargo make track-local-dry-fix permission"),
];

pub(crate) const FORBIDDEN_ALLOW: &[&str] = &[
    "Bash(git:*)",
    "Bash(git add:*)",
    "Bash(git commit:*)",
    "Bash(git fetch:*)",
    "Bash(git remote:*)",
    "Bash(git tag:*)",
    "Bash(cat:*)",
    "Bash(ls:*)",
    // find は `-exec` / `-execdir` で任意 utility を exec する wrap-execute 脆弱性を持つ
    // (env と同型)。`-delete` / `-fprint FILE` / `-fls FILE` で destructive 操作も可能。
    // したがって env と同じ理由で FORBIDDEN 維持。
    "Bash(find:*)",
    // sort は GNU sort の `--compress-program=PROG` で temporary files 処理時に任意
    // プログラムを exec する wrap-execute 脆弱性を持つ (env / find -exec と同型)。
    // したがって FORBIDDEN 維持 (2026-04-23 reviewer P0 finding で確定)。
    "Bash(sort:*)",
    // grep / uniq / diff / jq / pwd は baseline の EXPECTED_ALLOW に移行した
    // (2026-04-23 user 判断)。専用 tool (Glob / Grep / Read) が使える範囲は UX 上そちらを
    // 優先するが、GNU grep の独自 flag、jq の JSON filter 等、専用 tool で完全置換できない
    // 場面もあるため一律禁止を解除する。head/tail/wc も同様に allow 済 (WF-35)。
    // uniq は第 2 引数で write 可能だが exec 機構は持たないため、Write tool と同等権限の
    // 範囲内として allow する。
    //
    // env は `env [name=value ...] [utility [argument ...]]` 形式で任意 utility を exec
    // する wrapper として機能するため、allow すると `env git commit` 等で本 FORBIDDEN_ALLOW を
    // bypass できる。したがって env も引き続き FORBIDDEN を維持。
    "Bash(echo:*)",
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
    "Bash(sed:*)",
    "Bash(awk:*)",
    "Bash(env:*)",
    "Bash(xargs:*)",
    "Bash(cargo make add:*)",
    "Bash(cargo make commit:*)",
    "Bash(cargo make note:*)",
    "Bash(cargo make clean)",
    "Bash(cargo make architecture-rules-verify-sync:*)",
];

// ---------------------------------------------------------------------------
// Constants: expected deny (entry -> label)
// ---------------------------------------------------------------------------

pub(crate) const EXPECTED_DENY: &[(&str, &str)] = &[
    ("Read(./.env)", "env file read deny rule"),
    ("Read(./.env.*)", "env wildcard read deny rule"),
    ("Read(./.cache/cargo/**)", "cargo cache read deny rule"),
    ("Grep(./.cache/cargo/**)", "cargo cache grep deny rule"),
    ("Read(./.cache/home/**)", "home cache read deny rule"),
    ("Grep(./.cache/home/**)", "home cache grep deny rule"),
    ("Read(./.cache/sccache/**)", "sccache read deny rule"),
    ("Grep(./.cache/sccache/**)", "sccache grep deny rule"),
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

pub(crate) const SUBAGENT_MODEL_ALLOWLIST: &[&str] =
    &["claude-sonnet-4-6", "claude-opus-4-7", "claude-haiku-4-5-20251001"];

// ---------------------------------------------------------------------------
// Constants: allowed extra git subcommands in permission-extensions.json
// ---------------------------------------------------------------------------

pub(crate) const ALLOWED_EXTRA_GIT_SUBCOMMANDS: &[&str] =
    &["cat-file", "describe", "for-each-ref", "merge-base", "name-rev", "rev-list", "show-ref"];

// ---------------------------------------------------------------------------
// Constants: model resolution targets
// (path, label, required_snippets, forbidden_snippets)
// ---------------------------------------------------------------------------

pub(crate) const MODEL_RESOLUTION_TARGETS: &[(&str, &str, &[&str], &[&str])] = &[
    (
        ".claude/skills/codex-system/SKILL.md",
        "codex-system capability-centric model resolution",
        &["capabilities.<capability>.model", ".harness/config/agent-profiles.json"],
        &[
            "profiles.<active_profile>.provider_model_overrides.codex",
            "providers.codex.default_model",
        ],
    ),
    (
        ".claude/commands/track/plan.md",
        "track-plan provider-specific invocation",
        &["Agent tool", "bin/sotp plan codex-local"],
        &["codex exec --model gpt-5.3-codex --sandbox read-only --full-auto \""],
    ),
    (
        ".claude/commands/track/review.md",
        "track review model resolution",
        // ADR 2300 D3: the orchestrator no longer resolves or passes the reviewer model;
        // `sotp review local` self-resolves it from `capabilities.reviewer` in
        // agent-profiles.json. review.md must keep documenting that capability-centric
        // source (and must never reintroduce provider-centric resolution, below).
        &[
            "reads `capabilities.reviewer` from `.harness/config/agent-profiles.json`",
            "the orchestrator does not pass a reviewer model",
        ],
        &[
            "providers.<reviewer_provider>.fast_model",
            "providers.<reviewer_provider>.default_model",
        ],
    ),
];

// ---------------------------------------------------------------------------
// Constants: reviewer wrapper guidance
// (path, label, required_snippets, forbidden_snippets)
// ---------------------------------------------------------------------------

pub(crate) const REVIEW_WRAPPER_TARGETS: &[(&str, &str, &[&str], &[&str])] = &[
    (
        crate::agent_profiles::AGENT_PROFILES_PATH,
        "agent profile reviewer capability",
        &["\"capabilities\"", "\"reviewer\"", "\"provider\""],
        &["codex exec review --uncommitted --json --model {model} --full-auto"],
    ),
    (
        ".claude/commands/track/review.md",
        "track review wrapper path",
        &[
            "bin/sotp review local --round-type fast --group",
            "--track-id",
            "--briefing-file tmp/reviewer-runtime/briefing-",
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
            "bin/sotp review local --round-type {round_type} --group {scope} --model {model} --briefing-file tmp/codex-briefing.md",
            "bin/sotp review local --round-type {round_type} --group {scope} --model {model} --prompt \"",
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
    // .claude/rules/02-codex-delegation.md entry removed — file deleted in T08
];

// ---------------------------------------------------------------------------
// Constants: TeammateIdle markers
// ---------------------------------------------------------------------------

pub(crate) const TEAMMATE_IDLE_MARKERS: &[(&str, &str)] = &[
    (
        "parent directory",
        "TeammateIdle feedback instructs creating parent directory before writing",
    ),
    ("agent-teams", "TeammateIdle feedback references agent-teams log directory"),
];
