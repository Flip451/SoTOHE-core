---
adr_id: 2026-07-08-1405-grandfathered-tech-and-product-baseline
decisions:
  - id: D1
    grandfathered: true
    status: accepted
  - id: D2
    grandfathered: true
    status: accepted
  - id: D3
    grandfathered: true
    status: accepted
  - id: D4
    grandfathered: true
    status: accepted
  - id: D5
    grandfathered: true
    status: accepted
  - id: D6
    grandfathered: true
    status: accepted
  - id: D7
    grandfathered: true
    status: accepted
  - id: D8
    grandfathered: true
    status: accepted
  - id: D9
    grandfathered: true
    status: accepted
  - id: D10
    grandfathered: true
    status: accepted
  - id: D11
    grandfathered: true
    status: accepted
  - id: D12
    grandfathered: true
    status: accepted
  - id: D13
    grandfathered: true
    status: accepted
  - id: D14
    grandfathered: true
    status: accepted
  - id: D15
    grandfathered: true
    status: accepted
  - id: D16
    grandfathered: true
    status: accepted
---
# 技術スタック・製品ガイドラインの grandfathered baseline

## Context

`2026-07-08-1020-retire-todo-marker-state-and-track-docs.md` §D2 は、track 直下の技術スタック文書と製品ガイドライン文書 (`track/tech-stack.md` / `track/product-guidelines.md`) を廃止し、これら文書に載っていた「決定として現在も生きている内容」を ADR 層に昇格することを決めている。同 ADR は昇格経路として (a) 内容ごとに関連 ADR へ integrate、(b) `grandfathered: true` の集約 ADR 1 枚に載せる、のいずれか (混在可) を認めており、本 ADR は path (b) の集約 ADR として起草されたものである。

`grandfathered: true` を用いる根拠は `knowledge/conventions/adr.md` §Grandfathered 用途と整合する: 廃止対象文書は本 ADR front-matter フォーマット (`user_decision_ref` / `review_finding_ref` を要求する schema) 採択以前に author され、個別 decision の承認に至った具体的な chat segment / review finding を遡って再構成するコストが高い。各 decision の「現行のコードベース / 運用がそう振る舞っている」という事実自体は観察可能である。

対象は「決定として現在も生きている運用・設計方針」に限定した。元文書中の運用ログ・変更履歴・特定 crate の具体版数の羅列は含めない。版数参照方針 (どの調査ノートを SoT とするか、といった運用決定) は昇格するが、具体版数は `Cargo.toml` および `knowledge/research/version-baseline-*.md` 側に残す。

## Decision

### D1: 実行モデルは Rust stable + Edition 2024 + 同期

処理系は Rust stable 最新安定版、edition は 2024 を採る。CLI / Repository surface の実行モデルは同期とし、domain / usecase / infrastructure の Repository 契約も同期トレイトを前提として書く。`tokio` / `async-std` 等で CLI や Repository 契約自体を async 化する必要が出た場合は、runtime 選択に加えて Repository 契約の変更を伴うため、まとめて再評価する。

この同期の既定に対し、**境界 adapter が async 前提のライブラリを同期 port の背後で橋渡しするために infrastructure 内部にローカルな Tokio runtime を保持する局所例外**は、個別 ADR が明示的に authorise した範囲で認める。これらの局所例外は CLI / Repository surface を async 化する決定ではなく、既定 (同期) を上書きしない。既存の該当例外は `knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md` (元 authorization) および同 adapter の運用を拡張する `knowledge/adr/2026-06-02-0716-dry-checker.md` / `knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md` が扱う semantic-dup / dry-checker の LanceDB adapter (LanceDB の async API を同期 `SemanticIndexPort` の背後で橋渡しする infrastructure-local runtime) であり、本 grandfathered baseline はこれを取り消さない。以後、同種の局所例外を追加する場合は、当該 adapter を導入する新規 ADR で個別に authorise する。

MSRV の具体値は `Cargo.toml` を SoT とし本 ADR には固定しない。

### D2: Workspace は 6 crate の layered / hexagonal 構成

`libs/domain` (最内層) / `libs/usecase` / `libs/infrastructure` の 3 layered crate に、CLI delivery を `apps/cli` (bin) / `apps/cli-composition` (composition root) / `apps/cli-driver` (primary adapter: invoke + render) の 3 crate に分けた 6 crate 構成を採る。依存方向は `domain ← usecase ← infrastructure`、`apps/*` は composition から下流へ配線する片方向。強制の SSoT は `architecture-rules.json` とし、`deny.toml` と `sotp verify layers` (`cargo make check-layers` / `cargo make deny`) がそこから派生する。

### D3: Domain モデリングは Enum/Struct + Newtype + thiserror

domain 型は Rust の enum / struct を素直に用い、primitive obsession を newtype で解消する。domain error は `thiserror` の `#[derive(Error)]` で declarative に定義し、`Display` は日本語ユーザー向けメッセージ、`Debug` は開発者向けとして両輪で使う。domain は原則 I/O を持たないが、`DateTime<Utc>` のような純粋ユーティリティ型は newtype でラップして domain 内で利用してよい。

### D4: 永続化は JSON ファイル (RDBMS / KVS / マイグレーション不採用)

**本 decision の対象は authoritative SoT の永続化** (track artifact / metadata / signal / review state など、リポジトリに commit する SoT ファイル) に限定する。この対象範囲において、永続化方式は JSON ファイルとする。RDBMS (sqlite / postgres) / 埋め込み KVS (redb / sled) / マイグレーションエンジン (sqlx-migrate 等) は、これら SoT の永続化方式としては導入しない。JSON は git diff / 手動 inspection 適正が高く、CLI 単発実行の workflow に適合する。永続化 layer が domain-crossing になった場合は D2 に沿って port trait 側で抽象化する。

上記の JSON 制約は authoritative SoT に限られ、**個別 ADR が生成系ローカル成果物として明示的に authorise した再構築可能な派生 index はこの対象外**とする。これらの派生 index は gitignore 済みのローカルキャッシュ (再生成可能・commit 対象外) であり、authoritative SoT の permanent storage を代替する趣旨のものではない。既存の該当例外は `knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md` および `knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md` が扱う semantic-dup / dry-checker の LanceDB vector index (`.semantic_index/` 配下、ワーキングツリー内容ハッシュで増分維持する再構築可能な補助 index) であり、本 grandfathered baseline はこれを取り消さない。以後、同種の生成系 index を追加する場合は、当該 index を導入する新規 ADR で個別に authorise する。

### D5: Delivery は CLI 単独 (Web framework 不採用) / 外部 HTTP は blocking

配布物は CLI バイナリ `sotp` 単独とし、Web framework (axum / actix / warp) は採用しない。CLI 引数パースは `clap` を SoT とする。外部 HTTP 呼び出しが必要な場合は `reqwest` の blocking feature を用いる。async runtime を持ち込まない D1 の帰結として、HTTP client も blocking で統一する。

### D6: Observability は tracing 系、メトリクス基盤は不採用

構造化ログは `tracing` + `tracing-subscriber` に統一する。span-based で workflow 観測を行う。`log` crate / `env_logger` 系の非構造化ログは採用しない。メトリクス収集基盤 (prometheus / opentelemetry) は導入しない — CLI 単発実行が主で、メトリクス集約が第一義の情報源にならない。

### D7: 開発ツールチェーン

Rust 開発の工程別ツールは以下で固定する。個別 crate の版数選択は D9 に委ねる。

| 工程 | ツール |
|---|---|
| タスクランナー | `cargo-make` (Makefile.toml) |
| テストランナー | `cargo-nextest` |
| 静的解析 | `cargo clippy` (deny warnings) |
| フォーマッタ | `rustfmt` (rustfmt.toml) |
| 依存監査 | `cargo-deny` (deny.toml) + `cargo-machete` |
| カバレッジ | `cargo-llvm-cov` |

CI と developer inner loop の再現性を Docker Compose 経由で担保する前提として、ホスト直呼び (`-local` サフィックスタスク) は実装詳細扱いとする。

### D8: Dev-only tooling は nightly rustdoc JSON を持ちうる

crate 本体の toolchain は stable のまま維持する。ただし domain crate の pub API を JSON 抽出する dev-only 用途 (`sotp domain export-schema` 等) では nightly rustdoc の `-Z unstable-options --output-format json` を利用してよい。nightly を要求するのは rustdoc JSON 生成のみで、通常のビルド / テスト経路は stable で完結する。nightly 不在時は fail-closed (`SchemaExportError::NightlyNotFound` 相当) とし、silent fallback は許容しない。

### D9: 版数の SoT は `Cargo.toml` と `knowledge/research/version-baseline-*.md`

crate の具体版数、Rust toolchain 版数、開発ツールの版数の SoT は 2 面で管理する: (a) 実行に効く版数は `Cargo.toml` / `rust-toolchain.toml` / `Dockerfile` / `Makefile.toml` 側、(b) 選定判断の根拠は `knowledge/research/version-baseline-YYYY-MM-DD.md` 形式の調査ノート側。ADR / convention / README には具体版数を書かず、参照経路を示すに留める。定期的な調査 (プロジェクト開始時 / 大幅アップグレード時) を researcher capability で実行し、その版数ベースラインを Cargo.toml 等へ反映してから実装に入る。

### D10: 認証・機微データ扱いは製品スコープ外

パスワードハッシュ (`argon2` / `bcrypt`)、トークン発行・検証 (`jsonwebtoken`)、セッション管理などの authentication / authorization 機能は組み込まない。SoTOHE-core は開発者ローカル CLI を想定しており、機微情報の永続化・伝送は責務外。必要な API key / 認証情報は環境変数 / OS keychain など外部機構に委譲する。

### D11: 製品設計原則: SoT + CQRS / CLI-first / AST ガードレール

3 つの直交する設計原則を採る。

1. **SoT + CQRS**: 限られた SSoT (JSON) を authoritative とし、そこから派生する md ビューは読み取り専用として生成する。ビューへの直接書き込みは禁止し、CQRS の書き込み経路は SSoT のみを対象とする。
2. **CLI-first**: すべての operational 操作は CLI コマンド (`bin/sotp` サブコマンドおよび `cargo make` タスク) から起動できる状態を保つ。GUI や外部管理コンソールは前提としない。
3. **AST ベースの厳格なガードレール**: 文字列 pattern ではなく AST 解析ベースの検証を hook / verify 経路に埋め込み、機構による強制を優先する (`knowledge/conventions/enforce-by-mechanism.md` 側に系統的な規範がある)。

### D12: API 出力デフォルトは JSON、派生ビューは md

CLI サブコマンドの標準出力は JSON を第一とする (人間可読形式は補助扱い)。md ビュー (`plan.md` / `registry.md` / `contract-map.md` 等) は SSoT からの生成物として writer が emit する。JSON を base にすることで下流 tool (jq / 他 CLI) との pipe 化・機械処理を容易にする。

### D13: エラーハンドリング方針

エラー処理の姿勢は 3 点で固定する。

1. **ユーザー向けエラーは明確 & 行動可能**: どのファイル / どのフィールドが失敗したか、次に何をすればよいかが読み取れるメッセージにする。
2. **内部エラーは詳細をログ、ユーザーには抽象化**: スタックや内部データ構造の生値をユーザー表示に混ぜない。詳細は tracing ログに残す。
3. **公開エラーは `# Errors` 付き rustdoc 化**: `pub fn` / `pub trait method` が返しうるエラー variant を `# Errors` セクションで列挙する。

### D14: パフォーマンス目標: CLI コマンド応答 500ms 以内

CLI サブコマンドの応答時間は 500ms を上限目標とする。track / registry / plan / signal 系の日常操作を interactive に感じられる粒度に抑える。大量ファイル走査や外部プロセス起動を伴う長時間コマンド (verify 系、full CI 実行) はこの上限外だが、その場合も progress 表示や structured log で観測可能にする。

### D15: セキュリティガイドライン

以下を規範として組む。

1. **シークレット非ハードコード**: 認証情報 / API key / URL に混ぜた credential をコードに埋めない。環境変数 / 外部管理システム経由で受け取る。
2. **外部入力はドメイン型で検証**: CLI 引数・ファイル入力・環境変数などの外部入力は domain 型の newtype コンストラクタ (fail-closed) を通してから domain / usecase 層に渡す。
3. **SQL クエリはパラメータバインド**: 現時点で RDBMS は採用していない (D4) が、将来 SQL を導入した場合の規範として、文字列連結ではなくパラメータバインド API を用いる。
4. **詳細なエラーをユーザーに露出しない**: D13 の内部エラー抽象化と整合する形で、SQL error / IO error の生メッセージを外部に漏らさない。

### D16: コード品質基準

コミット / PR merge の前提条件として以下を satisfy する状態を維持する。

| 項目 | 基準 |
|---|---|
| 静的解析 | `cargo make clippy` (deny warnings) がクリーン |
| フォーマット | `cargo make fmt-check` が pass |
| カバレッジ (新規コード) | 80% 以上を目標 (line coverage) |
| pub 項目のドキュメント | `pub struct` / `pub fn` / `pub trait` (公開 method) すべてに `///` docstring |

pub 項目 docstring は D13 の `# Errors` 要件と組み合わせ、公開 API surface が rustdoc 出力上で自立する状態を作る。

## Rejected Alternatives

### 非同期 CLI (D1 / D5 反対)

`tokio` / `async-std` を使って CLI / Repository surface を非同期化する経路。並行 I/O の実需が現時点で無く、runtime 導入 + Repository 契約の async 化 (D1 の同期 Repository 契約変更) コストが利得を上回るため却下。LanceDB の async API を同期 port の背後で橋渡しする infrastructure-local runtime は、この反対案の対象外とする。

### RDBMS / KVS 永続化 (D4 反対)

`sqlite` / `postgres` / `redb` / `sled` などを authoritative SoT 永続化に採用する経路。track / metadata は小規模ファイル永続化で足り、Human-readable な JSON の方が git diff / 手動 inspection 適正が高い。DB を SoT storage に導入すると migration engine / connection pool / SQL 生成の全経路が新たな責務として増える。semantic-dup の再構築可能な補助 index はこの反対案の対象外とする。

### Web framework 内蔵 (D5 反対)

`axum` / `actix-web` / `warp` を組み込んで HTTP surface を持つ経路。CLI 単独配布を前提とする SoTOHE-core では Web layer は out of scope。

### 汎用ログ crate (D6 反対)

`log` + `env_logger` 系の非構造化ログ。span / structured field を持つ tracing の方が workflow 観測 (track lifecycle / phase transition) に必要な粒度を提供する。

### 別タスクランナー / メトリクス基盤 (D6 / D7 反対)

`xtask` / `just` / GNU Make による task runner、`prometheus` / `opentelemetry` によるメトリクス収集経路。前者は Docker Compose ラッパーとの整合、後者は CLI 単発実行との整合の観点で cargo-make + tracing の組み合わせに劣る。

### 版数の ADR 直書き (D9 反対)

crate の具体版数を ADR / convention / README 本文に列挙する経路。`Cargo.toml` との二重管理になり drift の温床。版数の SoT は 2 面 (Cargo.toml + version-baseline 調査ノート) で管理する。

### 認証機能内蔵 (D10 反対)

`argon2` / `jsonwebtoken` を組み込んで認証機構を内包する経路。開発者ローカル CLI の責務外であり、外部機構への delegation で足りる。

### md ファイルを SSoT とする運用 (D11 / D12 反対)

md ファイル (spec.md / plan.md) を authoritative source として直接編集する運用。ビューへの直接書き込みは D11 の SoT + CQRS 原則に反し、writer / view の役割分離を崩す。

### 統一的な performance budget を持たない運用 (D14 反対)

コマンドごとに応答時間目標を持たない運用。CLI-first (D11) の下で日常操作の interactive 性を保つには全体的な上限が必要。

## Consequences

### Positive

- 技術スタック / 製品設計の baseline decision が ADR 層に一元化され、廃止対象の track 直下文書との二重管理が消える。
- 個別 decision (Rust / architecture / persistence / delivery / observability / tooling / dev-only tooling / version 参照 / auth / design principles / API / error handling / performance / security / code quality) が front-matter を持つ機械検証対象になる (`bin/sotp signal check-adr-user`)。
- 新規 track が同種の判断を再演せず、ADR 索引経由で参照できる。

### Negative

- 個々の decision の chat / review 由来 trace は再構成しないため、grandfathered 期 (本 front-matter フォーマット採択前) の判断根拠は「元文書の記述 + 現行コードの振る舞い」からの逆算に留まる。この点は grandfathered exemption の性質上 acceptable と扱う。
- 16 decision を単一 ADR にまとめたため、個別 decision を後日 supersede する場合は「本 ADR §Dn を supersede する」形の後続 ADR で record することになる (超粒度の分解 ADR を後から作らない前提)。

### Neutral

- 具体版数を書かないため、この ADR は「どの crate を選んだか」の記録には残るが、「どの版でどう振る舞ったか」は Cargo.toml / lockfile / version-baseline 調査ノート側を参照する必要がある。

## Reassess When

- **D1 / D5**: 並行 I/O を伴うユースケース (並列 HTTP fetch / streaming) が実需として出たとき。async runtime + Repository 契約の変更をまとめて評価する。
- **D2**: hexagonal 層構成そのものを見直す必要が出たとき (別バイナリの追加、GUI 併設等)。`architecture-customizer` skill 経由で `architecture-rules.json` から派生させる。
- **D4**: 永続化サイズが JSON 走査の性能限界に達したとき、または横断 query が必要になったとき。migration 経路も含めて別 ADR で扱う。
- **D6**: メトリクス収集が第一義の観測情報になるユースケース (long-running daemon 化等) が出たとき。
- **D9**: 版数管理のオーバーヘッド (Cargo.toml と調査ノートの drift 検知等) が問題化したとき。
- **D10**: SoTOHE-core を配布物として authentication を要する surface に載せるとき (現状想定外)。
- **D14**: 500ms 上限が制約になるコマンドが増えたとき、または上限を守れないコマンド分類ができたとき。

## Related

- `knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md` — 本 ADR の起因となった上流決定 (track 直下文書廃止 + ADR 昇格経路)
- `knowledge/conventions/adr.md` — front-matter schema と `grandfathered: true` 用途
- `knowledge/conventions/hexagonal-architecture.md`（廃止 — 現行 SSoT: `architecture-rules.json` / `knowledge/conventions/type-designer-kind-selection.md` R1。経緯: `knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md`） — D2 の層構成規範
- `knowledge/conventions/coding-principles.md` — D3 / D13 / D16 の日常規範
- `knowledge/conventions/enforce-by-mechanism.md` — D11 の機構強制方針
- `knowledge/conventions/nightly-dev-tool.md` — D8 の nightly dev-tool 規範
- `knowledge/conventions/security.md` — D15 の詳細
- `knowledge/conventions/testing.md` — D16 のカバレッジ / テスト規範
- `knowledge/conventions/no-upstream-restatement.md` — 具体版数を ADR に載せない (D9) の背景
- `knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md` — D1 (LanceDB adapter の Tokio bridge) / D4 (semantic index) を authorise した元 ADR。本 baseline はこれを取り消さない
- `knowledge/adr/2026-06-02-0716-dry-checker.md` — D1 の scope から除外される LanceDB adapter を再利用する dry-checker capability
- `knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md` — D4 の scope から除外される `.semantic_index/` の永続化と増分維持 (dry-checker の運用 ADR)
- `knowledge/adr/2026-03-11-0070-conch-parser-selection.md` — 個別 crate 選定の既存 ADR (shell parser)
- `knowledge/adr/2026-03-23-1000-shell-parser-port.md` — D2 hexagonal 実践例 (port + adapter)
