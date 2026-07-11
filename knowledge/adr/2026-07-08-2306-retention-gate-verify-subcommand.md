---
adr_id: 2026-07-08-2306-retention-gate-verify-subcommand
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01B7qXkotQ9TA1WJoz4Xyefg:2026-07-09"
    candidate_selection: "from:[A,B,C,D,verify-subcommand] chose:verify-subcommand"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01B7qXkotQ9TA1WJoz4Xyefg:2026-07-09"
    status: proposed
---
# retention gate の verify サブコマンド化

## Context

track `retire-todo-marker-state-and-track-docs-2026-07-08` の T011 で、廃止識別子の再出現を検知する retention gate を `apps/cli/tests/retention_gate.rs` の integration test として導入した。しかしこの配置には 2 つの問題がある。

1. テンプレート利用者への越境: `apps` は境界 manifest で overlay 分類（overlay 側に対応物がないため as-is 出荷）であり、テストが exported template に同梱される。テストは自分が置かれた repo の working tree をスキャンするため、利用者の CI が利用者自身の命名や文書で fail する。利用者の repo には廃止の歴史がなく、守るべき退行が存在しない。

2. 実装場所の不整合: 本 repo で「repo tree をスキャンする CI ゲート」は `sotp verify` サブコマンド + Makefile gate task が確立パターン（verify latest-track / module-size / doc-links 等。撤去した verify tech-stack 自身もこの形だった）。テストスイートに repo スキャンを埋めると、配布除外に脆い迂回が必要になる。

なお per-file の manifest exclude は境界 manifest の prefix-free 不変量により構築不能であることを確認済み。

## Decision

### D1: retention gate を verify サブコマンドとして再実装する

integration test (`apps/cli/tests/retention_gate.rs`) を削除し、スキャナを verify チェーン（infrastructure の checker + usecase の port/service method + CLI サブコマンド）に移す。Makefile に gate task を追加し ci 依存に接続する。検査の意味論（存在ベース / fail-closed / negative test 保持）は変えない。

### D2: 配布 CI からは Makefile 非接続で除外する

retention gate は maintainer repo 限定の退行ガードとする。配布除外は overlay Makefile の ci 依存配列に gate task を載せないことで実現する（撤去した verify tech-stack と同じ配線形）。利用者側での同等ゲートの要否は利用者の責任範囲とする。

## Rejected Alternatives

### A. 境界 manifest への per-file exclude 追加

manifest の pattern は prefix-free 不変量（entry 同士が祖先関係を持てない）を持ち、`apps` 配下の per-file entry は構築時に拒否される。却下。

### B. overlay Makefile の test task に nextest 除外フィルタを追加

テストのまま残るため `cargo test` 直叩きで発火し、llvm-cov 経路に重複フィルタが要り、rename drift を smoke gate が検出できない（export 直後の tree は token-clean のためフィルタが死んでいても green）。却下。

### C. テストの runtime skip（overlay/ 不在判定）

存在ベースではあるが、配布政策の知識をテスト内部に埋める配置の歪みが残る。却下。

### D. 環境変数による opt-out

宣言的状態スイッチの再導入であり、TODO マーカー廃止の趣旨と矛盾する。却下。

## Consequences

### Positive

- 利用者の命名・文書の自由が回復する（配布除外が「接続しない」だけで構造的に成立し、フィルタの失効や直叩きの抜け道がない）
- repo tree スキャン型ゲートの実装パターンが verify サブコマンドに統一される
- ゲートの単発実行（`cargo make verify-*`）が可能になり診断性が上がる

### Negative

- VerifyPort / VerifyService の契約が 16 → 17 methods に再拡大する（前 track で縮めた直後の拡張）
- 新 track 1 本分の設計・実装・レビューコストが発生する

### Neutral

- negative test（スキャナの検出能力検証）は infrastructure checker の unit test に移動する
- テストファイルは削除されるが、検査の意味論（対象 surface / トークン集合 / fail-closed）は不変

## Reassess When

- 同形式の repo-tree 参照テスト（shipped-config と code の整合を pin するもの）の verify 化に実需が出たとき——これらは利用者 repo でも意味を持つため本件では対象外とした
- TODO マーカー併存行ルールの誤検出が maintainer repo で繰り返されたとき（検査ルールの絞り込みを別途検討）
- テンプレート利用者から同種の退行ガードへの opt-in 需要が観測されたとき

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md` — retention gate を導入した元の廃止決定（D4）
- `knowledge/adr/2026-07-06-1717-template-extraction-boundary.md` — 境界 manifest / overlay 方式と prefix-free 不変量の出典
- `knowledge/conventions/responsibility-boundary.md` — 利用者 posture は利用者の責任という境界原則
- `knowledge/conventions/enforce-by-mechanism.md` — 機械検証優先の原則
- `knowledge/conventions/workflow-ceremony-minimization.md` — 宣言的状態スイッチを避ける原則（代替案 D の却下根拠）
