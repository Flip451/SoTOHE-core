<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 32, yellow: 0, red: 0 }
---

# 配布 scaffold の host-first 刷新と Makefile ゼロベース再構成

## Goal

- [GO-01] 配布 scaffold の Makefile を、workflow に必要なオーケストレーションだけから構成され、ソースリポジトリの Makefile と手動同期を要しない状態にする。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D2]
- [GO-02] 配布 scaffold の既定品質ゲートを Docker 非必須の host-first 実行にし、必要な利用者には同じ workflow 契約を保つ Docker 選択肢を提供する。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D3, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D8]
- [GO-03] 配布 scaffold を、固定 Rust toolchain と補助ツール、CI での再現可能な sotp 調達、および個人環境への非依存により、採用直後から再現可能にする。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D4, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D6, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D7]

## Scope

### In Scope
- [IN-01] 配布用 overlay Makefile を、workflow / capability / briefing / process enforcement / 配布 CI / utility command / sotp 案内文から参照されるタスクとその推移的な必要タスクだけでゼロベース再構成する。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1] [tasks: T001, T007]
- [IN-02] 配布 workflow 面を、集約ゲートの `cargo make <task>` と単発操作の `bin/sotp <sub>` の呼び出し規約へ共同更新し、単発 passthrough Makefile wrapper を除去する。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D2, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D3] [tasks: T001, T003, T004, T007]
- [IN-03] 配布 scaffold の既定品質ゲートを host toolchain で直接実行する経路へ変更し、Docker を採用前提から外す。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D3] [tasks: T002, T007]
- [IN-04] 共通の配布 Makefile と対称な host / Docker 環境ファイルを整備し、環境選択を共通部の extend 参照先だけで切り替えられるようにする。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D8] [tasks: T002, T007]
- [IN-05] 配布物の Rust toolchain 固定、bootstrap による補助ツールの固定版導入、および同じ pin を用いる CI 経路を整備する。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D4] [tasks: T006, T007]
- [IN-06] 配布 CI を host runner 上で動作する形へ変更し、CI 内で pinned tag の install-sotp により sotp を調達する。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D7] [tasks: T006, T007]
- [IN-07] bin/sotp track views validate を重複実行する配布 task を、track views 全体を検証する track-metadata 名義の一つの task に統合する。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D5] [tasks: T001, T007]
- [IN-08] 配布面から個人環境に結び付く review 実行探索と未設定の worker 分岐を除去し、Docker 時の並列分離を CARGO_TARGET_DIR_RELATIVE に一本化する。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D6] [tasks: T002, T007]
- [IN-09] 配布文書を採録後の task 集合へ共同更新し、配布文書だけが参照する task を配布 Makefile に残さない。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1] [tasks: T005, T007]

### Out of Scope
- [OS-01] SoTOHE 本体リポジトリの Makefile 実行モデル、docker-first 方針、コンテナ内ビルド、または ci-container 構成の変更。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D3] [tasks: T002]
- [OS-02] 配布 scaffold で Docker を既定または必須の実行環境として維持すること。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D3] [tasks: T002, T005]
- [OS-03] 共通 Makefile を複製した完全独立の Docker 用 Makefile を配布すること、または host 用環境ファイルを継承する非対称な patch 構成。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D8] [tasks: T002]
- [OS-04] 配布物へ sotp バイナリを Git 管理下の成果物として commit すること。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D7] [tasks: T006]
- [OS-05] toolchain または補助ツールのドリフトだけを事前に検査する専用 preflight 機構の導入。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D4] [tasks: T006]

## Constraints
- [CN-01] 配布 Makefile の task 採録は workflow で必要なオーケストレーションとその構成員に限定し、単発の bin/sotp passthrough を残さない。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D2] [tasks: T001, T003, T004]
- [CN-02] 共有 workflow 面は、集約ゲートを cargo make、単発操作を bin/sotp とする呼び出し契約を守り、同名ゲートの実行環境は各リポジトリの Makefile が決める。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D3] [tasks: T001, T003, T004]
- [CN-03] host-first の再現性は固定 Rust toolchain と bootstrap / CI の locked かつ pinned な補助ツール調達で担保し、専用 preflight は導入しない。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D4] [tasks: T006]
- [CN-04] 環境依存ゲートだけを host と Docker の対称な peer 環境ファイルへ分離し、共通部と環境部で task 名を重複させず、Docker 選択時も bin/sotp 系ゲートは host 実行に保つ。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D8] [tasks: T002]
- [CN-06] review 実行用の Codex 解決は CODEX_BIN または command -v codex に限り、未設定の WORKER_ID 分岐は置かず、Docker 時の並列分離は CARGO_TARGET_DIR_RELATIVE に一本化する。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D6] [tasks: T002]
- [CN-07] 配布文書は削除済み task や Docker 必須という旧前提を参照せず、実行機構の挙動を重複した SSoT として維持しない。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D3] [conv: knowledge/conventions/no-upstream-restatement.md#Rules] [tasks: T005, T007]

## Acceptance Criteria
- [ ] [AC-01] export される Makefile の task 集合は、定義済みの workflow 参照面から必要とされる task とその推移的な依存だけで構成され、配布文書だけが参照する task と参照のない task を含まず、その生成または更新にソースリポジトリの Makefile の読取り、複製、手動同期を必要としない。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1] [tasks: T001, T007]
- [ ] [AC-02] workflow 面は単発の staging、sync、branch、PR、note などを bin/sotp へ直接委譲し、配布 Makefile は対応する単発 passthrough task を提供しない。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D2] [tasks: T001, T003, T004, T007]
- [ ] [AC-03] 配布 scaffold の既定構成で、fmt check、clippy、test、deny、および bin/sotp verify 群を Docker なしの host toolchain で実行できる。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D3] [tasks: T002, T007]
- [ ] [AC-04] Docker を選ぶ利用者は共通 Makefile の extend 参照先を host 用から Docker 用へ一行変更するだけで切り替えられ、切替前後で workflow 面と capability 面の呼び出しを変更しない。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D8] [tasks: T002, T007]
- [ ] [AC-05] 配布物に rust-toolchain.toml があり Rust toolchain が固定され、bootstrap と CI は cargo-nextest、cargo-deny などの補助ツールを同じ pinned version かつ --locked で導入する。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D4] [tasks: T006, T007]
- [ ] [AC-06] 配布 CI は host runner 上で固定 toolchain と既存のゲート集約を実行し、gitignore 対象の bin/sotp を commit せず、pinned tag の install-sotp で調達する。CI キャッシュは pinned tag に対応する取得済み sotp を再利用できる。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D7] [tasks: T006, T007]
- [ ] [AC-07] track views 全体の整合検証は track-metadata 名義の一つの task として実行され、同じ bin/sotp track views validate を重複実行する task は残らない。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D5] [tasks: T001, T007]
- [ ] [AC-08] 配布面の review 実行は asdf を参照せず、CODEX_BIN 未設定時には command -v codex を用いる。WORKER_ID 参照はなく、Docker 用の cache directory 準備は Docker 環境ファイルだけに置かれる。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D6] [tasks: T002, T007]
- [ ] [AC-09] 配布文書は新しい host-first / Docker 任意の配布契約に整合し、削除した Makefile task または個人環境を前提とする説明を残さない。 [adr: knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D3, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D6] [tasks: T005, T007]

## Related Conventions (Required Reading)
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/no-upstream-restatement.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/track-lifecycle.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 32  🟡 0  🔴 0

