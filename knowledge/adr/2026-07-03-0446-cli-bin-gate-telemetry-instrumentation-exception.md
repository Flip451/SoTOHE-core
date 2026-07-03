---
adr_id: 2026-07-03-0446-cli-bin-gate-telemetry-instrumentation-exception
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-approve-cli-bin-gate-telemetry:2026-07-03"
    candidate_selection: "from:[clock-port-in-usecase,infrastructure-decorator,cli-driver-timing,bin-level-instrumentation] chose:bin-level-instrumentation"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:adr-approve-cli-bin-gate-telemetry:2026-07-03"
    candidate_selection: "from:[wide-exception,narrow-observability-only] chose:narrow-observability-only"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:adr-approve-cli-bin-gate-telemetry:2026-07-03"
    candidate_selection: "from:[permanent-exception,transitional-refactor-planned] chose:transitional-refactor-planned"
    status: proposed
---
# CLI bin 層の GateEval telemetry 計装を thin-bin 例外として認める

## Context

ADR `2026-06-21-1328-cli-composition-split-presentation-layer.md` の D3（`cli-driver` を primary adapter とし invoke + render を同一層に置く）・D5（thin-bin: `bin: build → handle → emit`）・D8（bin の直接 I/O 撤去、telemetry は infrastructure adapter 化し composition が wire して driver 経由で呼ぶ経路にする）を前提としても、`apps/cli/src/main.rs::execute_verify_with_telemetry` 以来、`verify-*` ゲート家族では bin 層で `std::time::Instant` により driver 呼び出しの前後を計測し、`cli_composition::telemetry_wiring::{resolve_telemetry_writer, emit_gate_eval}` ヘルパーを bin から直接呼び出して `TelemetryEvent::GateEval` を emit する precedent が既に確立している。`sotp dry check-approved` も同じ precedent に従い bin 層 telemetry パターンで揃える（`apps/cli/src/commands/dry.rs::execute_dry_check_approved`）。

代替案として一度検討・実装した以下は、レビュー通過過程でいずれも hex-arch のレイヤー境界と衝突することが判明した。

- usecase 側に `DryCheckApprovedClockPort` などの Clock port を新設して interactor に注入する構成は、usecase reviewer の severity policy（`.harness/custom/review-prompts/usecase.md`）で「time 値は usecase entrypoint の parameter として渡すべきで、ポート経由の内部 retrieval は fig-leaf」と判定された。
- infrastructure に `TimedGateEvalDryCheckApprovedService` などの decorator を置き `DryCheckApprovedDriverService`（primary ApplicationService）を実装する構成は、infrastructure が usecase の primary port を実装する形になりレイヤー境界違反となる（hex-arch は infrastructure を driven / secondary 側の実装専用に限定している）。
- cli_driver に timing / telemetry を持たせる構成は、cli_driver reviewer に `business_logic_in_adapter` として P1 で flag された（cli_driver は invoke + render の controller に責任範囲を限定するべき）。

その結果、実装コード側で既に確立している bin-level GateEval 計装パターンを、ADR 側でも正式に認知し、thin-bin 原則の legitimate な例外として言辞的に位置づけておく必要がある。

## Decision

### D1: bin 層の GateEval telemetry 計装を thin-bin 原則の legitimate 例外として認める

CLI ゲートの `GateEval` telemetry 計装だけを、ADR `2026-06-21-1328` D3 / D5 が定める thin-bin 原則（bin は `build → handle → emit` に留まる）の legitimate な例外として明文で許容する。具体的には、bin 層のコマンドエントリ関数（`apps/cli/src/main.rs::execute_verify_with_telemetry`、`apps/cli/src/commands/dry.rs::execute_dry_check_approved` など）が `std::time::Instant::now()` で driver 呼び出しの前後の経過時間を計測し、`cli_composition::telemetry_wiring::{resolve_telemetry_writer, emit_gate_eval}` ヘルパーを bin から直接呼び出して `TelemetryEvent::GateEval` を emit する形態を認める。同 ADR の thin-bin 原則自体（D3 / D5）と、telemetry 永続化を infrastructure adapter 側に閉じる原則（D8 の一般部分）は変更しない。本例外は cross-cutting instrumentation（ゲートの実行時間・成否観測）に限定した narrow なものである。

### D2: 例外の適用範囲は CLI ゲートの横断的 observability に限定する

D1 の例外化が適用されるのは、CLI ゲートの `GateEval` telemetry のような **CLI 実行そのものの横断的 observability（ゲート成否 + 所要時間の計測）** だけとする。以下は本例外の適用範囲外であり、ADR `2026-06-21-1328` D8 の一般原則（infrastructure adapter 化 + composition が wire + driver 経由）を引き続き適用する。

- ドメインデータの永続化（domain event / gate 結果そのものの JSON ファイル書き込み、DB 反映など）。
- ドメイン用途での時刻取得（例: domain event の `timestamp` を `chrono::Utc::now()` から取得する等、ビジネスロジック上意味のある時刻）。
- 任意の fs / network / process 直接操作。

なお `GateEval` telemetry の fs 書き込み自体は `infrastructure::telemetry::TelemetryWriter`（SecondaryAdapter）に既に委譲されており、bin 層に残るのは `Instant` ストップウォッチの開始点保持と `emit_gate_eval` ヘルパー呼び出しだけである。この境界を明確に維持し、本例外を「ゲート実行の観測」以外の目的に携帯拡張しないこと。

### D3: 本例外は transitional であり、将来 tracing crate ベースの計装統一を前提とする

本 ADR の例外化は **一時的（transitional）** な位置付けであり、将来のリファクタリングで以下のいずれか（または両方）へ移行することを前提とする。

- `tracing` crate を導入し、usecase interactor 側に `#[tracing::instrument]` annotation を付け、infrastructure に custom Layer subscriber を実装して `TelemetryEvent::GateEval` JSONL を emit する declarative instrumentation パターンへ移す（use case は宣言的 annotation だけを持ち、時刻取得は subscriber 側に閉じる）。
- `execute_verify_with_telemetry` を含めて codebase 全体の `GateEval` telemetry を `tracing` ベースに統一し、bin 層に `std::time::Instant` や `emit_gate_eval` の直接呼び出しを残さない形へ揃える。

このリファクタリングが完了した時点で、本 ADR は新 ADR に supersede されるものとする。それまでの間、本 ADR は bin-level GateEval 計装パターンの「橋渡し」として機能する。

## Rejected Alternatives

### A. usecase に Clock port を導入して interactor に注入する

`DryCheckApprovedClockPort` のような secondary port を usecase に新設し、`interactor.new(..., clock)` の形で注入して interactor 内部で `clock.now_millis()` を 2 サンプル取って duration を計算する構成。この構成は一度実装したが、usecase reviewer severity policy（`.harness/custom/review-prompts/usecase.md`）の「implicit time / env / process dependency」カテゴリで「time / env / process values must be parameters to the usecase entrypoint, not retrieved inside」と判定された。documented port contract であってもポート越しの time 取得は fig-leaf 扱いになるため、この方向は却下した。

### B. infrastructure decorator で primary ApplicationService を実装する

`TimedGateEvalDryCheckApprovedService` のような decorator を `libs/infrastructure/` に置き `usecase::dry_check_approved_driver::DryCheckApprovedDriverService`（primary ApplicationService）を実装する構成。infrastructure decorator が inner service を wrap して `Instant` 計測 + telemetry emit を追加し、composition が decorator を wire する。この構成は一度実装したが、hex-arch のレイヤー分離を破っている（infrastructure は driven / secondary 側の port 実装専用で、primary side の ApplicationService を実装するのは境界違反）と judgment され、user 判断で却下した。type-designer の role taxonomy にも「infrastructure が primary port を実装する」ケースを表現する role が存在しないため、この構成を維持することは taxonomy 拡張まで含む大きな変更を要求する。

### C. cli_driver に timing / telemetry を搭載する

`cli_driver::dry::DryDriver::dry_check_approved` に `Arc<dyn DryCheckApprovedTelemetryPort>` を注入し、driver 内部で `Instant` 計測 + telemetry port 呼び出しを行う構成。この構成は一度実装したが、cli_driver reviewer に `business_logic_in_adapter` として P1 で flag された。cli_driver（primary adapter）は invoke + render の controller に責任範囲を限定するべきで、横断的関心事（timing / telemetry の orchestration）を含めるべきではない、との judgment。

## Consequences

### Positive

- `apps/cli/src/main.rs::execute_verify_with_telemetry` を始めとする既存 `verify-*` ゲート家族のパターンと `execute_dry_check_approved` が同一パターンで一貫し、codebase 内の `GateEval` telemetry パターンが統一される。
- usecase interactor は 7-port pure（`std::time` 直接依存なし）を保ち、hex-arch の usecase 純粋性が守られる。
- infrastructure は secondary port 実装（`SecondaryAdapter` role）のみを担う厳密な役割分離が守られる。primary port を実装する infrastructure 型は存在しない。
- cli_driver は invoke + render の controller という単一責任を保つ。

### Negative

- thin-bin 原則（ADR `2026-06-21-1328` D3 / D5）の字面と本 ADR の例外化との間に不整合が読める。将来この不整合が矛盾と誤解されないよう、bin-level GateEval 計装に触れる spec 側の記述は本 ADR を `adr_refs` として明示的に引くようにしておく必要がある。
- bin 層に `std::time::Instant` の使用が発生し、将来の `tracing` crate 導入 refactor（D3）までの間、codebase に「例外パターン」が明示的に混在する状態になる。
- 新規 gate を追加する際に、この例外パターンを安易に携帯拡張してしまう危険がある。D2 の境界定義（CLI ゲートの横断的 observability に限定）を厳守する運用が必要。

## Reassess When

- `tracing` crate の導入と `#[tracing::instrument]` ベースの GateEval telemetry 統一 refactor が完了したとき（本 ADR は新 ADR に supersede される）。
- usecase / cli_driver 層に他の cross-cutting instrumentation パターン（例: audit log、performance metrics、request tracing など）が必要になったとき（本例外の適用範囲を拡張するか、新 ADR を作るかを検討する）。
- hex-arch の role taxonomy（`SecondaryAdapter` / `PrimaryAdapter` / 他）に decorator-of-primary-port の新 role が追加されたとき（代替案 B が再検討可能になる）。

## Related

- `knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md` — D3 / D5 / D7 / D8 の refinement 対象として参照する。本 ADR はその thin-bin 原則の narrow な例外化。
- `knowledge/adr/` — ADR 索引。
