---
adr_id: 2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-04:spec-states-gate-tolerate-missing-spec-artifact"
    status: proposed
---
# spec-states commit ゲートを spec 成果物未生成の段階でも通す — トラック解決時のシグナル評価を欠損入力に寛容にする

## Context

commit ゲート(`track-commit-message`)は内部で `cargo make ci` を走らせ、その依存に `verify-spec-states-current`(`cargo run -p cli -- verify spec-states`、spec パス引数なし)が含まれる。

このチェックは spec パス未指定のとき、現在のブランチ名から対象トラックを解決し、`track/items/<id>/spec.md` を組み立てて検証する。spec 成果物(spec.json / spec.md)は Phase 1(spec-design)で初めて生成されるため、Phase 0(init / ADR baseline)の段階では存在しない。

実際に、ADR と metadata.json だけを最初に commit しようとした(pre-track-adr-authoring が想定する「init 直後に review → commit で ADR を初回 commit する」標準フロー)ところ、解決された spec パスが存在しないために spec-states が非ゼロ終了し、`cargo make ci` が落ちて commit ゲート全体が block した。

一方、同じ commit ゲートの `cargo make ci` に含まれる他のトラック解決系チェックは、いずれも「入力が存在しないフェーズでは検証を skip」で一貫している:

- `verify-latest-track`: impl-plan.json が無いフェーズ(Phase 0-2)では検証を skip する。
- `verify-plan-artifact-refs`: spec.json が無ければ silent PASS する。
- `verify-catalogue-spec-refs`: 層カタログが 1 つも存在しなければ、spec.json を読む前に silent PASS する。
- `check-catalogue-spec-signals`: シグナルファイルが無い層を lenient に skip する。

つまり commit ゲート内で、spec-states だけが欠損入力に非寛容で、spec 成果物の生成前は標準フローを塞ぐという不整合がある。これは型カタログ未生成段階の同種の誤爆を解いた先行 ADR `2026-06-01-0406-review-gate-tolerate-missing-catalogue.md` と同型の問題である。

## Decision

### D1: spec-states ゲートのトラック解決経路を欠損入力に寛容にする

spec-states ゲートのトラック解決パス(spec パス未指定で、ブランチから対象トラックを解決する経路)は、検証対象の spec 成果物(spec.json / spec.md のいずれも)が存在しないフェーズでは評価を skip(no-op + success、SKIP 表示)し、非ゼロ終了しない。挙動を、同じ commit ゲートで既に欠損入力を skip している兄弟チェックの寛容さに揃える。

これにより spec 成果物の生成前(Phase 0)でも commit ゲートが通り、「init → review → commit で ADR を初回 commit する」標準フローが機能する。

厳格性は維持する。spec.json が存在するフェーズでは従来どおりシグナルを評価し、Red は引き続きゲートを block する(CI 中間モードでは Yellow は warning、merge ゲートの strict モードでは Yellow も block するという既存の使い分けも不変)。skip は「spec 成果物そのものが存在しない」場合に限定し、成果物があるのに評価を省くことはしない(fail-open を作らない)。

明示的に spec パスを引数で指定する経路(`verify spec-states <path>`)はこの skip の対象外で、従来どおり当該ファイルを検証する(存在しなければエラー)。

具体的な修正対象コードと skip 判定の実装位置は track 実装で確定する。

## Rejected Alternatives

### A. commit ゲートから spec-states を外す

`cargo make ci` の依存から spec-states を削除する案。ゲート全体を bypass すると、spec 成果物が揃った後のフェーズでもシグナル評価が走らず、Red を見落とす(fail-open)。skip は「入力不在のフェーズ」に限定すべきで、チェック自体の削除は危険。却下。

### B. Phase 0 専用の別 commit 経路を新設する

フェーズごとに commit コマンドを分岐させる案。兄弟チェックと同じ「入力不在なら skip」という一様なルールで足りるため、フェーズ別の経路を増やすのは不要な複雑化。却下。

### C. spec パスを必須引数にしてトラック解決経路を廃止する

`verify spec-states` を常に spec パス必須にし、自動解決をやめる案。CI タスクが毎回 spec パスを明示する必要が生じ、ブランチからの自動解決という利点を失う。また他の兄弟チェックはトラック自動解決 + 入力不在 skip で揃っているため、spec-states だけ設計を変えると一貫性が崩れる。却下。

## Consequences

### Positive

- spec 成果物の生成前でも commit ゲートが通り、ADR + metadata の初回 commit を含む標準フローが機能する。
- commit ゲート内のトラック解決系チェックの欠損入力ハンドリングが一貫する。

### Negative

- 「spec 成果物が不在なら skip」の境界判定を誤ると、本来評価すべきフェーズを取りこぼす fail-open リスクがある。実装では「spec.json / spec.md が実在するか」を厳密に判定し、存在するフェーズは必ず評価する。

### Neutral

- spec 成果物が揃った後(Phase 1 以降)のゲート挙動は不変。明示的に spec パスを指定する経路の挙動も不変。

## Reassess When

- フェーズ構成や spec 成果物の生成タイミングが変わり、spec-states が前提とする成果物が増減した場合。
- skip 判定が原因でシグナルの取りこぼし(fail-open)が観測された場合。

## Related

- `knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md` — 型カタログ未生成段階の同型の誤爆を解いた先行 ADR
- `knowledge/conventions/pre-track-adr-authoring.md` — init 直後の ADR 初回 commit フロー
- `knowledge/adr/` — ADR 索引
- `Makefile.toml` の `track-commit-message` / `verify-spec-states-current`
