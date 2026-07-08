---
adr_id: 2026-07-08-0541-template-export-sotp-binary-transplant
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-0182uGsSwBmcuwAHkF2GHn8R:2026-07-08"
    candidate_selection: "from:[self-transplant,A,B,C] chose:self-transplant"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-0182uGsSwBmcuwAHkF2GHn8R:2026-07-08"
    status: proposed
---
# export は自バイナリを移植する

## Context

現行実装では `sotp template export` は `bin` を exclude し、出力テンプレートの `bin/sotp` は bootstrap 内の install-sotp が `.harness/config/sotp-version.json` の固定タグから `cargo install` で導入する。

テンプレート利用者の初回導入手順を検証したところ、3 点が顕在化した。

1. 公開リポジトリにタグが 1 本も無く、初回導入が成立しない。
2. 利用者は準備段階で SoTOHE-core を clone し `cargo make build-sotp` 済みであり、同一ホストで同じバイナリを固定タグから再ビルドするのは冗長。
3. export に使ったバイナリと出力テンプレート側の sotp のバージョン一致が保証されない。

## Decision

### D1: export は実行中の自バイナリを出力ツリーへ移植する

`sotp template export` は、実行中の自バイナリを出力ツリーの `bin/sotp` へ移植する（実行権限を保持したコピー）。これにより export に使ったバイナリと出力テンプレートの sotp は常に同一になる。

### D2: 固定タグ経路は更新・他ホスト導入用として残す

固定タグ経路（`sotp-version.json` + bootstrap の install-sotp）は廃止せず、更新時と他ホストでの再導入用の経路として残す。初回導入はタグ非依存になり、タグの位置づけは「初回導入の前提」から「更新時の再現性」へ変わる。

## Rejected Alternatives

### A. 現行方式のみ（固定タグから cargo install）を維持

タグ運用が初回導入のブロッカーになり、同一ホストでの再ビルドが冗長。export に使ったバイナリと出力側のバージョン一致も保証されないため却下。

### B. プレビルトバイナリ配布 (GitHub Releases)

ビルド・署名・マルチプラットフォーム対応の運用コストが先行する。利用プロジェクトが複数現れてから再評価する事項として却下（先送り）。

### C. テンプレートに sotp ソースを同梱し利用者側でビルド

利用者のワークスペースに sotp の重いビルド前提が漏れる。境界分離の趣旨に反するため却下。

## Consequences

### Positive

- 初回導入がタグ非依存で完結し、clone ⇒ build ⇒ export ⇒ bootstrap で閉じる
- export に使ったバイナリと出力テンプレートの sotp が常に同一になる
- 同一ホストでの二重ビルドが消える

### Negative

- 移植バイナリはビルドしたホスト固有で、別ホストのメンバーは D2 の固定タグ経路で再導入が必要
- 出力にバイナリが含まれるため、出力を git 管理する際の bin の ignore 方針確認が要る

### Neutral

- `sotp-version.json` / install-sotp の役割が「初回導入」から「更新・他ホスト導入」へ移る

## Reassess When

- プレビルトバイナリ配布（GitHub Releases 等）を開始したとき
- 利用プロジェクトが複数になり、複数ホストへの初回導入が主経路になったとき
- ビルドホストと利用ホストの分離（クロスプラットフォーム利用）が現実の要求になったとき

## Related

- `knowledge/adr/2026-07-06-1717-template-extraction-boundary.md` — 本 ADR が変更する導入経路（bootstrap の固定タグ導入）を定めた元の決定
- `knowledge/adr/` — ADR 索引
