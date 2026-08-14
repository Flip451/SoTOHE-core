# Observations

## 2026-08-13: spec-design pre-entry の check-zero-findings 一時除去

Phase 0 の guarded commit 直後に `bin/sotp phase enter spec-design` を実行したところ、
pre-entry の `bin/sotp review check-zero-findings --scope adr --round final` が
「no final review verdict exists for this scope」で fail した。adr scope は commit 直後で
空（NotRequired(Empty)）であり、primary ADR の Context が記述する矛盾そのものが本 track の
phase entry で再現した形。

ADR Context に記録された operator 裁定（2026-08-13、当該検査の phase 宣言 config からの
一時除去）に従い、`.harness/config/phase-commands.json` の spec-design pre-entry から
`check-zero-findings` エントリのみを一時除去した。D1 実装後、D2 に基づき本 track 内で
このエントリを復旧する（除去状態は恒久化しない）。

type-design / impl-plan の同種 pre-entry check は、各 entry 時点で対象 scope（spec /
types）が dirty かつ直前の単一 scope review により final verdict を持つため除去しない。

## 2026-08-13: 一時除去の復元（Phase 3 完了直後）

`apps/cli/src/commands/phase.rs` の
`test_shipped_phase_commands_declare_direct_upstream_convergence_matrix` が `include_str!`
で shipped config の完全な pre-entry 行列を assert しているため、除去状態では
`cargo make ci` が通らず、除去は commit を跨げない working-tree 過渡状態としてしか成立
しないことが判明した（impl-plan scope の review-fix が blocked_cross_scope でこの依存を
報告）。Phase 1-3 の entry は完了済みで除去はもう不要のため、plan-artifacts commit の前に
エントリを復元した。除去は committed history には一度も現れない。D2 / IN-02 / AC-04 の
「復旧」はこの復元により満たされ、T2 の実行点では entry の存在確認（AC-04 検証）が残る。
