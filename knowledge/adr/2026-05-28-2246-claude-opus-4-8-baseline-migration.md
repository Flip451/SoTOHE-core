---
adr_id: 2026-05-28-2246-claude-opus-4-8-baseline-migration
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-opus-4-8-baseline-migration:2026-05-29"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-opus-4-8-baseline-migration:2026-05-29"
    status: proposed
---
# Claude モデル baseline を Opus 4.7 から 4.8 へ更新する

## Context

Anthropic が Claude Opus 4.8 をリリースした。本リポジトリで Claude を provider とする capability の model baseline は `.harness/config/agent-profiles.json` で `claude-opus-4-7` を明示指定している（orchestrator / spec-designer / impl-planner / type-designer / adr-editor / implementer / review-fix-lead の 7 capability）。この baseline を 4.8 に更新したい。

公式 migration guide によれば 4.7 から 4.8 は API 互換で破壊的変更はなく、4.7 向けの設定はそのまま動作する。挙動の違いとしては effort の既定が high に変わる点・1M context が既定化される点（beta header 不要）・effort レベルの token 配分が見直された点・mid-conversation system messages が追加された点・prompt cache の最小長が下がった点などがある。本 ADR は model baseline の更新に範囲を限定し、effort 方針は対象外とする。

着手前の調査で、変更が必要な箇所は当初の想定より狭いことが分かった。

- `.claude/agents/*.md` の frontmatter は `model: opus` というエイリアスを使っており、特定バージョンを指していない。エイリアスは最新の Opus に解決されるため、4.8 リリース後は編集しなくても 4.8 が使われる。
- `track/tech-stack.md` は Rust の技術スタック専用で、Claude モデルへの参照を持たない。
- `.claude/settings.json` の `CLAUDE_CODE_SUBAGENT_MODEL` は `claude-sonnet-4-6`（Sonnet）であり、本 ADR の対象ではない。
- reviewer（Codex `gpt-5.5`）と researcher（Gemini）は別 provider のため対象外。

つまり baseline の単一の真実の源泉は `agent-profiles.json` であり、明示指定された 7 capability のみが更新対象になる。

## Decision

### D1: agent-profiles.json の 7 Claude capability を claude-opus-4-8 に更新する

`.harness/config/agent-profiles.json` で Claude を provider とする 7 capability（orchestrator / spec-designer / impl-planner / type-designer / adr-editor / implementer / review-fix-lead）の `model` を `claude-opus-4-7` から `claude-opus-4-8` に更新する。reviewer / researcher / pr-reviewer は別 provider のため変更しない。

明示バージョン指定を維持するのは、どの model で workflow を検証したかを exact-version で記録に残すためである。reviewer の `gpt-5.5` / `gpt-5.4-mini` と同じ「外部に出す model はバージョンを明示する」慣例に揃える。model ID は素の `claude-opus-4-8` を使う（1M context は 4.8 で既定のため、長コンテキスト用のサフィックスは付けない）。

### D2: agent .md frontmatter は opus エイリアスのまま維持する

`.claude/agents/*.md` の `model: opus` は変更しない。エイリアスは最新の Opus に自動で解決されるため、4.8 への移行にあたって編集は要らない。`agent-profiles.json` は明示バージョン指定・各 agent の frontmatter はエイリアス、という非対称をそのまま許容する。

この非対称には、最新 Opus がリリースされたときに agent の挙動が自動で追従し、`agent-profiles.json` の明示値だけが記録として残る、という意図がある。食い違いが実害になる条件は Reassess When に記す。

## Rejected Alternatives

### A. agent-profiles.json も opus エイリアスに切り替える

profiles をエイリアス化すれば release のたびに編集する必要がなくなる。しかし、どの exact model で workflow を検証したかという baseline の記録を失う。本リポジトリはテンプレートであり、採用者が「この workflow がどの model で検証されたか」を後から辿れることに価値があるため却下した。

### B. agent .md も claude-opus-4-8 に明示指定する

profiles と agent .md を両方明示指定にすれば、両者が常に同じバージョンを指す厳密な対応関係になる。しかし agent .md は現状エイリアスで問題なく動いており、明示指定を足すと release のたびに更新する箇所が増える。エイリアスの自動解決で実害がなく、baseline の記録は profiles 側だけで足りるため却下した。

### C. tech-stack.md に Claude モデル baseline を新設する

`track/tech-stack.md` に AI capability のバージョンを記録する案。`agent-profiles.json` と二重管理になり、片方だけ更新されて食い違う原因になる。model baseline の真実の源泉は `agent-profiles.json` に一本化するため却下した。

## Consequences

### Positive

- Claude capability が Opus 4.8 の性能・1M context 既定・mid-conversation system messages などを利用できるようになる。
- baseline の記録が `agent-profiles.json` の exact-version に一本化された状態を保てる。
- 変更は `agent-profiles.json` の 7 箇所のみで、agent .md / tech-stack / settings は触らずに済む。

### Negative

- profiles は明示指定・agent .md はエイリアスという非対称のため、次の Opus（4.9 など）がリリースされると agent .md が先に新バージョンへ自動解決し、profiles の明示値と一時的に食い違う。各 release で profiles を追従更新する運用が要る。

## Reassess When

- 次の Opus（4.9 以降）がリリースされ、profiles の明示指定を追従更新するとき。そのタイミングで profiles / agent .md の指定方針（明示 vs エイリアス）の非対称を見直す。
- agent .md の `opus` エイリアスが profiles の明示値と食い違い、検証した model と実行される model の不一致が実害として観測されたとき。

## Related

- `knowledge/adr/` — ADR 索引
- `.harness/config/agent-profiles.json` — capability と provider / model の対応の真実の源泉
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR の配置・ライフサイクル規約
