# Review Protocol Convention

## 概要

reviewer capability によるコードレビューサイクルの運用ルール。
reviewer は外部プロバイダー（既定: Codex CLI）であり、self-review は代替にならない。

## コミット前レビュー必須

- **すべてのコミット**（コード変更だけでなく計画 artifact 含む）の前にレビューサイクルを実行する
- レビューなしのコミットは禁止。CI ゲートではなくワークフロー規律として強制

## zero_findings 完了条件

- reviewer が `zero_findings` を返すまでレビューサイクルを継続する
- 修正後に「たぶん通るだろう」で完了宣言してはならない。修正後に必ず confirmation round を実行する

## reviewer の独立性

- Claude Code の inline review（self-review）を reviewer capability の代替にしてはならない
- 外部 reviewer（Codex CLI 等）が失敗した場合はリトライ（最大 2 回）→ ユーザーにエスカレーション
- self-review で `zero_findings` を宣言してコミットに進むことは禁止

## モデルエスカレーション順序

- fast model → full model の順で実行する
- fast model が `zero_findings` を返した後に、full model で confirmation round を実行する
- fast model のみで完了宣言してはならない

## verdict の改竄禁止

- reviewer が返した verdict をそのまま記録する
- out-of-scope の finding があっても verdict を書き換えない
- 対処できない finding がある場合はユーザーに相談する

## finding の disposition ルール

### やってはいけないこと

- P1 finding を「pre-existing」「out of scope」として勝手に棄却しない → ユーザーに確認
- finding を修正せずに accepted list に追加して回避しない
- reviewer finding を「幻覚だろう」と推測で棄却しない → 必ず `Grep` でソースを確認してから判断

### やるべきこと

- finding は原則として修正する
- テスト失敗を「既存の問題」として無視しない → `git stash` + main でベースライン確認
- P1 deviation の受け入れはユーザー承認が必要

## レビュー対象サイズ

- タスク単位でコミット・レビューする（バルクコミット禁止）
- 目安: 1 レビュー briefing あたり **500 行以下**
- 3,000+ 行の diff はエッジケースの無限ループを引き起こす → タスク分割を検討
- 5 ラウンド超えたら分割を検討

## ローカルレビュー → PR レビューの順序

- PR レビュー（@codex review）の前にローカル Codex レビューを実行する
- ローカルで finding を潰してから PR に出すことで、高コストな PR ラウンドトリップを削減

## PR レビュー時の accepted deviation 記載

- accepted deviation は **PR body** に記載する（`pr-review.md` ではなく）
- フォーマット: 番号付きリスト、太字タイトル
- accepted deviation が PR review ノイズを圧倒する場合は squash を検討
