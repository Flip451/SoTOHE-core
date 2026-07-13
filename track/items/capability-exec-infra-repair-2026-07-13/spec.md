<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 27, yellow: 0, red: 0 }
---

# 外部 provider 実行基盤の修復

## Goal

- [GO-01] capability wrapper が provider shell tool の完了時の exit code と stdout を同じ呼び出しで受け取る blocking 実行契約を維持し、長時間の reviewer / fixer 実行を結果なしの実行中 session として誤読しないようにする。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D1]
- [GO-02] `sotp capability exec` の provider subprocess 待機時間を dispatch ごとに CLI から表現可能にし、未指定の実行を時間上限なく完了まで待機させる。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D2]
- [GO-03] package 名の catalogue 識別と rustdoc crate root の異なる命名域を cargo metadata から動的に翻訳し、crate / bin の rename や bin-only package に対しても schema export、catalog import、signal 評価が一貫して動作するようにする。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3]

## Scope

### In Scope
- [IN-01] provider project 設定で PTY-backed exec を無効にし、capability wrapper が blocking shell-tool を前提として完了結果を扱う。正当に長時間となる reviewer subprocess には呼び出し側 timeout を明示し、運用文書でその timeout 下限を示す。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D1] [tasks: T001]
- [IN-02] `sotp capability exec` に任意の `--timeout-seconds <N>` を追加し、正の値が指定されたときだけ provider process の待機上限として適用する。flag 未指定時は既存の固定 600 秒上限を使わず、provider process の完了まで待機する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D2] [tasks: T001]
- [IN-03] timeout 値を CLI 境界で正の整数として検証し、検証済みの値だけを usecase を通じて subprocess 待機処理へ型付きで伝搬する。0 は subprocess を起動せず入力として拒否する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D2] [tasks: T001]
- [IN-04] package の rustdoc target を cargo metadata で解決する単一機構を導入する。lib target があればそれを優先し、lib が無ければ唯一の bin target を選び、複数 bin の場合は `default_run` が一致する場合だけ選ぶ。選択できない metadata は列挙順に依存せず fail-closed とする。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]
- [IN-05] 選択した metadata target の name を rustdoc 実行 target とし、その `-` を `_` に正規化した値を rustdoc crate root とする。schema export、catalog import の型照合、signal 評価器の identity key 正規化はこの同じ翻訳機構を使用し、catalogue 利用者向けの型 path は package 名 root のまま維持する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]
- [IN-06] crate 名又は bin 名を対象にした literal 対応表を廃止し、rustdoc root 翻訳のための新しい設定 field を導入しない。翻訳事実の唯一の情報源は cargo metadata とする。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]
- [IN-07] `Phase1Error` を catalogue で modify 宣言する際は、既存の `ActionContradiction`、`UnresolvedTypeRef`、`DanglingId` variant の payload を生の `String` のまま保持せず、検証済み newtype `DiagnosticMessage` へ移行する。これにより `RustdocRootResolution` を追加した post-state 全体を `ErrorType` の variant field に対する primitive obsession guard に適合させる。 [adr: knowledge/adr/2026-07-01-0004-catalogue-primitive-obsession-guard.md#D7] [tasks: T002]

### Out of Scope
- [OUT-01] `sotp review local` の既存の 1800 秒 default と `--timeout-seconds` による可変指定は変更しない。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D2] [tasks: T001]
- [OUT-02] capability profile に timeout の既定値を持たせる仕組みを追加しない。timeout の意図は capability 単位ではなく dispatch ごとの CLI flag で表現する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D2] [tasks: T001]
- [OUT-03] package と rustdoc root の対応を設定 file 又は crate / bin 名の個別 literal map で管理しない。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]

## Constraints
- [CN-01] timeout の省略は無期限待機を意味し、実行中 process を wrapper が独自に kill しない。無期限待機の進行監視は呼び出し側 orchestrator の責務とする。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D2] [tasks: T001]
- [CN-02] timeout が指定された場合は正の整数だけを受け入れ、0 又は検証不能な入力を fail-closed で拒否する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D2] [tasks: T001]
- [CN-03] metadata に lib target が無い package で複数 bin target を選ぶには `default_run` の一致を必要とし、`default_run` の欠如、不一致、target 名欠如、又は一意な選択不能を成功扱いにしない。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]
- [CN-04] rustdoc root の導出では package 名からの推測、個別 surface の独自実装、又は静的 fallback を用いず、target metadata の name と `-`→`_` 正規化だけに従う。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]

## Acceptance Criteria
- [ ] [AC-01] capability wrapper の provider shell tool が、完了した command の exit code と stdout を同一呼び出しで返す blocking 挙動となることを確認する。timeout が不足する command は結果なしの実行中 session ではなく exit 124 と経過時間を伴う明示的な失敗として返り、reviewer 呼び出しの運用文書には正当な長時間実行に対する timeout 下限が記載されていることを確認する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D1] [tasks: T001]
- [ ] [AC-02] `sotp capability exec` を `--timeout-seconds` なしで実行すると provider process に一律の 600 秒上限を設定せず、完了まで待機することを検証する。正の timeout を指定した場合だけ、その値が provider process の待機上限として使われることも検証する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D2] [tasks: T001]
- [ ] [AC-03] `--timeout-seconds 0` を与えると command が subprocess を起動せず非成功で終了し、正の timeout は CLI 境界から検証済みの値として usecase と subprocess adapter に到達することを検証する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D2] [tasks: T001]
- [ ] [AC-04] cargo metadata に lib target がある package では、package 名と異なる lib 名及び `-` を含む lib 名であっても lib target を選び、その target 名を `-`→`_` 正規化した rustdoc root を得ることを検証する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]
- [ ] [AC-05] lib を持たない package では唯一の bin target を選び、package 名と異なる bin 名及び `-` を含む bin 名から正規化済み rustdoc root を得ることを検証する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]
- [ ] [AC-06] lib を持たず複数 bin target を持つ package では `default_run` が既存 bin 名に一致するときだけその bin を選ぶことを検証する。`default_run` が無い、不一致である、又は target 名を取得できない場合は metadata の順序に関係なく失敗することを検証する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]
- [ ] [AC-07] schema export の rustdoc 実行が解決済み target 名を使い、package 名と異なる lib / bin target 又は rename 後の target で正しい rustdoc root を解決することを検証する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]
- [ ] [AC-08] catalog import が package 名 root の catalogue 型 path を維持したまま、照合時だけ共有 resolver で rustdoc root に翻訳し、bin-only package の reference / modify / delete 型操作を成功させることを検証する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]
- [ ] [AC-09] signal 評価器の identity key 正規化が共有 resolver を使い、既存の crate 名 literal 対応表に依存せず、rename 後の package / target の組合せを metadata から解決することを検証する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]
- [ ] [AC-10] rustdoc root 翻訳のための config field、crate / bin 名の static map、又は surface 固有の fallback が存在せず、schema export、catalog import、signal 評価器が同じ cargo metadata 解決規則に従うことを検証する。 [adr: knowledge/adr/2026-07-13-0410-capability-exec-infra-repair.md#D3] [tasks: T002]

## Related Conventions (Required Reading)
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 27  🟡 0  🔴 0

