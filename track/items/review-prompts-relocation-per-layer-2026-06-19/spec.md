<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 37, yellow: 0, red: 0 }
---

# Reviewer briefing のレイヤー別化と review-prompts の .harness/custom/ への移設

## Goal

- [GO-01] code 5 層 (domain / usecase / infrastructure / cli / cli_composition) と harness-policy scope にレイヤー固有の reviewer severity policy briefing を新設し、briefing の scope 間非対称性を解消する。各 reviewer がレイヤー固有の観点 (domain: 型安全 / 不変条件 / enum-first / typestate / no-panics; infrastructure: adapter rules / I-O 境界 / serde codec; cli: CLI→usecase 経由強制 / domain 直参照禁止 等) で findings を選別できるようにする [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D3, knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D4]
- [GO-02] track/review-prompts/ (review-prompts md 群) を .harness/custom/review-prompts/ に、track/review-scope.json を .harness/config/review-scope.json に移設し、briefing_file 値と loader ハードコード path を新しい配置先に一括更新する clean move を実施する [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D1, knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2, knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D5]
- [GO-03] .harness/custom/ を利用者所有ゾーンとして確立し、framework methodology (briefings/ / prompts/ / capabilities/) と利用者カスタムを物理的に分離することで、テンプレート配布時に framework 更新と利用者 review カスタムが衝突しない構造を作る [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D6]

## Scope

### In Scope
- [IN-01] track/review-prompts/plan-artifacts.md を .harness/custom/review-prompts/plan-artifacts.md に移設する (内容は維持) [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D1] [tasks: T001]
- [IN-02] .harness/custom/review-prompts/ 配下に domain.md / usecase.md / infrastructure.md / cli.md / cli_composition.md / harness-policy.md を新規作成する。各ファイルは plan-artifacts.md と同じ 'What to report / What NOT to report' 構成に従い、各レイヤー固有の severity 観点を盛り込む [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D3, knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D4] [tasks: T001]
- [IN-03] track/review-scope.json を .harness/config/review-scope.json に移設し、各 scope (domain / usecase / infrastructure / cli / cli_composition / plan-artifacts / harness-policy) の briefing_file 値を .harness/custom/review-prompts/<scope>.md に更新する [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2] [tasks: T002]
- [IN-04] apps/cli-composition/src/review_v2/scope.rs (または等価箇所) の 'track/review-scope.json' ハードコードパスを '.harness/config/review-scope.json' に更新する [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2] [tasks: T002]
- [IN-05] harness-policy scope の patterns から明示的な 'track/review-scope.json' エントリを削除する。移設先 .harness/config/review-scope.json は既存の '.harness/**' パターンで自動的に harness-policy に分類されるため冗長エントリを解消する [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2] [tasks: T002]
- [IN-06] track/review-prompts/ ディレクトリが空になった後に削除する。旧 track/review-scope.json も同コミットで削除する (D5: clean move、旧 path への fallback なし) [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D5] [tasks: T002]
- [IN-07] track/review-scope.json を参照しているドキュメント (README.md / .claude/rules/ / knowledge/conventions/ 等の現役 doc) を .harness/config/review-scope.json への参照に一括更新する [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D5] [tasks: T004]
- [IN-08] track/review-prompts/ を参照しているドキュメント (現役 doc) を .harness/custom/review-prompts/ への参照に一括更新する [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D1, knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D5] [tasks: T004]
- [IN-09] review-scope.json の version フィールドは据え置く (schema 変更なし)。briefing_file は既存の optional field であり、path 値の変更と未設定 scope への追加のみを行う [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2] [tasks: T002]
- [IN-10] apps/cli-composition/src/review_v2/briefing.rs の append_scope_briefing_reference_str および helpers.rs の build_base_prompt_from_input が runtime で参照する briefing_file パスが新しいパス (.harness/custom/review-prompts/<scope>.md) になっていることを確認する。注入機構 (Read パス参照方式) 自体は変更しない [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2] [tasks: T002]

### Out of Scope
- [OS-01] briefing_file の broken path を検出する CI lint の導入は本 track のスコープ外。runtime の Read 失敗に委ねる既存方針を維持する (ADR Open Questions 参照) [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D6]
- [OS-02] .harness/custom/ never-clobber の具体的な配布機構 (merge tool / docs convention / .gitattributes 等) の確定は本 track のスコープ外。custom/ ゾーンの確立 (D6) を先行し、配布機構は別途詰める [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D6]
- [OS-03] review-scope.json の schema version 変更は行わない。briefing_file は後方互換の optional field である [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2]
- [OS-04] Rust の reviewer briefing 注入機構 (append_scope_briefing_reference_str / build_base_prompt_from_input) 自体のロジック変更は行わない。本 track が変えるのはパス値のみ [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2]
- [OS-05] architecture-customizer が層をリネームする際の review-scope.json / .harness/custom/review-prompts/<scope>.md への追従連動は本 track のスコープ外 [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D3]
- [OS-06] マージ済み過去 ADR に残る track/review-scope.json や track/review-prompts/ への参照は更新しない。過去 ADR は当時の文脈の記録であり post-merge 不変原則に従う [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D5] [conv: knowledge/conventions/adr.md#Lifecycle: pre-merge draft vs post-merge record]

## Constraints
- [CN-01] 移行は clean move (D5) で実施する。旧 path (track/review-scope.json / track/review-prompts/) への fallback は設けず、移行コミットで全参照 (loader ハードコード path / briefing_file 値 / harness-policy patterns / docs / tests) を同時更新する [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D5] [tasks: T002, T003, T004]
- [CN-02] .harness/custom/ 配下のコンテンツ (briefing md の文言・scope 選択) を CI で hard-fail enforce しない。提供 + docs のみとし、enforcement は利用者の責任領域とする (responsibility-boundary 原則) [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D6] [conv: knowledge/conventions/responsibility-boundary.md#Rules] [tasks: T001, T002, T003, T004, T005]
- [CN-03] 各 briefing md は plan-artifacts.md の 'What to report / What NOT to report' 構成を踏襲し、reviewer が 'findings として挙げる / 無視する' を判断できる粒度で明文化する。convention 本文との重複は層固有観点に絞って最小化する [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D4] [tasks: T001]
- [CN-04] briefing_file の値は workspace-relative path 文字列のままとする。loader の解決規則 (workspace-relative) は変更しない [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2] [tasks: T002]
- [CN-05] 注入機構は Read パス参照方式を維持する。briefing_file の内容を loader / composer が fs::read して inline 展開してはならない [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2] [tasks: T002]
- [CN-06] review-scope.json を .harness/config/ に置く場合も agent-profiles.json と同様の構造化 JSON wiring として扱い、samples/ 方式は後続 track に委ねる (本 track では直置きのみ) [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2] [tasks: T002]

## Acceptance Criteria
- [ ] [AC-01] .harness/custom/review-prompts/plan-artifacts.md が存在し、移設前の track/review-prompts/plan-artifacts.md と同内容である [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D1] [tasks: T001]
- [ ] [AC-02] .harness/custom/review-prompts/ 配下に domain.md / usecase.md / infrastructure.md / cli.md / cli_composition.md / harness-policy.md の 6 ファイルが存在する [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D3] [tasks: T001]
- [ ] [AC-03] 各 briefing md (AC-02 の 6 ファイル) が 'What to report' / 'What NOT to report' の両セクションを持ち、そのレイヤー固有の severity 観点を明文化している [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D4] [tasks: T001]
- [ ] [AC-04] .harness/config/review-scope.json が存在し、全 7 scope (domain / usecase / infrastructure / cli / cli_composition / plan-artifacts / harness-policy) の briefing_file が .harness/custom/review-prompts/<scope>.md を指している [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2, knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D3] [tasks: T002]
- [ ] [AC-05] track/review-scope.json が存在しない (git status で削除として記録される) [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D5] [tasks: T002]
- [ ] [AC-06] track/review-prompts/ ディレクトリが存在しない (git status で削除として記録される) [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D5] [tasks: T002]
- [ ] [AC-07] apps/cli-composition/src/review_v2/ の loader が .harness/config/review-scope.json を参照している (旧 track/review-scope.json へのハードコード参照が存在しない) [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2] [tasks: T002]
- [ ] [AC-08] .harness/config/review-scope.json の harness-policy scope の patterns に 'track/review-scope.json' エントリが存在しない [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2] [tasks: T002]
- [ ] [AC-09] 現役 doc (README.md / .claude/rules/ / knowledge/conventions/ / CLAUDE.md 等) に track/review-scope.json または track/review-prompts/ への参照が存在しない [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D5] [tasks: T004]
- [ ] [AC-10] cargo make ci が pass する (移行後もビルド / lint / test / check-layers / verify-arch-docs が全て緑になる) [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D5] [tasks: T005]
- [ ] [AC-11] .harness/ 配下のディレクトリ構造が ADR D2 最終レイアウト図と一致する: config/ に review-scope.json、custom/ に review-prompts/ (全 7 ファイル) [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2, knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D6] [tasks: T001, T002]
- [ ] [AC-12] /track:review を実行したとき、domain / usecase / infrastructure / cli / cli_composition / plan-artifacts / harness-policy の各 scope reviewer に scope 固有の severity policy briefing が注入される (runtime 検証: 各スコープの briefing_file が Some になっている) [adr: knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D3, knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md#D2] [tasks: T002]

## Related Conventions (Required Reading)
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/track-lifecycle.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 37  🟡 0  🔴 0

