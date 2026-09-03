# Environment Assumptions Convention

## この source tree における位置付け

このファイルは SoTOHE-core source repository 自身が consumer として使用する環境前提宣言であり、
以下の具体的な値はこの repository の実際の実行環境を記録する。`.harness/config/template-boundary.json`
では source 側の `knowledge/conventions/` を `overlay` として扱うため、template export はこの
source 側の文書を reusable scaffold の共通既定値として出荷しない。exported scaffold に供給される
初期文書は consumer-neutral な別の overlay 内容であり、そこでは各 consumer が値を記入する。

> **強制先**: review 観点 — harness-policy scope

## Purpose

実行環境に依存する前提を、consumer が自分のプロジェクトの仕様として宣言するための枠を
提供する。この文書は前提の既定値を決める場所ではない。

> **強制先**: review 観点 — harness-policy scope

## Scope

- 新しく追加するコードと、環境との境界に関わる振る舞いを変更するコードに適用する。既存コードや既存の抽象の組を、この規約に合わせて遡及修正するためには使わない。
  > **強制先**: review 観点 — harness-policy scope
- この文書の宣言欄は原則として consumer が所有する。テンプレートは consumer 固有の platform、protocol、encoding、resource limit、concurrency model を既定値として記入しない。ただし、`対応プラットフォーム (Supported Platforms)` 欄の出荷済み type-signals rustdoc 評価器および gate-log persistence の platform bound と、`資源上限 (Resource Limits)` 欄の出荷済み `gate-output` 子プロセス捕捉契約は、consumer アプリの OS / limit 選択ではなく SoTOHE が出荷するコンポーネントの固定実行契約である。これらは **この source tree の consumer 宣言**（本ファイル）と source `README.md` の前提条件節に置く。template export は overlay の consumer-neutral 文書を出荷するため、exported scaffold へこれらの具体値を複製しない。
  > **強制先**: review 観点 — harness-policy scope

## Environment Declaration

この節を consumer の環境前提宣言として使う。`対応プラットフォーム (Supported Platforms)` 欄の
type-signals rustdoc 評価器および gate-log persistence と、`資源上限 (Resource Limits)` 欄の
`gate-output` 子プロセス捕捉に関する記載は、consumer アプリの OS / limit 選択ではなく、SoTOHE が
出荷するコンポーネントの固定実行契約であり、この source tree が consumer として記録する。
exported overlay はこれらの具体値を持たず、各 consumer が自分の宣言欄を記入する。
consumer 固有の前提は、これらの出荷契約を除く欄を実際の前提で埋め、適用外の欄には「適用外」と
その理由を記入する。空欄や、判断を先送りする `TODO` を残したまま、前提に依存する設計や実装を
追加してはならない。

> **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）

### 対応プラットフォーム (Supported Platforms)

出荷する type-signals rustdoc 評価器と gate-log persistence の platform 前提。consumer プロジェクト固有の OS 選択ではなく、出荷コンポーネントが trusted-root を検証できる範囲を宣言する。

- 対応する platform、architecture、runtime: type-signals rustdoc 評価器は Unix（Linux を含む）で、architecture は特定の CPU に依存しない。runtime は、解決済み target root から専用 selection directory と lock file を descriptor-relative に開き、親を含む path を no-follow で検証できる Unix runtime。gate-log persistence は Linux (x86_64) と `cargo make` が使う Docker `tools` container。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- 対応外または条件付きの範囲: Windows、および descriptor-relative open と no-follow 検証を提供しない platform は unsupported。そのような環境では rustdoc snapshot の再利用および export を fail-closed で拒否する。gate-log persistence は macOS/Windows で `FsGateLogPersistence::reserve` が Linux-only reason の `CreateFile` を返す。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- platform 差が入力、ファイル、時刻、プロセス、または終了処理に与える条件: 専用 selection directory の immediate parent の file name は正確に `.sotp-rustdoc` であること。親 Cargo `target/` や他の非専用 directory を authoritative rustdoc output home として受け入れない。条件を満たせない場合は identity resolution を開始せず失敗する。exclusive create と nofollow directory open は Unix descriptor-relative API を使う。`RenameFlags::EXCHANGE` publish は Linux-only で、explicit platform gate は非 Linux では log file を作る前に失敗する。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）

### 入力エンコーディング方針 (Input-Encoding Policy)

- 受け付ける入力経路と encoding: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- decode、正規化、改行や byte order などの扱い: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- encoding が不正、未指定、または判定できない場合の扱い: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）

### 資源上限 (Resource Limits)

出荷する `gate-output` 子プロセス捕捉の前提。consumer アプリ固有の入力サイズ上限ではない。

- 入力サイズ、メモリ、保存領域、処理時間、同時実行数などの上限: 子プロセスの stdout/stderr に追加の byte cap は置かない。`ProcessGateRunner` は `Command::output()` で両 stream を全量メモリに保持し、`combine_output` が結合済みの追加コピーを作るため、設定されたメモリ上限はなく、必要メモリは子の出力サイズに比例し、捕捉中は複数の出力コピーが同時に存在する。メモリ枯渇時は allocation failure によりプロセスが abort / terminate し得て、完全な log や `GateLogWriteOutcome::Unavailable` は生成されない。捕捉結果は `tmp/gate/` に全量保存する。`ProcessGateRunner` は独自の実行期限・cancellation を持たない。処理時間の上限は、明示的に timeout を提供する呼び出し側だけが強制する。同時実行数の上限は呼び出し側のゲート直列化に従う。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- 上限の単位、適用範囲、超過時の失敗動作: byte 上限は適用しない。子が終了しない場合、この adapter は wait し続ける。呼び出し側が明示的に timeout を設定した場合に限り、その時間境界がプロセスを切る。scoped `cargo make` entrypoint 自体は timeout を設定しないため、local execution では無期限に待つことがある。保存領域の枯渇は OS の write 失敗として `GateLogWriteOutcome::Unavailable` になる。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- 上限を設けない項目がある場合の理由と、代わりに置く境界: ゲート失敗の診断はフルログを必要とし、compact stdout 契約は全文をファイルへ退避する。代わりの境界は、timeout を明示的に設定する呼び出し側の CI / オーケストレータとディスク容量である。scoped `cargo make` entrypoint は timeout を設定しない。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）

### 並行モデル (Concurrency Model)

- thread、task、process などの実行単位と、同時実行の上限: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- shared state の所有、同期、順序、再入可能性: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- cancellation、shutdown、失敗時の処理: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）

## Writing Guidance

- 前提に依存する仕様、型、または実装を追加する前に、該当する宣言欄を更新する。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- platform や limit だけでなく、適用範囲、単位、条件、失敗時の動作を記入する。読み手が値の意味を推測しなければならない書き方をしない。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- 既存の前提が変わったときは、影響を受ける仕様と境界を同じ変更で確認し、宣言と実装の食い違いを残さない。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- template の欄に project 固有の値を補って出荷しない。値が必要な場合は consumer が自分の宣言として記入する。
  > **強制先**: review 観点 — harness-policy scope
- ここにない外部条件へ依存する場合は、暗黙の前提として扱わず、該当欄を追加・更新してからその依存を採用する。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）

## Examples

### 記入済みの形

値そのものは consumer ごとに異なるため、テンプレートでは placeholder のみを示す。

> **強制先**: review 観点 — harness-policy scope

```text
Supported Platforms: <platform / architecture / runtime>
Input-Encoding Policy: <accepted encoding and invalid-input policy>
Resource Limits: <limit, unit, scope, and excess behavior>
Concurrency Model: <execution unit, sharing, ordering, and cancellation>
```

### 避ける形

- `TODO` を残したまま、入力や資源の前提をコードの暗黙の既定値にする。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- template が特定の platform や encoding を全 consumer の既定値として扱う。
  > **強制先**: review 観点 — harness-policy scope

## Exceptions

- 4 つの分類のいずれかが本当に適用外の場合は、宣言欄に「適用外」と理由を記録する。空欄で済ませたり、個別の実装だけに例外を隠したりしない。
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- project 固有の前提をテンプレート側の共通既定値に昇格する必要が生じた場合は、この文書を黙って変更せず、別途その判断を記録する。
  > **強制先**: review 観点 — harness-policy scope

## Review Checklist

- [ ] 変更が接する platform、encoding、resource、concurrency の前提が列挙されているか
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- [ ] 前提がこの宣言または仕様に記録され、未宣言の前提に依存していないか
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- [ ] 上限には単位・範囲・超過時の動作があり、並行モデルには共有状態・順序・取消しの扱いがあるか
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- [ ] template の本文に consumer 固有の platform、protocol、encoding、resource limit、concurrency model が混入していないか
  > **強制先**: review 観点 — harness-policy scope

## Related Documents

- [Project Conventions](README.md)
- [Enforce by Mechanism Convention](enforce-by-mechanism.md)
- [型シグナルの rustdoc 再利用と実行環境を拘束する](../adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md) D7
