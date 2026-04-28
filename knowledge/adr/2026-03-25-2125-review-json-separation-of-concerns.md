---
adr_id: 2026-03-25-2125-review-json-separation-of-concerns
decisions:
  - id: 2026-03-25-2125-review-json-separation-of-concerns_grandfathered
    status: accepted
    grandfathered: true
---
# FsTrackStore + review.json: 関心事の分離

## ステータス

Superseded by 2026-03-29-0947-review-json-per-group-review-state.md（ADR-2026-03-24-1200 の実装方針を置き換え → さらに 2026-03-29-0947 で per-group review model に発展）

## 背景

トラック `review-json-separation-2026-03-25` で、レビュー状態を metadata.json から
独立した review.json に切り出した。初期実装では `track.review()` が review.json を
裏側で自動的に読み込むよう、`FsTrackStore::read_track()` にマイグレーション・
自動読み込み・セキュリティ用のクリア処理を詰め込んだ。
その結果、つぎはぎ的な結合が生まれ、14回のレビューラウンドを経ても通過できなかった：

- ~~マイグレーションのタイミング衝突（read_track がセキュリティのためにレガシーレビューを
  消すが、write パスではマイグレーションのために必要）~~
- ロック管理の不整合（finalize_review は単純な load/save、RecordRoundProtocol は
  別のロックを使用）
- エラー処理方針の不統一（パスによって「できる限り続行」と「即座に失敗」が混在）
- review.json ハッシュの寿命問題（コードを直すたびに全レビューグループが無効化される）

## 決定

`metadata.json` と `review.json` を一つの論理ドキュメントとして扱うのをやめる。

1. `FsTrackStore::read_track()` は副作用なし・メタデータのみにする。
2. `TrackReader::find()` は review.json の内容を `track.review()` に詰め込まない。
3. レビュー状態は専用の `ReviewReader` / `ReviewWriter` ドメインポート経由でアクセスする。
4. `FsTrackStore` は `FsReviewStore` に委譲してレビューポートを実装する。
5. `FsTrackStore` の write パスではレビューの変更検知や無効化を行わない。
6. 無効化は、正規化ハッシュを持つレビューロック操作の中でだけ行う
   （`check_approved`、`record_round`、`resolve_escalation`）。

### read_track

review をクリアしたトラックとドキュメントメタを含むプライベートな
`TrackReadSnapshot` を返す。~~旧データからのマイグレーションは不要（2026-03-26 決定）~~

### Write パス（save/update/with_locked_document）

1. ~~旧レビューデータを review.json に移行（不在の場合）してからメタデータから除去~~ （不要: 2026-03-26 決定）
2. metadata.json を `review: None` で書き込む
3. レビュー状態の無効化は一切しない

### ポートの配置

`ReviewReader` と `ReviewWriter` を `libs/domain/src/repository.rs`（ドメイン永続化層）に配置。

### 無効化の方針

レビューロック操作だけが無効化できる。即座に失敗: 無効化の保存に失敗したら操作全体を失敗させる。

### 即座に失敗ルール

1. 無効化の保存失敗 → 操作失敗
2. ロック取得失敗 → 操作失敗
3. review.json が存在しない → NotStarted (Ok(None))。読み込み・パース・ロックのエラーを NotStarted に丸めない。

### ロック順序

リポジトリ全体の git ロック > metadata ロック > review ロック。

### ~~マイグレーション方針~~

~~明示的な write のタイミングでだけマイグレーション。read 時には行わない。~~ （不要: 2026-03-26 決定）

旧データのマイグレーションは不要。review.json は新規トラックでのみ作成する。
review.json がない旧トラックは ReviewReader が NotStarted (Ok(None)) として扱う。

## Canonical Blocks

```rust
use domain::ReviewState;

struct TrackReadSnapshot {
    track: TrackMetadata,
    meta: DocumentMeta,
}
```

```rust
pub trait ReviewReader: Send + Sync {
    fn find(&self, id: &TrackId) -> Result<Option<ReviewState>, TrackReadError>;
}

pub trait ReviewWriter: Send + Sync {
    fn save(&self, id: &TrackId, review: &ReviewState) -> Result<(), TrackWriteError>;

    fn with_locked_review<F, T>(&self, id: &TrackId, f: F) -> Result<T, TrackWriteError>
    where
        F: FnOnce(&mut ReviewState) -> Result<T, TrackWriteError>;
}
```

```rust
impl FsTrackStore {
    fn read_track(&self, id: &TrackId) -> Result<Option<TrackReadSnapshot>, RepositoryError> {
        // Side-effect-free: reads metadata.json only, review cleared
        todo!()
    }
}
```

```rust
pub fn check_approved(
    input: CheckApprovedInput,
    tracks: &impl TrackReader,
    review_reader: &impl ReviewReader,
    review_writer: &impl ReviewWriter,
    hasher: &impl GitHasher,
) -> Result<(), String> {
    // Orchestrates: hash → read review → check_commit_ready → persist invalidation
    todo!()
}
```

```rust
pub fn resolve_escalation(
    input: ResolveEscalationInput,
    tracks: &impl TrackReader,
    reviews: &impl ReviewWriter,
) -> Result<String, String> {
    // Validates evidence → clears escalation → invalidates review
    todo!()
}
```

## 影響

- `track.review()` はデコード互換のためだけに残る非推奨フィールドになる
- レビューを扱う全ての呼び出し元が `ReviewReader`/`ReviewWriter` ポートを受け取る必要がある
- CLI のコンポジションルートで `FsTrackStore` を作り、`TrackReader`/`TrackWriter` と
  `ReviewReader`/`ReviewWriter` の両方として渡す
- ~~マイグレーションはトラックごとに1回限り、write 操作でトリガーされる~~ （不要: 2026-03-26 決定）

## 出典

Codex planner (gpt-5.4) により設計、2026-03-25。完全な設計文書:
`tmp/review-json-redesign-design-2026-03-25.md`
