# Observations — d15-task-status-check-gate-2026-07-11

機械検証不能な運用観測の記録（free-form）。

## 義務ゲート dogfood（本 track 自身が新ゲートの初適用）

- **収束軌跡**: evaluate 9 ラウンド / repair 8 ラウンドで 36 edges 全 pass（fail 数の推移: 24 → 15 → 7 → 2 → 4 → 2 → 2 → 0。calibration probe は全ラウンド 100%）。retro-fit モード（実装後に bindings 著作）の初回拒否率は 24/36 ≈ 67% で、ADR の「高い first-pass rejection rate」予測どおり。
- **binding 形の往復**: (1) 全 edge に同一テスト群を束ねる → anchor 無関係テストが Contradiction を誘発。(2) edge ごとに単一テストへ絞る → CentralUnverified の穴。(3) 収束形は「anchor の約束の構成要素を列挙し、要素ごとに 1 focused test の union」。導出義務は fulfillment レコード（obligation_id ベース、set は義務の全 edge で共有）、非導出 edge は voluntary_binding が正準形（other スコープレビューの P1 指摘で是正）。
- **set 共有の flip コスト**: fulfillment レコードの tests 集合を 1 回編集するごとに、その義務の全 edge（8 本）が再判定され、既 pass の edge が別の理由で flip する事象が 2 回発生（handoff の予測どおり）。集合編集は「まとめて 1 回」が安い。
- **判定者の固着**: エントリ自身をテスト本体で直接行使する evidence（inline `new(...)` 構築）を要求する傾向を再確認。helper 内構築は scanner に不可視。

## レビュー運用

- **nested codex reviewer のタイムアウト**: review-fix-lead（codex）内から `cargo make track-local-review` を呼ぶ nested 構成で、レビュー所要が長い scope（types / usecase / 大 diff）は verdict 未記録のまま落ちる事象が頻発（3 連続）。orchestrator が wrapper を直接実行すると全て成功。恒久対処はフォローアップ候補（fixer 内 exec タイムアウトの調整 or レビューだけ orchestrator 駆動に分離）。
- **types スコープの設計往復**: fast reviewer が「12 引数コンストラクタ重複 → 共有依存バンドル化」を要求 → role 語彙に受動 DI キャリアの合法 role が無く（R1/R5）撤去 → 「責務別の明示的コンストラクタ + 差分（results は rules loader 非依存）」で確定。role 語彙拡張は ADR レベルの将来課題。

## ツール面

- `bin/sotp track contract-map <id>`（positional）は現行 CLI で拒否される（branch-bound 引数なし形式 or `--track-id`）。`.harness/capabilities/type-designer.md` のコマンド例が旧形式のまま — ドキュメント修正のフォローアップ候補。
- 旧 track の spec.json を修正した場合、その track の spec.md（rendered view）は WRITE guard（branch 束縛）により再生成不能。既知の制約として受容（本 track では前 track の IN-08/AC-04 追記分が該当）。
