# Verification: Full model reviewer の --full-auto 自動付与

## Scope Verified

- [ ] `agent-profiles.json` に `model_profiles` セクションが追加されている（`gpt-5.4`, `gpt-5.3-codex`, `gpt-5.3-codex-spark` の3エントリ）
- [ ] `resolve_full_auto` が設定ファイルから full_auto フラグを正しく解決する
- [ ] 未知モデルで fail-closed（full_auto: true）になる
- [ ] `model_profiles` セクション欠如時に fail-closed（full_auto: true）になる
- [ ] `agent-profiles.json` 読み込み失敗時に fail-closed（full_auto: true）になる
- [ ] `build_codex_invocation` が full_auto=true 時に `--full-auto` を含む
- [ ] `build_codex_invocation` が full_auto=false 時に `--full-auto` を含まない
- [ ] fake-codex 統合テストで full model 時に引数に `--full-auto` が渡される
- [ ] fake-codex 統合テストで spark model 時に引数に `--full-auto` が含まれない
- [ ] ワークアラウンドスクリプトが削除されている
- [ ] `cargo make ci` グリーン

## Manual Verification Steps

1. `cargo make test` で全テスト通過を確認
2. `cargo make ci` で全ゲート通過を確認
3. 実際の Codex CLI で `cargo make track-local-review -- --model gpt-5.4 --prompt "..."` を実行し、verdict が返ることを確認（optional: Codex CLI が利用可能な場合のみ）

## Result

_未実施_

## Open Issues

_なし_

## Verified At

_未検証_
