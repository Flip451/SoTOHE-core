# Review Process Audit Report (2026-03-31)

> **目的**: review.json 移行完了後の review プロセス全体を監査し、既知 TODO と新発見を統合整理する
> **対象コード**: `libs/domain/src/review/`, `libs/usecase/src/review_workflow/`, `libs/infrastructure/src/review_*.rs`, `apps/cli/src/commands/review/`
> **関連 SKILL**: `.claude/commands/track/review.md`
> **関連設定**: `track/review-scope.json`, `.claude/agent-profiles.json`

---

## 1. アーキテクチャの現状

### 1.1 二重モデルの整理状況

review.json 移行（`review-json-per-group-review` + `autorecord-reviewjson-wiring`）により:

- **production path**: `ReviewCycle` + `all_groups_approved`（cycle model）のみ使用
- **legacy**: `ReviewState` + `check_commit_ready` は domain 層に残存するが、usecase からの呼び出しはゼロ

**判定**: 構造的問題ではなく legacy dead code の問題。以下の legacy 関連 TODO は深刻度を **LOW** に下げる:

| ID | 元の深刻度 | 修正後 | 理由 |
|---|---|---|---|
| RVW-24 | HIGH | LOW | `update_status_after_record` は legacy model。production 未使用 |
| WF-62 | MEDIUM | LOW | 同上 |
| RVW-09 | MEDIUM | LOW | legacy `invalidate()` は production 未使用 |
| WF-48 | LOW | LOW | 維持（legacy `with_fields` バリデーション） |
| WF-44 | LOW | LOW | 維持（legacy codec 整合性） |

**推奨**: legacy `ReviewState` の除去を検討。除去すれば RVW-24, WF-62, RVW-09, C-3, C-5 が一括解消。

### 1.2 エスカレーション状態の二重管理（既知: T005/T006）

- エスカレーション状態が `metadata.json` に残存
- `check_approved` と `RecordRoundProtocolImpl::execute` が `metadata.json` からエスカレーションゲートを別途読み込み
- review.json への移行が未完了

**対応 TODO**: RVW-06（既存）。移行完了まで drift リスクあり。

---

## 2. 新発見の問題（既存 TODO 未登録）

### 2.1 CRITICAL: グループ名不一致 `infra` vs `infrastructure`

- **場所**: `.claude/commands/track/review.md` vs `track/review-scope.json`
- **影響**: review.md（SKILL.md）が `--group infra` を渡すが、`review-scope.json` は `infrastructure` を定義。`check-approved` のパーティション比較でキー不一致が発生し、レビュー承認が通らなくなる
- **CLI に alias/mapping なし**: `ReviewGroupName` は trim のみ実行、変換なし
- **修正**: review.md の `infra` → `infrastructure` に統一（または `review-scope.json` を `infra` に変更）
- **推奨 ID**: RVW-37

### 2.2 HIGH: `FindingDocument` に `category` フィールドなし（データ消失）

- **場所**: `libs/infrastructure/src/review_json_codec.rs` の `FindingDocument`、`libs/domain/src/review/cycle/round_types.rs` の `StoredFinding`
- **影響**: reviewer が返す `category` フィールドが review.json のラウンドトリップで消失。`findings_to_concerns()` の 3 段階フォールバックの第 1 段階（reviewer 提供 category）が機能しない
- **修正**: `FindingDocument` と `StoredFinding` に `category: Option<String>` を追加
- **推奨 ID**: RVW-38

### 2.3 HIGH: Timeout/ProcessFailed で auto-record スキップ → review.json stale

- **場所**: `apps/cli/src/commands/review/codex_local.rs` lines 93-102
- **影響**: Codex タイムアウトやプロセス失敗時に review.json が更新されない。次ラウンドでコード変更があると hash mismatch で invalidation cascade が発生
- **現状**: 意図的設計（failed round は記録しない）だが、orchestrator が stale 状態を認識する手段がない
- **提案**: (1) failed/timeout を verdict として review.json に記録（informational round）、(2) 最低限 stderr に stale 警告を出力、(3) orchestrator 向けに exit code を stale-aware にする
- **推奨 ID**: RVW-39

### 2.4 HIGH: `RecordRoundProtocolImpl::execute` の巨大関数 + policy resolution 重複

- **場所**: `libs/infrastructure/src/review_adapters.rs` lines 261-555（295 行、`#[allow(clippy::too_many_lines)]`）
- **影響**: policy resolution + diff 取得が同一関数内で 2 回実行される（cycle auto-create 時と group scope hash 計算時）。保守性低下 + 微妙な乖離リスク
- **修正**: policy resolution 結果をキャッシュし、cycle auto-create と scope hash 計算で共有
- **推奨 ID**: RVW-40

### 2.5 MEDIUM: `check_approved` の `_writer` 未使用パラメータ

- **場所**: `libs/usecase/src/review_workflow/usecases.rs` line 352
- **影響**: API の誤解を招く。`TrackWriter` を受け取るが一切使用しない
- **修正**: パラメータ削除、呼び出し元も更新
- **推奨 ID**: RVW-41

### 2.6 MEDIUM: `StoredFinding` lossy conversion の codec 側

- **場所**: `review_adapters.rs` lines 451-458
- **影響**: `findings_remain` verdict で concern slug のみが `StoredFinding::new(slug, None, None, None)` に変換され、元の message/severity/file/line が消失。review.json には concern 名しか残らない
- **関連**: RVW-34（既存）。ここでは codec 側の `FindingDocument` にも severity/file/line が保持されない点を補足
- **修正**: `RecordRoundProtocol` trait に `findings: Vec<StoredFinding>` パラメータを追加し、元データを保持

### 2.7 MEDIUM: corrupted partial output で session log fallback 不発

- **場所**: `codex_local.rs` の `run_codex_child` — session log fallback は `Missing` 状態でのみ起動
- **影響**: output-last-message ファイルが存在するが中身が壊れている（`Invalid` 状態）場合、session log からの verdict 回復が試みられない
- **修正**: `Invalid` 状態でも session log fallback を試行
- **推奨 ID**: RVW-42

### 2.8 LOW: `serde_json::to_string().unwrap_or_default()` in policy hash

- **場所**: `review_group_policy.rs` line 273
- **影響**: 現実には `serde_json::Value` の serialization は失敗しないため、実害なし。ただし将来のリファクタで失敗パスが導入された場合、すべてのポリシーが同一 hash になり stale 検出が全面的に壊れる
- **修正**: `.expect("serde_json::Value is always serializable")` に変更
- **推奨 ID**: RVW-43

---

## 3. 設定・ドキュメントの乖離

### 3.1 `escalation_threshold` が JSON から読まれない

- **場所**: `.claude/agent-profiles.json` の `"escalation_threshold": 3` vs `libs/domain/src/review/escalation.rs` のハードコード `3`
- **影響**: JSON 値を変更しても効果なし
- **既知**: `10-guardrails.md` に「registered for future configurability but is not yet read by the runtime」と記載済み
- **対応**: 既知の設計判断。変更時は `.claude/rules/10-guardrails.md` の記述も更新必要

### 3.2 output schema の `category` required vs Rust の optional

- **場所**: `REVIEW_OUTPUT_SCHEMA_JSON` vs `ReviewFinding` struct
- **影響**: Codex に `category` を required として指示するが、Rust 側は `#[serde(default)]` で欠損を許容。schema と実装が乖離
- **修正**: (a) schema から required を外す、または (b) Rust の shape validator に `!seen_category` チェックを追加
- **関連**: RVW-38（category の codec round-trip 問題）と同時修正が効率的

### 3.3 `full_auto` フラグは reviewer では意図的に無視

- **場所**: `codex_local.rs` の `build_codex_invocation`
- **判定**: **非問題**。RVW-29 修正の結果。`--full-auto` は `--sandbox workspace-write` を強制するため reviewer では使用禁止。テスト `build_codex_invocation_never_includes_full_auto_even_for_full_model` で保証済み
- **対応不要**: ただし `agent-profiles.json` の `full_auto: true` は reviewer 以外（planner 等）向けであり、reviewer では事実上 dead config

### 3.4 `RecordRoundArgs` の全フィールド `String` 型

- **場所**: `apps/cli/src/commands/review/mod.rs` lines 178-211
- **影響**: `round_type` は `CodexRoundTypeArg`（`ValueEnum`）が `CodexLocalArgs` 側に存在するが、手動 `record-round` では生 `String`。`expected_groups`/`concerns` も `value_delimiter` 未使用
- **修正**: `RecordRoundArgs` を `CodexLocalArgs` と同様に型付き引数に統一
- **関連**: 既存 TODO に該当なし。minor cleanup

---

## 4. legacy ReviewState 関連（production 未使用だが残存）

以下は legacy `ReviewState` model に属する問題。production path（cycle model）では影響なし。

| ID | 深刻度 | 内容 | 推奨対応 |
|---|---|---|---|
| RVW-24 | ~~HIGH~~ → LOW | 過剰降格ロジック | legacy 除去で消滅 |
| WF-62 | ~~MEDIUM~~ → LOW | Approved→fast で降格しない | legacy 除去で消滅 |
| RVW-09 | ~~MEDIUM~~ → LOW | 全グループ一律 invalidation | legacy 除去で消滅 |
| (C-3) | LOW | `check_commit_ready` が Approved+NotRecorded を許容 | legacy 除去で消滅 |
| (C-5) | LOW | `FinalOnly` が NoRounds から runtime 到達可能 | legacy 除去で消滅 |
| WF-44 | LOW | codec phase/streak 不整合 | legacy 除去時に codec も整理 |
| WF-48 | LOW | `with_fields` の threshold/block バリデーション | legacy 除去で消滅 |
| WF-49 | LOW | streak リセット方式 | legacy 除去で消滅 |

**推奨**: legacy `ReviewState` を domain から除去する単独トラックを検討。8 件の TODO が一括解消。

---

## 5. 既存 TODO の現状整理（セクション L: RVW-*）

### 完了済み（5件）
- ~~RVW-21~~: per-group 独立レビュー進行 ✅
- ~~RVW-22~~: diff_base 永続化 ✅
- ~~RVW-23~~: is_planning_only_path SSoT 統合 ✅
- ~~RVW-29~~: --full-auto sandbox override ✅
- ~~RVW-31~~: review.json 分離 ✅
- ~~RVW-32~~: round 同期制約緩和 ✅
- ~~RVW-33~~: frozen partition scope 接続 ✅

### 有効（production path に影響、HIGH）
| ID | 内容 | 備考 |
|---|---|---|
| RVW-06 | エスカレーション状態を review.json に移行 + 順序強制 | T005/T006 残り |
| RVW-07 | Codex verdict 抽出の stderr fallback + session log 保存 | RVW-42 と関連 |
| RVW-08 | ScopeFilteredPayload 削除（findings フィルタ禁止） | |
| RVW-20 | ACCEPTED finding 仕組み化 + dispute adjudication | |
| RVW-25 | CodeHash inner の ReviewHash newtype 化 | |
| RVW-30 | staging 漏れによるコミット/承認状態乖離 | |
| RVW-34 | StoredFinding lossy conversion | |
| RVW-36 | Codex 上限到達時の fallback | |

### 有効（MEDIUM 以下）
| ID | 内容 | 備考 |
|---|---|---|
| RVW-01 | Frontmatter パーサー抽出 | コード品質 |
| RVW-02 | conch-parser AST 直接走査 | コード品質 |
| RVW-03 | typed deserialization convention | IN PROGRESS |
| RVW-14 | path normalization 改善 | |
| RVW-15 | GitDiffScope テスト強化 | |
| RVW-26 | fast model false positive 対策 | |
| RVW-35 | group_scope_hash volatile field | |

### 有効（LOW）
RVW-04, RVW-05, RVW-16, RVW-17, RVW-18, RVW-19, RVW-27, RVW-28

---

## 6. 優先度順アクションリスト

### Tier 0: 即時修正（レビューが通らなくなる）

1. **RVW-37 (NEW)**: review.md の `infra` → `infrastructure` に修正。5 分で完了

### Tier 1: 次トラックで対応（データ品質・信頼性）

2. **RVW-38 (NEW)**: `FindingDocument` + `StoredFinding` に `category` 追加 + schema 整合性修正
3. **RVW-34**: StoredFinding lossy conversion（元 findings データ保持）
4. **RVW-08**: ScopeFilteredPayload 削除
5. **RVW-39 (NEW)**: timeout/failed の stale 対策

### Tier 2: 構造改善（保守性）

6. **RVW-40 (NEW)**: `RecordRoundProtocolImpl::execute` 分割 + policy resolution 重複排除
7. **RVW-41 (NEW)**: `_writer` パラメータ削除
8. **RVW-06**: エスカレーション状態の review.json 移行
9. **Legacy ReviewState 除去トラック**: RVW-24, WF-62, RVW-09 等 8 件を一括解消

### Tier 3: 防御強化

10. **RVW-20**: ACCEPTED finding 仕組み化
11. **RVW-36**: Codex 上限 fallback
12. **WF-36**: model-tier 強制（sequential escalation の機構化）
13. **RVW-30**: staging 漏れ検出

---

## 7. 新規 TODO ID 採番まとめ

| ID | 深刻度 | 内容 | Tier |
|---|---|---|---|
| **RVW-37** | CRITICAL | review.md グループ名 `infra` → `infrastructure` 修正 | 0 |
| **RVW-38** | HIGH | `FindingDocument`/`StoredFinding` に `category` 追加 + schema 整合性 | 1 |
| **RVW-39** | HIGH | timeout/failed 時の review.json stale 対策 | 1 |
| **RVW-40** | MEDIUM | `RecordRoundProtocolImpl::execute` 分割 + policy 重複排除 | 2 |
| **RVW-41** | MEDIUM | `check_approved` の `_writer` 未使用パラメータ削除 | 2 |
| **RVW-42** | MEDIUM | corrupted output での session log fallback 不発 | 1 |
| **RVW-43** | LOW | policy hash の `unwrap_or_default` → `expect` | 2 |
