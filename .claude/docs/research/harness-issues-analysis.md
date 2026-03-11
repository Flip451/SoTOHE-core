# Harness Issues Analysis: Python/Shell → Rust CLI Migration Assessment

Source: `tmp/review-2026-03-10.md` (comprehensive review of current harness)
Date: 2026-03-11

## Issue Taxonomy

The review identifies ~40 issues across 7 structural categories.
Below we classify each, assess whether a Rust CLI can address it, and note the resolution approach.

### Legend

- **R** = Fully resolvable by Rust CLI
- **P** = Partially resolvable (needs infra/config changes too)
- **I** = Infrastructure-level (Docker/OS), not CLI
- **D** = Design-level (architecture rethink needed)

---

## Category 1: Security & Guardrail Bypass

| # | Issue | Severity | Rust? | Notes |
|---|-------|----------|-------|-------|
| 1.1 | Regex fallback in `block-direct-git-ops.py` when `bashlex` missing | CRITICAL | **R** | Rust binary with `conch-parser` or `tree-sitter-bash` eliminates optional-dependency problem entirely |
| 1.2 | Codex `workspace-write` bypasses all Claude hooks | CRITICAL | **I** | Rust CLI cannot fix this — needs Docker `.git/` read-only mount or git binary wrapper |
| 1.3 | `cargo make shell` allows container-internal git bypass | HIGH | **I** | Same as 1.2 — infrastructure-level container restriction |
| 1.4 | `KNOWN_SHELLS` mismatch between logger and blocker hooks | HIGH | **R** | Single Rust binary with unified shell recognition |
| 1.5 | Subshell `( )` missing from regex anchor | HIGH | **R** | AST parser makes regex irrelevant |
| 1.6 | Secret directory access only blocked by prompt instruction | CRITICAL | **I** | Docker volume mount restriction (don't mount `private/` into Codex container) |
| 1.7 | Path traversal in `cache_path` validation | HIGH | **R** | Rust `Path::canonicalize()` + `starts_with()` |

**Summary**: 3/7 fully R, 1/7 partially, 3/7 infrastructure-only.
Rust CLI eliminates the entire "regex vs AST" class of bugs.

---

## Category 2: Concurrency & Race Conditions

| # | Issue | Severity | Rust? | Notes |
|---|-------|----------|-------|-------|
| 2.1 | `metadata.json` R-M-W without lock | CRITICAL | **R** | ✅ Already solved by file lock manager (this track) |
| 2.2 | Log rotation race in `log-cli-tools.py` | MEDIUM | **R** | Rust CLI can own log rotation with flock |
| 2.3 | `post-implementation-review.py` state R-M-W no lock | MEDIUM | **R** | Same pattern as 2.1 |
| 2.4 | Sequential task ID (`_next_task_id`) collision | HIGH | **R** | UUID-based ID generation in Rust |
| 2.5 | `Cargo.lock` concurrent modification by agents | HIGH | **P** | File lock manager can protect; also needs agent coordination |
| 2.6 | Single `tools-daemon` build dir contention | HIGH | **P** | Needs per-worker `CARGO_TARGET_DIR` + container orchestration |
| 2.7 | Global single error log overwrite (`last-failure.log`) | MEDIUM | **R** | Task-scoped log paths |

**Summary**: 5/7 fully R, 2/7 partially. This is the Rust CLI's strongest category.
The file lock manager we just built is the foundation for most of these.

---

## Category 3: Environment & Platform Dependencies

| # | Issue | Severity | Rust? | Notes |
|---|-------|----------|-------|-------|
| 3.1 | `.venv` activation required for hooks to work | HIGH | **R** | Static Rust binary has zero runtime dependencies |
| 3.2 | `fcntl` POSIX-only, silently disabled on Windows | HIGH | **R** | `fd-lock` crate is cross-platform |
| 3.3 | Python interpreter startup latency per hook | MEDIUM | **R** | Rust binary starts in ~2ms vs ~50ms for Python |
| 3.4 | Bootstrapping paradox (hooks need `.venv`, `.venv` needs bootstrap) | HIGH | **R** | Pre-compiled binary works before any setup |
| 3.5 | Dynamic Python path resolution overhead | LOW | **R** | Not needed with compiled binary |

**Summary**: 5/5 fully R. Complete elimination of environment dependency issues.

---

## Category 4: Parsing & Validation Fragility

| # | Issue | Severity | Rust? | Notes |
|---|-------|----------|-------|-------|
| 4.1 | Regex-based Markdown `TODO:` detection (false positives in code blocks) | MEDIUM | **R** | Use `pulldown-cmark` for AST-based Markdown parsing |
| 4.2 | Nested quote parsing bug in `extract_command_token` | MEDIUM | **R** | `shlex` crate or `conch-parser` |
| 4.3 | LLM JSON response wrapped in markdown code blocks | LOW | **R** | Strip fences before `serde_json::from_str` |
| 4.4 | Hardcoded English scaffold headings vs Japanese language policy | LOW | **R** | HTML comment markers (`<!-- section: steps -->`) or i18n aliases |
| 4.5 | `git diff --stat HEAD` misses untracked files for loop detection | MEDIUM | **R** | Combine `git diff` + `git status --short` in Rust |

**Summary**: 5/5 fully R. Type-safe parsing is Rust's core strength.

---

## Category 5: Architecture & Design Patterns

| # | Issue | Severity | Rust? | Notes |
|---|-------|----------|-------|-------|
| 5.1 | SSoT split-brain (Planner writes both plan.md and metadata.json) | HIGH | **D** | Rust CLI can enforce single-write-point, but needs workflow redesign |
| 5.2 | `plan.md` destructive re-render loses human edits | MEDIUM | **D** | Optimistic concurrency (hash check before overwrite) — implementable in Rust |
| 5.3 | Canonical Blocks "copy by prompting" data corruption risk | MEDIUM | **D** | Tool-calling to write structured data directly — workflow change |
| 5.4 | Stateless agent router (no conversation history) | LOW | **D** | Claude Code's native tool-calling is better than regex routing |
| 5.5 | O(N) context manual loading by agent | MEDIUM | **P** | Context injection at takt launch — partially CLI, partially orchestration |
| 5.6 | `todo→done` direct transition forbidden | LOW | **R** | State machine redesign in Rust |
| 5.7 | Global "latest track" by timestamp — context stealing | MEDIUM | **R** | Branch-bound or session-bound track context |
| 5.8 | Git Notes local-only (not pushed by default) | LOW | **P** | Bootstrap script can configure refspec |

**Summary**: 2/8 fully R, 2/8 partially, 4/8 design-level. These need architectural decisions beyond just Rust.

---

## Category 6: Observability & Error Handling

| # | Issue | Severity | Rust? | Notes |
|---|-------|----------|-------|-------|
| 6.1 | Fail-open exception handling in hooks | HIGH | **R** | ✅ Already fixed for lock hooks (fail-closed); pattern applicable system-wide |
| 6.2 | Error log tail-only truncation (loses root cause) | MEDIUM | **R** | `cargo test --message-format=json` structured error extraction |
| 6.3 | Timeout kills Python but not grandchild processes | MEDIUM | **R** | `os.setsid` + process group kill — easier in Rust with `nix` crate |
| 6.4 | Command string matching for hook triggers (fragile) | MEDIUM | **R** | Exit code + output content based triggers |
| 6.5 | TDD: compile error vs test failure not distinguished | HIGH | **R** | `--message-format=json` parsing distinguishes compiler vs test errors |
| 6.6 | Circuit breaker uses LLM to judge LLM loops | MEDIUM | **D** | Deterministic heuristics (same error code N times) in Rust |

**Summary**: 5/6 fully R, 1/6 design-level.

---

## Category 7: Scalability & Lifecycle

| # | Issue | Severity | Rust? | Notes |
|---|-------|----------|-------|-------|
| 7.1 | Sync hook latency as codebase grows | MEDIUM | **R** | Rust binary ~2ms startup; async background analysis |
| 7.2 | Archived tracks pollute AI search context | LOW | **P** | Move to `.archive/` + settings.json deny — CLI can automate |
| 7.3 | `tools-daemon` statefulness causes flaky tests | LOW | **I** | `run --rm` ephemeral containers or sccache optimization |
| 7.4 | SSoT config spread across 5 files | MEDIUM | **P** | Code-generate deny.toml/Makefile from architecture-rules.json |
| 7.5 | JSON/YAML format inconsistency | LOW | **R** | Unify on JSON with serde |
| 7.6 | Single-branch trunk-based dev forces contention | MEDIUM | **D** | Feature branch strategy — workflow decision |
| 7.7 | Verification.md can be faked by AI | MEDIUM | **D** | Human approval gate — needs interactive CLI |
| 7.8 | `.gitignore` responsibility leaked to app layer | LOW | **R** | Add patterns to `.gitignore` instead of `:(exclude)` |

**Summary**: 3/8 fully R, 2/8 partially, 2/8 design, 1/8 infrastructure.

---

## Overall Feasibility Assessment

| Category | Total | Rust CLI (R) | Partial (P) | Infra (I) | Design (D) |
|----------|-------|-------------|-------------|-----------|------------|
| 1. Security & Guardrails | 7 | 3 | 0 | 4 | 0 |
| 2. Concurrency | 7 | 5 | 2 | 0 | 0 |
| 3. Environment | 5 | 5 | 0 | 0 | 0 |
| 4. Parsing | 5 | 5 | 0 | 0 | 0 |
| 5. Architecture | 8 | 2 | 2 | 0 | 4 |
| 6. Observability | 6 | 5 | 0 | 0 | 1 |
| 7. Scalability | 8 | 3 | 2 | 1 | 2 |
| **Total** | **46** | **28 (61%)** | **6 (13%)** | **5 (11%)** | **7 (15%)** |

### Conclusion

**Rust CLI alone resolves 61% of identified issues and contributes to another 13%.**

The remaining 26% breaks down as:
- **Infrastructure (11%)**: Docker container restrictions, `.git/` read-only mounts, ephemeral containers. These complement the CLI but are independent.
- **Design (15%)**: Workflow redesign (SSoT write model, branch strategy, human approval gates, agent context sharing). These require architectural decisions that the CLI implements but doesn't dictate.

**Verdict: YES, a Rust CLI as the core harness is a highly viable strategy.**

The current file lock manager (`FsFileLockManager`) already demonstrates the pattern:
domain trait → infrastructure implementation → CLI subcommand → hook delegation.
This same architecture scales to cover command parsing, state management, log routing,
and TDD state machine — the four highest-impact areas.

### Recommended Migration Order (by impact × feasibility)

1. **File locking** (✅ Done — ownership-file-lock track)
2. **Command parsing / git guard** (conch-parser, eliminates all regex security issues)
3. **State management** (metadata.json atomic R-M-W with lock, UUID task IDs)
4. **TDD state machine** (structured cargo output parsing, compile vs test distinction)
5. **Log management** (task-scoped paths, structured error extraction)
6. **Infrastructure hardening** (Docker config, parallel alongside CLI work)
