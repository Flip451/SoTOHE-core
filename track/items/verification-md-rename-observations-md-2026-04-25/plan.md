<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# verification.md を observations.md に改名 — 役割を手動観測ログに限定

## Tasks (2/7 resolved)

### S1 — S1 — Infrastructure code: verify-latest-track の縮退

> VERIFICATION_SCAFFOLD_LINES static set 削除 (T001)
> scaffold_placeholder_lines 関数削除 (T001)
> validate_verification_file 関数削除 (T001)
> other_files 配列から verification.md タプルを除去 → plan.md のみに縮退 (T001)
> 削除に伴う unit/integration test の更新・削除 (T002)
> libs/infrastructure/src/verify/latest_track.rs 単一ファイルへの集中変更

- [x] **T001**: libs/infrastructure/src/verify/latest_track.rs から VERIFICATION_SCAFFOLD_LINES static set、scaffold_placeholder_lines 関数、validate_verification_file 関数を削除し、verify() 内の other_files 配列から "verification.md" タプルを除去して plan.md のみに縮退させる。削除後の CI gate は spec.md / spec.json / plan.md のみを検証する。CN-01 (最新 track 選択ロジックは維持) を確認する (IN-01、infrastructure 層)
- [x] **T002**: libs/infrastructure/src/verify/latest_track.rs の unit/integration test を VERIFICATION_SCAFFOLD_LINES 削除後の縮退仕様に合わせて更新・削除する: verification.md を必要とする setup_complete_track / setup_complete_track_with_spec_json 等のヘルパーから verification.md 書き込みを除去し、test_scaffold_placeholder_detected / test_complete_v5_track_passes などの verification.md 依存テストを縮退仕様 (plan.md のみ) を正とするよう修正または削除する。cargo make ci が pass することを確認する (IN-06、infrastructure 層)

### S2 — S2 — Command docs: /track:implement / /track:full-cycle / /track:commit

> .claude/commands/track/implement.md の無条件 verification.md 更新手順を observations.md 条件付き手順に置換 (T003)
> .claude/commands/track/full-cycle.md の post-loop verification.md 更新を同様に置換 (T003)
> .claude/commands/track/commit.md から verification.md 必須 source 参照を削除し observations.md を optional source に変更 (T004)

- [ ] **T003**: .claude/commands/track/implement.md のステップ「After CI passes, update verification.md…」を削除し、「D4 (a)/(b) の条件に該当する場合のみ observations.md を作成/追記する」手順に置き換える。.claude/commands/track/full-cycle.md の「## Post-loop」セクションにある「update verification.md with overall results and verified_at」を同様に observations.md の条件付き手順に書き換える (IN-02、command docs)
- [ ] **T004**: .claude/commands/track/commit.md の Step 0 および Step 3 から verification.md への参照を削除し、observations.md を optional source (存在する場合のみ参照) として扱う記述に更新する。Step 3a の「Read spec.md, plan.md, and verification.md」を「Read spec.md and plan.md; if observations.md exists, also read it」相当に書き換える (IN-03、command docs)

### S3 — S3 — Workflow + reference docs の更新

> track/workflow.md の「## verification.md」セクションを「## observations.md」に置換、役割・作成条件・制約を記述 (T005)
> CLAUDE.md / START_HERE_HUMAN.md / .claude/rules/08-orchestration.md / .claude/rules/10-guardrails.md の verification.md 言及を削除または observations.md 文脈に書き換え (T006)

- [ ] **T005**: track/workflow.md の「## verification.md」セクションを廃止し、「## observations.md」セクションに置き換える。新セクションには observations.md の役割 (機械検証不能な手動観測ログ、自由形式、scaffold なし)、作成条件 (D4 (a) implementer 裁量 / D4 (b) spec AC 明示)、および CN-02 (ファイル不在 = 観測なし、CI は不在を error にしない)・CN-03 (フォーマット自由)・CN-04 (新規 track で verification.md を作成しない) の制約を記述する (IN-04、workflow docs)
- [ ] **T006**: CLAUDE.md / START_HERE_HUMAN.md / .claude/rules/08-orchestration.md / .claude/rules/10-guardrails.md から verification.md への言及を削除または observations.md の文脈に書き換える。CLAUDE.md の Priority references にある verification.md エントリを削除する。08-orchestration.md の Source Of Truth セクションにある verification.md 参照を削除する。10-guardrails.md にある verification.md 言及を確認し該当箇所を更新する (IN-05、docs)

### S4 — S4 — Off-topic bookkeeping

> merge.md IMPORTANT-note ホットフィックス (T007)
> render.rs テキスト修正 (T007)
> catalogue_spec_signal.rs rustdoc intra-doc link 修正 (T007)
> spec.json IN-* とは無関係のため task-coverage マッピングなし

- [ ] **T007**: オフトピック bookkeeping 3 件をまとめてコミットする: (1) .claude/commands/track/merge.md の IMPORTANT-note ホットフィックス (auto-selection of merge method 禁止)、(2) libs/infrastructure/src/track/render.rs:362 の `/track:implement` → `/track:impl-plan` テキスト修正、(3) libs/domain/src/tddd/catalogue_spec_signal.rs:324 の rustdoc intra-doc link 完全修飾パス修正。これらは spec.json の IN-* に属さないため task-coverage の対象外 (bookkeeping のみ)
