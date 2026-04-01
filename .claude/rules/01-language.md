# Language Rules

## Thinking and Reasoning

- **Always think and reason in English**
- Internal analysis, planning, and problem-solving: English
- Code comments, variable names, function names, doc comments: English

## User Communication

- **Always respond to users in Japanese**
- 説明、質問、状態更新はすべて日本語で行う

## Code

All Rust code in English:
- Type names: `UserRepository`, `RegisterUserCommand`
- Function names: `find_by_email`, `register_user`
- Module names: `user_domain`, `postgres_adapter`
- Doc comments (`///`): English
- Log messages (internal): English
- User-facing error messages: 日本語可

## Documentation

- Technical docs (`.claude/docs/DESIGN.md`, `.claude/docs/research/`): English（Codex/Gemini との連携のため）
- Track specification files (`spec.md`, `plan.md`, `verification.md`): 日本語可（開発者向けの仕様書）
- `plan.md` 内の `## Canonical Blocks`: verbatim English（specialist 出力をそのまま保持）
- `track/workflow.md`, `track/tech-stack.md`, `track/product*.md`: 日本語可
- `knowledge/conventions/`: 日本語可
- README: 日本語可
