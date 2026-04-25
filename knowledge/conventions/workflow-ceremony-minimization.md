# Workflow Ceremony Minimization Convention

## Purpose

track ワークフローの形式的手順 (ceremony) を最小化し、WIP コストが実際の設計・実装作業より早く膨らまないようにする。エラーを実質的に防がない ceremony (形骸化した `approved` / `Status` フィールド、実成果物と乖離する要約ベース事前承認など) は廃止する。不可逆なミスを食い止める ceremony (push / commit ガード、外部 API 呼び出しなど) は残す。

## Scope

- 適用対象: `/track:*` コマンド (planning / design / implementation / review / commit)、それらのライフサイクルを記述する SKILL / command / agent ドキュメント、track artifacts を検査する CI ゲート。
- 適用外: プロジェクト固有のコーディング規約 (hexagonal-architecture、source-attribution など) や、明示的な人間承認を要求するセキュリティ境界。

## Rules

- **成果物レビューは事後方式**: 成果物の生成ステップはフェーズごとのゲート (SoT Chain 信号機評価 🔵🟡🔴 または binary pass/fail) + 実成果物の事後レビューで判定する。要約ベースの事前承認は作らない (要約が実物と乖離して見逃しが増えるため)。
- **人工的な状態フィールドを作らない**: `status: approved/draft`、`approved_at`、トップレベル `content_hash` のような人工状態は形骸化するので導入しない。ゲートは file 存在 + signal 評価で表現する。
- **ユーザー事前承認は不可逆 action 限定**: 次のケースに限って事前承認を取る。
  - `git push` / `git commit` (既存ガード)
  - 外部 API 呼び出し (PR / issue 作成)
  - 破壊的なファイルシステム操作
  - 環境破壊 (CI 設定・lockfile の強制書き換え)
- **file 存在 = phase 状態**: CI ゲートは「該当ファイルがあれば検証、なければ skip」の単純分岐にする。optional field の条件付きチェックは避ける。
- **形式的手順の追加は justification 必須**: 新しい承認ステップや Status フィールドを提案する際は、それが防ぐ具体的エラーを ADR で明示する。

## Examples

- Good: `/track:plan` が spec.json / plan.md を生成して実物を user に提示、signal 評価が 🔵 の要素は pass、🟡 の要素のみユーザー確認する。機械検証不能な観測値が出た場合のみ `observations.md` を任意作成する。
- Good: `verify-latest-track-local` が `impl-plan.json` の存在を検出したときのみ task 項目をチェックする。
- Bad: spec.json に `status: "approved"` を書き込み、commit 前に `cargo make spec-approve` を要求する (CI 信号機で同等の判定ができる)。
- Bad: `/track:plan` の Phase 1 で要約を提示して承認を取り、Phase 3 で実成果物を別途作成する (要約と実物が乖離して前回承認の意味が希薄化する)。

## Exceptions

- 事前承認ゲートの **追加** を検討する場合は ADR で記録する。候補は Rules セクションの 4 項目に限定されるが、プロジェクト固有の不可逆操作 (本番 DB への破壊的 migration など) が発生したら明示的に上書きする。
- 本 convention で禁止する `approved` 概念を一時的に保持する必要がある場合、移行期間の終了条件を track artifact で明文化すること。

## Review Checklist

- [ ] 新規 / 変更されたワークフローステップが事後レビュー方式に沿っているか
- [ ] 人工状態フィールド (`approved` 等) を追加していないか
- [ ] file 存在ベースの gate に置き換えられる条件付きチェックを残していないか
- [ ] 事前承認を要求する action が不可逆かつ Rules セクションの 4 カテゴリに該当するか

## Decision Reference

- [knowledge/adr/README.md](../adr/README.md) — ADR 索引。本 convention の原典となる ADR はこの索引から辿る
- [knowledge/conventions/adr.md](./adr.md) — ADR 化の基準
