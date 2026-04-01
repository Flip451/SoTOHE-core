<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# ドキュメント群の knowledge ディレクトリへの集約と整理

3箇所に分散したドキュメント群（.claude/docs/, project-docs/, docs/）を既存の knowledge/ ディレクトリに段階的ハードカット方式で集約する。Phase 1 (ADR導入) と Phase 2 (strategy移行) は完了済み。残りの 3 ディレクトリを移行し、sotp verify doc-links CI ガードを追加する。

## Phase A: テスト基盤と CI ガード準備

新パスに対する failing tests を追加し、sotp verify doc-links を実装する。
ファイル移動前にテスト基盤を整え、各タスクの CI 通過を保証する。

- [x] Add failing tests for new paths and temporary dual-read compatibility in planning-only detection, git staging allowlists, verifier lookups, Python scripts, and hook output
- [x] Implement sotp verify doc-links subcommand with tempdir-based tests (generic Markdown link existence check, not wired into CI yet)

## Phase B: CI 設定ファイルと規約の移行

architecture-rules.json を repo root に移動し、Rust/Python の参照を更新する。
project-docs/conventions/ を knowledge/conventions/ に移動し、規約インデックスツールを更新する。

- [x] Move docs/architecture-rules.json to repo root, update all Rust const/doc-comment/test references, Python scripts, and config files
- [x] Move project-docs/conventions/ to knowledge/conventions/, update convention index tooling, Rust verify modules, architecture-rules.json internal convention field, and all doc references

## Phase C: 外部ガイドとドキュメントの移行

外部ガイド資産を knowledge/external/ に移動する。
WORKFLOW.md, research, designs, schemas を knowledge/ に移動する。

- [x] Move docs/EXTERNAL_GUIDES.md to knowledge/external/POLICY.md and docs/external-guides.json to knowledge/external/guides.json, update workflow and command references, delete docs/README.md (content absorbed into knowledge/README.md) and docs/ directory
- [x] Move .claude/docs/WORKFLOW.md, .claude/docs/research/, .claude/docs/designs/, .claude/docs/schemas/ into knowledge/, update review-scope.json and hook references

## Phase D: アーキテクチャドキュメントと最終整備

knowledge/README.md と knowledge/architecture.md を作成する。
verify doc-links を CI に組み込み、互換性フォールバックを削除し、旧ディレクトリを廃止する。

- [ ] Create knowledge/README.md (index with reading order) and knowledge/architecture.md (slimmed DESIGN.md without Canonical Blocks), update all .claude/docs/DESIGN.md references
- [ ] Wire verify doc-links into cargo make ci, remove temporary dual-read compatibility fallbacks, delete abolished directories (.claude/docs/, project-docs/, docs/), final reference sweep
