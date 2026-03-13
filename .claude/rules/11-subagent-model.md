# Subagent Model

Default Claude Code subagent model is `claude-sonnet-4-6`.

Override guidance:

- Keep the default for normal planning, review, and routine implementation support.
- Override to `claude-opus-4-6` only when the task needs the highest reasoning depth, especially for complex Rust implementation, architecture refactors, or hard debugging.
- Do not downgrade to Haiku for normal track work. `claude-haiku-4-5-20251001` remains allowlisted only as an escape hatch for narrowly scoped, low-risk automation.

When documentation or prompts mention a subagent model, prefer describing the default plus override criteria rather than hardcoding Opus as the default.
