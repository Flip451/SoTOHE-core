# Verification: spec-code-consistency-2026-04-08

## Scope Verified

- [ ] ADR 2026-04-08-0045 の Decision 1-4 がタスクでカバーされている (Decision 5 は out_of_scope として明示的に先送り済み)
- [ ] 既存テスト (domain_types, code_profile_builder, schema_export) が TypeGraph リネーム後も通る
- [ ] done/archived トラックの views が保護されている

## Manual Verification Steps

- [ ] T001: done トラックの spec.md/plan.md/domain-types.md が `cargo make track-sync-views` (bulk パス) 後に変更されていないことを `git diff` で確認 (archived トラックについても同様に確認)
- [ ] T001: single-track パス (sync_rendered_views に track_id 指定) でも done および archived トラックの views が上書きされないことをユニットテストで確認
- [ ] T001: `cargo make track-sync-views` 後に `registry.md` が更新されていること (アクティブトラックの情報が反映されていること) を `git diff track/registry.md` で確認
- [ ] T001: アクティブトラック (planned/in_progress) の spec.md/plan.md/domain-types.md が `cargo make track-sync-views` 後に正しく再生成されていることを確認
- [ ] T002: `cargo make export-schema -- --crate domain --pretty` の出力で generic wrapper (`Vec<T>`, `HashMap<K,V>`, `Box<T>`, `Arc<T>` 等) の型引数が return_type_names に含まれていないことを確認
- [ ] T002: `Result<T, E>` の T が return_type_names に含まれること (Result は展開される) を `cargo make export-schema` の出力またはユニットテストで確認
- [ ] T002: `Option<T>` の T が return_type_names に含まれること (Option は展開される) を `cargo make export-schema` の出力またはユニットテストで確認
- [ ] T002: `&T` (BorrowedRef) が引き続き inner の T を展開すること、`(A, B)` (Tuple) が展開されないことをユニットテストで確認
- [ ] T003: `CodeProfile` への参照が Rust ソースコードに残っていないことを `grep -r CodeProfile libs/ apps/` で確認 (knowledge/ や track/ などのドキュメントは除外する)
- [ ] T003: `TypeGraph.outgoing` がテスト用の型セット (一部のみが typestate として宣言されたもの) で typestate 型のみを含み、通常の戻り値型 (Vec や String 等) を含まないことをユニットテストで確認
- [ ] T004: 同名型がある場合に warning が出ることを確認
- [ ] T004: `TypeInfo.module_path` が rustdoc JSON から抽出され、`build_type_graph` によって `TypeNode.module_path` に正しく伝播されることをユニットテストで確認
- [ ] T005: `sotp verify spec-code-consistency --track-id <id> --crate <name>` がテスト用 crate に対し `undeclared_types` と `undeclared_traits` の両方を正しく報告することを確認
- [ ] T006: `sotp verify spec-code-consistency --track-id <id> --crate <name>` が `ConsistencyReport` JSON を stdout に出力することを確認 (forward_signals / undeclared_types / undeclared_traits フィールドの存在を検証)
- [ ] T006: `ConsistencyReport.forward_signals` の per-entry signal 結果 (type_name, kind_tag, signal, found_items, missing_items, extra_items) がユニットテストで検証されていること (evaluate_domain_type_signals の結果が正しく forward_signals に設定されることの保証)
- [ ] T006: 不正な track-id や読み取り不能な domain-types.json を渡した場合にゼロ以外の終了コードと分かりやすいエラーメッセージが返されることを確認
- [ ] 全タスク完了後: `cargo make ci` が通ることを確認 (fmt-check + clippy + test + deny + check-layers の全パス)

## Result / Open Issues

- (実装後に記録)

## Verified At

- (検証完了時に記録)
