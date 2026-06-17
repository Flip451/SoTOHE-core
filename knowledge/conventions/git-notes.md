# Git Notes Convention

## Purpose

コミットへの構造化メモ（git notes）を、コミットハッシュを変えずに実装文脈・対応タスク・主要変更を残すための補助 SSoT として扱う。自動化フローでは `tmp/track-commit/` 配下のスクラッチファイル + `cargo make` ラッパー経由の file-based wrapper を正規経路とし、ターミナル直実行用に短い inline text の低レベル経路を残す。notes は補助情報なので失われてもワークフロー本体は壊れない。

## Scope

- 適用対象: `/track:commit` が生成する git note、`cargo make track-note` / `cargo make note` ラッパー、`tmp/track-commit/note.md` / `tmp/track-commit/add-paths.txt` / `tmp/track-commit/commit-message.txt` の scratch file 経路、note の remote 共有設定。
- 適用外: コミット本体のメッセージ（`tmp/track-commit/commit-message.txt` + `cargo make track-commit-message`、`/track:commit` 本体側で定義）、PR description（`/track:pr-review` 側で定義）、トラック status の遷移（`knowledge/conventions/track-lifecycle.md`）。

## Rules

### 自動生成フロー（正規経路）

`/track:commit <message>` は track context（spec.md / plan.md / observations.md / 変更ファイル一覧）から note を生成し、`tmp/track-commit/note.md` を `cargo make track-note` で適用する。`cargo make track-note` は file-based wrapper（`bin/sotp git note-from-file`）経由で note を適用し、成功後に scratch file を削除する。

### 手動生成

```bash
# 正規経路: /track:commit が生成した scratch file を適用して削除
cargo make track-note

# 低レベル: 短い inline text を直接適用（terminal 直実行用）
cargo make note "note text here"
```

`cargo make note` は `git notes add -f -m "$CARGO_MAKE_TASK_ARGS" HEAD` を直接実行する低レベル経路。自動化フローでは `tmp/track-commit/` を使う file-based wrapper を優先する。

選択的 staging を自動化する場合は `tmp/track-commit/add-paths.txt` に repo-relative path を 1 行ずつ書き、`cargo make track-add-paths` を使う。`cargo make add <files>` はターミナル直実行用の低レベル staging として扱う。

コミット本体の正規経路は `tmp/track-commit/commit-message.txt` を用意して `cargo make track-commit-message` を使うこと（唯一のコミット経路）。

### note フォーマット

```markdown
## Task Summary: <brief task description>
**Track:** <track-id>
**Task:** <task name from impl-plan.json or plan.md task entry>
**Date:** YYYY-MM-DD
### Changes
- <filename>: <what changed and why — one line per key file, max 10 bullets>
### Why
<1–3 sentences from spec.md or plan.md rationale>
```

### チーム間での notes 共有

git notes はデフォルトで `git fetch` / `git push` に含まれない。チーム開発やマシン間で notes を共有するには以下を設定する:

```bash
# clone ごとに一度実行（fetch 時に notes を自動取得）
git config --add remote.origin.fetch "+refs/notes/*:refs/notes/*"

# notes を remote に push
git push origin "refs/notes/*"
```

notes はトレーサビリティの補助情報であり、失われてもワークフローは壊れない。

### 参照コマンド

```bash
git notes list                 # note 一覧
git notes show <commit>        # 特定 commit の note 表示
git log --show-notes           # log に note を含めて表示
```

## Examples

- Good: `/track:commit <message>` が track context から `tmp/track-commit/note.md` を生成し、`cargo make track-note` が適用 → scratch file 削除まで完了する。
- Good: 機械再現可能でない判断（review fixer がどの finding を accept して何を fix したかの要約など）を note の `### Why` セクションに 1–3 sentence で残す。
- Bad: `git notes add -m "..."` を直接呼ぶ（ターミナル直実行で短文を残すときも `cargo make note "..."` を経由する。CI / hook ガードが効くように file-based wrapper を優先する）。
- Bad: 長文の note を `cargo make note` の inline text に詰め込む（複数行・複数 backtick を含む長文は `tmp/track-commit/note.md` + `cargo make track-note` で扱う）。

## Exceptions

- notes の remote 共有が不要な単一開発者ワークフローでは fetch refspec の追加を省略してよい。
- notes が壊れた / 失われた場合でも復旧フローは不要（補助情報なのでコミット本体から再構築する）。

## Review Checklist

- [ ] note 適用が `cargo make track-note` / `cargo make note` 経由になっているか（`git notes add` 直叩きが混入していないか）
- [ ] file-based wrapper 用の scratch file（`tmp/track-commit/note.md`）が成功後に削除されているか
- [ ] note フォーマットの主要セクション（`Task Summary` / `Track` / `Task` / `Date` / `Changes` / `Why`）が揃っているか
- [ ] 機密情報（API key、人物特定情報など）が note に紛れ込んでいないか

## Decision Reference

- [knowledge/adr/README.md](../adr/README.md) — ADR 索引。本 convention の原典となる ADR はこの索引から辿る
- [knowledge/conventions/branch-strategy.md](./branch-strategy.md) — `track/<id>` ブランチでのコミット運用
- [knowledge/conventions/track-lifecycle.md](./track-lifecycle.md) — タスク状態遷移と SSoT 維持
