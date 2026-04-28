---
adr_id: 2026-04-09-2235-agent-profiles-redesign
decisions:
  - id: 2026-04-09-2235-agent-profiles-redesign_grandfathered
    status: accepted
    grandfathered: true
---
# Agent Profiles Redesign (config relocation + schema cleanup)

## Status

Accepted — Implemented by track `agent-profiles-redesign-2026-04-10`

## Context

現行の `.claude/agent-profiles.json` は、ハーネス (この template project) における各 capability (orchestrator / planner / designer / implementer / reviewer / debugger / researcher / multimodal_reader) をどの provider (Claude / Codex / Gemini) とモデルで実行するかを定義する設定ファイルである。

### Prerequisite: Python hooks removal (先行 ADR)

本 ADR 実装の前提として、**ADR `2026-04-09-2323-python-hooks-removal.md` の実装が完了していること**。先行 ADR の完了後は以下の状態になっている:

- `.claude/hooks/` ディレクトリは完全に削除されている (Python hook なし)
- `.claude/hooks/_agent_profiles.py` (Python 側 loader) は存在しない
- `.claude/hooks/test_agent_profiles.py` など関連テストも存在しない
- `.claude/hooks/` に起因する Python runtime 依存が消滅している (`scripts/` Python と `requirements-python.txt` は引き続き存在)

したがって本 ADR では `.claude/agent-profiles.json` を読む実装は **`libs/infrastructure/src/agent_profiles.rs` (Rust) のみ** となり、二重実装の問題は既に解消済み。本 ADR のスコープは Rust loader の再設計と配置変更に集中できる。

### 現行の問題点

### 問題 1: 配置が不適切 — `.claude/` は Claude Code の設定ディレクトリ

`.claude/` ディレクトリは Claude Code (CLI ツール) 自身の設定を置く場所として慣例的に使われる。`agent-profiles.json` はこの template プロジェクト (ハーネス) の設定であり、Claude Code の設定ではない。

この配置による実害:

- **編集時の confirmation 疲労**: ハーネス開発中に `.claude/` 配下のファイルを編集すると Claude Code が毎回確認を求め、開発効率が低下する
- **責務の混同**: Claude Code のユーザー設定とハーネスのプロジェクト設定が同じディレクトリに並んでおり、何がどちらの責務なのか不明瞭
- **ツール非依存性の破壊**: このプロジェクトは Claude Code 以外の provider (Codex / Gemini) も使うため、Claude Code 専用ディレクトリに置くのは意味的に矛盾

### 問題 2: スキーマが散漫 — providers と profiles の情報重複

現行スキーマは大きく `providers` と `profiles` の 2 つに分かれているが、両者の責務が曖昧で情報が重複している:

```json
{
  "providers": {
    "claude": {
      "supported_capabilities": ["orchestrator", "planner", "designer", ...],
      "invoke_examples": { "planner": "/track:plan <feature>", ... }
    },
    "codex": {
      "default_model": "gpt-5.4",
      "fast_model": "gpt-5.4-mini",
      ...
    }
  },
  "profiles": {
    "default": {
      "planner": "claude",
      "designer": "claude",
      "reviewer": "codex",
      ...
    }
  }
}
```

- `providers.*.supported_capabilities` と `profiles.*.<capability>` は暗黙のリンクがあり、provider が `supported_capabilities` に持たない capability を profile で割り当てると logical error になる (静的検証されていない)
- `providers.*.invoke_examples` は capability 単位の command テンプレートだが、これはドキュメントで管理すべき情報であり、設定ファイルに置くのは責務違反
- `default_model` / `fast_model` は provider 単位だが、実際は capability ごとに異なるモデルを使う要求がある (例: reviewer は fast と default の 2 モデル、orchestrator は 1 モデルで十分)

結果として、「この capability を実行するのにどの provider と model を使うのか」を知るには `profiles` と `providers` の両方を参照する必要があり、SoT 性が失われている。

### 問題 3: 複数 profile を 1 ファイルに詰め込む設計

現行は 1 つの `agent-profiles.json` に `default` / `claude-heavy` / `codex-heavy` の 3 profile を詰め込み、`active_profile` フィールドで切り替える。

問題点:

- **user の自由度を制約**: ユーザーが自分の環境に合わせて profile をカスタマイズしようとすると、他の profile も同じファイルに存在するためコンフリクトの原因になる
- **profile 追加の重さ**: 新しい profile を試したいとき、既存ファイルを編集する必要があり、git 管理上のノイズになる
- **SoT の曖昧さ**: どの profile が「正式」なのかが `active_profile` に依存し、ファイル自体を見ても分からない
- **sample と active の混同**: `default` profile は template のサンプルなのか、実際に active にすべき推奨値なのか不明瞭

理想は「active な profile は 1 つだけをファイルに記述し、samples は別ファイルとして提供する」という分離。

## Decision

### 1. 配置を `.harness/config/agent-profiles.json` に移動

`.claude/agent-profiles.json` を廃止し、ワークスペース直下の新規ディレクトリ `.harness/config/` 配下に移動する:

- `.harness/config/agent-profiles.json` — active なハーネス設定 (ユーザーが編集するファイル)
- `.harness/config/samples/agent-profiles.default.json` — デフォルト推奨 profile のサンプル
- `.harness/config/samples/agent-profiles.claude-heavy.json` — claude 中心 profile のサンプル
- `.harness/config/samples/agent-profiles.codex-heavy.json` — codex 中心 profile のサンプル

`.harness/` は本 ADR で新設する **ハーネス専用** のトップレベルディレクトリで、将来的に `review-scope.json`、`architecture-rules.json`、`planning-artifacts.json` などのハーネス設定ファイルも段階的に `.harness/config/` に移行する (本 ADR では agent-profiles のみを対象とする)。

**`.harness/` を採用した理由**:

- **用語の一貫性**: このリポジトリは「ハーネス (harness)」として一貫して位置付けられており、ドキュメント全体で同じ用語を使用
- **意味の精度**: `agents/` では AI エージェントフレームワーク (crew-ai, langchain 等) と混同される。`harness` は「構造 / framework」を指し、provider (Claude / Codex / Gemini) の組み合わせを orchestrate するという本来の責務を表現
- **template 利用者の UX**: この template を派生させたプロジェクトでは、ユーザーが自分のアプリ用に `config/` ディレクトリを自由に使えるように、ハーネス設定の namespace を `.harness/` に隔離する
- **dot-prefix の慣習**: `.claude/`, `.github/`, `.vscode/` と同じく「meta / infrastructure」系であることを示し、通常のアプリコードと区別される
- **`.claude/` との区別**: `.claude/` は Claude Code 専用、`.harness/` は provider 横断のハーネス設定。Claude Code の confirmation 疲労を回避しつつ、用途も明確に分離

### 2. スキーマを capability 中心に再設計

`providers` と `profiles` の 2 層構造を廃止し、**capability を第一級の概念**として再編する。各 capability が直接 provider と model を指定する:

```json
{
  "schema_version": 1,
  "providers": {
    "claude": {
      "label": "Claude Code"
    },
    "codex": {
      "label": "Codex CLI"
    },
    "gemini": {
      "label": "Gemini CLI"
    }
  },
  "capabilities": {
    "orchestrator": {
      "provider": "claude",
      "model": "claude-opus-4-6"
    },
    "planner": {
      "provider": "claude",
      "model": "claude-opus-4-6"
    },
    "designer": {
      "provider": "claude",
      "model": "claude-opus-4-6"
    },
    "implementer": {
      "provider": "claude",
      "model": "claude-opus-4-6"
    },
    "reviewer": {
      "provider": "codex",
      "model": "gpt-5.4",
      "fast_model": "gpt-5.4-mini"
    },
    "researcher": {
      "provider": "gemini"
    }
  }
}
```

#### トップレベル構造

- `schema_version`: 1 から開始
- `providers`: provider 単位の **共通メタデータのみ** を保持 (label など、capability に依存しない属性)
- `capabilities`: capability → provider + model(s) の直接マッピング

#### `providers` の責務を最小化

`providers` セクションには **`label` のみ** を保持する (人間向けの表示名、ログ出力やエラーメッセージで利用)。

本 ADR 時点では provider に保持すべきグローバルパラメータは存在しない。将来、provider ごとに本当に必要な属性 (例: 認証方式の種別、バージョン情報など) が発生した場合にのみ追加する。**機能固有のパラメータ (例: review escalation の閾値) は機能側のコンフィグに配置し、`providers` には置かない** (責務の混同を避けるため)。

**廃止する項目**:

- `supported_capabilities`: 明示的な validation は行わず、`capabilities` 側が provider 名を自由に指定できる。provider の実装が capability をサポートしているかは CLI 側の責務
- `invoke_examples`: CLI 呼び出しテンプレートは設定ファイルの責務外。ドキュメント (`knowledge/` 配下、各 `.claude/rules/*.md` 等) で管理する
- `default_model` / `fast_model`: provider 共通のモデルという概念を廃止し、capability ごとに必要なモデルを直接指定する
- `escalation_threshold`: Codex 固有のフィールドだったが、(a) v2 では review escalation 未実装 (RV2-06 で将来対応予定)、(b) 機能固有パラメータを agent routing config に混ぜるべきではない、の 2 点から削除。将来 escalation を v2 で再実装する際は review 機能側のコンフィグに配置する

#### `capabilities` の各エントリ

各 capability は以下を持つ:

- `provider` (required): どの provider で実行するか (final round のデフォルト provider)
- `model` (optional): 使用する default model (final round)。省略時は provider が決めるデフォルト (Gemini など model 指定不要な場合もある)
- `fast_model` (optional): 2-tier review で使う fast round 用モデル (reviewer capability 等でのみ使用)
- `fast_provider` (optional): fast round 用の provider 上書き。省略時は `provider` にフォールバック。fast round で別 provider を使いたい場合のみ指定

**round_type 別の解決ルール**:

| round_type | provider resolution | model resolution |
|------------|---------------------|------------------|
| `final`    | `provider`          | `model`          |
| `fast`     | `fast_provider` → `provider` (fallback) | `fast_model` → `model` (fallback) |

#### fast round で別 provider を使う例

```json
"reviewer": {
  "provider": "claude",
  "model": "claude-opus-4-6",
  "fast_provider": "codex",
  "fast_model": "gpt-5.4-mini"
}
```

- final round: Claude Opus でレビュー
- fast round: Codex gpt-5.4-mini でレビュー

**`fast_provider` を省略した場合** (同一 provider 内で model のみ切り替え、デフォルト):

```json
"reviewer": {
  "provider": "codex",
  "model": "gpt-5.4",
  "fast_model": "gpt-5.4-mini"
}
```

- final round: Codex gpt-5.4
- fast round: Codex gpt-5.4-mini (`fast_provider` 省略 → `provider` フォールバック)

**将来の複数 round 対応**: 現状は reviewer のみが 2-tier 構成。将来 researcher など他 capability で複数 round が必要になった場合も、同じ `fast_*` prefix パターンで拡張可能 (さらに複雑な round 構成が必要になった時点で re-design を検討)。

#### capability 一覧

track workflow で実際に使用されているものに絞る (6 個):

- `orchestrator`: workflow の制御とユーザー対話 (Claude Code 本体)
- `planner`: 計画作成 (`/track:plan`)
- `designer`: ドメイン型設計 (`/track:design`)
- `implementer`: 実装 (`/track:implement`, `/track:full-cycle`)
- `reviewer`: コードレビュー (`/track:review`, `/track:pr-review`、fast_model を持つ)
- `researcher`: 調査・情報収集 (`/track:plan` Phase 1 の version baseline / codebase analysis)

**廃止する capability**:

- `workflow_host`: `orchestrator` と責務が実質同一 (現行 default profile でも両方 `claude`、model も同じ値を指定) だったため廃止。ワークフロー制御ホストの役割は `orchestrator` に一本化する
- `debugger`: track workflow のどのコマンドからも参照されていない。ルールファイル (`.claude/rules/02-codex-delegation.md`) での言及のみで実用性が弱い。デバッグが必要な場面は `implementer` や対話的な Claude Code 利用でカバーできる
- `multimodal_reader`: track workflow で一度も使われていない。PDF/画像/動画を読む必要があるケースはプロジェクト内で稀であり、必要になった時点で再追加する

将来 capability を追加する場合は、`capabilities` にエントリを追加するだけで済む (profile 側の変更も supported_capabilities の変更も不要)。逆に不要になった capability は本 ADR のように削除する (schema_version bump なしで行える変更)。

### 3. 単一 profile ファイル + 別途 samples 提供

`.harness/config/agent-profiles.json` には **1 つの profile のみ** を記述する。複数 profile を 1 ファイルに詰め込む現行方式は廃止する。

代替として `.harness/config/samples/` ディレクトリに複数のサンプルを用意する:

```
.harness/
└── config/
    ├── agent-profiles.json                      # active (user-edited)
    └── samples/
        ├── agent-profiles.default.json          # default 推奨
        ├── agent-profiles.claude-heavy.json     # claude 中心
        └── agent-profiles.codex-heavy.json      # codex 中心
```

ユーザーのワークフロー:

1. 初期状態では `.harness/config/agent-profiles.json` が存在しない (または `samples/agent-profiles.default.json` のコピー)
2. `.harness/config/samples/` から好みのサンプルを `.harness/config/agent-profiles.json` としてコピー
3. 自由に編集 (provider 変更、model 変更、capability 追加など)
4. ハーネスは `.harness/config/agent-profiles.json` のみを読み込む

`active_profile` フィールドは廃止。ファイル自体が active 設定である。

`.harness/config/samples/` は git 管理し、新しいサンプルの追加や既存サンプルの更新は PR で行う。**解決済み (2026-04-10)**: `.harness/config/agent-profiles.json` は git 管理で確定。template 派生プロジェクトの bootstrap 容易性を優先し、`.gitignore` には追加しない。

### 4. 読み込み API の刷新

`libs/infrastructure/src/agent_profiles.rs` (既存) を再設計し、新スキーマを読み込む API を提供する:

```rust
pub struct AgentProfiles {
    providers: HashMap<ProviderName, ProviderMetadata>,
    capabilities: HashMap<CapabilityName, CapabilityConfig>,
}

pub struct CapabilityConfig {
    provider: ProviderName,
    model: Option<ModelName>,
    fast_provider: Option<ProviderName>,
    fast_model: Option<ModelName>,
}

/// Fully resolved provider + model pair for a specific round.
pub struct ResolvedExecution {
    pub provider: ProviderName,
    pub model: Option<ModelName>,
}

pub enum RoundType {
    Fast,
    Final,
}

impl AgentProfiles {
    pub fn load(path: &Path) -> Result<Self, AgentProfilesError> { ... }
    pub fn resolve_capability(&self, capability: &str) -> Option<&CapabilityConfig> { ... }

    /// Resolve (provider, model) pair for a capability + round_type.
    ///
    /// - `RoundType::Final`: returns `(config.provider, config.model)`
    /// - `RoundType::Fast`: returns `(config.fast_provider ?? config.provider, config.fast_model ?? config.model)`
    pub fn resolve_execution(
        &self,
        capability: &str,
        round_type: RoundType,
    ) -> Option<ResolvedExecution> { ... }
}
```

`resolve_execution` は RV2-16 の `sotp review plan --round-type <fast|final>` で使われるエントリポイント。capability 名 + round_type を渡すと、provider と model の両方を同時に解決する (fast round で別 provider を指定した場合にも対応)。

`model` のみ必要な caller のために shortcut helper を提供してもよい:

```rust
impl AgentProfiles {
    pub fn resolve_model(&self, capability: &str, round_type: RoundType) -> Option<&ModelName> {
        self.resolve_execution(capability, round_type)?.model.as_ref()
    }

    pub fn resolve_provider(&self, capability: &str, round_type: RoundType) -> Option<&ProviderName> {
        self.resolve_execution(capability, round_type).map(|r| r.provider)
    }
}
```

### 5. 既存参照箇所の更新

本 ADR の実装タスクで、以下 2 種類の変更を広範囲のファイルに適用する:

**(a) 参照パスの切り替え**: `.claude/agent-profiles.json` → `.harness/config/agent-profiles.json`

**(b) 廃止 capability の参照削除**: `workflow_host`, `debugger`, `multimodal_reader` への言及を削除・更新する

対象ファイル (Python hook 関連は先行 ADR で削除済み):

- `libs/infrastructure/src/agent_profiles.rs`: ファイルパス + デコードロジック + 廃止 capability の扱い (この ADR の中核)
- `libs/infrastructure/src/verify/orchestra.rs`: ガードチェックパスとキーワード、capability リストの更新
- `.claude/rules/*.md`: 各種ルールドキュメントの参照更新
  - `02-codex-delegation.md`: `debugger` capability の言及を削除 (もしくは「直接対話で diagnose」に言い換え)
  - `03-gemini-delegation.md`: `multimodal_reader` capability の言及を削除
  - `08-orchestration.md`: capability 一覧から廃止項目を削除
  - `11-subagent-model.md`: 該当箇所の更新
- `.claude/commands/track/*.md`: コマンドドキュメント内の capability 言及の更新
- `CLAUDE.md`: プロジェクト index の参照更新
- `DEVELOPER_AI_WORKFLOW.md` / `knowledge/WORKFLOW.md`: capability 一覧表の更新
- `knowledge/conventions/`: 関連 convention の更新 (もしあれば)
- `LOCAL_DEVELOPMENT.md`: agent-profiles.json への言及更新
- `knowledge/research/*.md`: `workflow_host` 等を参照する既存文書は legacy として残す (既定では修正対象外) が、本 ADR で新規作成するタスクの spec/plan では新しい capability セットを使う

**reviewer からの指摘を避けるため、これらの更新は本 ADR の実装タスクに含める**。段階的に対応すると、移行期間中にドキュメントと実装の不整合が発生し、レビューで指摘される原因になる。

### 6. 後方互換性はサポートしない

本 ADR で行う変更は **全て breaking change** であり、既存 `.claude/agent-profiles.json` からの自動マイグレーションは実装しない:

- `.claude/agent-profiles.json` → 削除
- 旧スキーマ (`providers` + `profiles` 2 層) → 削除
- `active_profile` フィールド → 廃止
- 参照パスの自動書き換え → 手動更新で対応

理由: この template は個人開発 / 小規模運用で、マイグレーション対象の既存データが限定的であるため。マイグレーションコードを書くより、一度クリーンに作り直す方が効率的。

## Rejected Alternatives

### A. `.claude/` 配下に残し、スキーマだけ改善

配置の問題 (confirmation 疲労、責務の混同) が解決しない。配置とスキーマ両方を一度に直すべき。

### B. providers セクションを完全廃止

本 ADR 時点では providers の内容は `label` のみで、完全廃止 (label を capability エントリに直接埋め込むなど) も検討可能。しかし以下の理由で最小限の `providers` セクションを残す:

- 将来 provider ごとに本当に必要なグローバル属性 (認証情報、バージョン情報など) が発生した場合の拡張点として確保
- `label` を各 capability エントリに埋め込むと同じ provider が複数 capability で参照される時に重複する
- provider 名の一覧が `providers` を見れば分かるため、設定ファイルの自己記述性が高まる

逆に、capability に依存する (model、fast_model) や機能固有の設定 (review escalation の閾値など) は providers に置かない。

### C. profile 概念を残してカスタマイズのみ促す

複数 profile を 1 ファイルに詰め込む問題が解決しない。profile 概念そのものが冗長で、capability 中心の flat 構造の方がシンプル。

### D. `.harness/` 新設を避けて既存ディレクトリ (例: `track/`) に置く

`track/` は feature tracking の artifact を置く場所。設定ファイル置き場として責務が異なる。`.harness/` 新設が妥当。

### D2. dot-prefix なしの `config/` や `harness/` を使う

`config/` はアプリケーション設定と混同される可能性がある (template 派生プロジェクトで `config/` をアプリ側で使いたい場合にコンフリクト)。`harness/` は dot なしで通常のソースディレクトリと並ぶため「ハーネス実装コード」と混同されかねない。dot-prefix で meta config であることを明示するのが妥当。

### D3. `.agents/config/` を使う

"agents" は AI エージェントフレームワーク (crew-ai、langchain agents 等) と混同される余地がある。この template の設定は「エージェントそのもの」ではなく「provider の組み合わせを orchestrate するハーネス」に関するものなので、`.harness/` の方が意味的に正確。

### E. YAML / TOML に変更

既存の `review.json` / `metadata.json` 等が JSON のため、設定フォーマットの一貫性を崩すメリットがない。JSON を維持する。

### F. `active_profile` フィールドを残して 1 profile のみ記述

`active_profile` は複数 profile を前提とした仕組み。1 profile のみなら不要なフィールドで、設定ファイルの簡潔性を損なう。

## Consequences

### Good

- **編集 confirmation の解消**: `.harness/` 配下は Claude Code の保護対象外のため、編集時の確認プロンプトが不要になる
- **template vs アプリ config の境界明確化**: template 派生プロジェクトでユーザーがアプリ用の `config/` を自由に使える (ハーネス設定と衝突しない)
- **SoT の明確化**: capability → provider + model のマッピングが 1 箇所で完結、`providers` と `profiles` の相互参照が不要
- **カスタマイズの自由度**: ユーザーは `.harness/config/agent-profiles.json` を自由に編集でき、samples が別ファイルなのでコンフリクトしない
- **責務分離**: ハーネス設定とツール (Claude Code) 設定が別ディレクトリに分かれる
- **拡張容易**: 新 capability 追加は `capabilities` に 1 エントリ追加するだけ

### Bad

- **全参照箇所の手動更新が必要**: `.claude/agent-profiles.json` を参照している箇所が広範囲 (Python hooks、Rust コード、ドキュメント) に及ぶ
- **一時的な混乱**: 移行期間中はドキュメントと実装の整合性に注意が必要
- **`.gitignore` 戦略**: 解決済み — `.harness/config/agent-profiles.json` は git 管理で確定 (2026-04-10)
- **CI の影響**: `verify-orchestra-guardrails` などの既存ガードチェックが新パスを参照するよう更新が必要

## Reassess When

- provider ごとのグローバルパラメータが増えすぎて `providers` セクションが肥大化した場合: より細かい構造化を検討
- 1 ユーザーが複数 profile を環境変数で切り替えたい要求が強い場合: `AGENT_PROFILES_PATH` 環境変数で `.harness/config/agent-profiles.json` 以外を指定可能にする (既存の `CLAUDE_AGENT_PROFILES_PATH` と同じ考え方)
- `.harness/config/` 配下に他の設定ファイルを移行する際: 本 ADR の方針 (単一 SoT、breaking change 許容、samples 分離) を踏襲するか、ファイル特性に応じて再検討する
- capability の数が増え、特定 capability に provider-specific な動作パラメータが必要になった場合: `capabilities.<name>` に provider-specific フィールドを追加するか、`providers` 側に capability-specific なサブセクションを作るか判断する

## Related

- **ADR `2026-04-09-2323-python-hooks-removal.md`** (本 ADR の prerequisite): `.claude/hooks/` 配下の Python hook を全削除する ADR。本 ADR 実装時には Python 側の参照更新が不要になるため、スコープが縮小される。先行実装が必須
- **RV2-16 Planning Review Phase Separation (ADR `2026-04-09-2047-planning-review-phase-separation.md`)** (本 ADR の後続): `sotp review plan --round-type <fast|final>` がこの新 agent-profiles.json を読んで model を解決する。本 ADR 完了後に実装
- 将来: `review-scope.json`, `architecture-rules.json`, `planning-artifacts.json` の `.harness/config/` への段階的移行 (本 ADR の方針を踏襲)
