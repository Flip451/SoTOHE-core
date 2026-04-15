<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-15T12:35:15Z"
version: "1.0"
signals: { blue: 29, yellow: 0, red: 0 }
---

# Catalogue active-track guard + rendered view source-file-name fix + sync_rendered_views multi-layer rollout

## Goal

sotp track type-signals が status=done/archived の track の catalogue を上書きしないよう fail-closed guard を追加する
render_type_catalogue のヘッダが呼び出し元の source file 名を正しく反映するよう API を変更する
sync_rendered_views が architecture-rules.json の tddd.enabled=true 全 layer を iterate し、各 <layer>-types.md を一括生成するよう拡張する (現状 domain-types.md のみで多層化に追従していない)
過去のレンダリング bug で破損した rendered view のヘッダをデータ復旧する
.claude/skills/track-plan/SKILL.md の古い記述 (feedback=Blue) を source-attribution.md SSoT に整合させる (本 track の plan 起草時の誤認識原因を除去する structural fix)

## Scope

### In Scope
- execute_type_signals に metadata.json.status == Done | Archived の fail-closed guard を追加する [source: apps/cli/src/commands/track/tddd/signals.rs §execute_type_signals line 96-128, libs/infrastructure/src/track/render.rs §sync_rendered_views line 573 is_done_or_archived pattern, feedback — ユーザー指摘: 現在アクティブなトラック外でビューファイルが生成される (2026-04-15)] [tasks: T003]
- render_type_catalogue の signature を (doc) → (doc, source_file_name: &str) に変更し、呼び出し元 2 箇所 (signals.rs:347, render.rs multi-layer loop 内) で source file 名を渡す [source: libs/infrastructure/src/type_catalogue_render.rs §line 64 hardcoded header, apps/cli/src/commands/track/tddd/signals.rs §validate_and_write_catalogue line 347, libs/infrastructure/src/track/render.rs §sync_rendered_views (D3 loop 内で呼ばれる)] [tasks: T004]
- sync_rendered_views を multi-layer 対応に拡張する — architecture-rules.json の tddd.enabled=true 全 layer を iterate し、各 <layer>-types.md を一括生成する。既存 libs/infrastructure/src/verify/tddd_layers.rs::parse_tddd_layers (tddd-01 Phase 1 Task 7 で導入、apps/cli::resolve_layers が既に reuse している public resolver) を直接 reuse する — 新 helper は作成しない。既存 is_done_or_archived / rendered_matches / TypeCatalogueCodecError::Json warn-and-continue の 3 pattern は保持する。 [source: libs/infrastructure/src/track/render.rs §sync_rendered_views line 568-606 (現状 domain-only loop), libs/infrastructure/src/verify/tddd_layers.rs §parse_tddd_layers line 139 (既存 public resolver), apps/cli/src/commands/track/tddd/signals.rs §line 15 §line 28 resolve_layers (既存 parse_tddd_layers reuse 箇所), architecture-rules.json §layers tddd.enabled, feedback — ユーザー指摘: sync_rendered_views が domain_types.md だけをレンダーするのはバグ (2026-04-15)] [tasks: T004]
- 既存の破損 rendered view のヘッダを source file 名の正しい値に復旧する (done track の is_done_or_archived guard を一時 bypass する手動 Edit が必要) [source: track/items/tddd-02-usecase-wiring-2026-04-14/usecase-types.md (現状: Generated from domain-types.json), track/items/domain-serde-ripout-2026-04-15/infrastructure-types.md (T001 で HEAD 状態に復元済み)] [tasks: T005]
- .claude/skills/track-plan/SKILL.md の line 165 classification table と line 283-284 diff hearing update guidance を source-attribution.md SSoT (2026-04-12 以降 feedback=Yellow) に整合させ、Blue 昇格には ADR/convention 永続化が必要な旨を明記する [source: .claude/skills/track-plan/SKILL.md §line 165-167 classification table (古記述: feedback=Blue), .claude/skills/track-plan/SKILL.md §line 283-285 diff hearing update guidance (古記述: feedback → Blue 昇格), knowledge/conventions/source-attribution.md §Source Tag Types Table, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §問題 2 feedback が Blue にマッピングされていた] [tasks: T006]
- ADR で D1 (status guard) + D2 (signature 変更) + D3 (sync_rendered_views multi-layer) + D4 (SKILL.md fix) を Accepted とし、B1-B5 の Rejected Alternatives (Fix B / Fix C / test detect / sync_rendered_views 別 track / SKILL.md 別 track) を理由付きで記録する。ADR-first gate は process constraint のため spec.json::constraints に記録し ADR に含めない (architectural decision と運用制約の分離) [source: knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D1 ADR-first 原則 (Track 1 教訓), knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §Reassess When feedback を Blue に再昇格する場合, knowledge/conventions/source-attribution.md §Upgrading Yellow to Blue] [tasks: T002]

### Out of Scope
- baseline-capture / design など他の catalogue write 経路への同種 guard 追加 (本 bug fix と同じ pattern だが別 sub-track に切り出し review cost を制御する) [source: convention — .claude/rules/10-guardrails.md §Small task commits 原則]
- 過去 track の signal を再計算する --read-only / --dry-run フラグ (Fix B) [source: knowledge/conventions/source-attribution.md §Strict gate semantics (merged track は全 blue 確定のため再計算は情報獲得にならない)]
- current git branch と track.branch の一致検証 (Fix C) [source: knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md §B2 (D1 status guard で core protection が完結、cross-track 誤操作は rare で over-engineering を回避する判断), convention — .claude/rules/10-guardrails.md §Small task commits 原則]
- parse_tddd_layers の 3 箇所目以降の caller のための追加共通化 (本 track の D3 で parse_tddd_layers は apps/cli::resolve_layers と sync_rendered_views loop の 2 箇所 caller で既に共通化されており、3 箇所目が必要になった時点で accessor 追加や error 型拡張など resolver 本体の evolution を別 track で検討) [source: libs/infrastructure/src/verify/tddd_layers.rs §parse_tddd_layers (既存 public resolver, 2 caller), convention — .claude/rules/10-guardrails.md §Small task commits 原則]

## Constraints
- fail-closed: guard が発動するケースは明示的な CliError::Message で reject する (sotp の既存エラー伝播 pattern) [source: convention — .claude/rules/06-security.md §fail-closed pattern, libs/infrastructure/src/verify/spec_states.rs §他 guard の既存実装 pattern]
- layer 依存方向を維持: signals.rs の guard は apps/cli 層で infrastructure の track metadata loader を呼ぶ (apps/cli → infrastructure の正しい方向)。sync_rendered_views の multi-layer loop は libs/infrastructure 層で既存 parse_tddd_layers (verify/tddd_layers.rs) を直接 reuse する。apps/cli::resolve_layers が既に parse_tddd_layers を import / call している事実がその依存方向の正当性を示す [source: architecture-rules.json, knowledge/conventions/hexagonal-architecture.md]
- BRIDGE-01 JSON wire format (domain schema export) は変更しない — 本 fix は catalogue render の rendering 層のみ対象 [source: knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D7]
- render_type_catalogue の signature 変更は backward compat を持たない breaking change だが、呼び出し側は workspace 内 2 箇所のみで一括修正可能 [source: apps/cli/src/commands/track/tddd/signals.rs §validate_and_write_catalogue line 347, libs/infrastructure/src/track/render.rs §sync_rendered_views line 577]
- sync_rendered_views の multi-layer loop は既存 3 pattern (is_done_or_archived guard / rendered_matches drift check / TypeCatalogueCodecError::Json warn-and-continue) を loop 内の各 layer に個別適用し、既存 domain-types.md の挙動との互換性を完全維持する [source: libs/infrastructure/src/track/render.rs §sync_rendered_views line 572-606 (既存 pattern)]
- per-layer opt-out を尊重: catalogue file (例 usecase-types.json) が track dir に存在しない場合は render を skip する (domain-serde-ripout Track 1 の opt-out pattern を壊さない) [source: knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D8 per-layer opt-out]
- ADR-first gate: T003 以降の code 変更は T002 ADR が Status=Accepted になってから着手する (Track 1 §D1 教訓の再発防止) [source: knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D1 プロセス違反の記録]

## Acceptance Criteria
- [ ] sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure を main branch 上で実行すると CliError が返り、catalogue / rendered view とも touch されない [source: feedback — ユーザー観測バグの再現シナリオ (2026-04-15), apps/cli/src/commands/track/tddd/signals.rs §execute_type_signals (guard 追加対象)] [tasks: T003]
- [ ] test_execute_type_signals_rejects_done_track が新規追加され pass する (archived variant も含む) [source: convention — .claude/rules/05-testing.md §Core Principles TDD (Red/Green/Refactor)] [tasks: T003]
- [ ] cargo make export-schema -- --crate infrastructure --pretty の出力は本 fix 前後で structural に同一 (BRIDGE-01 互換性) [source: knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D7] [tasks: T004]
- [ ] render_type_catalogue signature 変更後の既存 test (type_catalogue_render.rs:211) が pass し、新規 test (non-domain source file 名の header 生成) が追加され pass する [source: libs/infrastructure/src/type_catalogue_render.rs §line 211 existing header assertion] [tasks: T004]
- [ ] sync_rendered_views が tddd.enabled=true の全 layer (domain / usecase / infrastructure) の <layer>-types.md を生成する。新規テスト sync_rendered_views_generates_usecase_types_md_from_usecase_types_json、sync_rendered_views_generates_infrastructure_types_md_from_infrastructure_types_json、sync_rendered_views_generates_multiple_layer_types_md_independently が追加され pass する [source: libs/infrastructure/src/track/render.rs §sync_rendered_views (multi-layer loop 拡張対象), architecture-rules.json §layers] [tasks: T004]
- [ ] 既存 test sync_rendered_views_generates_domain_types_md_from_domain_types_json が signature 変更後も pass (backward compat) [source: libs/infrastructure/src/track/render.rs §line 1940 existing test] [tasks: T004]
- [ ] track/items/tddd-02-usecase-wiring-2026-04-14/usecase-types.md の 1 行目が <!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY --> に復旧される [source: track/items/tddd-02-usecase-wiring-2026-04-14/usecase-types.md (現在の破損状態)] [tasks: T005]
- [ ] track/items/domain-serde-ripout-2026-04-15/infrastructure-types.md の 1 行目が <!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY --> のまま維持される (T001 revert で復元済み、T004 fix 後に dry re-run で drift しない) [source: track/items/domain-serde-ripout-2026-04-15/infrastructure-types.md (T001 で HEAD 状態に復元済み)] [tasks: T004, T005]
- [ ] .claude/skills/track-plan/SKILL.md line 165 + 283-284 の記述が source-attribution.md SSoT と一致する (feedback = Yellow、Blue 昇格には ADR/convention 永続化が必要) [source: knowledge/conventions/source-attribution.md §Upgrading Yellow to Blue, .claude/skills/track-plan/SKILL.md §line 165-167 classification table (修正対象), .claude/skills/track-plan/SKILL.md §line 283-285 diff hearing update guidance (修正対象)] [tasks: T006]
- [ ] ADR 2026-04-15-1012-catalogue-active-guard-fix.md が Accepted 状態で knowledge/adr/README.md の信号機アーキテクチャ section に索引登録される [source: knowledge/adr/README.md §索引 信号機アーキテクチャ section] [tasks: T002, T007]
- [ ] 最終 smoke test: cargo make track-sync-views を active track (catalogue-active-guard-fix-2026-04-15) 上で実行し、domain / usecase / infrastructure 各 layer の catalogue file が存在すれば対応する <layer>-types.md が生成または更新される挙動を手動確認する [source: libs/infrastructure/src/track/render.rs §sync_rendered_views (T004 後の振る舞い)] [tasks: T007]
- [ ] cargo make ci (fmt-check + clippy -D warnings + nextest + deny + check-layers + verify-spec-states + verify-arch-docs) 全 pass [source: convention — .claude/rules/07-dev-environment.md §Pre-commit Checklist] [tasks: T007]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/filesystem-persistence-guard.md
- knowledge/conventions/prefer-type-safe-abstractions.md
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 29  🟡 0  🔴 0

