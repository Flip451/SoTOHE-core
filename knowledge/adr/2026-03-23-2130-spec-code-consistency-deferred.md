# spec ↔ code 整合性チェックは Phase 3 に送る

## Status

Accepted

## Context

ドメインモデリングの保証には 2 つの軸がある:

1. **実行保証**（モデリングをやったか）: Phase 2 の Domain States 必須化 + 信号機で対処
2. **実装保証**（正しく実装されたか）: 既存 CI（verify-domain-strings, verify-domain-purity 等）で部分対処

しかし「spec の Domain States に書いた状態と、code の domain 型が一致しているか」を検証する仕組みがない。spec に「3 状態」と書いて code に 5 状態定義しても、両方の信号が green になりうる。

## Decision

spec ↔ code 整合性チェックを Phase 3 の新項目 3-12 として追加する。BRIDGE-01（`sotp domain export-schema`）の出力と spec.md の Domain States セクションを突合し、未実装・不一致を検出する。

Phase 2 では対処しない。理由:
- BRIDGE-01（syn による型抽出）が Phase 3 の前提
- Phase 2 の段階ではコード側の型情報を機械的に取得する手段がない
- 手動での整合性確認はワークフローに既に組み込まれている（`/track:plan` Phase B）

## Rejected Alternatives

- **Phase 2 で手動チェックリストとして導入**: CI 強制力がなく形骸化する。BRIDGE-01 待ちの方が確実
- **Phase 2 で簡易版（件数一致チェック）を導入**: spec の状態名と code の型名の対応が自明でないため、件数だけでは誤検出が多い

## Consequences

- Good: Phase 2 のスコープが膨らまない
- Good: BRIDGE-01 の出力を活用するため、重複実装を回避
- Bad: Phase 3 までの間、spec ↔ code の矛盾は人間の目に依存
- Bad: 2 段階信号機の両 Stage が green でも矛盾が存在しうる

## Reassess When

- BRIDGE-01 が完成した時点で 3-12 の具体的な実装方針を決定
- Phase 2 の実運用で spec ↔ code の矛盾が頻発する場合は前倒しを検討
