# Design: reviewer process termination

## Problem Summary

local reviewer (`/track:review`) の実行経路が複数に分かれており、
repo 側が reviewer child process の lifecycle を制御できていない。

現状の主な食い違いは次。

- `.claude/commands/track/review.md` は `timeout 600 codex exec ...` 前提
- `.claude/agent-profiles.json` reviewer invoke example は `codex exec review --uncommitted --json --model {model} --full-auto`
- `.claude/rules/02-codex-delegation.md` と `.claude/skills/codex-system/SKILL.md` は raw `codex exec` guidance を持つ

その結果、reviewer loop の終端条件と read-only contract が repo の管理外になっている。

## Empirical Findings

2026-03-14 の sandbox 内観測では、`codex exec` が `--output-last-message` file を
書いた後も親 process 側が安定して終わらないケースがあった。

また、`codex exec review --help` では reviewer subcommand に `--output-last-message` はある一方、
明示 `--sandbox` option が見えなかった。さらに `--full-auto` は CLI help 上
`workspace-write` 寄りの alias と説明されている。

MVP の local reviewer で優先したいのは、
review loop を deterministic に終えられることと、
LLM の free-form wording に依存しない machine-readable final payload を持つことなので、
MVP では `codex exec review` より explicit read-only 制御を優先する。

## Chosen Architecture

### Public path

- user-facing command: `/track:review`
- reviewer provider resolution: 既存どおり `.claude/agent-profiles.json`
- provider が `claude` のときは inline review を継続
- provider が `codex` のときは repo-owned Rust wrapper を起動する
- Claude Code から実際に叩く runnable path は `cargo make track-local-review -- ...` とし、内部で Rust CLI subcommand を呼ぶ

### Internal path

canonical internal command:

```text
cargo run --quiet -p cli -- review codex-local --model {model} --briefing-file tmp/codex-briefing.md
```

public wrapper command:

```text
cargo make track-local-review -- --model {model} --briefing-file tmp/codex-briefing.md
```

実装の基点は Rust CLI に置き、public docs / provider examples / permission allowlist は cargo-make wrapper に揃える。

## Why Generic `codex exec`

MVP では `codex exec review` を canonical path にしない。

理由:

- read-only sandbox を明示的に保ちたい
- `--full-auto` 依存を reviewer canonical path から外したい
- local reviewer で必要なのは JSON review schema より deterministic な final verdict

そのため wrapper は generic `codex exec` を read-only で起動し、
`--output-schema` と `--output-last-message` を組み合わせて final JSON payload を回収する。

将来 `codex exec review` に read-only contract を明示できるなら、
wrapper の内部実装だけ差し替えればよい。

## Wrapper Lifecycle

1. review prompt または briefing file path を受け取る
2. runtime scratch path を `tmp/` 配下に作る
3. final message 用 file path を決める
4. reviewer JSON Schema file を runtime scratch 配下へ書く
5. `codex exec --model <model> --sandbox read-only --output-schema <schema-path> --output-last-message <path> ...` を spawn する
6. child stdout / stderr は relay する
7. Rust 側で timeout を監視する
8. timeout 超過なら child を kill して `timeout` として返す
9. child 終了後に final message file を読む
10. trimmed final message を JSON object として parse / validate する
11. final payload と exit status から repo-defined verdict を返す

## Final Payload Schema

MVP の wrapper が期待する final payload は次のどちらか。

```json
{"verdict":"zero_findings","findings":[]}
```

```json
{
  "verdict":"findings_remain",
  "findings":[
    {
      "message":"explain the bug or logic error",
      "severity":"P1",
      "file":"apps/cli/src/example.rs",
      "line":123
    }
  ]
}
```

MVP では許可する top-level key を `verdict` と `findings` に固定し、
wrapper もその 2 つだけを正本として読む。

validation rule:

- top-level は JSON object
- すべての object field は required とし、省略可能に見せたい field は `null` を使う
- `verdict` は `zero_findings` または `findings_remain`
- `zero_findings` では `findings` は空 array が必須
- `findings_remain` では `findings` は non-empty array が必須
- `--output-schema` は shape を固定し、wrapper は verdict/findings の semantic validation も行う
- malformed JSON や `verdict` 不整合は fail-closed

## Verdict Mapping

MVP の wrapper verdict enum は次の 5 状態だけを持つ。

- `zero_findings`
- `findings_remain`
- `timeout`
- `process_failed`
- `last_message_missing`

判定ルール:

- valid payload の `verdict == zero_findings` なら `zero_findings`
- valid payload の `verdict == findings_remain` なら `findings_remain`
- timeout で child を kill したら `timeout`
- child が non-zero exit かつ final message が無いなら `process_failed`
- final message が non-empty でも JSON parse / validation に失敗したら `process_failed`
- child が zero exit でも final message が無いなら `last_message_missing`

MVP では raw sentinel allowlist を持たず、success / findings 判定は JSON payload のみを正本にする。

## Read-only Contract

local reviewer wrapper は reviewer canonical path で `--full-auto` を使わない。

理由:

- reviewer は file write や git mutation を必要としない
- `--full-auto` は local reviewer の意図より広い権限 contract を連想させる

したがって docs / config / tests も reviewer local path を read-only 契約で説明する。

## Affected Surfaces

### Rust

- `apps/cli/src/main.rs`
- `apps/cli/src/commands/mod.rs`
- 新規 `apps/cli/src/commands/review.rs`

### Public docs / config

- `Makefile.toml`
- `.claude/settings.json`
- `.claude/agent-profiles.json`
- `.claude/commands/track/review.md`

### Guidance surfaces

- `.claude/rules/02-codex-delegation.md`
- `.claude/skills/codex-system/SKILL.md`

### Verification

- Rust tests for wrapper lifecycle and verdict normalization
- `.claude/hooks/test_agent_profiles.py`
- `scripts/verify_orchestra_guardrails.py`
- `scripts/test_verify_scripts.py`
- 必要に応じて orchestration tests

## Out of Scope

- `scripts/pr_review.py`
- PR review posting flow
- non-Codex reviewer provider support
- reviewer findings JSON schema redesign

## Implementation Notes

- reviewer wrapper は fail-closed を優先し、malformed JSON や曖昧な state を success にしない
- debug 用に final message output path を上書きできてもよいが、通常 path は wrapper 側で管理する
- stdout event log の永続保存は MVP の必須要件にしない
