# Verification: Container Security Hardening

## Scope Verified

- [x] compose.yml .git read-only mount
- [x] compose.yml private/ and config/secrets/ tmpfs overlay
- [x] compose.dev.yml same changes
- [x] CI tasks work with read-only .git
- [x] security.md documentation updated

## Manual Verification Steps

1. [x] `cargo make ci` passes — **PASS** (218 tests, all verifiers)
2. [ ] `docker compose exec tools git add .` fails with EROFS — **requires manual verification** (tools-daemon not running)
3. [ ] `docker compose exec tools ls /workspace/private/` returns empty — **requires manual verification**
4. [ ] `docker compose exec tools ls /workspace/config/secrets/` returns empty — **requires manual verification**

## Result / Open Issues

- **PASS** — compose.yml, compose.dev.yml に `.git:ro` マウントと tmpfs オーバーレイを追加済み
- CI は全チェック通過（read-only .git の影響なし）
- `docker compose exec` による手動検証は tools-daemon 起動後に実施推奨
- トラック名に "git" を含む場合の guard hook 誤検知に対し、`track_schema.py` に `RESERVED_ID_WORDS` バリデーションを追加済み

## verified_at

- 2026-03-11
