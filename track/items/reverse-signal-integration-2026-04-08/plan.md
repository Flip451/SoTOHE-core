<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# TDDD: 逆方向チェック信号機統合 + designer capability + /track:design

TDDD (Type-Definition-Driven Development) ワークフローを確立する。
逆方向チェックの undeclared types/traits を Red、定義済み未実装を Yellow として信号機に統合する。
verify spec-states ゲートを 2 段階判定に変更: 途中コミット時は Red なし → pass (Yellow 許容)。merge 時は全 Blue 必須 (Yellow もブロック)。
designer capability + /track:design コマンドを新設し、domain-types.json の初回作成と更新を体系化する。

## 逆方向 Red + 未実装 Yellow シグナル (domain 層)

undeclared types/traits を Red の DomainTypeSignal (kind_tag: undeclared_type / undeclared_trait) に変換する関数を追加。
定義済みだが TypeGraph に見つからない型に Yellow シグナルを返すよう evaluate_domain_type_signals を拡張。
Blue = 定義+実装+構造一致、Yellow = 定義+未実装 (WIP)、Red = 未定義+実装済み (TDDD 違反)。

- [x] undeclared types/traits を Red の DomainTypeSignal (kind_tag: undeclared_type / undeclared_trait) に変換する関数を domain 層に追加 45d7023
- [x] 定義済みだが未実装の型に Yellow シグナルを返すよう evaluate_domain_type_signals を拡張 (型が TypeGraph に見つからない場合 → Yellow) 2c3748a

## domain-type-signals コマンド拡張 + verify spec-states ゲート

domain-type-signals: forward 評価 (Blue/Yellow) + 逆方向チェック (Red) → domain-types.json 保存 → domain-types.md レンダリング → サマリ出力。
domain-types.json 不在時はエラー終了し /track:design を促す (TDDD: 初回作成は designer の責務)。
verify spec-states 2 段階ゲート: 途中コミット時は Red なし → pass (Yellow 許容、Red → fail + /track:design 案内)。merge 時は全 Blue 必須 (Yellow もブロック)。

- [x] domain-type-signals コマンドを拡張: 逆方向チェック → undeclared Red + 未実装 Yellow を domain-types.json に保存。不在時はエラー終了し /track:design を促す。domain-types.md レンダリング。サマリ出力 blue=N yellow=M red=K (undeclared=U) d143714
- [x] verify spec-states ゲートを 2 段階判定に変更: 途中コミット時は Red なし → pass (Yellow 許容、Red ブロック + /track:design 案内)。merge 時は全 Blue 必須 (Yellow もブロック) 0ba172e

## designer capability + /track:design コマンド

agent-profiles.json の全 profile に designer capability を追加 (既定 provider: claude)。
/track:design コマンドを .claude/commands/track/design.md に作成。
ワークフロー: (1) 対象トラックの plan.md を読み込み, (2) 既存 domain-types.json があれば読み込み (増分更新), (3) 既存コードがあれば TypeGraph も参照, (4) designer capability を呼び出し DomainTypeKind / members / transitions を設計, (5) domain-types.json を生成・更新。

- [x] agent-profiles.json の全 profile に designer capability を追加 (既定 provider: claude) 784346b
- [x] /track:design コマンドを作成: 対象トラックの plan.md + 既存 domain-types.json (あれば) を入力に designer capability を呼び出し domain-types.json を生成・更新 5047c95

## ワークフロー導線整備 + ADR 最終化

/track:plan 完了メッセージ・registry.md に /track:design を次ステップとして案内。
DEVELOPER_AI_WORKFLOW.md と knowledge/WORKFLOW.md に TDDD フロー (plan → design → implement) を追記。
ADR 2026-04-08-1800 の内容を最終確認。ADR README 索引のタイトルを実ファイルと一致させる。

- [~] /track:plan 完了メッセージ・registry.md・DEVELOPER_AI_WORKFLOW.md・knowledge/WORKFLOW.md に /track:design を次ステップとして案内する導線を追加
- [ ] ADR 2026-04-08-1800 を最終化し、ADR README 索引のタイトルを実ファイルと一致させる
