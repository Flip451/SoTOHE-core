---
adr_id: 2026-04-08-1200-remove-agent-router-hook
decisions:
  - id: 2026-04-08-1200-remove-agent-router-hook_grandfathered
    status: accepted
    grandfathered: true
---
# agent-router フックを skill 遵守フックに置換

## Status

Accepted

## Context

`.claude/hooks/agent-router.py` は UserPromptSubmit フックとして動作し、ユーザープロンプトからキーワード/正規表現でインテント（capability）を検出し、対応するプロバイダー情報を `additionalContext` として注入していた。

このフックは約 850 行の Python コードで、以下の 2 つの機能を持つ：
1. **intent 検出 + ルーティングヒント注入**: 正規表現・キーワードマッチングでユーザーの意図を推定し、対応プロバイダーの実行例を注入
2. **external guide injection**: `/track:*` コマンド検出時に `external_guides.find_relevant_guides_for_track_workflow()` を呼び、関連ガイドサマリーを注入

機能 1 は以下の理由で冗長：
- Claude は `.claude/rules/08-orchestration.md` で capability ルーティングルールを既に把握している
- `agent-profiles.json` の `invoke_examples` で各 capability の実行例が定義済み
- ルーティング判断は LLM の自然言語理解で十分であり、500 行超のキーワードマッチングと重複している
- フックが出力するのは「ヒント」に過ぎず、Claude の判断を上書きしない（advisory のみ）

一方、フックが**持っていない**機能がある：
- `/track:plan` 等のスキルが呼ばれた際に、SKILL.md に定義されたフェーズ（Phase 1.5 の planner 呼び出し等）の遵守を強制する機能がない
- 実際に `/track:plan` を呼んだにもかかわらず planner capability を省略してしまう事故が発生した

## Decision

`agent-router.py` フックを廃止し、**skill 遵守フック**に置換する（WF-67）。

### 廃止する機能（intent 検出 + ルーティングヒント）

capability ルーティングは以下の既存メカニズムで十分にカバーされる：
- **ルーティングルール**: `.claude/rules/08-orchestration.md`（capability 選択の判断基準）
- **プロバイダー解決**: `.claude/agent-profiles.json`（capability → provider マッピング + invoke_examples）
- **ヘルパーライブラリ**: `.claude/hooks/_agent_profiles.py`（プロバイダー情報の API。skill 等から引き続き利用）

### 新設する機能（skill 遵守フック — Rust 実装）

UserPromptSubmit フックとして `sotp hook dispatch skill-compliance` を新設する：
- **実装言語**: Rust（`sotp hook dispatch` サブコマンド）。Python ではなく既存の Rust hook dispatch 基盤に統合する
- `/track:*` コマンドの検出時に、対応する SKILL.md のフェーズ要件をリマインドとして注入（例: 「Phase 1.5 planner review は Full モードで必須」）
- external guide injection の引き継ぎ（`scripts/external_guides.py` の機能を Rust に移植、または Rust から呼び出し）
- intent 検出・ルーティングヒント注入は**行わない**（rules + profiles に委譲）

Rust 移行の理由：
- `sotp hook dispatch block-direct-git-ops` / `block-test-file-deletion` と同じパターンに統合
- Python optional ポリシー（`command -v python3 >/dev/null 2>&1 || exit 0` で Python 不在時スキップ）により、Python フックは advisory に留まる。skill 遵守は advisory ではなく確実に実行される必要がある
- `bin/sotp` が存在すれば Python 不在でも動作する

## Rejected Alternatives

- **router をそのまま残す**: intent 検出の 500 行超が LLM 理解と重複。skill 遵守機能が無いため、本来必要な機能を提供していない
- **router を簡素化して残す**: キーワード数を減らしても、LLM との重複は解消しない。必要なのはルーティングではなく skill 遵守の強制
- **router を agent-profiles.json に統合**: トリガーパターンを JSON に持たせる案。同じ問題が残り、JSON が肥大化する
- **フックなしで rules のみに依存**: external guide injection が失われ、skill 遵守の強制手段もなくなる

## Consequences

- Good: intent 検出の冗長コード（~500 行）の削除
- Good: ルーティングの正本が rules + profiles の 2 箇所に集約される（router との 3 箇所管理が解消）
- Good: SKILL.md フェーズ遵守の強制により、planner 省略等の事故を防止
- Good: external guide injection 機能の継続
- Bad: 新フックの設計・実装コスト（ただし intent 検出より大幅に単純）

## Reassess When

- Claude Code のルールファイルロードの仕組みが変わり、rules が圧縮・省略されるようになった場合
- skill 遵守フックの実効性が低い場合（Claude がリマインドを無視するケースが多い場合は、より強制的なメカニズムが必要）
- プロバイダー数が増え、ルーティング判断が LLM だけでは困難になった場合（その時は intent 検出の復活を検討）
