# TDDD Product Correctness Convention

## 概要

SoTOHE / TDDD は **任意の Rust コードを対象とする汎用プロダクト**であり、特定のトラックのためだけに存在するのではない。signal evaluator・codec・identity matching・catalogue schema の正しさは「**どんな adopter のコードでも正しく動くか**」で判断する。

## 判断基準

- 評価器 / codec / signal matching / schema の finding・設計判断は、**任意の Rust adopter のコード**で正しいかで評価する
- 現在のトラックの catalogue（`*-types.json`）に該当パターンが含まれるか否かは、コア機構の正しさとは無関係

## 「現トラックでは発生しない」は dismiss 理由にならない

以下のような、プロダクトが対象とする一般ケースで成立するなら実バグとして扱う。「今の catalogue には無いから latent」で finding を棄却・defer しない。

- cross-crate impl / 外部 self type（`impl MyTrait for std::vec::Vec<i32>` 等）
- 同名型の identity-key 衝突（local `Error` と `std::error::Error` 等）
- catalogue と baseline で shape（generics / where / methods）が相違するケース
- 全 action type（Add / Modify / Reference / Delete）

## コア機構 vs トラック固有データ

- コア機構（評価器・codec）の正しさと、トラック固有の catalogue データを混同しない
- トラック固有の catalogue 記述ミスは catalogue 側で直す。コア機構の欠陥はコア側で直す

## consistency と completeness の区別

- 自動レビュー（reviewer capability）は **consistency**（コードが spec / ADR と整合するか）には強いが、**completeness**（設計自体が完全か）には弱い
- spec / ADR 自体に設計 gap があると、コードがそれと整合していても finding は挙がらない
- 何かを「独立 entry」化する（親を外す）等の設計変更では、**親が暗黙に供給していた性質**（例: `action`）を洗い出して明示的に置き換えたか、を設計レベルでレビューする
