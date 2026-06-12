<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 33, yellow: 0, red: 0 }
---

# SoT 本体への参照 hash 埋め込みを廃止し、新鮮度判定を verify-cache の実行時突合に一元化する

## Goal

- [GO-01] spec_refs[].hash フィールドを <layer>-types.json カタログから撤去し、SoT 本体（spec.json / 型カタログ / ADR）がいかなる参照 hash も保持しない構造にする。hash の保管場所を verify-cache（spec-adr-verify-cache.json / <layer>-catalogue-spec-verify-cache.json）のみに限定し、記録を ref-verify run 実行時の自動計算・自動書き込みに一元化する [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1]
- [GO-02] 参照の新鮮度判定を、ゲート判定のたびにチェーン両端ノードの hash を再計算して verify-cache と突合する方式（既存の ref-verify run 差分キャッシュ機構）に一元化する。verify catalogue-spec-refs と verify plan-artifact-refs の SpecRef hash mismatch 検査を撤去し、決定論的な新鮮度検査の必要箇所は ref-verify check-approved 相当の cache 突合で置き換える。dangling anchor 検出（anchor 実在確認）は hash と独立した構造検査として維持する [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D2]
- [GO-03] 整合確認の実体を verify-cache の再レビュー発火（両端 hash 変化 → stale → 意味論レビュー）に一本化する。hash の一致を整合確認の代替として扱う運用・文書記述（spec-element-hash の help テキスト、type-designer agent 定義の転記手順等）を撤去する [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D3]

## Scope

### In Scope
- [IN-01] domain 層の SpecRef 型（libs/domain/src/plan_ref/spec_ref.rs）から hash フィールドを削除し、{ file, anchor } の 2 フィールド構造にする。これにより AdrRef { file, anchor } と同型になり、SoT Chain 全体で参照表現が統一される [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1] [tasks: T001]
- [IN-02] <layer>-types.json カタログの schema_version を更新し、spec_refs[] から hash フィールドを除いた新スキーマを正式とする。既存進行中トラックのカタログを移行するか、または破壊的変更の旨を明示して新規カタログのみ適用する [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1] [tasks: T001]
- [IN-03] verify plan-artifact-refs の SpecRef hash mismatch 検査（libs/infrastructure/src/verify/plan_artifact_refs/mod.rs の hash verification ブロック）を撤去する。anchor 実在確認（dangling anchor 検出）は引き続き実施する [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D2] [tasks: T001]
- [IN-04] verify catalogue-spec-refs の hash mismatch 検査を撤去する。apps/cli/src/commands/verify_catalogue_spec_refs.rs 及びその委譲先の infrastructure 実装から SpecRef.hash 照合ロジックを削除する。anchor 実在確認は引き続き実施する [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D2] [tasks: T001]
- [IN-05] Chain② pair 列挙時（libs/infrastructure/src/ref_verify/pair_source_chain2.rs）の claim_hash は catalogue entry の canonical JSON 全体の SHA-256 として算出する。spec_refs[].hash フィールドが撤去されると、そのフィールドを含まない entry canonical JSON が claim_hash の計算基礎となる。これにより、hash フィールドの同期だけで claim_hash が変化してノイズ再レビューが発火する問題が解消される [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1, knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T001]
- [IN-06] apps/cli/src/commands/track/tddd/spec_element_hash.rs の help テキストおよびドキュメントコメントから「type-designer が出力を転記する」用途を撤去する。spec-element-hash コマンドが hash 転記目的で使われなくなることを反映した説明に更新する [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D3] [tasks: T002]
- [IN-07] .claude/agents/type-designer.md の spec_refs 転記手順（hash フィールドを spec-element-hash コマンドで取得して転記する手順）を撤去する。type-designer の spec_refs 記載は { file, anchor } の 2 フィールドのみとする [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D3] [tasks: T002]
- [IN-08] catalogue infrastructure codec（CatalogueDocumentCodec）を { file, anchor } 構造の SpecRef を正しく encode / decode するように更新する。hash フィールドを含む旧スキーマを読み込んだ場合の挙動（エラーまたは無視）を明示的に定義する [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1] [tasks: T001]

### Out of Scope
- [OS-01] spec_refs[].hash を維持したまま resync コマンドで一括再同期する方式（Rejected Alternative B）。転記の摩擦は消えるが埋め込み宣言値が残るため、Chain② claim_hash に意味なしノイズ再レビューが発生し続ける。撤去すれば原理的に問題が消えるため却下された [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1]
- [OS-02] spec 編集時に下流カタログを自動で書き換える保存フック（Rejected Alternative C）。暗黙の連鎖書き換えは事故時の原因切り分けを難しくし、1 ファイル 1 writer の原則と衝突する。却下された [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1]
- [OS-03] catalogue_declaration_hash（<layer>-catalogue-spec-signals.json 側、機械生成）の変更。このフィールドは hash と無関係で存続する。catalogue-spec-signals の presence 判定（spec_refs の有無による 🔵🟡🔴）も hash と無関係のため影響なし [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1]
- [OS-04] verify-cache artifact（spec-adr-verify-cache.json / <layer>-catalogue-spec-verify-cache.json）のスキーマ変更。これらは機械生成キャッシュであり、claim_hash / evidence_hash の意味論は変わらない（D4 定義通り）。ただし Chain② claim_hash の計算基礎が「hash フィールドを含まない entry canonical JSON」へ変わることで hash 値が変わり、既存キャッシュエントリは stale として再レビューが発火する [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1, knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4]
- [OS-05] 意味論レビューゲートの設計変更（Chain① / Chain② の ref-verifier capability、prompt template、段階引き上げロジック）。意味論レビューの仕組みは変えず、hash フィールド撤去の副作用として既存キャッシュが stale 扱いになって再レビューが発火することを受け入れる [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D1]
- [OS-06] verify-cache の verdict（Pass/Fail 判定）を機械的に書き換える仕組みの導入。hash の再記録は許すが Pass/Fail 判定の改変は禁止という原則は本トラックの変更後も維持する [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D3]

## Constraints
- [CN-01] SoT 本体（spec.json / 型カタログ / ADR）はいかなる参照 hash も保持しない。hash の保管場所は verify-cache のみとし、記録は ref-verify run 実行時の自動計算・自動書き込みに限る。手動転記可能な宣言値をいかなる SoT artifact にも追加しない [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1]
- [CN-02] anchor の実在確認（dangling anchor 検出）は hash と独立した構造検査として維持しなければならない。SpecRef の hash フィールドを撤去しても、anchor が spec.json の spec element ID（goal / scope.in_scope / scope.out_of_scope / constraints / acceptance_criteria の id）に実在するかの検証は引き続き実施する。hash 検査の撤去が anchor 検査の撤去を含意しない [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D2]
- [CN-03] verify-cache の verdict（Pass/Fail 判定）をいかなるコマンドも機械的に書き換えてはならない。hash の再記録（cache entry の自動更新）は許容するが、Pass/Fail 判定の改変は禁止する。fail-closed は維持される [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D3]
- [CN-04] カタログスキーマの breaking 変更（schema_version 更新）が必要になる。既存トラックのカタログ移行が必要になるため、進行中トラックが少ないタイミングでのマージを考慮する。移行方針（マイグレーションガイドまたは自動変換の有無）を明示する [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1]
- [CN-05] Chain② claim_hash の計算は catalogue entry の canonical JSON 全体の SHA-256 として算出する（spec_refs[].hash フィールドを含まない新構造の canonical JSON が対象）。2026-05-27-1601 D4 の「per-entry SHA-256 として claim_hash を計算する」定義は維持されるが、フィールド撤去により計算基礎が変わる [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4, knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1]

## Acceptance Criteria
- [ ] [AC-01] SpecRef 型が { file: PathBuf, anchor: SpecElementId } の 2 フィールドのみを持ち、hash: ContentHash フィールドを持たない。SpecRef::new() のシグネチャが file と anchor のみを引数に取る [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1] [tasks: T001]
- [ ] [AC-02] 有効な <layer>-types.json（新 schema_version）の spec_refs[] エントリが { "file": "...", "anchor": "..." } の 2 フィールドのみを持ち、"hash" フィールドを含まない。CatalogueDocumentCodec がこの構造を正しく decode できる [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1] [tasks: T001]
- [ ] [AC-03] verify plan-artifact-refs が catalogue entry の SpecRef に対して hash mismatch 検査を行わない。anchor が spec.json に存在し、かつ hash フィールドがない SpecRef で verify plan-artifact-refs が exit code 0 を返す [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D2] [tasks: T001]
- [ ] [AC-04] verify catalogue-spec-refs が SpecRef の hash 照合を行わない。anchor が spec.json に存在し、かつ hash フィールドがない SpecRef で verify catalogue-spec-refs が exit code 0 を返す（stale signals チェックが無効の場合） [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D2] [tasks: T001]
- [ ] [AC-05] verify catalogue-spec-refs が dangling anchor（spec.json に存在しない anchor を参照している SpecRef）を引き続き検出する。hash フィールド撤去後も anchor 実在確認の失敗は exit code 1 を返す [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D2] [tasks: T001]
- [ ] [AC-06] verify plan-artifact-refs が dangling anchor（spec.json に存在しない anchor を参照している SpecRef）を引き続き検出する。hash フィールド撤去後も anchor 実在確認の失敗は finding としてレポートされる [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D2] [tasks: T001]
- [ ] [AC-07] Chain② pair 列挙（pair_source_chain2.rs）が spec_refs[].hash フィールドなしで正しく動作する。claim_hash は hash フィールドを含まない catalogue entry canonical JSON の SHA-256 として算出される [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1, knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T001]
- [ ] [AC-08] sotp track spec-element-hash コマンドの help テキストおよびドキュメントコメントから「type-designer が出力を転記する」主旨の記述が撤去されている [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D3] [tasks: T002]
- [ ] [AC-09] .claude/agents/type-designer.md の catalogue 作成手順に spec_refs[].hash の転記手順が存在しない。spec_refs の記載は { file, anchor } の 2 フィールドのみを示している [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D3] [tasks: T002]
- [ ] [AC-10] cargo make ci（または相当するテストスイート）が hash フィールド撤去後も通過する。SpecRef 型変更に起因するコンパイルエラーがない [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D1] [tasks: T001]
- [ ] [AC-11] spec 要素を 1 文字でも編集した後に ref-verify run を実行すると、変更された spec 要素を参照する catalogue entry の verify-cache エントリが stale として検出され、意味論レビューが発火する（再レビュー発火）。hash フィールドを転記しなくても CI がブロックされない [adr: knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md#D2, knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/source-attribution.md#Source Tag Types
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Error Handling: Result and ? Operator
- .claude/rules/04-coding-principles.md#Make Illegal States Unrepresentable

## Signal Summary

### Stage 1: Spec Signals
🔵 33  🟡 0  🔴 0

