# Observations — tddd-v2-2026-05-08

## AC-12 統合確認 (T028, 2026-05-13)

v3-native catalogue-gate 移行完了後の end-to-end 検証結果。

### AC-12.1 — `catalogue_codec` / `TypeCatalogueDocument` / `v3_doc_to_stub` 参照ゼロ

- 確認コマンド: `grep -rn "catalogue_codec\|TypeCatalogueDocument\|v3_doc_to_stub" libs/ apps/ --include=*.rs`
- 結果: コンパイル関連の参照ゼロ。残っていた doc コメント内の言及（`catalogue_codec` / `v3_stub` / `v3_doc_to_stub` / `TypeCatalogueDocument` への参照、`// T008: ... 削除` 形式の墓標コメント）はすべて現状記述に書き換え済み。
- 付随削除: `libs/domain/src/tddd/signals.rs`（T008 以降 `mod` 宣言が外れたまま残っていた約 3,700 行の孤立 dead ファイル。削除済みの v2 カタログ型 `TypeCatalogueEntry` / `TypeDefinitionKind` / `TypeAction` / `TraitImplDecl` / `TypestateTransitionsSpec` を import していたため AC-12.1 の grep で検出された）を削除。コンパイル対象外だったため挙動に影響なし（gate-safe）。

### AC-12.2 — `check_type_signals` の純粋関数化

- `domain::tddd::consistency::check_type_signals(&TypeSignalsDocument, bool) -> VerifyOutcome` — カタログ非依存の純粋関数（`libs/domain/src/tddd/consistency.rs`）。T022 で `(&TypeCatalogueDocument, bool, &str)` から切り替え済み。`declaration_hash` 鮮度チェックは呼び出し元（`verify_from_spec_json` / `check_strict_merge_gate`）側に移動し、カタログはバイト列としてのみ読む形になった。

### AC-12.3 — コミットゲート / マージゲートのフェイルクローズド動作

- コミットゲート（`verify spec-states-current` 経由 = `infrastructure::verify::spec_states::verify_from_spec_json`）とマージゲート（`usecase::merge_gate::check_strict_merge_gate`）の各フェイルクローズド分岐（catalogue 不在 → BLOCKED / `declaration_hash` 不一致 → BLOCKED / 非 v3 catalogue → BLOCKED / 🔴 → BLOCKED / strict=true かつ 🟡 → BLOCKED）はユニットテストでカバー済み。
- ドッグフーディング: T021–T027 の各コミットで `cargo make track-commit-message` のコミットゲートパイプライン（type-signals 再生成 → Red/Yellow チェック → catalogue-spec-refs 検証 → catalogue-spec-signals 再生成 → sync-views → `cargo make ci` → review approval チェック → commit）が v3 catalogue の本トラックに対して正しく評価・通過した。非 v3 catalogue は `CatalogueDocumentCodec::decode` の fail-closed エラーで自動的にブロックされる（CN-11）。

### AC-12.4 — `cargo make track-pr-merge` の正常動作

- **本トラックでの live 検証は M10（`/track:pr-review` 通過後）の実マージ操作に委ねる** — ユーザー指示により「明示的に指示されるまでマージしない」運用のため、`cargo make track-pr-merge` の実行はマージ承認後に行う。マージゲート（`check_strict_merge_gate`）のロジック自体は AC-12.3 のユニットテスト + T021–T027 のコミットゲート通過実績で検証済み。実マージ時にゲートが v3 catalogue を正しく評価してマージが完了することで AC-12.4 が充足される。

### AC-12.5 — `cargo make ci` の通過

- `cargo make ci`（fmt-check + clippy `-D warnings` + nextest + deny + check-layers + verify-*）が pass（3,000+ テスト緑、`verify-plan-progress` / `verify-catalogue-spec-refs` / `check-catalogue-spec-signals` を含む全ゲート通過）。
