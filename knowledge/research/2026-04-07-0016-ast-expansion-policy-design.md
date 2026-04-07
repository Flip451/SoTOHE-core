# AST-Aware Expansion Policy Refinement — Design Document

Date: 2026-04-07
Author: Planner (claude-opus-4-6)
Track context: guard policy refinement for `for`-loop / `$VAR` allowance

---

## 1. Problem Restatement

`has_expansion_marker(token: &str)` in `policy.rs:674` treats every token
containing `$` or a backtick as a potential bypass vector:

```rust
fn has_expansion_marker(token: &str) -> bool {
    token.contains('$') || token.contains('`')
}
```

This fires on harmless constructs like `echo $x` or `for f in *.rs; do wc -l "$f"; done`,
making it impossible to write `for` loops or pass environment variable values through
shell commands. The infrastructure layer (conch.rs + flatten.rs) already distinguishes
`SimpleWord::Param` (parameter expansion, no execution) from `SimpleWord::Subst` /
`ParameterSubstitution::Command` (command substitution, actual execution), but that
distinction is lost when words are flattened to `Vec<String>` before the domain layer
sees them.

---

## 2. Approach Recommendation

**Recommended: Option A — Add `has_command_substitution: bool` to `SimpleCommand`.**

### Option Comparison

| Option | Description | Domain/Infra boundary | Complexity | Risk |
|--------|-------------|----------------------|------------|------|
| **A** | Add `has_command_substitution: bool` to `SimpleCommand` | Clean — domain struct extended, no conch types leak | Low | Low |
| B | Change `argv` to `Vec<Token>` where Token carries expansion kind | Domain struct API break; all argv consumers must be updated | High | Medium |
| C | Detect during AST walk, set flag before flattening (same as A but via separate pass) | Effectively equivalent to A, slightly less minimal | Low | Low |
| D | Refine `has_expansion_marker` to distinguish patterns from flattened strings | Fragile string heuristics; `$(...)` appears in flattened output but loses nesting context | Medium | High |

**Rationale for Option A:**

1. **Minimal interface surface.** `SimpleCommand` already has `has_output_redirect: bool` — a boolean flag for a property detected at parse time. Adding `has_command_substitution: bool` follows the same pattern exactly. The domain layer needs only a single semantic distinction: "does this command involve actual command execution via substitution?"

2. **No conch-parser types in domain.** The flag is a primitive `bool`; the detection logic stays in `infrastructure::shell::conch` where the AST is available.

3. **Backward-compatible policy refactor.** The policy function `command_contains_expansion` becomes `command_contains_command_substitution`, which checks the new flag. The change is localized to one struct field and two functions.

4. **Option D is rejected** because the flattened string for a command substitution is `$(inner command flattened)` — indistinguishable by simple pattern matching from a literal token that starts with `$(`. More importantly, nested `${VAR:-$(cmd)}` would need recursive string parsing to detect, which recreates the fragility the conch-parser AST was meant to eliminate.

5. **Option B is rejected** because changing `Vec<String>` to `Vec<Token>` is a large API surface change. Every caller of `SimpleCommand::argv` (policy, usecase hook handlers, tests) must adapt. The enum-first principle applies when variant-dependent data is needed — but the policy only needs one bit per command, not per token.

---

## 3. Type Design

### 3.1 Enum-First / Make Illegal States Unrepresentable Analysis

The coding principle says: use an enum when different variants hold different data. Here the question is whether to represent "expansion kind" at the **command level** or the **token level**.

At the **token level** (Option B), an enum like the following would apply:

```rust
pub enum Token {
    Literal(String),
    WithParamExpansion(String),    // $VAR, ${VAR}, $1, $#, $@
    WithCommandSubstitution(String), // contains $(…) or `…`
}
```

This is richer but over-engineers the consumer: the policy only needs to know "does any token in this command contain a command substitution?" The per-token distinction is only needed for richer policy messages, not for the block/allow decision.

At the **command level** (Option A), the flag `has_command_substitution: bool` captures the only distinction the policy needs. An illegal state would be `has_command_substitution: true` when no argv or redirect_text actually contains a command substitution — but this is not an illegal state the type system can eliminate more elegantly than a constructor invariant.

**Decision: command-level flag is appropriate.** The flag maps cleanly to the single policy question. This is the same trade-off as `has_output_redirect: bool` — the AST knows which redirect is which; the domain only needs the boolean.

### 3.2 `SimpleCommand` Extension

```rust
/// A parsed simple command (argv list + redirect texts + redirect flags).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleCommand {
    /// The argument vector of the command.
    pub argv: Vec<String>,
    /// Flattened text from redirect targets (including heredoc bodies).
    pub redirect_texts: Vec<String>,
    /// Whether this command has any output redirect (Write/Append/Clobber/ReadWrite).
    /// Does NOT include DupWrite (`>&fd`) or Read (`<`).
    pub has_output_redirect: bool,
    /// Whether any argv token or redirect text contains a command substitution
    /// (`$(...)` or backtick form). Set by the infrastructure parser from the
    /// conch-parser AST. Parameter expansions (`$VAR`, `${VAR}`, `$1`, etc.)
    /// do NOT set this flag.
    pub has_command_substitution: bool,
}
```

`has_command_substitution` is `false` for:
- `$VAR`, `$1`, `$HOME`, `$@`, `$*`, `$#`, `$?` — `SimpleWord::Param`
- `${VAR}`, `${#VAR}`, `${VAR:-default}` — `SimpleWord::Subst` variants other than `Command`
- `$((expr))` — `ParameterSubstitution::Arith`
- Glob expansions `*`, `?`, `[...]` — `SimpleWord::Star/Question/SquareOpen/SquareClose`

`has_command_substitution` is `true` for:
- `$(cmd)` — `ParameterSubstitution::Command` in argv token
- `` `cmd` `` — backtick form (also `ParameterSubstitution::Command` in conch-parser)
- `${VAR:-$(cmd)}` — nested command substitution inside a parameter expansion word

### 3.3 Infrastructure: Detection Logic in `collect_from_conch_simple`

The flag is set during `SimpleCommand` construction in `conch.rs`. The detection walks the same AST that `collect_command_substitutions_from_word` already walks, but only needs to return `bool` rather than collecting the substitution bodies. A new helper:

```rust
/// Returns true if the word contains any ParameterSubstitution::Command node
/// (i.e., an actual command substitution, not mere parameter expansion).
fn word_has_command_substitution(word: &ast::TopLevelWord<String>) -> bool {
    complex_word_has_command_substitution(&word.0)
}

fn complex_word_has_command_substitution(cw: &ConchComplexWord) -> bool {
    match cw {
        ast::ComplexWord::Single(w) => word_node_has_command_substitution(w),
        ast::ComplexWord::Concat(words) => {
            words.iter().any(word_node_has_command_substitution)
        }
    }
}

fn word_node_has_command_substitution(word: &ConchWord) -> bool {
    match word {
        ast::Word::Simple(sw) => simple_word_has_command_substitution(sw),
        ast::Word::SingleQuoted(_) => false,
        ast::Word::DoubleQuoted(parts) => {
            parts.iter().any(simple_word_has_command_substitution)
        }
    }
}

fn simple_word_has_command_substitution(sw: &ConchSimpleWord) -> bool {
    if let ast::SimpleWord::Subst(subst) = sw {
        param_subst_has_command_substitution(subst)
    } else {
        false
    }
}

fn param_subst_has_command_substitution(
    subst: &ast::ParameterSubstitution<
        ast::Parameter<String>,
        ast::TopLevelWord<String>,
        ast::TopLevelCommand<String>,
        ast::Arithmetic<String>,
    >,
) -> bool {
    match subst {
        ast::ParameterSubstitution::Command(_) => true,
        // Nested: ${VAR:-$(cmd)} — the word itself may contain a command substitution
        ast::ParameterSubstitution::Default(_, _, Some(w))
        | ast::ParameterSubstitution::Assign(_, _, Some(w))
        | ast::ParameterSubstitution::Error(_, _, Some(w))
        | ast::ParameterSubstitution::Alternative(_, _, Some(w))
        | ast::ParameterSubstitution::RemoveSmallestSuffix(_, Some(w))
        | ast::ParameterSubstitution::RemoveLargestSuffix(_, Some(w))
        | ast::ParameterSubstitution::RemoveSmallestPrefix(_, Some(w))
        | ast::ParameterSubstitution::RemoveLargestPrefix(_, Some(w)) => {
            word_has_command_substitution(w)
        }
        _ => false,
    }
}
```

In `collect_from_conch_simple`, after building `argv` and `redirect_texts`, compute the flag:

```rust
let has_command_substitution = {
    let in_env_vars = simple.redirects_or_env_vars.iter().any(|item| match item {
        ast::RedirectOrEnvVar::EnvVar(_, Some(w)) => word_has_command_substitution(w),
        ast::RedirectOrEnvVar::Redirect(r) => {
            extract_redirect_word(r).is_some_and(word_has_command_substitution)
        }
        _ => false,
    });
    let in_cmd_words = simple.redirects_or_cmd_words.iter().any(|item| match item {
        ast::RedirectOrCmdWord::CmdWord(w) => word_has_command_substitution(w),
        ast::RedirectOrCmdWord::Redirect(r) => {
            extract_redirect_word(r).is_some_and(word_has_command_substitution)
        }
    });
    in_env_vars || in_cmd_words
};

out.push(SimpleCommand {
    argv,
    redirect_texts,
    has_output_redirect,
    has_command_substitution,
});
```

---

## 4. Security Analysis

### 4.1 Threat Model

The guard policy runs inside Claude Code's pre-tool-call hook. Its adversary model is:

- Claude itself, proposing shell commands in generated code that could trigger git operations the user has not authorized.
- Commands are passed as strings to `Bash(command=...)` calls, then checked synchronously by the policy before execution.
- **Shell variables are NOT expanded by the policy checker.** The raw string `echo $HOME` is checked, not `echo /home/user`.

This means the policy operates on the *literal* form of the command, not its post-expansion form. This is a fundamental constraint that shapes the security analysis below.

### 4.2 `$VAR` Bypass Vector: Can Parameter Expansion Route Around `command_contains_git`?

**Yes, it can — under a specific precondition.**

Consider:
```bash
$CMD add .
```
If `CMD=git` in the shell environment, this expands to `git add .` at runtime. The policy sees `argv = ["$CMD", "add", "."]`. With the proposed change:
- `has_command_substitution = false` (this is `$CMD`, a `SimpleWord::Param`)
- `effective_cmd = basename("$cmd").to_lowercase()` → `"$cmd"` (the literal string)
- `effective_cmd == "git"` → **false**
- `command_contains_git` checks for "git" substring in all tokens: `"$cmd"` contains no "git" → **false**
- Result: **ALLOWED** — a bypass.

**However, the precondition for this bypass is meaningful:**

1. Claude would need to generate `$CMD add .` instead of `git add .`. This is an unusual pattern — it requires Claude to have set `CMD=git` in a previous command or to know that an existing environment variable expands to `git`.
2. Environment variables set in previous Bash tool calls persist only within that invocation's shell session (unless exported). Claude Code typically issues independent `Bash()` calls; `CMD=git` would need to be set in the same call or in an exported environment.
3. The template workflow explicitly states: "The template workflow never needs `$VAR`, `$(cmd)`, or backtick in any position." This remains the default fallback message.

**Proposed Mitigation:**

The policy should scan argv tokens for a `$`-prefixed token at the **effective command position** (after skipping var assignments and launchers) and at the effective git subcommand position:

```rust
fn is_variable_reference(s: &str) -> bool {
    s.starts_with('$')
}
```

Specifically:
- If the effective command token starts with `$`, block with `GIT_VARIABLE_BYPASS_MESSAGE`. This prevents `$CMD add .` from bypassing detection.
- Parameter expansions in **non-command-position** argv tokens (e.g., `echo $HOME`, `wc -l $f`) are safe to allow because these tokens are arguments, not commands.

This targeted check replaces the current broad `command_contains_expansion` with two narrower checks:
1. Block if `effective_cmd` starts with `$` (command position bypass).
2. Block if `has_command_substitution` is true (arbitrary execution in any position).

The string `$1`, `$@`, `$*`, `$#` at command position should also be blocked — they are all positional/special parameters that could be set by the caller.

### 4.3 What `$VAR` in Non-Command Position Can Do

In non-command-position argv tokens, `$VAR` is an argument to a program. The policy already knows what program it is (the `effective_cmd`). If that program is allowed (not `git`, not a file-writer, etc.), then expanding an argument via a variable is safe in the Claude Code hook context:

- `echo $HOME` — echo is not blocked; `$HOME` expands to a path. No git operation.
- `wc -l $f` — wc is not blocked; `$f` is a filename. No git operation.
- `cargo test $TEST_NAME` — cargo is not blocked (unless targeting git-adjacent operations).
- `git diff $FILE` — git is blocked at `check_git_command`. Even if `FILE=--` the subcommand detection still applies (and `git diff` is allowed in the current policy).

The risk is: `git --option $VAR` where `$VAR=add .`. But `extract_git_subcommand` skips option-valued arguments when the option is in `GIT_OPTIONS_WITH_ARG`. A variable at a literal git subcommand position would cause `extract_git_subcommand` to return `Some("$var")`, which does not match `Some("add")`, `Some("commit")`, etc. — so the command would be allowed. This is an accepted residual risk: `git $SUBCMD` could bypass the subcommand check.

**Mitigation for git subcommand bypass:**

If `effective_cmd == "git"`, the token immediately following git (after global option skipping) should also be checked for `$`-prefix. If it starts with `$`, block with `GIT_VARIABLE_BYPASS_MESSAGE`. `extract_git_subcommand` can return an `Option<&str>` and the caller checks `starts_with('$')`.

### 4.4 `${VAR:-$(cmd)}` — Nested Command Substitution

The `flatten_substitution` function in `flatten.rs` renders `${VAR:-$(cmd)}` as `${VAR:-$(cmd flattened)}` in the flattened string. The `collect_command_substitutions_from_word` function already extracts the inner `$(cmd)` body and submits it as a separate `SimpleCommand` for policy evaluation. 

The `has_command_substitution` flag for the **outer** command is also set to `true` because `param_subst_has_command_substitution` recurses into the word of `Default`/`Assign`/etc. variants. So `${VAR:-$(cmd)}` is doubly blocked: both as a command substitution in the outer command and as an extracted inner command.

### 4.5 `${#VAR}` and `$((expr))`

- `${#VAR}` — `ParameterSubstitution::Len`. No execution. `has_command_substitution = false`. ALLOWED.
- `$((1+2))` — `ParameterSubstitution::Arith`. No execution. `has_command_substitution = false`. ALLOWED.
- Both still produce flattened strings containing `$` — the old `has_expansion_marker` would block them. The new approach allows them.

### 4.6 Risk Acceptance Summary

| Vector | Residual after fix | Severity | Accepted? |
|--------|--------------------|----------|-----------|
| `$CMD add .` at command position | Blocked by `effective_cmd.starts_with('$')` check | High | N/A (blocked) |
| `git $SUBCMD` | `extract_git_subcommand` returns `$subcmd`, then new check blocks | High | N/A (blocked) |
| `echo $HOME` | Allowed (echo in non-git context) | None | Yes |
| `${#VAR}` / `$((expr))` | Allowed (no execution) | None | Yes |
| `${VAR:-$(cmd)}` | Blocked by `has_command_substitution = true` AND inner cmd extracted | High | N/A (blocked) |
| `sudo git $SUBCMD` | Launcher skipping reaches `git`, then git subcommand variable check fires | High | N/A (blocked) |

---

## 5. Policy Decision Matrix

The new `check_simple_command` logic replaces the single `command_contains_expansion` call with three targeted checks, applied in order:

| Check | Condition | Verdict | Rationale |
|-------|-----------|---------|-----------|
| Output redirect | `cmd.has_output_redirect` | BLOCK | Unchanged from current policy |
| Command substitution in any position | `cmd.has_command_substitution` | BLOCK | Arbitrary execution, always dangerous |
| Variable in command position | `effective_cmd.starts_with('$')` | BLOCK | Could expand to blocked command (git, etc.) |
| Variable in git subcommand position | (effective_cmd=="git") AND subcommand starts with `$` | BLOCK | Could expand to blocked subcommand (add, commit, etc.) |
| `env` command | `effective_cmd == "env"` | BLOCK | Unchanged |
| File-write commands (`tee`, `sed -i`) | existing checks | BLOCK | Unchanged |
| `git add/commit/push/...` | existing `check_git_command` | BLOCK | Unchanged |
| Non-git command with "git" in tokens | `command_contains_git` | BLOCK | Unchanged |
| All other | — | ALLOW | — |

The ordering matters: command substitution check fires before the effective-cmd check, so `$(git add .)` is blocked at the substitution level, not the git level (either would block it, but substitution check is first).

### 5.1 What This Changes

Previously ALL `$` or backtick usage in any token → BLOCK.

After the change:
- `$VAR`, `${VAR}`, `${#VAR}`, `$1`, `$@`, `$*` in **non-command, non-git-subcommand** position → ALLOW.
- `$VAR` in **command position** (first non-assignment, non-launcher token) → BLOCK.
- `$VAR` as **git subcommand** position → BLOCK.
- `$(cmd)`, `` `cmd` `` anywhere → BLOCK.
- `${VAR:-$(cmd)}` anywhere → BLOCK (nested command substitution).
- `$((expr))` in any position → ALLOW (arithmetic, no execution).

---

## 6. Test Case Matrix

All test cases use the real `ConchShellParser` + `check_commands` integration path to exercise the full data flow.

### 6.1 Currently Blocked, Should Be ALLOWED After Change

| Input | Why safe | Notes |
|-------|----------|-------|
| `echo $HOME` | param expansion, echo is harmless | |
| `echo $USER` | param expansion | |
| `wc -l $f` | param expansion in arg position | `$f` is a filename arg |
| `for x in a b c; do echo $x; done` | `$x` in body echo arg | body cmds checked individually |
| `for f in *.rs; do wc -l "$f"; done` | `$f` in arg, glob expansion | |
| `printf '%s\n' $@` | `$@` in arg position | printf is harmless |
| `cargo test $TEST_FILTER` | `$TEST_FILTER` in cargo arg | cargo is allowed |
| `echo ${#VAR}` | length expansion, no execution | |
| `echo $((1 + 2))` | arithmetic expansion, no execution | |
| `mkdir -p $HOME/tmp` | `$HOME` in path arg | |
| `cat $CONFIG_FILE` | `$CONFIG_FILE` in arg | |

### 6.2 Currently Blocked, Should Remain BLOCKED After Change

| Input | Blocking reason | Expected message |
|-------|-----------------|-----------------|
| `echo $(git status)` | command substitution in arg | `GIT_VARIABLE_BYPASS_MESSAGE` (has_command_substitution) |
| `` echo `git log` `` | backtick command substitution | `GIT_VARIABLE_BYPASS_MESSAGE` |
| `$CMD add .` | variable at command position | `GIT_VARIABLE_BYPASS_MESSAGE` |
| `$git add .` | variable at command position | `GIT_VARIABLE_BYPASS_MESSAGE` |
| `git $SUBCMD .` | variable at git subcommand position | `GIT_VARIABLE_BYPASS_MESSAGE` |
| `git add $(echo .)` | command substitution in git arg | `GIT_VARIABLE_BYPASS_MESSAGE` (has_command_substitution) |
| `echo ${VAR:-$(cmd)}` | nested command substitution | `GIT_VARIABLE_BYPASS_MESSAGE` |
| `bash -c "$(git add .)"` | command substitution | `GIT_VARIABLE_BYPASS_MESSAGE` |
| `for x in $(git add .); do echo $x; done` | command substitution in iterator | Already extracted and checked as separate cmd |
| `> $HOME/file` | output redirect (unchanged) | `OUTPUT_REDIRECT_MESSAGE` |
| `` x=`git add .` `` | backtick command substitution | `GIT_VARIABLE_BYPASS_MESSAGE` |
| `sudo $CMD status` | variable at command position after launcher | `GIT_VARIABLE_BYPASS_MESSAGE` |

### 6.3 Currently ALLOWED, Must Remain ALLOWED

| Input | Notes |
|-------|-------|
| `git status` | direct git, subcommand allowed |
| `git diff HEAD` | allowed subcommand |
| `cargo build` | non-git command |
| `ls -la` | non-git command |
| `echo 'hello $world'` | single-quoted, `$` not expanded |
| `echo "hello world"` | double-quoted literal |

### 6.4 Regression: `for` Loop Iterator with Variable

| Input | Expected |
|-------|----------|
| `for x in a b c; do echo $x; done` | ALLOW |
| `for x in $(git add .); do echo hi; done` | BLOCK (command substitution in iterator, already extracted) |
| `for f in *.rs; do cargo fmt $f; done` | ALLOW |
| `for f in *.rs; do git add $f; done` | BLOCK (git add in body) |

Note: `git add $f` — `$f` is in **git subcommand argument** position, not git subcommand position. `check_git_command` returns BLOCK for `add`. The variable `$f` is the path argument, not the subcommand. This is correctly blocked by the git add rule, not the variable rule.

---

## 7. Canonical Blocks

### 7.1 `libs/domain/src/guard/types.rs` — Extended `SimpleCommand`

```rust
//! Shell command types used by guard policy and parsing.

/// A parsed simple command (argv list + redirect texts + redirect flags).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleCommand {
    /// The argument vector of the command.
    pub argv: Vec<String>,
    /// Flattened text from redirect targets (including heredoc bodies).
    /// Used by policy to detect git references hidden in heredocs.
    pub redirect_texts: Vec<String>,
    /// Whether this command has any output redirect (Write/Append/Clobber/ReadWrite).
    /// Does NOT include DupWrite (`>&fd`) or Read (`<`).
    pub has_output_redirect: bool,
    /// Whether any argv token, redirect text, or env var value in this command
    /// contains a command substitution (`$(…)` or backtick form).
    ///
    /// Set to `true` only for `ParameterSubstitution::Command` nodes in the AST.
    /// Parameter expansions (`$VAR`, `${VAR}`, `${#VAR}`, `$1`, `$((expr))`) do NOT
    /// set this flag.
    ///
    /// This flag is set by the infrastructure parser (`ConchShellParser`);
    /// the policy layer uses it to block commands containing arbitrary execution.
    pub has_command_substitution: bool,
}
```

### 7.2 `libs/domain/src/guard/policy.rs` — Revised Check Functions

```rust
/// Checks if a command contains a command substitution ($(…) or backtick).
/// Parameter expansions ($VAR, ${VAR}, $1, etc.) are NOT considered substitutions.
fn command_contains_command_substitution(cmd: &SimpleCommand) -> bool {
    cmd.has_command_substitution
}

/// Checks if a token is a shell variable reference (starts with `$`).
/// Used to detect `$CMD` at command position, which could bypass git detection.
fn is_variable_reference(token: &str) -> bool {
    token.starts_with('$')
}

/// Checks a single simple command against the policy.
fn check_simple_command(cmd: &SimpleCommand) -> GuardVerdict {
    // Check output redirects first — even redirect-only commands (empty argv)
    // like `> /tmp/file` must be blocked.
    if cmd.has_output_redirect {
        return GuardVerdict::block(OUTPUT_REDIRECT_MESSAGE);
    }

    let argv = &cmd.argv;
    if argv.is_empty() {
        return GuardVerdict::allow();
    }

    // Block command substitutions in any position — arbitrary execution.
    // Note: parameter expansions ($VAR, ${VAR}, $1, $((expr))) are NOT blocked here;
    // they are safe because the policy runs before shell expansion.
    if command_contains_command_substitution(cmd) {
        return GuardVerdict::block(GIT_VARIABLE_BYPASS_MESSAGE);
    }

    // Skip VAR=val assignments and command launchers to find the effective command
    let effective_start = skip_var_assignments(argv, 0);
    let effective_start = skip_command_launchers(argv, effective_start);

    if effective_start >= argv.len() {
        return GuardVerdict::allow();
    }

    let effective_token = &argv[effective_start];

    // Block variable reference at command position: `$CMD …` could expand to a
    // blocked command (e.g., `git`) that the string-level checks would not detect.
    if is_variable_reference(effective_token) {
        return GuardVerdict::block(GIT_VARIABLE_BYPASS_MESSAGE);
    }

    let effective_cmd = basename(effective_token).to_lowercase();

    // Block `env` command unconditionally.
    if effective_cmd == "env" {
        return GuardVerdict::block(ENV_COMMAND_MESSAGE);
    }

    // --- File-write guards (CON-07) ---
    if FILE_WRITE_COMMANDS.contains(&effective_cmd.as_str()) {
        return GuardVerdict::block(FILE_WRITE_COMMAND_MESSAGE);
    }

    if effective_cmd == "sed" && has_sed_inplace_flag(argv, effective_start) {
        return GuardVerdict::block(FILE_WRITE_COMMAND_MESSAGE);
    }
    // --- End file-write guards ---

    // Direct git command — check specific subcommands
    if effective_cmd == "git" {
        // Block variable reference at git subcommand position: `git $SUBCMD …`
        // could expand to a blocked subcommand (add, commit, etc.).
        if git_subcommand_is_variable(argv, effective_start) {
            return GuardVerdict::block(GIT_VARIABLE_BYPASS_MESSAGE);
        }
        return check_git_command(argv, effective_start);
    }

    // Block `cp` (or `mv`) targeting `bin/sotp`.
    if is_bin_sotp_overwrite(argv, effective_start) {
        return GuardVerdict::block(BIN_SOTP_OVERWRITE_MESSAGE);
    }

    // Non-git command: block if any argv token or redirect text contains "git".
    if command_contains_git(cmd) {
        return GuardVerdict::block(NESTED_GIT_REFERENCE_MESSAGE);
    }

    GuardVerdict::allow()
}

/// Returns true if the token immediately following `git` (and its global options)
/// starts with `$`, indicating the git subcommand is a shell variable reference.
fn git_subcommand_is_variable(argv: &[String], git_index: usize) -> bool {
    let mut i = git_index + 1;

    while i < argv.len() {
        let token = &argv[i];

        if token == "--" {
            i += 1;
            break;
        }

        if GIT_OPTIONS_WITH_ARG.contains(&token.as_str()) {
            i += 2;
            continue;
        }

        if token.starts_with('-') {
            i += 1;
            continue;
        }

        // First non-option token is the subcommand
        return token.starts_with('$');
    }

    // If we fall through to here, either there's a positional after `--` or nothing.
    // Check the token at i if it exists.
    argv.get(i).is_some_and(|t| t.starts_with('$'))
}
```

### 7.3 `libs/infrastructure/src/shell/flatten.rs` — New Detection Helpers

```rust
/// Returns `true` if the word contains any `ParameterSubstitution::Command` node.
///
/// Used to set `SimpleCommand::has_command_substitution` during AST flattening.
/// Only `Command(…)` substitutions (which execute a command) return `true`.
/// Parameter expansions (`Param`, `Len`, `Arith`, `Default`, etc.) return `false`
/// unless they contain a nested `Command` substitution in their value word.
pub(super) fn word_has_command_substitution(word: &ast::TopLevelWord<String>) -> bool {
    complex_word_has_command_substitution(&word.0)
}

fn complex_word_has_command_substitution(cw: &ConchComplexWord) -> bool {
    match cw {
        ast::ComplexWord::Single(w) => word_node_has_command_substitution(w),
        ast::ComplexWord::Concat(words) => {
            words.iter().any(word_node_has_command_substitution)
        }
    }
}

fn word_node_has_command_substitution(word: &ConchWord) -> bool {
    match word {
        ast::Word::Simple(sw) => simple_word_has_command_substitution(sw),
        ast::Word::SingleQuoted(_) => false,
        ast::Word::DoubleQuoted(parts) => {
            parts.iter().any(simple_word_has_command_substitution)
        }
    }
}

fn simple_word_has_command_substitution(sw: &ConchSimpleWord) -> bool {
    if let ast::SimpleWord::Subst(subst) = sw {
        param_subst_has_command_substitution(subst)
    } else {
        false
    }
}

fn param_subst_has_command_substitution(
    subst: &ast::ParameterSubstitution<
        ast::Parameter<String>,
        ast::TopLevelWord<String>,
        ast::TopLevelCommand<String>,
        ast::Arithmetic<String>,
    >,
) -> bool {
    match subst {
        ast::ParameterSubstitution::Command(_) => true,
        // Recursively check the value word of parameter expansion operators
        // to catch `${VAR:-$(cmd)}` patterns.
        ast::ParameterSubstitution::Default(_, _, Some(w))
        | ast::ParameterSubstitution::Assign(_, _, Some(w))
        | ast::ParameterSubstitution::Error(_, _, Some(w))
        | ast::ParameterSubstitution::Alternative(_, _, Some(w))
        | ast::ParameterSubstitution::RemoveSmallestSuffix(_, Some(w))
        | ast::ParameterSubstitution::RemoveLargestSuffix(_, Some(w))
        | ast::ParameterSubstitution::RemoveSmallestPrefix(_, Some(w))
        | ast::ParameterSubstitution::RemoveLargestPrefix(_, Some(w)) => {
            word_has_command_substitution(w)
        }
        _ => false,
    }
}
```

### 7.4 `libs/infrastructure/src/shell/conch.rs` — Updated `collect_from_conch_simple`

The construction of `SimpleCommand` gains the new flag. The relevant section of `collect_from_conch_simple` changes at the `out.push(...)` call site:

```rust
// Compute has_command_substitution from AST — set before flattening loses the distinction.
let has_command_substitution = {
    let in_env_vars = simple.redirects_or_env_vars.iter().any(|item| match item {
        ast::RedirectOrEnvVar::EnvVar(_, Some(w)) => word_has_command_substitution(w),
        ast::RedirectOrEnvVar::Redirect(r) => {
            extract_redirect_word(r).is_some_and(word_has_command_substitution)
        }
        ast::RedirectOrEnvVar::EnvVar(_, None) => false,
    });
    let in_cmd_words = simple.redirects_or_cmd_words.iter().any(|item| match item {
        ast::RedirectOrCmdWord::CmdWord(w) => word_has_command_substitution(w),
        ast::RedirectOrCmdWord::Redirect(r) => {
            extract_redirect_word(r).is_some_and(word_has_command_substitution)
        }
    });
    in_env_vars || in_cmd_words
};

if !argv.is_empty() || has_output_redirect {
    out.push(SimpleCommand {
        argv,
        redirect_texts,
        has_output_redirect,
        has_command_substitution,
    });
}
```

The `use super::flatten::word_has_command_substitution;` import must be added to `conch.rs`.

### 7.5 `ShellParser` Trait — No Change Required

The `ShellParser` trait signature remains unchanged:

```rust
pub trait ShellParser: Send + Sync {
    fn split_shell(&self, input: &str) -> Result<Vec<SimpleCommand>, ParseError>;
}
```

The new field is carried transparently in `SimpleCommand`.

---

## 8. Implementation Sequencing

1. **Step 1 — Extend `SimpleCommand`** (`libs/domain/src/guard/types.rs`): Add `has_command_substitution: bool`. Default for test helpers in `policy.rs` tests: add `has_command_substitution: false` to all `SimpleCommand { ... }` literal constructions. No policy logic changes yet.

2. **Step 2 — Add detection helpers in `flatten.rs`** (`libs/infrastructure/src/shell/flatten.rs`): Add `word_has_command_substitution` and its helper functions as `pub(super)`.

3. **Step 3 — Set flag in `conch.rs`** (`libs/infrastructure/src/shell/conch.rs`): Update `collect_from_conch_simple` to compute and set `has_command_substitution`. Add integration tests to verify `echo $(git status)` produces `has_command_substitution: true` and `echo $HOME` produces `has_command_substitution: false`.

4. **Step 4 — Update policy checks** (`libs/domain/src/guard/policy.rs`):
   - Remove `has_expansion_marker`.
   - Rename/replace `command_contains_expansion` with `command_contains_command_substitution` (uses `cmd.has_command_substitution`).
   - Add `is_variable_reference` helper.
   - Add `git_subcommand_is_variable` helper.
   - Update `check_simple_command` to the new check sequence.
   - Update the test helper at the bottom of policy.rs to set `has_command_substitution: false` for all test-constructed `SimpleCommand` values (they are constructed manually, so the flag needs an explicit value). Add new test cases for the behaviors described in Section 6.

5. **Step 5 — Update any other `SimpleCommand` constructors**: Search for `SimpleCommand {` across the workspace and add `has_command_substitution: false` to all non-infra construction sites (tests, mock parsers).

6. **Step 6 — Update policy test helper**: The test-only `check()` function in `policy.rs` constructs `SimpleCommand` from a string split; it should set `has_command_substitution` by calling the existing `extract_command_substitutions` from `domain::guard::text` and checking if the result is non-empty. Or, simpler for unit tests: test cases that need `has_command_substitution: true` should set it explicitly in the struct literal.

---

## 9. Edge Cases and Known Limitations

### 9.1 Compound Command `has_command_substitution` Propagation

For compound commands (`for`, `while`, `if`), `collect_from_compound_kind` already propagates `redirect_texts` to all inner commands from `compound.io`. The same propagation currently applies for `has_output_redirect`. The `has_command_substitution` flag does NOT need this propagation because command substitutions in compound iterator words are already extracted as separate `SimpleCommand` entries (via `collect_command_substitutions_from_word`). The body commands are checked individually.

### 9.2 `VAR=$(cmd) command` — Env Var Assignment with Command Substitution

When an env var assignment like `VAR=$(git status)` precedes a command, `collect_from_conch_simple` processes it in `redirects_or_env_vars` as `EnvVar("VAR", Some(word))`. The word `$(git status)` has `word_has_command_substitution = true`. The flag is set on the surrounding `SimpleCommand`. The inner substitution body `git status` is also extracted as a separate `SimpleCommand` by the recursive walk. Both the outer command (flagged) and the inner `git status` (blocked by git detection) will be blocked. Defense in depth.

### 9.3 `SimpleCommand` Default Value for Test Helpers

When the domain-layer test helper (the `check()` function in `policy.rs`) constructs `SimpleCommand` manually, it must default `has_command_substitution` to `false` for literal test inputs. For test cases that need to verify the substitution block path, the test should construct `SimpleCommand { has_command_substitution: true, ... }` directly or use the real `ConchShellParser` (which lives in infrastructure tests). The policy unit tests use the manual constructor; they should add new cases using explicit `has_command_substitution: true`.

### 9.4 Flattened String Still Contains `$` in `redirect_texts`

Even after this change, `redirect_texts` still contains flattened strings like `$HOME` or `${VAR:-default}`. The `command_contains_git` check searches redirect_texts for "git" substring. A contrived token like `${gitfoo:-bar}` would match — this is an existing acceptable false positive documented in the policy comments.

---

## 10. Files to Modify

| File | Change |
|------|--------|
| `libs/domain/src/guard/types.rs` | Add `has_command_substitution: bool` field |
| `libs/domain/src/guard/policy.rs` | Replace `has_expansion_marker` / `command_contains_expansion` with new checks; add `is_variable_reference`, `git_subcommand_is_variable`; update test helper |
| `libs/infrastructure/src/shell/flatten.rs` | Add `word_has_command_substitution` and helpers |
| `libs/infrastructure/src/shell/conch.rs` | Set `has_command_substitution` in `collect_from_conch_simple` |

No changes to: `libs/domain/src/guard/port.rs` (ShellParser trait is unchanged), `libs/domain/src/guard/mod.rs` (no new exports needed), `libs/domain/src/guard/text.rs` (unchanged).
