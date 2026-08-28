---
adr_id: "2026-08-28-1034-cross-crate-add-declaration-resolution"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01Dajmv24iFw9CX22WMxG3Fw:2026-08-28"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01Dajmv24iFw9CX22WMxG3Fw:2026-08-28"
    status: proposed
---
# 参照先 crate の add 宣言を解決集合に加える

## Context

用語: **解決集合**とは、評価器が catalogue の型参照を照合する識別子の集合である。**add 宣言**とは、catalogue が `action: add` で宣言した、まだ rustdoc に無い型または trait である。

型シグナルの評価は層ごとに独立して行われる。各層の解決集合は、その層の rustdoc `paths`（当該 crate が参照済みの外部 crate 項目を含む）と、その層自身の catalogue の add 宣言から合成した summary だけで作られる。

このため宣言先行は自層に閉じている。同じ track の別層 catalogue が add 宣言した型を参照すると、実装前はその型が rustdoc にも自層の合成 summary にも無いため、codec は未解決識別子として fail-closed で停止する。上層が下層の新しい型を受け取る設計（例: cli の引数型が cli_driver の新しい検証済み入力型を持つ）は hexagonal では常態であり、これが実装前に評価できない。

短名で識別していた時代は名前だけの比較でこの参照が偶然通っていた。完全修飾識別（`2026-08-21-0055-type-identity-fully-qualified-paths.md`）と、自層の add 宣言を解決集合に加える修復（`2026-08-25-0804-post-fq-identity-regression-repair.md` D1）の後に、層をまたぐ場合だけが残った。

## Decision

### D1: 同じ track の他層 catalogue の add 宣言を外部 crate 項目として解決集合に加える

ある層の解決集合を作るとき、同じ track の他の TDDD 有効層の catalogue が add 宣言した型と trait を、その宣言層の crate に属する外部項目として合成し、加える。

参照側 catalogue に追加の記述は求めない。

「他の TDDD 有効層」の集合は `architecture-rules.json` の TDDD 有効層に委ね、本 ADR はそれを列挙しない。catalogue ファイルが track dir に無い層は宣言なしとして扱う。

### D2: 合成する外部項目の識別と配置は宣言層の規則に従う

合成する項目の識別子は、宣言層の catalogue の crate 名を根とする。bin ターゲットの別名は既存の正準化（`2026-08-25-0804` D2）を通す。

モジュールの配置は宣言層自身の解決（明示した `module_path`、または省略時は `2026-08-25-0804` D3 の規則による未確定）をそのまま用いる。

同一の識別子が参照側の rustdoc `paths` に既にあれば rustdoc を優先し、合成しない。

### Existing decision relationship

本 ADR の D1 は `2026-08-25-0804-post-fq-identity-regression-repair.md` D1（catalogue が宣言する add 型を解決集合に加える）を **refines** する。同 D1 の「解決集合の構築は 1 箇所で行う」原則は変更せず、その 1 箇所の入力に他層の add 宣言を加える。

## Rejected Alternatives

- **A: 参照側 catalogue に cross-crate 宣言を書かせる**: 同じ型の宣言が二重化し、宣言層と参照層の不整合を新たに検査する必要が生じる。catalogue は自 crate の型を宣言するものである。
- **B: cross-crate 参照だけ短名フォールバックで黙認する**: 完全修飾識別が閉じた短名識別を経路限定で復活させ、`2026-08-25-0804` D1 が廃止した経路別の add 特例を再導入する。
- **C: 実装されるまで cross-crate 参照を未解決のまま許容する**: 宣言先行の型契約が層をまたぐところで評価されなくなり、Phase 2 のゲートがその境界だけ穴になる。

## Consequences

- 良: 宣言先行が層をまたいで成立し、上層が下層の新型を実装前に受け取れる。
- 良: 宣言は各層 catalogue に 1 回だけ書かれ、参照側に追加の記述は不要。
- 中立: 層の評価は同じ track の他層 catalogue を読むようになり、評価の入力が自層に閉じなくなる。
- 負: 他層の add 宣言が誤っていると、参照層の評価もその誤りを引き継ぐ。

## Reassess When

- 1 つの track が複数の track dir や別 workspace の catalogue を参照するようになったとき。

## Related

- `knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md` D1 — D1 の refines 対象。D2 / D3 は D2 が識別と配置の規則として参照する。
- `knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md` — 完全修飾識別の起点。本 ADR が扱う残余はその導入で顕在化した。
