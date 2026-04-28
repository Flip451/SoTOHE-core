---
adr_id: 2026-04-04-1456-review-system-v2-redesign
decisions:
  - id: 2026-04-04-1456-review-system-v2-redesign_grandfathered
    status: accepted
    grandfathered: true
---
# Review System v2: frozen scope 廃止とスコープ独立型レビュー

## Status

Accepted

## Context

Review System v1 は以下の構造的問題を抱え、開発効率を著しく阻害していた:

1. **frozen scope / current partition / check_approved のスコープ不整合**:
   サイクル作成時は `base_ref` ("main") から全ブランチ diff で frozen scope を構築。
   `record_round` は `effective_diff_base` (approved_head) からのインクリメンタル diff で
   ハッシュ計算。`check_approved` は frozen scope でハッシュ計算。
   3 者のファイルリストが異なるため、`approved_head` が進むほど乖離が拡大し
   HashMismatch が頻発。

2. **`has_scope_drift` によるサイクル全体の無効化**:
   1 つのスコープにファイルが追加されるだけで `PartitionChanged` が発生し、
   全 6 グループの再レビューが必要になる。本来は影響を受けたスコープだけで済むはず。

3. **ワークフローアーティファクトのハッシュ循環**:
   `metadata.json`, `plan.md` が `other` グループの frozen scope に含まれるため、
   レビュー記録やタスク遷移の副作用でハッシュが変わり、`other` が永続的に
   HashMismatch。全グループ再レビューの無限ループに陥る。

4. **approved_head がサイクル作成時に無視される**:
   サイクル作成時は常に `base_ref` を使用。既存サイクルが存在する場合は
   新サイクルを作らないため、`approved_head` が進んでも frozen scope は
   最初のサイクル作成時のまま更新されない。

これらは個別の修正では対処不可能で、継ぎ足し設計（RVW-37〜RVW-57）の結果として
不整合が構造的に組み込まれている。ゼロからの再設計が必要と判断した。

## Decision

Review System v2 として以下の設計を採用する。

### 設計原則

1. **frozen scope を廃止** — スコープは毎回 diff から動的に計算する
2. **スコープ単位で独立** — 1 つのスコープの変更が他のスコープを無効化しない
3. **before/after hash 比較** — レビュー中のファイル変更を検出する
4. **シンプルな状態遷移** — `ReviewState` enum で状態を明確に表現
5. **fast は参考値** — fast → final の順序制約を廃止。approved 判定は最新の final 結果のみ

### レイヤー配置

| 層 | 配置するもの |
|----|-------------|
| **domain** | 純粋データ型: `ScopeName`, `ReviewTarget`, `ReviewHash`, `ReviewState`, `Verdict`, `FastVerdict`, `Finding`, `ReviewOutcome<V>` |
| **domain** | 純粋分類ロジック: `ReviewScopeConfig`（`classify`, `get_scope_names`, `contains_scope`） |
| **domain** | 永続化 port trait: `ReviewReader`, `ReviewWriter`, `CommitHashReader`, `CommitHashWriter` |
| **usecase** | アプリケーション port trait: `Reviewer`, `DiffGetter`, `ReviewHasher` |
| **usecase** | オーケストレーター: `ReviewCycle<R, H, D>` |
| **infra** | port 実装: `CodexReviewer`, `GitDiffGetter`, `SystemReviewHasher`, `FsReviewReader`, `FsReviewWriter`, `FsCommitHashReader`, `FsCommitHashWriter` |
| **CLI** | composition root: port の構築と注入 |

判断基準:
- 永続化に関するポート → domain（`TrackReader` / `TrackWriter` と同じ位置）
- アプリケーションサービスが必要とするポート → usecase（外部プロセス呼び出し、git 操作、ハッシュ計算）
- フロー制御・オーケストレーション → usecase（`ReviewCycle` が reviewer/hasher/diff_getter を呼ぶ）
- 純粋なデータ型とバリデーション → domain

### Domain 型

#### ScopeName

```rust
struct MainScopeName(String);

impl MainScopeName {
    fn new(s: impl Into<String>) -> Result<Self, ScopeNameError> {
        let s = s.into();
        if s == "other" { return Err(ScopeNameError::Reserved); }
        if s.is_empty() { return Err(ScopeNameError::Empty); }
        if !s.is_ascii() { return Err(ScopeNameError::NotAscii); }
        Ok(Self(s))
    }
}

enum ScopeName {
    Main(MainScopeName),
    Other,
}
```

#### ReviewTarget / ReviewHash

```rust
struct ReviewTarget(Vec<FilePath>);

enum ReviewHash {
    Some(String),  // "rvw1:sha256:<hex>" 形式
    Empty,
}
```

#### ハッシュ計算 contract

`ReviewHasher.calc()` の実装は v1 の `SystemGitHasher::group_scope_hash()` から移植する。
具体的な計算手順:

1. スコープ内のファイルパスをアルファベット順にソート
2. 各ファイルについて worktree から `O_NOFOLLOW` で読み込み:
   - 存在するファイル → `"<path>\t<sha256_of_content>\n"`
   - 存在しないファイル → `"<path>\tDELETED\n"`（tombstone）
3. マニフェスト全体を SHA256 → `"rvw1:sha256:<hex>"` 形式で返す
4. 空の `ReviewTarget` → `ReviewHash::Empty`

不変条件:
- パスのソートにより順序非依存
- tombstone によりファイル削除を検出
- `rvw1:` バージョン接頭辞により将来のフォーマット変更に対応
- `O_NOFOLLOW` により symlink 追跡を防止
- post-open で repo root 内に留まることを検証（TOCTOU 対策）

#### Verdict / FastVerdict / Finding

```rust
enum Verdict {
    ZeroFindings,
    FindingsRemain(Vec<Finding>),  // 非空保証（コンストラクタで検証）
}

impl Verdict {
    fn findings_remain(findings: Vec<Finding>) -> Result<Self, VerdictError> {
        if findings.is_empty() { return Err(VerdictError::EmptyFindings); }
        Ok(Self::FindingsRemain(findings))
    }
}

enum FastVerdict {
    ZeroFindings,
    FindingsRemain(Vec<Finding>),
}

impl FastVerdict {
    fn findings_remain(findings: Vec<Finding>) -> Result<Self, VerdictError> {
        if findings.is_empty() { return Err(VerdictError::EmptyFindings); }
        Ok(Self::FindingsRemain(findings))
    }
}

struct Finding {
    message: String,  // 非空保証（コンストラクタで検証）
    severity: Option<String>,
    file: Option<String>,
    line: Option<u64>,
    category: Option<String>,
}

impl Finding {
    fn new(
        message: impl Into<String>,
        severity: Option<String>,
        file: Option<String>,
        line: Option<u64>,
        category: Option<String>,
    ) -> Result<Self, FindingError> {
        let message = message.into();
        if message.trim().is_empty() { return Err(FindingError::EmptyMessage); }
        Ok(Self { message, severity, file, line, category })
    }
}
```

#### ReviewOutcome

`review()` / `fast_review()` の戻り値。空スコープの場合は `Skipped` を返す。
型パラメータ `V` で `Verdict` と `FastVerdict` を区別する。

```rust
enum ReviewOutcome<V> {
    Reviewed {
        verdict: V,
        log_info: LogInfo,
        hash: ReviewHash,
    },
    Skipped,  // 空スコープ — レビュー対象ファイルなし
}

// review()       → Result<ReviewOutcome<Verdict>, ReviewCycleError>
// fast_review()  → Result<ReviewOutcome<FastVerdict>, ReviewCycleError>
```

#### ReviewState

各スコープのレビュー状態。レビューが必要かどうかを表現する。

```rust
enum ReviewState {
    Required(RequiredType),
    NotRequired(NotRequiredType),
}

enum RequiredType {
    NotStarted,       // final 未実施
    FindingsRemain,   // 最新 final が findings_remain
    StaleHash,        // 最新 final のハッシュと現在のハッシュが不一致
}

enum NotRequiredType {
    Empty,            // スコープに対象ファイルなし
    ZeroFindings,     // 最新 final が zero_findings かつハッシュ一致
}
```

#### ReviewScopeConfig

review-scope.json を入力として構築される、スコープ名と glob パターンのマッピング。

- `other` スコープは暗黙的に存在し、どの名前付きスコープにもマッチしないファイルを受け取る
- GlobSet は空も可能
- review.json 等の operational ファイルはここで除外する

```rust
struct ReviewScopeConfig {
    scopes: HashMap<MainScopeName, GlobSet>,
    operational: GlobSet,   // 除外パターン（review.json 等）
    other_track: GlobSet,   // 他トラックのアーティファクト除外パターン
}

impl ReviewScopeConfig {
    /// review-scope.json を入力として構築。
    ///
    /// `track_id` を使って以下のプレースホルダを展開する（v1 から移植）:
    /// - `<track-id>` → 現在のトラック ID に展開（operational / other_track パターン内）
    /// - `<other-track>` → 現在のトラック ID 以外の track/items/*/ にマッチするパターンに展開
    ///
    /// プレースホルダ展開は `new()` 内で行い、構築後の `ReviewScopeConfig` は
    /// 展開済み glob のみを保持する。
    fn new(
        track_id: &TrackId,
        entries: Vec<(String, Vec<String>)>,
        operational: Vec<String>,
        other_track: Vec<String>,
    ) -> Result<Self, ScopeConfigError>;

    /// ファイル一覧からスコープに分類（純粋ロジック。I/O なし）
    /// 1つのファイルが複数の named scope にマッチする場合、両方のスコープに含める。
    /// 重複しても各スコープのレビュワーが独立に検査するだけで問題ない。
    fn classify(&self, files: &[FilePath]) -> HashMap<ScopeName, Vec<FilePath>>;

    /// ファイル一覧から関連するスコープ名の集合を取得
    fn get_scope_names(&self, files: &[FilePath]) -> HashSet<ScopeName>;

    /// 指定されたスコープ名がこの config に存在するか検証
    fn contains_scope(&self, scope: &ScopeName) -> bool;

    /// config に定義されている全スコープ名を返す（Other 含む）
    fn all_scope_names(&self) -> HashSet<ScopeName>;
}
```

**注意:** `get_review_target()` や `calc_all_hash()` は `DiffGetter` / `ReviewHasher` に
依存するため、domain 層ではなく usecase 層の `ReviewCycle` に移動。
`ReviewScopeConfig` は純粋な分類ロジックのみ提供する。

分類ルール:
- 1 つのファイルが複数の named scope にマッチする場合、両方のスコープに含める
- どの named scope にもマッチしないファイルは `Other` に分類
- `operational` パターンにマッチするファイルは事前に除外
- `other_track` パターンにマッチするファイル（他トラックのアーティファクト）は事前に除外

#### v1 review-scope.json のポリシーフィールドの v2 での扱い

| v1 フィールド | v2 での扱い | 理由 |
|--------------|------------|------|
| `groups` | `ReviewScopeConfig.scopes` に引き継ぎ | スコープ分類の主要機能 |
| `review_operational` | `ReviewScopeConfig.operational` に引き継ぎ | review.json 等の除外 |
| `other_track` | `ReviewScopeConfig.other_track` として引き継ぎ | 作業中トラックと作業外トラックの修正は区別すべき |
| `planning_only` | **廃止・見直し** | 以下参照 |
| `normalize` | **廃止** | frozen scope がないためハッシュ正規化不要。diff は毎回動的計算 |

**`planning_only` の見直し:**

v1 の `planning_only` パターンは以下を含んでいたが、混在が問題:
- 存在しない系列: `".claude/docs/**/*.md"`, `"docs/**/*.md"`, `"docs/**/*.json"` → 削除
- 現存する系列: `"knowledge/**/*.md"` → `Other` スコープに含めて通常レビュー対象
- git-tracked されていないファイル: `"track/registry.md"` → gitignored なのでそもそも diff に出ない
- 運用文書: `"DEVELOPER_AI_WORKFLOW.md"`, `"README.md"` 等 → `harness-policy` スコープに含めるべき

v2 では `planning_only` フィールド自体を廃止し、各ファイルを適切なスコープに分類する。
`harness-policy` スコープの glob パターンを拡張して以下を含める:
- 運用文書: `DEVELOPER_AI_WORKFLOW.md`, `README.md` 等
- レビューポリシー: `track/review-scope.json`（ポリシー変更が `harness-policy` スコープの
  `StaleHash` として検出されるために必要）

#### ReviewReader (domain port)

review.json からレビュー状態を読み込む。永続化ポートなので domain 層に定義。

```rust
trait ReviewReader: Send + Sync {
    /// 全スコープの最新 final verdict とハッシュを読み込み
    fn read_latest_finals(
        &self,
    ) -> Result<HashMap<ScopeName, (Verdict, ReviewHash)>, ReviewReaderError>;
}
```

#### ReviewWriter (domain port)

レビュー結果を review.json に永続化する。永続化ポートなので domain 層に定義。

```rust
trait ReviewWriter: Send + Sync {
    /// final verdict（findings 含む）とハッシュをスコープの履歴に追記
    fn write_verdict(
        &self,
        scope: &ScopeName,
        verdict: &Verdict,
        hash: &ReviewHash,
    ) -> Result<(), ReviewWriterError>;

    /// fast verdict（findings 含む）とハッシュをスコープの履歴に追記
    fn write_fast_verdict(
        &self,
        scope: &ScopeName,
        verdict: &FastVerdict,
        hash: &ReviewHash,
    ) -> Result<(), ReviewWriterError>;

    /// 新規 review.json を作成（トラック初期化時に使用）
    /// CommitHashWriter.clear() と組み合わせて呼ぶ。
    fn init(&self) -> Result<(), ReviewWriterError>;

    /// 既存 review.json をアーカイブし、新規 review.json を作成（レビュー状態のリスタート時に使用）
    /// .commit_hash はクリアしない — diff base は維持し、インクリメンタルスコープで再レビューする。
    fn reset(&self) -> Result<(), ReviewWriterError>;
}
```

#### CommitHashReader / CommitHashWriter (domain port)

diff base のコミットハッシュを読み書きする。永続化ポートなので domain 層に定義。

```rust
trait CommitHashReader: Send + Sync {
    /// .commit_hash ファイルからコミットハッシュを読み込む。
    /// ファイルが存在しない場合は None を返す。
    fn read(&self) -> Result<Option<CommitHash>, CommitHashError>;
}

trait CommitHashWriter: Send + Sync {
    /// コミットハッシュを .commit_hash ファイルに書き込む。
    /// atomic write で crash safety を保証する。
    fn write(&self, hash: &CommitHash) -> Result<(), CommitHashError>;

    /// .commit_hash ファイルを削除する（トラック初期化時に使用）。
    /// ファイルが存在しない場合は成功として扱う。
    fn clear(&self) -> Result<(), CommitHashError>;
}
```

### UseCase Port Traits

#### Reviewer (usecase port)

外部レビュワー（Codex 等）を抽象化するポート。

```rust
trait Reviewer {
    fn fast_review(
        &self,
        target: &ReviewTarget,
    ) -> Result<(FastVerdict, LogInfo), ReviewerError>;

    fn review(
        &self,
        target: &ReviewTarget,
    ) -> Result<(Verdict, LogInfo), ReviewerError>;
}

enum ReviewerError {
    UserAbort,
    ReviewerAbort,
    Timeout,
    IllegalVerdict,  // Verdict の JSON 形状不正
    Unexpected(String),
}
```

#### DiffGetter (usecase port)

Git を通じて現在の差分ファイル一覧を取得する。
v1 の `GitDiffScopeProvider` から移植。以下の 4 ソースの和集合を返す:

1. `git diff --name-only --diff-filter=ACDMRT $(git merge-base HEAD <base>) HEAD` — base と HEAD の共通祖先からのコミット済み差分
2. `git diff --name-only --cached` — ステージ済み未コミット
3. `git diff --name-only` — 未ステージの worktree 変更
4. `git ls-files --others --exclude-standard` — 未追跡（非 gitignore）ファイル

重複はパスの `BTreeSet` で排除。各パスは `RepoRelativePath` に正規化。

```rust
trait DiffGetter {
    fn list_diff_files(
        &self,
        base: &CommitHash,
    ) -> Result<Vec<FilePath>, DiffGetError>;
}
```

#### ReviewHasher (usecase port)

レビュー対象のハッシュを計算する。

```rust
trait ReviewHasher {
    fn calc(&self, target: &ReviewTarget) -> Result<ReviewHash, ReviewHasherError>;
}
```

### UseCase オーケストレーター

レビューサイクルのオーケストレーター。
ジェネリクスで port を注入する。

```rust
struct ReviewCycle<R, H, D> {
    base: CommitHash,
    scope_config: ReviewScopeConfig,
    reviewer: R,
    diff_getter: D,
    hasher: H,
}

impl<R: Reviewer, H: ReviewHasher, D: DiffGetter> ReviewCycle<R, H, D> {
    /// レビューサイクルを構築
    pub fn new(
        base: CommitHash,
        scope_config: ReviewScopeConfig,
        reviewer: R,
        diff_getter: D,
        hasher: H,
    ) -> Self;

    /// 指定スコープをレビュー（final）
    pub fn review(
        &self,
        scope: &ScopeName,
    ) -> Result<ReviewOutcome<Verdict>, ReviewCycleError>;

    /// 指定スコープを fast レビュー（参考値。approved 判定には使わない）
    pub fn fast_review(
        &self,
        scope: &ScopeName,
    ) -> Result<ReviewOutcome<FastVerdict>, ReviewCycleError>;

    /// diff からスコープ別のレビュー対象を取得
    pub fn get_review_targets(
        &self,
    ) -> Result<HashMap<ScopeName, ReviewTarget>, ReviewCycleError>;

    /// 全スコープのレビュー状態を取得
    pub fn get_review_states(
        &self,
        reader: &impl ReviewReader,
    ) -> Result<HashMap<ScopeName, ReviewState>, ReviewCycleError>;
}
```

#### review() の内部フロー

```rust
fn review(&self, scope: &ScopeName) -> Result<ReviewOutcome<Verdict>, ReviewCycleError> {
    if !self.scope_config.contains_scope(scope) {
        return Err(ReviewCycleError::UnknownScope(scope.clone()));
    }
    let files = self.diff_getter.list_diff_files(&self.base)?;
    let classified = self.scope_config.classify(&files);
    let review_target = ReviewTarget::new(classified.get(scope).cloned().unwrap_or_default());
    let hash_before = self.hasher.calc(&review_target)?;
    if hash_before.is_empty() {
        return Ok(ReviewOutcome::Skipped);
    }
    let (verdict, log_info) = self.reviewer.review(&review_target)?;
    let files_after = self.diff_getter.list_diff_files(&self.base)?;
    let classified_after = self.scope_config.classify(&files_after);
    let review_target_after = ReviewTarget::new(classified_after.get(scope).cloned().unwrap_or_default());
    let hash_after = self.hasher.calc(&review_target_after)?;
    if hash_before != hash_after {
        return Err(ReviewCycleError::FileChangedDuringReview);
    }
    Ok(ReviewOutcome::Reviewed { verdict, log_info, hash: hash_after })
}
```

呼び出し側（usecase or CLI）で `review_writer.write_verdict()` を呼んで永続化する。

#### approved 判定ロジック

各スコープについて:
1. `review.json` から最新の `type: "final"` エントリを取得
2. なければ → `Required(NotStarted)`
3. verdict が `findings_remain` → `Required(FindingsRemain)`
4. verdict が `zero_findings` だがハッシュが現在と不一致 → `Required(StaleHash)`
5. verdict が `zero_findings` かつハッシュが一致 → `NotRequired(ZeroFindings)`
6. スコープに対象ファイルがない → `NotRequired(Empty)`

全スコープが `NotRequired` なら approved。

**base 進行時のリセットは不要:**
コミットで `.commit_hash` が進むと diff のファイルセットが変わり、ハッシュも変わるため、
古い verdict は自動的に `StaleHash` になる。同じファイルが同じ内容で diff に残る場合は
レビュー済みの内容と同一なので verdict の再利用は正しい動作。
コミット時に `review.json` のリセットや `.commit_hash` のクリアは不要。

**ポリシー変更時の approval 無効化は行わない (accepted risk):**
review-scope.json や `.claude/rules/**` が変更された場合、それらのファイルは
`harness-policy` スコープに属するため、そのスコープ自体は `StaleHash` になりレビュー対象になる。
ただし他のスコープ（domain 等）の既存 approval は無効化されない。
v1 では PolicyChanged で全サイクルを無効化していたが、これが全グループ再レビューの
主要因の一つだったため、v2 では意図的に採用しない。
ポリシー変更後に全スコープの再レビューが必要な場合は `reset()` を手動で呼ぶ。

**init と reset の使い分け:**

| 操作 | review.json | .commit_hash | 用途 |
|------|-------------|--------------|------|
| `init()` + `clear()` | 新規作成 | 削除 | トラック初期化。diff base を main に戻す |
| `reset()` | アーカイブ → 新規作成 | 維持 | レビュー状態のリスタート。diff base は維持しインクリメンタルスコープで再レビュー |
| コミット後 | 変更なし | 新 HEAD SHA を書き込み | 通常のコミットフロー |

#### get_review_states() の内部フロー

```rust
fn get_review_states(
    &self,
    writer: &impl ReviewWriter,
) -> Result<HashMap<ScopeName, ReviewState>, ReviewCycleError> {
    // 1. 現在の diff からスコープ別ハッシュを計算
    let files = self.diff_getter.list_diff_files(&self.base)?;
    let classified = self.scope_config.classify(&files);
    let mut current_hashes: HashMap<ScopeName, ReviewHash> = HashMap::new();
    for (scope, scope_files) in &classified {
        let target = ReviewTarget::new(scope_files.clone());
        current_hashes.insert(scope.clone(), self.hasher.calc(&target)?);
    }

    // 2. 永続化された最新 final verdict を読み込み
    let latest_finals = reader.read_latest_finals()?;

    // 3. configured-but-empty スコープも含めて全スコープの状態を判定
    let mut states = HashMap::new();

    // 3a. diff にファイルがあるスコープ
    for (scope, current_hash) in &current_hashes {
        let state = match current_hash {
            ReviewHash::Empty => ReviewState::NotRequired(NotRequiredType::Empty),
            ReviewHash::Some(_) => match latest_finals.get(scope) {
                None => ReviewState::Required(RequiredType::NotStarted),
                Some((Verdict::FindingsRemain(_), _)) => {
                    ReviewState::Required(RequiredType::FindingsRemain)
                }
                Some((Verdict::ZeroFindings, stored_hash)) => {
                    if stored_hash == current_hash {
                        ReviewState::NotRequired(NotRequiredType::ZeroFindings)
                    } else {
                        ReviewState::Required(RequiredType::StaleHash)
                    }
                }
            },
        };
        states.insert(scope.clone(), state);
    }

    // 3b. config に定義されているが diff にファイルがないスコープ → Empty
    for scope in self.scope_config.all_scope_names() {
        states.entry(scope).or_insert(ReviewState::NotRequired(NotRequiredType::Empty));
    }

    Ok(states)
}
```

### ユースケースフロー

#### 1. 修正ファイル名一覧 → レビュー対象スコープ一覧

```rust
let targets = review_cycle.get_review_targets()?;
// targets: HashMap<ScopeName, ReviewTarget>
```

変更されたファイルがどのスコープに属するかを判定。
レビューが必要なスコープを特定する。

#### 2. スコープ指定してレビュー → verdict 取得

```rust
// final レビュー
match review_cycle.review(&scope)? {
    ReviewOutcome::Reviewed { verdict, log_info, hash } => {
        review_writer.write_verdict(&scope, &verdict, &hash)?;
    }
    ReviewOutcome::Skipped => { /* 空スコープ — 何もしない */ }
}

// fast レビュー（参考値）
match review_cycle.fast_review(&scope)? {
    ReviewOutcome::Reviewed { verdict, log_info, hash } => {
        review_writer.write_fast_verdict(&scope, &verdict, &hash)?;
    }
    ReviewOutcome::Skipped => { /* 空スコープ */ }
}
```

fast は参考値として記録されるが、approved 判定には使わない。

#### 3. check-approved

```rust
let states = review_cycle.get_review_states(&review_reader)?;
let approved = states.values().all(|s| matches!(s, ReviewState::NotRequired(_)));
```

全スコープが `NotRequired` であれば approved。

#### 4. review 状態確認

```rust
let states = review_cycle.get_review_states(&review_reader)?;
for (scope, state) in &states {
    println!("{scope}: {state:?}");
}
```

#### 5. コミット

```rust
// track-commit-message 内で CI → check-approved → commit を atomic に実行
cargo make track-commit-message
// コミット成功後
commit_hash_writer.write(&new_commit_hash)?;
```

**TOCTOU 対策と accepted risk:**
`track-commit-message` ワークフロー内で CI → check-approved → commit が連続実行される。
check-approved とコミットの間に別プロセスがファイルを変える理論的可能性はあるが、
single-user 開発環境を前提とし、この TOCTOU は accepted risk とする。
完全に解決するには git index からハッシュ計算する設計変更が必要であり、
v2 の設計目標（シンプルさ）とトレードオフになるため採用しない。

**worktree / index 同期の前提条件:** `ReviewHasher` は worktree からハッシュを計算するが、
`git commit` は index（staging area）をコミットする。partial staging により両者が
乖離するとレビュー済みでないコードがコミットされる。
コミット前に `add-all`（worktree 全体を staging）を実行して worktree = index を保証する。
`add-all` を経由しない直接の `git commit` は禁止する
（既存の hook `block-direct-git-ops` で強制）。

**review.json の並行書き込み保護:**
複数スコープのレビューが並列実行される場合、review.json への書き込みが競合する。
infra 実装 (`FsReviewWriter`) は `fs4::lock_exclusive()` によるファイルロックで
read-modify-write を直列化する（既存の `FsTrackStore` と同じパターン）。
trait 契約としては「write_verdict / write_fast_verdict は排他的に実行される」ことを前提とする。

### 永続化: review.json v2

```json
{
  "schema_version": 2,
  "scopes": {
    "domain": {
      "rounds": [
        { "type": "fast", "verdict": "zero_findings", "findings": [], "hash": "rvw1:sha256:abc...", "at": "2026-04-04T10:00:00Z" },
        { "type": "final", "verdict": "zero_findings", "findings": [], "hash": "rvw1:sha256:abc...", "at": "2026-04-04T10:05:00Z" }
      ]
    },
    "infrastructure": {
      "rounds": [
        { "type": "fast", "verdict": "findings_remain", "findings": [{ "message": "...", "severity": "P1", "file": "src/lib.rs", "line": 42, "category": "correctness" }], "hash": "rvw1:sha256:def...", "at": "2026-04-04T10:00:00Z" },
        { "type": "final", "verdict": "zero_findings", "findings": [], "hash": "rvw1:sha256:ghi...", "at": "2026-04-04T10:15:00Z" }
      ]
    },
    "other": {
      "rounds": []
    }
  }
}
```

### diff base の管理: .commit_hash

`track/items/<track-id>/.commit_hash`（gitignored）にコミットハッシュを保存。

#### 書き込み仕様

- コミット成功直後に CLI が `git rev-parse HEAD` で新コミットの SHA を取得し、`CommitHashWriter.write()` を呼ぶ
- 書き込む値は常にコミット直後の HEAD SHA（任意の ancestor ではない）
- atomic write（一時ファイル → rename）で crash safety を保証

#### 読み込み仕様

trait 契約（domain port）:
- `CommitHashReader.read()` は `Result<Option<CommitHash>, CommitHashError>` を返す
- ファイルが存在しない → `None`
- ファイルの内容が有効なコミットハッシュ（hex SHA）でない → エラー

infra 実装 (`FsCommitHashReader`) の追加責務:
- 読み込み後に `git merge-base --is-ancestor <hash> HEAD` で ancestry 検証
- 検証失敗（rebase 等） → `None` を返す（fail-closed でスコープ拡大）
- ancestry 検証は infra 実装の内部詳細であり、trait 契約には含まない

#### フォールバック

- `None` の場合、CLI の composition root が `git rev-parse main` で
  main の HEAD SHA を解決し `CommitHash` として使用
- `CommitHash` は常に有効な hex SHA。`"main"` のようなブランチ名は
  CLI で SHA に解決してから渡す。domain/usecase 層にブランチ名は入らない

#### gitignore / ブランチ切替

- `.commit_hash` は gitignored。ローカル状態のみ
- ブランチ切替/clone: ファイルが存在しない → フォールバック（正しい動作）

#### ReviewCycle での使用

```rust
// CLI (composition root)
let commit_hash_reader = FsCommitHashReader::new(&track_items_dir, &track_id);
let base: CommitHash = match commit_hash_reader.read()? {
    Some(hash) => hash,
    None => {
        // main の最新コミット SHA を解決
        let main_sha = git.resolve_ref("main")?;
        CommitHash::try_new(main_sha)?
    }
};
let review_cycle = ReviewCycle::new(base, scope_config, reviewer, diff_getter, hasher);
```

### v1 → v2 マイグレーション

- v1 の review.json (`schema_version: 1`) は無視（デコーダが空として扱う）
- 新トラックは v2 で開始（`review_writer.init()` + `commit_hash_writer.clear()`）
- 既存トラックは `review_writer.init()` + `commit_hash_writer.clear()` で v2 に移行
  （v1 の review.json は init 時に上書きされる。.commit_hash は clear で削除され、
  diff base が main にフォールバックするため全変更が再レビュー対象になる）
- v1 の `approved_head`（review.json 内）は `.commit_hash` ファイルに置き換え

### 廃止される v1 の概念

| v1 の概念 | 廃止理由 |
|-----------|---------|
| `CycleGroupState` / frozen scope | スコープ不整合の根本原因 |
| `ReviewCycle` (domain 層) | usecase 層の `ReviewCycle<R,H,D>` に置換 |
| `has_scope_drift` | frozen scope がなくなるため不要 |
| `reclassified_paths_outside_cycle_groups` | 同上 |
| `check_cycle_staleness_any` | `ReviewState` enum で代替 |
| `ReviewPartitionSnapshot` | 毎回 diff から計算するため不要 |
| `RecordRoundProtocol` | `ReviewCycle.review()` + `ReviewWriter` に分解 |
| `effective_diff_base` | `.commit_hash` + `CommitHashReader` で直接管理 |
| `GroupPartition` | `ReviewScopeConfig.classify()` で代替 |
| `DiffScope` | `DiffGetter.list_diff_files()` の戻り値 `Vec<FilePath>` で代替 |

### 現行設計 (v1) との根本的な違い

| 項目 | 現行 (v1) | v2 |
|------|-----------|------|
| スコープ保持 | frozen scope (`Vec<String>`) をサイクル作成時に凍結 | 毎回 diff から動的に計算 |
| ハッシュ計算タイミング | `record_round` と `check_approved` で異なるスコープを使用 | 常に同じ `classify()` + `calc()` で統一 |
| スコープ間の依存 | `has_scope_drift` がサイクル全体を無効化 | スコープ単位で独立。他スコープに影響なし |
| ファイル変更検出 | record 時のハッシュ vs check 時のハッシュ比較 | before/after hash でレビュー中の変更を即座に検出 |
| diff base 管理 | `approved_head` がサイクル作成時に無視される | `.commit_hash` ファイルで明示的管理。常にインクリメンタル |
| fast/final 関係 | fast zero_findings が final の前提条件 | fast は参考値。approved 判定は最新 final のみ |
| 状態遷移 | 複数の staleness reason + approval check | `ReviewState` enum でシンプルに表現 |
| PartitionChanged | 1 ファイルの追加で全グループ再レビュー | 変更があったスコープだけ `StaleHash` |
| review.json のスコープ混入 | frozen scope に含まれてハッシュ循環 | `ReviewScopeConfig` の operational 除外で解決 |
| 永続化 | サイクル全体の rounds 履歴 | スコープ毎の rounds 履歴（findings 含む） |
| レイヤー配置 | domain に ReviewCycle + usecase に RecordRoundProtocol | domain に純粋型 + port、usecase に ReviewCycle |
| glob 重複 | `OverlappingGroups` エラーで fail-closed | 両方のスコープに含める（独立レビューなので問題なし） |

### ReviewScopeConfig の設計判断

`ReviewScopeConfig` は domain 層に配置するが、提供するのは純粋な分類ロジックのみ。

v1 では `get_review_target()` や `calc_all_hash()` のように `DiffGetter` / `ReviewHasher` を
引数に取るメソッドが scope config に含まれていたが、これは domain 層の純粋性に反する。
v2 では diff 取得・ハッシュ計算のオーケストレーションを usecase 層の `ReviewCycle` に移動し、
`ReviewScopeConfig` は `classify()`, `get_scope_names()`, `contains_scope()` の 3 メソッドのみ提供する。

## Rejected Alternatives

- **v1 の部分修正（normalize + has_scope_drift グループ単位化）**: frozen scope / current partition / check_approved の 3 者のスコープ不整合は構造的問題であり、部分修正では根本解決できない。RVW-37〜RVW-57 の 20 件以上のパッチが継ぎ足しで追加された結果、複雑さが制御不能になった。
- **frozen scope を approved_head で再計算**: サイクル作成時に `effective_diff_base` を使う案。`record_round` との整合性は改善するが、frozen scope の概念自体が不整合の原因なので根本解決にならない。
- **frozen scope を globset で保持**: ファイルリストではなく glob パターンで保持する案。動的計算に近づくが、frozen scope + current partition の二重管理が残る。

## Consequences

- Good: frozen scope 廃止により、スコープ計算が一箇所に統一され不整合が構造的に不可能になる
- Good: スコープ独立により、1 ファイル追加で全グループ再レビューが不要になる
- Good: `.commit_hash` による明示的な diff base 管理で、インクリメンタルレビューが正しく動作する
- Good: `ReviewState` enum で状態がシンプルに表現され、staleness reason の組み合わせ爆発がなくなる
- Bad: v1 のレビューコード（domain/usecase/infra/CLI）を大幅に書き換える必要がある
- Bad: v1 → v2 マイグレーション中、既存トラックのレビュー状態がリセットされる

## Reassess When

- レビューシステムの要件が変わり、スコープ間の依存関係（「A スコープが approved でないと B を review できない」等）が必要になった場合
- 外部レビュワーが独自のスコープ管理機能を持つようになった場合
