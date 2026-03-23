# Verification: INF-20 conch-parser infra move

## Scope Verified

- [ ] domain から conch-parser 依存が除去されていること
- [ ] infrastructure に conch-parser 依存が追加されていること
- [ ] ShellParser trait が domain に定義されていること
- [ ] ConchShellParser が infrastructure に実装されていること
- [ ] policy が &[SimpleCommand] を受け取る形に変更されていること
- [ ] usecase handlers が DI 化されていること
- [ ] CLI composition root で parser が注入されていること

## Manual Verification Steps

1. `cargo make ci` が pass すること（ci-rust + verify-arch-docs + verify-tech-stack + domain-purity 含む）
2. `cargo make check-layers` が pass すること
3. `cargo make deny` が pass すること
4. `grep -r "conch_parser\|conch-parser" libs/domain/src/` が空であること
5. `grep -r "conch_parser\|conch-parser" libs/domain/Cargo.toml` が空であること
6. `grep "conch-parser" libs/infrastructure/Cargo.toml` が存在すること
7. 既存テスト（guard policy + hook tests）がすべて pass すること
8. `docs/architecture-rules.json` の canonical_modules owner が更新されていること
9. `project-docs/conventions/shell-parsing.md` の API パスが更新されていること
10. `track/tech-stack.md` の conch-parser 記載が infrastructure 層に更新されていること

11. `.claude/docs/DESIGN.md` が更新されていること: Module Structure テーブル、Key Design Decisions テーブル、Shell Command Guard Canonical Blocks セクション

## Result / Open Issues

- 未検証

## Verified At

- 未検証
