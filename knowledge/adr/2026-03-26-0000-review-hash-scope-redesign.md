---
adr_id: 2026-03-26-0000-review-hash-scope-redesign
decisions:
  - id: 2026-03-26-0000-review-hash-scope-redesign_grandfathered
    status: accepted
    grandfathered: true
---
# Review Hash スコープ再設計

## ステータス

Superseded by 2026-04-04-1456-review-system-v2-redesign.md

## 背景

現行の `index_tree_hash_normalizing` は git index 全体の tree hash を計算してレビューの鮮度を判定する。
この設計には 3 つの構造的問題がある：

1. **未ステージ失敗**: `git show :path` は index に登録されていないファイルで exit 128 失敗。新規 track で auto-record が動作しない
2. **並列レビュー干渉**: 各グループの `record-round` が `git add review.json` するたびに tree hash が変化し、先行グループの hash が stale になる
3. **無関係ファイル変更**: レビュー対象外のファイル（他トラック、ドキュメント等）の変更でも hash が変わりレビューが無効化される

3 つとも **hash 計算対象が index 全体** であることに起因する。

### 関連実装

- `libs/infrastructure/src/git_cli/mod.rs`: `index_tree_hash_normalizing`（10 ステップの tree hash 計算）
- `libs/infrastructure/src/review_adapters.rs`: `RecordRoundProtocolImpl`（hash 計算 → record-round → review.json 保存）
- `libs/usecase/src/review_workflow/usecases.rs`: `GitHasher` trait、`check_approved`
- `libs/domain/src/review/state.rs`: `ReviewState::record_round`（hash 文字列を受け取り保存）

## 決定

index 全体の tree hash を **review-scope manifest hash** に置き換える。

### スコープモデル

hash に含めるもの:
- 当該 track の `track/items/<track-id>/` 配下（review 運用ファイルを除く）
- 実装ファイル（`libs/**`, `apps/**`, `Cargo.*` 等）

hash から除外するもの:
- `track/items/<track-id>/review.json`
- `track/items/<track-id>/review-artifacts/**`
- `track/items/<other-track>/**`
- planning-only/documentation ルート（`.claude/docs/**`, `project-docs/**`, `knowledge/**`, `track/registry.md`, `track/tech-stack.md`）

スコープポリシーは `track/review-scope.json` で設定ファイル駆動にする（プロジェクト構造に依存しない汎用設計）。

### hash 計算アルゴリズム

1. git diff（merge-base + staged + unstaged + untracked）でパス一覧を収集
2. `ReviewScopePolicy` でパスを分類、included のみ残す
3. 各ファイルを **worktree から直接読む**（`git show :path` 不要）
4. `metadata.json` は volatile フィールド正規化（`updated_at` → epoch、`review` サブツリー除去）
5. 削除ファイルは tombstone として記録
6. ソート済み manifest を JSON 化 → SHA-256 → `rvw1:sha256:<hex>`

### stored format

`rvw1:sha256:<hex>` prefix でバージョン判別。旧形式（prefix なし）は legacy として migration error を返す。

### RecordRoundProtocolImpl の簡素化

review.json が hash scope 外になるため、hash は record-round の前後で不変。
`CodeHash::Pending` と二相プロトコルが不要になり、single-phase に簡素化できる。

## 却下した代替案

| 代替案 | 却下理由 |
|--------|---------|
| A. track-scoped tree hash | 実装ファイル（`libs/**` 等）を見逃す — scope が狭すぎる |
| B. diff output hash | パッチフォーマットに依存 — 表現に敏感すぎる |
| C. .rs/Cargo ファイル限定 hash | ヒューリスティック — 非 Rust 成果物を見逃す |
| D. metadata-only hash | コード変更を検知できない |
| E. worktree fallback（Step 1 のみ修正） | tree hash は index 全体のため、並列干渉と無関係変更の問題は解決しない |

## Canonical Blocks

### GitHasher port (usecase 層)

```rust
use std::path::PathBuf;

use domain::TrackId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewHashInput {
    pub items_dir: PathBuf,
    pub track_id: TrackId,
    pub base_ref: String,
}

pub trait GitHasher {
    fn review_hash(&self, input: &ReviewHashInput) -> Result<String, String>;
}
```

### ReviewScopePolicyConfig (infrastructure 層: `libs/infrastructure/src/review_scope.rs`)

```rust
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use domain::TrackId;
use globset::{Glob, GlobMatcher, GlobSet, GlobSetBuilder};
use thiserror::Error;
use usecase::review_workflow::scope::RepoRelativePath;

pub const REVIEW_SCOPE_CONFIG_PATH: &str = "track/review-scope.json";

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewScopePolicyConfig {
    pub version: u32,
    #[serde(default)]
    pub review_operational: Vec<String>,
    #[serde(default)]
    pub planning_only: Vec<String>,
    #[serde(default)]
    pub other_track: Vec<String>,
    #[serde(default)]
    pub normalize: BTreeMap<String, NormalizeRule>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizeRule {
    #[serde(default)]
    pub remove_fields: Vec<NormalizeRemovedField>,
    #[serde(default)]
    pub fixed_fields: NormalizeFixedFields,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeRemovedField {
    Review,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizeFixedFields {
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Error)]
pub enum ReviewScopeConfigError {
    #[error("missing review scope config: {path}")]
    Missing { path: PathBuf },

    #[error("failed to read review scope config {path}: {source}")]
    Read { path: PathBuf, #[source] source: std::io::Error },

    #[error("failed to parse review scope config {path}: {source}")]
    Parse { path: PathBuf, #[source] source: serde_json::Error },

    #[error("unsupported review scope config version: {found}")]
    UnsupportedVersion { found: u32 },

    #[error("invalid review scope pattern '{pattern}': {source}")]
    InvalidPattern { pattern: String, #[source] source: globset::Error },

    #[error("multiple normalize rules matched '{path}': {patterns:?}")]
    AmbiguousNormalize { path: String, patterns: Vec<String> },
}

pub fn load_review_scope_policy_config(
    repo_root: &Path,
) -> Result<ReviewScopePolicyConfig, ReviewScopeConfigError> {
    let path = repo_root.join(REVIEW_SCOPE_CONFIG_PATH);
    let raw = std::fs::read_to_string(&path).map_err(|source| match source.kind() {
        std::io::ErrorKind::NotFound => ReviewScopeConfigError::Missing { path: path.clone() },
        _ => ReviewScopeConfigError::Read { path: path.clone(), source },
    })?;
    let config: ReviewScopePolicyConfig = serde_json::from_str(&raw)
        .map_err(|source| ReviewScopeConfigError::Parse { path: path.clone(), source })?;
    if config.version != 1 {
        return Err(ReviewScopeConfigError::UnsupportedVersion { found: config.version });
    }
    Ok(config)
}
```

### ReviewScopePolicy (infrastructure 層)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewPathClass {
    TrackContent,
    Implementation,
    ReviewOperational,
    PlanningOnly,
    OtherTrack,
}

#[derive(Debug, Clone)]
pub struct ReviewScopePolicy {
    pub track_id: TrackId,
    pub config: ReviewScopePolicyConfig,
    review_operational: GlobSet,
    planning_only: GlobSet,
    other_track: Vec<OtherTrackMatcher>,
    normalize: Vec<NormalizeMatcher>,
}

impl ReviewScopePolicy {
    pub fn try_new(
        track_id: TrackId,
        config: ReviewScopePolicyConfig,
    ) -> Result<Self, ReviewScopeConfigError> {
        // 1. <track-id> を展開して review_operational GlobSet をコンパイル
        // 2. planning_only GlobSet をコンパイル
        // 3. <other-track> を sentinel 方式で OtherTrackMatcher にコンパイル
        // 4. normalize パターンを NormalizeMatcher にコンパイル
        todo!()
    }

    pub fn classify(&self, path: &RepoRelativePath) -> ReviewPathClass {
        // ブートストラップ: review-scope.json 自体は無条件 Implementation
        // 評価順序: review_operational → other_track → planning_only → track content → Implementation
        todo!()
    }

    pub fn includes(&self, path: &RepoRelativePath) -> bool {
        matches!(self.classify(path), ReviewPathClass::TrackContent | ReviewPathClass::Implementation)
    }

    pub fn normalize_rule_for(
        &self,
        path: &RepoRelativePath,
    ) -> Result<Option<&NormalizeRule>, ReviewScopeConfigError> {
        // 複数マッチ → AmbiguousNormalize エラー（fail-closed）
        todo!()
    }
}
```

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeEntryState {
    File { sha256: String },
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewScopeEntry {
    pub path: RepoRelativePath,
    pub state: ScopeEntryState,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReviewScopeManifest {
    pub version: u8,
    pub algorithm: &'static str,
    pub base_ref: String,
    pub entries: Vec<ReviewScopeManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReviewScopeManifestEntry {
    pub path: String,
    pub state: &'static str,
    pub sha256: Option<String>,
}
```

```rust
fn normalize_track_file_for_hash(
    policy: &ReviewScopePolicy,
    repo_rel_path: &RepoRelativePath,
    bytes: Vec<u8>,
) -> Result<Vec<u8>, ReviewScopeConfigError> {
    // Config-driven: consult policy's normalize rules from review-scope.json
    let Some(rule) = policy.normalize_rule_for(repo_rel_path)? else {
        return Ok(bytes);
    };

    let mut json: serde_json::Value = serde_json::from_slice(&bytes).map_err(|source| {
        ReviewScopeConfigError::Parse {
            path: PathBuf::from(repo_rel_path.as_str()),
            source,
        }
    })?;

    if let serde_json::Value::Object(obj) = &mut json {
        for field in &rule.remove_fields {
            if matches!(field, NormalizeRemovedField::Review) {
                obj.remove("review");
            }
        }
        if let Some(updated_at) = &rule.fixed_fields.updated_at {
            obj.insert("updated_at".to_owned(), serde_json::Value::String(updated_at.clone()));
        }
    }

    serde_json::to_vec_pretty(&json).map_err(|source| ReviewScopeConfigError::Parse {
        path: PathBuf::from(repo_rel_path.as_str()),
        source,
    })
}
```

```rust
pub struct SystemGitHasher {
    pub base_ref: String,
}

impl GitHasher for SystemGitHasher {
    fn review_hash(&self, input: &ReviewHashInput) -> Result<String, String> {
        let git = crate::git_cli::SystemGitRepo::discover()
            .map_err(|e| format!("git error: {e}"))?;
        // Load config from track/review-scope.json (fail-closed on missing/invalid)
        let config = load_review_scope_policy_config(git.root())
            .map_err(|e| e.to_string())?;
        let policy = ReviewScopePolicy::try_new(input.track_id.clone(), config)
            .map_err(|e| e.to_string())?;
        let scope = build_review_scope(&git, input, &policy)?;
        let manifest = build_review_scope_manifest(&git, scope, &input.base_ref)?;
        let json = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
        let digest = sha2::Sha256::digest(&json);
        Ok(format!("rvw1:sha256:{digest:x}"))
    }
}
```

```rust
impl RecordRoundProtocol for RecordRoundProtocolImpl {
    fn execute(
        &self,
        track_id: &TrackId,
        round_type: RoundType,
        group_name: ReviewGroupName,
        verdict: Verdict,
        concerns: Vec<ReviewConcern>,
        expected_groups: Vec<ReviewGroupName>,
        timestamp: Timestamp,
    ) -> Result<(), RecordRoundProtocolError> {
        let hash_input = ReviewHashInput {
            items_dir: self.items_dir.clone(),
            track_id: track_id.clone(),
            base_ref: "main".to_owned(),
        };
        let current_hash = self.hasher.review_hash(&hash_input)
            .map_err(RecordRoundProtocolError::Other)?;

        self.reviews.with_locked_review(track_id, |review| {
            let round = build_round_result(
                review, round_type, verdict, concerns.clone(), timestamp.clone(),
            );
            review.record_round(
                round_type, &group_name, round, &expected_groups, &current_hash,
            ).map_err(map_review_error)?;
            Ok(())
        })?;

        self.git_add_review_json(track_id)?;
        Ok(())
    }
}
```

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredReviewHash {
    Legacy(String),
    ReviewScopeV1(String),
}

impl StoredReviewHash {
    pub fn parse(raw: &str) -> Result<Self, ReviewError> {
        if let Some(hex) = raw.strip_prefix("rvw1:sha256:") {
            let is_valid = hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit());
            if !is_valid {
                return Err(ReviewError::InvalidConcern(format!(
                    "malformed rvw1 hash: expected 64 hex chars after prefix, got '{hex}'"
                )));
            }
            Ok(Self::ReviewScopeV1(raw.to_owned()))
        } else {
            Ok(Self::Legacy(raw.to_owned()))
        }
    }
}
```

## 影響

- `GitHasher` trait: `normalized_hash` → `review_hash(ReviewHashInput)` に更新（互換 shim あり）
- `SystemGitHasher`: `index_tree_hash_normalizing` 委譲をやめ、scope 収集 → worktree 読込 → manifest hash の実アダプターに
- `RecordRoundProtocolImpl`: single-phase 化（`Pending` 不要）
- `CodeHash::Pending`: 使用停止、最終的に除去
- domain 層: 変更なし（hash 文字列の比較のみ、hash 計算ロジックに依存しない）
- migration: 旧 hash は `rvw1:` prefix なしで判別 → `check_commit_ready` で明確な migration error

## 追加決定: 設定ファイル駆動ポリシー（track/review-scope.json）

ユーザーとの議論（2026-03-27）で、ハードコードされたディレクトリパターンを設定ファイル駆動に変更することを決定。
Codex planner (gpt-5.4) による検証結果を含む。

### スキーマ

```json
{
  "version": 1,
  "review_operational": ["track/items/<track-id>/review.json", "track/items/<track-id>/review-artifacts/**"],
  "planning_only": [".claude/docs/**", "project-docs/**", "knowledge/**", "track/registry.md", "track/tech-stack.md"],
  "other_track": ["track/items/<other-track>/**", "track/archive/**"],
  "normalize": {
    "**/metadata.json": {
      "remove_fields": ["review"],
      "fixed_fields": { "updated_at": "1970-01-01T00:00:00Z" }
    }
  }
}
```

- `<track-id>` と `<other-track>` はランタイムで展開
- マッチしないパスはデフォルトで `Implementation`（hash に含む）
- glob パターンマッチングを使用（パス分類セマンティクスに適合）
- `normalize` は hash セマンティクスの一部として同一ファイル内に維持（汎用 rewrite DSL にはしない）

### fail-closed ポリシー

- `review-scope.json` が存在しない場合: **明示的エラーで失敗**（サイレント fallback 不可）
- 理由: このファイルはセキュリティ境界を定義する。デフォルト fallback はスコープの暗黙拡大リスク

### config 自体の hash 包含

- `review-scope.json` 自体は hash scope に **含める**（除外しない）
- 理由: ポリシー変更は旧承認を無効化すべき。除外すると「scope を広げて review を迂回」が可能

### ブートストラップルール

`review-scope.json` の分類は自己参照になる（ポリシーファイルをロードする前に、そのファイル自体をどう分類するか不明）。
解決: `track/review-scope.json` は **ハードコードされた特別パス** として、ポリシーロード前に無条件で `Implementation`（hash scope に含む）に分類する。
ポリシーファイル内のパターンが `review-scope.json` 自体にマッチしても無視する（ブートストラップ優先）。

### パターン優先順位

- 優先順位はコードで固定（ファイル内の記述順序には依存しない）
- 評価順序: `review_operational` → `other_track` → `planning_only` → `track content` → `Implementation`（デフォルト）

## 追加決定: トラック依存順序

- `tamper-proof-review-2026-03-26` は本トラック完了後に unblock
- additive な作業（provenance 型定義、schema v2 serde、artifact layout、verifier 抽象）は並行可能
- `record-round` 削除、provenance enforcement、auto-record 唯一化は本トラック完了が前提

## 出典

Codex planner (gpt-5.4) により設計、2026-03-26。
ユーザーとの議論で設定ファイル駆動ポリシー（`track/review-scope.json`）を追加決定、2026-03-27。
Codex planner (gpt-5.4) により 3 決定の検証完了、2026-03-27。
