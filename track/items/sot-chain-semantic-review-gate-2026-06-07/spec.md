<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 46, yellow: 0, red: 0 }
---

# SoT Chain に意味論レビューゲートを追加する

## Goal

- [GO-01] SoT Chain の各リンク（Chain①: spec → ADR、Chain②: catalogue → spec）について、引用の有無（presence 信号）だけでなく「引用先が主張を意味的に裏付けているか」を検証する意味論レビュー層（層③）を追加し、現在は検出不能な『青なのに意味的に矛盾している』ケースを捉えられるようにする [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D1]
- [GO-02] 意味論レビューをハードゲートとして commit フローに統合し、hash ベースのキャッシュで変更差分のみを再検証することで、検証コストを許容範囲に抑えながら判定の決定性を確保する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4]
- [GO-03] catalogue entry の per-entry hash を新規導入することで、Chain② の hash 整合ゲートの粒度を `<layer>-types.json` 全体から entry 単位へ精緻化し、差分再評価を entry 単位で可能にする [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4]

## Scope

### In Scope
- [IN-01] 意味論レビューを presence 信号（🔵🟡🔴）とは独立した判定結果レーンとして設計する。既存の code-review インフラ（`Reviewer` port・`Verdict`・`review.json`・`ClaudeReviewer` adapter・review-fix-lead）を流用せず、専用の port・型・adapter・capability（D7）・キャッシュ artifact（D8）を持つ独立レーンとして実装する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D1] [tasks: T002, T007]
- [IN-02] 意味論レビューを2段階で発火させる。spec-design 後に Chain① を検証し、type-design 後に Chain② を検証する（phase 第一防衛線）。さらに commit 前に全 Chain を最終関門として検証し、既存 code-review の `check-approved` と AND して commit を通す [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D2] [tasks: T010, T011]
- [IN-03] Chain① 用の ADR 参照に対して、anchor の実在検証を追加する。`adr_refs[].anchor` の値（例: `"D1"`）が ADR front-matter の `decisions[].id` に実際に存在するかを `plan-artifact-refs` 検証器で確認する。ADR の同一性は git blob hash に委ね、ADR への canonical hash は導入しない [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D3] [tasks: T001]
- [IN-04] 意味論の判定結果を `(claim_hash, evidence_hash)` ペアで凍結するキャッシュを実装する。Chain① では claim=spec element の per-element SHA-256 / evidence=ADR の git blob hash、Chain② では claim=catalogue entry の per-entry SHA-256（新規）/ evidence=spec element の per-element SHA-256 とする。どちらかの hash が変化したときのみ再レビューを実行する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T002, T007, T008]
- [IN-05] catalogue entry に per-entry SHA-256 hash を新規追加する。現行の `catalogue_declaration_hash`（`<layer>-types.json` 全体粒度）はそのまま維持しつつ、entry 単位の hash を追加することで変更されたエントリのみを再レビューする差分処理を可能にする [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T003, T005]
- [IN-06] 参照ペアの意味論検証に3段のモデル階層（段1: fast 解決の軽量モデル、段2: final 解決の重量級モデル、段3: 人間）を適用する。fast 段で全ペアを並列評価し、通常時は不合格または判定保留の production pair のみを final 段へ引き上げる。ただし同一バッチの known_bad 検出率が閾値未満の場合は、fast Pass を含む同一バッチの未cache production pair を untrusted として final 段で再評価してから cache / human escalation を判断する。引き上げの単位は参照ペア単位であり、全体一括の引き上げではない [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D5] [tasks: T007]
- [IN-07] 合格判定時に根拠（ADR または spec element）のどの箇所が主張を裏付けるかの引用を必須とする。引用を提示できない合格は判定保留として不合格方向に倒す。これにより素通りを構造的に排除する（裏付け箇所の引用必須化） [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D6] [tasks: T002, T007]
- [IN-08] 各バッチに答えが不合格と分かっているペア（既知の誤り例）を注入し、検出率を監視する。検出率が閾値を下回ったら D5 の段階引き上げ経路に入る。注入率と検出率閾値は設定ファイルで指定可能とし、初期デフォルトは注入率 10% / 検出率閾値 90% とする [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D6] [tasks: T007, T008, T009]
- [IN-09] `agent-profiles.json` に `ref-verifier` 専用 capability を新設する。`reviewer` capability と独立してモデル・タイムアウト・プロンプトを設定でき、`fast_provider` / `fast_model` と `provider` / `model` を別々に指定することで段1（Fast）と段2（Final）に異なる provider を割り当てられる [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D7] [tasks: T006, T008]
- [IN-10] 意味論レビューの判定結果を Chain ごとの別 artifact に収容する。Chain① の結果は `spec-adr-verify-cache.json`（track 単位）、Chain② の結果は `<layer>-catalogue-spec-verify-cache.json`（domain / usecase / infrastructure / cli ごとに4ファイル）として新設する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D8] [tasks: T004, T007, T008]
- [IN-11] 意味論検証の呼び出し口として専用の `/track:ref-verify` skill（公開 UI）と `bin/sotp ref-verify` サブコマンドを新設する。既存の `sotp verify *`（層②の構造/新鮮度チェック）や `/track:review`（code review）には相乗りせず、層③専用の独立した surface を持つ [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D9] [tasks: T007, T009, T011]
- [IN-12] `bin/sotp ref-verify` は1コマンドで全 Chain を扱い、スコープを呼び出しコンテキストから自動解決する。spec-design 後は Chain①、type-design 後は Chain②、commit 最終関門は両 Chain を対象とする。差分キャッシュにより、対象のうち変化したペアだけが再レビューされる [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D9] [tasks: T007, T008, T009]
- [IN-13] catalogue entry の section-qualified key（`section_key`）は、`types:<name>` / `traits:<name>` / `functions:<path>` の形式で一つの `CatalogueDocument` 内でグローバルに一意な識別子とする。短い bare key がタイプ・トレイト・関数の複数セクションに重複して存在するケース（クロスセクション衝突）を防ぐため、インフラストラクチャアダプターが per-entry hash マップを構築する際のキーには bare key ではなく `section_key` を使用する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T003, T005]
- [IN-14] catalogue の全エントリを正規の順序（types → traits → functions、各セクション内は BTreeMap 順）でイテレートし、各エントリに bare `key` と section-qualified `section_key` の両方を付与するファンクションを一箇所に集約する。`catalogue_spec_refs` と `catalogue_spec_signals` で独立してキー導出ロジックを持つと drift が生じるため、すべてのインフラアダプターはこの集約関数経由でエントリを走査する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T003, T005]

### Out of Scope
- [OS-01] 意味論検証結果を presence 信号（🔵🟡🔴）の色に統合すること。presence（引用したか）と意味論（裏付けるか）は別の層であり、混在させると信号の情報量が落ちる [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D1] [tasks: T002]
- [OS-02] ADR decision への canonical hash 導入（`AdrRef` への `hash: ContentHash` 追加）。ADR は設計判断を散文で記述する文書であり、typo 修正などの些細な変更で過敏に古くなる hash を追加することは採用しない。ADR の同一性は git blob hash と anchor 存在確認 + 意味論レビューで代替する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D3] [tasks: T001]
- [OS-03] 意味論検証結果を参考情報（ゲートなし）として出力するのみの実装。参考情報は開発フロー中で無視されるため、ハードゲートとして機能させる [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D6] [tasks: T009, T010]
- [OS-04] 既知の誤り例検査で劣化を検知した場合の人間への即時引き上げ。より安価な重量級モデルへの昇格を先に試み、それでも解決しない場合のみ人間へ報告する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D5] [tasks: T007]
- [OS-05] catalogue → 実装のリンク（SoT Chain③）の意味論検証。このリンクは TDDD の type-signals が別途担う。本トラックの対象は Chain①（spec → ADR）と Chain②（catalogue → spec）のみ [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D1] [tasks: T002, T007]
- [OS-06] 意味論検証を `/track:review` の scope に相乗りさせること。code review（`review.json` / ScopeRound）と意味論検証レーン（verify-cache artifacts）は責務が異なり、D1 の SRP 分離と整合させるため相乗りは行わない [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D9] [tasks: T009, T011]

## Constraints
- [CN-01] 意味論レビューレーンは code-review インフラ（`Reviewer` port・`ReviewTarget`・`Verdict`・`ReviewerFinding`・`review.json`・`ClaudeReviewer` adapter）の型や責務を共有しない。専用の port・型・adapter を独立に持ち、SRP 違反を構造的に排除する。`agent-profiles.json` の capability 解決機構（`resolve_execution`）は責務中立な部品として共有する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D1] [tasks: T002, T007, T008]
- [CN-02] commit 最終関門では、既存 code-review の `check-approved` と意味論側 approved を独立した2つのゲートとして AND 評価する。既存 `check-approved` に意味論 artifact の読み込みを追加して拡張することは行わない（D1 の SRP 分離と整合） [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D2] [tasks: T009, T010]
- [CN-03] Chain① の再走トリガーは ADR ファイルの git blob hash 変化、または spec element の per-element hash 変化とする。git blob hash は追加の hash 計算なしに ADR ファイルの同一性を表す [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D3] [tasks: T001]
- [CN-04] catalogue entry の per-entry hash の計算は、既存の `canonical_json_sha256` ヘルパー（`libs/infrastructure/src/verify/plan_artifact_refs/spec_refs.rs`）を流用する。計算対象は entry 単位の canonical JSON subtree とする [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T003, T005]
- [CN-05] fast 段でのペア評価は並列実行する。Chain① は (spec element × ADR file) のペア単位で並列化、Chain② は layer 単位バッチ（domain / usecase / infrastructure / cli ごと）で並列化する。バッチ上限とスロットリングの具体数値は設定ファイルで指定可能とする [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D5] [tasks: T007]
- [CN-06] 意味論検証の判定結果は fail-closed で扱う。「引用を提示できない合格」は合格ではなく判定保留（不合格方向）とする。キャッシュに凍結済みの判定結果は `(claim_hash, evidence_hash)` ペアが変化しない限り再実行しない [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D6] [tasks: T002, T007]
- [CN-07] `ref-verifier` capability の具体的なモデル値は `agent-profiles.json` で指定し、ADR 本文には非 canonical なモデル値を記載しない。段1（Fast）と段2（Final）の役割のみ ADR が規定し、具体値は設定に委ねる [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D7] [tasks: T006]
- [CN-08] 意味論側 approved の評価は `spec-adr-verify-cache.json` および `<layer>-catalogue-spec-verify-cache.json` を読む専用ゲートとして実装する。既存の `check-approved` コマンドを意味論 artifact 対応に拡張することは行わない [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D8] [tasks: T009, T010]
- [CN-09] `/track:plan` の phase オーケストレーションが phase 第一防衛線を、commit ゲートが最終関門を、それぞれ `bin/sotp ref-verify` 経由で起動する。発火点ごとに別実装を持たず、同一コマンドを呼ぶ [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D9] [tasks: T009, T010, T011]
- [CN-10] 新規に追加する hash は catalogue entry の per-entry hash のみとする。現行の `catalogue_declaration_hash`（`<layer>-types.json` 全体粒度）は廃止せず、per-entry hash と共存させる [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T003, T005]

## Acceptance Criteria
- [ ] [AC-01] `agent-profiles.json` に `ref-verifier` capability エントリが存在し、`fast_provider` / `fast_model`（段1）と `provider` / `model`（段2）を独立して設定できる。`libs/infrastructure/src/agent_profiles.rs` の `resolve_execution` が `RoundType::Fast` で `fast_provider ?? provider` / `fast_model ?? model` を解決し、`RoundType::Final` で `provider` / `model` を返すことで `ref-verifier` の provider をまたぐ段階引き上げが機能する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D7] [tasks: T006, T008]
- [ ] [AC-02] `bin/sotp ref-verify` サブコマンドが実装されており、呼び出しコンテキストから対象 Chain を自動解決する。spec-design 後は Chain①、type-design 後は Chain②、commit 最終関門は両 Chain を対象とし、スタンドアロン実行では未確定の全ペアを対象とする [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D9] [tasks: T009]
- [ ] [AC-03] Chain① の検証結果が `track/items/<id>/spec-adr-verify-cache.json` に書き込まれ、Chain② の検証結果が `track/items/<id>/<layer>-catalogue-spec-verify-cache.json`（domain / usecase / infrastructure / cli ごと）に書き込まれる。各 artifact は `(claim_hash, evidence_hash)` ペアと判定結果（合格/不合格/判定保留）を含む [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D8] [tasks: T004, T007, T008, T009]
- [ ] [AC-04] 意味論側 approved ゲートが verify-cache artifact を読んで評価し、commit 最終関門で code-review の `check-approved` と AND される。コード変更なしで verify-cache が全合格の場合に commit を通し、未確定または不合格のペアが残る場合は commit をブロックする [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D2] [tasks: T009, T010]
- [ ] [AC-05] `plan-artifact-refs` 検証器が `adr_refs[].anchor` の値を ADR front-matter の `decisions[].id` に対して実在確認する。存在しない anchor を持つ `AdrRef` が spec.json に含まれる場合に `sotp verify plan-artifact-refs` が失敗する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D3] [tasks: T001]
- [ ] [AC-06] catalogue entry の canonical JSON subtree から計算した per-entry SHA-256 hash が `<layer>-catalogue-spec-signals.json` の `CatalogueSpecSignal.entry_hash` として追加されており、この signals codec が hash を encode/decode できる。per-entry hash は `canonical_json_sha256` ヘルパーで計算され、既存の document-level `catalogue_declaration_hash`（全体粒度）と共存する [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T003, T005]
- [ ] [AC-07] キャッシュの差分動作が正しい。`(claim_hash, evidence_hash)` ペアが変化していないエントリは再レビューをスキップし、いずれかが変化したエントリのみを再レビューする。phase 第一防衛線で通過済みの不変ペアは commit 最終関門で再レビューされない [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T007, T008, T012]
- [ ] [AC-08] fast 段で不合格または判定保留となった production pair は final 段へ引き上げられる。fast 段で合格した production pair は、同一バッチの known_bad 検出率が閾値以上で fast tier が健全と確認できた場合のみ final 段を実行しない。known_bad 検出率が閾値未満のバッチでは fast verdict を信頼済み Pass として cache せず、同一バッチの未cache production pair を final 段で再評価してから cache / human escalation を判断する。引き上げの単位は問題が生じたバッチ内の参照ペア単位であり、全 Chain 一括の引き上げは発生しない [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D5] [tasks: T007, T012]
- [ ] [AC-09] バッチに注入された既知の誤り例ペアが正しく不合格と判定される割合が閾値（初期値 90%）以上であれば通常処理を継続し、閾値を下回った場合は D5 の段階引き上げ経路（段2 → 段3）に入る [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D6] [tasks: T007, T008, T009, T012]
- [ ] [AC-10] 合格判定の際に根拠のどの箇所が主張を裏付けるかの引用が判定結果 artifact に記録される。引用なしの合格は判定保留として扱われ、verify-cache に合格として記録されない [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D6] [tasks: T002, T007]
- [ ] [AC-11] `/track:ref-verify` skill と `bin/sotp ref-verify` が存在し、`sotp verify *`（層②）および `/track:review`（code review）とは独立した呼び出し口として機能する。3つの検証層がそれぞれ独立した surface を持つ [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D9] [tasks: T009, T011]
- [ ] [AC-12] `ref-verifier` capability と `reviewer` capability が同時に実行される場合に、設定（モデル・タイムアウト・プロンプト）が互いに干渉しない。各 capability は `agent-profiles.json` 内で独立した設定エントリを持つ [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D7] [tasks: T006, T008, T012]
- [ ] [AC-13] `RefreshCatalogueSpecSignalsInteractor` がインフラアダプターから受け取る per-entry hash マップは `section_key`（`types:<name>` / `traits:<name>` / `functions:<path>` 形式）をキーとして構成される。`iter_catalogue_entries` が生成した `section_key` に対応するエントリハッシュがマップに存在しない場合、ユースケースは偽のゼロ hash を補完せず `MissingEntryHash` エラーを返してフェイルクローズする [adr: knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D4] [tasks: T003, T005, T012]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Port Placement Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/typed-deserialization.md#Rule
- knowledge/conventions/no-backward-compat.md#Rules
- knowledge/conventions/source-attribution.md#Source Tag Types
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Error Handling: Result and ? Operator
- .claude/rules/04-coding-principles.md#Make Illegal States Unrepresentable

## Signal Summary

### Stage 1: Spec Signals
🔵 46  🟡 0  🔴 0

