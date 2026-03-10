# Codex Provider Guidance

**Codex CLI は、active profile が `planner` / `reviewer` / `debugger` / `implementer` を Codex に割り当てた時の specialist provider として使う。**

## 既定 profile での主担当 capability

### 1. `planner` / `reviewer`

- アーキテクチャ設計、モジュール構成
- 実装計画の策定（ステップ分解、依存関係整理）
- トレードオフ評価、技術選定
- コードレビュー（品質・正確性分析）

### 2. `debugger` / `implementer`

- Rust の所有権・ライフタイム設計
- 複雑なトレイト境界・ジェネリクス
- 非同期コードのデータ競合解析
- 根本原因が不明なコンパイルエラーの診断

補足: どの capability が実際に Codex を使うかは `.claude/agent-profiles.json` を正本とする。

## When Codex Is A Good Fit

| 状況 | 例 |
|------|------|
| **Rust 設計** | 「このトレイト設計は正しい？」「ライフタイムをどう設計する？」 |
| **所有権・借用エラー** | E0382（移動済み値）、E0505（借用衝突） |
| **計画が必要** | 「どう設計？」「計画を立てて」「アーキテクチャ」 |
| **デバッグ** | 「なぜ動かない？」「Rustコンパイラエラーの原因は？」 |
| **比較検討** | 「Arc vs Rc どちらがいい？」「async-trait vs RPITIT?」 |
| **コードレビュー** | 「このRustコードをレビューして」 |

## How to Consult When Assigned

`{model}` is resolved at runtime from `agent-profiles.json`:
profile `provider_model_overrides` > provider `default_model`.

### `planner` / `reviewer` 向け Read-only 例

```bash
codex exec --model {model} --sandbox read-only --full-auto \
  "Review this Rust trait design: {description}" 2>/dev/null
```

### `implementer` 向け With file access 例

```bash
codex exec --model {model} --sandbox workspace-write --full-auto \
  "Implement this Rust feature: {description}" 2>/dev/null
```

### `debugger` 向け Rust Compiler Error 診断例

```bash
codex exec --model {model} --sandbox read-only --full-auto "
Debug this Rust compiler error:
Error code: E0XXX
Full error: {error message}
Code: {relevant snippet}
Analyze root cause (ownership/lifetime/trait bound) and suggest a fix.
" 2>/dev/null
```

## Sandbox and Hook Coverage Warning

Claude Code hooks (`block-direct-git-ops.py`, `check-codex-before-write.py`, etc.) only intercept
**Claude Code's own tool calls**. They do NOT apply to operations performed inside a Codex subprocess.

| Sandbox | File writes | Git operations | Hook coverage |
|---------|-------------|----------------|---------------|
| `read-only` | Blocked by sandbox | Blocked by sandbox | N/A |
| `workspace-write` | Allowed | **Allowed — hooks do NOT fire** | None |

**Consequences when using `workspace-write`:**

- Codex can run `git add` / `git commit` / `git push` directly, bypassing `block-direct-git-ops.py`.
- Codex can write any file, bypassing `check-codex-before-write.py`.

**Rules for `workspace-write` usage:**

1. Prefer `read-only` for `planner` / `reviewer` / `debugger` — they should never need to write files.
2. When `implementer` is routed to Codex with `workspace-write`, instruct Codex explicitly:
   - Do not run `git add` or `git commit` directly.
   - Do not run `git push` under any circumstance.
   - For selective staging, write repo-relative paths to `tmp/track-commit/add-paths.txt` and run `cargo make track-add-paths`.
   - For guarded commits, use `/track:commit` or the exact wrappers `cargo make track-commit-message` / `cargo make track-note`.
3. Do not change `.takt/config.yaml` `provider` to `codex`. The takt provider must remain `claude`
   so that hook protections apply to all operations performed during autonomous task execution.

## Canonical Block Preservation

When a `planner` capability response contains a `## Canonical Blocks` section, Claude Code must
copy every block in that section verbatim into `plan.md`, `DESIGN.md`, or other design artifacts.

Canonical Blocks are the implementation-critical artifacts explicitly placed in the
`## Canonical Blocks` section by the specialist. They typically include:
- fenced `rust` code blocks (trait / struct / enum / error type definitions,
  signatures containing lifetimes / generics / trait bounds)
- module trees
- Mermaid `flowchart TD` diagrams

**Scope**: only the `## Canonical Blocks` section qualifies. Fenced blocks in other sections
(`## Rust Code Example`, `## Analysis`, `## Risks`, etc.) are illustrative and may be summarized
or omitted when writing to durable docs.

Claude Code may summarize or translate surrounding explanation text, but must not rewrite, omit,
translate, normalize, or partially copy blocks inside `## Canonical Blocks`.

If a Canonical Block cannot be embedded directly due to document format constraints, store the
full specialist output in `.claude/docs/research/{capability}-{feature}.md` and reference it from
the derived document instead of rewriting the block.

## Language Protocol

1. Ask Codex in **English**
2. Receive response in **English**
3. Execute based on advice
4. Report to user in **Japanese**
