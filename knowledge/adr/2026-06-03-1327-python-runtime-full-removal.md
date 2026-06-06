---
adr_id: 2026-06-03-1327-python-runtime-full-removal
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-add-python-runtime-full-removal:2026-06-03"
    candidate_selection: "from:[full-removal, partial-keep-ruff, status-quo] chose:full-removal"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:adr-add-python-runtime-full-removal:2026-06-03"
    candidate_selection: "from:[arch-unified, workspace-arch-split, drop-direct-checks] chose:arch-unified"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:adr-add-python-runtime-full-removal:2026-06-03"
    candidate_selection: "from:[conventions-new-with-verify-unified, add-update-index-only] chose:conventions-new-with-verify-unified"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:adr-add-python-runtime-full-removal:2026-06-03"
    candidate_selection: "from:[migrate-behavioral-drop-surface, full-rust-migrate, delete] chose:delete"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:adr-add-python-runtime-full-removal:2026-06-03"
    candidate_selection: "from:[migrate-observations-to-rust-tests, minimal-only] chose:migrate-observations-to-rust-tests"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:adr-add-python-runtime-full-removal:2026-06-03"
    candidate_selection: "from:[supersede-link, related-reference-only] chose:supersede-link"
    status: proposed
---
# Python 固有ロジックの Rust 完全移行と Python ランタイム依存の撤去

## Context

SoTOHE は Python（`scripts/`）から Rust CLI（`bin/sotp` / `apps/cli` / `libs/infrastructure`）への段階的移行を進めてきた。`scripts/verify_*.py` は `sotp verify *` に置換され、`.claude/hooks/*.py` も撤去済み。`scripts/` ヘルパーは移行ロードマップ（`2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md`）で 3 フェーズの削減計画が立てられ、Phase 1（track_branch_guard / markdown / registry / state_machine）・Phase 2（atomic_write）は完了、Phase 3 対象の external_guides も `2026-04-28-1258-remove-external-guides.md` により「Rust 化」ではなく「機能撤去」で解消された。

2026-06-03 時点の棚卸しで、残存 Python は以下のみと判明した。

- 実装: `architecture_rules.py`（435 行）— `workspace-tree` / `workspace-tree-full` / `workspace-members` / `direct-checks` が Rust 未移行（`verify-sync` は `verify/architecture_rules.rs` に移植済みで、Makefile の `architecture-rules-verify-sync` ラッパーは CI を通らない重複）。`convention_docs.py`（321 行）— `add` / `update-index` が未移行（`verify-index` のロジックは `verify/convention_docs.rs` に存在）。
- ユーティリティ: `__init__.py` / `conftest.py`。
- テスト: `test_architecture_rules.py`（672 行）/ `test_convention_docs.py`（314 行）/ `test_make_wrappers.py`（919 行、Makefile 構成検証）。
- 設定: `requirements-python.txt`（PyYAML / ruff）/ `ruff.toml`。

CI ゲート（`ci-rust`）は検証系をすべて Rust（`cargo run -p cli -- verify *`）で実行しており、Python が CI に関与するのは `scripts-selftest-local`（上記 3 テストを pytest 実行）と `python-lint-local`（ruff check scripts/）の 2 経路のみ。Docker の tools イメージは `python3` / `python3-yaml` / `python3-pytest` を apt で入れ、`uv` 経由で `requirements-python.txt`（ruff）を導入している。ホスト `bootstrap` も venv に同じ依存を入れる。

つまり Python ランタイム依存（python3 / pytest / ruff / PyYAML / uv）を残している本体は `architecture_rules.py` の 4 コマンドと `convention_docs.py` の 2 コマンドだけであり、この 6 コマンドを Rust 化すれば `scripts/` ごと削除でき、ロードマップ ADR が目標とした「Python ランタイム依存の完全除去」を達成できる。

## Decision

### D1: Python 固有ロジックの Rust 完全移行と `scripts/` の削除

残存する Python 固有ロジック（`architecture_rules.py` の `workspace-tree` / `workspace-tree-full` / `workspace-members` / `direct-checks`、`convention_docs.py` の `add` / `update-index`）をすべて Rust（`apps/cli` / `libs/infrastructure`）へ移植し、`scripts/` ディレクトリを丸ごと削除する。あわせて Python ランタイム依存を撤去する。

- `Dockerfile`: `python3` / `python3-yaml` / `python3-pytest` の apt インストールと、`uv` 経由の `requirements-python.txt`（ruff）導入を削除する。
- ルート設定: `requirements-python.txt` と `ruff.toml` を削除する。
- `Makefile.toml`: `python-lint` / `python-lint-local` / `scripts-selftest` / `scripts-selftest-local` など Python 専用タスクを削除し、`bootstrap` のホスト venv 構築ステップも削除する。python3 を呼んでいた `conventions-*` / `architecture-rules-*` / `workspace-tree*` 経路は Rust-native entrypoint（`bin/sotp arch ...` / `bin/sotp conventions ...`）へ同期する。non-git の `cargo make` wrapper をタスク名維持のために残すか削除するかは、`2026-06-05-1535-cargo-make-teardown.md` の wrapper teardown 方針に従う。
- CI 移行済みで重複している `verify-sync`（`verify/architecture_rules.rs`）と `verify-index`（`verify/convention_docs.rs`）の Python スクリプト呼び出し経路を削除する。旧 Makefile タスク名（`architecture-rules-verify-sync` / `conventions-verify-index`）の入口を残すかは cargo-make teardown 側の責務であり、本 ADR は同等の Rust 検証が到達可能で Python wrapper が消えることだけを要求する。

最終状態は「`scripts/` が存在せず、tools イメージとホスト bootstrap が python3 / pytest / ruff / PyYAML / uv を一切要求しない」こととする。

### D2: architecture_rules.py の 4 コマンドを `sotp arch` に集約する

`workspace-tree` / `workspace-tree-full` / `workspace-members` / `direct-checks` を、`sotp arch tree` / `sotp arch tree-full` / `sotp arch members` / `sotp arch direct-checks` として 1 つの `arch` サブコマンドグループに集約する。4 コマンドはいずれも `architecture-rules.json` を入力とする同一の関心（ワークスペース構造とレイヤー方針の SSoT 読み出し）であり、単一グループに置くことで CLI の discoverability と関心の一貫性を保つ。

既存の `workspace-tree` / `workspace-tree-full` / `architecture-rules-workspace-members` / `architecture-rules-direct-checks` 利用箇所は、cargo-make teardown ADR の D3 / D8 に従って `bin/sotp arch ...` の Rust-native entrypoint へ同期する。移行順序上、一時的な互換 wrapper が残る場合も、その実装は同じ Rust サブコマンド呼び出しに限定し、長期 API としての `cargo make` タスク名維持は本 ADR では要求しない。`direct-checks` は自動消費先こそ無いが deny.toml の手動保守を支援する出力であり、permission allowlist と orchestra 検証の参照は wrapper teardown 側の同期対象としつつ、コマンドの挙動は保ったまま移植する。

### D3: convention_docs.py を `sotp conventions` に新設し verify を一本化する

`add`（convention 文書のテンプレ生成）と `update-index`（README 索引の書換）を `sotp conventions add` / `sotp conventions update-index` として新設する。さらに既に Rust 化済みの `verify-index` ロジック（`verify/convention_docs.rs`）も `sotp conventions verify-index` として同じ `conventions` グループ配下に集約し、convention 文書に対する操作（add / update-index / verify-index）の CLI 面を一本化する。これにより「convention の索引整合」という関心が 1 グループにまとまり、将来の保守が単純になる。

### D4: test_make_wrappers.py を削除する

`test_make_wrappers.py`（919 行）は Makefile のタスク名・引数列・wrapper 構成を表層的に assert する検証であり、Makefile は宣言的構成で regression リスクが低い。orchestra 検証（`verify/orchestra.rs`）が permission allowlist や delegation の重要な整合を別途カバーしているため、本ファイルは Rust に移植せず削除する。

### D5: 既存 Python テストの検証観点を新サブコマンドの Rust テストに移植する

`test_architecture_rules.py`（672 行）と `test_convention_docs.py`（314 行）が検証していた観点（tree レンダリング結果、members 列挙、direct-checks の出力ペア、convention テンプレ生成、index 書換の冪等性など）を、D2 / D3 で新設する `sotp arch` / `sotp conventions` サブコマンドの Rust unit / integration テストとして再現する。コマンドの Rust 化に伴いテストも Rust 側へ移し、移行前と同等の coverage を維持する。

### D6: ロードマップ ADR の残 Phase 3 を本 ADR で完遂し supersede 意図を記録する

本 ADR は `2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md` が掲げた段階的移行の残作業（Phase 3 の `convention_docs` / `architecture_rules` 残コマンド移行と `scripts/` 削除）を完遂するものと位置づける。ロードマップ ADR は grandfathered かつ post-merge の ADR であり、front-matter の `status` 変更や `superseded_by` 追加のような semantic 変更は行わない。置き換え関係は本 ADR の D6 decision と、ロードマップ ADR 本文の Phase 3 セクションへ追加する newer ADR back-reference note で明示する（back-reference 追加は、本 ADR の commit 時に adr-editor サブエージェント経由で適用する）。両 ADR は `## Related` で相互参照する。

## Rejected Alternatives

### A. 現状維持（`scripts/` 放置）

本番は sotp・テストは Python というねじれを温存する案。却下理由: スキーマ変更のたびに Rust / Python の両方を更新する必要があり不整合リスクが残る。また `scripts/` が存在する限り tools コンテナの python3 / pytest / ruff / PyYAML 依存も消えず、ロードマップ ADR が示した移行方針に逆行する。

### B. 部分移行（ruff lint を残す）

impl コマンドだけ Rust 化し、ruff によるリントを残す案。却下理由: `scripts/` を削除すればリント対象の Python ソースが消えるため、ruff を残す意味がない。`requirements-python.txt` / `uv` / Docker の Python 依存も残り、「完全撤去」という目的を達成できない。

### C. workspace / arch サブコマンド分割（roadmap 案）

`sotp workspace tree/members` と `sotp arch direct-checks` の 2 グループに分ける案。却下理由: 4 コマンドはいずれも `architecture-rules.json` を入力とする同一関心であり、2 グループに分けると discoverability が下がる。`sotp arch` 1 グループへの集約の方が CLI 構造が一貫する。

### D. direct-checks を drop

自動消費先が無い `direct-checks` を移行せず撤去する案。却下理由: deny.toml の手動保守を支援する authoring aid として価値があり、permission allowlist と orchestra 検証が存在を前提にしている。撤去は機能損失かつ周辺更新コスト（settings.json / orchestra 期待値の修正）を生むため、`sotp arch direct-checks` として挙動を保つ方が安全。

### E. convention の verify を一本化しない

`add` / `update-index` のみ移行し、`verify-index` を現状の Rust 経路（`verify` 配下）に残す案。却下理由: convention の add / update-index / verify は同一関心であり、`sotp conventions` グループへの集約の方が CLI 面が一貫し、将来の保守が単純になる。

### F. test_make_wrappers.py の全観点を Rust 移植

919 行の検証観点をすべて Rust integration テストに移植する案。却下理由: 大半は Makefile の構成（タスク名・引数列）を表層的に assert するもので、Makefile は宣言的構成であり regression リスクが低い。重要な整合は orchestra 検証等が別途カバーしており、全移植は労力に見合わない。

### G. roadmap ADR を Related 参照のみ

ロードマップ ADR の status を変えず Related 参照だけにする案。却下理由: grandfathered / post-merge のロードマップ ADR では front-matter の semantic 変更を行わないが、単なる Related 参照だけでは本 ADR が残 Phase 3 を完遂する置き換え関係が弱い。本 ADR の D6 decision とロードマップ ADR 本文への newer ADR back-reference note の組み合わせで traceability を保つ。

## Consequences

### Positive

- Python ランタイム依存（python3 / pytest / ruff / PyYAML / uv）が完全に消え、tools イメージが軽量化し、ホスト `bootstrap` の venv ステップも不要になる。
- 本番 sotp・テスト Python の二重実装のねじれが解消し、保守対象が Rust 一本に統合される。
- `sotp arch` / `sotp conventions` の新設で CLI 面が一貫し、discoverability が上がる。
- CI から `scripts-selftest` / `python-lint` が消え、`ci-rust` が単純化・高速化する。
- ロードマップ ADR の Phase 3 が完遂し、`scripts/` ディレクトリが消滅する（移行プロジェクトの完了）。

### Negative

- workspace tree レンダリング / convention テンプレ生成 / index 書換を Rust で書き直す一回限りの実装コスト。
- `test_make_wrappers.py` の削除により、Makefile 構成の一部 regression coverage が失われる（orchestra 検証等の既存カバーに依存する）。
- `direct-checks` 等の出力を Rust で正確に再現する必要があり、出力フォーマットの差異が周辺運用（permission / deny.toml 手動保守）に影響しないか確認が必要。

### Neutral

- Python runtime removal は長期の `cargo make` wrapper 保持方針を決めない。利用者向け入口は cargo-make teardown ADR に従って Rust-native entrypoint へ寄せるが、本 ADR の範囲では Python 呼び出しが消え、同等の `sotp arch` / `sotp conventions` 挙動が残ることが中立的な互換条件になる。

## Reassess When

- `bin/sotp` のビルドが恒常的に不可能になり、Python fallback が再び本番パスで必要になった場合。
- workspace tree レンダリング / convention テンプレ生成 / index 書換を Rust で再現するコストが想定を大きく超え、Python 維持の方が合理的と判明した場合。
- `direct-checks` の出力フォーマットや deny.toml 手動保守の運用が大きく変わり、Rust 再実装の前提が崩れた場合。
- Python エコシステム固有の機能（複雑な外部 API 連携など）が必要になり、Rust より Python が適切な領域が再び生じた場合。

## Related

- `knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md` — 本 ADR が残 Phase 3 を完遂するロードマップ。
- `knowledge/adr/2026-04-28-1258-remove-external-guides.md` — Phase 3 の external_guides を機能撤去で解消した先行 ADR。
- `knowledge/adr/2026-04-09-2323-python-hooks-removal.md` — `.claude/hooks/*.py` の撤去（Python 依存除去の companion）。
- `knowledge/adr/` — ADR 索引。
- `knowledge/conventions/adr.md` — ADR 運用ルール。
