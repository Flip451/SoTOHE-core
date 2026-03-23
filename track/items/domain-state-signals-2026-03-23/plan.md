<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# SPEC-05 Domain States 信号機 Stage 2 — per-state signal + 遷移関数検証

Domain States の per-state 信号評価 (Stage 2) を実装する。
syn AST スキャンで domain コードから型名 + 遷移関数を自動検出し、主観排除の 🔵🟡🔴 判定を行う。

信号基準:
- 🔵 Blue: 型存在 AND (終端状態 OR 全宣言遷移関数が存在)
- 🟡 Yellow: 型存在だが遷移未発見、または transitions_to 未宣言
- 🔴 Red: 型未存在、またはプレースホルダー

前提: Stage 1 (spec signals red==0) が通過済みであること。

## Domain 型 + Codec 拡張

DomainStateEntry に transitions_to (Option<Vec<String>>) を追加。
spec.json codec に domain_state_signals フィールドを追加。
transitions_to の参照先が domain_states に存在しない場合は検証エラー。

- [x] Domain 型拡張: DomainStateEntry に transitions_to 追加 + DomainStateSignal per-state 型
- [x] spec.json codec 拡張: transitions_to + domain_state_signals フィールド + 参照整合性検証

## Domain コードスキャナー + 信号評価

syn AST で libs/domain/src/ をスキャン。型名検出 + 遷移関数検出。
Result<T, E> / Option<T> のアンラップで遷移先を判定。
終端状態 (transitions_to: []) は型存在のみで Blue。

- [x] Domain コードスキャナー (syn AST): 型名検出 + 遷移関数検出 (Result/Option アンラップ対応)
- [x] Per-state 信号評価ロジック: 型存在 × 遷移関数存在 × 終端/未宣言区別 → Blue/Yellow/Red

## CLI + Verifier

sotp track domain-state-signals コマンドで評価・書き戻し。
sotp verify spec-states に red==0 gate + Stage 1 前提条件を追加。

- [x] sotp track domain-state-signals CLI: 評価 → spec.json domain_state_signals 書き戻し
- [x] sotp verify spec-states 拡張: red==0 gate + Stage 1 前提条件チェック (spec signals red==0)

## レンダリング

spec.md Domain States テーブルに Signal + Transitions 列を追加。
plan.md に Stage 1 + Stage 2 信号サマリーを表示。

- [x] render_spec() 拡張: Domain States テーブルに Signal + Transitions 列追加
- [x] plan.md 信号サマリー: Stage 1 + Stage 2 集計表示を render_plan に追加
