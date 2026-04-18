# Verification — TDDD 信号機評価の CI ゲート接続と宣言/評価結果ファイル分離

## Scope Verified

本 track の実装完了時点で、以下のスコープが全て満たされていることを確認する。

- ADR `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md` の決定 D1–D7 に対応する実装が全て完了し、ADR が Proposed から Accepted に昇格している。
- spec.md の Acceptance Criteria 全項目が実装で満たされている。
- spec.md の「Behavior Truth Table」の全セル (3 経路 × 8 状態) が実装で満たされており、fail-open が発生していない。

## Manual Verification Steps

実装完了後、以下を手動で検証する。

> **注**: 本 track (`tddd-ci-gate-and-signals-separation-2026-04-18`) は TDDD ツール実装 track であり、per-track の TDDD 宣言ファイル (`<layer>-types.json`) を持たない。V1/V2/V3/V4/V5/V6 の手順は、TDDD 宣言ファイルを持つ別 track (検証用の新規 track または既存 track) を対象として実行すること。V7 (CI 通過) は本 track の CI 環境で検証可能。

### V1: 宣言/評価結果ファイルの分離

- [ ] `sotp track type-signals <track-id>` 実行後、`<layer>-types.json` に `signals` フィールドが存在しないこと (`jq 'has("signals")' <layer>-types.json` が `false`)。
- [ ] 同実行後、`<layer>-type-signals.json` が生成され `schema_version: 1`, `generated_at`, `declaration_hash`, `signals` を含むこと。
- [ ] `declaration_hash` が `sha256sum <layer>-types.json` の出力と一致すること。

### V2: pre-commit 自動再計算と Red ブロック

- [ ] 意図的に宣言を改変して Red 信号を作り、`/track:commit` を実行すると `[BLOCKED]` で停止し、`tmp/track-commit/commit-message.txt` が保持されること。
- [ ] Red 解消後に `/track:commit` を再実行すると commit が通過すること。
- [ ] Yellow 信号のみの状態で `/track:commit` を実行すると stderr に `[WARN]` を出しつつ commit が通過すること。

### V3: review hash 除外

- [ ] 評価結果ファイルのみを変更した commit を試行し、`SystemReviewHasher` が計算する `code_hash` が変動しないこと (`cargo make track-review-status` で approved のまま維持される)。

### V4: stale 検出 (CI / merge gate 両経路)

CI 経路 (ワーキングツリー変更で検証可能):
- [ ] 評価結果ファイルの `declaration_hash` を不正な値に書き換え、Docker 内 CI (`cargo make ci`) で `VerifyFinding::error` が emit されること。
- [ ] 評価結果ファイルを削除した状態で CI が `VerifyFinding::error` を返すこと (Missing)。

merge gate 経路 (`check_strict_merge_gate` は `git show origin/<branch>:<path>` でコミット済みコンテンツを読むため、push 済みの commit が必要):
注: `/track:commit` は pre-commit 再計算が自動で走るため、stale / Missing な状態では commit が完了しない。merge gate 検証には `git commit` を直接使用してバイパス commit を作成するか、意図的に stale なファイルを含む fixture ブランチを使用すること。
- [ ] stale な評価結果ファイルを `git commit` で直接コミットして検証ブランチを push し、merge gate (`check_strict_merge_gate`, strict=true) が `VerifyFinding::error` を返すこと。
- [ ] 評価結果ファイルを削除して `git commit` で直接コミットし push した状態で merge gate が `VerifyFinding::error` を返すこと (Missing)。

### V5: symlink 拒否

- [ ] 評価結果ファイルを symlink に置き換え、CI が `reject_symlinks_below` 由来の error を返すこと。
- [ ] 宣言ファイルが symlink の場合にも CI が error を返すこと (既存挙動が新コードで損なわれないこと)。
- [ ] 評価結果ファイルを symlink に置き換えた状態で `/track:commit` を実行すると、`sotp track type-signals` の書き込みステップが `reject_symlinks_below` 由来の error でブロックされること (pre-commit が symlink への書き込みを行わないこと)。

### V6: truth table 全セル検証

- [ ] spec.md「Behavior Truth Table」の 3 経路 × 8 状態 = 24 セルのうち N/A を除く全セルで期待動作を確認する (手動テストスクリプトか統合テストで網羅)。

### V7: CI 通過

- [ ] `cargo make ci` が本 track ブランチで通過すること。
- [ ] `cargo make deny` が通過すること。
- [ ] `cargo make check-layers` が通過すること (domain ← usecase ← infrastructure ← cli の依存方向維持)。

## Result

(実装完了後に記入する)

## Open Issues

(実装中に発見された未解決項目を記入する)

## Verified At

- Verified at: (未検証)
- Verified by: (未検証)
