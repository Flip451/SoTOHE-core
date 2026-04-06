# Claude Code as Reviewer — Structured JSON Output Capability

Date: 2026-04-06
Source: Claude Code official documentation (headless.md, cli-reference.md)

## Summary

Claude Code CLI natively supports structured JSON output via `--json-schema`,
enabling its use as a code reviewer with stable, machine-parseable verdict output.
This can integrate directly into the review system v2 harness.

## Key Flags for Reviewer Integration

### 1. Non-interactive Execution (`-p` / `--print`)

Required for headless/CI operation. Runs Claude Code as a one-shot command.

```bash
claude -p "Review this code for correctness"
```

### 2. Bare Mode (`--bare`)

Skips hooks, skills, MCP servers, and CLAUDE.md auto-loading.
Ensures reproducible execution in CI and reviewer contexts.

```bash
claude --bare -p "Review this code"
```

### 3. JSON Output (`--output-format json`)

Returns a JSON envelope with the response in `.result` and metadata.

```bash
claude -p "Summarize" --output-format json
```

Response structure:
```json
{
  "result": "...",
  "session_id": "...",
  ...
}
```

### 4. Schema-Enforced Output (`--json-schema`) — Critical Feature

When combined with `--output-format json`, forces the model response to conform
to the provided JSON Schema. The schema-compliant output is placed in
`.structured_output` (not `.result`).

```bash
claude -p "Review this diff" \
  --output-format json \
  --json-schema '{ ... }'
```

Extract with:
```bash
... | jq '.structured_output'
```

### 5. System Prompt Override (`--system-prompt` / `--append-system-prompt`)

Inject reviewer-role instructions without modifying CLAUDE.md.

### 6. Cost & Scope Controls

| Flag | Purpose |
|------|---------|
| `--max-turns` | Limit agent turns (prevent runaway) |
| `--max-budget-usd` | API cost cap |
| `--permission-mode dontAsk` | Reject all tools not in `permissions.allow` |
| `--allowedTools` | Whitelist specific tools |

## Verdict Schema for Review System v2

The following schema matches the existing `track-local-review` output contract:

```json
{
  "type": "object",
  "properties": {
    "verdict": {
      "type": "string",
      "enum": ["zero_findings", "findings_remain"]
    },
    "findings": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "message": { "type": "string" },
          "severity": { "type": "string", "enum": ["P1", "P2", "P3"] },
          "file": { "type": ["string", "null"] },
          "line": { "type": ["integer", "null"] }
        },
        "required": ["message", "severity", "file", "line"]
      }
    }
  },
  "required": ["verdict", "findings"],
  "additionalProperties": false
}
```

## Example Invocation

```bash
REVIEW_SCHEMA='{"type":"object","properties":{"verdict":{"type":"string","enum":["zero_findings","findings_remain"]},"findings":{"type":"array","items":{"type":"object","properties":{"message":{"type":"string"},"severity":{"type":"string","enum":["P1","P2","P3"]},"file":{"type":["string","null"]},"line":{"type":["integer","null"]}},"required":["message","severity","file","line"]}}},"required":["verdict","findings"],"additionalProperties":false}'

claude --bare -p "Review this Rust implementation for correctness, safety, and idiomatic patterns:

$(git diff HEAD~1)

Report findings with severity (P1=must-fix, P2=should-fix, P3=nit).
If no issues found, return zero_findings." \
  --output-format json \
  --json-schema "$REVIEW_SCHEMA" \
  --max-turns 3 \
  --permission-mode dontAsk \
  | jq '.structured_output'
```

## Comparison with Existing Providers

| Aspect | Codex CLI (`track-local-review`) | Claude Code (`claude -p`) |
|--------|----------------------------------|---------------------------|
| Schema enforcement | `--output-schema` (wrapper validates) | `--json-schema` (native model constraint) |
| Sandbox | `--sandbox read-only` | `--permission-mode dontAsk` |
| Hook bypass | N/A (Codex ignores hooks) | `--bare` (explicit skip) |
| Streaming | Limited | `--output-format stream-json` |
| Cost control | Timeout only | `--max-budget-usd` + `--max-turns` |
| Model selection | `--model gpt-5.4` | Inherits configured model |
| Tool access | Full (sandbox-gated) | `--allowedTools` whitelist |

## Integration Considerations

### Advantages
- **Native schema enforcement**: `--json-schema` is model-level, more reliable than
  post-hoc parsing/validation
- **Tool access**: Claude Code can use Read/Grep/Glob to explore code context,
  unlike pure prompt-based review
- **Same ecosystem**: No external CLI dependency; uses the same Claude model stack
- **Cost visibility**: `--max-budget-usd` prevents runaway costs

### Risks & Mitigations
- **Self-review concern**: Claude reviewing Claude-written code may have blind spots.
  Mitigation: use `--bare` to strip project context, use different model tier,
  or combine with Codex as cross-provider validation.
- **Determinism**: LLM output is inherently non-deterministic. `--json-schema`
  guarantees structure but not content consistency.
  Mitigation: fail-closed validation in the harness wrapper.
- **Hook interaction**: Without `--bare`, project hooks may interfere.
  Mitigation: always use `--bare` for reviewer invocations.

## Recommended Architecture

```
track-local-review wrapper
  ├── provider: codex  → codex exec --output-schema ...
  ├── provider: claude → claude --bare -p --json-schema ...
  └── provider: ...    → (future extensible)
```

The `track-local-review` cargo-make task should resolve the provider from
`agent-profiles.json` and dispatch to the appropriate CLI with unified
verdict extraction (`.structured_output` for Claude, stdout parse for Codex).

## References

- Claude Code CLI Reference: https://docs.anthropic.com/en/docs/claude-code/cli-reference
- Claude Code Headless Mode: https://docs.anthropic.com/en/docs/claude-code/headless-mode
- Agent SDK Structured Output: https://docs.anthropic.com/en/docs/claude-code/sdk
