<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# TSUMIKI-03 差分ヒアリング — /track:plan skill に差分ヒアリングフローを導入

/track:plan skill のヒアリングプロセスに差分ヒアリング（Tsumiki TSUMIKI-03）を導入する。
既存 spec.json の信号機評価（Blue/Yellow/Red）と source tags を活用し、不足・曖昧な項目のみをユーザーに質問する。
Rust コード変更なし。SKILL.md のプロンプト/ワークフロー変更のみ。

## 既存コンテキスト収集と分類

Phase 1 Step 3 で既存 spec.json を検出した場合、各項目のシグナルレベルを分類する。
Blue（source tag が document/feedback/convention）→ 確定済み、ヒアリング不要。
Yellow（source tag が inference/discussion）→ 確認推奨。
Red（MissingSource）→ 必須ヒアリング。
spec.json に記載されていないが必要な情報 → 欠落として必須ヒアリング。

- [x] SKILL.md Phase 1 Step 3 改修 — 既存 spec.json 検出時にシグナル分類を実施し、項目を確定(Blue)/要確認(Yellow)/要議論(Red)/欠落に分類するロジックを追加

## 差分ヒアリングフロー

Phase 1 Step 4 の固定質問リストを条件分岐フローに置換する。
既存 spec.json あり → 差分ヒアリング（Yellow/Red/欠落のみ質問）。
既存 spec.json なし → 従来の全体ヒアリング（フォールバック）。
ヒアリング結果は新しい source tag として spec.json に反映される。

- [x] SKILL.md Phase 1 Step 4 改修 — 固定質問リストを差分ヒアリングフローに置換。Blue 項目はスキップし、Yellow/Red/欠落の項目のみユーザーに質問。既存 spec.json がない新規 track の場合は従来の全体ヒアリングにフォールバック

## 提示形式と CI

Phase 3 Step 3 で差分ヒアリング結果を提示する形式を追加。
確定済み項目と新規確認項目を区別して表示し、ユーザーが何が変わったか一目でわかるようにする。
cargo make ci で全チェック通過を検証。

- [x] SKILL.md Phase 3 Step 3 改修 — 差分ヒアリング結果の提示形式を追加。確定済み項目（前回から変更なし）と新規確認項目を明確に区別して表示
- [x] CI 確認 — cargo make ci で全チェック通過を検証
