<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# ドキュメント断捨離 — 重複 narrative の即時削除 (knowledge/WORKFLOW.md / knowledge/architecture.md)

## Summary

本 track は ADR D2 が確定した削除対象のうち本 track スコープ分 (`knowledge/WORKFLOW.md` / `knowledge/architecture.md`) と scratch cleanup (`repomix-output.*`) を実施し、削除後の derived link 修正 (`CLAUDE.md`) を単一 commit にまとめる。ADR D2 の 3 件目 (`TRACK_TRACEABILITY.md`) は ADR D6 の判断で別 track (`track-traceability-merge-...`) に分離されており本 track では実施しない (spec OS-01 / CN-03 参照)。
Rust コードへの変更はゼロ。実装は T001 (削除 + link 修正 + ADR コミット) と T002 (CI gate 確認) の 2 タスク構成。
CN-04 の「ADR を最初の commit と同一 commit に含める」制約を満たすため、全削除操作と ADR staging を T001 に集約している。
注意: Tier 0 SoT ファイル (`.claude/rules/09-maintainer-checklist.md` / `.claude/rules/10-guardrails.md` / `.claude/commands/track/setup.md`) にも `knowledge/WORKFLOW.md` への参照が残るが、CN-02 により本 track での変更は禁止されている。AC-04 は Tier 1/2 ファイルのみを対象に絞った文言に narrowing されており、Tier 0 SoT ファイルの残存参照は対象外。

## Tasks (2/2 resolved)

### S001 — 削除 + リンク更新 + ADR コミット (T001)

> IN-01: `knowledge/WORKFLOW.md` をリポジトリから削除する。このファイルは `DEVELOPER_AI_WORKFLOW.md` と全章で重複しており、ADR D2 が削除を確定している。
> IN-02: `knowledge/architecture.md` をリポジトリから削除する。自称「`knowledge/DESIGN.md` の slim 版」だが両方とも古く、slim 版を残す価値がない。
> IN-03: worktree 上の `repomix-output.*` ファイル群を削除する。`.gitignore` 済みのため commit 対象ではないが、worktree を clean にする副次対応。
> IN-04: `CLAUDE.md` の priority references から `knowledge/WORKFLOW.md` 行を削除する。Tier 1 entry-point index の broken link を解消する。
> CN-04: `knowledge/adr/2026-04-27-0554-doc-reorganization.md` を同一 commit に含める。pre-track-adr-authoring convention の終端処理ルールに従う。
> CN-02 例外 2: `knowledge/adr/README.md` の「ドキュメント運用」セクションに新規 ADR (`2026-04-27-0554-doc-reorganization.md`) のエントリを追加する。新規 ADR commit の必須終端処理であり、T001 の commit に含める。
> Tier 0 SoT ファイル (`.claude/rules/` / `.claude/commands/`) への broken link は CN-02 により本 track では修正しない。これらのファイルの link 修正は上流の adr-editor または別 track のスコープとなる。

- [x] **T001**: ADR コミット準備 + Tier 1 リンク更新 + 削除対象 narrative ファイル削除 + scratch ファイル削除 + ADR index 更新をひとつの commit にまとめる。具体的には: (1) `CLAUDE.md` priority references から `knowledge/WORKFLOW.md` 行を削除する (IN-04 / AC-04)。(2) `knowledge/WORKFLOW.md` をリポジトリから削除する (IN-01 / AC-01)。(3) `knowledge/architecture.md` をリポジトリから削除する (IN-02 / AC-02)。(4) worktree 上の `repomix-output.*` ファイル群 (repomix-output.txt / repomix-output.xml / repomix-output.1.txt / repomix-output.1.xml / repomix-output.2.txt / repomix-output.2.xml) をローカル削除する (IN-03 / AC-03)。(5) `knowledge/adr/2026-04-27-0554-doc-reorganization.md` を staging に含める (CN-04 / AC-06)。(6) `knowledge/adr/README.md` の「ドキュメント運用」セクションに新規 ADR のエントリを追加する (CN-02 例外 2)。すべてをひとつの commit にまとめることで CN-04 を満たす。 (`4429a833b1b905373636e77696915421f36f8ad2`)

### S002 — CI gate 確認 (T002)

> T001 の commit 後に `cargo make ci` を実行し全 gate が pass することを確認する。
> 本 track では Rust ソースコードを一切変更しないため、fmt-check / clippy / test / deny / check-layers は変更前後で差異がない想定。
> verify-* (verify-latest-track / verify-view-freshness / verify-catalogue-spec-refs 等) がドキュメント削除により誤検出しないことを確認する。

- [x] **T002**: `cargo make ci` を実行して全 gate が pass することを確認する (AC-05)。Rust コードへの変更はないため fmt-check / clippy / test / deny / check-layers はそのまま通過する想定。削除による verify-* への影響がないことを確認し、T001 の commit が clean であることを最終確認する。 (`4429a833b1b905373636e77696915421f36f8ad2`)
