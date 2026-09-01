---
adr_id: "2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts"
decisions:
  - id: D3
    review_finding_ref: "infrastructure final@2026-08-29T18:02:54Z; adr-scope fast@2026-08-29T18:16:16Z findings 1-2; impl-plan final@2026-08-31T02:22:06Z"
    status: proposed
  - id: D4
    review_finding_ref: "infrastructure final@2026-08-29T18:02:54Z; impl-plan final@2026-08-31T02:22:06Z"
    status: proposed
  - id: D5
    review_finding_ref: "infrastructure final@2026-08-29T18:02:54Z"
    status: proposed
  - id: D6
    review_finding_ref: "infrastructure final@2026-08-29T18:02:54Z; impl-plan final@2026-08-31T02:22:06Z"
    status: proposed
  - id: D7
    review_finding_ref: "infrastructure final@2026-08-29T18:02:54Z; impl-plan final@2026-08-31T02:22:06Z"
    status: proposed
  - id: D8
    review_finding_ref: "infrastructure final@2026-08-29T18:02:54Z; impl-plan final@2026-08-31T02:22:06Z"
    status: proposed
---
# 型シグナルの rustdoc 再利用と実行環境を拘束する

## Context

`2026-08-28-1034-cross-crate-add-declaration-resolution.md` は、同じ track の別層が宣言した add 型を解決集合へ加える識別・配置・優先順位を定めた。しかし、型シグナルを再利用できる入力同一性、rustdoc の出力先を共有する実行の資源上限と排他、対応 platform、及び評価中に入力または出力が置き換わる場合の一貫性は未決定であった。

これらを暗黙の実装詳細にすると、同じ catalogue 宣言でも別の解決集合又は Rust 実装に対する結果を再利用し得る。また、Cargo の共有出力先に対する並行した rustdoc 実行は、別の実行の JSON を読んでしまう ABA 型の不整合を生み得る。ここでは、解決集合を拡張する既存決定を変えずに、その評価結果を信頼できる条件と fail-closed の環境契約を補う。

## Decision

### D3: 再利用キーは宣言・解決・実装・出力先を同時に識別する

型シグナルの記録済み結果を再利用するキーは、対象 catalogue の宣言 hash、baseline hash、評価開始時の commit 識別子、実装 fingerprint、解決 fingerprint、解決済み Cargo target directory、選択済み crate・feature・profile、及び期待 rustdoc JSON path をすべて含める。いずれかが異なる、記録を復号できない、又は worktree が clean でない場合は再利用せず、rustdoc を再抽出して評価する。

実装 fingerprint は、`.git`、`.harness`、`.codex`、`.claude`、`.agents`、`target`、`track`、`tmp` を除く workspace 内の全 regular file の相対 path と内容 hash、次の環境値、`cargo +nightly rustdoc` が実際に使用する nightly 選択済み `cargo`、`rustc`、`rustdoc` の各 content fingerprint、及び `cargo metadata --no-deps --locked` の結果 bytes を順序付きで含める。`PATH` 上の rustup proxy 自体の fingerprint をこれらの tool fingerprint として用いてはならない。環境値は `CARGO_BUILD_TARGET`、`CARGO_ENCODED_RUSTFLAGS`、`CARGO_HOME`、`CARGO_TARGET_DIR`、`CARGO_NET_OFFLINE`、`PATH`、`RUSTC`、`RUSTC_WRAPPER`、`RUSTC_WORKSPACE_WRAPPER`、`RUSTDOC`、`RUSTDOCFLAGS`、`RUSTFLAGS`、`RUSTUP_TOOLCHAIN` とする。

この実装 fingerprint は、上記で列挙した非除外 workspace regular file、環境値、nightly 選択済み tool の content fingerprint、及び Cargo metadata の bytes からなる記録済み identity に対する再利用だけを定める。include graph、build-script の入力及び出力、proc-macro expansion はこの identity の対象外とする。これは Cargo rustdoc の意味的な完全入力集合でも、rustdoc JSON のあらゆる変更を検出する fingerprint でもない。D4 の I/O 上限内でこの記録対象を取得できない、nightly 選択済み tool を確定できない、又はその content snapshot に失敗する場合は、partial fingerprint を成功として扱わない。

解決 fingerprint は architecture rules、設定済み catalogue の各 bytes、及び設定済み rustdoc baseline の各 bytes を含める。対象 catalogue と baseline の完全集合は、`architecture-rules.json` が列挙する TDDD 有効層と、その catalogue・baseline 解決規則に委ねる。独自の filesystem 発見で補完したり、解決規則が返さない任意の入力を取り込んだりしない。規則により完全集合を確定できない場合は authoritative-input error として失敗させる。Cargo 出力先は `CARGO_TARGET_DIR` 又は Cargo metadata で厳密に解決し、推測した既定値で snapshot を再利用してはならない。出力側で fingerprint 又は snapshot として読むことを許すのは、解決済み target directory の下で、選択済み crate・feature・profile に対応する期待 rustdoc JSON path とその bytes のみとする。任意の `target` 配下の走査又は別 crate の出力の混入を許さない。

### D4: 実装 fingerprint の I/O は定量上限を超えた時点で fail-closed にする

D3 の workspace 入力走査には、directory depth 64、directory entry 65,536 件、regular file 32,768 件、1 file 64 MiB、総 bytes 512 MiB、相対 path 16 KiB、allowlist の各環境値 64 KiB の上限を適用する。これらの上限は、`cargo +nightly rustdoc` が実際に使用する nightly 選択済み `cargo`、`rustc`、`rustdoc` の解決及び content snapshot と、`cargo metadata --no-deps --locked` の取得を含む、D3 の実装 fingerprint 作成全体にも適用する。

symlink、I/O error、nightly 選択済み tool の解決又は content snapshot の失敗、metadata 実行の失敗、型の途中変更、又はいずれかの上限超過では partial fingerprint を作らず、結果を authoritative-input error として失敗させる。失敗時は古い型シグナル又は snapshot へ fallback せず、再利用も成功扱いもしない。

### D5: 一回の context 組立てで rustdoc export は 64 層までとする

解決集合を組み立てる一回の評価で実行できる rustdoc export は最大 64 層とする。この上限は設定により要求された全 context に適用し、65 層目が必要なら export、評価、及び結果の再利用を fail-closed で停止する。層数を分割して黙って続行したり、上限外の層を既存 snapshot で補ったりしない。

### D6: 専用出力 directory の協調 writer は 120 秒の排他 lock で直列化する

期待 rustdoc JSON の選択先は、immediate parent の file name が正確に `.sotp-rustdoc` である専用 selection directory に限る。この directory を使う協調 rustdoc exporter の閉じた expected-output writer 集合だけを対象に、その directory に置く一つの advisory exclusive lock で直列化する。lock の待機上限は、個々の rustdoc 実行の上限と同じ 120 秒とする。poll 間隔はこの契約に含めず、待機時間の上限を延長してはならない。

120 秒以内に lock を取得できない、lock file を安全に開けない、又は lock 操作が失敗した場合は、その評価を `RustdocFailed` として停止する。lock なしの export、以前の JSON の再利用、又は待機超過後の retry は許さない。

### D7: 専用 selection directory を descriptor-relative に閉じ込められる Unix のみを対応 platform とする

共有出力を安全に扱う対応 platform は、解決済み target root から descriptor-relative に専用 selection directory と lock file を開き、親を含む path を no-follow で検証できる Unix とする。selection directory の immediate parent が `.sotp-rustdoc` でない場合、親 Cargo `target/` 又は他の非専用 directory を authoritative rustdoc output home として受け入れてはならず、identity resolution を fail-closed にする。Windows と、それ以外の platform はこの保証を満たす実装が提供されるまで unsupported とし、rustdoc snapshot の再利用及び export を fail-closed で拒否する。

絶対 `CARGO_TARGET_DIR` は明示設定された場合だけ workspace 外を許す。その場合も target directory の全親 component と専用 selection directory に symlink がないことを検証する。相対 path による workspace 外への脱出、又は symlink を経る target directory は拒否する。この契約は任意の shared Cargo target の writer に対する unforgeable な OS mandatory-access 境界を主張しない。非協調 writer を全て検出するのではなく、非専用 target を拒否して expected-output writer 集合から除外する。

### D8: 入出力を immutable snapshot と lock の同一臨界区間に束縛して ABA を排除する

評価は source、catalogue、architecture rules、及び D3 が定める関連入力を、評価開始時の内容 address された in-memory snapshot として捕捉してから行う。後続の解決・比較・評価はその snapshot だけを読み、snapshot 後の path 再読を評価入力にしてはならない。

rustdoc JSON では、同一の専用 selection directory の lock を、期待出力 path の選択、export、出力 path の一致確認、及び lock 内で copy する JSON bytes まで保持する。`SkipEvaluation` の成功を永続化する場合及び cache を再利用する場合は、評価区間の終端で snapshotted input の fingerprint が開始 snapshot と一致することを確認する。終端確認のための path 読み取りは評価入力ではない。不一致なら結果を破棄して失敗させる。disk 上で A から B を経て開始時と同じ bytes の A に戻る場合は、その snapshot と同じ generation として扱う。評価は snapshot 後にそれらの path を再読しないため、異なる generation の bytes を混在させる経路を持たない。

### Existing decision relationship

本 ADR の D3 から D8 は `2026-08-28-1034-cross-crate-add-declaration-resolution.md` D1 と D2 を **refines** する。D1 の他層 add 宣言を含む解決集合、D2 の宣言層に基づく identity・配置、及び rustdoc 優先順位を変更しない。それらの解決がどの入力と実行環境に対して有効か、及び安全に再利用できる条件だけを追加する。

## Rejected Alternatives

- **A: catalogue の宣言 hash だけで結果を再利用する**: Rust 実装、baseline、解決集合又は toolchain 環境の差異を見逃し、異なる入力の結果を有効にしてしまう。
- **B: Cargo の target directory 全体を fingerprint する**: 他 crate の生成物と実行中の一時物を入力へ取り込み、再利用の安定性も資源上限も失う。
- **C: lock 取得に失敗したとき lock なしで export するか、既存 JSON を読む**: 共有出力を別実行が置換する ABA を許し、結果の出所を保証できない。
- **D: platform ごとの path-based fallback を許す**: no-follow の trusted-root 保証を弱め、symlink 又は親 directory の置換を見逃す。

## Consequences

- 良: 型シグナルの再利用は、宣言だけでなく解決集合・Rust 実装・出力 artifact の同一性に結び付く。
- 良: 入力、I/O、並行 export の上限と超過時の挙動が明示され、資源枯渇又は不確実な snapshot を成功として扱わない。
- 負: 上限超過、未対応 platform、又は lock 競合では評価が停止するため、利用者は環境又は設定を修正する必要がある。
- 負: 64 を超える層を一度に評価するには、別の明示的な設計判断が必要になる。

## Reassess When

- supported platform が descriptor-relative、no-follow の trusted-root lock を実装できたとき。
- 64 層又は D4 の I/O 上限が正当な workspace で継続的に不足し、測定値に基づく変更が必要になったとき。
- Cargo が artifact 世代を識別する安定した API を提供し、D8 より狭い安全な snapshot 境界を定義できるとき。

## Related

- `knowledge/adr/2026-08-28-1034-cross-crate-add-declaration-resolution.md` D1 / D2 — 本 ADR が refines する解決集合の範囲、identity、配置、rustdoc 優先順位。
- `knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md` D1 — 解決集合を一箇所で構築する原則。本 ADR はその入力の再利用条件を補う。
