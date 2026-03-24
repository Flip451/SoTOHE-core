<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# CC-SDD-02 明示的承認ゲート（spec.json approved_at + 自動降格）

spec.json の status フィールドを String から SpecStatus enum (Draft/Approved) に型昇格する。
approved_at タイムスタンプと content_hash によるコンテンツ変更検出で、仕様変更時に自動降格する仕組みを実装する。
sotp spec approve CLI コマンドで明示的な承認ゲートを提供する。

## Domain: SpecStatus enum + 承認/降格ロジック

SpecStatus enum (Draft/Approved) を導入し status: String を型昇格。
approved_at: Option<Timestamp> と content_hash: Option<String> を SpecDocument に追加。
approve() / is_approval_valid() / effective_status() メソッドを実装。

- [ ] domain: SpecStatus enum + approved_at + content_hash + approve/integrity メソッド + テスト

## Infrastructure: codec + render 変更

codec.rs: SpecStatus + approved_at + content_hash の serialize/deserialize。
content hash 計算ロジック (sha2)。auto-demote on decode。
render.rs: ステータスバッジと approved_at の表示。

- [ ] infra: codec (serialize/deserialize + auto-demote + hash 計算) + テスト
- [ ] infra: render (ステータスバッジ + approved_at 表示) + テスト

## CLI: sotp spec approve コマンド

sotp spec approve <track-dir> CLI コマンドを追加。
spec.json を読み込み、content hash 計算、status=approved に更新して書き戻す。

- [ ] CLI: sotp spec approve コマンド

## Makefile + /track:plan skill 更新

Makefile.toml に cargo make spec-approve / track-record-round / track-check-approved ラッパーを追加。
permissions.allow に新ラッパーを登録。
/track:plan skill の spec.json 生成後に承認フローを案内。

- [ ] Makefile (spec-approve + track-record-round + track-check-approved ラッパー) + permissions.allow 登録 + /track:plan skill 更新

## ドキュメント + 統合テスト

DESIGN.md, TRACK_TRACEABILITY.md を更新。
統合テストで承認→変更→自動降格の end-to-end フローを確認。

- [ ] ドキュメント更新 (DESIGN.md, TRACK_TRACEABILITY.md) + 統合テスト
