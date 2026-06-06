<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 39, yellow: 0, red: 0 }
---

# Python 固有ロジックの Rust 完全移行と Python ランタイム依存の撤去

## Goal

- [GO-01] 残存する Python 固有ロジック（architecture_rules.py の workspace-tree / workspace-tree-full / workspace-members / direct-checks、convention_docs.py の add / update-index）をすべて Rust（apps/cli / libs/infrastructure）へ移植し、scripts/ ディレクトリを丸ごと削除する。あわせて Python ランタイム依存（python3 / pytest / ruff / PyYAML / uv）を tools イメージとホスト bootstrap から完全に撤去する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1]
- [GO-02] architecture_rules.py の 4 コマンドを sotp arch サブコマンドグループ（sotp arch tree / sotp arch tree-full / sotp arch members / sotp arch direct-checks）に集約する。4 コマンドはいずれも architecture-rules.json を入力とする同一関心であり、CLI の discoverability と関心の一貫性を保つ [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D2]
- [GO-03] convention_docs.py の add / update-index を sotp conventions add / sotp conventions update-index として新設し、既存の verify-index ロジック（verify/convention_docs.rs）も sotp conventions verify-index として同じ conventions グループ配下に集約する。convention 文書に対する操作（add / update-index / verify-index）の CLI 面を一本化する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D3]
- [GO-04] test_make_wrappers.py（919 行）を Rust に移植せず削除する。Makefile 構成の表層的な assert は regression リスクが低く、orchestra 検証（verify/orchestra.rs）が重要な整合を別途カバーしているため、全移植は労力に見合わない [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D4]
- [GO-05] test_architecture_rules.py と test_convention_docs.py が検証していた観点を、D2 / D3 で新設する sotp arch / sotp conventions サブコマンドの Rust unit / integration テストとして再現し、移行前と同等の coverage を維持する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D5]
- [GO-06] 本 ADR は 2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md が掲げた Phase 3 残作業（convention_docs / architecture_rules 残コマンド移行と scripts/ 削除）を完遂するものと位置づける。ロードマップ ADR の Phase 3 セクションに newer ADR back-reference note を追記することで、置き換え関係を明示する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D6, knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md#フェーズ 3]

## Scope

### In Scope
- [IN-01] architecture_rules.py の workspace-tree / workspace-tree-full / workspace-members / direct-checks の Rust 移植：sotp arch tree / sotp arch tree-full / sotp arch members / sotp arch direct-checks として apps/cli 配下に新設する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D2] [tasks: T001, T003]
- [IN-02] convention_docs.py の add / update-index の Rust 移植：sotp conventions add / sotp conventions update-index として apps/cli 配下に新設する。既存の verify/convention_docs.rs の verify-index ロジックを sotp conventions verify-index として同グループに集約する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D3] [tasks: T002, T004]
- [IN-03] scripts/ ディレクトリの全削除：scripts/architecture_rules.py / scripts/convention_docs.py / scripts/__init__.py / scripts/conftest.py / scripts/test_architecture_rules.py / scripts/test_convention_docs.py / scripts/test_make_wrappers.py / scripts/requirements-python.txt / scripts/ruff.toml を削除する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1] [tasks: T005]
- [IN-04] Dockerfile の Python ランタイム依存削除：python3 / python3-yaml / python3-pytest の apt インストール行と、uv 経由の requirements-python.txt（ruff）導入行を削除する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1] [tasks: T005]
- [IN-05] Makefile.toml の Python 専用タスク削除：python-lint / python-lint-local / scripts-selftest / scripts-selftest-local などの Python 専用タスクを削除し、bootstrap のホスト venv 構築ステップも削除する。python3 を呼んでいた conventions-* / architecture-rules-* / workspace-tree* 経路を Rust-native entrypoint（bin/sotp arch ... / bin/sotp conventions ...）へ同期する。wrapper teardown の要否は cargo-make teardown ADR の方針に従う [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1, knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D3] [tasks: T005]
- [IN-06] CI 重複経路の削除：verify-sync（verify/architecture_rules.rs）と verify-index（verify/convention_docs.rs）の Python スクリプト呼び出し経路を削除する。Rust 検証が到達可能であり Python wrapper が消えることだけを要求し、旧 Makefile タスク名の入口を残すかは cargo-make teardown 側の責務とする [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1] [tasks: T005]
- [IN-07] test_architecture_rules.py（672 行）と test_convention_docs.py（314 行）が検証していた観点（tree レンダリング結果、members 列挙、direct-checks 出力ペア、convention テンプレ生成、index 書換の冪等性など）を、新設 sotp arch / sotp conventions サブコマンドの Rust unit / integration テストとして再現する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D5] [tasks: T001, T002, T003, T004]
- [IN-08] test_make_wrappers.py（919 行）の削除。Rust integration テストへの移植は行わない [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D4] [tasks: T005]
- [IN-09] ロードマップ ADR（2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md）の Phase 3 セクションへ newer ADR back-reference note を追記する。front-matter の semantic 変更（superseded_by 追加等）は post-merge ADR ルールに従い行わない [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D6] [tasks: T007]
- [IN-10] direct-checks の Rust 移植において、出力フォーマットを Python 版と同等に保ち、permission allowlist と orchestra 検証の参照が破綻しないようにする [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D2] [tasks: T001, T003]

### Out of Scope
- [OS-01] external_guides 機能の Rust 化または復元。external_guides は 2026-04-28-1258-remove-external-guides.md の D1 により機能撤去済みであり、本 ADR のスコープ外 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1]
- [OS-02] test_make_wrappers.py の全観点を Rust integration テストに移植すること。大半は Makefile 構成の表層的な assert であり、重要な整合は orchestra 検証等が別途カバーしているため移植しない [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D4]
- [OS-03] cargo make wrapper の長期存続方針の決定。利用者向け入口を Rust-native entrypoint へ寄せることは本 ADR のスコープだが、cargo make タスク名の長期維持や廃止タイムラインの決定は cargo-make teardown ADR の責務 [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D3]
- [OS-04] ロードマップ ADR の front-matter semantic 変更（status: superseded への書き換え、superseded_by の追記）。grandfathered / post-merge ADR に許容されない編集範囲のため実施しない [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D6]
- [OS-05] Python による部分移行（ruff lint を残す等）。scripts/ を削除すればリント対象の Python ソースが消えるため ruff を残す意味がなく、全廃が前提 [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#Rejected Alternatives]
- [OS-06] architecture_rules.py の 4 コマンドを sotp workspace と sotp arch に 2 グループ分割する方式。4 コマンドはすべて architecture-rules.json を入力とする同一関心であり、1 グループへの集約を選択したため対象外 [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#Rejected Alternatives]
- [OS-07] convention の verify-index を現状の Rust 経路（verify 配下）に残し、add / update-index のみ移行すること。convention の add / update-index / verify は同一関心であり conventions グループへの集約を選択したため対象外 [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#Rejected Alternatives]

## Constraints
- [CN-01] ヘキサゴナルアーキテクチャ層依存ルールを遵守する。sotp arch / sotp conventions の実装ロジックは CLI（composition root）と infrastructure 層に配置し、architecture-rules.json の解析・workspace tree の構築・convention テンプレ生成・index 書換などの処理は infrastructure 層に置く。architecture-rules.json を読むロジックも infrastructure 層に置く [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D2] [conv: knowledge/conventions/hexagonal-architecture.md#Layer Dependencies] [tasks: T001, T002, T003, T004]
- [CN-02] sotp arch direct-checks の出力フォーマットは、Python 版（architecture_rules.py の direct-checks）と観測上同等の内容を出力しなければならない。deny.toml の手動保守を支援する authoring aid として permission allowlist と orchestra 検証が参照しており、フォーマット差異が周辺運用に影響しないことを確認する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D2] [tasks: T001, T003]
- [CN-03] scripts/ 削除後、tools イメージとホスト bootstrap が python3 / pytest / ruff / PyYAML / uv を一切要求しない最終状態にする。Dockerfile と requirements-python.txt / ruff.toml のすべての Python 関連エントリを削除した上で、tools イメージのビルドが通ることを確認する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1] [tasks: T005]
- [CN-04] scripts/ 削除と CI・検証ロジック更新は同一変更内で行う。orchestra.rs の許可リストや doc_patterns.rs などと実体が乖離すると CI（verify-orchestra / verify-arch-docs 等）が落ちるため、削除とドキュメント・検証ロジック同期を分割することを禁止する。cargo-make teardown ADR D8 の同期方針に準拠する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1, knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D8] [tasks: T005]
- [CN-05] sotp arch / sotp conventions の新コマンドは、対応する Rust unit / integration テストを同一変更内に含める。test_architecture_rules.py / test_convention_docs.py の検証観点（tree レンダリング、members 列挙、direct-checks 出力ペア、convention テンプレ生成、index 書換の冪等性）が Rust 側でカバーされていることを移行完了の条件とする [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D5] [tasks: T001, T002, T003, T004]
- [CN-06] sotp conventions verify-index は既存の verify/convention_docs.rs のロジックを移動して conventions グループに集約する。verify/convention_docs.rs の内部実装を再実装するのではなく移設する形とし、行動の同等性を保つ [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D3] [tasks: T002, T004]

## Acceptance Criteria
- [ ] [AC-01] scripts/ ディレクトリが存在しない。architecture_rules.py / convention_docs.py / __init__.py / conftest.py / test_architecture_rules.py / test_convention_docs.py / test_make_wrappers.py / requirements-python.txt / ruff.toml のいずれも存在しない [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1] [tasks: T005]
- [ ] [AC-02] tools イメージのビルドが通る。Dockerfile に python3 / python3-yaml / python3-pytest の apt インストール行が存在せず、uv 経由の requirements-python.txt 導入行も存在しない。docker compose run --rm tools bin/sotp --help が正常終了する [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1] [tasks: T005]
- [ ] [AC-03] Makefile.toml に python-lint / python-lint-local / scripts-selftest / scripts-selftest-local タスクが存在しない。bootstrap タスクにホスト venv 構築ステップが含まれない [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1] [tasks: T005]
- [ ] [AC-04] bin/sotp arch tree が実行可能であり、Python 版 workspace-tree と同等のワークスペース構造を出力する。bin/sotp arch tree-full / bin/sotp arch members / bin/sotp arch direct-checks も同様に実行可能であり、Python 版と観測上同等の出力を返す [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D2] [tasks: T001, T003]
- [ ] [AC-05] bin/sotp conventions add が実行可能であり、Python 版 convention_docs.py add と同等の convention 文書テンプレを生成する。bin/sotp conventions update-index も同様に実行可能であり、Python 版と同等の README 索引書換を行う [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D3] [tasks: T002, T004]
- [ ] [AC-06] bin/sotp conventions verify-index が実行可能であり、既存の verify/convention_docs.rs のロジックが conventions グループ配下に集約されている。sotp verify が verify-index 相当の検証を引き続き実行できる [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D3] [tasks: T002, T004]
- [ ] [AC-07] sotp arch / sotp conventions の各サブコマンドに対応する Rust unit / integration テストが存在する。tree レンダリング結果、members 列挙、direct-checks の出力ペア、convention テンプレ生成、index 書換の冪等性のうち、test_architecture_rules.py と test_convention_docs.py が検証していた主要観点がカバーされている [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D5] [tasks: T001, T002, T003, T004]
- [ ] [AC-08] cargo make ci（全体 CI: fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する。特に verify-orchestra / verify-arch-docs が pass し、orchestra.rs の許可リスト等と scripts/ 削除後の実体が整合している [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1, knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D8] [tasks: T005, T006]
- [ ] [AC-09] ロードマップ ADR（2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md）の Phase 3 セクションに、本 ADR（2026-06-03-1327-python-runtime-full-removal.md）への back-reference note が追記されている。本 ADR と Related セクションで相互参照が行われている [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D6] [tasks: T007]
- [ ] [AC-10] スラッシュコマンド / スキル / エージェント定義 / 運用ドキュメントが、削除された cargo make Python wrapper タスク（python-lint / scripts-selftest 等）を参照していない。移行後の入口（bin/sotp arch ... / bin/sotp conventions ...）が参照箇所に反映されている [adr: knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md#D1, knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D8] [tasks: T005, T006]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/hexagonal-architecture.md#CLI as Composition Root
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/no-backward-compat.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Trait-Based Abstraction (Hexagonal Architecture)
- .claude/rules/04-coding-principles.md#Error Handling: Result and ? Operator

## Signal Summary

### Stage 1: Spec Signals
🔵 39  🟡 0  🔴 0

