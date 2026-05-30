<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 32, yellow: 0, red: 0 }
---

# コード意味重複検出による DRY 防止（discoverability + soft gate）

## Goal

- [GO-01] コード片の意味をローカル完結のベクトルDBに保存し、コードを追加・変更する前に意味的に類似する既存コード片を `sotp find-similar` サブコマンドで提示できるようにする（discoverability 補助）。これにより「既存実装を発見できずに再実装する」根本原因を直接解消する [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D1, knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3]
- [GO-02] ローカル完結スタック（`fastembed-rs` × Jina v2 base code × `LanceDB`）でコード埋め込みを実行し、外部 embedding API / クラウドサービスへの必須依存なしに再現可能 CI・オフライン環境での動作を保証する [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2]
- [GO-03] 段階導入（discoverability → PoC 品質実測 → soft gate）により、転移品質の不確実性を制御しながら価値を確かめる。ハードブロック型の強制ゲートは精度実測まで導入しない [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3]

## Scope

### In Scope
- [IN-01] コードフラグメントを受け取りベクトル埋め込みを生成するための infrastructure adapter（`EmbeddingPort` の実装: `FastEmbedAdapter`）を `libs/infrastructure` に追加する。実装は `fastembed-rs`（ONNX Runtime 経由、同期 API、Tokio 非依存）× Jina v2 base code モデルを使用する [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2] [conv: knowledge/conventions/hexagonal-architecture.md#Adapter Rules] [tasks: T005]
- [IN-02] コードフラグメントの埋め込みベクトルをローカルファイルとして永続化・検索するための infrastructure adapter（`SemanticIndexPort` の実装: `LanceDbSemanticIndexAdapter`）を `libs/infrastructure` に追加する。実装は `LanceDB`（ローカルファイルDB、Apache 2.0、公式 Rust SDK）を使用する [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2] [conv: knowledge/conventions/hexagonal-architecture.md#Adapter Rules] [tasks: T006]
- [IN-03] `sotp find-similar` サブコマンドを CLI に追加する。コードフラグメント（文字列またはファイルパス）を入力として受け取り、意味DBから top-k 類似コード片とそのファイルパス・類似度スコアを提示する情報提供のみのコマンドとする（ブロックや警告は行わない） [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D1, knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T003, T008]
- [IN-04] `sotp dup-index build` サブコマンドを CLI に追加する。workspace 内の Rust ソースファイルを走査してコードフラグメントを抽出し、埋め込みを計算してローカル意味DBに登録する（インデックス構築コマンド） [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D1, knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T003, T007, T008]
- [IN-05] PoC 品質実測のために、Jina v2 base code モデルの Rust コードへの転移品質を計測するサブコマンド（`sotp dup-index measure-quality` 相当）または計測スクリプトを追加する。cosine 類似度分布・閾値超過率（ラベルなしワークスペースから計測可能な false positive のプロキシ指標）を出力し、soft gate の閾値設定の判断材料とする [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T003, T009]
- [IN-06] soft gate（warning 止まり・ack 付き override 可）として、追加・変更されたコードフラグメントのみを対象に意味DBを照会し、類似度が閾値を超えるフラグメントが存在する場合に警告を出力するコマンド（`sotp dup-check` 相当）を追加する。soft gate は override コマンドで明示的に抑制できる [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D1, knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T003, T009]
- [IN-07] port trait（`EmbeddingPort`、`SemanticIndexPort`）を domain または usecase 層に定義し、infrastructure adapter がそれを実装する hexagonal 構造を成立させる。どちらの層に置くかは hexagonal-architecture convention の port placement rules に従い type-design フェーズで決定する [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2] [conv: knowledge/conventions/hexagonal-architecture.md#Port Placement Rules] [tasks: T002, T003]

### Out of Scope
- [OS-01] ハードブロック型の強制ゲート（CI/pre-commit でコミットを拒否する機構）。ADR D3 は精度実測まで保留と明示している（Rejected Alternative A） [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T009]
- [OS-02] 外部 embedding API / クラウドサービスへの依存（Rejected Alternative B）。ローカル完結が必須制約 [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2] [tasks: T005, T006]
- [OS-03] 重量級モデル（7B 級）の採用（Rejected Alternative D）。Jina v2 base code（~137M, ~550MB）が案 A として選択されており、nomic-embed-code 等の 7B 級は対象外 [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2] [tasks: T005]
- [OS-04] `canonical_modules` grep 禁止のみによる対処（Rejected Alternative C）。意味レベルの重複は文字列照合で捉えられない [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D1] [tasks: T003]
- [OS-05] ハードゲート化の是非の判断。ADR §Neutral に「本 ADR では保留する（実測後・別途）」と明示されている [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T009]
- [OS-06] `canonical_modules` 機構の要否評価。ADR §Neutral に「本 ADR では扱わない（別トピック）」と明示されている [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D1] [tasks: T003]
- [OS-07] 埋め込みモデル重み（~550MB）のビルドキャッシュ同梱の要否決定。ADR §Negative に「要確認」として明示的に保留されている [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2] [tasks: T005]
- [OS-08] 将来の ANN バックエンド比較・置換（sqlite-vec 等）。ADR §Reassess When に「LanceDB が限界に達した時点で別途検討」と明示されている [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2] [tasks: T006]

## Constraints
- [CN-01] 外部 embedding API / クラウドサービスへの必須依存を持ってはならない。意味DBの構築・検索はすべてローカル実行で完結する [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2] [tasks: T005, T006]
- [CN-02] soft gate は warning 止まり（非ゼロ exit code でブロックしない）とし、ack 付き override 可能にする。`module_size` 検証が warning 止まりである前例と整合する [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D1, knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T003, T009]
- [CN-03] soft gate は追加・変更された差分フラグメントのみを対象とする。全コードベースを毎回スキャンして警告を出す方式は採らない [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T003, T009]
- [CN-04] 新規依存（`fastembed-rs` / `ort` / `lancedb`）は infrastructure 層にのみ追加する。domain 層・usecase 層はこれらクレートに依存してはならない [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2] [conv: knowledge/conventions/hexagonal-architecture.md#Layer Dependencies] [tasks: T004]
- [CN-05] `sotp find-similar` は情報提供のみを行う。類似結果を提示した上でコミットやコード追加を止める動作を本コマンドは行わない [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D1] [tasks: T003, T008]
- [CN-06] インデックス鮮度の管理と ANN の非決定性に対処するため、`sotp dup-check` の soft gate は再現性に影響しない設計にする。具体的には差分フラグメントの固定スナップショット対象と閾値設定で非決定性の影響を封じ込める（ANN の非決定性が CI の再現可能性を破壊してはならない） [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T009]

## Acceptance Criteria
- [ ] [AC-01] `sotp find-similar <fragment>` コマンドが動作し、意味DBに登録済みのコードフラグメントの中から cosine 類似度の高い上位 k 件（k はオプションで指定可能）のファイルパス・フラグメント・類似度スコアを標準出力に表示して終了コード 0 で返す [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D1, knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T003, T008, T010]
- [ ] [AC-02] `sotp dup-index build` コマンドが workspace 内の Rust ソースファイルを走査してコードフラグメントを抽出し、ローカル LanceDB インデックスに登録して終了コード 0 で返す。外部ネットワーク通信なしに完了する [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2, knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T003, T007, T008, T011]
- [ ] [AC-03] Jina v2 base code モデルの Rust コードへの転移品質（cosine 類似度分布・閾値超過率）を計測できるコマンドまたはスクリプトが存在し、実行すると JSON または TSV 形式で計測結果を出力する。なお「閾値超過率」はラベルなしワークスペースデータから計測可能な false positive のプロキシ指標（ランダムサンプリングした異ファイル間フラグメントペアのうち raw cosine が閾値を超える割合）であり、本 PoC では真の false positive 率に代わる指標として使用する [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T003, T009, T010]
- [ ] [AC-04] `sotp dup-check` コマンドが差分フラグメントを対象として意味DBを照会し、類似度が設定閾値を超えるフラグメントがある場合に警告メッセージを標準出力（または標準エラー）に表示する。このコマンドは警告があっても終了コード 0 を返す（soft gate、非ブロック） [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D1, knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T003, T009, T011]
- [ ] [AC-05] `sotp dup-check` は override（ack）コマンドまたはフラグによって警告を抑制できる。ack 後に同一フラグメントで再実行すると警告が表示されない [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D1, knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T009, T011]
- [ ] [AC-06] 新規依存（`fastembed-rs` / `ort` / `lancedb`）が `libs/infrastructure/Cargo.toml` にのみ追加されており、`libs/domain/Cargo.toml` および `libs/usecase/Cargo.toml` にこれらクレートへの依存が存在しない [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2] [conv: knowledge/conventions/hexagonal-architecture.md#Layer Dependencies] [tasks: T004, T010, T011]
- [ ] [AC-07] `cargo make check-layers` および `cargo make deny` が pass する（新規依存の追加後も layer 境界違反がない） [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D2] [conv: knowledge/conventions/hexagonal-architecture.md#Layer Dependencies] [tasks: T004, T010, T011]
- [ ] [AC-08] `cargo make ci` の全項目（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する [adr: knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md#D3] [tasks: T011]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/hexagonal-architecture.md#Port Placement Rules
- knowledge/conventions/hexagonal-architecture.md#Adapter Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Trait-Based Abstraction (Hexagonal Architecture)
- .claude/rules/04-coding-principles.md#Make Illegal States Unrepresentable
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 32  🟡 0  🔴 0

