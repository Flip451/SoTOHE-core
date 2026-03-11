# Spec: Track Store with TrackReader/TrackWriter Ports

## Goal

metadata.json の並行書き込み破損を防止する。既存の `TrackRepository` (find/save) パターンをISP に基づき `TrackReader` / `TrackWriter` に分離し、`TrackWriter::update` でロック付き read-modify-write をポートレベルで表現する。

## Scope

### In scope

- **Domain layer** (`libs/domain/src/`):
  - `error.rs`: `TrackReadError` / `TrackWriteError` 型付きポートエラー（domain 所有、DIP）
  - `repository.rs`: `TrackRepository` → `TrackReader` + `TrackWriter` に分離（ISP）
  - `TrackWriter::update<F>` でアトミック変更をポート契約として定義（non-object-safe、許容）
  - `track.rs`: `TrackStatus::Archived` バリアント追加（Python スキーマ互換）
- **Infrastructure layer** (`libs/infrastructure/src/track/`):
  - `codec.rs`: `TrackDocumentV2` serde 型（Python `track_schema.py` との互換性維持）
  - `atomic_write.rs`: `atomic_write_file()` — tmp + fsync + rename + parent fsync
  - `fs_store.rs`: `FsTrackStore` — `TrackReader` + `TrackWriter` 実装、`FileLockManager` で排他制御
- **UseCase layer** (`libs/usecase/src/`):
  - `SaveTrackUseCase` → `TrackWriter::save` を使用
  - `LoadTrackUseCase` → `TrackReader::find` を使用
  - `TransitionTaskUseCase` → `TrackWriter::update` に移行（find/save レース解消）
  - `InMemoryTrackRepository` → `InMemoryTrackStore`（`TrackReader` + `TrackWriter` 実装、テスト用）
- **CLI layer** (`apps/cli/src/`):
  - `FsTrackStore` をコンポジションルートに配線
- **Python scripts** (`scripts/`):
  - `track_state_machine.py` の metadata.json 書き込みを `sotp track` コマンドに委譲

### Out of scope

- plan.md / registry.md の Rust レンダリング（advisory 扱い、フェーズ1では Python のまま）
- external_guides.json のロック（atomic-write-standard トラックで対応）

## Constraints

- `TrackDocumentV2` の JSON スキーマは Python `track_schema.py` と完全互換を維持
- `FsTrackStore` は既存の `FileLockManager` (fd-lock ベース) を再利用
- アトミック rename は同一ファイルシステムを前提（tmp file を対象ディレクトリ内に作成）
- `chrono` を使用（tech-stack.md に準拠）、`time` クレートは採用しない
- `TrackWriter::update` のクロージャ内で I/O を行わない（domain 層の純粋性維持）
- `DomainError` から `Repository` バリアントを削除（`DomainError` は `Validation` + `Transition` のみ）
- Repository エラーは `TrackReadError::Repository` / `TrackWriteError::Repository` でのみ伝播
- UseCase の戻り値は `TrackReadError` / `TrackWriteError`（`DomainError` ではない）

## Acceptance Criteria

1. `TrackReader` と `TrackWriter` が別 trait として定義されている（`TrackReadError`/`TrackWriteError` 使用）
2. `TrackWriter::update<F>` が排他ロック下でアトミック read-modify-write を実行（non-object-safe 許容）
3. `FsTrackStore` が `FileLockManager` で metadata.json をロック
4. `atomic_write_file` が tmp + rename パターンを使用
5. `TransitionTaskUseCase` が `TrackWriter::update` を使用（find/save 不使用）
6a. `SaveTrackUseCase` が `TrackWriter::save` を使用
6b. `LoadTrackUseCase` が `TrackReader::find` を使用
6c. 旧 `TrackRepository` trait が完全削除（deprecation 不要 — 内部 API）
7. `DomainError::Repository` バリアント削除済み
8. UseCase の戻り値が `TrackReadError` / `TrackWriteError` に移行済み
9. Python → Rust のスキーマ互換性テストが通過
10. 並行 write シミュレーションテストが通過
11. `cargo make ci` が全チェック通過

## Resolves

- TODO CON-01: metadata.json の排他制御不在
- TODO INF-04: クロスプラットフォーム設計の破綻 — fcntl

## Related Conventions (Required Reading)

- `project-docs/conventions/security.md`
