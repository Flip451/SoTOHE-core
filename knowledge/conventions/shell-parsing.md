# Shell Parsing Convention

## Single Parser Rule

All shell command parsing in the workspace MUST use the `ShellParser` port
(`domain::guard::ShellParser`) with the `ConchShellParser` adapter
(`infrastructure::shell::ConchShellParser`).

**Do not** implement custom shell tokenization, separator normalization, or redirect handling
in any crate. The following patterns are prohibited outside `domain::guard::text` and
`infrastructure::shell`:

- Custom `split_whitespace()` / `split()` based command tokenization
- Hand-rolled redirect operator detection (`>`, `>>`, `>&`, etc.)
- Custom `normalize_separators()` functions
- Manual quote stripping for shell tokens

## Available API

| Function / Trait | Location | Purpose |
|----------|----------|---------|
| `ShellParser::split_shell(input)` | `domain::guard` (port) | Parse a full shell command into `Vec<SimpleCommand>` |
| `ConchShellParser` | `infrastructure::shell` (adapter) | conch-parser backed implementation of `ShellParser` |
| `tokenize(input)` | `domain::guard` | Tokenize a single simple command (quote-aware) |
| `extract_command_substitutions(input)` | `domain::guard` | Extract `$(...)` and backtick contents |

`SimpleCommand::argv` returns **quote-stripped** tokens. No post-processing needed.

## Fail-Closed on Parse Error

`ShellParser::split_shell` returns `Err(ParseError)` for unparseable input (e.g., bash extensions
like `<<<` that conch-parser's POSIX parser does not support). Callers MUST treat parse errors as
potentially dangerous (fail-closed):

```rust
use domain::guard::{ShellParser, policy};

let commands = match parser.split_shell(command) {
    Ok(cmds) => cmds,
    Err(err) => {
        // Fail-closed: block or flag as suspicious
        return policy::block_on_parse_error(&err);
    }
};
let verdict = policy::check_commands(&commands);
```

## Rationale

In R17-R25 of the `phase1-sotp-hardening` review cycle, `usecase::hook.rs` maintained its own
shell tokenizer (`normalize_separators` + `shell_tokenize`) while `domain::guard::policy.rs`
already used conch-parser via `split_shell`. Each reviewer round found a new bypass
(redirects, fd digits, here-strings, quoted paths) because the hand-rolled tokenizer could
not match a proper parser's coverage. Migrating to `split_shell` eliminated all bypass classes
at once.

INF-20 moved the conch-parser dependency from domain to infrastructure behind the `ShellParser`
port trait, keeping domain free of external parser dependencies while preserving the single
parser rule.

## Scope

This convention applies to all guard, hook, and policy code that inspects shell commands.
It does NOT apply to:

- String matching on non-shell input (e.g., markdown parsing, YAML frontmatter)
- Test helpers that construct shell command strings
