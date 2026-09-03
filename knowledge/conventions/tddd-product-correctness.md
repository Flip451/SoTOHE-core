# TDDD Product Correctness Convention

## 概要

SoTOHE / TDDD は **任意の Rust コードを対象とする汎用プロダクト**であり、特定のトラックのためだけに存在するのではない。signal evaluator・codec・identity matching・catalogue schema の正しさは「**どんな adopter のコードでも正しく動くか**」で判断する。

> **強制先**: review 観点 — types / infrastructure / harness-policy scope

## 層の性質と adopter の構成

role の意味と TDDD 機構の正しさは、特定の crate 名や固定された layer id に依存してはならない。role × layer の配置は `type-designer-kind-selection.md` R1 の `innermost` / `application` / `driven adapter` / `driving adapter` / `composition root` という層の性質で判断し、実際の layer id は adopter の `architecture-rules.json` の `layers[]` 宣言を参照して定める。`architecture-rules.json` は layer id・path・依存方向を宣言するが、層の性質を自動解決する機械写像ではない。

> **強制先**: review 観点 — types / infrastructure / harness-policy scope

機械検査へ渡す layer id は、その architecture declaration に存在する id と `KindLayerConstraint` の literal な設定が一致していなければならない。この一致と、literal id が R1 の層の性質へ正しく対応することは review で確認する。

> **強制先**: review 観点 — types / infrastructure / harness-policy scope

binary crate の薄い process entrypoint は R1 の五つの層の性質への機械写像を持たない。引数解析、composition root / adapter の呼出し、終了コード変換だけを行う起動シェルに対する `Dto` / `FreeFunction` / `ErrorType` の literal な lint allowance は、R1 の layer mapping ではなく、その境界表現のための例外として review する。独自の application operation・adapter 実装・object graph を持つ責務は process entrypoint に残さず、対応する R1 の性質の layer で表現する。

> **強制先**: review 観点 — types / infrastructure / harness-policy scope

機械検査が通ることだけで層の性質の判断を済ませず、crate 名を変えた adopter でも同じ role の意味と配置制約が保たれることをレビューする。

> **強制先**: review 観点 — types / infrastructure / harness-policy scope

## 判断基準

- 評価器 / codec / signal matching / schema の finding・設計判断は、**任意の Rust adopter のコード**で正しいかで評価する
- 現在のトラックの catalogue（`*-types.json`）に該当パターンが含まれるか否かは、コア機構の正しさとは無関係

> **強制先**: review 観点 — infrastructure / types / harness-policy scope

## 「現トラックでは発生しない」は dismiss 理由にならない

以下のような、プロダクトが対象とする一般ケースで成立するなら実バグとして扱う。「今の catalogue には無いから latent」で finding を棄却・defer しない。

> **強制先**: review 観点 — infrastructure / types / harness-policy scope

- cross-crate impl / 外部 self type（`impl MyTrait for std::vec::Vec<i32>` 等）
- 同名型の identity-key 衝突（local `Error` と `std::error::Error` 等）
- catalogue と baseline で shape（generics / where / methods）が相違するケース
- 全 action type（Add / Modify / Reference / Delete）

> **強制先**: review 観点 — infrastructure / types scope

## コア機構 vs トラック固有データ

- コア機構（評価器・codec）の正しさと、トラック固有の catalogue データを混同しない
- トラック固有の catalogue 記述ミスは catalogue 側で直す。コア機構の欠陥はコア側で直す

> **強制先**: review 観点 — infrastructure / types / harness-policy scope

## consistency と completeness の区別

- 自動レビュー（reviewer capability）は **consistency**（コードが spec / ADR と整合するか）には強いが、**completeness**（設計自体が完全か）には弱い
- spec / ADR 自体に設計 gap があると、コードがそれと整合していても finding は挙がらない
- 何かを「独立 entry」化する（親を外す）等の設計変更では、**親が暗黙に供給していた性質**（例: `action`）を洗い出して明示的に置き換えたか、を設計レベルでレビューする

> **強制先**: review 観点 — spec / types / infrastructure / harness-policy scope
