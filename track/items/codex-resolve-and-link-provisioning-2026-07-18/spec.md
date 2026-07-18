<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 19, yellow: 0, red: 0 }
---

# codex reviewer runtime の bootstrap 解決リンク（resolve & link）配備

## Goal

- [GO-01] SoTOHE の bootstrap 済み consumer が、codex の npm + toolchain-manager を含む通常の導入形態でも、利用者による `CODEX_BIN` 設定・追加の環境変更・文書参照なしに、初回からサニタイズされた review / dry-fix の codex 実行を開始できるようにする。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D1, knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D2]
- [GO-02] codex の解決不能・リンク劣化・子プロセス失敗を、利用者が exit code、session log、bootstrap 再実行案内から診断して回復できるものにする。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D4]

## Scope

### In Scope
- [IN-01] `cargo make bootstrap` に、通常環境で codex 候補をサニタイズ模擬環境の `--version` で検証し、成功した公開エントリを repo-local の gitignored symlink として配備する処理を含める。処理は再実行可能で、既存リンクを最新の解決結果へ張り直す。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D1] [tasks: T001, T003, T004]
- [IN-02] PATH 上の `codex` 候補がサニタイズ模擬環境で動かない場合、npm の公開 interface である `npm prefix -g` とその `bin/codex` エントリだけを fallback 候補として解決・検証する。公開されていない package 内部の実行ファイルは探索・リンク対象にしない。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D1] [tasks: T001, T003]
- [IN-03] review fixer、dry-fix、およびそれらが起動する nested reviewer を含む全 codex spawn 経路で、project-root 相対の repo-local link を最初に使い、存在しない・実行不能・dangling の場合だけ OS PATH に fallback する共通の解決規約を適用する。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D2] [tasks: T002]
- [IN-04] runtime の `CODEX_BIN` 読み取り、`resolve_codex_via_asdf` と asdf 向け環境引き継ぎ、source / overlay Makefile の `CODEX_BIN` inline 解決を撤去する。`#[cfg(test)]` のテスト専用 `SOTP_CODEX_BIN` は維持する。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D3] [tasks: T002, T004]
- [IN-05] サニタイズ環境で起動した codex の失敗報告と session log に、復旧・実行経路の判断に必要な情報を記録する。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D4] [tasks: T002]

### Out of Scope
- [OUT-01] Linux 以外または symlink 制約のあるプラットフォーム向けのコピー配備など、symlink を代替する配備方式は初版の対象外とする。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D5]
- [OUT-02] checksum 付きダウンロード、実バイナリの複製、または codex CLI バージョンを固定する pin mode は初版の対象外とする。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D1, knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D5]
- [OUT-03] サニタイズ契約を緩和して実 HOME や利用者 credential を子プロセスへ渡す変更は対象外とする。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D2]

## Constraints
- [CN-01] repo-local 配備は実体の複製ではなく symlink とし、利用者が管理する toolchain-manager 領域、PATH、または npm package 内部の非公開レイアウトを書き換えたり解決根拠にしたりしない。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D1] [conv: knowledge/conventions/responsibility-boundary.md#Rules] [tasks: T001, T003]
- [CN-02] repo-local link を使う spawn では、child PATH の先頭に bootstrap が記録した公開エントリの親ディレクトリを置く。canonicalized package-internal executable の親ディレクトリを前置してはならない。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D2] [tasks: T002]
- [CN-03] リンクが absent または dangling で PATH fallback も codex を解決できない場合、spawn は壊れた link を黙って実行せず、`cargo make bootstrap` の再実行による再解決を案内する。 [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D2, knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D4] [tasks: T002]

## Acceptance Criteria
- [ ] [AC-01] bootstrap succeeds for a codex entry that only becomes runnable after the public npm global `bin/codex` entry and its colocated runtime directory are selected; it creates or refreshes the repo-local symlink after the sanitized-simulated `--version` probe succeeds. [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D1] [tasks: T001, T003, T004]
- [ ] [AC-02] bootstrap fails when neither the PATH candidate nor the public npm fallback passes the sanitized-simulated probe; its diagnostic identifies each attempted source (PATH candidate and public npm fallback), states the failing probe result for each attempted source, and directs the user to repair or install a codex entry before rerunning `cargo make bootstrap`. It does not create a link to an unverified candidate. [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D1] [tasks: T001, T003]
- [ ] [AC-03] Each codex spawn path, including the nested reviewer path, chooses a valid repo-local link before PATH; a dangling link is skipped and PATH is attempted instead of treating the link as executable. [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D2] [tasks: T002]
- [ ] [AC-04] When the repo-local link is selected, the spawned child's PATH begins with the recorded public-entry parent directory, so the selected launcher finds its colocated runtime without depending on a sanitized HOME-compatible shim. [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D2] [tasks: T002]
- [ ] [AC-05] Production codex resolution no longer reads runtime `CODEX_BIN`, invokes asdf resolution, or injects asdf-specific environment values; source and overlay Makefiles no longer contain `CODEX_BIN` inline resolution, while test-only `SOTP_CODEX_BIN` behavior remains available under test configuration. [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D3] [tasks: T002, T004]
- [ ] [AC-06] A failed sanitized codex child reports its exit code and session-log path. If no valid link and no PATH fallback are available, that report also directs the user to rerun `cargo make bootstrap`; the session log records the resolved real path and the result of `codex --version` for every spawn outcome. [adr: knowledge/adr/2026-07-18-1359-codex-resolve-and-link-provisioning.md#D4] [tasks: T002]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/no-upstream-restatement.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 19  🟡 0  🔴 0

