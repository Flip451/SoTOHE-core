---
adr_id: "2026-08-14-0442-template-export-test-source-fixture-placement"
decisions:
  - id: D1
    review_finding_ref: "track:2026-08-14-workflow-byproduct-disk-hygiene:T003 blocker; rollback-diagnoser verdict: D3 scaffold placement vs source-root guard"
    candidate_selection: "from:[isolated_source_fixture,test_only_guard_allowance,external_temp_area] chose:isolated_source_fixture"
    status: proposed
---
# template export 統合テストの source fixture 隔離配置

## Context

template export の実 workspace 統合テストでは、scaffold の書き出し先を
`CARGO_TARGET_TMPDIR`（通常は実 workspace の `target/tmp`）へ移す方針が採られていた。
しかし export の production guard は、出力先が `workspace_root` または `overlay_dir` の
内側に入ることを拒否する。実 workspace 自体を `workspace_root` に指定したまま
`target/tmp` へ出力すると、この拒否条件に必ず該当する。

一方、guard をテスト時だけ緩和すると、production と異なる安全境界を統合テストが通ることになり、
実際の export 経路を検証できない。書き出し先を再び汎用の `/tmp` に戻すと、テストプロセスの
強制終了後に既存の maintenance scope から回収できない残骸が残る。このため、guard、
production export behavior、異常終了後の回収可能性を同時に維持できる配置が必要になった。

## Decision

### D1: 実 workspace 由来の source fixture と scaffold を清掃可能な親の sibling に置く

実 workspace の template export 統合テストは、実 workspace をそのまま `workspace_root` に
指定しない。各 export 実行について一つの所有された一時親ディレクトリを
`CARGO_TARGET_TMPDIR` 配下に作り、その直下へ次の sibling を配置する。

- `source/`: 実 workspace の現在内容から template export の入力面を materialize した隔離
  source fixture。実際の boundary manifest、その manifest が参照する workspace 内容、overlay
  内容を保持し、production CLI にはこの fixture 内の `workspace_root`、`manifest_path`、
  `overlay_dir` を渡す。
- `scaffold/`: production CLI が生成する出力先。`source/` とその配下の overlay のどちらにも
  入らない sibling とする。

source fixture は symlink で実 workspace を指し戻さず、export が読む時点で独立した
filesystem tree として materialize する。fixture の作成は現在の実 workspace の export 入力を
反映し、実 boundary manifest と実 overlay を別の簡略化した test manifest へ置き換えない。
これにより、統合テストは実際の出荷契約と production binary を検査しながら、production の
`ensure_output_dir_outside_source_roots` を変更せず通過する。

一時親の解決は、空でない `CARGO_TARGET_TMPDIR` を優先し、未定義または空の場合は実 workspace の
`target/tmp` を使う。正常終了時は所有された一時親を削除し、強制終了で残った場合も全体が
`target/` 配下に留まるため、`cargo clean` と `sotp maintenance` の既存 scope で回収できる。
guard を迂回する test-only flag、環境変数、条件分岐は導入しない。

本 D1 は D3 を supersede せず、
`knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D3` を refine する。
同 D3 は引き続き integration / in-process の全 scaffold を `CARGO_TARGET_TMPDIR` 配下へ置く
有効な決定であり、本 D1 は実 workspace 統合テストについて、その一時親の直下に
materialized `source/` と `scaffold/` を sibling として置く追加制約である。同 ADR の D1、D2、
D4、D5 は変更しない。

## Rejected Alternatives

### A. production guard に test-only の出力許可を追加する

統合テストが production と異なる安全境界を通るため、実際の nested-output 拒否を検証できない。
許可を process environment や hidden flag で運ぶ方式は、production 実行から到達可能な迂回路も
増やすため採用しない。

### B. 実 workspace を source root のまま `target/tmp` へ出力する

出力先が source root の子孫になるため、production guard を維持する限り成立しない。
guard の拒否を期待どおり確認する負例には使えるが、scaffold を生成して検査する統合テストの
配置には使えない。

### C. source root の外側にある汎用 `/tmp` または別の external temp area へ出力する

guard は通過するが、プロセス強制終了後の残骸が `cargo clean` と現行の `sotp maintenance`
scope から外れる。回収不能な大容量 scaffold を再発させるため採用しない。

### D. source fixture を symlink tree として構成する

fixture の作成コストは下がるが、export の symlink 拒否と衝突し、実 workspace への参照を残すため
source root の隔離にもならない。独立した filesystem tree の materialization を採用する。

## Consequences

### Positive

- production の source-root guard と export behavior を変更せず、成功経路を同じ binary で検査できる。
- 正常終了時に削除できなかった source fixture と scaffold も `target/` 配下に留まり、既存の清掃
  command で一括回収できる。
- 実 boundary manifest と実 overlay を使うため、出荷面の drift を統合テストで引き続き検出できる。

### Negative

- 統合テストは export 前に source fixture を materialize するため、fixture 作成の I/O と補助実装が
  増える。
- テストは実 workspace の絶対パスそのものからの export ではなく、その現在内容から作った隔離
  snapshot を export する。literal な source-root 配置の拒否は別の負例で担保する必要がある。
- fixture materialization が実 export 入力面を欠落させると false success になり得るため、実
  boundary manifest が要求する入力を欠落なく反映する検査が必要になる。

### Neutral

- production 利用者が指定する `workspace_root`、`manifest_path`、`overlay_dir`、`output_dir` の契約は
  変わらない。
- template export の成果物内容と binary transplant の方式は変更しない。

## Reassess When

- production export が nested output を安全に扱える別の原子的 materialization 方式を導入したとき。
- cargo または `sotp maintenance` の清掃 scope が `target/tmp` を含まなくなったとき。
- 実 workspace の export 入力面を隔離 fixture に欠落なく materialize するコストが、統合テストの
  実行時間または容量の支配要因になったとき。

## Related

- `knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D3` — integration / in-process の
  全 scaffold を `CARGO_TARGET_TMPDIR` 配下へ置く有効な決定。本 D1 は実 workspace 統合テストの
  sibling 配置を追加制約として refine する。
- `knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md` D4 — 一時親の process-lifetime
  所有と正常終了時の削除を補完する決定。
- `knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md` D5 — 大容量 test binary の複製量を
  抑える決定。本 D1 はその transplant 方針を変更しない。
