# Verification: CI Verification Hardening

## Scope Verified

- WF-23: ci-container / ci-rust-container タスク追加、CI ワークフロー移行
- WF-09: placeholder_lines() のフェンスドコードブロックスキップ
- WF-11: verify_tech_stack_ready.py の計画フェーズバイパスロジック
- WF-13: VERIFICATION_SCAFFOLD_LINES の日本語対応

## Manual Verification Steps

- ci-container タスクがコンテナ内から --allow-private なしで実行可能か確認
- フェンスドコードブロック内に TODO を含む spec.md/plan.md で verify-latest-track が pass するか確認
- 全トラック planned 時に tech-stack.md に TODO があっても CI pass するか確認
- 日本語見出しの verification.md で scaffold 検出が機能するか確認
- cargo make ci が全体として pass するか確認

## Result / Open Issues

- WF-23: ci-container / ci-rust-container タスクを Makefile.toml に追加。ci.yml を ci-container に移行。test_make_wrappers.py にテスト追加。
- WF-09: placeholder_lines() にフェンスドコードブロックスキップを実装。テスト 2 件追加（コードブロック内 pass / コードブロック外 fail）。
- WF-11: all_tracks_planned() を verify_tech_stack_ready.py に追加。fail-closed ポリシー維持。テスト 5 件追加（all-planned pass, in_progress block, done block, missing metadata fail, corrupt metadata fail）。
- WF-13: VERIFICATION_SCAFFOLD_LINES に日本語本文行等価表現を追加（検証範囲, 手動検証手順, 結果 / 未解決事項, 検証日）。テスト 1 件追加。
- WF-13 注記: scaffold 検出は本文行（リストアイテム）のみ対象。見出し行（`##` プレフィックス）はスキップ対象のまま（意図的設計）。
- 全 95 テスト pass、cargo make ci pass。

## Verified At

2026-03-12
