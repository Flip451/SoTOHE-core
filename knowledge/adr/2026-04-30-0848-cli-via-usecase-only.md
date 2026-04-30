---
adr_id: 2026-04-30-0848-cli-via-usecase-only
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-add-cli-via-usecase-only:2026-04-30; oq-resolution:direction-x-full-strict:2026-04-30"
    candidate_selection: "from:[A-status-quo,B-pub-use-reexport,C-cli-infra-also-banned,strict-DTO] chose:strict-DTO"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:adr-add-cli-via-usecase-only:2026-04-30"
    candidate_selection: "from:[phased-by-module,big-bang,by-concern] chose:big-bang"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:adr-add-cli-via-usecase-only:2026-04-30"
    candidate_selection: "from:[ssot-only,cli-extra-check,transitional-allow] chose:ssot-only"
    status: proposed
---
# CLI→domain 直接参照禁止と usecase 経由への一本化

## Context

現状の `architecture-rules.json` では `apps/cli` (cli crate) の `may_depend_on` が `["domain", "infrastructure", "usecase"]` となっており、cli から domain crate への直接依存が許可されている。

実態として `apps/cli/src/` 配下の **25 ファイル** が `use domain::...` の形で domain 型・関数を直接 import している（例: `domain::Decision`, `domain::guard::ShellParser`, `domain::tddd::*`, `domain::review_v2::*`, `domain::ImplPlanReader`, `domain::CommitHash`, `domain::TrackId`, `domain::TrackMetadata`, `domain::DomainError`, `domain::derive_track_status`, `domain::schema::SchemaExporter`, `domain::hook::HookContext`, `domain::ConfidenceSignal` など）。

この経路が広く使われている結果、本来 usecase 層に入るべきオーケストレーション・整形・組み立てロジックが cli 側で書かれ、または cli の都合で domain 側に流れ込みやすい状態になっている。具体的には:

- cli が domain の typestate / value object を直接組み立てて usecase に渡す形が定着し、cli が domain の不変条件を直接知る作りになっている。
- domain 型の field・enum variant を cli が直接消費するため、domain 側を変えると cli の表示・整形・コマンド I/O も同時に書き換える必要が生じ、層の責務分離が崩れている。
- 同種の懸念で過去 ADR `knowledge/adr/2026-03-25-0000-diff-scope-in-usecase.md` が DiffScope と scope filtering を usecase 層に置く判断を下しており、`knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md` も scope 分類の CLI 表面化を `ScopeQueryService` という usecase service に集約する方針を採っている。本 ADR はこれら個別の cli→usecase shift を「cli は domain を直接参照しない」というアーキテクチャ全体ルールに引き上げて固定化する位置付け。

`architecture-rules.json` は workspace の layer policy の SSoT で、`deny.toml` (`cargo make deny`) と `scripts/check_layers.py` / `bin/sotp verify check-layers` が同 SSoT を参照して機械検証を行うため、ここで `may_depend_on` を絞れば追加の lint 機構を増やさずに enforcement を統一できる。

## Decision

### D1: cli の may_depend_on から `domain` を外し、usecase 側で DTO に包んで domain 型を外部に出さない

`architecture-rules.json` の `apps/cli` レイヤーの `may_depend_on` を `["usecase", "infrastructure"]` に変更する。cli crate の `Cargo.toml` からも `domain = { path = "../../libs/domain" }` を削除する。

cli が必要とするデータ・操作はすべて usecase 層を経由してアクセスする。usecase は domain 型をそのまま外部に再公開せず（`pub use domain::...` を介した re-export を禁ずる）、cli が消費する形に整えた DTO / Command / Query / Result 型を usecase 自身が定義してそれを公開する。usecase の内部実装では domain 型を扱ってよいが、その型は usecase の public API には出てこない。

これにより cli は domain 型の存在を知らずに動き、boundary の型は usecase が単独責任で持つ。将来 cli 以外の delivery adapter（例: web server / gRPC）が増えた場合も同じ usecase API を経由できる前提が整う。

#### D1 適用時の境界判断パターン

D1 の「全 strict」解釈（cli は domain 型を一切参照しない）を実装に落とす際、以下のパターンで usecase 側の責任範囲を確定した。これらはユーザーが Direction X (full strict) を選択した際の具体的境界判断の永続 record である。

**エラー集約（OQ-1）**

cli が従来直接参照していた domain error の `#[from]` variants は、usecase 側で単一のエラー型（`TaskOperationError` 等）に集約する。cli は usecase error のみを知り、`cli/src/error.rs` でこれを `CliError::Message` 等に変換する。

**string プリミティブ受け渡しと domain 型の usecase 内部構築（OQ-2）**

cli から usecase へのコマンドは string プリミティブを渡す。`domain::TrackId`・`domain::TaskId`・`domain::CommitHash`・`domain::StatusOverride` 等の domain 値オブジェクトへの変換は usecase（interactor）内部で行い、cli の `commands/` 配下に domain import を出さない。Query 系も同様に usecase 内で`domain::TaskStatusKind`・`domain::ImplPlanReader` 等を用いて結果を DTO に変換してから返す。

**ConfidenceSignal 分類の usecase 閉じ込め（OQ-3）**

`domain::ConfidenceSignal` の Red / Yellow / Blue 分類ロジックは usecase 内 service (`PreCommitTypeSignalsService` 等) に収め、cli の `commands/make.rs` が同 enum を直接参照しない形にする。出力 DTO に verdict と信号リストを格納し、cli は DTO フィールドを読むだけにする。

**review run 境界（OQ-4）**

`domain::review_v2::ReviewCycle` のオーケストレーション、および `domain::review_v2::Verdict`・`FastVerdict`・`ReviewCycleError` の知識は usecase 内の service (`RunReviewService` 等) に封じ込める。cli の `commands/review/` は RunReviewCommand (string / primitive のみ) を渡し、RunReviewOutput (DTO) を受け取る。

**verify 系 domain 型の cli への流出防止（OQ-6）**

`domain::tddd::LayerId`・`domain::ContentHash`・`domain::SpecRefFinding`・`domain::check_catalogue_spec_ref_integrity` 等は usecase 層の service に包み (`VerifyCatalogueSpecRefsService` 等)、cli の `commands/verify_*` はその DTO のみを参照する。findings の整形（`domain::SpecRefFindingKind` への match など）も usecase 側で済ませてから文字列として返す。

**commit-hash 永続化の境界（OQ-5）**

`domain::CommitHash` と `domain::review_v2::CommitHashWriter` のアクセスは usecase 内 service (`CommitHashPersistenceService`) に収める。cli は記録済み SHA を string として受け取り、確認メッセージの表示のみを担う。

**DTO の usecase 層への配置（一般原則）**

上記以外の DTO（`HookVerdictOutput`・`TrackPhaseOutput`・`VerifyCatalogueConsistencyOutput`・`VerifySpecSignalsOutput`・`LayerSignalSummary` 等）も、対応する usecase service の出力として usecase crate に置く。cli はこれらを import するが、その背後にある domain 型 (`domain::hook::HookVerdict`・`domain::track_phase::TrackPhaseInfo`・`domain::ConfidenceSignal` 等) は import しない。

### D2: 移行は専用の単一 track で一括で行う

cli の 25 ファイルにわたる domain 直接参照は、専用の 1 つの track で一括して usecase 経由に置き換える。track 内では (a) usecase 側に必要な DTO / Command / Query / Result 型と service / facade を整備し、(b) cli 側を順次切り替え、(c) `architecture-rules.json` の `may_depend_on` 更新と `apps/cli/Cargo.toml` からの `domain` 依存削除を最後に行って layer rule を固定化する。

「段階的に command module 単位で track を切る」「concern 単位で分ける」案も検討したが、usecase facade を部分整備しながら走らせると一時的な API 不整合が長期化し、cli のうち未移行 module が新 API と旧 API を混在で使う期間が生じ得る。これに比べて 1 track で一斉に切り替える方が、boundary 型の整合性と build 通過の保証コストが低い。

track 内の commit 粒度は本 ADR では規定せず、track 計画 (`/track:plan`) 側でサブタスク化する。

### D3: enforcement は architecture-rules.json (SSoT) の更新で済ませる

cli→domain 直接依存の禁止は `architecture-rules.json` の `may_depend_on` を変更するだけで成立し、追加の lint・grep ガード・期限付き allowance などは導入しない。

`architecture-rules.json` を更新すれば:

- `cargo make deny` が `deny.toml` 経由で cli の domain 依存を reject する
- `cargo make check-layers` (scripts/check_layers.py) と `bin/sotp verify check-layers` が同 SSoT を読んで CI gate に反映する
- `cargo make ci` の標準フローに enforcement が含まれる

これらが既に揃っているため、cli 専用の追加 grep / clippy lint や、移行期間限定の allow list は導入しない。本 ADR の D2 で「1 track で一括移行」を採用したことにより、移行途中の build 不通状態を CI で許容する必要がない。

## Rejected Alternatives

### A. 現状維持 (cli から domain 直接参照を許す)

cli の `may_depend_on` を変更せず、現行の `["domain", "infrastructure", "usecase"]` を維持する案。

却下理由:

- 既に 25 ファイルで cli → domain 直接 import が広がっており、ロジックが domain 層に流出するパターンが定常化している。何もしないと boundary が更に曖昧化する。
- 過去 ADR (DiffScope-in-usecase / scope-lookup-commands) と同じ「cli の関心事を usecase に押し戻す」判断を個別案件ごとに繰り返しており、全体ルールに引き上げないと類似の流出が今後も発生する。

### B. usecase が domain 型を pub use で re-export する

cli の `may_depend_on` から `domain` を外す（layer rule は変える）一方、usecase 側で必要な domain 型を `pub use domain::...` で再公開する案。型名空間としては usecase 経由になるが、型の実体は domain。

却下理由:

- cli から見える型はそのまま domain 型なので、cli が domain の field・variant・不変条件に直接依存し続ける。layer rule 上の boundary は守られても、設計上の boundary は破られたままになる。
- 「型を直接 import するか re-export 経由か」という違いだけが残り、ロジック流出という本来の問題は解決しない。
- 後から DTO 化に切り替えるとしても、移行コストは結局 D1 を最初から選ぶのと同じになる。

### C. cli → infrastructure も同時に禁止する

cli の `may_depend_on` を `["usecase"]` のみに絞り、cli から infrastructure への直接依存も同時に禁止する厳格 hexagonal 案。

却下理由:

- 現状 cli は composition root（main.rs と起動経路）で infrastructure adapter を直接 wiring しており、これを usecase 経由に変えると DI / wiring の構造再設計が必要になり scope が大きく膨らむ。
- 今回の主眼は「ロジック流出の抑止」であり、その観点では cli → domain の遮断で十分。infrastructure 遮断は composition root の再設計と一括で扱うべき別判断として切り出す。
- cli → infrastructure を残しても、infrastructure crate 自身が domain port を実装する立場で domain 型を外に押し出すわけではないため、boundary の崩れは限定的。

### D. 段階的 + command module 単位の移行

cli の command module（review / track / hook / verify / make / domain / guard など）ごとに別 track を立て、各 track 内で usecase facade を部分整備しながら順次移行していく案。

却下理由:

- usecase facade を部分的に整備する間、cli の未移行 module は旧 API（domain 直接）を使い、移行済み module は新 API（usecase 経由）を使う混在期間が長く続く。boundary 型 / DTO の整合性を partial integration で維持し続けるコストが高い。
- 移行途中で `architecture-rules.json` の layer rule を強制できず、その間は本 ADR の D3 で書いた enforcement が効かない状態が続く。
- 1 track で一括 refactor する案 (D2) と比較してレビュー回数の合計は減らないかむしろ増える可能性が高い。

## Consequences

### Positive

- cli が domain に直接依存しなくなり、cli の単体テストで domain 型の準備が不要になる（データ・操作の取得はすべて usecase 経由になるため、usecase の port / mock のみでテストが完結する）。
- usecase が boundary 型を単独責任で持つため、domain の内部実装を変更しても cli の I/O 整形・表示には波及しない。layer の責務境界が型として明示される。
- 過去の cli→usecase shift 系 ADR（DiffScope-in-usecase / scope-lookup-commands）が示してきた「cli の関心事を usecase に押し戻す」判断が、個別案件単位ではなくプロジェクト全体のアーキテクチャルールとして固定化される。
- 将来 cli 以外の delivery adapter（web server / gRPC など）を追加するときも、同じ usecase API を経由できる前提が整う。

### Negative

- 一括 refactor の対象が 25 ファイル相当と大きく、対応 track の commit サイズと review 量が膨らむ。track 計画でサブタスクに切る運用が前提となる。
- usecase 側に DTO / Command / Query / Result 型を新たに揃える必要があり、似た形の型が usecase（boundary 用）と domain（不変条件用）の双方に並ぶ場面が出る（例: identifier / 簡易 metadata 構造）。
- usecase の public API が cli の I/O 都合に引きずられて広がる可能性があり、usecase 側で「cli 専用の型」をどこまで認めるかの判断は別途運用ルールが必要になる。

### Neutral

- enforcement は `architecture-rules.json` の更新だけで `cargo make deny` / `cargo make check-layers` / `bin/sotp verify check-layers` に反映されるため、新規の lint や verify サブコマンドは増えない。

## Reassess When

- usecase 側の DTO 重複（cli boundary 用と domain 不変用で似た型が並ぶ）が顕著に保守コストになり、boundary 型を domain と共有する別の手段（pub use の限定的解禁、共通 boundary crate の追加 など）を検討する必要が出たとき。
- cli 以外の delivery adapter（web server / gRPC / GUI 等）が追加され、usecase の public API を multi-adapter で再評価する必要が出たとき。
- workspace 全体の hexagonal 構造を再編し、composition root の wiring も含めて層分けを大きく変える判断（例: cli → infrastructure 直接依存も同時に禁止する案 = Rejected Alt C の再評価）が立ち上がったとき。
- Rust 側で boundary 型を低コストに表現できる新しい言語機能・パターン（公開 type alias / view 型など）が定着し、DTO を別途定義しなくて済むようになったとき。

## Related

- `knowledge/adr/2026-03-25-0000-diff-scope-in-usecase.md` — 個別の cli→usecase shift 判断（DiffScope と scope filtering の配置）。本 ADR の前駆事例。
- `knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md` — `ScopeQueryService` という usecase service を介して scope 分類ロジックを CLI 表面化した事例。同方向の判断。
- `architecture-rules.json` — workspace の layer policy の SSoT。本 ADR の Decision はここを直接書き換える形で実体化される。
- `knowledge/conventions/README.md` — convention 索引。本 ADR の運用上の補完となる convention（usecase boundary 型の運用ルール 等）が将来追加される場合の置き場所。
