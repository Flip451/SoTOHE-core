---
adr_id: "2026-07-23-1318-composition-root-pure-di-migration-initiative"
decisions:
  - id: D1
    status: proposed
    user_decision_ref: "chat_segment:current-task:2026-07-23:composition-root-pure-di-migration-hearing"
  - id: D2
    status: proposed
    user_decision_ref: "chat_segment:current-task:2026-07-23:composition-root-pure-di-migration-hearing"
  - id: D3
    status: proposed
    user_decision_ref: "chat_segment:current-task:2026-07-23:composition-root-pure-di-migration-hearing"
  - id: D4
    status: proposed
    user_decision_ref: "chat_segment:current-task:2026-07-23:composition-root-pure-di-migration-hearing"
  - id: D5
    status: proposed
    user_decision_ref: "chat_segment:current-task:2026-07-23:composition-root-pure-di-migration-hearing"
  - id: D6
    status: proposed
    user_decision_ref: "chat_segment:current-task:2026-07-23:composition-root-pure-di-migration-hearing"
  - id: D7
    status: proposed
    user_decision_ref: "chat_segment:current-task:2026-07-23:composition-root-pure-di-migration-hearing"
  - id: D8
    status: proposed
    user_decision_ref: "chat_segment:current-task:2026-07-23:composition-root-pure-di-migration-hearing"
  - id: D9
    status: proposed
    user_decision_ref: "chat_segment:current-task:2026-07-23:composition-root-pure-di-migration-hearing"
  - id: D10
    status: proposed
    user_decision_ref: "chat_segment:current-task:2026-07-23:composition-root-pure-di-migration-hearing"
---
# Composition root 純 DI 化を単一改善イニシアチブと複数独立 track で完遂する

## Context

`knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md` は、CLI composition root を wiring 専用とし、実行ロジック、`CommandOutcome` の生成、直接 I/O、および usecase から composition root への逆委譲を禁止する純 DI 境界を定めた。同 ADR は lint と参照実装も導入した一方、D4 では既存 root の移行を「各 command 文脈を変更するときに段階的に行う」方針とした。

2026-07-23 時点のスナップショットでは、composition root は 26 個、public method は 136 個、そのうち実行責務を持つ public surface は概算 68 個に達している。さらに一部の driver は usecase interactor を経由した後、legacy composition method へ戻る経路を残している。この状態では、純 DI 化の規則と実装の間に既知の乖離がありながら、完了期限、全体の完了条件、全体責任の単位が存在しない。

機会的移行は個々の変更を小さくできる反面、変更頻度の低い command 文脈を恒久的に取り残し得る。また、逆委譲を仲介する `*ServiceImpl` や互換 shim が「一時的な構造」のまま定着し、driver → usecase → port という単一経路への収束を検証しにくい。

一方、全 root を一つの track と一つの PR で変更すると、レビュー範囲、回帰時の原因特定、並行作業、ロールバック単位が過大になる。そのため、全体を一つの改善イニシアチブとして扱いつつ、独立に検証・統合できる複数 track に分割する必要がある。

## Decision

### D1: 純 DI 化を単一の完了可能な改善イニシアチブとして扱う

全 composition root の既知の乖離をゼロにするまでを、一つの改善イニシアチブとして計画・追跡する。機会的移行を終了し、対象全体、完了条件、および最終収束を明示する。

本決定は `knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md` の D4 にある機会的移行方針だけを置き換える。同 ADR の D1〜D3 および D5 が定めた境界規則、lint、参照実装は引き続き有効とする。

### D2: 実装を約 5 個の独立 track に分割する

計画時の標準分割数を約 5 個とし、実際の依存関係、変更量、ファイル競合に応じて 4〜6 個の範囲で調整できるものとする。各 track は、単独でレビュー可能であり、CI を green に保ったまま統合できなければならない。

track 数そのものは目的ではない。責務の凝集度、依存方向、および安全な統合単位を優先し、root 数を均等に割るためだけの分割は行わない。

### D3: command 文脈の凝集度に基づく標準分割を採用する

計画開始時は、次の 5 track を標準案とする。

1. **Leaf commands**: ADR baseline、Catalog、Template、TaskContract
2. **Support workflows**: RefVerify、SemanticDup、TestObligation、Verify
3. **Collaboration workflows**: PR、Review、Signal
4. **Core lifecycle**: Track、TDDD
5. **Final convergence**: 互換 shim と中間 `*ServiceImpl` の削除、全体 lint、文書と export の同期

各 track の詳細な対象は Phase 3 の実装計画で確定する。依存関係や所有ファイルの重なりが判明した場合は D2 の範囲内で再分割できるが、同一 command 文脈の責務を不自然に分断してはならない。

### D4: 外部観測可能な CLI 契約を維持する

純 DI 化の各 track は、CLI 引数、stdout、stderr、exit code、および永続化結果を変更しない。構造変更の前後で、同じ入力に対して同じ外部観測可能な結果を返すことを統合テストで保証する。

外部挙動を変更する必要が生じた場合は、本イニシアチブへ混在させず、別 ADR で判断する。

### D5: 各 track は担当文脈内の逸脱をゼロにする

各 track は担当する command 文脈について、composition root から次の責務をすべて除去する。

- command 実行ロジック
- `CommandOutcome` の生成
- filesystem、process、network、terminal への直接 I/O
- usecase から composition root へ戻る逆委譲

担当文脈の実行経路は driver → usecase interactor → port の一方向かつ単一の経路へ収束させる。部分的な移送や、新しい名前の互換 facade を残した状態を track 完了とはみなさない。

### D6: リポジトリ全体の収束条件をイニシアチブ完了 gate とする

最終収束 track は、次の条件がすべて満たされた場合にのみイニシアチブを完了と判定する。

- 全 composition root が `CompositionRootPureDi` 規則に適合し、違反がゼロである
- CLI integration test が外部契約の維持を確認する
- `cargo make ci` が成功する
- legacy 互換 shim、および composition root への逆委譲を仲介する `*ServiceImpl` が残っていない
- 関連する文書、export、lint 設定が最終構造と一致する

個別 track の完了だけでは、イニシアチブ全体の完了を宣言しない。

### D7: 変更ファイルが重ならない track だけを並行実行する

変更対象ファイルが重ならず、依存する型・port・export が安定している track は並行実行できる。ファイル所有または公開型の変更が重なる場合は、順次実行するか、先行 track を `develop` へ統合してから後続 track を開始する。

各 track は独立して CI green を満たす。未統合 track の変更を暗黙に前提とする実装は認めない。

### D8: 各 track は本 ADR を直接参照し、実状態から進捗を判定する

各 track は本 ADR の関連する Decision を直接参照する。track 間の親子関係や進捗を保持する専用の親 state JSON は新設しない。

最終収束 track は、中間的な進捗記録ではなく、リポジトリに存在する code、test、lint、export、および文書の実状態から D6 の完了条件を判定する。

### D9: Interactor と Application Service のクレート分離を同時に行わない

本イニシアチブでは crate topology を変更しない。Interactor と Application Service は、`libs/usecase` 内の適切な command 文脈 module に配置し、同じ application boundary の中で役割を分離する。

役割名だけを根拠に水平 crate へ分割すると、同一 usecase の型と制御フローが crate 境界をまたぎ、依存と変更理由が増える。純 DI 化と crate 再編を同時に行うと、回帰原因とレビュー論点も混ざる。将来、bounded context ごとの独立リリース、明確に異なる依存集合、または別 delivery mechanism の要求が現れた場合は、垂直な context 単位の crate 分割を別 ADR で検討する。

### D10: 停滞時は完了済みの純化を維持し、残作業だけを再計画する

ある track が停滞しても、すでに green で統合された track の純 DI 化を取り消さない。停滞原因を解消するため、未完了の文脈だけを D2 の範囲内で再分割または順序変更する。

再計画によって D4〜D6 の契約や完了条件を緩和してはならない。これらを変更する必要がある場合は、新しい ADR で判断する。

## Rejected Alternatives

### A. 機会的移行を継続する

変更対象になった command 文脈だけを純化する方法は、短期の変更量を抑えられる。しかし、完了期限、全体完了条件、全体責任がなく、変更頻度の低い root と逆委譲仲介層が恒久化するため採用しない。

### B. 全 root を一つの巨大 track と PR で変更する

一括変更は最終形を一度に得られるが、レビュー範囲が過大になり、回帰原因の切り分け、並行作業、部分的な統合とロールバックが困難になるため採用しない。

### C. 現在の composition facade を正式な application service として認める

既存構造を正当化すれば移行量は減る。しかし、composition root を wiring 専用とする境界、`cli_driver` と usecase の責務分離、既存 lint と参照実装を実質的に無効化するため採用しない。

### D. Interactor と Application Service のクレート分離を同時に行う

純 DI 化と crate topology の変更は異なる設計判断である。同時実施は変更理由、回帰原因、レビュー論点を混在させ、役割別の水平分割による結合を増やし得るため採用しない。

### E. 1 composition root を 1 track とする

26 前後の track は、管理、レビュー、統合の儀式的コストを増やす。同一 command 文脈の型、port、test を不自然に分割し、独立性も必ずしも高めないため採用しない。

## Consequences

### Positive

- 既知の architecture deviation に明確な終点と全体完了条件が生まれる。
- 全 command の実行経路が driver → usecase interactor → port に統一される。
- application logic を composition root から切り離すことで、usecase 単体テストが容易になる。
- lint と最終収束 gate により、逆委譲や実行責務の再流入を検出できる。

### Negative

- 複数 track の所有範囲、統合順序、公開型の変更を調整するコストが生じる。
- 最終収束までは、新旧の構造がリポジトリ内に一時的に共存する。
- typed port、DTO、test fixture、integration test の追加実装が必要になる。
- 個別 track が完了しても、イニシアチブ全体の完了までには時間差がある。

### Neutral

- CLI の外部観測可能な契約は変更しない。
- crate topology は変更せず、Interactor と Application Service は `libs/usecase` 内で責務分離する。

## Reassess When

- 同じ阻害要因により残作業の再計画が 2 回行われても、完了条件へ到達できないとき。
- 純 DI 化の完了後、context 間依存が安定し、新しい delivery mechanism、独立リリース、または異なる依存集合への要求が現れたとき。
- `CompositionRootPureDi` が正当な wiring API を反復して誤検出するとき、または Primary Adapter の責務そのものを変更する必要が生じたとき。

## Related

- `knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md`
- `knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md`
- `knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md`
- `knowledge/adr/2026-07-04-0155-git-sync-dedicated-command.md`
- `knowledge/conventions/type-designer-kind-selection.md`
- `.harness/custom/review-prompts/cli_composition.md`
- `.harness/catalogue-lint/config.json`
