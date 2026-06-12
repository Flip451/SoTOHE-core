# Observations — ref-verify-existence-scope-2026-06-10

手動観測ログ（機械検証不能な workflow 観測のみ）。

## 2026-06-10/11: Phase 0 ブロッカーの実地確認（鶏と卵）

- 本トラック自身の初回コミットが、修正対象のバグ（Phase 0 で `ref-verify run --context commit-gate` が spec.json 不在により決定論的 fail）に block されることを実地確認した。ユーザー判断で ADR 初回コミットを Phase 1-3 完了後まで繰り延べ、spec.json + 全 catalogue が揃った時点の一括コミット（3c758ca2）で commit gate が通ることを確認。メモ（tmp/adr/memo/2026-06-10-implementation-order.md）の Phase 1 側の指摘（catalogue 全不在のハードエラー）は先行トラック T013 で修正済みで、Phase 0 側のみが実在ブロッカーだった。

## 2026-06-11: バイナリ/Makefile 移行順序の実地確認（T002）

- 互換マトリクスを実運用で確認: T001 コミット（旧 Makefile + 旧バイナリ）→ T002 コミット（新 Makefile `ref-verify run` + 旧バイナリ、旧既定 context standalone が同一 All スコープに解決され gate 通過、コミット bd8342cd）→ `cargo make build-sotp` 再ビルド → 新バイナリで `bin/sotp ref-verify run` が [OK]、`--context commit-gate` は clap エラーで拒否（AC-03）。移行中にコミットゲートが壊れる瞬間は存在しなかった。

## 2026-06-11: wall-time 観測

- 全ペアキャッシュ済み状態での `bin/sotp ref-verify run`（新バイナリ・All スコープ・差分キャッシュ全ヒット）は数秒で完了（probe スキップ含む）。コミットゲート全体（ci + review + ref-verify + DRY + commit）は warm cache で約 40-100 秒。
