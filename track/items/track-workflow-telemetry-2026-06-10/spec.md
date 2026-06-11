<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 39, yellow: 0, red: 0 }
---

# track ワークフロー telemetry の導入 — tracing + JSONL による事後観測

## Goal

- [GO-01] track ワークフロー（Phase 0–3 → 実装 → review → commit）の進行をイベント単位で記録し、ボトルネック発見・gate や guard の false positive 検出・テンプレートバグ洗い出しを事後に行えるようにする [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D1]
- [GO-02] telemetry の記録が tracing + tracing-subscriber だけで完結し、OpenTelemetry バックエンドなどの外部インフラなしに offline-first で動作する。将来 OTel Collector との接続余地（timestamp / duration / track_id 等の相関フィールド）は残す [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D2]
- [GO-03] telemetry ログが track dir 配下（`track/items/<id>/logs/telemetry.jsonl`）に置かれ、track のライフサイクル（active → archive）に retention が自動追随する [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3]

## Scope

### In Scope
- [IN-01] subscriber の初期化を `apps/cli-composition` の composition root のみで行い、domain 層への計装は行わない。subscriber 初期化はアプリケーション起動パスで1回のみ実行する [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [conv: knowledge/conventions/hexagonal-architecture.md#CLI as Composition Root] [tasks: T004]
- [IN-02] telemetry ファイルへの書き込みは O_APPEND フラグを立てた単発 write syscall で行い、1 イベント行の上限を 4096 バイトとする。ユーザーランドの lock 機構は使わない（fire-and-forget）。並行 worker からの同時 append による稀な行の乱れは診断用途として許容する [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [tasks: T003]
- [IN-03] 記録対象とするイベント種別: track 操作 subcommand（command・exit code・所要時間・track_id）/ gate 評価（gate 名・verdict・理由要約）/ review・dry round（provider・model・round 種別・所要時間・findings 件数）/ 外部 subprocess（所要時間・retry 回数・verdict parse 失敗）/ hook dispatch の block 時および advisory（注入型）hook の発火時 / 非ゼロ exit（error chain）。ここで理由要約は gate 出力の人が読める要約（D3 の 4096 バイト行上限が長さを規定）であり、D1 の false-positive 内訳分析を JSONL 直読みで行うために記録する [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T002, T004, T005, T006]
- [IN-04] track 帰属を branch-bound で解決する（現在の git ブランチに紐づく track）。track を解決できないコマンド実行（`track/*` 以外のブランチ上の実行）では telemetry を記録しない。track 引数を取らないリポジトリ全域ゲートも、track ブランチ上での実行は当該 track に帰属させる [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T003, T004]
- [IN-05] telemetry を既定 ON とし、`SOTP_TELEMETRY=0` で全記録を無効化（kill switch）、`SOTP_TELEMETRY_DIR` で書き出し先を差し替えられるようにする。初期化は lazy とし、イベントが 1 件も発生しなければ file open を行わない [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D5] [tasks: T003]
- [IN-06] テスト実行時は telemetry を強制隔離する。Makefile の test 系タスクと integration test helper の両方で telemetry env（`SOTP_TELEMETRY=0` または `SOTP_TELEMETRY_DIR` で一時ディレクトリを指定）を設定し、spawn された実バイナリも含めてテスト由来イベントが track dir を汚染しないようにする [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D5] [tasks: T007]
- [IN-07] `bin/sotp telemetry report <track-id>` subcommand を初期スコープに含める。フェーズ別所要時間・エラー一覧・hook block 一覧を集計し、集計ロジックは Rust native とする（cargo make に置かない）。読み手は破損行・未知 `schema_version` の行を fail せず skip し、skip 件数を報告する [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D6] [tasks: T008, T009]
- [IN-08] 各イベント行（JSONL の 1 行）に `schema_version` フィールドを持たせる。スキーマ変更時の互換性判定が行単位で可能であり、file header イベント方式は採用しない [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D7] [tasks: T002]

### Out of Scope
- [OS-01] OpenTelemetry crate の導入。分析が事後・バッチ型でライブ可視化の強みが活きない。collector 運用と per-invocation 接続コストが重い [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D2] [tasks: T002]
- [OS-02] domain 層への計装（span の付与等）。domain 層は計装しないことが D3 で決定されている。関数レベルの細粒度 span も対象外 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [tasks: T004]
- [OS-03] hook の素通り（block も advisory 注入もない allow 経路）の記録。通過が圧倒的多数でノイズになるため対象外。hook の allow 経路にファイル IO を足さない [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T005]
- [OS-04] 純表示系コマンド・findings / briefing 本文の複製・プロンプト本文等の機密になりうる引数の記録。SoT への参照のみを持ち、コンテンツ本文は記録しない。なお、リポジトリ内のファイル path は gitignored のローカル診断ログ（D3）における「機密になりうる引数」に該当しないため、gate 評価イベントへの path 記録は行ってよく redaction は行わない [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T006]
- [OS-05] top-level `logs/telemetry/<track-id>.jsonl` や `_global.jsonl` への集約出力。retention を track ライフサイクルと別に管理しなければならず、テンプレートルートへの top-level dir 増加も不利 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [tasks: T001]
- [OS-06] usecase 層への AuditSink port 新設（監査 grade の確実書き込み）。目的が診断であり、イベントが欠落しても SoT は無傷。port 新設は過剰設計 [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D1] [tasks: T003]
- [OS-07] `track/*` 以外のブランチ（`main` 等）での telemetry 記録。main ブランチ上の作業は track フェーズに帰属しない操作（マージ後確認等）が混在するため、branch-bound 帰属に絞って track 外ノイズを排除する [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T003, T004]
- [OS-08] チャット / orchestrator 層で起きる人手介入（human rescue）そのものの記録。sotp プロセス内のイベントのみが観測対象。人手介入率は blocked 終端・同一コマンドの再実行などの代理指標で近似する [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D1] [tasks: T004]

## Constraints
- [CN-01] telemetry は診断用途であり、コンプライアンス監査グレードの確実書き込みは保証しない。イベントの稀な欠落は許容し、SoT 成果物（metadata.json / review.json / git notes）は telemetry と独立して無傷で残ることを前提とする [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D1] [tasks: T003]
- [CN-02] tracing + tracing-subscriber のみを追加依存とし、opentelemetry-* 系 crate は導入しない。イベントには将来の OTel 接続に備えて timestamp / duration / track_id 等の相関フィールドを含める [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D2] [tasks: T002]
- [CN-03] telemetry の出力先は `track/items/<id>/logs/telemetry.jsonl` とし、`track/items/**/logs/` を gitignore に追加する。archive ツールは gitignored の `logs/` もファイルシステム移動の対象とし、archived track との突き合わせを可能にする [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [tasks: T001, T011]
- [CN-04] subscriber 初期化は `apps/cli-composition` の composition root のみで行い、usecase 層は `std::fs`・`std::env` などへの直接依存を持たない。hexagonal-architecture convention の usecase 層純粋性規則に従う [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [conv: knowledge/conventions/hexagonal-architecture.md#Usecase Layer Purity Rules] [tasks: T004]
- [CN-05] 書き込みは O_APPEND フラグを立てた単発 write syscall で行い、1 イベント行は 4096 バイト以内に収める。超過しうる可変長フィールドは要約して収める。ユーザーランドの lock 機構を追加して fire-and-forget のレイテンシを増やしてはならない [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [tasks: T003]
- [CN-06] テスト実行時（Makefile test 系タスクおよび integration test helper）は telemetry env 変数で隔離する。spawn された実バイナリには `cfg(test)` が効かないため、env 変数（`SOTP_TELEMETRY=0` または `SOTP_TELEMETRY_DIR`）による隔離を行う [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D5] [tasks: T007]
- [CN-07] `bin/sotp telemetry report` の集計ロジックは Rust native に実装する。cargo make のシェルスクリプトにロジックを流出させない [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D6] [tasks: T009]
- [CN-08] report の読み手は破損行・未知 `schema_version` の行を fail せず skip する（fail-open）。skip 件数を出力に含める。lock なしの並行 append で稀に生じうる破損行を読み手側で吸収する [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D6] [tasks: T008]
- [CN-09] 各イベント行に `schema_version` フィールドを必須とする。file header イベント方式（並行 append 時の初回書き込み競合あり）は採用しない。スキーマ変更時の互換性判定は行単位で行う [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D7] [tasks: T002]

## Acceptance Criteria
- [ ] [AC-01] `apps/cli-composition` に tracing subscriber の初期化コードが存在し、domain 層に tracing macro を含む計装コードが存在せず、usecase 層に subscriber の初期化コードが存在しない [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [tasks: T004]
- [ ] [AC-02] `bin/sotp` の track 操作 subcommand 実行後に `track/items/<id>/logs/telemetry.jsonl` へイベント行が追記される。記録される内容は command・exit code・所要時間・track_id を含む [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T004]
- [ ] [AC-03] gate 評価イベントが gate 名・verdict・理由要約を含んで記録される（入力 hash は記録されない）。review / dry round イベントが provider・model・round 種別・所要時間・findings 件数を含んで記録される [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T005, T006]
- [ ] [AC-04] hook dispatch の block イベントおよび advisory（注入型）hook の発火イベントが記録される。hook の素通り（allow）は記録されない [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T005]
- [ ] [AC-05] `SOTP_TELEMETRY=0` を設定した状態で `bin/sotp` を実行したとき、`logs/` ディレクトリへのファイル書き込みが一切行われない。`SOTP_TELEMETRY_DIR` を設定したとき、指定先ディレクトリに telemetry.jsonl が書き出される [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D5] [tasks: T003]
- [ ] [AC-06] イベントが 1 件も発生しない実行パス（例: 純表示系コマンド、hook の allow 経路）では `logs/` ディレクトリへのファイル open が行われない（lazy 初期化） [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D5] [tasks: T003]
- [ ] [AC-07] Makefile の test 系タスクおよび integration test helper が telemetry env 変数を設定しており、テスト実行中に track dir の `logs/telemetry.jsonl` が生成・汚染されない [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D5] [tasks: T007]
- [ ] [AC-08] `bin/sotp telemetry report <track-id>` が実行でき、フェーズ別所要時間・エラー一覧・hook block 一覧を集計して出力する。破損行および未知 `schema_version` を持つ行は skip され（fail-open）、report コマンドは終了コード 0 で完了し、skip 件数が出力に含まれる [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D6] [tasks: T008, T009]
- [ ] [AC-09] telemetry.jsonl の各イベント行が `schema_version` フィールドを含む。file header イベント方式は採用されておらず、スキーマ互換性の判定は行単位で可能である [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D7] [tasks: T002]
- [ ] [AC-10] `track/items/**/logs/` が .gitignore に追加されており、telemetry.jsonl がリポジトリに誤ってコミットされない [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D3] [tasks: T001]
- [ ] [AC-11] `track/*` 以外のブランチ（例: `main`）で `bin/sotp` を実行したとき、telemetry.jsonl への書き込みが行われない [adr: knowledge/adr/2026-06-10-1129-track-workflow-telemetry.md#D4] [tasks: T003, T004]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#CLI as Composition Root
- knowledge/conventions/hexagonal-architecture.md#Usecase Layer Purity Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Error Handling: Result and ? Operator
- .claude/rules/04-coding-principles.md#Make Illegal States Unrepresentable

## Signal Summary

### Stage 1: Spec Signals
🔵 39  🟡 0  🔴 0

