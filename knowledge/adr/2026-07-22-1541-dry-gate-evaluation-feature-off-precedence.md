---
adr_id: "2026-07-22-1541-dry-gate-evaluation-feature-off-precedence"
decisions:
  - id: D1
    review_finding_ref: "rollback-diagnoser:2026-07-23 routing_target=adr; dry gate evaluation and feature-off precedence"
    status: proposed
---
# dry gate 評価点における設定無効と feature 無効の優先規則

## Context

DRY ゲートには、commit 可否を判定する `sotp dry check-approved` と、fixpoint 解決時に DRY Fix Phase（DFP）を起動する必要があるかを判定する評価点がある。DRY ゲート設定の `enabled: false` は、これらの評価点で semantic duplicate 検出と DFP 修正ループを実行せず、「通過」または「DFP 不要」を返す意味として記録されている。

一方、`semantic-dup` feature 無効の binary では dry 系コマンドを一律に fail-closed とする決定も記録されている。この二つの決定は、feature 無効かつ `enabled: false` の binary が gate 評価点へ到達した場合の優先規則を定めていない。feature の有無を先に判定すると、利用者が DRY ゲートを無効にしていても guarded commit が常に失敗し、設定無効を「ゲート通過」とする意味が失われる。

## Decision

### D1: gate 評価点では `enabled` を feature の有無より先に評価する

`sotp dry check-approved` と、fixpoint 解決時の DFP 起動判定は gate 評価点として、まず DRY ゲート設定の boolean パラメータ `enabled` を評価する。

- `enabled: false` の場合、`semantic-dup` feature の有無にかかわらず semantic duplicate 機能を実行せず、`check-approved` は通過、DFP 起動判定は DFP 不要を返す。
- `enabled: true` かつ `semantic-dup` feature が無効の場合、gate 評価点は feature 無効を明示するメッセージと非成功終了で fail-closed とする。
- `enabled: true` かつ `semantic-dup` feature が有効の場合、既存の blocking gate と DFP 判定を実行する。

この優先規則は `2026-07-20-1608-disk-footprint-and-dry-feature-gating.md` D2 を gate 評価点に限って **refines** する。`write`、`results`、`fix-local` など、gate 評価点ではなく利用者が dry 処理そのものを実行するコマンドは同 D2 の対象に残り、feature 無効時に一律 fail-closed とする。

理由は、`enabled: false` が semantic duplicate 機能を利用できるという表明ではなく、DRY ゲートを評価対象から外す設定だからである。設定無効を先に処理すれば重量 feature を必要とせずに「ゲート通過」の意味を維持でき、設定有効時と dry 処理の直接実行時には feature 不足を silent skip せず検出できる。

## Consequences

### Positive

- 軽量な feature 無効 binary でも、DRY ゲートを無効にした構成の guarded commit chain をブロックしない。
- `enabled: false` の「検出も DFP も実行せず通過」という意味が、binary の feature 構成に左右されない。
- DRY ゲートを有効にした構成では feature 不足を明示的に fail-closed とし、意図しない検査省略を防ぐ。

### Negative

- dry 系の入口は、gate 評価点か dry 処理の直接実行かによって feature 無効時の結果が異なるため、その境界を保つ必要がある。
- gate 評価点は feature 判定より前に設定を読めなければならず、設定読込エラーと feature 不足を区別して扱う必要がある。

### Neutral

- `enabled: true` かつ feature 有効時の blocking 性、検出内容、DFP 修正ループは変わらない。
- gate 評価点以外の dry 系実行コマンドに対する feature 無効時の fail-closed 規則は変わらない。

## Rejected Alternatives

### A. feature の有無を常に最初に判定する

feature 無効 binary の挙動は単純になるが、`enabled: false` でも gate 評価点が失敗し、設定無効を通過として扱う既存決定を破るため採用しない。

### B. feature 無効時はすべての dry 系入口を通過扱いにする

gate 評価点以外の明示的な dry 処理まで silent skip となり、利用者が要求した処理の未実行を検出できないため採用しない。

## Reassess When

- DRY ゲートの `enabled` 設定がリポジトリ全体の boolean ではなくなり、トラック単位や違反単位の override など異なる粒度を持つとき。
- commit 可否と DFP 起動判定以外に新たな gate 評価点が追加されるなど、gate 評価点の集合や責務境界が変わるとき。
- `semantic-dup` feature の既定が有効へ戻る、または feature 境界自体が廃止され、feature 無効 build を前提とする優先規則の必要性が変わるとき。

## Related

- `knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md` D2 — feature 無効時の dry 系コマンドの fail-closed 規則。本 ADR D1 は gate 評価点の優先規則に限ってこの決定を refines する。
- `knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md` D2 — `enabled: false` のとき gate 評価点を通過または DFP 不要として扱う決定。本 ADR D1 はこの意味を feature 無効 build にも適用する。
