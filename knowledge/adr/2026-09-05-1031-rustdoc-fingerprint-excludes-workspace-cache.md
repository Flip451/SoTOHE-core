---
adr_id: "2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache"
decisions:
  - id: D1
    review_finding_ref: "phase2:type-designer:calc-impl-catalog-workspace-cache-64mib"
    status: proposed
  - id: D2
    review_finding_ref: "phase2:type-designer:calc-impl-catalog-workspace-cache-64mib"
    status: proposed
  - id: D3
    review_finding_ref: "phase2:type-designer:calc-impl-catalog-workspace-cache-64mib"
    status: proposed
---
# rustdoc の実装 fingerprint から workspace 直下の生成キャッシュを除外する

## Context

型シグナルの実装 fingerprint は、除外領域以外の workspace regular file を走査する。現行実装は Cargo が解決した target directory と既存の運用領域を除外するが、workspace 直下の `.cache/` は除外していない。

一方、workspace の設定は `.cache/` を Cargo、sccache、ツールの生成キャッシュに使用し、Git の管理対象からも除外している。この領域を走査した結果、`.cache/home/.cache/ort.pyke.io/` 配下の `libonnxruntime.a`（90,647,244 bytes）が 1 file 64 MiB（67,108,864 bytes）の上限に達し、型シグナルを評価する前に失敗した。

64 MiB は既存 ADR が定めた安全上限であり、超過時の失敗自体は契約どおりである。今回の問題は、生成キャッシュを実装 fingerprint の入力集合に含めたことにある。ファイル単位の上限だけを引き上げても、依存パッケージやキャッシュの蓄積が総 bytes・件数上限を消費し、キャッシュ更新が実装変更として扱われる問題が残る。

`CARGO_TARGET_DIR=.cache` という単独の設定変更でも解消しなかった。今度は従来の `target/` が走査対象となり、106,558,007 bytes の生成ファイルで同じ上限を超えた。生成物の配置替えや定期削除を正常動作の前提にせず、入力集合の境界を定める必要がある。

## Decision

### D1: workspace 直下の `.cache/` を生成物専用領域として入力集合から除外する

実装 fingerprint の workspace 走査で、workspace root の直下にある `.cache/` を既存の除外領域に追加する。配下を列挙・読取・内容 hash 化せず、その内容を入力ファイル件数と bytes の予算に含めない。配下の生成ファイルの作成・更新・削除や、64 MiB を超える生成ファイルの存在だけでは、実装 fingerprint を変更したり取得を失敗させたりしない。

追加する除外はこの固定された領域に限る。深い階層の同名 directory、任意の Git ignored path、拡張子が `.a` や `.bin` のファイル一般へ拡張しない。Cargo target directory の解決と既存の除外規則は維持する。workspace 直下の `.cache` 自体が symlink の場合も、その参照先へ走査を進めない。除外対象外の symlink は、参照先がキャッシュであっても既存規則どおり拒否する。

`.cache/` は再生成可能なキャッシュの領域とし、正式な workspace ソースや入力データの置き場所にしない。この除外は workspace 内の記録対象を定めるものであり、Cargo の意味的な完全入力集合を独自に列挙する保証は追加しない。既存決定が対象外とする include graph、build-script 入出力、proc-macro expansion の扱いも変更しない。

### D2: 除外対象以外の入力に対する資源上限と fail-closed を維持する

1 file 64 MiB、総 bytes 512 MiB を含む既存の件数・サイズ・path・環境値の上限と、評価開始時の実行・drain 時間上限を維持する。対象入力の symlink、I/O error、途中変更、toolchain や Cargo metadata の取得失敗に対する既存の拒否規則も維持する。

除外対象外の入力が上限を超えた場合は、引き続き authoritative-input error として失敗させる。大きなファイルだけを黙って読み飛ばす、上限までの内容だけを hash 化する、失敗を旧 snapshot で補う、といった成功扱いは追加しない。

### D3: 新しい入力集合の規則で fingerprint を取得し直す

除外規則の変更前に得た fingerprint や型シグナルを、変更後の規則で取得・検証した結果として流用しない。新しい規則で入力 identity を取得し直し、その identity に対して再利用可否を判定する。

キャッシュ以外の記録対象、たとえば Rust ソースや Cargo manifest・lockfile の内容が変われば、引き続き入力 identity に反映する。評価開始時と終端の確認には同じ除外規則を適用する。失敗した評価の旧結果への fallback は許さない。

### Existing decision relationship

本草案の D1 と D3 は `2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md` D3 の workspace 入力集合と再利用条件を **refines** する。D2 は同 ADR の D4、および `2026-09-02-0000-evaluation-start-capture-time-bounds.md` D1 の上限・失敗契約を維持する。型シグナルの入力分類を扱う独立した判断である。

## Rejected Alternatives

- **A: 64 MiB 上限を引き上げる、または撤廃する**: 生成キャッシュを入力へ取り込む分類の問題が残り、総 bytes や件数の上限にも波及する。正当な対象入力について上限が不足する場合は、別の測定と判断を要する。
- **B: 大きなファイルやバイナリ拡張子を一律に無視する**: 正式な入力データまで除外し、サイズや拡張子で変更検知の保証が変わってしまう。
- **C: `.gitignore` に従って入力を決める**: バージョン管理への採否と fingerprint の入力契約を結び付け、既存決定が対象とする管理外の regular file まで除外し得る。
- **D: キャッシュ削除・移動、または target directory の切替を都度要求する**: 通常のビルドでキャッシュが再生成されると再発し、環境管理の負担と再ビルドのコストを利用者へ移す。
- **E: fingerprint 取得失敗時にも古い結果で先へ進む**: 確認できていない入力に対して検証成功を主張し、既存の fail-closed 契約を破る。

## Consequences

- 良: 標準の生成キャッシュが蓄積していても、キャッシュを理由とする fingerprint のサイズ・件数超過を避けられる。
- 良: キャッシュの更新と実装入力の変更を区別し、実装入力への既存の安全上限を維持できる。
- 負: `.cache/` に正式なソースや入力データを置く構成は、この入力 identity の保証対象にならない。その構成が必要になった場合は除外契約を再評価する。
- 負: 規則の切替時には fingerprint と関連評価を取得し直すコストが発生する。
- 中立: `.cache/` 以外の生成領域を自動で発見・除外する機構や、意味的な完全入力追跡は追加しない。

## Reassess When

- 標準のビルド設定が、`.cache/` と解決済み Cargo target directory 以外へ大きな生成物を置くようになったとき。
- `.cache/` 配下を正式な workspace ソースや記録対象入力に使う必要が生じたとき。
- 生成キャッシュを除外しても、正当な対象入力が既存の bytes・件数・時間上限に継続的に達すると測定されたとき。
- Cargo がより狭く信頼できる入力 identity を提供し、現在の workspace 走査を置き換えられるようになったとき。

## Related

- `knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md` D3 / D4 — 入力集合と定量上限。
- `knowledge/adr/2026-09-02-0000-evaluation-start-capture-time-bounds.md` D1 — authoritative input 捕捉の時間上限。
