<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 27, yellow: 2, red: 0 }
---

# 3-12 spec ↔ code 整合性チェック — TypeGraph + 逆方向検出

## Goal

spec の domain-types.json と code の domain 型の双方向整合性を検証する仕組みを構築する。
CodeProfile を TypeGraph に拡張し、型間の遷移関係をグラフとして表現する。
逆方向チェック (code → spec) を新設し、spec に書き忘れた型を CI で検出可能にする。
track-sync-views の done/archived トラック views 消失バグを修正する。

## Scope

### In Scope
- sync_rendered_views() が done/archived トラックの views を再生成しないよう制限する [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Critical Bug] [tasks: T001]
- collect_type_names を Result/Option のみ展開に制限し、Vec<T> 等の false positive を排除する。BorrowedRef は引き続き inner を展開し、Tuple 展開は削除する [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §課題1] [tasks: T002]
- CodeProfile → TypeGraph にリネーム・拡張し、outgoing (typestate 遷移) フィールドを追加する。typestate フィルタを evaluate_domain_type_signals から build_type_graph (構築時) に移動する [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.2] [tasks: T003]
- TypeInfo と TypeNode に module_path を追加し、同名型衝突を警告する [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.3] [tasks: T004]
- 逆方向チェック: TypeGraph の全ノードのうち domain-types.json に宣言がない型および trait を検出する (undeclared_types + undeclared_traits) [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.4] [tasks: T005]
- ConsistencyReport 型と sotp verify spec-code-consistency CLI コマンドを新設する [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.4] [tasks: T006]

### Out of Scope
- TypeGraph のグラフアルゴリズム (到達可能性、サイクル検出) — petgraph 導入は将来検討 [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Reassess When]
- domain-types.json への module_path 指定フィールド追加 — 衝突が実運用で頻発時に検討 [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Reassess When]
- 逆方向チェックの auto-add (AI が未宣言型を自動追加) — Yellow 再導入は別 track [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Reassess When]
- 同名型の完全な区別 (bare name キーによる masking) — T004 は warning のみ。domain-types.json に module_path 指定フィールドを追加して逆方向チェックの false negative を解消する対応は将来 track で実施 [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.3]

## Constraints
- Rust コードで実装。domain 層に I/O を含めない (hexagonal architecture) [source: convention — knowledge/conventions/hexagonal-architecture.md]
- TDD ワークフローに従う (Red → Green → Refactor) [source: convention — .claude/rules/05-testing.md]
- TypeGraph へのリネームは CodeProfile を参照する全コード (~17%) の更新が必要 [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Consequences]
- rustdoc JSON の nightly 依存は既存と同様。module_path は nightly バージョン間で変わる可能性あり [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Consequences]

## Acceptance Criteria
- [ ] sync_rendered_views が done/archived トラックの spec.md/plan.md/domain-types.md を上書きしないこと (bulk パス track_id=None と single-track パス track_id=Some の両方で保護されること) [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Critical Bug] [tasks: T001]
- [ ] cargo make track-sync-views が registry.md を引き続き再生成すること (done/archived トラックの個別 views のスキップは registry.md の更新を妨げない) [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Critical Bug] [tasks: T001]
- [ ] cargo make track-sync-views がアクティブトラック (planned/in_progress) の spec.md/plan.md/domain-types.md を引き続き再生成すること [source: inference — done/archived スキップがアクティブトラックの再生成を壊さないことの保証] [tasks: T001]
- [ ] Vec<Published>, HashMap<K, Published>, Box<Published>, Arc<Published> 等の generic wrapper が return_type_names に Published を含まないこと (Result<Published, E> は含む) [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.1] [tasks: T002]
- [ ] Option<Published> が return_type_names に Published を含むこと (Option<T> は Result<T,E> と同様に T を展開する) [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.1] [tasks: T002]
- [ ] &T (BorrowedRef) が引き続き inner の T を展開すること、(A, B) (Tuple) の要素が展開されないこと [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.1] [tasks: T002]
- [ ] CodeProfile → TypeGraph のリネームが完了し、既存テストが全て通ること [source: inference — リファクタリング回帰防止] [tasks: T003]
- [ ] TypeGraph.outgoing が typestate 型のみに絞り込まれていること [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.2] [tasks: T003]
- [ ] TypeInfo.module_path が rustdoc JSON から抽出され TypeNode.module_path に伝播されること (schema_export → TypeGraph のデータフローが完結していること) [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.3] [tasks: T004]
- [ ] 同名型が複数のモジュールに存在する場合に tracing::warn が出力されること (衝突検出が実装されていること) [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.3] [tasks: T004]
- [ ] 逆方向チェックで code にあるが domain-types.json にない型および trait が両方検出されること (ConsistencyReport.undeclared_types と ConsistencyReport.undeclared_traits の両フィールドが正しく設定されること) [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.4] [tasks: T005]
- [ ] ConsistencyReport.forward_signals が evaluate_domain_type_signals の結果で正しく設定されること (双方向整合性チェックの前方向が機能していること) [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.4] [tasks: T006]
- [ ] sotp verify spec-code-consistency --track-id <id> --crate <name> が ConsistencyReport を出力すること [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Decision.4] [tasks: T006]
- [ ] sotp verify spec-code-consistency が不正な track-id や読み取り不能な domain-types.json に対してゼロ以外の終了コードと分かりやすいエラーメッセージを返すこと (エラーケースのテストが存在すること) [source: convention — .claude/rules/05-testing.md] [tasks: T006]
- [ ] cargo make ci が通ること [source: convention — .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004, T005, T006]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 27  🟡 2  🔴 0

