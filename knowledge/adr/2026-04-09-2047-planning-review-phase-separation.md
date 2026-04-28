---
adr_id: 2026-04-09-2047-planning-review-phase-separation
decisions:
  - id: 2026-04-09-2047-planning-review-phase-separation_grandfathered
    status: accepted
    grandfathered: true
---
# Planning Review Phase Separation (RV2-16)

## Status

Proposed

## Context

v2 review システム (scope-based parallel review) は実装フェーズのコードレビューに最適化されている。一方、計画フェーズの review には以下の問題がある:

### 問題 1: 無限ループ

計画 artifact (`metadata.json`, `spec.json`, `plan.md`, `spec.md`, `verification.md`) は現在 `review-scope.json` の `other` グループに分類され、通常の scope-based review を通る。v2-escalation-redesign-2026-04-09 の planning review で以下が実測された:

- **30+ ラウンド** でも zero_findings に収束しない
- レビューアが SoT (`metadata.json` / `spec.json`) と View (`plan.md` / `spec.md` / `verification.md`) の両方を読み、同じ情報の表現齟齬を次々と指摘する
- 1つ直すと別の section で同じ話題の類似 finding が出る
- view-freshness CI で整合性は既に保証されているが、レビューアはそれを知らず cross-file consistency を延々と検証する

### 問題 2: 必要度の曖昧さ

計画レビューの指摘には「必ず直すべき (仕様の穴)」から「表現の揺らぎ」まで幅広い必要度があるが、現状の verdict JSON には severity (P0-P2) しかなく、必要度を表現できない。結果、全ての finding が同じ重みで扱われ、些末な指摘でもループが続く。

### 問題 3: フェーズ境界の不明瞭さ

計画フェーズと実装フェーズの境界が曖昧で、同じ review 機構を両方に使うため、計画品質と実装品質のレビュー観点が混在する。

## Decision

### 1. 計画専用 CLI コマンドを新設 (既存 review システムは無変更)

既存の scope-based review システム (`sotp review codex-local`, `review.json`, `review-scope.json`, `track-check-approved`) は一切変更しない。代わりに計画フェーズ専用の独立した CLI コマンドを追加する:

- `sotp review plan --round-type <fast|final> [--briefing-file <path>]`: 計画 artifact の view のみをレビュー
- `sotp commit plan`: 計画 artifact のみをコミット

#### `sotp review plan` のモデル自動解決

既存 `sotp review codex-local` は呼び出し側で agent-profiles.json を読んで `--model <name>` を explicit に渡す設計だが、本 ADR の `sotp review plan` は **モデル解決をコマンド内部で行う**:

- 入力: `--round-type <fast|final>` (必須)、`--briefing-file <path>` (optional)
- 内部で `.harness/config/agent-profiles.json` (prerequisite ADR `2026-04-09-2235-agent-profiles-redesign.md` で移行済み) を読み込み、以下のルールで model を解決:
  - `capabilities.reviewer` エントリを取得
  - `round-type == final` → `(provider, model)` を返す
  - `round-type == fast` → `(fast_provider ?? provider, fast_model ?? model)` を返す
  - capability 不在、provider 解決失敗などはエラー終了
- 呼び出し側は model / provider 名を知らなくてよい

**モデル自動解決の根拠**: agent-profiles.json を読む処理は prerequisite ADR で再設計された `libs/infrastructure/src/agent_profiles.rs` (`AgentProfiles::resolve_execution` API) をそのまま再利用する。コマンドごとに caller 側で model 解決を重複実装するより、CLI 内部で解決する方が DRY で UX も良い。

#### cargo make wrapper

両コマンドとも `cargo make` wrapper を併設する (既存 `cargo make track-local-review` / `track-commit-message` の慣例に準じる):

- `cargo make track-plan-review` → `sotp review plan --round-type final` を呼び出す (デフォルトは final、fast 指定は wrapper 側で別タスクまたは引数渡し)
- `cargo make track-plan-commit-message` → `sotp commit plan` を呼び出す (`tmp/track-commit/commit-message.txt` からコミットメッセージを読む既存パターンを踏襲)

Claude Code からは必ず cargo make wrapper 経由で呼び出し、`sotp` バイナリ直接呼び出しはしない。これにより hook / guardrails / worker 隔離の整合を保つ。

### 2. 必要度分類付き verdict (Necessity enum)

planning reviewer は各 finding に必要度 (`Necessity`) を必須で分類する:

- **must**: この review cycle で必ず修正すべき (correctness, architecture 不整合, 仕様の穴、実装を正しく進めるために不可欠な指摘)
- **advisory**: 修正が望ましいが、ブロックしない (表現改善、代替案、トレードオフの議論、レビュアーの主観的な改善提案)
- **info**: 情報提供のみ (背景、FYI、観察、参照情報、action を要求しない)

**Gate rule**: `must` count == 0 の時のみ gate 通過。advisory/info は通過 (warning / info で display)。

必要度分類は Codex `--output-schema` で required フィールドとして強制する。**fail-closed**: `necessity` 欠損時は `must` として扱う (安全側)。

#### Calibration examples

reviewer が必要度を適切に判断できるよう、briefing に以下の例を自動注入する:

- `"resolve_escalation_v2 に scope 引数が必須だが仕様に明記されていない"` → **must** (仕様の穴、実装が曖昧になる)
- `"T05 の description が T07 と表現齟齬"` → **advisory** (内容は一致、表現の揺らぎ)
- `"Mermaid 図があると理解しやすい"` → **info** (任意の改善案)
- `"plan-review.json の schema が既存 review.json と乖離"` → **must** (設計の一貫性に関わる)
- `"task description の文言を XXX から YYY に統一した方が読みやすい"` → **advisory** (好みの問題)
- `"この ADR は関連 ADR 2026-03-24-1210 とも併読すると背景が明確"` → **info** (参考情報)
- 判断に迷う場合は **高い必要度** に倒す (must が判断閾値以下の場合に advisory へ降格)

これらの例は system-level contract として CLI バイナリに固定で埋め込まれ、全 review 呼び出しで同じテキストが注入される。

### 3. 計画 artifact の中央集権 allowlist

`track/planning-artifacts.json` に計画 artifact のパターンを集約する:

```json
{
  "schema_version": 1,
  "track_artifacts": [
    "track/items/<track-id>/metadata.json",
    "track/items/<track-id>/spec.json",
    "track/items/<track-id>/plan.md",
    "track/items/<track-id>/spec.md",
    "track/items/<track-id>/verification.md",
    "track/items/<track-id>/domain-types.json",
    "track/items/<track-id>/domain-types.md"
  ],
  "auxiliary_artifacts": [
    "knowledge/adr/**/*.md",
    "knowledge/research/**/*.md",
    "knowledge/strategy/**/*.md"
  ]
}
```

**2 種類の allowlist**:

- `track_artifacts`: `<track-id>` placeholder 付きの glob パターン。実行時に **現在のトラック id** で `<track-id>` を literal 置換してから match する。他のトラックのファイル (`track/items/<other-id>/*`) は placeholder 置換後のパターンに含まれないため、**異なるトラックの artifact を同時 commit できない** (single-track 境界を強制)。`*` ではなく `<track-id>` とすることで scope を現在のトラックに限定する意図を明示する
- `auxiliary_artifacts`: ワークスペース相対 glob パターン (placeholder なし)。計画時に参照・更新される横断ドキュメント (ADR, research, strategy) を対象にする。これらは特定トラックに紐付かないため track-id 展開しない

両カテゴリとも glob 形式で統一されているため、config 読み込み側のロジックは「`<track-id>` があれば置換 → glob match」という単一パスで扱える。

`sotp commit plan` は staged files を `resolved(track_artifacts) ∪ auxiliary_artifacts` と照合し、全て match する時のみ commit を許可する。`track/registry.md` は `.gitignore` 済みのため allowlist から除外。

### 4. 計画フェーズと実装フェーズの境界 (review.json は fail-safe な追加ゲート)

以下の片方向の含意が成立する:

- `review.json が存在する → 実装フェーズ以降である` (one-way implication, 成立)

逆 (`review.json が存在しない → 計画フェーズである`) は成立しない (例: 実装フェーズだがまだ一度も local `/track:review` を実行していない、PR ベースレビューのみ使用、過去に review.json をリセット、など)。

しかし、片方向の含意だけでも `/track:commit-plan` のゲート条件として十分有用である:

- `review.json が存在する` → 実装フェーズ以降 → 計画 commit は不適切 → **block** (fail-safe)
- `review.json が存在しない` → phase 不明だが、後続の allowlist / plan-review.json / hash / must_count ゲートで安全を確保

したがって `review.json` 存在チェックは gate の 1 段階として組み込む。ただし「phase 検出の唯一手段」ではなく、「実装フェーズ確定時の fail-safe ブロック」として位置付ける。

#### 推奨ワークフロー

- **計画フェーズ** (review.json 不在): `/track:plan` / `/track:design` で artifact 作成 → `/track:review-plan` で品質確認 → `/track:commit-plan` でコミット
- **実装フェーズ** (review.json 存在以降): `/track:implement` / `/track:full-cycle` で実装 → `/track:review` (既存) でコードレビュー → `/track:commit` (既存) でコミット。`/track:commit-plan` は review.json 存在ゲートで自動的にブロックされる
- **実装フェーズで計画を refine したい場合**: 通常の `/track:commit` で実装変更と一緒にコミットする (既存 review システムで扱う)

計画レビューの無限ループ問題は「diff がほぼ 100% 計画 artifact」の時に発生しており、コードが混在する diff ではレビューアの注意がコードに向くため実測上問題ない。

### 5. `plan-review.json` による gate

`track/items/<id>/plan-review.json` に planning review 結果を永続化する。スキーマは **ADR `2026-04-04-1456-review-system-v2-redesign.md` §永続化: review.json v2 で定義された構造を踏襲** し、finding entry に `necessity` フィールド (required) を追加するのみ。

既存 `review.json` との差分:

- トップレベル構造 (`schema_version`, `scopes.<name>.rounds[]`) は **完全一致**
- round entry の `type` / `verdict` / `findings[]` / `hash` / `at` フィールドは **完全一致**
- finding entry に **`necessity` フィールド (required) を追加** (`must` / `advisory` / `info`)
- scope 名は `"planning"` 固定 (計画レビューは単一 scope)
- `schema_version` は 1 から開始 (plan-review.json 独自の version 管理)

追加される `necessity` フィールドの例:

```json
{
  "message": "T05 の threshold 定義が曖昧",
  "severity": "P1",
  "file": "plan.md",
  "line": 42,
  "category": "correctness",
  "necessity": "must"
}
```

既存 `review.json` の設計 (rounds 配列による履歴保持、fast/final 2 段階レビュー、hash ベースの stale 検出、fs4 ファイルロックによる atomic write) をそのまま活用できる。infra 実装も `FsReviewStore` と同じパターンで `FsPlanReviewStore` を作る。

#### `hash` フィールドの計算対象

`hash` は **レビュー対象となった全ファイル** から計算する (既存 review.json の scope hash と同じ考え方):

- 現在のトラックの view (`plan.md`, `spec.md`, `verification.md`, `domain-types.md` if exists)
- `auxiliary_artifacts` の glob に match し、かつ直近の diff (対 main) に含まれるファイル (新規追加・変更された ADR / research / strategy ドキュメント)

SoT (`metadata.json`, `spec.json`, `domain-types.json`) は hash 対象外 (view-freshness CI が整合性を保証)。

レビュー対象に auxiliary を含める理由: 計画段階では ADR による設計決定の確定、research ドキュメントの参照追加、strategy ドキュメント (TODO.md 等) への新項目追加などが頻繁に発生する。これらは計画品質の一部として review すべき。

#### Gate 判定に使う round

`plan-review.json` は rounds の履歴を保持するが、gate 判定には **scopes.planning.rounds の最新 entry** を使う:

- 最新 round の `hash` が現在の計算値と一致 (stale でない)
- 最新 round の `findings[]` 全てで `necessity != "must"` (すなわち must count == 0)

過去の round 結果は履歴参照用に保持 (review.json と同じ方針)。

### 6. `/track:commit-plan` の 5 段階ゲート

```
1. staged files 全てが resolved(track_artifacts) ∪ auxiliary_artifacts に match
2. track/items/<id>/review.json が存在しない (実装フェーズ確定時の fail-safe ブロック)
3. track/items/<id>/plan-review.json が存在する (review 未実行でない)
4. plan-review.json の scopes.planning.rounds[-1].hash が現在の view ファイルハッシュと一致 (stale でない)
5. plan-review.json の scopes.planning.rounds[-1].findings[] で necessity == "must" のエントリが 0 件
```

全て満たして commit 可能。各条件の reject には対応する復旧案内を英語で出す:

- (1) reject: `non-planning files in staged diff: <file>, <file>... Use /track:commit for code changes.`
- (2) reject: `review.json already exists — track is past planning phase. Use /track:commit for commits during implementation.`
- (3) reject: `plan-review.json not found. Run /track:review-plan first.`
- (4) reject: `plan-review.json is stale — view files changed since last review (hash mismatch). Run /track:review-plan again.`
- (5) reject: `N "must" finding(s) remain in plan-review.json. Address them and re-run /track:review-plan.`

**gate (2) の論理的根拠**: `review.json 存在 → 実装フェーズ以降` は成立するため、存在時のブロックは正当。逆含意は成立しないが gate として問題ない (存在しない場合は後続 gate で安全確保)。

### 7. `/track:review-plan` のフロー

`sotp review plan` は既存 `ReviewCycle::review` の before/after hash 比較パターン (ADR `2026-04-04-1456-review-system-v2-redesign.md` 参照) を踏襲する。hash 計算はレビュー実行の **直前と直後** に行い、差分があれば stale としてエラーを返す (レビュー中のファイル変更を検出)。

以下の全ステップは `sotp review plan` コマンド **内部の処理** であり、呼び出し側 (`cargo make track-plan-review` wrapper / `/track:review-plan` コマンド) は track id の解決や file 列挙を行わない。呼び出し側は optional な `--briefing-file` (トラック固有コンテキスト) を渡すのみ。

1. (内部) 現在のトラック id を git ブランチから解決
2. (内部) `.harness/config/agent-profiles.json` を読み込み、`--round-type` に応じて **(provider, model) を自動解決** (prerequisite ADR `2026-04-09-2235` で再設計された `AgentProfiles::resolve_execution` API を使用。`fast` → `(fast_provider ?? provider, fast_model ?? model)`、`final` → `(provider, model)`)
3. (内部) `track/planning-artifacts.json` を読み込み、glob パターンに従ってレビュー対象ファイルを **自動検出** する:
   - `track_artifacts`: `<track-id>` を literal 置換 → view `.md` ファイルに絞り込み (SoT JSON は除外)
   - `auxiliary_artifacts`: workspace に対して glob match → さらに diff (対 `.commit_hash` または main) 内のファイルのみに絞り込み
4. (内部) **hash_before** を計算 (検出されたレビュー対象ファイル群のハッシュ、既存 `ReviewHasher` と同じ方式)
5. (内部) briefing に **以下を自動注入** する (既存 `CodexReviewer::build_full_prompt` と同じパターン):
   - 検出されたレビュー対象ファイル一覧
   - **必要度分類 (Necessity) の定義と指示**
   - **Gate rule** (`must_count == 0` でゲート通過)
   - **Calibration examples** (must / advisory / info の判断例)
   - **fail-closed の扱い** (必要度欠損 → must)
   - ユーザー提供の optional briefing file の内容 (あれば末尾に追加)
6. (内部) Codex に必要度分類付き briefing で review 依頼 (step 2 で解決した model を使用、`--output-schema` で `necessity` required 強制)
7. (内部) verdict を parse (fail-closed: `necessity` 欠損 → `must`)
8. (内部) レビュー対象ファイルを **再度列挙** し、**hash_after** を計算 (step 3 と同じロジックで再実行)
9. (内部) `hash_before != hash_after` なら `FileChangedDuringReview` エラーを返して終了 (plan-review.json は更新しない)
10. (内部) `plan-review.json` に atomic write (hash は `hash_after` を記録、既存 `FsReviewStore` の fs4 ロックパターンを踏襲)
11. (内部) verdict を stdout に出力 (`must_count > 0` なら exit 2、else exit 0)

**before/after 比較の意図**: レビュー実行中 (Codex 呼び出し中) にユーザーが並行して view ファイルを編集した場合、reviewer が見た内容と persist される内容が食い違う。hash 差分で検出して fail-closed にすることで、stale verdict の記録を防ぐ。

**gate 判定時の stale 検出** (`/track:commit-plan` の gate (4)): 最新 round の `hash` と、gate 実行時に再計算した hash を比較する (既存 review.json の `Required(StaleHash)` 判定と同じロジック)。レビュー完了後に view が変更されればこの gate で弾かれる。

**自動注入のメリット**:
- ユーザーがレビュー対象ファイルや必要度分類の指示を手動で記述する必要がない
- 必要度分類の定義・gate rule・calibration examples は **システムレベルの契約** であり、トラックごとに変わらない
- `planning-artifacts.json` と内蔵 necessity template が **単一 SoT** となり、briefing の記述揺らぎを排除
- ユーザー提供の briefing file は optional で、**トラック固有のコンテキスト** (設計背景、既知の accepted deviations など) のみに集中できる
- 既存 `cargo make track-local-review --briefing-file` と同じ UX を維持 (briefing file はユーザー追加コンテキスト、システム要素は自動注入)

### 7.1 自動注入される必要度分類テンプレート

実装は新 CLI 内部に固定 template を持ち、全 review 呼び出しで同じテキストを注入する。template の内容は §2 を参照する (定義 + gate rule + calibration examples + fail-closed)。将来 template を更新する場合は CLI バイナリを更新する (設定ファイル化しない — system-level contract として versioning する)。

### 8. 既存 `track-check-approved` との関係

`track-check-approved` は **既存のまま** (review.json ベース) で、plan-review.json を参照しない。`/track:commit-plan` は独自に plan-review.json をチェックするため、統合は不要。

### 9. 後方互換性はサポートしない

本 ADR で導入する新規要素 — `track/planning-artifacts.json`、`track/items/<id>/plan-review.json`、`sotp review plan`、`sotp commit plan`、`cargo make track-plan-review` / `track-plan-commit-message`、`/track:review-plan` / `/track:commit-plan` — は **全て新規追加** であり、既存システムとの後方互換性を考慮しない:

- **`plan-review.json` の schema version migration なし**: schema_version 1 から開始。将来のスキーマ変更は `schema_version` を上げた時点で旧データを破棄 (decode 時に無視、新規作成し直す)。マイグレーションパスは実装しない
- **`planning-artifacts.json` の schema version migration なし**: 同上。schema_version 1 固定で開始し、将来変更時は旧データ破棄
- **Legacy フォールバックなし**: `sotp review plan` は新しい flow のみをサポート。旧来の `sotp review codex-local` への fallback は実装しない
- **既存トラックへの retrofit なし**: 本 ADR 実装以前に作成されたトラックでは、`plan-review.json` が存在しないため `/track:commit-plan` の gate (3) で reject される。必要なら `/track:review-plan` を実行して plan-review.json を生成すれば使える
- **既存 `/track:commit` との共存**: `/track:commit-plan` は追加オプションであり、`/track:commit` の動作は一切変えない。ユーザーは既存フローのまま `/track:commit` を使い続けることも可能

後方互換性を放棄する理由:

- 本 ADR の対象は **新規機能** であり、移行対象の legacy データが存在しない
- migration コードを書くと複雑化し、バグの温床になる
- プロジェクトは個人開発でユーザー数が限定的なため、breaking change の影響範囲が小さい
- 将来のスキーマ変更時は `schema_version` bump + 旧データ破棄で対応し、migration コードを書かない方針を継続する

## Rejected Alternatives

### A. `--view-only` フラグを既存 `track-local-review` に追加

既存 review システムに変更を加えることになる。scope 分類ロジックに mode を追加する必要があり、race condition やリグレッションのリスクがある。

### B. `review-scope.json` から planning artifact を除外

planning artifact は実装フェーズでも変更される (task 遷移、commit_hash 記録など) ため、無条件除外は不可能。phase 判定を review-scope.json に持ち込むと設定ファイルの責務を超える。

### C. briefing template のみで「view だけ見ろ」と指示 (prompt-only)

強制力がなく、reviewer が briefing を無視すれば無限ループに逆戻りする。既存 review システムの review.json / scope 分類が変わらないため、gate ロジックも不明瞭。

### D. severity (P0-P2) ベースでのゲート緩和

必要度は severity と独立した概念 (P1 でも advisory は存在する)。severity 閾値で gate を緩めると、P0 の些末な指摘 (typo 等) でブロックし続ける。必要度は reviewer の判断で決めるべき。

### E. 混在 commit を許可 (計画 + コード同一コミット)

コミットの atomic 境界が曖昧になり、`git log` から「何が計画変更で何が実装変更か」が読めない。レビュー scope も混在し、既存 review 問題に回帰する。

### F. `check-approved` を phase 判定で分岐

既存 `check-approved` に phase 判定を追加すると、現状の単純な実装が複雑化する。新コマンド `sotp commit plan` を独立させれば `check-approved` は無変更で済む。

### G. `review.json` 存在チェックを phase の双方向判定器として使う

「review.json 存在 → 実装フェーズ以降」は成立するが、その逆「review.json 不在 → 計画フェーズ」は成立しない (例: 実装フェーズだが一度も local `/track:review` を実行していない、PR ベースレビューのみ使用、など)。

このため `review.json` を唯一の phase 判定器として使って「存在しなければ計画フェーズ確定」とする設計は誤り。本 ADR では代わりに、片方向の含意 (`存在 → 実装フェーズ以降`) のみを活用して **実装フェーズ確定時の fail-safe ブロック** として gate (2) に組み込む。review.json 不在時は allowlist / plan-review.json / hash / must_count の後続 gate で安全を確保する。

## Consequences

### Good

- **無限ループ排除**: 必要度ベース gate により明確な脱出条件 (`must_count == 0`)
- **既存 review システム無変更**: 実装フェーズのレビュー機構に一切リスクを加えない
- **フェーズ境界の明確化**: `review.json` 存在有無で物理的に分離
- **atomic commit 境界**: 計画変更と実装変更がコミット単位で分離され、`git log` が読みやすい
- **集中管理**: planning artifact の定義が `track/planning-artifacts.json` に一元化され、将来の変更が容易
- **計画品質の可視化**: advisory / info finding が `plan-review.json` に蓄積され、PR レビューの議論材料になる

### Bad

- **新コマンドの学習コスト**: 開発者は `/track:commit` と `/track:commit-plan` の使い分けを覚える必要がある
- **実装フェーズでの計画更新は既存 review 経由**: これは計画単独 review の無限ループ問題を継承しうるが、混在 diff ではコードに注意が向くため実測上マシ。将来課題として残す
- **reviewer 依存の必要度分類**: reviewer が全て `must` 分類すると loop 脱出できない。briefing の calibration examples で緩和するが完全防止ではない
- **plan-review.json の state 管理コスト**: 新しい永続化層が増え、atomic write / view hash 同期などの実装負担がある
- **hook / workflow 整合性**: `block-direct-git-ops` 等のフック設定、`/track:*` コマンド一覧、wrapper タスクに新規追加が必要

## Reassess When

- reviewer (Codex gpt-5.4) の必要度分類精度が不十分と判明した場合: briefing の calibration examples を拡充するか、fail-closed の扱いを見直す
- 実装フェーズで計画レビュー無限ループが再発した場合: 混在 diff 時の review 観点を見直す必要あり (RV2-06 で対処予定の v2 escalation と組み合わせるなど)
- `plan-review.json` の schema に追加要素 (approver 情報、history など) が必要になった場合: schema_version を上げてマイグレーションする
- `/track:commit-plan` の allowlist がプロジェクトごとに変わる要件が発生した場合: `track/planning-artifacts.json` の per-project カスタマイズ方針を定義する
- 計画フェーズから実装フェーズへの逆戻り (例: 実装開始後に大幅な計画変更) が必要な要求が出た場合: `review.json` のリセット方法と `/track:commit-plan` の再有効化フローを定義する

## Related

**Prerequisite chain** (実装順序):

1. **ADR `2026-04-09-2323-python-hooks-removal.md`** (RV2-17): `.claude/hooks/` 配下の Python hook を全削除。これにより後続 ADR の Python 側参照更新が不要になる
2. **ADR `2026-04-09-2235-agent-profiles-redesign.md`**: `.harness/config/agent-profiles.json` への移行と capability 中心スキーマへの再設計。`sotp review plan --round-type <fast|final>` のモデル自動解決が依存する
3. **本 ADR** (RV2-16): Planning review phase separation

上記 1 → 2 → 3 の順で実装する。各 ADR は前段の完了を前提とする。

**その他**:

- **ADR `2026-04-04-1456-review-system-v2-redesign.md`**: review.json v2 スキーマ・`ReviewCycle` の before/after hash 比較パターン・`FsReviewStore` の fs4 ロックパターンの定義元。本 ADR はこれら全てを踏襲する
- RV2-06: v2 Review Escalation Re-design — 必要度分類と似た概念 (escalation は streak ベース、planning review は必要度ベース) で、設計哲学が共通
- RV2-07: v1 review コード削除 — 独立して進められる
- WF-55 (TODO.md §L): metadata.json SSoT 一貫化 + view-freshness CI — 本 ADR の「view-freshness CI が整合性を保証」前提の根拠
