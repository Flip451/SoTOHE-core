# Shell Parsing Convention

## Single Parser Rule

All shell command parsing in the workspace MUST use `domain::guard::parser` (conch-parser backed).

**Do not** implement custom shell tokenization, separator normalization, or redirect handling
in any crate. The following patterns are prohibited outside `domain::guard::parser`:

- Custom `split_whitespace()` / `split()` based command tokenization
- Hand-rolled redirect operator detection (`>`, `>>`, `>&`, etc.)
- Custom `normalize_separators()` functions
- Manual quote stripping for shell tokens

## Available API

| Function | Purpose |
|----------|---------|
| `domain::guard::split_shell(input)` | Parse a full shell command into `Vec<SimpleCommand>` |
| `domain::guard::tokenize(input)` | Tokenize a single simple command (quote-aware) |
| `domain::guard::extract_command_substitutions(input)` | Extract `$(...)` and backtick contents |

`SimpleCommand::argv` returns **quote-stripped** tokens. No post-processing needed.

## Fail-Closed on Parse Error

`split_shell` returns `Err(ParseError)` for unparseable input (e.g., bash extensions like `<<<`
that conch-parser's POSIX parser does not support). Callers MUST treat parse errors as
potentially dangerous (fail-closed):

```rust
let commands = match split_shell(command) {
    Ok(cmds) => cmds,
    Err(_) => {
        // Fail-closed: block or flag as suspicious
        return handle_unparseable(command);
    }
};
```

## Rationale

In R17-R25 of the `phase1-sotp-hardening` review cycle, `usecase::hook.rs` maintained its own
shell tokenizer (`normalize_separators` + `shell_tokenize`) while `domain::guard::policy.rs`
already used conch-parser via `split_shell`. Each reviewer round found a new bypass
(redirects, fd digits, here-strings, quoted paths) because the hand-rolled tokenizer could
not match a proper parser's coverage. Migrating to `split_shell` eliminated all bypass classes
at once.

## Scope

This convention applies to all guard, hook, and policy code that inspects shell commands.
It does NOT apply to:

- String matching on non-shell input (e.g., markdown parsing, YAML frontmatter)
- Test helpers that construct shell command strings
