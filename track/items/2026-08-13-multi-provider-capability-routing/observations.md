# Observations — 2026-08-13-multi-provider-capability-routing

## 2026-08-13: phase-enter gate と正規 workflow 順序の構造ギャップ（operator 判断で config 修正）

Phase 1 入口で `bin/sotp phase enter spec-design` の pre-entry check
`review check-zero-findings --scope adr --round final` が構造的に通過不能だった。

- 正規順序（adr2pr / plan workflow）は ADR-baseline commit → Phase 1 entry。
- guarded commit が `.commit_hash` を進めるため、commit 直後は adr scope が empty になる。
- `check-zero-findings` の実装（`libs/usecase/src/review_v2/check_zero_findings.rs`）は
  `NotRequired(Empty)` を `MissingFinalVerdict`（fail）に写す。
- 空 scope に final verdict を新規記録する経路は存在しない（review-fix-lead は空境界を
  fail-closed で拒否、`review local` は auto-record を skip）。

Phase 0 の収束実体は存在する: commit `7a78be80` 前に adr scope final `zero_findings` に
2 回到達（review.json 記録済み）、`adr_user` signal は blue=673 / yellow=0 / red=0。

**処置**: user 承認（2026-08-13, 本セッション）の下、operator-owned な
`.harness/config/phase-commands.json` から `spec-design` の adr check-zero-findings
エントリを除去して続行した。adr 収束は第 1 pre-entry check（`signal check-adr-user
--gate commit`）と Phase 0 gate 群で引き続き担保される。

**未解決の恒久課題**（後続 track 候補）: empty-scope 合格レーンの定義
（例: 空 scope かつ最新記録 round が final zero_findings なら pass、または phase gate
用 diff base の再定義）。type-design / impl-plan の同種 check は spec / types scope が
phase 直前に非空（uncommitted 成果物あり）となる想定のため影響しない見込みだが、
再発時は同じ判断枠組みで扱う。

**2026-08-13 追記（解消）**: 恒久修正が別 track
（`2026-08-13-check-zero-findings-empty-scope-pass`、専用 ADR で決定記録済み、PR #241
として base branch に merge 済み）で landed。guarded base merge で取り込み、
`.harness/config/phase-commands.json` の spec-design adr check-zero-findings pre-entry と
`apps/cli/src/commands/phase.rs` の two-command expectation を base branch の canonical
文面へ復元（byte 一致確認済み）。本暫定措置はこれで終結。
