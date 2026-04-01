# knowledge/ ディレクトリへのドキュメント集約

## Status

Accepted

## Context

プロジェクトのドキュメントが 5 箇所に分散していた:

1. `knowledge/` — ADR、research、strategy（Phase 1/2 で部分移行済み）
2. `.claude/docs/` — DESIGN.md、WORKFLOW.md、designs/、schemas/、research/
3. `project-docs/conventions/` — コーディング規約
4. `docs/` — 外部ガイド、architecture-rules.json
5. ルートレベル — CLAUDE.md、DEVELOPER_AI_WORKFLOW.md 等

問題点:
- `.claude/docs/` 配下はハーネス設定領域のため、ドキュメント編集のたびに許可プロンプトが発生
- ディレクトリの役割分担が曖昧で新規参加者にとって分かりにくい
- 同種のドキュメント（research 等）が複数箇所に存在
- DESIGN.md の Canonical Blocks がコードと乖離するリスク

## Decision

`knowledge/` ディレクトリを統合ルートとして全ドキュメントを集約する。

移行マッピング:
- `.claude/docs/DESIGN.md` → `knowledge/architecture.md`（Canonical Blocks 削除、スリム化）
- `.claude/docs/WORKFLOW.md` → `knowledge/WORKFLOW.md`
- `.claude/docs/research/` → `knowledge/research/`（既存とマージ）
- `.claude/docs/designs/` + `schemas/` → `knowledge/designs/`
- `project-docs/conventions/` → `knowledge/conventions/`
- `docs/EXTERNAL_GUIDES.md` → `knowledge/external/POLICY.md`
- `docs/external-guides.json` → `knowledge/external/guides.json`
- `docs/architecture-rules.json` → `./architecture-rules.json`（CI 設定ファイルとしてルートへ）

移行方式: **段階的ハードカット**（シンボリックリンクなし、一時的な dual-read 互換性をコードに追加）

## Rejected Alternatives

- **シンボリックリンクによる段階的移行**: クロスプラットフォーム/DevContainer 環境で複雑化。doc-links CI ガードの検証も弱まる
- **ディレクトリ分散のまま維持**: `.claude/docs/` の許可問題が解決せず、research が 2 箇所に存在する状態が続く
- **`docs/` をメインディレクトリに選択**: `knowledge/` は Phase 1/2 で既に ADR・strategy を収容しており、移行コストが少ない

## Consequences

- Good: 単一の `knowledge/` ディレクトリで全ドキュメントにアクセス可能
- Good: `.claude/docs/` の許可プロンプト問題が解消
- Good: `sotp verify doc-links` CI ガードで今後のリンク切れを自動検出
- Good: Canonical Blocks 削除でコードとドキュメントの乖離リスクが解消
- Bad: 50+ ファイルの参照パス更新が必要（Rust ソース、Python スクリプト、設定ファイル）
- Bad: 完了済みトラックの歴史的参照は旧パスのまま残る（意図的）

## Reassess When

- `.claude/` 配下のドキュメント書き込みに許可不要になった場合（Claude Code の権限モデル変更）
- `knowledge/` の肥大化で別のサブディレクトリ分割が必要になった場合
