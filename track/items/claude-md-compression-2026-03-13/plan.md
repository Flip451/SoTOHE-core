<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# CLAUDE.md 50行以下圧縮 + workspace-tree コマンド化 + 07-dev-environment.md 追記

CLAUDE.md を 250 行から 50 行以下に圧縮する。
詳細ルールは .claude/rules/08-10 に分離する。
Workspace Map は CLAUDE.md から削除し、architecture-rules.json を SSoT とする workspace-tree 系コマンドへ置換する。
07-dev-environment.md に新タスクを追記する。

## Phase 1: Extract detailed sections to rules

- [x] Create .claude/rules/08-orchestration.md — move §1 Primary Role, §2 Source Of Truth, §4 Delegation Rules from CLAUDE.md
- [x] Create .claude/rules/09-maintainer-checklist.md — move §6 Maintainer Checklist from CLAUDE.md
- [x] Move §8 Guardrails to .claude/rules/10-guardrails.md (hook constraint block + core guardrails)

## Phase 2: Replace Workspace Map with workspace-tree commands

- [x] Add extra_dirs field to architecture-rules.json for non-crate directories (project-docs, track, etc.)
- [x] Add workspace-tree (crate only) and workspace-tree-full (crate + extra_dirs) subcommands to architecture_rules.py
- [x] Add cargo make workspace-tree and workspace-tree-full tasks to Makefile.toml + document in 07-dev-environment.md
- [x] Remove verify_claude_workspace_map and all consumers: architecture_rules.py, verify_architecture_docs.py, Makefile.toml task, test_architecture_rules.py, test_verify_scripts.py, test_make_wrappers.py, verify_orchestra_guardrails.py whitelist, .claude/settings.json allowlist; remove CLAUDE.md Workspace Map section

## Phase 3: Compress CLAUDE.md

- [x] Compress CLAUDE.md to ≤50 lines: minimal pointers to rules, SSoT list, /track:* note (no workspace map)

## Phase 4: CI validation + commit

- [x] Run cargo make ci to verify no regressions; commit all changes together
