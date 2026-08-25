---
adr_id: "2026-08-21-0055-type-identity-fully-qualified-paths"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-21 dangling-id root-cause adjudication"
    candidate_selection: "from:[full-path-identity,duplicate-rename-hotfix,duplicate-name-lint] chose:full-path-identity"
    status: proposed
---
# 型シグナル評価の型識別を完全修飾パスで行う

## Context

型シグナル評価器は、複数の解決経路で型・トレイトを非修飾の短い名前で識別している（impl identity map の generic 引数、trait パス正規化の短名 fallback）。同一 crate 内に同名の公開型を複数持つことは合法な Rust であり、実際に大規模リファクタリングが同名の入力型 2 つを別モジュールに生んだ時点で、name→id 対応が 1 つに潰れ、閉世界検査が `DanglingId` で fail する。track・catalogue の内容に依存せず決定論的に再現し、全 track の型シグナル評価を封鎖する。

短名識別による衝突の可能性は、過去の track の観測記録に「将来修正候補」として既に記録されていた（`From<serde_json::Error>` 等の generic 引数の既知の偽陽性衝突）。

## Decision

### D1: 識別キーを完全修飾パス（モジュールパス + 名前）にする

型・トレイトの識別を要するすべての解決経路 — impl identity、trait パス正規化、generic 引数の識別、catalogue 参照の解決 — は、rustdoc の `paths` を権威として完全修飾パスをキーに用いる。短名 fallback は、列挙済みの compiler-internal トレイト（ユーザーコードから命名不能なもの）に限定して残す。それ以外でパスが解決できない場合は、対象を診断メッセージで名指しして fail-closed とし、短名への暗黙の退化はしない。

利用者が短名を書く面(ソースコード・catalogue の字句照合対象の署名表記)は変更しない。短名の入力が複数候補に一致して文脈から一意に解決できない場合は、候補の完全パスを列挙して fail-closed とする(暗黙にどれかへ解決しない)。

## Rejected Alternatives

- **重複側の改名（応急）**: 封鎖は解けるが、合法なコード形状が評価器の都合で禁止されたままになり、次の重複で再発する。
- **同一 crate 内の重複型名を lint で禁止する**: 合法な Rust の一部を拒否し続ける受理集合の歪みであり、評価器の欠陥をユーザー制約に転嫁する。
- **短名 fallback の維持 + 衝突検出時のみエラー**: 検出時点で評価は失敗しており現状と変わらない。識別自体を正すべき。

## Consequences

- 良: 同名型が何個あっても識別が一意になり、封鎖が解ける。観測記録にあった generic 引数の偽陽性衝突も同時に解消される。
- 負: 短名を前提にした既存テスト・fixture の改訂が必要。
- 中立: 引き金となった重複型名はそのまま残ってよい（合法であり、本修正後は問題にならない）。

## Reassess When

- rustdoc JSON の `paths` の意味論（完全パスの供給保証）が変わったとき。
- compiler-internal トレイトの列挙に新種を追加する必要が生じたとき。
