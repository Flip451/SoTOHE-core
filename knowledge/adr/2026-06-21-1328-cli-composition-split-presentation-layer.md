---
adr_id: 2026-06-21-1328-cli-composition-split-presentation-layer
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-add-cli-composition-split-presentation:2026-06-21"
    candidate_selection: "from:[separate-presentation-layer,merge-invoke-render-into-driver,internal-purify-only] chose:merge-invoke-render-into-driver"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:adr-add-cli-composition-split-presentation:2026-06-21"
    candidate_selection: "from:[composition-root-wire-only,merged-composer] chose:composition-root-wire-only"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:adr-add-cli-composition-split-presentation:2026-06-21"
    candidate_selection: "from:[invoke-and-render-together,invoke-only-render-separate] chose:invoke-and-render-together"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:adr-add-cli-composition-split-presentation:2026-06-21"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:adr-add-cli-composition-split-presentation:2026-06-21"
    candidate_selection: "from:[usecase-output-dto,composition-owned-dto,shared-contract-crate] chose:usecase-output-dto"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:adr-add-cli-composition-split-presentation:2026-06-21"
    candidate_selection: "from:[one-track-phased,multiple-tracks,unspecified] chose:one-track-phased"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:adr-add-cli-composition-split-presentation:2026-06-21"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:adr-add-cli-composition-split-presentation:2026-06-21"
    status: proposed
---
# CLI delivery 側の責務分離 — composition root(wire) と primary adapter(invoke+render) への分解

## Context

`cli_composition`（`apps/cli-composition`）は composition root（DI を行う層）として意図された（`2026-05-27-0110-composition-root-dedicated-crate.md`）。しかし 2026-06-21 の層違反監査（`knowledge/research/2026-06-21-1420-layer-violation-check/`）が示すとおり、実態は **DI + presentation + orchestration + use case の invoke + stringly-typed error + god-facade** を一手に抱えている（`CliApp` は stateless unit struct に 51 メソッド/27 ファイル、整形 144 箇所、全メソッド `Result<_, String>`）。

責務を分けるにあたり、2 種類の「分ける／分けない」を区別する。

- **wire（DI）と invoke は別責務**: composition root は object graph を**組む**だけで、use case を**呼ばない**のが canonical（Mark Seemann / Clean Architecture）。配線はリクエストごとに繰り返さない（起動時に一度）。よって wire（composition root）と invoke（driving adapter）は層として分離する。
- **invoke と render は同一責務（primary adapter の双方向変換）**: driving/primary adapter の仕事は「外部入力 → use-case 呼び出し（invoke）」と「use-case 結果 → 外部出力（render）」の双方向の変換で、リクエストごとに 1 対 1 で対になる同じ演者の表裏である。我々が選んだモデル（use case が DTO を返す, D5）では render は adapter の出力側変換そのものであり、Clean Architecture の output-port Presenter（use case が push する別 port）ではない。よって invoke と render は**同一層（primary adapter）**に置き、分離しない。render を独立層にするのは consumer が driver 1 つ・delivery 固有（横断再利用なし）の over-decomposition になる。

したがって delivery 側を **3 層**に分離する: bin（parse + emit）/ composition root（wire）/ primary adapter（invoke + render）。

## Decision

### D1: primary adapter 層 `cli-driver` を新設する（invoke + render を担う）

`apps/cli-driver`（crate 名 `cli_driver`）を新設し、**driving adapter（primary adapter / controller）**とする。注入された use case を保持し、request を受けて use case を呼び（invoke）、結果を整形して（render）`CommandOutcome` を返す。`CommandOutcome` 型・render 関数群もこの層に置く（render は層内 module であり別 crate にしない）。`cli_driver → usecase`。

分解後の依存グラフ:

```text
# <!-- illustrative, non-canonical -->
cli (bin)        -> cli_composition, cli_driver
cli_composition  -> domain, infrastructure, usecase, cli_driver
cli_driver       -> usecase
infrastructure   -> domain, usecase
usecase          -> domain
domain           -> []
```

`architecture-rules.json` の `layers` を次のとおり更新する（SSoT）。

```json
// <!-- illustrative, non-canonical -->
// (1) 追加: cli_driver 層（invoke + render を担う primary adapter）
{ "crate": "cli_driver", "path": "apps/cli-driver", "may_depend_on": ["usecase"],
  "deny_reason": "CLI primary adapter holds injected use cases, invokes them and renders the result into CommandOutcome.", "tddd": { "enabled": false } }
// (2) 更新: cli_composition の may_depend_on に cli_driver を追加 -> ["domain","infrastructure","usecase","cli_driver"]
// (3) 更新: cli の may_depend_on に cli_driver を追加 -> ["cli_composition","cli_driver"]
```

`tddd.enabled` は本 ADR では `false`（層構造の決定に限定）。TDDD 有効化は下流の TDDD ADR がフリップする。この SSoT 変更は `deny.toml` / `Cargo.toml`（workspace members に `apps/cli-driver` 追加）/ `architecture-rules-verify-sync` と整合させ、**層エントリだけ先行追加すると `sotp verify layers` が実在しない crate を検出して CI を割る**ため、crate 実体の追加とともに実装 track の最終 commit（D6）で一括 flip する。

### D2: cli_composition を純 DI composition root にする（wire のみ・invoke しない）

`cli_composition` の責務を **DI（object graph の組み立て）のみ**に絞る。

- secondary adapter（infrastructure）→ interactor（usecase）→ driving adapter（cli_driver）を構築し、use case を driving adapter に**注入**する。**use case を invoke しない**（invoke は cli_driver の責務）。
- 戻り値のエラーは stringly-typed をやめ、**typed `CompositionError`** を新設する（構築・配線の失敗を表す）。
- **god-facade `CliApp`（51 メソッド/27 ファイルの unit struct）を廃止**し、bounded-context 別の `CompositionRoot` 構造（各 context の driver を構築する wiring）に分解する。`CliApp` という単一型は残さない。

```rust
// <!-- illustrative, non-canonical -->
// CompositionRoot: wire して driver を構築するだけ。invoke しない。
impl GuardCompositionRoot {
    pub fn build(&self) -> Result<GuardDriver, CompositionError> {
        let parser = Arc::new(ConchShellParser);
        let interactor = HookDispatchInteractor::new(parser, /* ... */);  // use case
        Ok(GuardDriver::new(interactor))  // driving adapter に use case を注入して返す
    }
}
```

### D3: cli-driver を primary adapter（controller）層とし、invoke と render を同一層に置く

`cli_driver` の各 driving adapter は **注入された use case を保持**し、`handle(input)` で **use case を呼び（invoke）、続けて結果を整形して（render）`CommandOutcome` を返す**。invoke と render はリクエストごとに対になる同じ adapter の双方向変換なので、同一層に置く（別 crate に分けない）。DI はしない（注入される側）。

```rust
// <!-- illustrative, non-canonical -->
pub struct GuardDriver { use_case: HookDispatchInteractor }  // use case を注入保持（DI しない）
impl GuardDriver {
    pub fn handle(&self, input: GuardInput) -> CommandOutcome {
        let result = self.use_case.dispatch(input.into_command());  // invoke（use case 呼び出し）
        render_guard(result)                                        // render（同一層の整形）
    }
}
```

controller 型（invoke + render）にすることで、bin は driver を呼んで `CommandOutcome` を emit するだけで済み、thin-bin が保たれる（D5）。これは web の Controller（request を受け use case を呼び response を組む）と同型で canonical である。

### D4: orchestration を usecase の application service へ移す

`merge_outcomes` や signal layer chain のような複数ステップのオーケストレーション・統合ロジックは application レベルの関心事であり、composition / driver ではなく **usecase の application service** に置く。driving adapter（cli_driver）は単一 use case の invoke + render に留め、複数ステップの統合は usecase が担う。

### D5: 出力 DTO は usecase 出力 DTO を流用し、bin は thin を維持する

driving adapter が呼ぶ use case の出力（render の入力）は、**usecase の出力 DTO（application の出力契約）を流用**する（`2026-04-30-0848-cli-via-usecase-only.md` で多くが既存）。bin は composition root から driving adapter を受け取り、`driver.handle(input)` を呼んで `CommandOutcome` を emit するのみで、`usecase` / `domain` を import しない（thin-bin）。

```rust
// <!-- illustrative, non-canonical -->
// bin: build(composition) -> handle(driver) -> emit。usecase/domain は import しない
let driver  = composition.build_guard()?;          // cli_composition が wire して返す
let outcome = driver.handle(parse_guard(args));    // cli_driver が invoke + render
emit(outcome);
```

### D6: 移行は専用 1 track 内の段階 commit + 最終 flip で行う

専用の 1 track 内で、(a) `cli-driver` crate 新設、(b) command-family 単位で composition から invoke / 整形を cli_driver へ移動、(c) orchestration の usecase 移譲、(d) `CompositionError` 導入、(e) `CliApp` の `CompositionRoot` / `Driver` への分解、(f) port 実装 adapter の infrastructure 移設（D7）、(g) cli bin の直接 I/O 撤去（D8）、(h) reviewer severity policy（`.harness/custom/review-prompts/`）の整備、を段階 commit で進める。re-export shim で CI 緑を保ちつつ family 単位で移し、**最終 commit で String 境界の廃止・`architecture-rules.json` / `deny.toml` / `Cargo.toml` の依存グラフ更新を一括 flip** する。中間状態は main にマージしない（`2026-05-27-0110` D5 と同じ移行パターン）。

#### reviewer severity policy（review-prompts）の整備

層構成の変更に伴い、レイヤー別 reviewer briefing（`.harness/custom/review-prompts/<layer>.md`、利用者所有の severity policy。`2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md`）を新構成に追従させる。各決定（D2/D3/D5/D7/D8）の review 観点をそのまま severity policy に落とす。

- **`cli_composition.md`（更新）**: 純 DI composition root 用に改める。報告カテゴリ: composition root が use case を invoke している（invoke leak、本 ADR で禁止）/ 整形・文字列組み立てを持つ（render leak、cli_driver の責務）/ `Result<_, String>` を返す（typed `CompositionError` でない）/ `CliApp` god-facade 残存 / port を実装する adapter をここに定義（D7 違反）。現行 briefing の "application logic in composition" / "orchestration leak" カテゴリは維持・強化する。
- **`cli_driver.md`（新設）**: primary adapter（invoke + render）用。報告カテゴリ: adapter が DI している（注入される側のはず）/ business logic を持つ（usecase の責務）/ public method signature に domain/usecase 固有ロールの型が出る / `handle` が `CommandOutcome` 以外を返す（エラーも整形して返す）。
- **`cli.md`（更新）**: thin bin 用。報告カテゴリ: bin に business logic / 直接 I/O（`std::fs` / `serde_json` / `chrono`、D8）/ `usecase`・`domain` の import / driver と presenter を bin で coordinate（thin-bin 違反）。

review-prompts は reviewer capability が層別 review 時に読む利用者所有のファイルだが、SoTOHE 自身は dogfooding として上記を整備する。実ファイルの更新は新層 crate の追加と同じ実装 track で行う（本 ADR は要件のみ記録）。

### D7: port を実装する adapter を libs/infrastructure へ移設する

純 DI 化（D2）に伴い、cli_composition 内に定義された port 実装 adapter（secondary port を impl する struct）を libs/infrastructure へ移設する。composition root は adapter を **wire** するが **定義** はしない。2026-06-21 の監査で検出された対象:

- `FsReviewGateStateAdapter` / `FsRefVerifyGateStateAdapter`（filesystem で review / ref-verify gate 状態を読む）— high
- `RecordingDryAgent`（`DryCheckAgentPort` の telemetry decorator）/ `NullInsertIndexProxy`（`SemanticIndexPort` の insert 抑止 proxy + LanceDB lock 管理）— medium
- `NoopSemanticIndexPort` / `NoOpDryApprovalService`（null-object stub）— low。infrastructure の companion stub にするか、infra 非依存なら usecase 提供の test-double にする
- `LazyBranchReader`（`BranchReaderPort` の UFCS 回避用の二重 adapter）— 呼び出し側の trait 曖昧性解消で除去し、infrastructure の `SystemGitRepo` 実装に一本化する

### D8: cli bin の直接 I/O を撤去する（telemetry の infrastructure adapter 化）

`apps/cli/src/main.rs` の `emit_archived_track_subcommand` は bin 内で `std::fs` / `serde_json` / `chrono::Utc::now()` を直接使って telemetry を永続化しており、thin-bin の原則（`2026-05-27-0110` D3 / `2026-04-30-0848` / 本 ADR D5）に反する。telemetry 永続化を infrastructure の adapter（時刻取得・fs 書き込み）として切り出し、composition が wire して driver 経由で呼ぶ経路にする。bin は parse + dispatch + emit のみに戻す。

## Rejected Alternatives

### A. render を独立層（cli-presentation）に分離する（4 層案、D3 の代替）

invoke（cli_driver）と render（cli_presentation）を別々の crate/層に分ける案。

却下理由: invoke と render は primary adapter の双方向変換（入力→use case / 結果→出力）の表裏で、リクエストごとに 1 対 1 で対になる同じ演者・同じ種類の仕事である。独立 presentation 層は consumer が driver 1 つ・delivery 固有（web とは別整形で横断再利用なし）・invoke と常に一緒に跨がれる境界であり over-decomposition。render は cli_driver 内の module に留める。整形を text/JSON で差し替えたい・複数 entry point で共有したい状況が生じれば、その時に分離を再評価する（Reassess When）。

### B. 境界据え置きで内部純化のみ

`2026-05-27-0110` の String 境界と `CliApp` facade を維持し、typed error 化と module 化だけ行う案。

却下理由: invoke / render が composition に残り「層分割」にならない。責務混在という構造問題が解けない。

### C. composition root が invoke も担う（presentation だけ分離する初期案）

driving adapter を独立させず、cli_composition の composer が wire + invoke を兼ねたまま render だけ切り出す案。

却下理由: composition root が interactor を invoke している限り「wire のみ」にならず、composition root と driving adapter の責務混在が残る。canonical な分離（composition root は組むだけ / driving adapter が呼ぶ）にするには invoke を別層（cli_driver）へ出す必要がある。

### D. composition root が wired use case を返し bin が直接 invoke する

driving adapter 型を持たず、composition root が `Arc<dyn UseCase>` を返して bin が直接呼ぶ案。

却下理由: bin が usecase の port / Command / Result 型を知る必要があり thin-bin が崩れる（`2026-05-27-0110` D3 の獣子に戻る）。

## Consequences

### Positive

- delivery 側 3 層（bin=I/O / composition root=wire / cli-driver=invoke+render）に責務が分かれ、wire と invoke の混在（純 DI を妨げる根本）が解消する。
- `cli_composition` が真の純 DI composition root になり（wire のみ、invoke しない）、`CompositionRoot` ロールが canonical な意味を得る。
- `cli_driver` の primary adapter が use case を**注入されて呼び、結果を整形する** canonical な driving adapter（controller）になり、DI と invoke が層レベルで分離する。invoke と render は一体として cohesion を保つ。
- typed `CompositionError` でエラーが型付けされ、stringly-typed の drift が解消する。
- bin は thin のまま（driver を呼んで emit、usecase/domain を非 import）。
- 下流の TDDD/ロール制約 ADR が clean な 3 層構造の上に乗り、各層の allowlist が意味を持つ。

### Negative

- 新 crate `cli-driver` を 1 つ追加し、delivery が 3 層になる。各層境界で入出力 DTO のマッピングが要る。
- `2026-05-27-0110` D2（String 境界）を supersede する大規模移行を伴う: `Result<_, String>` → DTO + `CompositionError`、整形 144 箇所と invoke の cli_driver 移動、51 メソッドの `CompositionRoot` / `Driver` 分解、orchestration の usecase 移譲。
- usecase 側に orchestration application service を新設する作業が発生する。
- 移行スコープに port 実装 adapter 7 件の infrastructure 移設（D7）と cli bin の telemetry I/O 撤去（D8）が加わり、1 track の作業量が大きい。

### Neutral

- bin の依存は `{cli_composition, cli_driver}` になる（`usecase` / `domain` は非 import）。
- 層依存の enforcement は `architecture-rules.json` / `deny.toml` / `Cargo.toml` の更新で成立し、既存の検証機構でカバーされる。

## Reassess When

- render（整形）を差し替えたい（text / JSON 出力モード）、または複数 entry point（web / gRPC）で整形を共有したくなった場合: render を cli_driver から独立 presentation 層へ分離することを再評価する（Rejected Alternative A の再検討）。
- driving adapter（controller）が単なる use case → 整形の pass-through に留まり独自ロジックを持たない場合: cli_driver を composition root 内の型に戻す（層併合）ことを再評価する。
- web / gRPC など 2 つ目の entry point が追加された場合: cli_driver の共有可否（entry point ごとに分けるか）を再評価する。
- usecase 出力 DTO が CLI 整形都合に引きずられて膨張した場合: cli_driver 専用の view-model 導入を再評価する。

## Related

- `knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md` — 本 ADR が一部 supersede する（D2 String 境界 / D3 の bin 依存）。cli_composition crate 新設・`CliApp` facade・`CommandOutcome` の元 ADR。Reassess When 第2項が本 ADR の trigger。
- `knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md` — usecase 出力 DTO（本 ADR が D5 で流用）と cli→usecase 境界の前提。
- `knowledge/adr/2026-06-21-1420-cli-layers-tddd-and-role-placement-lint.md` — 下流の TDDD/ロール配置制約 ADR。本 ADR の 3 層分解後に cli 系の型・ロール・allowlist を定義する（それまで保留）。
- `knowledge/research/2026-06-21-1420-layer-violation-check/` — 本 ADR の D2/D4/D7/D8 の根拠となる層違反監査（36 件）。
- `knowledge/conventions/hexagonal-architecture.md` — CLI as Composition Root / port placement。composition root（wire）と primary adapter（invoke+render）の区別、`SecondaryAdapter`（driven）との対比の根拠。本 ADR 実装後にこの節を 3 層構成へ更新する。
- `knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md` / `.harness/custom/review-prompts/` — レイヤー別 reviewer severity policy。本 ADR の層構成変更に伴い `cli_driver.md` 新設 + `cli.md` / `cli_composition.md` 更新が必要（D6）。
- `architecture-rules.json` — 層定義と依存ポリシーの SSoT。本 ADR は `cli-driver` 層エントリの追加と依存グラフ更新で実体化される。
