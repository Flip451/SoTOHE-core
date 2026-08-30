<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 24, yellow: 0, red: 0 }
---

# ゲートの標準出力をサマリ契約にする

## Goal

- [GO-01] 対象となるゲート・検証タスクの標準出力を、判定・フルログの保存先・失敗時だけの診断抜粋からなる簡潔なサマリ契約に統一する。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1]
- [GO-02] 出力表示を簡潔化しても、既存の exit code と状態検査コマンドによる機械判定を維持する。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D2]

## Scope

### In Scope
- [IN-01] Makefile.toml と bin/sotp の既存定義でテスト実行・義務評価・コミット前の集約ゲートとして扱われるタスクに、共通の stdout サマリ契約を適用する。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T002, T003]
- [IN-02] ログ保存先の予約・準備に成功して子プロセスを実行できた各対象タスクは、フルログの保存が完了した場合にフル実行ログを tmp/gate/ 配下に保存し、stdout でそのログファイルへのパスを示す。子プロセス起動前に予約・準備が失敗した場合は、フルログパスを示さず、その失敗を短い理由として示す。子プロセス起動後にフルログの保存が失敗した場合は、フルログパスを示さず、フルログ未作成または参照不能をその短い理由として示す。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003, T004, T005, T006, T007]
- [IN-03] 子プロセスが起動し、フルログの保存が完了した対象タスクの失敗時には、失敗した項目と短い理由だけを stdout の診断抜粋として表示する。起動前の予約・準備失敗時は、ログ未作成を示す短い理由を表示する。起動後にフルログの保存が失敗した場合は、子プロセスが返した状態に対応する判定と、フルログ未作成または参照不能を示す短い理由を表示し、完全でないログのパスを表示しない。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003, T004, T005, T006, T007]
- [IN-04] 永続化ポートが一意なログ保存先を予約または準備し、その予約済み保存先を用いて子プロセスの実行ログを書き込み、永続化で予約を消費する流れを対象に含める。 [adr: knowledge/adr/2026-08-29-1030-gate-log-name-feasibility.md#D1] [tasks: T004, T005, T006, T007]
- [IN-05] 予約後のログ永続化で、trusted root 内の保存先を最終公開時に再検証し、親ディレクトリの移動または置換による root 外への公開を失敗として扱う流れを対象に含める。 [adr: knowledge/adr/2026-08-29-1030-gate-log-name-feasibility.md#D2] [tasks: T006]

### Out of Scope
- [OUT-01] 対象タスクの所属を新たに定義または変更することは対象外とし、その所属は既存の Makefile.toml と bin/sotp の定義に委ねる。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T002, T003]
- [OUT-02] stdout の文面をパースして合否または状態を判定する新しい機械可読経路を導入することは対象外とする。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D2] [tasks: T001, T002, T003]

## Constraints
- [CN-01] ログ保存先の予約・準備に成功した対象タスクの stdout は、フルログの保存が完了した場合には PASS または FAIL の判定、フルログファイルのパス、ならびに失敗時だけの失敗項目と短い理由に限らなければならない。子プロセス起動前に予約・準備が失敗した場合は、FAIL の判定とログ未作成を示す短い理由に限り、存在しないフルログパスを出力してはならない。子プロセス起動後にフルログの保存が失敗した場合は、子プロセスが返した状態に対応する PASS または FAIL の判定とフルログ未作成または参照不能を示す短い理由に限り、完全でないログのパスをフルログパスとして出力してはならない。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003, T004, T005, T006, T007]
- [CN-02] 対象タスクの成功時には、個別 PASS 行および内部レコードの Debug 表現を stdout に出力してはならない。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003]
- [CN-03] 機械による合否は既存の exit code で、状態照会は既存の check 系コマンドで引き続き判定しなければならない。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D2] [tasks: T001, T002, T003]
- [CN-04] コマンド構築は、エンコード後のログ名に対するファイルシステムのコンポーネント長予算を所有または判定してはならない。 [adr: knowledge/adr/2026-08-29-1030-gate-log-name-feasibility.md#D1] [tasks: T004]
- [CN-05] ログ保存先の選択、衝突回避、名前の実現可能性、および子プロセス起動前に必要な予約は、永続化ポートとそのアダプターが所有しなければならない。 [adr: knowledge/adr/2026-08-29-1030-gate-log-name-feasibility.md#D1] [tasks: T004, T005, T006]
- [CN-06] 予約時の確認だけで保存先の包含を保証してはならない。永続化は trusted root 内にあることを最終段階で再検証した保存先へ publish し、予約から永続化まで親ディレクトリが移動しないという実行環境上の仮定に依存してはならない。 [adr: knowledge/adr/2026-08-29-1030-gate-log-name-feasibility.md#D2] [tasks: T006]
- [CN-07] 永続化は予約 token を消費しなければならない。予約の cancel API、数値による pending-reservation 上限、adapter shutdown 時の未消費予約の暗黙的 reclaim を導入してはならず、アダプターは未消費 token に対応する予約済み名を TOCTOU に unlink してはならない。 [adr: knowledge/adr/2026-08-29-1030-gate-log-name-feasibility.md#D3] [tasks: T005]

## Acceptance Criteria
- [ ] [AC-01] フルログの保存が完了した対象タスクが成功すると、stdout は PASS の判定と tmp/gate/ 配下のフルログファイルのパスを示し、個別 PASS 行や内部レコードの Debug 表現を含まない。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003]
- [ ] [AC-02] ログ保存先の予約・準備に成功して子プロセスが起動し、フルログの保存が完了した対象タスクが失敗すると、stdout は FAIL の判定、tmp/gate/ 配下のフルログファイルのパス、失敗した項目および各項目の短い理由を示す。子プロセス起動前に予約・準備が失敗する場合は、stdout は FAIL の判定とフルログ未作成を示す短い理由を示し、存在しないパスを示さない。子プロセス起動後にフルログの保存が失敗する場合は、stdout は子プロセスが返した状態に対応する PASS または FAIL の判定と、フルログ未作成または参照不能を示す短い理由を示し、完全でないログのパスを示さない。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003, T004, T005, T006, T007]
- [ ] [AC-03] 子プロセスが起動し、フルログの保存が完了した対象タスクの詳細な診断情報は stdout ではなく tmp/gate/ 配下のフルログとして参照できる。起動前の予約・準備失敗ではフルログは作成されず、stdout の短い理由でその状態を確認できる。起動後にフルログの保存が失敗する場合は、完全なフルログを参照できないことを stdout の短い理由で確認でき、完全でないログのパスは示されない。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D1] [tasks: T001, T002, T003, T004, T005, T006, T007]
- [ ] [AC-04] 出力契約の変更後も、対象タスクの成功・失敗は従来どおり exit code で判断でき、既存の check 系コマンドで状態を照会できる。 [adr: knowledge/adr/2026-08-25-0425-gate-output-summary-contract.md#D2] [tasks: T001, T002, T003]
- [ ] [AC-05] ログ名を保存先で実現できない場合、永続化ポートは子プロセスを起動する前に予約・準備の不成立を報告でき、対象タスクは FAIL とログ未作成を示す短い理由を stdout に出力して子プロセスを起動しない。 [adr: knowledge/adr/2026-08-29-1030-gate-log-name-feasibility.md#D1] [tasks: T004]
- [ ] [AC-06] 子プロセスの起動後にフルログの保存失敗が報告されても、stdout の PASS または FAIL の判定と exit code は子プロセスが実際に返した終了状態に対応し、永続化の問題は別の短い理由として stdout で確認できる。フルログの保存が完了していない場合は、完全でないログのパスをフルログパスとして示さない。 [adr: knowledge/adr/2026-08-29-1030-gate-log-name-feasibility.md#D1] [tasks: T006, T007]
- [ ] [AC-07] 予約後に親ディレクトリが移動または置換され、trusted root 内でログの最終公開を完了できなくなった場合、永続化は Unavailable または書込みエラーとして失敗し、trusted root 外で書き込まれた inode を指す語彙上のパスをフルログパスとして返さない。 [adr: knowledge/adr/2026-08-29-1030-gate-log-name-feasibility.md#D2] [tasks: T006]
- [ ] [AC-08] 各 GateRunInteractor::execute は同時に一つ以下の live reservation だけを保持し、ログを永続化するとその予約を消費する。 [adr: knowledge/adr/2026-08-29-1030-gate-log-name-feasibility.md#D3] [tasks: T005]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 24  🟡 0  🔴 0

