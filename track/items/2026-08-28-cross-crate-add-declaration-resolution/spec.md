<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 28, yellow: 0, red: 0 }
---

# 参照先 crate の add 宣言を解決集合に加える

## Goal

- [GO-01] 同じ track の層をまたぐ宣言先行の型・trait 参照を、完全修飾 identity と入力・出力 snapshot の同一性を保った fail-closed な解決集合で実現する。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1, knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D2, knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D3, knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D8]

## Scope

### In Scope
- [IN-01] ある層の解決集合に、同じ track の他の TDDD 有効層の catalogue が add 宣言した型と trait を、宣言層 crate の外部項目として加えること。参照側 catalogue に重複する記述は要求しない。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T001, T002, T004, T005, T006, T007, T008, T009]
- [IN-02] 他層の add 宣言から合成する項目の identity と配置を、宣言層 catalogue の crate 名、既存の bin-target root 正準化、および宣言層自身の module 解決に従わせること。参照側 rustdoc paths に同一 identity がある場合は rustdoc 項目を優先する。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D2] [tasks: T001, T002, T003, T006, T007, T008, T009]
- [IN-03] 型シグナル再利用を、宣言、解決集合、Rust 実装、選択した Cargo 実行条件、期待 rustdoc 出力の完全な同一性に束縛し、入力確定又は fingerprint 作成に失敗した場合は fail-closed にすること。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D3, knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D4] [tasks: T010, T011, T013, T018]
- [IN-04] 一回の context 組立てで実行する rustdoc export を最大 64 層に制限し、65 層目が必要なら export、評価、及び結果の再利用を fail-closed に停止すること。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D5] [tasks: T012]
- [IN-05] 同じ解決済み Cargo target directory の rustdoc export を 120 秒上限の単一排他 lock で直列化し、lock を取得できない又は lock 操作が失敗した場合は lockless export、既存 JSON の再利用、又は retry を行わず fail-closed に停止すること。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D6] [tasks: T011, T014]
- [IN-06] rustdoc の共有出力 lock を、descriptor-relative かつ no-follow の trusted-root 検証を提供する Unix だけで扱い、それ以外の platform、symlink を経る target directory、又は検証不能な target directory を fail-closed で拒否すること。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D7] [tasks: T010, T017]
- [IN-07] 解決済み入力を immutable な content-addressed snapshot に固定し、同じ lock を期待出力 path の決定、export、出力 path の確認、及び JSON bytes の snapshot 読み取りまで保持して、入力又は出力の世代混在と ABA を fail-closed に防ぐこと。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D8] [tasks: T011, T016, T019]

### Out of Scope
- [OS-01] 参照側 catalogue に cross-crate 宣言を追加して、宣言を二重化する方式。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T002, T007, T008, T009]
- [OS-02] cross-crate 参照に限って短名 fallback を復活させる方式。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T003, T007]
- [OS-03] cross-crate 参照を実装まで未解決のまま残す方式。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T003, T007]
- [OS-04] catalogue の宣言 hash だけ、partial fingerprint、又は任意 target 出力の走査により型シグナルを再利用する方式。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D3, knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D4] [tasks: T010, T018]
- [OS-05] 64 層超過時の分割継続、lockless export、lock 待機超過後の retry、既存 JSON への fallback、又は Unix 以外で path-based fallback を許す方式。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D5, knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D6, knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D7, knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D8] [tasks: T012, T014, T017]

## Constraints
- [CO-01] 解決集合は一箇所で構築し、既存の自層 add 宣言の入力に他層 add 宣言を加える。経路ごとの add 型特例を新設しない。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009]
- [CO-02] 対象となる他層の集合は architecture-rules.json が定める TDDD 有効層に委ね、catalogue ファイルがない層は宣言なしとして扱う。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T001, T002, T005, T006, T008, T009]
- [CO-03] 合成項目は宣言層の crate 名を identity root とし、bin-target alias は既存の正準化を通す。module_path は明示時はその配置を用い、省略時は既存の配置未確定規則に従う。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D2, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D2, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D3] [tasks: T001, T002, T003, T006, T007, T008, T009]
- [CO-04] 実装 fingerprint は、rustdoc graph を変え得る入力を D3 の authoritative input set、すなわち workspace 内で Cargo rustdoc の入力となる通常ファイルの相対 path と内容 hash、および D3 が列挙する環境値に限定し、除外 directory を走査しない。workspace 外の path dependency、build script の入力・生成物、toolchain・CARGO_HOME の内容など、この集合に含められない入力を検出した場合は、外部境界を暗黙に許可せず authoritative-input error として export と再利用を fail-closed にする。catalogue と baseline の完全集合は architecture-rules.json の解決規則に委ね、独自の発見で補完しない。I/O の各定量上限、symlink、I/O error、又は入力の途中変更では partial fingerprint を作らず、古い結果へ fallback しない。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D3, knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D4] [tasks: T010, T013, T018]
- [CO-05] 一回の context 組立てで実行する rustdoc export は最大 64 層とし、65 層目が必要なら評価、export、結果再利用のすべてを fail-closed にする。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D5] [tasks: T012]
- [CO-06] 同じ解決済み Cargo target directory の export は 120 秒上限の単一排他 lock で直列化する。評価に使う target directory はこの機能が所有する専用領域として確保し、期待 rustdoc JSON を作成又は置換する全 writer（通常の Cargo rustdoc と別 exporter を含む）は同じ lock と所有境界を通らなければならない。非協調 writer を排除できない、又は専用領域の排他的所有を検証できない target directory は使用せず fail-closed にする。lock は descriptor-relative・no-follow の trusted-root 検証を提供する Unix だけで扱い、絶対 CARGO_TARGET_DIR は明示設定時でも全親 component の symlink 不在を検証する。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D6, knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D7] [tasks: T011, T014, T015, T017]
- [CO-07] 解決済み入力は content-addressed の in-memory snapshot に固定し、出力は同一 lock の臨界区間で期待 path の確認と JSON bytes の snapshot 読み取りまで完了する。target directory の専用所有と全 writer の同一 lock 参加を確認できない場合は出力を authoritative として扱わない。結果を書き出す前に開始時 snapshot との fingerprint 一致を確認する。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D8] [tasks: T011, T015, T016, T019]

## Acceptance Criteria
- [ ] [AC-01] ある TDDD 有効層が add 宣言した未実装の型又は trait を、同じ track の別の TDDD 有効層が参照できる。参照側 catalogue には、その外部項目を重複記述しない。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D1] [tasks: T001, T002, T004, T005, T006, T007, T008, T009]
- [ ] [AC-02] 合成された cross-crate 項目は、宣言層の crate 名を root とする fully-qualified identity で解決される。bin target の crate 名と rustdoc root 名が異なる場合も既存の正準化により解決され、明示 module_path 又は省略時の配置未確定の扱いは宣言層自身の規則と一致する。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D2, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D2, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D3] [tasks: T001, T002, T006, T007, T008, T009]
- [ ] [AC-03] 参照側の rustdoc paths が cross-crate add 宣言と同じ identity を持つ場合、rustdoc 項目が解決に用いられ、同一項目は合成されない。未根拠の cross-crate 参照は短名 fallback 又は実装までの未解決許容によって通過しない。 [adr: knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md#D2, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1] [tasks: T001, T003, T005, T006, T007, T008, T009]
- [ ] [AC-04] 型シグナル結果は、宣言・baseline・HEAD・実装 fingerprint・解決 fingerprint・解決済み Cargo target directory・crate/feature/profile・期待 rustdoc JSON path がすべて一致し、worktree が clean な場合にだけ再利用される。実装 fingerprint は、workspace 内で Cargo rustdoc の入力となる通常ファイルの相対 path と内容 hash、および D3 が列挙する環境値を含める。Cargo rustdoc が workspace 外の path dependency、build script の入力・生成物、又は D3 の環境値で識別できない toolchain・CARGO_HOME の内容を入力として必要とし、それを authoritative input set に完全に含められない場合は、export 前に authoritative-input error として失敗させる。同じ path の内容変更を fingerprint 一致として扱わず、完全な authoritative 入力の確定又は fingerprint 作成が失敗・上限超過した場合は、partial fingerprint、旧結果、又は snapshot を成功として利用しない。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D3, knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D4] [tasks: T010, T011, T013, T018]
- [ ] [AC-05] 一回の評価で 65 層目の rustdoc export が必要な場合は、export、評価、及び結果の再利用を fail-closed で停止し、層を分割して続行せず、上限外の層を既存 snapshot で補わない。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D5] [tasks: T012]
- [ ] [AC-07] 同じ解決済み Cargo target directory の共有出力 lock を 120 秒以内に取得できない、又は lock 操作に失敗した場合は評価を fail-closed で停止し、lockless export、既存 JSON の再利用、又は待機超過後の retry を行わない。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D6] [tasks: T011, T014]
- [ ] [AC-08] rustdoc export と snapshot の再利用は、descriptor-relative かつ no-follow の trusted-root lock を提供できる Unix でだけ許可し、それ以外の platform、symlink を経る target directory、又は trusted-root を検証できない target directory では fail-closed で拒否する。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D7] [tasks: T010, T017]
- [ ] [AC-06] 評価は解決済み入力と rustdoc JSON の immutable な内容 snapshot だけを用い、排他的に所有でき、期待 JSON を更新する全 writer が同じ lock に参加する target directory の lock を export から期待出力の確認・読み取りまで保持する。非協調 writer を排除又は検出できない場合は結果を受理せず失敗させる。開始後に入力 fingerprint 又は解決 fingerprint が変われば結果を破棄して失敗させ、path の再読による変化なし判定又は ABA を許さない。 [adr: knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md#D8] [tasks: T011, T015, T016, T019]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 28  🟡 0  🔴 0

