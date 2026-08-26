# Environment Assumptions Convention

## この文書の所有権

この規約は **利用プロジェクトが所有する**。初期値としてテンプレートから供給されるが、以後の改稿・改名・削除はプロジェクトの裁量である。テンプレートが提供するのは、環境前提を宣言するための枠と記入指針だけであり、**どの前提を採用するか** はこの文書を使うプロジェクトが決める。

> **強制先**: review 観点 — harness-policy scope

## Purpose

実行環境に依存する前提を、consumer が自分のプロジェクトの仕様として宣言するための枠を
提供する。この文書は前提の既定値を決める場所ではない。

> **強制先**: review 観点 — harness-policy scope

## Scope

- 新しく追加するコードと、環境との境界に関わる振る舞いを変更するコードに適用する。既存コードや既存の抽象の組を、この規約に合わせて遡及修正するためには使わない。
  > **強制先**: review 観点 — harness-policy scope
- この文書の宣言欄は consumer が所有する。テンプレートは欄と記入指針だけを提供し、特定の platform、protocol、encoding、resource limit、concurrency model を既定値として記入しない。
  > **強制先**: review 観点 — harness-policy scope

## Environment Declaration

この節を consumer の環境前提宣言として使う。4 つの欄を実際の前提で埋め、適用外の欄には
「適用外」とその理由を記入する。空欄や、判断を先送りする `TODO` を残したまま、前提に
依存する設計や実装を追加してはならない。

> **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）

### 対応プラットフォーム (Supported Platforms)

- 対応する platform、architecture、runtime: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- 対応外または条件付きの範囲: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- platform 差が入力、ファイル、時刻、プロセス、または終了処理に与える条件: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）

### 入力エンコーディング方針 (Input-Encoding Policy)

- 受け付ける入力経路と encoding: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- decode、正規化、改行や byte order などの扱い: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- encoding が不正、未指定、または判定できない場合の扱い: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）

### 資源上限 (Resource Limits)

- 入力サイズ、メモリ、保存領域、処理時間、同時実行数などの上限: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- 上限の単位、適用範囲、超過時の失敗動作: `TODO: consumer が記入`
  > **強制先**: review 観点 — harness-policy scope（宣言欄の編集は `knowledge/conventions/**` として harness-policy scope が審査する）/ spec scope（宣言に依存する仕様変更）
- 上限を設けない項目がある場合の理由と、代わりに置く境界: `TODO: consumer が記入`
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
