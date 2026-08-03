---
adr_id: adr-readme-index
decisions: []
---
# Architecture Decision Records (ADR)

このディレクトリは設計判断の記録を管理する。

## 運用ルール

- **フォーマット**: Nygard 式 + Rejected Alternatives + Reassess When
- **言語**: 日本語
- **採番**: `YYYY-MM-DD-HHMM-slug.md`（例: `2026-03-11-1430-track-status-derived.md`）
- **front-matter**: MD body の前に `adr_id` と `decisions[]` を必須で置く。各 decision は根拠 ref と decision 単位の `status` を持つ。
- **decision status**: `proposed` / `accepted` / `implemented` / `superseded` / `deprecated`。`implemented` には `implemented_in`、`superseded` には `superseded_by` が必須。
- **根拠**: 新規 decision には `user_decision_ref` または `review_finding_ref` を入れる。file-level の `## Status` は使用しない。

## ADR テンプレート

```markdown
---
adr_id: "<YYYY-MM-DD-HHMM>-<slug>"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:<session>:<date>"
    status: proposed
---
# {タイトル}

## Context

{なぜこの判断が必要だったか}

## Decision

{何を選んだか}

## Rejected Alternatives

- {選択肢B}: {却下理由}
- {選択肢C}: {却下理由}

## Consequences

- Good: {良い影響}
- Bad: {悪い影響・トレードオフ}

## Reassess When

- {前提が変わる条件}
```

## ADR と Convention の関係

| | ADR | Convention |
|---|---|---|
| 問い | 「なぜこうした？」 | 「これからどうする？」 |
| 時制 | 過去形（あの時点で判断した） | 現在形（今後はこうせよ） |
| 寿命 | 永続（superseded でも残る） | 現行ルールのみ有効 |
| 例 | 「conch-parser を選んだ。理由は...」 | 「shell パースは conch-parser を使え」 |

Convention に `## Decision Reference` セクションを追加し ADR にリンクする。

## 索引

### プロジェクト戦略

| ADR | Status | Date |
|-----|--------|------|
| [Phase 1.5 を good enough 宣言](2026-03-23-2100-phase-1.5-good-enough.md) | Accepted | 2026-03-23 |
| [sotp CLI 外部ツール化は Moat 後に再評価](2026-03-23-2110-sotp-extraction-deferred.md) | Accepted | 2026-03-23 |
| [sotp 開発領域と汎用テンプレートの分離境界・切り出し方式](2026-07-06-1717-template-extraction-boundary.md) | Proposed | 2026-07-06 |
| [export は自バイナリを移植する](2026-07-08-0541-template-export-sotp-binary-transplant.md) | Proposed | 2026-07-08 |
| [TODO マーカー状態管理の廃止](2026-07-08-1020-retire-todo-marker-state-and-track-docs.md) | Proposed | 2026-07-08 |
| [技術スタック・製品ガイドラインの grandfathered baseline](2026-07-08-1405-grandfathered-tech-and-product-baseline.md) | Proposed | 2026-07-08 |
| [retention gate の verify サブコマンド化](2026-07-08-2306-retention-gate-verify-subcommand.md) | Proposed | 2026-07-08 |
| [公開テンプレート配布前の阻害要因解消](2026-07-13-0818-public-template-blocker-cleanup.md) | Proposed | 2026-07-13 |
| [scaffold の初期化列を単一タスクへ畳む](2026-07-23-0115-scaffold-first-run-experience.md) | Proposed | 2026-07-23 |
| [出荷面を最小化し、workflow と出荷物の乖離クラスを閉じる](2026-07-23-0117-export-surface-minimization.md) | Proposed | 2026-07-23 |
| [機械可読な契約を持たない出荷面 assert を削除する](2026-07-25-0045-drop-contractless-export-surface-assertions.md) | Proposed | 2026-07-25 |
| [scripts/ Python ヘルパーの段階的 Rust 移行ロードマップ](2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md) | Proposed | 2026-04-13 |
| [external_guides 撤去 — Python migration roadmap Phase 3 supersede](2026-04-28-1258-remove-external-guides.md) | — | 2026-04-28 |
| [Python 固有ロジックの Rust 完全移行と Python ランタイム依存の撤去](2026-06-03-1327-python-runtime-full-removal.md) | Proposed | 2026-06-03 |

### 信号機アーキテクチャ

| ADR | Status | Date |
|-----|--------|------|
| [2 段階信号機アーキテクチャ](2026-03-23-2120-two-stage-signal-architecture.md) | Accepted | 2026-03-23 |
| [spec ↔ code 整合性チェックは Phase 3 に送る](2026-03-23-2130-spec-code-consistency-deferred.md) | Accepted | 2026-03-23 |
| [Coverage は信号機ではなく CI ゲートとする](2026-03-24-0900-coverage-not-a-signal.md) | Accepted | 2026-03-24 |
| [Stage 2 信号機にコンパイル通過を条件に入れない](2026-03-24-0910-stage2-no-compile-check.md) | Accepted | 2026-03-24 |
| [3-12 spec ↔ code 整合性チェック — TypeGraph + 既知課題の解決](2026-04-08-0045-spec-code-consistency-check-design.md) | Accepted | 2026-04-08 |
| [TDDD: 逆方向チェック信号機統合 + designer capability](2026-04-08-1800-reverse-signal-integration.md) | Accepted | 2026-04-08 |
| [TDDD-02: Baseline-Aware Reverse Signal Detection](2026-04-11-0001-baseline-reverse-signals.md) | Proposed | 2026-04-11 |
| [TDDD-01: Multilayer Extension — 型カタログ多層化 + シグネチャ検証](2026-04-11-0002-tddd-multilayer-extension.md) | Proposed | 2026-04-11 |
| [TDDD-03: 型アクション宣言 — add / modify / delete](2026-04-11-0003-type-action-declarations.md) | Accepted | 2026-04-11 |
| [TDDD 型カタログ Taxonomy 拡張 — アプリケーション層パターンの幅を広げる](2026-04-13-1813-tddd-taxonomy-expansion.md) | Accepted | 2026-04-13 |
| [Finding 型 Taxonomy クリーンアップ — 同名衝突の解消と hexagonal 分離の維持](2026-04-14-0625-finding-taxonomy-cleanup.md) | Accepted | 2026-04-14 |
| [Domain serde 依存除去 — hexagonal 純粋性回復 + infrastructure 層 TDDD partial dogfood](2026-04-14-1531-domain-serde-ripout.md) | Accepted | 2026-04-14 |
| [Catalogue active-track guard + renderer source-file-name + sync_rendered_views multilayer](2026-04-15-1012-catalogue-active-guard-fix.md) | Accepted | 2026-04-15 |
| [TDDD-05: Secondary Adapter variant の追加 — infrastructure 層における hexagonal port 実装の検証](2026-04-15-1636-tddd-05-secondary-adapter.md) | Accepted | 2026-04-15 |
| [TDDD Type Graph View — TypeGraph から mermaid 図をレンダーして型間関係を可視化する](2026-04-16-2200-tddd-type-graph-view.md) | Accepted | 2026-04-16 |
| [TDDD Contract Map — 全層カタログを入力とする統合 mermaid view](2026-04-17-1528-tddd-contract-map.md) | Accepted | 2026-04-17 |
| [型カタログ → 仕様書 signal 評価の有効化 (SoT Chain ②)](2026-04-23-0344-catalogue-spec-signal-activation.md) | — | 2026-04-23 |
| [type-designer Phase 2 reconnaissance step — 設計開始前に baseline + type-graph で既存型インベントリを把握する](2026-04-25-0353-type-designer-reconnaissance-step.md) | — | 2026-04-25 |
| [type-designer reconnaissance のレンダリングオプション既定値 — depth=1+2 + edges=all](2026-04-25-0530-type-designer-recon-options-defaults.md) | — | 2026-04-25 |
| [TDDD struct kind taxonomy の field/method 均質化と type catalogue linter 機構の導入](2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md) | Proposed | 2026-04-28 |
| [method / param 型宣言で generic 引数を含む完全な型文字列を強制する](2026-04-29-0240-method-type-full-generic-declaration.md) | Proposed | 2026-04-29 |
| [typestate 遷移を contract-map に描画する renderer 拡張](2026-04-29-0241-typestate-transition-edge-rendering.md) | Proposed | 2026-04-29 |
| [secondary_adapter が参照する port は当該 track の catalogue に必ず declare する](2026-04-29-0243-cross-track-port-reference.md) | Proposed | 2026-04-29 |
| [Free Function L2 Evaluator: returns 比較を source form に統一する](2026-05-01-0702-free-function-l2-source-form-evaluator.md) | Proposed | 2026-05-01 |
| [Reality View renderer の edge カバレッジ拡張 — receiver-less method / trait-method incoming + 起源別視覚区別](2026-05-01-1226-reality-view-edge-coverage-expansion.md) | — | 2026-05-01 |
| [旧 spec-code-consistency の廃止と catalogue-impl-signals 診断コマンドの導入: レイヤー配置・インターフェース・CI ゲート](2026-05-11-2330-catalogue-impl-signals-command-layering.md) | Proposed | 2026-05-11 |
| [TDDD: struct の inherent method 比較を enum と同じ両側対称比較に統一する](2026-05-20-0413-tddd-struct-inherent-method-symmetric-comparison.md) | Proposed | 2026-05-20 |
| [Contract Map Renderer: catalogue schema v3 対応設計](2026-05-20-2221-contract-map-renderer-catalogue-v3-adaptation.md) | Proposed | 2026-05-20 |
| [Reality View Renderer: rustdoc_types::Crate 入力への対応設計 (v3 schema 移行)](2026-05-22-1507-baseline-graph-renderer-rustdoc-adaptation.md) | Proposed | 2026-05-22 |
| [TDDD カタログ taxonomy の意味論拡張 — パターン固有の機械検査ルールを持たせる](2026-05-25-0000-tddd-pattern-semantics-extension.md) | Proposed | 2026-05-25 |
| [型シグネチャ codec の generic param 名前衝突の恒久対策](2026-05-25-0423-tddd-codec-generic-name-collision-fix.md) | Proposed | 2026-05-25 |
| [完了済みトラック保護を frozen から現在ブランチ紐付きバリデーションへ置換](2026-05-26-0518-active-track-write-guard.md) | Proposed | 2026-05-26 |
| [typestate は struct 形状と直交配置する — 全 struct 形状を typestate 状態にする](2026-05-26-1002-typestate-struct-kind-orthogonal.md) | Proposed | 2026-05-26 |
| [SoT Chain に意味論レビューゲートを追加する](2026-05-27-1601-sot-chain-semantic-review-gate.md) | Proposed | 2026-05-27 |
| [`--lenient` と `--force` の実行経路を削除する](2026-06-01-1206-remove-lenient-and-force-flag-paths.md) | — | 2026-06-01 |
| [`sotp ref-verify results` で verify-cache 直読みを置き換える](2026-06-26-0842-ref-verify-results-command.md) | — | 2026-06-26 |
| [signal CLI 名前空間統一と gate strictness の宣言的管理](2026-06-16-1030-signal-gate-strictness-config.md) | Proposed | 2026-06-16 |
| [TDDD GAT trait サポート: パーサ QualifiedPath + 比較フォーマッター正規化 + カタログ関連アイテムスキーマ](2026-06-18-0822-typeref-parser-qualified-path-support.md) | Proposed | 2026-06-18 |
| [cli 系 3 層への TDDD 適用と既存 linter によるロール配置制約の設定](2026-06-21-1420-cli-layers-tddd-and-role-placement-lint.md) | Proposed | 2026-06-21 |
| [TDDD chain ③ の `cargo rustdoc` 呼び出しに `--document-hidden-items` を追加する](2026-06-27-0440-tddd-rustdoc-document-hidden-items.md) | Proposed | 2026-06-27 |
| [TDDD chain ③ の rustdoc 抽出を track 単位の feature 宣言に基づかせる](2026-07-27-0039-tddd-track-scoped-feature-declaration.md) | Proposed | 2026-07-27 |
| [catalogue の適用範囲を実装の追加・変更に一致させる](2026-07-28-1024-catalogue-scope-is-implementation-delta.md) | Proposed | 2026-07-28 |
| [sotp 生成 JSON のキー順を決定的にする](2026-07-29-0839-deterministic-json-serialization.md) | Proposed | 2026-07-29 |
| [信号機 Yellow/Red 内訳を横断列挙する signal report コマンドを追加する](2026-07-29-0839-signal-report-command.md) | Proposed | 2026-07-29 |
| [signal report の発生単位データ取得方針](2026-07-31-2134-signal-report-occurrence-source.md) | Proposed | 2026-07-31 |
| [型カタログ作成の「生成 + 注釈」への移行 — 意図入力スキャフォールディング API](2026-07-02-1345-catalogue-generation-annotation.md) | — | 2026-07-02 |
| [テスト義務ゲートと obligation-fulfillment 意味論検証 — SoT chain 第三リンクの意味論検証の完成](2026-07-02-0359-test-obligation-and-fulfillment-gate.md) | Proposed | 2026-07-02 |
| [テスト義務ゲートにおける skipped task status レーン](2026-07-11-0802-test-obligation-skipped-status-lane.md) | Proposed | 2026-07-11 |
| [テスト義務ゲートへの登録を機構化し、成果物不在による空振り合格を廃する](2026-07-23-0240-test-obligation-enrollment-mechanization.md) | Proposed | 2026-07-23 |
| [contract-map renderer: `dyn Trait` return/param edge の解決](2026-07-13-0308-contract-map-dyn-trait-return-edge.md) | Proposed | 2026-07-13 |
| [composition root 規範を純 DI に確定し、実践側の逸脱を解消する](2026-07-23-0111-composition-root-pure-di-realignment.md) | Proposed | 2026-07-23 |
| [Composition root 純 DI 化を単一改善イニシアチブと複数独立 track で完遂する](2026-07-23-1318-composition-root-pure-di-migration-initiative.md) | Proposed | 2026-07-23 |
| [DDD・Clean Architectureに整合する型配置と境界依存の再調整](2026-07-24-1001-architecture-pattern-placement-guard-realignment.md) | Proposed | 2026-07-24 |
| [型配置是正における CLI 契約の維持](2026-07-25-0313-architecture-pattern-placement-cli-contract-preservation.md) | Proposed | 2026-07-25 |
| [型契約パイプラインの規範と機構を実挙動に整合させる](2026-07-23-0113-type-contract-pipeline-consistency.md) | Proposed | 2026-07-23 |
| [role × 層マトリクスを機構で強制し ValueObject の層勾配を是正する](2026-07-25-0538-role-layer-matrix-enforcement.md) | Proposed | 2026-07-25 |

### トラック・ワークフロー

| ADR | Status | Date |
|-----|--------|------|
| [計画成果物ワークフローの再構築 — SoT Chain に沿ったフェーズ分離](2026-04-19-1242-plan-artifact-workflow-restructure.md) | — | 2026-04-19 |
| [Phase command 共通構造 + subagent 内部 pipeline 決定](2026-04-22-0829-plan-command-structural-refinements.md) | — | 2026-04-22 |
| [sotp track branch create: main 上の activation commit regression 修正](2026-04-22-1432-branch-create-commit-ordering.md) | — | 2026-04-22 |
| [verification.md を observations.md に改名 — 役割を手動観測ログに限定](2026-04-24-2356-verification-md-rename-observations-md.md) | — | 2026-04-24 |
| [verify チェーンを file 存在ベースの phase 責務分離に揃える](2026-04-27-0324-phase-aware-verify-gates.md) | — | 2026-04-27 |
| [plan-only / activate ワークフローレーンの削除](2026-05-26-1123-remove-plan-only-activate-lane.md) | Proposed | 2026-05-26 |
| [track-id 引数を省略可能にし、省略時は現在ブランチに紐づくアクティブトラックを既定値とする](2026-05-26-1813-track-id-default-active-track.md) | Proposed | 2026-05-26 |
| [review / commit ゲートを型カタログ未生成の段階でも通す — active-gate のシグナル評価を欠損入力に寛容にする](2026-06-01-0406-review-gate-tolerate-missing-catalogue.md) | — | 2026-06-01 |
| [spec-states commit ゲートを spec 成果物未生成の段階でも通す — トラック解決時のシグナル評価を欠損入力に寛容にする](2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md) | Proposed | 2026-06-03 |
| [cargo make ラッパー層の解体 — bin/sotp 直叩きへの一本化](2026-06-05-1535-cargo-make-teardown.md) | Proposed | 2026-06-05 |
| [track ワークフロー telemetry の導入 — tracing + JSONL による事後観測](2026-06-10-1129-track-workflow-telemetry.md) | Proposed | 2026-06-10 |
| [ref-verify のスコープ解決を artifact 存在ベースに一本化する — --context / --layer の削除と Phase 0 コミットゲート誤爆の解消](2026-06-10-1335-ref-verify-existence-based-scope-resolution.md) | Proposed | 2026-06-10 |
| [SoT 本体への参照 hash 埋め込みを廃止し、新鮮度判定を verify-cache の実行時突合に一元化する — spec_refs[].hash の撤去](2026-06-11-1018-spec-ref-embedded-hash-removal.md) | Proposed | 2026-06-11 |
| [feature バッチ消化への既定反転 — per-layer 並列レビューを始動させる](2026-06-22-1327-feature-batch-default-inversion.md) | Accepted | 2026-06-22 |
| [impl 段階の構造的不整合検出時のフェーズ遷移診断スキル](2026-06-26-0503-adr2pr-back-and-forth-skill-definition.md) | Proposed | 2026-06-26 |
| [タスク単位の契約履行 pre-review ゲート — Phase 3 attribution artifact と impl_catalog 信号の binary 再利用](2026-06-27-0852-pre-review-task-contract-conformance-gate.md) | Accepted | 2026-06-27 |
| [remote sync 専用コマンドの新設と git 操作の hexagonal 是正 — switch と pull の分離、意味論 port への全面移管](2026-07-04-0155-git-sync-dedicated-command.md) | Proposed | 2026-07-04 |
| [/track:adr2pr の呼び出し型を引数指定から文脈自動解決に戻す](2026-07-20-1508-adr2pr-argless-context-resolution.md) | Proposed | 2026-07-20 |
| [SoT 再入の順次処理規律 — ルーティング後のフェーズ収束 Prerequisite](2026-07-22-0400-sot-reentry-sequencing.md) | Proposed | 2026-07-22 |
| [ADR 収束に対する ref-verify 要求の除外](2026-07-22-0546-adr-convergence-ref-verify-scope-exemption.md) | Proposed | 2026-07-22 |
| [impl-plan task ステータス遷移後の review refresh](2026-07-22-0633-impl-plan-transition-review-refresh.md) | Proposed | 2026-07-22 |
| [上流収束における意味論検証の chain scope 明確化](2026-07-22-0817-deferred-upstream-semantic-verification.md) | Proposed | 2026-07-22 |
| [dry gate 評価点における設定無効と feature 無効の優先規則](2026-07-22-1541-dry-gate-evaluation-feature-off-precedence.md) | Proposed | 2026-07-22 |
| [per-scope diff ceiling を実装開始前の admission で機構強制する](2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md) | Proposed | 2026-07-28 |
| [タスク間の依存関係を impl-plan で宣言し、batch 順序をその宣言に対して検査する](2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md) | Accepted | 2026-07-29 |
| [`batch-plan.json` の scope 名を review scope 設定に照合し、未知名を fail-closed で拒否する](2026-07-30-0951-batch-plan-scope-name-config-validation.md) | Accepted | 2026-07-30 |
| [`batch-plan.json` の宣言対象を未 settle タスクに限定する](2026-07-30-1022-batch-plan-declaration-domain-unsettled-tasks.md) | Accepted | 2026-07-30 |
| [per-task commit hash の記録時に repository 実在と HEAD 到達可能性を要求する](2026-07-30-2101-per-task-commit-hash-record-time-validation.md) | Accepted | 2026-07-30 |
| [clean な base merge 後の baseline 再取得と同期状態の原子的 lifecycle を定める](2026-08-02-0715-base-merge-cleanup-state.md) | Accepted | 2026-08-02 |

### ADR 運用

| ADR | Status | Date |
|-----|--------|------|
| [ADR 自動導出: SSoT → ADR 候補検出の設計](2026-03-24-0930-adr-auto-derivation-design.md) | Accepted (設計のみ) | 2026-03-24 |
| [ADR decision の根拠 trace 信号機評価 + 個別 lifecycle 管理](2026-04-27-1234-adr-decision-traceability-lifecycle.md) | — (D1 superseded by 2026-06-16-0042) | 2026-04-27 |
| [ADR decision 根拠信号機: review grounding を一件でも持てば 🟡 とする優先規則修正](2026-06-16-0042-adr-signal-review-grounding-precedence.md) | — | 2026-06-16 |
| [ADR baseline の累積刻印とバイト照合 binary check による無断改変検出](2026-07-16-2001-adr-decision-freeze.md) | Proposed | 2026-07-16 |
| [ADR-baseline の review 入口検査を init 刻印の存在確認のみに縮小する](2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md) | Proposed | 2026-07-17 |
| [adr2pr 終端に ADR baseline diff の PR コメント投稿フェーズを追加](2026-07-18-0340-adr2pr-baseline-diff-comment.md) | Proposed | 2026-07-18 |
| [入力決定と pipeline 産決定の二箱分離](2026-07-19-0616-two-box-decision-separation.md) | Proposed | 2026-07-19 |
| [Phase 0 承認後に修正が入った場合は承認前へ戻して再収束する](2026-07-25-0716-phase0-post-approval-reconvergence-lane.md) | Proposed | 2026-07-25 |

### ドキュメント運用

| ADR | Status | Date |
|-----|--------|------|
| [運用ドキュメント断捨離方針 — SoT 一本化と narrative 重複の解消](2026-04-27-0554-doc-reorganization.md) | — | 2026-04-27 |
| [運用ドキュメント再編（統合版）— ルート文書一本化・track/workflow.md 分散・工学規約の conventions 移管](2026-06-15-0025-operational-docs-restructure-unified.md) | — | 2026-06-15 |
| [knowledge/strategy ディレクトリの整理方針](2026-06-17-1321-knowledge-strategy-cleanup.md) | — | 2026-06-17 |
| [同梱運用ドキュメントのアーキテクチャ記述 SSoT 再編](2026-07-17-0247-docs-architecture-ssot-realignment.md) | Proposed | 2026-07-17 |
| [consumer 規約の所有権分離と harness 固定依存の撤去](2026-07-24-0326-consumer-convention-ownership-and-harness-decoupling.md) | Proposed | 2026-07-24 |

### ドメインモデル・型設計 (DESIGN.md 由来)

| ADR | Status | Date |
|-----|--------|------|
| [TrackStatus を tasks から導出](2026-03-11-0000-track-status-derived.md) | Accepted | 2026-03-11 |
| [TaskStatus::Done が CommitHash を所有](2026-03-11-0010-done-owns-commit-hash.md) | Accepted | 2026-03-11 |
| [TaskTransition を明示的 enum コマンドに](2026-03-11-0020-task-transition-enum.md) | Accepted | 2026-03-11 |
| [StatusOverride の自動クリア](2026-03-11-0030-status-override-auto-clear.md) | Accepted | 2026-03-11 |
| [Plan-task 参照整合性を構築時に検証](2026-03-11-0040-plan-task-integrity.md) | Accepted | 2026-03-11 |
| [Fail-closed フック エラーハンドリング](2026-03-11-0050-fail-closed-hooks.md) | Accepted | 2026-03-11 |
| [Shell guard を domain 層に配置 (no trait)](2026-03-11-0060-shell-guard-in-domain.md) | Superseded | 2026-03-11 |
| [INF-20: ShellParser port + ConchShellParser adapter](2026-03-23-1000-shell-parser-port.md) | Accepted | 2026-03-23 |
| [conch-parser for shell AST (vendored, patched)](2026-03-11-0070-conch-parser-selection.md) | Accepted | 2026-03-11 |
| [Guard policy: ban edge-case-producing patterns](2026-03-11-0080-guard-policy-ban-patterns.md) | Accepted | 2026-03-11 |
| [Reviewer model_profiles in agent-profiles.json](2026-03-17-0000-reviewer-model-profiles.md) | Accepted | 2026-03-17 |
| [3-level signals with SignalBasis](2026-03-23-1010-three-level-signals.md) | Accepted | 2026-03-23 |
| [Two-stage signal architecture](2026-03-23-1020-two-stage-signals.md) | Accepted | 2026-03-23 |
| [DiffScope と scope filtering は usecase 層に配置](2026-03-25-0000-diff-scope-in-usecase.md) | Accepted | 2026-03-25 |
| [パス正規化: exact match + fail-closed](2026-03-25-0010-path-normalization-exact-match.md) | Accepted | 2026-03-25 |
| [Review state trust model と metadata.json 自己参照問題](2026-03-24-1200-review-state-trust-model.md) | Superseded | 2026-03-24 |
| [FsTrackStore + review.json: 関心事の分離](2026-03-25-2125-review-json-separation-of-concerns.md) | Superseded | 2026-03-25 |
| [Review Hash スコープ再設計](2026-03-26-0000-review-hash-scope-redesign.md) | Superseded | 2026-03-26 |
| [review.json 分離 + グループ独立レビュー状態](2026-03-29-0947-review-json-per-group-review-state.md) | Superseded | 2026-03-29 |
| [Review System v2: frozen scope 廃止とスコープ独立型レビュー](2026-04-04-1456-review-system-v2-redesign.md) | Accepted | 2026-04-04 |
| [Review System V1 完全撤去 — metadata.json review + V1 review.json codec + escalation + index_tree_hash_normalizing](2026-04-12-1800-reviewstate-v1-decommission.md) | Accepted | 2026-04-12 |
| [review-scope.json に scope 別 briefing 注入機構を追加する — plan-artifacts scope の新設](2026-04-18-1354-review-scope-prompt-injection.md) | Proposed | 2026-04-18 |
| [CLI→domain 直接参照禁止と usecase 経由への一本化](2026-04-30-0848-cli-via-usecase-only.md) | — | 2026-04-30 |
| [composition root を専用 crate (apps/cli-composition) に切り出す](2026-05-27-0110-composition-root-dedicated-crate.md) | Proposed | 2026-05-27 |
| [CLI delivery 側の責務分離 — composition root(wire) と primary adapter(invoke+render) への分解](2026-06-21-1328-cli-composition-split-presentation-layer.md) | Proposed | 2026-06-21 |
| [git 書き込みガードの enforcement を git hooks 層へ移行する](2026-06-10-1630-git-hooks-process-level-enforcement.md) | Proposed | 2026-06-10 |
| [hooksPath 未設定時の runtime fail-closed を agent 実行面の setup preflight に分離する](2026-06-12-1518-hooks-path-setup-fail-closed.md) | Proposed | 2026-06-12 |

### オーケストレーション・エージェント管理

| ADR | Status | Date |
|-----|--------|------|
| [agent-router フックを skill 遵守フックに置換](2026-04-08-1200-remove-agent-router-hook.md) | Accepted | 2026-04-08 |
| [review-fix-lead の provider を選択可能にする (Claude デフォルト、Codex オプション)](2026-05-23-1848-review-fix-lead-codex-migration.md) | Proposed | 2026-05-23 |
| [reviewer capability の provider を選択可能にする (Codex デフォルト、Claude オプション)](2026-05-23-2236-reviewer-provider-selectable-claude-option.md) | Proposed | 2026-05-23 |
| [Claude モデル baseline を Opus 4.7 から 4.8 へ更新する](2026-05-28-2246-claude-opus-4-8-baseline-migration.md) | Proposed | 2026-05-28 |
| [Codex review-fix-lead の hexagonal Rust 化 + 入れ子 reviewer session 失敗の解消 + 自己 dogfooding](2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md) | Proposed | 2026-05-31 |
| [Codex を Claude と同等の SoTOHE オーケストレーターにする設定追加](2026-06-13-0002-codex-orchestrator-settings-addition.md) | Proposed | 2026-06-13 |
| [Claude/Codex 運用文書の .harness SSoT 化](2026-06-30-0425-harness-workflow-ssot-adapters.md) | Proposed | 2026-06-30 |
| [capability exec: profile 駆動の汎用 capability dispatch コマンド](2026-07-12-0510-capability-exec-unified-dispatch.md) | Proposed | 2026-07-12 |
| [外部 provider 実行基盤の修復](2026-07-13-0410-capability-exec-infra-repair.md) | Proposed | 2026-07-13 |
| [外部 agent 呼び出しのコスト削減](2026-07-13-2217-agent-dispatch-cost-reduction.md) | Proposed | 2026-07-13 |
| [codex reviewer runtime の bootstrap 解決リンク（resolve & link）配備](2026-07-18-1359-codex-resolve-and-link-provisioning.md) | Proposed | 2026-07-18 |
| [.claude/agents の description に capability exec 経由を明記する](2026-07-21-1522-agent-md-capability-exec-routing.md) | Proposed | 2026-07-21 |
| [Codex 正規入口の整備](2026-07-22-1149-codex-merge-done-adr-entrypoints.md) | Proposed | 2026-07-22 |
| [reasoning effort に max 段を追加し、限定レーンを Luna Max へ移行する](2026-08-02-0151-codex-reasoning-effort-max.md) | Proposed | 2026-08-02 |

### テスト・CI ツーリング

| ADR | Status | Date |
|-----|--------|------|
| [`cargo make llvm-cov` を nextest 経路に統一する](2026-04-27-0124-llvm-cov-nextest-harness-alignment.md) | — | 2026-04-27 |
| [コード意味重複検出による DRY 防止（discoverability + soft gate）](2026-05-29-1118-semantic-dup-detection-discoverability-gate.md) | — | 2026-05-29 |
| [semantic-dup を活用した DRY 違反の自動検出 capability](2026-06-02-0716-dry-checker.md) | Proposed | 2026-06-02 |
| [dry-checker(sotp dry)の運用修正 — Codex アカウント対応・スキーマ厳格化・インデックス除外・insert と埋め込みの一括化・インデックス永続化](2026-06-04-1042-dry-checker-operability-and-batch-index.md) | Proposed | 2026-06-04 |
| [DFP⇄RFP 往復コストの削減 — dfl ループ効率化 / fixpoint 機械化 / 判定の並列化・較正 2 段 / check-approved 純読み化](2026-06-10-0413-dfp-rfp-loop-cost-reduction.md) | Proposed | 2026-06-10 |
| [DRY ゲートを利用者設定で切り替え可能にし、既定を無効（opt-in）とする](2026-06-19-2335-dry-gate-configurable-default-off.md) | — | 2026-06-19 |
| [長くなった CI の短縮 — ソースを変えずキャッシュ戦略のみ見直す](2026-06-01-0336-ci-shorten-cache-strategy-only.md) | — | 2026-06-01 |
| [モジュールサイズ制限の厳格化と分割リファクタリング](2026-06-06-1609-enforce-module-size-limit-splitting.md) | Proposed | 2026-06-06 |
| [ビルド成果物によるディスク圧迫の解消と dry gate 重量依存の feature flag 化](2026-07-20-1608-disk-footprint-and-dry-feature-gating.md) | Proposed | 2026-07-20 |

### DRY / リファクタ

| ADR | Status | Date |
|-----|--------|------|
| [既存 DRY 違反の一掃 — 横断・既存重複を正典へ集約する](2026-06-19-0924-existing-dry-violation-cleanup.md) | Proposed | 2026-06-19 |
| [catalogue_v2 エントリ型の catalogue linter 適合 refactor](2026-07-04-0525-catalogue-v2-entry-lint-conformance.md) | Proposed | 2026-07-04 |

### Review コマンド / API

| ADR | Status | Date |
|-----|--------|------|
| [`sotp review results` で review.json 直読みを置き換える](2026-04-28-1905-review-results-command.md) | — | 2026-04-28 |
| [scope 分類ロジックの CLI 公開 (classify / files)](2026-04-29-1547-review-scope-lookup-commands.md) | — | 2026-04-29 |
| [PR レビュー結果を解釈せず最新ラウンドのコメントを agent に渡す](2026-05-29-0526-pr-review-comment-passthrough.md) | — | 2026-05-29 |
| [review fixer がスコープ境界を自己解決する — `--scope-files` 廃止](2026-06-01-2300-review-fixer-self-resolve-scope-files.md) | — | 2026-06-01 |
| [レイヤー別 reviewer briefing prompt の導入と review-prompts ディレクトリの再配置](2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md) | — | 2026-06-18 |
| [内容レビューの SoT 別スコープ化](2026-06-30-1549-per-sot-review-scope.md) | — | 2026-06-30 |
| [レビュー負荷軽減 — findings 全件報告と下流 artifact の再記述禁止](2026-07-02-1600-review-load-batch-findings-no-restatement.md) | — | 2026-07-02 |
| [レビュー指示書のカテゴリ閉列挙を半開形式へ改める](2026-07-23-0109-review-briefing-open-category-format.md) | Proposed | 2026-07-23 |
| [機械導出される義務成果物を review 運用成果物として扱う](2026-07-25-0715-derived-obligation-artifact-review-scope.md) | Proposed | 2026-07-25 |
