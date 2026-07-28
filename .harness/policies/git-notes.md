# Policy: Git Notes

## Purpose

コミットへの構造化メモ（git notes）を、コミットハッシュを変えずに実装文脈・対応タスク・主要変更を残すための補助 SSoT として扱う。自動化フローでは `tmp/track-commit/` 配下のスクラッチファイルと guarded `bin/sotp` の file-based operation を正規経路とする。notes は補助情報なので失われてもワークフロー本体は壊れない。

## Scope

- 適用対象: git note を何のために残すか、`bin/sotp git note-from-file` による適用の正規経路、note の remote 共有設定、note に載せてはならない情報。
- 適用外: note の生成手順とフォーマット（`.harness/workflows/track/commit.md` が所有）、コミット本体のメッセージとその guarded 経路（同上、および `.claude/rules/dev-environment.md`）、PR description（`.harness/workflows/track/pr-review.md`）、トラック status の遷移（`.harness/policies/track-lifecycle.md`）。

## Rules

### 生成手順とフォーマットの所在

note をいつ・どの入力から生成し、どのフォーマットで書くかは `.harness/workflows/track/commit.md` が単独で所有する。本書はそれを複製しない — フォーマットを二箇所に置くと、片方に従った agent ともう片方に従った agent が違う note を書く。

### 適用の正規経路

note の適用は `bin/sotp git note-from-file <file> --cleanup` に限る。`git notes add -m` の直叩きは guarded file-based operation を迂回するため禁止。`--cleanup` は適用成功後に scratch file を削除する。

### チーム間での notes 共有

git notes はデフォルトで `git fetch` / `git push` に含まれない。`bootstrap` は clone ごとの
fetch refspec を設定するため、notes を受信するには追加設定は不要である。

```bash
# bootstrap を使わない clone でのみ、fetch 時に notes を自動取得する設定を追加する
git config --add remote.origin.fetch "+refs/notes/*:refs/notes/*"
```

Remote への note 公開は現在 workflow command として提供していない。直接 `git push` を実行せず、
共有が必要になった場合は guarded workflow command を追加してから利用する。

### 参照コマンド

```bash
git notes list                 # note 一覧
git notes show <commit>        # 特定 commit の note 表示
git log --show-notes           # log に note を含めて表示
```

## Examples

- Good: 機械再現可能でない判断（review fixer がどの finding を accept して何を fix したかの要約など）を note に残す。コミット本体からは再構築できない情報が note の存在理由である。
- Bad: `git notes add -m "..."` を直接呼ぶ（guarded file-based operation を使う）。
- Bad: 長文の note を inline text に詰め込む（`tmp/track-commit/note.md` + `bin/sotp git note-from-file` で扱う）。

## Exceptions

- notes の remote 共有が不要な単一開発者ワークフローでは fetch refspec の追加を省略してよい。
- notes が壊れた / 失われた場合の復旧フローは提供しない。ワークフロー本体は notes に依存しないので停止しない。復旧フローを置かないのは失われる内容を受け入れるという判断であって、内容が再構築できるからではない。

## Review Checklist

- [ ] note 適用が `bin/sotp git note-from-file` 経由になっているか（`git notes add` 直叩きが混入していないか）
- [ ] file-based wrapper 用の scratch file（`tmp/track-commit/note.md`）が成功後に削除されているか
- [ ] 機密情報（API key、人物特定情報など）が note に紛れ込んでいないか

## Decision Reference

- [knowledge/adr/README.md](../../knowledge/adr/README.md) — ADR 索引。本書の原典となる ADR はこの索引から辿る
- [.harness/policies/branch-strategy.md](./branch-strategy.md) — `track/<id>` ブランチの作成・切替とブランチ操作ガード
- [.harness/policies/track-lifecycle.md](./track-lifecycle.md) — タスク状態遷移と SSoT 維持
