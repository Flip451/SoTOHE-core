---
adr_id: 2026-05-20-0413-tddd-struct-inherent-method-symmetric-comparison
decisions:
  - id: D1
    user_decision_ref: "chat_segment:make-catalogue-schema-permissive-2026-05-19:2026-05-20"
    candidate_selection: "from:[skip-opt-out-maintain,action-aware-skip,enum-side-skip,symmetric-both-side-compare] chose:symmetric-both-side-compare"
    status: proposed
---
# TDDD: struct の inherent method 比較を enum と同じ両側対称比較に統一する

## Context

`make-catalogue-schema-permissive-2026-05-19` トラックの作業中に、TDDD signal evaluator (`libs/infrastructure/src/tddd/signal_evaluator_v2/structural_eq.rs`) に以下の問題が判明した。

### TDDD-BUG-03: struct の inherent method 比較 skip

`structs_structurally_equal` (l.211 付近) において、A 側 (catalogue) の inherent method マップが空 (`a_methods.is_empty()`) のとき、C 側 (rustdoc / 実コード) の method マップを **build せずに比較を全 skip** する opt-out が存在する。

この動作はコード内コメント (`structural_eq.rs:965-989`、「`methods: []` means no method contract declared」) で非公式に定義され、regression テストで保護されているが、**ADR として決定・記録された設計ではない**。

結果として生じる問題:

- catalogue が `methods: []` と記述すれば、実コードにどれだけ inherent method があっても method-drift を検出せず Blue になる (人工的 Blue)。
- action (Add/Modify/Reference) や role (ValueObject 等) による分岐はなく、すべての struct エントリで skip が発生する。
- **enum との非対称**: `enums_structurally_equal` (l.316-349) は A/B 両側を build して対称比較しており、opt-out が存在しない。struct だけが非対称になっている。

この穴により、catalogue 設計者が `methods: []` と書くことで strict signal gate を意図的あるいは無意識に迂回できる状態となっており、「catalogue は実コードと structurally identical」という TDDD の基本原則 (`knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` 参照) に反する。

## Decision

### D1: struct の inherent method 比較を enum と同じ両側対称比較に統一する (skip opt-out 撤廃)

`structs_structurally_equal` の `a_methods.is_empty()` による skip を撤廃し、`enums_structurally_equal` と同様に A/C 両側の inherent method マップを build して対称比較する。

期待される比較結果の変化:

<!-- illustrative, non-canonical -->
```text
A 側 methods: []  + C 側 methods: []  → 一致 → Blue   (純粋 data struct、現状と同じ)
A 側 methods: []  + C 側 methods: [f] → 不一致 → Yellow (declare 漏れ。現状は誤って Blue)
A 側 methods: [f] + C 側 methods: [f] → 一致 → Blue   (実 method を宣言済み、現状と同じ)
A 側 methods: [f] + C 側 methods: []  → 不一致 → Yellow (不存在 declare。現状と同じ)
```

現在の regression テスト `test_plain_struct_empty_a_side_methods_matches_c_side_with_methods` (`structs_structurally_equal` の skip 挙動を保護しているもの) は本決定と矛盾するため、本決定の実装時に削除または修正する。

## Rejected Alternatives

### A. skip opt-out を維持 (現状のまま)

却下。TDDD 原則 (catalogue は実コードと structurally identical) に違反し、`methods: []` で impl-drift を隠せる穴が残る。strict signal gate を迂回できる構造的欠陥を恒久化することになる。

### B. action-aware skip (modify action のときのみ C 側比較、Add は skip 維持)

却下。部分対処であり、Add action でも `methods: []` で C 側 method を隠せる穴が残る。enum との非対称も解消しない。全 action で対称比較する D1 が TDDD 原則と整合する。

### C. enum 側も struct と同じ opt-out にして対称化 (両方 skip する方向)

却下。catalogue が実コードと structurally identical であるという TDDD 原則を捨てる方向であり、method-drift を恒久的に検出できなくなる。signal gate が形骸化する。

## Consequences

### Positive

- struct/enum の inherent method 比較が対称になり、TDDD 原則 (catalogue は実コードと structurally identical) が回復する。
- `methods: []` による人工 Blue / impl-drift 隠蔽の穴が塞がれ、strict signal gate が迂回不能になる。

### Negative

- 本決定 (struct method 対称比較) を実装するトラックでは、対象 struct 型 (domain の modify ValueObject、usecase の Interactor 等) の catalogue エントリに実 inherent method を宣言する必要がある (宣言しなければ Yellow になる)。これは当該トラックの catalogue を新 schema で記述する作業であり、他トラックの過去 catalogue を更新する後方互換作業ではない (他トラックの catalogue は CI の signal 評価対象外であり、本決定では触らない)。
- 現在の regression テスト (`test_plain_struct_empty_a_side_methods_matches_c_side_with_methods`) は本決定と矛盾するため削除または修正が必要。
- `structs_structurally_equal` の呼び出し元への影響として、method マップ build のコスト増が生じるが、影響は軽微と見込まれる。

## Reassess When

- struct method 対称比較への変更後、純粋 data struct (inherent method なし) が誤って Yellow になるケースが確認された場合。
- 当該トラックの catalogue に実 method を宣言する作業で想定外の Red/Yellow が大量発生し、作業コストが許容範囲を超える場合。

## Related

- `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` — catalogue は実コードと structurally identical という原則の出典
- `knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md` — 本 ADR の問題が判明したトラックの母体 ADR
