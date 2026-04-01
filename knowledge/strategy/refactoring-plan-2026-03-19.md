# リファクタリング計画 + 再発防止メカニズム

> **作成日**: 2026-03-19
> **出典**: `tmp/TODO.md` からのリファクタリング関連タスク抽出 + 再発防止の設計議論
> **設計参考**: [Domain Modeling Made Functional 読書メモ](https://syu-m-5151.hatenablog.com/entry/2026/01/22/094654) — domain の型でビジネス状態を表現し、不正な状態をコンパイル時に排除する
> **目的**: TODO.md のリファクタリング関連セクションを本レポートに集約し、TODO.md 側は ID + 一行サマリー + リンクに圧縮する
> **核心課題**: domain 層に `String` が残っている → その String を解釈するロジックが CLI/usecase/codec に散らばる → ロジック流出。型化が根本解決。

---

## 0. ドメインモデリング強化（全リファクタリングの前提）

### 0-1. 問題: domain の `String` フィールドがロジック流出の根本原因

イベントストーミング的な状態洗い出しをせずに見切り発車した結果、有限の状態が `String` のまま domain 層に残り、
その解釈ロジックが CLI・usecase・codec の複数箇所に散らばっている。

| domain の `String` | 散らばった解釈ロジック | 影響箇所数 |
|---|---|---|
| `ReviewRoundResult::verdict: String` | `"zero_findings"` 比較 | domain L664,790 + codec L403,406 = **4箇所** |
| `PrReviewResult::state: String` | `"APPROVED"` 等の比較 | CLI pr.rs L753,874,954 = **3箇所** |
| `PrReviewFinding::severity: String` | `"P0"/"P1"` 比較 | CLI pr.rs L951 = **1箇所** |
| `ReviewRoundResult::timestamp: String` | 時間比較不能 | GAP-01 で起票済み |

型にすれば解釈は domain に閉じ、流出しようがない。CLI/usecase は `match` で網羅的に処理するだけになる。

### 0-2. 既存 String フィールドの型化

**DM-01: `ReviewRoundResult::verdict` → `Verdict` enum (HIGH)**

```rust
// libs/domain/src/review.rs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    ZeroFindings,
    FindingsRemain,
}
```

- `ReviewPayloadVerdict`（usecase 層に既存）を domain に移動・統合 → GAP-03 を同時解決
- domain の `record_round` 内の `r.verdict == "zero_findings"` → `match r.verdict { Verdict::ZeroFindings => ... }`
- codec の `doc.verdict == "zero_findings"` → serde で自動 deserialize
- **影響**: 4箇所の文字列比較が消滅

**DM-02: `PrReviewResult::state` → `GhReviewState` enum (HIGH)**

```rust
// libs/domain/src/review.rs (or libs/domain/src/pr.rs)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GhReviewState {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
    Pending,
}

impl GhReviewState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Approved | Self::ChangesRequested | Self::Commented)
    }
    pub fn is_passed(&self, actionable_count: u32) -> bool {
        matches!(self, Self::Approved) || (actionable_count == 0 && !matches!(self, Self::ChangesRequested))
    }
}
```

- CLI pr.rs の `matches!(state, "APPROVED" | ...)` → `state.is_terminal()`
- `passed` フィールド廃止 → `state.is_passed(actionable_count)` に置換
- **影響**: 3箇所の文字列比較が消滅 + `passed: bool` の導出ロジックが domain に集約

**DM-03: `PrReviewFinding::severity` → `Severity` enum (MEDIUM)**

```rust
// libs/domain/src/review.rs (or libs/domain/src/pr.rs)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    P0,
    P1,
    Low,
    Info,
}

impl Severity {
    pub fn is_actionable(&self) -> bool {
        matches!(self, Self::P0 | Self::P1)
    }
}
```

- CLI pr.rs の `f.severity == "P0" || f.severity == "P1"` → `f.severity.is_actionable()`
- **影響**: 1箇所 + 将来の severity 追加時にコンパイラが全箇所を検査

**DM-04: `ReviewRoundResult::timestamp` → `chrono::DateTime<Utc>` (MEDIUM)**

- GAP-01 で既に起票済み
- `String` → `chrono::DateTime<Utc>` + `#[serde(with = "...")]`
- 時間ベースのドメインロジック（停滞検知等）が型安全に書けるようになる

### 0-3. 状態遷移図の明示化

現在コード内に暗黙的に存在する状態遷移を明示化する：

**Review ライフサイクル**（domain に既存だが不完全）:
```
NotStarted → InProgress → FastPassed → Approved
                ↑              ↓
            Invalidated ←── (code change or Fast findings_remain)
```

**PR ライフサイクル**（domain に不在 → GAP-02）:
```
Created → CIWaiting → ReviewReady → Mergeable → Merged
                          ↓
                    ChangesRequested → ReviewReady (re-review)
```

**Track ライフサイクル**（domain に既存、型で表現済み）:
```
Planning → Implementing → Done
              ↓
           Blocked / Cancelled (StatusOverride)
```

### 0-4. 今後の新機能に対する要求

Phase 2 以降で追加される状態（信号機 `ConfidenceSignal`、リカバリー `RecoveryAction` 等）は、
**最初から enum として domain に定義する**。`String` での仮実装は禁止。

### 0-5. 信号機システムとの統合ビジョン（Phase 1.5 → Phase 2 の接続）

Phase 1.5 で「過去の負債を型で修復」し、Phase 2 で「今後は信号機でドメインモデルの品質を保証する仕組み」を入れる。

#### spec.md の Domain States セクションに信号機を付与

```markdown
## Domain States
| Entity | States | Signal | Source |
|---|---|---|---|
| ReviewRound verdict | ZeroFindings, FindingsRemain | 🔵 | [source: Codex review API output-schema] |
| PR review state | Approved, ChangesRequested, Commented | 🟡 | [source: inference from gh CLI] |
| PR lifecycle | Created → CIWaiting → Mergeable → Merged | 🔴 | [source: inference, 未検証] |
| Severity | P0, P1, Low, Info | 🟡 | [source: existing classify_severity impl] |
```

- 各ドメイン状態の信頼度が可視化される
- 🔴 があれば `implementing` 遷移がブロックされる（SPEC-05）
- source 帰属（TSUMIKI-02）により「なぜその状態が必要か」が追跡可能

#### フィードバックループ: 実装失敗がドメインモデルの再検討に直結

```
spec.md Domain States に状態を列挙 + 信号機
    ↓
🔴 あり → implementing 遷移ブロック（SPEC-05）
    ↓ 全て 🟡以上
domain に enum として実装
    ↓
reviewer が enum 設計のバグを発見（例: 状態の漏れ、遷移の誤り）
    ↓ 3ラウンド同一 concern（WF-36 escalation）
該当 Domain State の信号機を 🟡→🔴 に自動降格（SPEC-01）
    ↓
implementing ブロック → 差分ヒアリング再起動（TSUMIKI-03）
    ↓ ユーザーがドメインモデルを修正
信号機 🔴→🟡→🔵 に昇格（CI 客観証拠による: SPEC-03）
    ↓
実装再開
```

**核心**: ドメインモデル自体の品質が信号機で管理される。「実装がうまくいかない」→「ドメインモデルが間違っているのでは？」という逆流が仕組みとして発生する。

#### Phase 対応

| Phase | 役割 | 状態 |
|---|---|---|
| **1.5** DM-01〜03 | 既存の String を型化（過去の負債修復） | これから |
| **1.5** 9-6 domain-strings CI | 新たな String 追加を CI で検出（再発防止） | これから |
| **2** TSUMIKI-01 + SPEC-05 | 信号機を metadata.json に型として導入 | 未着手 |
| **2** 2-12 spec-states | Domain States セクション必須化 + 信号機付き | 未着手 |
| **2** SPEC-01 | 実装失敗 → Domain State の信号機降格（逆流ループ） | 未着手 |
| **3** SPEC-03 | 🟡→🔵 昇格を CI 客観証拠に限定 | 未着手 |

Phase 1.5 の型化が Phase 2 の信号機導入の**前提条件**となる。
String のままでは信号機を付けても「信号機が 🔵 なのに実装で String 比較が散らばる」という矛盾が生じる。

### 0-6. capability の細分化 — 各段階に専門家エージェントを配置

ひとつの機能追加でも、ドメインモデリング・設計・実装・レビュー・受け入れなど段階が異なる。
現在の capability はこの段階を十分にカバーしておらず、特にドメインモデリングと受け入れに隙間がある。

#### 現状の隙間

```
spec 作成 ──→ 設計 ──→ 実装 ──→ レビュー ──→ 受け入れ
   ???      planner   implementer  reviewer     ???
              ↑
         domain modeling が
         planner に暗黙的に混在
```

#### 提案する capability 体系

| capability | 責務 | 既存との関係 |
|---|---|---|
| **domain_modeler** | イベントストーミング的に状態・遷移を洗い出し、enum/struct/fn にマッピング。Domain States テーブル + 状態遷移図 + 信号機を出力 | planner から分離。planner はタスク分解、modeler はドメイン構造 |
| **spec_reviewer** | ドメインモデルの**設計レビュー**。状態の漏れ・遷移の矛盾・境界条件の欠落を検査。信号機を評価（🔵🟡🔴） | reviewer から分離。reviewer はコード品質、spec_reviewer はモデル品質 |
| **planner** | タスク分解・依存関係整理・実装順序の決定。Domain States が 🟡以上であることが前提 | 既存を維持。domain_modeler の出力を入力として受け取る |
| **implementer** | TDD で実装。domain の enum を使い、CLI は薄く保つ | 既存を維持 |
| **code_reviewer** | コード品質・アーキテクチャ準拠・idiomatic Rust のレビュー | 既存 reviewer を改名 |
| **acceptance_reviewer** | spec の各要件が実装で満たされているか突き合わせ。要件網羅率の検証 | 新規。code_reviewer とは評価軸が異なる |
| **researcher** | 外部調査・crate 調査 | 既存を維持 |
| **debugger** | コンパイルエラー・テスト失敗の診断 | 既存を維持 |

#### 各段階のフロー

```
1. domain_modeler
   入力: ユーザー要件 / PRD / 既存コードベース
   出力: spec.md の Domain States + State Transitions + 信号機

2. spec_reviewer
   入力: spec.md の Domain States
   出力: 信号機の評価（🔵🟡🔴）+ 漏れ・矛盾の指摘
   ゲート: 🔴 が 0 になるまで 1↔2 をループ

3. planner
   入力: spec.md（信号機 🟡以上を確認済み）
   出力: plan.md のタスク分解 + 要件トレーサビリティ

4. implementer
   入力: plan.md のタスク + domain の enum 定義
   出力: コード（TDD: Red → Green → Refactor）

5. code_reviewer
   入力: 実装 diff
   出力: findings（品質・アーキテクチャ）
   ゲート: zero_findings まで 4↔5 をループ
   逆流: 3ラウンド同一 concern → 信号機降格 → 1 に戻る（SPEC-01）

6. acceptance_reviewer
   入力: spec.md 要件 + 実装 + テスト
   出力: 要件網羅率 + 未達要件リスト
   ゲート: 網羅率閾値を満たすまで 4↔6 をループ
```

#### provider の分離が本質的に必要な箇所

全 capability を異なる provider にする必要はない。分離が**本質的に必要**なのは以下の 3 境界：

| 境界 | 理由 |
|---|---|
| **domain_modeler / spec_reviewer** と **implementer** | モデリングと実装は異なるスキルセット。モデラーはコードを書く必要がない |
| **implementer** と **code_reviewer** | self-review バイアス回避（CLAUDE-BP-02）。別セッション必須 |
| **code_reviewer** と **acceptance_reviewer** | 評価軸が異なる。「コードが良いか」vs「要件を満たしているか」 |

`domain_modeler` と `spec_reviewer` は同じ provider で context を共有して良い。
`planner` と `domain_modeler` も同じ provider で Phase を分けるだけで機能する。

#### 導入タイミング

- **Phase 1.5**: 既存 capability のまま DM-01〜03 を実施（手動で domain modeling を意識する段階）
- **Phase 2**: `domain_modeler` と `spec_reviewer` を `agent-profiles.json` に追加。`/track:plan` のフローを 1→2→3 の 3 段階に拡張
- **Phase 3**: `acceptance_reviewer` を追加。`sotp verify spec-states` と連動
- **Phase 5**: 全 capability が `/track:auto` の自律実行フローに統合される

### 0-7. TDD 状態マシンによる implementer の強制

現在の TDD は「ルール文書で推奨 + reviewer がチェック」のみ。implementer が実装先行→後からテストのパターンに陥ることを仕組みで防止できない。

#### 状態遷移

```
         ┌─────────────────────────────────┐
         │                                 │
    ┌────▼────┐    ┌─────────┐    ┌────────┴──┐
    │   Red   │───▶│  Green  │───▶│ Refactor  │
    │(テスト作成)│    │(実装)    │    │(整理)      │
    └─────────┘    └─────────┘    └───────────┘
     CI fail 必須   CI pass 必須    CI pass 維持
```

#### domain 型

```rust
// libs/domain/src/tdd.rs
pub enum TddPhase {
    Red,
    Green,
    Refactor,
}

pub enum TddTransitionError {
    CiMustFailInRed,       // Red なのに CI pass → テストが甘い
    CiMustPassInGreen,     // Green なのに CI fail → 実装不足
    CiMustPassInRefactor,  // Refactor で CI fail → リグレッション
    SkippedRed,            // Red を経ずに Green に遷移
}
```

#### metadata.json 拡張

```json
{
  "tasks": [
    {
      "id": "T001",
      "description": "...",
      "status": "in_progress",
      "tdd_required": true,
      "tdd_phase": "red"
    }
  ]
}
```

#### 強制メカニズム

| 仕組み | 動作 |
|---|---|
| `sotp tdd advance <task-id> --evidence <ci-fail\|ci-pass>` | CI の exit code を検証し、遷移条件を満たしていれば `tdd_phase` を進める |
| `/track:implement` | implementer に「`sotp tdd advance` を呼ばないと次に進めない」と指示 |
| `/track:commit` guard | `tdd_required: true` なタスクの `tdd_phase` が完了でなければ commit 拒否 |
| `tdd_required` の自動設定 | `/track:plan` の planner が Domain States に対応するタスクに `tdd_required: true` を自動付与 |

#### 適用対象

| 対象 | `tdd_required` |
|---|---|
| domain 型の追加・変更 | `true` |
| usecase ロジックの実装 | `true` |
| バグ修正（regression test first） | `true` |
| ドキュメント・設定変更 | `false` |
| convention 追加 | `false` |

#### 信号機 / escalation との連動

- Red phase で 3 ラウンド連続 CI fail → WF-36 escalation が発動（同一 concern: テスト設計の問題）
- escalation → 該当 Domain State の信号機降格（SPEC-01）→ ドメインモデル再検討

#### Phase 配置

- **Phase 1.5**: 手動 TDD（implementer への指示 + reviewer チェック）
- **Phase 3** (HARNESS-03 と同時): `sotp tdd advance` サブコマンド + metadata.json `tdd_phase` フィールド + commit guard

---

## 1. CLI 層の肥大化・ロジック流出 (CLI)

### CLI-01: `pr.rs` の review polling/parsing ロジックを usecase 層に移動 (HIGH)

- **現状**: `apps/cli/src/commands/pr.rs` (1432行) に review cycle のコアロジック（GitHub API ポーリング、reaction/comment 判定、bot 識別、review パース・findings 構築）が直接実装されており、usecase 層に委譲されていない。
- **問題**:
  - `poll_review_for_cycle` (L637-779): ポーリングループ、reaction/comment 判定、bot 検出 → usecase 層の責務
  - `check_reaction_zero_findings` / `check_comment_zero_findings` (L404-474): zero-findings 判定 → usecase 層 (`pr_review`) に統合すべき
  - `is_codex_bot` / `CODEX_BOT_LOGINS` (L353-359): bot 識別ポリシー → domain or usecase 層の定数/型
  - `parse_review` (L782-851): review JSON パース → usecase 層 (`pr_review`) に既に一部ある
  - `ensure_pr_for_cycle` (L579-634): `ensure_pr` のほぼ重複コード
- **アクション**:
  1. `is_codex_bot` / `CODEX_BOT_LOGINS` を `usecase::pr_review` に移動
  2. `check_reaction_zero_findings` / `check_comment_zero_findings` を `usecase::pr_review` に移動
  3. `poll_review_for_cycle` のコアロジックを `usecase::pr_review` に移動（CLI 層は薄いラッパーに）
  4. `parse_review` の findings 構築部分を `usecase::pr_review` に移動
  5. `ensure_pr` と `ensure_pr_for_cycle` の重複を解消
- **目標**: `pr.rs` を ~500行に削減
- **関連**: `libs/usecase/src/pr_review.rs`, `libs/usecase/src/pr_workflow.rs`

### CLI-02: `review.rs` の record-round / check-approved を usecase 層に移動 (HIGH)

- **現状**: `apps/cli/src/commands/review.rs` (1696行) に domain/infrastructure を直接操作する usecase レベルのオーケストレーションが CLI 層に実装されている。
- **問題**:
  - `run_record_round` (L714-851): domain::ReviewState / infrastructure::FsTrackStore を直接操作 → usecase 層のオーケストレーション
  - `run_check_approved` (L885-958): 同上（TOCTOU 対策の lock 付き read-modify-write）
  - `extract_verdict_from_session_log` (L566-615): session log からの verdict JSON 抽出 → usecase 層 (`review_workflow`)
  - `resolve_full_auto_from_profiles` (L392-418): agent-profiles.json の読み込み → infrastructure + usecase 層
- **アクション**:
  1. `extract_verdict_from_session_log` を `usecase::review_workflow` に移動
  2. `run_record_round` のコアロジックを `usecase::review_workflow` に UseCase として抽出
  3. `run_check_approved` のコアロジックを `usecase::review_workflow` に UseCase として抽出
  4. `resolve_full_auto_from_profiles` の profiles 読み込みを infrastructure 層に分離
  5. CLI 層にはプロセス管理（spawn, tee, terminate）+ 薄い UseCase 呼び出しだけ残す
  6. ReviewGroupState API 整理: 未使用 alias `from_legacy_final_only` の削除（done-hash-backfill Phase D 残件）
- **目標**: `review.rs` を ~700行に削減
- **関連**: `libs/usecase/src/review_workflow.rs`, `libs/domain/src/review.rs`

### CLI-03: コーディング原則適合チェック (LOW)

- **現状**: `.claude/rules/04-coding-principles.md` のモジュールサイズガイドライン（200-400行、最大700行）を超過するファイルが commands/ に 2 つ存在。
- **アクション**: CLI-01, CLI-02 完了後に全 commands/ ファイルがガイドライン内に収まることを確認

---

## 2. レビューサイクル品質改善 (RVW)

> **出典**: phase1-sotp-hardening-2026-03-17 レビューサイクル (infra-domain 8R, usecase 11R) で繰り返し発生したバグパターン分析
> **追加日**: 2026-03-17

### RVW-01: 共通 Frontmatter パーサー抽出 (HIGH)

- **現状**: `spec_attribution` と `spec_frontmatter` がそれぞれ独自に `---` delimiter 判定を実装。同じバグ（`trim_end()` vs exact match）が3回繰り返し発見された。
- **アクション**: `libs/infrastructure/src/verify/frontmatter.rs` に共通 `parse_yaml_frontmatter(content) -> Option<(usize, &str)>` を抽出。`spec_attribution` と `spec_frontmatter` から呼び出す。
- **関連**: T007, T008

### RVW-02: conch-parser AST 直接走査による hand-rolled shell 解析の廃止 (HIGH)

- **現状**: `extract_shell_reentry_arg`、`raw_mentions_rm`、`argv_has_rm` が POSIX shell option parsing を部分的に再実装。`-c` payload 抽出、combined flag 解析 (`-lc`, `-ec`, `-ce`)、launcher skip をすべて手書き。6ラウンドにわたりエッジケースが発見された。
- **アクション**: conch-parser の AST を直接走査して「コマンド内の全 `SimpleCommand` を再帰的に列挙する」関数を `domain::guard::parser` に追加。`-c` payload の手動抽出を不要にする。`extract_shell_reentry_arg` を廃止。
- **関連**: T005, `project-docs/conventions/shell-parsing.md`

### RVW-03: typed deserialization convention + canonical_modules serde 移行 (MEDIUM) — **IN PROGRESS: review-quality-quick-wins-2026-03-17 T002**

- **現状**: `canonical_modules.rs` が `serde_json::Value` の手動走査で JSON をパース。`filter_map` による silent drop は serde typed deserialization で根本解決済み。
- **アクション**: (T002 で対応中)
  1. `canonical_modules.rs` の `parse_canonical_rules` を `#[derive(Deserialize)]` 型に置き換え
  2. `project-docs/conventions/typed-deserialization.md` を作成（`serde_json::Value` 手動走査の禁止、typed deserialization 推奨）
- **関連**: canonical_modules, review-quality-quick-wins-2026-03-17

### RVW-04: `syn` crate による `is_inside_test_module` 置き換え + standalone テストファイル除外 (LOW)

- **現状**: 行ベースの後方スキャン + brace depth カウントで `#[cfg(test)]` スコープを判定。3ラウンドにわたりバグが発見された（初期実装 → brace depth → `any()` bug）。また `libs/*/tests/*.rs` の standalone テストファイルが除外されず、テストヘルパーが誤検知される（現時点では該当ファイル不在のため理論的）。
- **アクション**: `syn` crate で Rust ファイルを AST パースし、`#[cfg(test)]` attribute が付いた `mod` 内の行かどうかを正確に判定。パース失敗時は fail-closed (finding 報告)。standalone テストファイル (`tests/` ディレクトリ) も除外対象に追加。
- **関連**: canonical_modules verify

### RVW-05: `skip_command_launchers` の per-launcher フラグモデリング (LOW)

- **現状**: `sudo -n`/`sudo -i` 等の no-arg フラグが「次トークンを引数として消費する」と誤解釈される。`sudo -n cp bin/sotp` で `cp` がスキップされ `bin/sotp` ガードが発動しない。同様の問題が他の launcher でも理論的に存在。
- **アクション**: RVW-02 (conch-parser AST 直接走査) で根本解決。argv-level ヒューリスティクスの改善は費用対効果が低い。
- **関連**: `skip_command_launchers` accepted deviation, RVW-02

### RVW-06: metadata.json にレビュー状態統合 + エスカレーション順序強制 + コミットガード (HIGH)

- **現状**: 3つのプロセス違反が仕組みで防止できない:
  1. fast model zero_findings 後の full model 最終確認をスキップしてコミットに進む
  2. fast model と full model を同時に実行する（fast が findings を返したら full は無駄）
  3. full model の結果（zero_findings or accepted deviation のみ）を人間が確認せずにコミットに進む
- **設計方針**: レビュー状態は track の SSoT である `metadata.json` に統合。別ファイル（`review-state.json`）は作らない。
- **アクション**:
  1. `metadata.json` schema_version 4 に `review` セクション追加:
     - `review.status`: `null` → `in_progress` → `fast_passed` → `approved` の状態遷移
     - `review.code_hash`: git tree hash でコード変更後の stale verdict を検出
     - `review.groups.{name}.fast/full`: ラウンド番号 + verdict + timestamp
  2. `track-local-review` wrapper が `--round-type final` 実行時に `metadata.json` の `review.status` が `fast_passed` でなければ**実行拒否**（sequential escalation の強制）
  3. `track-local-review` wrapper がラウンド結果を `metadata.json` に自動書き込み
  4. `track-commit-message` wrapper に「`review.status == approved` でなければ commit 拒否」ガードを追加
  5. コード変更（`git diff` で検出）があれば `review.status` を自動リセット（stale verdict 防止）
  6. `sotp track views validate` で review セクションの schema validation を追加
- **関連**: RVW-01〜05, `.claude/skills/track-review/`, metadata.json schema

### RVW-07: Codex verdict 抽出の stderr フォールバック + セッションログ保存 (HIGH)

- **現状**: sotp の `review codex-local` は `codex-last-message` ファイルからのみ verdict を読む。Codex がサブエージェント spawn 後に異常終了すると、ファイルは空だが stderr の最終行に verdict JSON が含まれている。結果として exit 105 (ProcessFailed) となり、エージェントが「Codex が動かない」と誤判断する。
- **根本原因**: Codex CLI の `--output-last-message` はセッション正常終了時のみファイル書き込み。サブエージェント起因の異常終了では空ファイルのまま。
- **アクション**:
  1. sotp wrapper で Codex の stderr を `tmp/reviewer-runtime/codex-session-{pid}.log` に自動保存（トレーサビリティ確保）
  2. `codex-last-message` が空の場合、stderr ログの末尾から `codex` ラベル付き verdict JSON を抽出するフォールバックを追加
  3. stdout にも verdict を出力する現行動作は維持（デバッグ用）
  4. ReviewRunResult に `stderr_log_path` フィールドを追加し、失敗時の原因調査を容易に
- **関連**: RVW-06, `apps/cli/src/commands/review.rs`

---

## 3. Shell パース・Guard 強化 (SEC/WF)

### SEC-14: shell `-c` payload の再帰パース不足 (LOW-MEDIUM)

- **課題**: `policy.rs` の file-write guard は外側の `SimpleCommand` のみ検査。`bash -c 'echo > file'` や `sh -c 'tee out'` のように `-c` 引数に埋め込まれたファイル書き込みを検知できない。git guard も同じ制約を持つが、substring match で実用上は機能している
- **対応方針**: 既知シェル（bash/sh/dash/zsh/ash）の `-c` 引数を検出し、その payload 文字列に対して既存の `split_shell()` を再帰呼び出しする。新規クレート不要 — conch-parser で対応可能。実装は `parser.rs` の `collect_from_conch_simple` で argv から `-c` payload を抽出 → `split_shell_inner(payload, depth+1)` で再帰パース
- **残留リスク文書**: `project-docs/conventions/bash-write-guard.md`
- **出典**: bash-write-guard-2026-03-18 reviewer findings (gpt-5.4 R5/R7)

### WF-37: `argv_has_rm` / `extract_rm_args_from_argv` のランチャー後走査が過剰 (MEDIUM)

- **課題**: `libs/usecase/src/hook.rs` の `argv_has_rm` がシェルランチャー（`time`, `env` 等）の後、`rm` でないトークンをスキップして走査を継続するため、`time echo rm tests/foo.rs` のようなコマンドが誤って削除コマンドと判定される（false positive）
- **提案**: ランチャーが見つかったら次の 1 トークンだけをサブコマンドとして判定し、それが `rm` でなければそこで走査を終了する。既存の `COMMAND_LAUNCHERS` / `LAUNCHER_POSITIONAL_ARGS` パターン（`guard/policy.rs`）と同様のロジックを適用
- **出典**: spec-template-foundation-2026-03-18 reviewer finding (gpt-5.3-codex-spark, pre-existing code)

### WF-38: frontmatter パーサーの duplicate key 未検出 (LOW)

- **課題**: `spec_frontmatter.rs` の `lines().find()` が最初の一致行のみ検査するため、有効な `signals:` の後に malformed な重複 `signals:` があってもパスする。`status`/`version` も同じパターン
- **提案**: `frontmatter.rs`（共有パーサー）で duplicate key を検出し warning/error を返す
- **出典**: spec-template-foundation-2026-03-18 reviewer finding (gpt-5.4, pre-existing parser design)

---

## 4. ワークフロー改善 (WF)

### WF-36: Review Escalation Threshold の機構化 (HIGH)

- **課題**: `10-guardrails.md` に「同じカテゴリのバグ修正が3回連続したら escalation」ルールが定義されているが、ドキュメント上のプロンプト指示にとどまり、機構として enforce されていない。実際に review-infra-hardening トラックで reviewer が同質の edge case を繰り返し指摘し、人間が介入して抽象化を促す必要があった。
- **提案**: `sotp review record-round` に finding カテゴリ分類とラウンドカウンターを組み込み、同一カテゴリの findings が 3 ラウンド連続した場合に自動 escalation（Workspace Search → Reinvention Check → Decision）をトリガーする仕組みを構築。`ReviewState` インフラ（review-infra-hardening で追加済み）の上に構築可能。
- **目的**: 「プロンプトではなく機構として」抽象的思考への誘導を実現し、開発者の介入なしに車輪の再発明を自動検知する

### WF-39: `/track:catchup` の責務分割 — bootstrap と briefing の分離 (MEDIUM)

- **課題**: `/track:catchup` が「初回環境構築（bootstrap + setup）」と「プロジェクト状態の把握（briefing）」を1コマンドに混合。名前が「追いつく＝今の状態を知る」と読めるため、毎回の作業開始時に使うコマンドと誤解されやすい。実際には bootstrap を含むため初回または環境破損時のみが適切な使用タイミング
- **提案**:
  - `/track:catchup` を初回セットアップ専用に限定（Phase 1 bootstrap + Phase 2 setup + Phase 4 external guides）。名前を `/track:bootstrap` に改名
  - 新コマンド `/track:briefing` を作成。毎回の作業開始時に使う「プロジェクト状態の把握」コマンド:
    1. `track/registry.md` からアクティブ/完了トラック一覧
    2. 現在のトラックの `spec.md` / `plan.md` / `metadata.json` サマリー
    3. `track/tech-stack.md` の未解決 `TODO:` ハイライト
    4. 直近の git log（10件）
    5. `project-docs/conventions/README.md` のアクティブ規約一覧
    6. 推奨 next action
  - `DEVELOPER_AI_WORKFLOW.md` のフローチャート・コマンド表を更新
  - `README.md` のクイックスタートを更新
- **実装手順**:
  1. `.claude/commands/track/briefing.md` を新規作成（現 catchup.md の Phase 3 を移植）
  2. `.claude/commands/track/catchup.md` から Phase 3 を除去し、description を `"First-time environment bootstrap and track workflow setup"` に変更。ファイル名を `bootstrap.md` に改名
  3. `DEVELOPER_AI_WORKFLOW.md` のフローチャート・コマンド表・手順セクションを更新
  4. `README.md` のクイックスタートを更新（`/track:catchup` → `/track:bootstrap`、作業開始時は `/track:briefing`）
  5. skill 定義の description を更新（Skill tool のトリガー説明）
- **関連**: SURVEY-05（Session Startup Ritual）, HARNESS-08（ブートストラップルーチン標準化）

### WF-40: `ReviewState::record_round` が Approved 状態から Fast findings_remain で降格しない (MEDIUM)

- **課題**: `libs/domain/src/review.rs:226` — Approved 状態で Fast ラウンドの `findings_remain` を受けても降格しない。承認済みトラックが劣化したレビューデータのまま `check_commit_ready` を通過できる
- **提案**: Approved 状態で Fast findings_remain を受けた場合に `Invalidated` へ降格する
- **出典**: spec-template-foundation-2026-03-18 reviewer finding (gpt-5.4, pre-existing code)

### WF-41: `review_from_document` が偽の Fast ラウンドを合成する (LOW)

- **課題**: `libs/infrastructure/src/track/codec.rs:323` — グループに final ラウンドしかない場合に偽の fast ラウンドを合成し、JSON ラウンドトリップで fidelity が壊れる
- **提案**: fast ラウンドが存在しない場合は `None` として表現する
- **出典**: spec-template-foundation-2026-03-18 reviewer finding (gpt-5.4, pre-existing code)

---

## 5. SSoT・データ整合性 (SSoT/STRAT/GAP)

### SSoT-07: SSoT と AI プロンプトの矛盾 — Split-brain (§C-464) (MEDIUM)

- **課題**: Planner に plan.md と metadata.json の二重書き込みを指示
- **提案**: metadata.json のみ作成、plan.md はシステム側で自動生成

### STRAT-04: `track/registry.md` を Git 管理対象から外し、完全生成ビューへ移行 (MEDIUM)

- **背景**: `registry.md` は `metadata.json` から決定論的に再生成できる一方、git 管理していることでブランチ差分・stale view・コミット前 sync 漏れの問題を生む。
- **方針**: `registry.md` を commit 対象から外し、必要時に `sync-views` で生成する read model に下げる。

### GAP-03: `ReviewVerdict` / `ReviewPayloadVerdict` の重複 (LOW)

- **課題**: `review_workflow.rs` に「実行結果としての verdict」と「ペイロード内容としての verdict」が別 enum。実質同じ概念の二重定義
- **提案**: 統合するか、明示的な変換層を設けて責務を明確化

### GAP-04: `StatusOverride` の表現力不足 (MEDIUM)

- **課題**: `Blocked` / `Cancelled` の2つのみ。multi-agent オーケストレーションで必要な `AwaitingHuman`, `ReworkingPlan`, `Recovering` 等を型で表現できない
- **提案**: SPEC-06（3層リカバリー）の実装に合わせて variant を追加
- **関連**: SPEC-06（リカバリータクソノミー）

---

## 6. コード品質・責務分離 (MEMO)

### MEMO-02: `.claude/docs/DESIGN.md` の肥大化 (MEDIUM)

- **提案**: トラック別の設計文書に分割、DESIGN.md はインデックスとサマリーに限定

### MEMO-03: CLI 層から domain 層への直接参照を禁止 (MEDIUM)

- **提案**: `docs/architecture-rules.json` と `check_layers.py` / `deny.toml` にルール追加

### MEMO-04: `validator`, `strum`, `derive_more` による簡約 (LOW)

- **提案**: 計画着手時に対象箇所を調査し、適用可能な crate を導入

### MEMO-05: `.claude/rules/` の完全英語化 (LOW)

- **提案**: `01-language.md` のポリシーに従い、rules を英語に統一

### MEMO-06: 肥大化リスクファイルの分割 (LOW)

- **対象候補**: `error.rs`, `codec.rs`, `fs_store.rs`, `track.rs` (domain)
- **関連**: ERR-09b（`activate.rs` 分割）は既に起票済み
- **注**: 計画着手時に調査対応

---

## 7. CLI エラーハンドリング改善 (ERR)

### ERR-09b: `activate.rs` のモジュール分割 (MEDIUM)

- **背景**: `apps/cli/src/commands/track/activate.rs` が 1000行超で単一ファイルとして長すぎる
- **提案**: `activate.rs` → メイン、`branch.rs` → git コマンド、`preflight.rs` → バリデーション、`resume.rs` → resume marker
- **制約**: 別トラックとして切り出すのが適切

### ERR-11: `tracing` クレートの導入検討 (LOW)

- **背景**: 現状 `eprintln!` で直接エラー出力しており、構造化ログ・出力先切替・span 追跡ができない。
- **前提**: `track/tech-stack.md` への tracing 追加が必要

### ERR-13: コンテナ内 `-local` タスクの `cargo run --quiet -p cli` 置換 (LOW)

- **背景**: ホスト側 wrapper は `bin/sotp` に移行済みだが、コンテナ内の 3 タスクは引き続き `cargo run` を使用

### ERR-14: `_agent_profiles.py` に `fast_model` フィールドのバリデーション追加 (LOW)

### ERR-15: `bin/sotp` の staleness 検出 (MEDIUM)

- **背景**: ソース変更後に `cargo make build-sotp` を再実行しないと `bin/sotp` が古いまま動作する

### ERR-16: `test_make_wrappers.py` のテストカバレッジ拡大 (LOW)

- **背景**: `bin/sotp` 移行後も多くの wrapper テストが未追加

---

## 8. 設計レベルのリファクタリング

### CLAUDE-BP-02: Writer/Reviewer 分離パターンの `/track:review` 適用 (MEDIUM)

- **現状**: `/track:review` は Codex CLI に reviewer を委譲しているが、Claude Code 内での self-review（`claude-heavy` profile 時）は同一セッションで実施される可能性がある。同一セッション内レビューは自身が書いたコードへのバイアスがかかる。
- **公式の推奨**: Writer と Reviewer を別セッションで分離。「A fresh context improves code review since Claude won't be biased toward code it just wrote.」
- **アクション**: `claude-heavy` profile の reviewer を別サブエージェント（独立コンテキスト）として実行するよう `/track:review` スキルを改修。`subagent_type` で隔離されたレビューエージェントを起動。
- **スコープ**: `claude-heavy` profile 限定（Codex 委譲の既定 profile は影響なし）
- **関連**: 既存 TODO「/track:review の reviewer provider 移譲を強制する仕組み」

---

## 9. 再発防止メカニズム（プロンプト非依存）

### 9-1. CI ゲート: ファイル肥大化の自動検知 ✅ done (PR #46)

**実装先**: `sotp verify module-size` サブコマンド（Rust） — 実装済み。warning-only（既存超過ファイルが多数のため）。exclude に `.cache/` も追加。

**設定**: `docs/architecture-rules.json` に追加

```json
{
  "module_limits": {
    "max_lines": 700,
    "warn_lines": 400,
    "exclude": ["vendor/"]
  }
}
```

**動作**:
- `cargo make ci` で全 `.rs` ファイルの行数を検査
- `warn_lines` 超過 → 警告出力
- `max_lines` 超過 → CI fail（分割を強制）

**防止する問題**: CLI-01, CLI-02, ERR-09b, MEMO-06 の再発

### 9-2. clippy 属性による関数レベルの肥大化防止 ✅ done (PR #46)

**実装先**: CLI crate の `main.rs` — 実装済み。既存4関数に `#[allow]` 付与。

```rust
// apps/cli/src/lib.rs (or main.rs)
#![warn(clippy::too_many_lines)]
```

**効果**: 関数単位で肥大化を検知。`cargo make clippy` で自動検出。

**防止する問題**: 個別関数への過度なロジック集中

### 9-3. 層の責務分離（既存 trait ベース DI を維持）

#### 現状の強み

このプロジェクトは**既に正しい構造を持っている**:

- `libs/domain/` に `TrackReader`/`TrackWriter` trait が定義済み
- `libs/usecase/Cargo.toml` は `domain` のみに依存（infrastructure への依存ゼロ）
- `libs/infrastructure/` が trait の具象実装を提供
- `apps/cli/` が DI の組み立てを行う

**問題はアーキテクチャではなく、ロジックの配置**。CLI に書かれるべきでないコードが CLI にある。

#### 設計原則

| 層 | 責務 | I/O | 依存 |
|---|---|---|---|
| **domain** | 型定義 + 純粋関数 + trait（port）定義 | なし | なし |
| **usecase** | オーケストレーション。domain の trait を引数で受け取り、domain 関数を呼ぶ | trait 引数経由のみ | domain のみ |
| **infrastructure** | trait の具象実装（FsTrackStore 等） | あり | domain のみ |
| **CLI (main)** | DI 組み立て + usecase 呼び出し + 出力 | あり | 全層 |

#### 具体パターン

**usecase: 既存の trait を引数で受け取る**
```rust
// libs/usecase/src/review_workflow.rs
pub fn record_round(
    reader: &impl TrackReader,
    writer: &impl TrackWriter,
    input: RecordRoundInput,
) -> Result<RecordRoundOutput, UseCaseError> {
    let doc = reader.load_document(&input.track_id)?;
    let new_state = domain::review::apply_round(doc.review_state(), &input.round)?;
    let new_doc = doc.with_review_state(new_state);
    writer.save_document(&new_doc)?;
    Ok(RecordRoundOutput::from(&new_doc))
}
```

**CLI: DI 組み立て + 呼び出し + 出力**
```rust
// apps/cli/src/commands/review.rs
fn run_record_round(args: &RecordRoundArgs) -> Result<()> {
    let store = FsTrackStore::new(&args.track_dir);
    let result = usecase::review_workflow::record_round(
        &store, &store, RecordRoundInput::from(args),
    )?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
```

#### CLI に残して良いもの

- `clap` 引数パース
- プロセス管理（`Command::spawn`, tee, terminate）
- 出力フォーマット（`println!`, JSON serialize）
- infrastructure の具象型の組み立て（DI）

#### CLI に残してはいけないもの

- domain 型の直接構築（`ReviewState::new()` 等）
- ビジネスロジック・判定（zero-findings 判定、bot 識別等）
- 状態遷移（`record_round`, `check_approved` のコアロジック）
- I/O と判定の混合（ポーリングループ内での判定等）

### 9-4. Rust 可視性による層境界の強制

**実装先**: 各 crate のモジュール構造

```rust
// domain 層: 構築関数を pub(crate) にし、外部は usecase 経由でのみ状態遷移
pub struct ReviewState { ... }
impl ReviewState {
    pub(crate) fn new(...) -> Self { ... }  // domain 内でのみ構築可能
    pub fn status(&self) -> ReviewStatus { ... }  // 読み取りは公開
}
```

**効果**: CLI 層が domain 型を直接構築できない → 必ず usecase 経由

**防止する問題**: 層境界違反、型の二重定義（GAP-03）

### 9-5. convention ドキュメント化

**実装先**: `project-docs/conventions/layered-architecture.md`

以下を明文化し、reviewer のチェックリスト入力にする：
- usecase は domain の trait（`TrackReader`/`TrackWriter` 等）を引数で受け取る
- domain は I/O を一切持たない（型定義 + 純粋関数 + trait 定義のみ）
- DI の組み立ては CLI (main) で行う
- CLI にビジネスロジックを書かない

### 9-6. domain pub String 検出 CI ✅ done (PR #46)

**実装先**: `sotp verify domain-strings` サブコマンド（Rust） — 実装済み。行マッチング方式（syn ではなく）。warning-only（DM-01/02/03 完了後に error に切替）。fn/struct/enum/trait 等の非フィールド行を除外。

**動作**:
- `libs/domain/src/` 内の `pub` な struct フィールドで型が `String` のものを検出
- newtype（`pub struct TrackId(String)` 等）は除外（内部フィールドが `pub` でなければ OK）
- 検出時は ERROR + 「有限状態なら enum に、自由テキストなら newtype に」の HINT を出力

```
sotp verify domain-strings
→ ERROR: libs/domain/src/review.rs:421 — ReviewRoundResult::verdict is pub String
→ HINT: If this represents finite states, use an enum. If free text, wrap in a newtype.
```

**防止する問題**: domain 層への `String` フィールド追加 → ロジック流出の再発

### 9-7. spec.md に Domain States セクションを必須化

**実装先**: `sotp verify spec-states` + `/track:plan` スキルのプロンプト

**spec.md 必須セクション**:
```markdown
## Domain States
| Entity | States | Type |
|---|---|---|
| ReviewRound verdict | ZeroFindings, FindingsRemain | enum Verdict |
| PR review state | Approved, ChangesRequested, Commented, ... | enum GhReviewState |

## State Transitions
ReviewStatus: NotStarted → InProgress → FastPassed → Approved
```

**検証**:
- `sotp verify spec-states`: spec.md の Domain States に列挙された全状態が `libs/domain/` に enum として存在するか検査
- 不在なら ERROR（「spec に書いた状態が domain に型として存在しない」）

**効果**: イベントストーミング → 型マッピングのプロセスがワークフローに組み込まれ、`String` での仮実装を事前に防ぐ

### 9-8. 依存方向の拡張チェック（将来）

**実装先**: `sotp verify` 拡張

- CLI crate 内で `domain::` 型を `impl` していないか検査
- CLI crate 内で `struct` / `enum` 定義が増えていないか検査
- usecase crate 内で `std::fs`, `std::process`, `std::net` 等の I/O を直接使用していないか検査
- `syn` crate による AST 解析（ROI 判断後に導入）

**防止する問題**: CLI 内への型・ロジックの新規追加、usecase への I/O 混入

---

## 10. 導入順序（推奨）

```
Phase 0: ドメインモデリング強化（ロジック流出の根本原因を断つ）
├── DM-01: ReviewRoundResult::verdict → Verdict enum（+ GAP-03 統合）
├── DM-02: PrReviewResult::state → GhReviewState enum
├── DM-03: PrReviewFinding::severity → Severity enum
├── DM-04: timestamp → chrono::DateTime<Utc>（GAP-01）
└── 状態遷移図の明示化（Review / PR ライフサイクル）

Phase 1: 検知網の敷設 + 設計方針の明文化
├── project-docs/conventions/layered-architecture.md 作成
├── architecture-rules.json に module_limits 追加
├── sotp verify module-size + domain-strings 実装
├── clippy::too_many_lines 属性追加
└── cargo make ci に組み込み

Phase 2: パイロット移行（1 コマンドでパターンを実証）
├── CLI-02 の record_round を先行移行（最も自己完結的）
│   ├── domain: Verdict enum は Phase 0 で完了済み。純粋関数を確認
│   ├── usecase: record_round(&impl TrackReader, &impl TrackWriter, input) を新設
│   ├── CLI: DI 組み立て（FsTrackStore）+ usecase 呼び出しに書き換え
│   └── テスト: usecase テストで mockall の MockTrackReader/Writer を注入
└── パターンが確立したら Phase 3 に進む

Phase 3: 本格リファクタリング（パイロットのパターンを横展開）
├── CLI-02 残り: check_approved, extract_verdict → usecase に移動
├── CLI-01: pr.rs のポーリング・判定ロジック → usecase::pr_review に移動
├── ERR-09b: activate.rs のモジュール分割
├── RVW-01: frontmatter パーサー抽出
├── RVW-02: shell parsing を conch-parser に統合
└── GAP-03: verdict enum の統合

Phase 4: 構造的ロック（再発防止を CI で強制）
├── pub(crate) 可視性の適用（domain 型の外部構築を禁止）
├── usecase の I/O 直接使用を CI で検査
├── 層境界違反の CI チェック追加
├── MEMO-06 対象ファイルの分割
└── MEMO-03: architecture-rules.json にルール追加

Phase 5: 設計レベル改善
├── RVW-06: レビュー状態の metadata.json 統合
├── RVW-07: Codex verdict stderr フォールバック
├── SSoT-07: plan.md 生成の完全自動化
├── STRAT-04: registry.md の Git 管理除外
├── WF-36: Review Escalation Threshold の機構化
└── CLAUDE-BP-02: Writer/Reviewer 分離
```

### トラック分割計画（`/track:plan` 用リファレンス）

Phase 0〜4 を 7 トラックに分割する。`/track:plan` 時にこのセクションを参照すること。

| Track | ID 案 | 対応 Phase | 含む項目 | 規模 | 依存 |
|---|---|---|---|---|---|
| **A0** | `remove-file-lock-system` | Phase 0 | ファイルロック一式削除 (~2,100行): domain::lock, infra::lock, usecase hook handlers, CLI lock/hook, settings.json entries | M | なし（最初に着手。AI 探索ノイズ除去） |
| **A** | `domain-type-hardening` | Phase 0 | DM-01 (Verdict enum + GAP-03 統合), DM-02 (GhReviewState), DM-03 (Severity) | M | A0 完了後 |
| **B** | `ci-guardrails-phase15` | Phase 1 | module_limits, domain-strings CI, clippy::too_many_lines, layered-architecture.md | S | なし（A と並行可） |
| **C** | `review-usecase-extraction` | Phase 2-3 | CLI-02 パイロット (record_round) → CLI-02 完了 (check_approved, extract_verdict, resolve_full_auto) | L | A（Verdict enum が必要） |
| **D** | `pr-usecase-extraction` | Phase 3 | CLI-01: poll_review, check_zero_findings, parse_review, ensure_pr 重複解消 | L | A（GhReviewState, Severity が必要） |
| **E** | `activate-module-split` | Phase 3 | ERR-09b: activate.rs → branch/preflight/resume の 4 ファイル分割 | M | なし（独立） |
| **F** | `parser-consolidation` | Phase 3 | RVW-01 (frontmatter 共通化) + RVW-02 (conch-parser AST 走査) | M | なし（独立） |
| **G** | `structural-lockdown` | Phase 4 | pub(crate) 可視性適用, spec-states CI, domain-strings CI 強化 | S | C, D 完了後 |

**クリティカルパス**: A → C → D → G（4 トラック直列）

**依存関係上は独立**: B, E, F は A〜G と依存なし。任意の順序で着手可能。

**注意: 現状はすべて直列実行**。worktree 分離（SPEC-04, Phase 4）が未実装のため、
同時に複数トラックを並行作業することはできない。依存グラフは着手順序の自由度を示す。

**推奨実行順**:
```
A0 → A → B → C → E → D → F → G
```
- A0 で dead code を除去し AI の探索ノイズを低減
- A を完了し、C/D の前提を作る
- B は A 直後に実施（CI 検知網を早期に敷く）
- E, F は C↔D の間に挟んで気分転換にも使える（独立した M 規模）

### Phase 2 パイロット移行の詳細手順

`record_round` を選ぶ理由: (1) 自己完結的（外部プロセス管理なし）、(2) Load→変換→Save の典型的な流れ、(3) CLI-02 の中核ロジック

**Step 1: domain 層の確認**
```
libs/domain/src/review.rs:
- ReviewState::record_round / record_round_with_pending は既に純粋な状態遷移
- check_commit_ready も純粋
- 追加作業: 必要に応じてロジックの整理のみ（大きな変更は不要）
```

**Step 2: usecase 層にオーケストレーターを新設**
```
libs/usecase/src/review_workflow.rs に以下を追加:
- record_round(reader: &impl TrackReader, writer: &impl TrackWriter, input) -> Result<Output>
  - reader.load_document() → domain の純粋関数 → writer.save_document()
  - 既存の trait を使う。新しい trait や impl Fn は不要
```

**Step 3: CLI 層を薄いアダプターに書き換え**
```
apps/cli/src/commands/review.rs の run_record_round を:
- FsTrackStore の組み立て（DI）
- usecase::review_workflow::record_round(&store, &store, input) の呼び出し
- 結果の出力
の 3 行に圧縮
```

**Step 4: テスト**
```
libs/usecase/tests/ に:
- mockall の MockTrackReader/MockTrackWriter を注入した record_round テスト
- エラーケース（load 失敗、save 失敗）テスト
- domain の既存テストは維持
```

---

## 11. 成功指標

| 指標 | 現状 | 目標 |
|---|---|---|
| `pr.rs` 行数 | 1432行 | < 500行 |
| `review.rs` 行数 | 1696行 | < 700行 |
| `activate.rs` 行数 | 1000行超 | < 400行（分割後の各ファイル） |
| 700行超の `.rs` ファイル数 | 3+ (vendor/ 除く) | 0 |
| CLI crate 内の `struct`/`enum` 定義数 | (要計測) | CLI 固有のもののみ |
| shell parsing の手書き関数数 | 3+ (`extract_shell_reentry_arg` 等) | 0（conch-parser 委譲） |
| verdict enum の重複定義 | 2 (`ReviewVerdict` / `ReviewPayloadVerdict`) | 1（統合） |
| usecase 内の I/O 直接使用 (`std::fs` 等) | (要計測) | 0 |
| CLI の usecase 関数呼び出し以外のロジック行数 | (要計測) | 最小限（DI + 出力のみ） |
| domain の `pub String` フィールド（有限状態を表すもの） | 3+ (verdict, state, severity) | 0（全て enum 化） |
| domain 外での文字列状態比較 (`== "zero_findings"` 等) | 8箇所 | 0（`match` enum に置換） |
| spec.md に Domain States セクションがないトラック | (未計測) | 0（全トラック必須） |
