# Verification: spec-template-foundation-2026-03-18

## Scope Verified

- [x] T001: block-test-file-deletion hook が test ファイル削除をブロックすることを確認
- [x] T002: タスク説明変更時に save が拒否されることを確認
- [x] T003: spec.md テンプレートに source attribution が含まれることを確認
- [x] T004: frontmatter に signals optional field が追加されていることを確認

## Manual Verification Steps

1. `sotp hook dispatch block-test-file-deletion`: Write tool でテストファイルを空/None content で書き込み → block (テスト 7 本で検証済み)
2. `FsTrackStore::save()`: 既存タスクの description を変更 → `TaskDescriptionMutated` エラー返却 (テスト 4 本で検証済み)
3. `track-plan SKILL.md`: spec.md 初期化テンプレートに `[source: ...]` タグ追加済み。`project-docs/conventions/source-attribution.md` 作成済み (5 tag types)
4. `sotp verify spec-frontmatter`: signals フィールド付き/なし両方でパス。malformed signals は拒否 (テスト 6 本で検証済み)
5. `cargo make ci`: 971 テスト全パス + 全 verify ゲート通過

## Result / Open Issues

All 4 tasks implemented and verified. 971 tests pass.

- T001: `TestFileDeletionGuardHandler` が Write tool の空/None content + missing file_path を fail-closed で検出。settings.json に hook エントリ追加
- T002: `FsTrackStore::save()` の既存ファイル更新パスに `validate_descriptions_unchanged()` を接続
- T003: SKILL.md テンプレート更新 + `source-attribution.md` convention 追加 (5 tag types: document, feedback, convention, inference, discussion)
- T004: `OPTIONAL_FIELDS = ["signals"]` 追加。`is_valid_inline_mapping()` で balanced-brace 検証

## Accepted Deviations

1. **signals YAML validation** (infra-domain, escalation threshold 4 rounds): balanced-brace heuristic, not full YAML parser. Malformed content inside balanced braces is accepted. Full validation deferred to Phase 2 TSUMIKI-01. User-approved.
2. **Empty test file creation blocked** (usecase): Write with empty content to test paths is blocked even for new files. This is intentional — TDD workflow requires test files to contain test code from creation. User-approved.
3. **Task deletion not detected by immutability guard** (infra-domain): `validate_descriptions_unchanged()` checks description mutation, not task removal. Task removal is outside scope — task lifecycle is managed by `track-transition` API, not direct `save()`.

## verified_at

2026-03-18
