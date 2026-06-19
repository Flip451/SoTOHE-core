---
adr_id: 2026-06-16-1030-signal-gate-strictness-config
decisions:
  - id: D1
    user_decision_ref: "chat_segment:2026-06-17:signal-gate-strictness-config-draft"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:2026-06-17:signal-gate-strictness-config-draft"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:2026-06-17:signal-gate-strictness-config-draft"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:2026-06-17:signal-gate-strictness-config-draft"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:2026-06-17:signal-gate-strictness-config-draft"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:2026-06-17:signal-gate-strictness-config-draft"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:2026-06-17:signal-gate-strictness-config-draft"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:2026-06-19:signal-gate-strictness-config-d8-hexagonal"
    status: proposed
---
# signal CLI 名前空間の統一と gate strictness の宣言的管理 — `bin/sotp signal {calc,check}-<chain>` + `.harness/config/signal-gates.json`

## Context

### §1 SoTOHE の 4 つの評価信号 chain

SoTOHE は 4 つのシグナル chain でトラック・ADR 成果物の品質を評価する。各 chain は SoT Chain（下流 → 上流へ辿る参照の連鎖）の 1 リンクに対応する:

| chain | UI 名 | SoT Chain | 評価対象 |
| --- | --- | --- | --- |
| ⓪ | `adr-user` | ADR → user decision | ADR decision の来歴（grounding）完備性 |
| ① | `spec-adr` | spec → ADR | spec 要件の根拠完備性（grounding 品質） |
| ② | `catalog-spec` | catalogue → spec | 型カタログの spec 参照完備性 |
| ③ | `impl-catalog` | implementation ↔ catalogue | 型カタログ宣言と実装（rustdoc API）の一致（TDDD） |

各 chain のシグナルは `Blue` / `Yellow` / `Red` の 3 段階（domain の `ConfidenceSignal`）。`Blue`（根拠完備）は常に通過、`Red`（根拠欠落・矛盾）は strictness によらず常にブロック（`Finding::error`）。strictness が左右するのは `Yellow`（推論・部分検証）の扱いのみ — `strict=true` のとき Yellow もブロック（`Finding::error`）、`strict=false` のとき Yellow は warning（`Finding::warning`）のみ。ただし**現状この strict / interim 切替を持つのは chain ① ② ③ のみ**（`check_spec_doc_signals` / `check_catalogue_spec_signals` / `check_type_signals` が `strict: bool` を受ける）。chain ⓪ の `execute_verify_adr_signals` は `strict` 引数を持たず Yellow は無条件 warning であり、本 ADR §D2 でこの非対称を解消する。

chain ⓪（ADR 来歴）は `DecisionGrounds`（`libs/domain/src/adr_decision/grounds.rs`）で評価し、`UserDecisionRef`→🔵 / `ReviewFindingRef`→🟡 / `NoGrounds`→🔴 と同じ 3 色にマップする。加えて `Grandfathered`（legacy decision の backfill 免除）は信号対象外として skip される点だけが他 chain と異なる。

### §2 現状の CLI サーフェスと strictness 配置（散在・命名衝突・束縛）

現状、信号の「計算 / チェック / 描画」は単一の名前空間に整理されておらず、コマンドが `track` と `verify` に散在し、別役割なのに類似名で配置されたコマンドが並んでいる。さらに一部は単一フラグで束縛されている。

**calc（計算 / 再生成）系 — `track` 配下に散在**:

- `track signals`（① spec 信号、永続化）/ `track catalogue-spec-signals`（②、永続化）/ `track type-signals`（③、永続化 — 結果を `<layer>-type-signals.json` へ書く canonical な calc）
- `track catalogue-impl-signals`（③ 関連の **on-demand diagnostic** — `apps/cli/src/commands/track/mod.rs:435-441` 記載: 出力ファイル無し / Makefile wrapper 無し / `ADR 2026-05-11-2330 §D3`。`type-signals` の重複ではなく markdown レポート専用の派生コマンド）
- ⓪ は ADR frontmatter から live 計算され、永続化する calc コマンドは存在しない

**check（ゲート評価）系 — `verify` 配下に散在・束縛**:

- `verify spec-states`（`verify_from_spec_json`, `spec_states.rs`）が **Stage1 = `check_spec_doc_signals`（①, :102）と Stage2 = 各 tddd.enabled レイヤーの `check_type_signals`（③, :127-129 / :301）を単一の `strict` フラグで束ねて評価する**
- `verify spec-signals` — spec.md の source-tag と frontmatter の整合 + `red == 0` の binary gate（strict 無し）。`spec-states` とは別物
- `verify catalogue-spec-signals`（② strict 可）/ `verify catalogue-spec-refs`（② binary refs gate）
- `verify adr-signals`（⓪）— **`strict` 引数を持たず**、Red→error / Yellow→warning をハードコード。`knowledge/adr/` 全体を走査する（repo-global）

**commit gate（`ci-local`）**:

- `verify-spec-states-current-local` → `verify spec-states`（`--strict` なし＝interim）。**この経路で ① と ③ の双方が commit 時に interim 評価される**（Red は block、Yellow は warning）
- `check-catalogue-spec-signals-local`（②）/ `verify-adr-signals-local`（⓪, interim）
- `track-active-gate`（commit 直前）が `track type-signals` → `track catalogue-spec-signals` → `track views sync` を再生成（非ゼロ終了で fail-closed abort）

**merge gate（`check_strict_merge_gate`, `merge_gate.rs`）**:

- ①: `check_spec_doc_signals(&spec_doc, /* strict */ true)` を直接呼ぶ（`merge_gate.rs:275`）
- ③: `check_type_signals(&signals_doc, /* strict */ true)` を直接呼ぶ（`merge_gate.rs:389`）
- ②: `chain2_gate::check_chain2_for_layer`（`merge_gate/chain2_gate.rs`）に delegate。その中で binary refs gate + Red/Yellow signal gate を **inline 実装**（`chain2_gate.rs:240`「Red and Yellow both block in strict merge mode」）— domain の `check_catalogue_spec_signals` を再利用しておらず、①③ と非対称
- **⓪ は呼ばれない**（`merge_gate.rs` に `execute_verify_adr_signals` の呼び出しなし）

**render（結果描画）系**: per-chain で統一された render は無く、`track views sync` や型グラフ描画などに散在。

> 補足（旧版の事実誤認の訂正）: chain ③ は「commit 時に一切実行されない / CI 未登録」ではない。`verify spec-states` の Stage2 として **commit 時に interim で評価済み**であり、独立した `verify type-signals` サブコマンドが無いだけである。

### §3 問題点

1. **組織的散在と命名衝突**: 同種操作（計算 / チェック / 描画）が動詞でなく雑多な名前で `track` / `verify` に散らばり、chain × 動詞の一覧性が無い。さらに `verify spec-signals`（spec source-tag↔frontmatter binary）と `verify spec-states`（① ③ signal gate）、`track type-signals`（③ 永続化）と `track catalogue-impl-signals`（③ on-demand diagnostic）のように、別役割なのに類似名で配置されており、ユーザーが「同じものの重複」と誤読しやすい。
2. **① と ③ の strict 束縛（アーキテクチャ上の阻害）**: commit 経路の `verify spec-states` が ① と ③ に**単一の `strict` フラグ**を共有しているため、commit gate で「① は strict・③ は interim」を独立指定できない。chain 別 strictness を宣言しても実装が追随できない。
3. **⓪ の strict 経路欠如**: `execute_verify_adr_signals` に `strict` 引数が無く、`review_finding_ref` 止まりの Yellow を block にできない。merge gate にも結線されていない。
4. **strictness が分散ハードコード**: merge gate は `/* strict */ true`、commit gate は Makefile の `--strict` 有無で決まり、chain × gate を一元的に調整する口が無い。

### §4 chain の性質と適切な strictness の差異

**chain ① ②（grounding 完備性）**: spec 要件・型カタログエントリに ADR / spec への参照が揃っているかを検査する。完備性は impl 進行と独立しており、タスク途中で Yellow になる必然性がない。→ **commit gate で Yellow block が自然**。

**chain ③（catalogue-impl 整合 / TDDD）**: 型カタログ宣言と実装の一致を検査する。TDDD では catalogue を先行宣言し実装で Blue 化する流れが標準のため、タスク途中は必然的に Yellow（宣言済み・未実装）になる。→ **commit gate では interim（warning）で通し、merge gate でだけ block するのが筋**。

**chain ⓪（ADR 来歴 / provenance）**: ADR decision が `user_decision_ref`（user 承認）まで接地しているかを検査する。来歴完備性自体は impl 進行と独立で、**構造的には commit gate で strict block しても破綻しない**（chain ③ の TDDD のような「Yellow が必然的に通過する」事情は無い）。

ただし `Yellow`（`review_finding_ref` のみ — review 由来で user へのエスカレート待ち、`2026-06-16-0042` §D1）を commit ごとに block すると、`/track:adr2pr` フローの途中で user 確認のために頻繁に止まり、自律的な実装サイクルが崩れる。**SoTOHE 自身は user 確認を adr2pr フローの「最初」（authoring / `tmp/adr` 草案 → `knowledge/adr/` 昇格時の ADR ヒアリング）と「最後」（merge 直前の merge gate）に集約する運用ポリシー**を採り、その帰結として **commit gate では `interim`、merge gate でだけ `strict`** を推奨デフォルトとする。

この commit=interim は **SoTOHE テンプレート自身の開発フロー選択**であり、構造的必然ではない（chain ③ commit=interim とは性質が違う）。テンプレート利用者は自分のフローに合わせて `commit_gate.adr_user: "strict"` を選んでよい（例: 各 commit ごとに ADR の `user_decision_ref` 完備を強制したい場合）。

`Red`（`NoGrounds` — untraced decision）は strictness によらず常にブロックする（全 chain 共通）。

### §5 calc/check 分離における鮮度検証

`calc-*` と `check-*` を分離する以上、**check が calc 結果と現在入力の drift（stale）を検出して止められる**ことが前提になる。chain ごとに鮮度検証の手段は異なるが、いずれも実装可能であることを確認した:

| chain | 永続化先 | 入力 | 鮮度検証 mechanism |
| --- | --- | --- | --- |
| ⓪ `adr-user` | **なし**（live） | `knowledge/adr/` 内の ADR ファイル | **N/A** — check が直接 ADR を走査して live 計算するため drift 不能（`adr_signals.rs:33,66`）。`calc-adr-user` は退化セル（D1）であり、本項は問題化しない |
| ① `spec-adr` | spec.json の `signals` フィールド | **spec.json 自身**（`requirements[].adr_refs[]` / `informal_grounds[]`） | **self-consistency check（新規）** — check 時に `SpecDocument::evaluate_signals()`（`spec.rs:314`、純粋関数）で requirements から signals を再計算し、保存値 `doc.signals()`（`spec.rs:301`）と比較。不一致なら stale エラーを返し `signal calc-spec-adr` の再実行を促す。入力が spec.json 内で完結するため外部ファイル依存なし |
| ② `catalog-spec` | `<layer>-catalogue-spec-signals.json` | catalogue file (bytes) + spec.json | **entry_hash 比較（既存）** — 各 signal が記録時の catalogue entry SHA-256 (`entry_hash`) を保持。check 時に current bytes から再計算した hash と比較し、不一致なら stale エラー（`merge_gate/chain2_gate.rs:197-238`、L221 `current_hash != signal.entry_hash()` → error） |
| ③ `impl-catalog` | `<layer>-type-signals.json` | catalogue file (bytes) + rustdoc API | **declaration_hash 比較（既存）** — type-signals doc が記録時の catalogue bytes SHA-256 (`declaration_hash`) を保持。check 時に current bytes から再計算した hash と比較し、不一致なら stale エラー（`spec_states.rs:271-280`「`re-run sotp track type-signals to refresh the evaluation result`」） |

**chain ① だけが新規実装を要する**が、入力が spec.json 内で完結し `evaluate_signals()` が純粋関数として既に存在するため実装は軽い。chain ② ③ は既存の hash 比較 mechanism をそのまま新 `check-catalog-spec` / `check-impl-catalog` に引き継ぐ。chain ⓪ は永続化なし（live 計算）のため鮮度問題が構造的に発生しない。

stale 検出時の挙動は全 chain で共通: `Finding::error` を返して該当 `calc-*` の再実行を促す（warning 化はしない — stale 入力で signal を評価することはできない）。

なお上記マトリクスは **D7 で導入する `PersistedSoTChain` trait の `check_freshness` メソッドとして型化**される。①②③ では impl 必須・⓪ は `PersistedSoTChain` 対象外（live 計算で構造的に不要）。ADR とコードが drift しない構造をここで固定する。

## Decision

### D1: `bin/sotp signal` 名前空間と taxonomy を導入する

信号操作のうち **chain 軸で自然に直交するもの（`calc` / `check`）** を単一の `signal` 名前空間に集約し、**動詞 × chain の直交 8 コマンド**として再編する:

| chain ＼ 動詞 | `calc-*`（信号を計算 / 再生成） | `check-*`（ゲート評価 `[--strict]`） |
| --- | --- | --- |
| `adr-user`（⓪） | `signal calc-adr-user` ※ | `signal check-adr-user [--strict]` |
| `spec-adr`（①） | `signal calc-spec-adr` | `signal check-spec-adr [--strict]` |
| `catalog-spec`（②） | `signal calc-catalog-spec` | `signal check-catalog-spec [--strict]` |
| `impl-catalog`（③） | `signal calc-impl-catalog` | `signal check-impl-catalog [--strict]` |

- **UI 表記は `catalog`（米綴り）で統一する**。内部 Rust 型は既存の `catalogue`（例: `CatalogueSpecSignals`）を維持し、mass rename はしない（変更は CLI サーフェスのみ）。
- 動詞の責務: `calc` = 信号値（Blue/Yellow/Red）を計算し、永続物を持つ chain では再生成する。`check` = 信号をゲート評価して `Finding`（pass/warn/error）を返す。`calc` → `check` がゲートパイプラインを成し、`check-*` のみが `--strict` / `--gate` を取る。
- **退化セルの扱い**: `check-*` と `calc-*` は taxonomy 上 4 chain すべてに CLI コマンドとして存在させる。① ② ③ の `calc-*` は永続シグナルを再生成し、⓪ `calc-adr-user` は永続物を作らず ADR decision grounding を live 計算して validate / 表示する no-persist コマンドとする（省略しない）。ゲート判定は引き続き `check-adr-user` が担当する。

**`render`（描画）をこの直交枠に含めない理由**:

信号の描画は `calc` / `check` と性質が異なり、`render-<chain>` の固定 4 コマンドには押し込めない:

- **per-view であって per-chain でない**: 出力先（spec.md frontmatter / plan.md / contract-map / 型グラフ / サマリ）と形式（markdown / mermaid / table / JSON）で決まり、しばしば複数 chain を 1 ビューに集約する。chain 1 本ずつのコマンドに割ると集約ビューや複数形式が表現しづらい。
- **パラメータ形状が違う**: `check-*` は `--strict` / `--gate`（ゲートパイプライン）、描画は `--format` / 出力先 / 任意の chain フィルタ（プレゼンテーション）。同じ枠に入れると描画側が無関係なオプションを背負う。

よって `signal` taxonomy は `calc` / `check`（chain 直交）に限定する。描画は当面**現状経路（`track views sync` / 型グラフ描画 / spec.md frontmatter 生成）を維持**し、将来統一する場合も `signal` 名前空間には押し込めず、別の render/view 系コマンドファミリとして別 ADR で検討する（Rejected Alternative F）。

### D2: `check-*` を per-chain 化し、独立した `--strict` を与える

§3-2 / §3-3 の束縛を解消するため、check 経路を chain 単位に分割する:

- **`verify spec-states` を分割**する: Stage1（`check_spec_doc_signals` = ①）を `signal check-spec-adr` に、Stage2（`check_type_signals` = ③）を `signal check-impl-catalog` に分離し、**各々が独立した `--strict` を取る**。これにより commit gate で「① strict・③ interim」を独立指定できるようになる。
- **`signal check-adr-user` に `--strict` を新設**する（`execute_verify_adr_signals` に `strict: bool` 引数を追加）。`strict=true` のとき Yellow（`ReviewFindingRef`）も `Finding::error`、`strict=false` のとき `Finding::warning`。`Red`（`NoGrounds`）は常に `error`、`Grandfathered` は常に skip（不変）。
- `signal check-catalog-spec [--strict]` は現 `verify catalogue-spec-signals` を rehome したもの。

**影響範囲の明示**:

| 変更箇所 | 変更内容 |
| --- | --- |
| `libs/infrastructure/src/verify/spec_states.rs::verify_from_spec_json` | Stage1 / Stage2 を分離し、chain ごとに独立 `strict` を受け取る形へ分割。**Stage1（→ `check-spec-adr`）に self-consistency 鮮度検証を新規追加**: `doc.evaluate_signals()`（`spec.rs:314`）と `doc.signals()`（`spec.rs:301`）を比較し、不一致なら stale エラー（§5 参照） |
| `libs/infrastructure/src/verify/adr_signals.rs::execute_verify_adr_signals` | **`strict: bool` 引数を追加**（Yellow block 経路の新設） |
| `libs/domain/src/tddd/`（新規）+ `libs/usecase/src/merge_gate/chain2_gate.rs::check_chain2_for_layer` + `libs/infrastructure/src/verify/catalogue_spec_signals.rs` | chain ② の Red/Yellow 判定ロジックを domain 層に新規抽出（`check_catalogue_spec_signals(&doc, strict) -> VerifyOutcome` — chain ①③ と同型の純粋関数）。現状 infrastructure と usecase の 2 箇所に inline 実装されている同等ロジックを、新 domain 関数の呼び出しに置換して 1 本化し、config 駆動 strict を解決可能にする — ①③ との関数シグネチャ非対称も同時に解消 |
| `apps/cli/src/commands/`（新 `signal` モジュール） | `calc-*` / `check-*` サブコマンドを追加。`check-*` は `--strict` / `--gate` を受ける |
| `libs/usecase/src/merge_gate.rs::check_strict_merge_gate` | chain ⓪ の評価を追加（§D5） |

### D3: `.harness/config/signal-gates.json` を chain × gate × strictness の SoT とする

`check-*` が参照する strictness を宣言的に管理する。**この config は必須かつ完全**であり、不在・不正・キー欠落はいずれも §D4 で hard error（暗黙の default を持たない）。全 chain×gate セルを明示し、「ワークフロー上 `interim` が必要なセルだけ `interim`、残りは `strict`」と書き切る:

<!-- illustrative, non-canonical -->
```json
{
  "$schema_version": 1,
  "commit_gate": {
    "adr_user":     "interim",
    "spec_adr":     "strict",
    "catalog_spec": "strict",
    "impl_catalog": "interim"
  },
  "merge_gate": {
    "adr_user":     "strict",
    "spec_adr":     "strict",
    "catalog_spec": "strict",
    "impl_catalog": "strict"
  }
}
```

値は 2 値:

| 値 | 意味 |
| --- | --- |
| `"strict"` | Yellow もブロック（`Finding::error`） |
| `"interim"` | Yellow は warning のみ（`Finding::warning`）、Red はブロック |

この設定が chain ⓪ ① ② ③ × commit gate / merge gate の 8 セルを完全に制御する。`check-*` は `--gate commit|merge` で自分の chain × gate セルを解決し、`--strict` の明示が無い限り config を参照する。

**設計の意図**:

- chain ① ②（grounding 完備性）は commit gate で `strict`
- chain ③（catalogue-impl 整合）は commit gate で `interim`、merge gate で `strict`
- chain ⓪（ADR 来歴）は commit gate で `interim`（**SoTOHE のワークフロー選択** — adr2pr フローの最初と最後に user 確認を集約するため `review_finding_ref` Yellow の commit 通過を許容、§4 参照）、merge gate で `strict`（merge 前に `user_decision_ref` への昇格を強制）。`Red` は両ゲートで常時 block。**この commit=interim は構造的必然ではないので、テンプレート利用者は `"strict"` を選んでよい**（chain ③ commit=interim と性質が違う点に注意）
- chain × gate を 1 ファイルで一元調整できる

### D4: config の必須化と検証 — 不在・不正・不完全は hard error（fail-closed の最強形）

`.harness/config/signal-gates.json` は**必須かつ完全**とする。**存在しない / 読めない / パース不能 / `$schema_version` 不正 / 値が不正 / 必須キー（gate オブジェクトや chain×gate セル）の欠落**のいずれかがあれば、strict へ暗黙フォールバックせず **hard error で全ゲートを停止**し、`signal-gates.json を配置 / 修復せよ` という明示的・actionable な失敗にする:

| 状況 | 動作 |
| --- | --- |
| ファイル不在 | **hard error**（必須ファイル欠如 — 推奨デフォルトを配置せよ） |
| パース不能 / `$schema_version` 不正 / 値が不正 | **hard error**（設定ミスを silent に無視しない） |
| 必須キーの欠落（gate オブジェクト / chain×gate セルのいずれか） | **hard error**（暗黙の default を持たせない） |

**strict への silent フォールバックではなく error を採る理由**:

- config は D6 のとおり事実上**必須**（SoTOHE は TDDD のため `commit_gate.impl_catalog: "interim"` を持つ実ファイルを同梱する）。必須物の不在・不完全を silent な strict フォールバックで黙認すると、「config が無い / 不完全」という事実が表面化しないまま TDDD の commit が Yellow で止まり、原因が掴めない
- 設定の欠落・破損時に permissive 側（interim）へ倒すのは論外（fail-open, Rejected D）。silent な strict フォールバック（Rejected E）も「設定不備に気づけない」点で劣る。**最も actionable なのは error で止めて修復を促すこと**（これも block する以上 fail-closed の一種であり、その最強形）
- config には**暗黙の default を一切持たせない**。全 chain×gate セルを明示させることで、「書かれていないセルの挙動」という曖昧さを構造的に排除する

**TDDD ワークフローとの関係**:

commit gate で chain ③（impl-catalog）を `strict` にすると、TDDD の「catalogue 先行宣言 → 実装で Blue 化」ループが commit 時の Yellow で block される（Rejected Alternative A 参照）。したがって TDDD を回すには `commit_gate.impl_catalog: "interim"` を明示する必要があり、SoTOHE はこれを含む完全な config を実ファイルとして commit する（§D6）。

### D5: ゲート結線とコマンド移行

**ゲート結線**:

- commit gate（`ci-local`）と merge gate（`check_strict_merge_gate`）は、それぞれ 4 つの `signal check-* --gate commit|merge` を呼ぶ。利便のため集約コマンド `signal check --gate commit|merge`（4 chain を一括評価し signal-gates.json を解決）も用意する。
- merge gate に **chain ⓪ の評価を追加**する（現状未結線）。repo-global な ADR 来歴評価を `merge_gate.adr_user` の strict で判定する。
- `track-active-gate` の regen シーケンスを `signal calc-impl-catalog` → `signal calc-catalog-spec` → `track views sync` に更新する。

**コマンド移行（旧 → 新）**:

| 旧 | 新 |
| --- | --- |
| `track signals` | `signal calc-spec-adr` |
| `track catalogue-spec-signals` | `signal calc-catalog-spec` |
| `track type-signals` | `signal calc-impl-catalog` |
| `track catalogue-impl-signals` | on-demand diagnostic（出力ファイル無し / Makefile wrapper 無し）として現状経路に残置（`signal calc-impl-catalog` が永続化 calc、本コマンドはその markdown レンダー派生）。`signal` namespace へは移設しない（Rejected Alternative F） |
| `verify catalogue-spec-refs`（binary refs） | `--strict` を持たない別系統の binary gate として残置（`check-*` とは別物。`verify spec-signals` と同じ扱い） |
| `verify spec-states`（Stage1 ①） | `signal check-spec-adr` |
| `verify spec-states`（Stage2 ③） | `signal check-impl-catalog` |
| `verify catalogue-spec-signals` | `signal check-catalog-spec` |
| `verify adr-signals` | `signal check-adr-user`（+ `--strict` 新設） |
| `verify spec-signals`（source-tag↔frontmatter 整合） | `--strict` を持たない binary gate として残置（`check-*` とは別系統。将来 `render` 統一時に整合チェックとして再考） |

**後方互換**: SoTOHE はテンプレートであり、内部参照（`Makefile.toml` / `.claude/hooks` / `track-active-gate` / `merge_gate.rs` / docs）を**一括置換**する（alias 期間は設けない）。テンプレート利用者の fork は責務境界（§D6）。

### D6: テンプレート利用者の責務境界と同梱ファイル

`.harness/config/signal-gates.json` は **テンプレート利用者が編集可能** な設定ファイルとして位置づける:

- `knowledge/conventions/responsibility-boundary.md` のテンプレート利用者責任リストに追記する
- SoTOHE 側は推奨デフォルト（D3 の JSON）を **`.harness/config/signal-gates.json` の実ファイルとして commit する**（`agent-profiles.json` と同じ扱い）。D4 で config は必須（不在は hard error）であり、かつ SoTOHE 自身が TDDD を使う以上、`commit_gate.impl_catalog: "interim"` 等の緩和を持つ実ファイルが無いと gate が停止して commit ループが回らないため、`.example` のみの同梱では不十分
- **推奨デフォルトの 2 つの `commit_gate.*: "interim"` セルは性質が違う**ことに注意:
  - `commit_gate.impl_catalog: "interim"` は **TDDD ワークフローの構造的必然**（catalogue 先行宣言 → 実装で Blue 化、Rejected Alternative A）。TDDD を採るプロジェクトなら必須
  - `commit_gate.adr_user: "interim"` は **SoTOHE 自身の adr2pr フロー選択**（§4 / D3 参照: user 確認を adr2pr フローの最初と最後に集約）。**構造的必然ではない**ため、テンプレート利用者は自分のフローに合わせて `"strict"` を選んでよい（例: 各 commit ごとに `user_decision_ref` 完備を強制したい場合）
- 必要なら参考用に `.harness/config/samples/` 配下へ別ポリシーのバリアントを置く（`agent-profiles.*.json` の samples と同じ運用）。例: `signal-gates.adr-strict.json`（chain ⓪ も commit で strict）
- 設定スキーマを ADR 本体（本 ADR の D3 セクション）に文書化する

**スキーマ仕様**（config は完全かつ valid であることが必須 — 不在・不正・キー欠落はいずれも §D4 で hard error。下表の全キーが必須）:

| キー | 型 | 必須 | 説明 |
| --- | --- | --- | --- |
| `$schema_version` | integer | 必須（欠落 / 未知値は schema 不正 → hard error） | 現在は `1` のみ有効 |
| `commit_gate` | object | 必須 | commit gate（CI 経路）の chain 別 strictness |
| `merge_gate` | object | 必須 | merge gate の chain 別 strictness |
| `commit_gate.adr_user` | `"strict"` or `"interim"` | 必須 | chain ⓪ |
| `commit_gate.spec_adr` | `"strict"` or `"interim"` | 必須 | chain ① |
| `commit_gate.catalog_spec` | `"strict"` or `"interim"` | 必須 | chain ② |
| `commit_gate.impl_catalog` | `"strict"` or `"interim"` | 必須 | chain ③ |
| `merge_gate.adr_user` | `"strict"` or `"interim"` | 必須 | chain ⓪ |
| `merge_gate.spec_adr` | `"strict"` or `"interim"` | 必須 | chain ① |
| `merge_gate.catalog_spec` | `"strict"` or `"interim"` | 必須 | chain ② |
| `merge_gate.impl_catalog` | `"strict"` or `"interim"` | 必須 | chain ③ |

### D7: `ChainIdentity` / `SoTChain` / `LiveSoTChain` / `PersistedSoTChain` trait による chain 契約の型化

§5 で示した calc / check / freshness の共通形を Rust trait として明示化し、新 chain 追加時の "うっかり忘れ" と、chain ② で実際に起きたような **inline 重複実装による腐敗** を構造的に防ぐ:

```rust
/// 全 chain が持つ識別子と入力型。
pub trait ChainIdentity {
    const ID: ChainId;          // adr-user / spec-adr / catalog-spec / impl-catalog
    type Input<'a>;             // chain ごとに異なる（ADR dir / spec.json path / catalogue+spec.json / 同）
}

/// 全 chain が満たす check の最小契約（CLI check dispatch の入口）。
pub trait SoTChain: ChainIdentity {
    fn check(input: &Self::Input<'_>, strict: bool) -> VerifyOutcome;
}

/// 永続化を伴わない live calc を持つ chain（⓪）。
/// `signal calc-adr-user` はこの契約を使い、永続ファイルは作らない。
pub trait LiveSoTChain: SoTChain {
    type LiveCalc;
    type CalcError;

    fn calc_live(input: &Self::Input<'_>) -> Result<Self::LiveCalc, Self::CalcError>;
}

/// 永続化を伴う chain（① ② ③）。calc と freshness をコンパイル時に強制する。
pub trait PersistedSoTChain: ChainIdentity {
    type Persisted;             // SpecDocument / CatalogueSpecSignalsDocument / TypeSignalsDocument
    type CalcError;
    type StaleError;

    fn calc(input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError>;
    fn load(input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError>;
    fn check_freshness(input: &Self::Input<'_>, persisted: &Self::Persisted)
        -> Result<(), Self::StaleError>;
    fn evaluate_gate(persisted: &Self::Persisted, strict: bool) -> VerifyOutcome;
    fn calc_error(error: Self::CalcError) -> VerifyOutcome;
    fn stale_error(error: Self::StaleError) -> VerifyOutcome;
}

/// 永続 chain の SoTChain::check は blanket impl で 1 本化する。
impl<T> SoTChain for T
where
    T: PersistedSoTChain,
{
    fn check(input: &Self::Input<'_>, strict: bool) -> VerifyOutcome {
        let persisted = match T::load(input) {
            Ok(persisted) => persisted,
            Err(error) => return T::calc_error(error),
        };
        match T::check_freshness(input, &persisted) {
            Ok(()) => T::evaluate_gate(&persisted, strict),
            Err(error) => T::stale_error(error),
        }
    }
}
```

**trait 分割の根拠（chain ⓪ の構造的非対称）**:

| trait | 実装する chain | 理由 |
| --- | --- | --- |
| `SoTChain` + `LiveSoTChain` | ⓪ `adr-user` | live 計算で永続化なし。`calc_live` は必要だが、永続 `calc` / `check_freshness` は**構造的に不要** |
| `ChainIdentity` + `PersistedSoTChain`（blanket impl で `SoTChain`） | ① ② ③ | 永続化ファイルを持ち freshness 検証が必要。`SoTChain::check` は `load → check_freshness → evaluate_gate` の blanket impl でだけ提供する |

⓪ を 1 本の永続化トレイトに押し込むと不要メソッドを `unimplemented!()` 等で埋めることになり、「make illegal states unrepresentable」（CLAUDE.md `.claude/rules/04-coding-principles.md`）に反する。`PersistedSoTChain` を ⓪ に実装しないことで **「⓪ では永続 `calc` / `check_freshness` を呼べない」をコンパイル時に保証**し、D1 の `signal calc-adr-user` は `LiveSoTChain::calc_live` として明示的な no-persist 経路に置く。①②③ は `PersistedSoTChain` の blanket impl で `SoTChain::check` を得るため、`check` 経路から freshness を省いた個別実装を作れない。

**期待される効果**:

- 5 つ目の chain を追加すると live chain は `SoTChain`、persisted chain は `PersistedSoTChain` impl が必須 → CLI ディスパッチの漏れがコンパイルエラー
- 永続化を伴う chain は `PersistedSoTChain` impl が必須 → 「calc は実装したが freshness を忘れた」「inline 重複で check を書いた」がコンパイルエラー（chain ② の 2 重 inline 実装と同型の腐敗を構造的に再発不能化）
- 永続化しない `calc-adr-user` は `LiveSoTChain` impl が必須 → ⓪ の live calc だけを型化し、永続 freshness を誤って要求しない
- §5 の鮮度検証マトリクスが型シグネチャと **1:1 対応** → ADR とコードの drift が起きない
- domain 層の純粋関数（`check_spec_doc_signals` / `check_catalogue_spec_signals`（D2 新設）/ `check_type_signals`）は `evaluate_gate` から呼ぶ薄いラッパーで足り、既存ロジックの再実装は不要

**影響範囲**:

| 変更箇所 | 変更内容 |
| --- | --- |
| `libs/domain/src/chain.rs`（新規） | `ChainId` enum + `ChainIdentity` / `SoTChain` / `LiveSoTChain` / `PersistedSoTChain` trait 定義 + `impl<T: PersistedSoTChain> SoTChain for T` |
| `libs/usecase/src/chain/`（新規, 4 モジュール） | `AdrUserChain`（`SoTChain` + `LiveSoTChain`）/ `SpecAdrChain` / `CatalogSpecChain` / `ImplCatalogChain`（後 3 つは `PersistedSoTChain` 実装により `SoTChain` を blanket impl で得る） |
| `apps/cli/src/commands/signal/` | dispatch を trait 経由に集約（`calc-adr-user` は `LiveSoTChain`、①②③ の `calc-*` は `PersistedSoTChain`、全 `check-*` は `SoTChain` を経由する） |

D2 で言及した「chain ② の Red/Yellow 判定を domain 層に集約」は、`CatalogSpecChain::evaluate_gate` 経由で新 domain 関数 `check_catalogue_spec_signals` を呼ぶ形に自然に落ちる（trait 導入の副次効果として inline 重複の解消が強制される）。

### D8: ヘキサゴナル境界の固定 — `signal calc-*` / `signal check-*` のロジック配置

`signal` 名前空間の実装において、per-layer 反復・active-track 解決・TDDD レイヤー列挙・SHA-256 カタログハッシュ計算はすべて **domain または usecase の責務**とする。adapter（CLI コマンドファイル・cli-composition・Makefile タスク）へのロジック漏出を禁止し、ヘキサゴナルアーキテクチャの境界を構造的に固定する。

**D8-1: CLI コマンドファイルは薄いアダプターに限定する**

`apps/cli/src/commands/signal/<cmd>.rs` の各ファイルは以下の 3 要素のみを含んでよい:

1. clap 引数の解析（`struct` / `#[command]` 宣言）
2. usecase オーケストレーター（`libs/usecase/src/` のエントリポイント）への 1 回の呼び出し
3. `CommandOutcome` のレンダリング（stdout / stderr への書き出し）

per-layer ループ・パス構築・SHA-256 計算・レイヤー列挙（`domain / usecase / infrastructure` の列挙）・active-track 解決（ブランチ名 → track id の変換）は一切含んではならない。目標サイズは **1 ファイルあたり ≤ ~30 LOC**（集約目標であり、CI ゲートとしては計測しない）。

**D8-2: cli-composition はワイヤアップのみとする**

`apps/cli-composition/src/signal.rs` はアダプターグラフの構築・infrastructure primitive のクロージャ生成（依存注入）・usecase オーケストレーターへの呼び出しのみを行う。per-layer ループ・ハッシュ計算・パス構築・domain 判断をこのファイルに含んではならない。usecase オーケストレーターに渡すクロージャのシグネチャは `Fn(LayerId, &str) -> VerifyOutcome`（`&str` は usecase 内で計算された SHA-256 ハッシュ文字列）であり、クロージャ本体は対応する infrastructure 関数への委譲のみとする（例: `|layer_id, hash| infra_calc_impl_catalog(layer_id, hash)`）。infrastructure 関数が シグナルファイルのパスを layer_id から構築する責務を持つ。クロージャ本体にビジネスロジック・パス構築・ハッシュ計算を含んではならない。

**D8-3: Makefile タスクは bare コマンドシーケンスのみとする**

`Makefile.toml` 内の `signal` 名前空間タスクは `for LAYER`・`sha256sum`・パス構築パターンを含んではならない。`track-active-gate` は以下の 3 行の bare コマンドシーケンスへ縮小するか、完全に削除する（呼び出し箇所が直接 `bin/sotp` コマンドを呼べる場合）:

```makefile
# illustrative, non-canonical
bin/sotp signal calc-impl-catalog
bin/sotp signal calc-catalog-spec
bin/sotp track views sync
```

**D8-4: ロジック配置の原則**

責務の割り当ては以下の通りとする:

| 関心事 | 配置層 |
| --- | --- |
| per-layer 反復（domain / usecase / infrastructure を順に処理するループ） | `libs/usecase/src/`（usecase オーケストレーター — 具体的なモジュール配置は `chain/` または `signal/` のいずれでもよい） |
| active-track 解決（現在ブランチ名 → track id） | `libs/usecase/src/`（usecase オーケストレーター — 具体的なモジュール配置は `chain/` または `signal/` のいずれでもよい） |
| TDDD レイヤー列挙（`architecture-rules.json` から読み取るレイヤーセット） | `libs/domain/src/`（不変な primitive — `ChainIdentity` の定数として保持） |
| カタログ SHA-256 ハッシュ計算 | `libs/usecase/src/`（usecase オーケストレーター）または `libs/domain/src/`（domain primitive）。per-layer オーケストレーションの流れ上、usecase オーケストレーター内で計算してもよい（CN-17 参照）。 |
| signals ファイルパス構築（`<layer>-type-signals.json` 等、出力先） | `libs/infrastructure/src/`（infrastructure primitive — layer ID から output path を構築する）。CLI コマンドファイルおよび `apps/cli-composition` に含んではならない。 |
| CLI 引数解析・出力レンダリング | `apps/cli/src/commands/signal/` のみ |
| アダプターグラフ構築 | `apps/cli-composition/src/signal.rs` のみ |

**D8-5: infrastructure primitive の位置付けと依存注入方式**

usecase crate は `architecture-rules.json` の `may_depend_on: [domain]` 制約により infrastructure を直接 import できない。usecase オーケストレーターは 2 種類の依存注入を使う:

1. **ファイルシステムアクセス（`SignalLayerReader` ポート）**: `libs/usecase/src/signal/` にポートトレイト `SignalLayerReader` を定義する。メソッドは `active_track_id() -> Result<TrackId, ...>`・`enabled_layers(track_id: TrackId) -> Result<Vec<LayerId>, ...>`・`catalogue_bytes(track_id: TrackId, layer: LayerId) -> Result<Option<Vec<u8>>, ...>` の 3 つ。usecase オーケストレーターは `active_track_id()` で track ID を取得し、その `TrackId` を `enabled_layers(track_id)` と `catalogue_bytes(track_id, layer)` に明示的に渡す — これにより active-track 解決が usecase 層の責務として明確になる。直接ファイルシステムへのアクセスは usecase 関数に含まない。`libs/infrastructure/src/` がこのポートのローカル filesystem adapter を実装し、cli-composition はその adapter を usecase にワイヤするだけとする。テスト時はモック実装に差し替え可能。

2. **TDDD calc/check primitive（`per_layer_fn` ジェネリックパラメーター）**: usecase オーケストレーターは infrastructure の TDDD primitive を `per_layer_fn: impl Fn(LayerId, &str) -> VerifyOutcome` のジェネリックパラメーターとして受け取る。`&str` は SHA-256 ハッシュ文字列（hex）であり、`reader.catalogue_bytes()` から usecase 内で計算される。ファイルパスは usecase 関数にもクロージャのシグネチャにも含まない — signals ファイルのパスは infrastructure primitive（`infra_calc_impl_catalog` 等）が layer ID から内部で構築する責務を持つ。check variants では cli-composition 側のクロージャが `strict` をキャプチャするため、`per_layer_fn` のシグネチャ自体に `bool` は含まない。

呼び出し側（`apps/cli-composition/src/signal.rs`）が以下の infrastructure primitive をクロージャとして生成し、usecase オーケストレーターに渡す:

- calc-impl-catalog 相当: `libs/infrastructure/src/` の型シグナル計算関数（chain ③）
- calc-catalog-spec 相当: `libs/infrastructure/src/` のカタログスペックシグナル計算関数（chain ②）
- check-impl-catalog 相当: `check_impl_catalog_from_signals_file` またはその相当関数（chain ③）
- check-catalog-spec 相当: `check_catalog_spec_from_signals_file` またはその相当関数（chain ②）

これらの infrastructure 関数は `pub` を維持し、CLI コマンドや Makefile が直接呼ぶ対象ではない。`execute_verify_adr_signals_with_strict` は chain ⓪ (ADR-user, repo-global scan) の関数であり、TDDD の per-layer calc/check primitive ではない。chain ⓪ は per-layer オーケストレーターとは別の呼び出し経路（`check_strict_merge_gate` 内の chain ⓪ 評価）で使われるため、本 D8-5 の per-layer primitive リストには含めない。

## Acceptance Criteria

### AC-01: `signal` 名前空間が直交 8 コマンドで存在する（D1）

- `signal {calc,check}-{adr-user,spec-adr,catalog-spec,impl-catalog}` が CLI に存在する（UI 表記は `catalog`）
- `render` を per-chain 枠に含めない方針と、退化セル（`calc-adr-user`）の扱いが本 ADR D1 に文書化されている

### AC-02: `check-*` が per-chain で独立した `--strict` を持つ（D2）

- `verify spec-states` が `check-spec-adr`（①）と `check-impl-catalog`（③）に分割され、各々が独立 `--strict` を取る
- `check-adr-user` が `--strict` を取り、`execute_verify_adr_signals` に `strict: bool` が追加されている
- chain ② の Red/Yellow 判定が domain 層の `check_catalogue_spec_signals(&doc, strict)` に抽出され、`merge_gate/chain2_gate.rs` と `infrastructure/.../catalogue_spec_signals.rs` の 2 重 inline 実装がこの 1 本に集約されている（① ③ と同型の関数シグネチャ）
- commit gate で「① strict・③ interim」を独立に指定できる

### AC-03: `signal-gates.json` が 8 セルを制御する（D3）

- `check-*` が `--gate commit|merge` で chain × gate セルを解決する
- 推奨デフォルト config の下で、commit gate は ① ② strict・③ ⓪ interim、merge gate は全 strict で動作する

### AC-04: config 不在・不正・不完全が hard error になる（D4）

- `signal-gates.json` を削除した状態で commit gate / merge gate を実行すると、**hard error で停止**する（strict への暗黙フォールバックも fail-open もしない）
- パース不能 / `$schema_version` 不正 / 不正値の config は hard error
- **必須キー（gate オブジェクトや chain×gate セル）が 1 つでも欠落していれば hard error**（暗黙の default を持たない）

### AC-05: chain ⓪ が config 駆動で gate 別に動作する（D2 / D5）

- `merge_gate.adr_user: "strict"` のとき Yellow（`review_finding_ref`）が merge gate で block される
- commit gate では `commit_gate.adr_user: "interim"` により Yellow は warning で通過する
- `Red`（`NoGrounds`）は両ゲートで常に block、`Grandfathered` は常に skip
- `check_strict_merge_gate` が chain ⓪ を評価する

### AC-06: コマンド移行が完了し内部参照が一貫している（D5）

- 旧コマンド（`track signals` / `track type-signals` / `track catalogue-spec-signals` / `verify spec-states` / `verify adr-signals` 等）が新 `signal` 名前空間に移行 / 廃止されている
- `Makefile.toml` / hooks / `track-active-gate` / `check_strict_merge_gate` / docs が新コマンド名で一貫している

### AC-07: 各 chain の `check-*` が calc 結果の stale 入力を検出する（§5）

- `check-adr-user`: ADR ファイルを直接走査して live 計算する（鮮度問題が構造的に発生しない）
- `check-spec-adr`: `doc.evaluate_signals()` と `doc.signals()` を比較し、不一致のとき stale エラーを返して `signal calc-spec-adr` の再実行を促す（self-consistency check, 新規実装）
- `check-catalog-spec`: signal 記録時の `entry_hash` と current catalogue bytes の SHA-256 が不一致のとき stale エラー（既存 mechanism を引き継ぎ）
- `check-impl-catalog`: signal 記録時の `declaration_hash` と current catalogue bytes の SHA-256 が不一致のとき stale エラー（既存 mechanism を引き継ぎ）
- stale 検出は全 chain で `Finding::error` 固定（strictness によらず常時 block）

### AC-08: `ChainIdentity` / `SoTChain` / `LiveSoTChain` / `PersistedSoTChain` trait が 4 chain を覆う（D7）

- `libs/domain/src/chain.rs` に `ChainId` enum と `ChainIdentity` / `SoTChain: ChainIdentity` / `LiveSoTChain: SoTChain` / `PersistedSoTChain: ChainIdentity` の 4 traits が定義され、`impl<T: PersistedSoTChain> SoTChain for T` が定義されている
- ⓪ `AdrUserChain` は `SoTChain` + `LiveSoTChain` を実装し、① `SpecAdrChain` / ② `CatalogSpecChain` / ③ `ImplCatalogChain` は `PersistedSoTChain` を実装して blanket impl により `SoTChain` を得る
- chain ⓪ に対して永続 `calc` / `check_freshness` を呼ぶコードが**コンパイルエラー**になる一方、`signal calc-adr-user` は `LiveSoTChain::calc_live` 経由で実装される
- ①②③ の `PersistedSoTChain::evaluate_gate` は既存 domain 純粋関数（`check_spec_doc_signals` / `check_catalogue_spec_signals`（D2 新設）/ `check_type_signals`）を呼ぶ薄いラッパーとし、⓪ は `SoTChain::check` / `LiveSoTChain::calc_live` で ADR decision grounding を live 評価する
- CLI ディスパッチが trait 経由になっており、`calc-adr-user` は `LiveSoTChain`、①②③ の `calc-*` は `PersistedSoTChain`、全 `check-*` は `SoTChain`（①②③は blanket impl）に配線される

## Rejected Alternatives

### A: 全 chain を commit gate で strict に固定する（緩和不可）

chain ③（impl-catalog）も commit gate で恒常的に Yellow block にし、interim へ緩める手段を設けない選択肢。

却下理由: TDDD では catalogue 先行宣言 → 実装で Blue 化が標準であり、タスク途中に必然的に Yellow になる。全 strict に固定すると impl ループが阻害される。本 ADR は config を必須かつ完全とする（D4: 不在・不正・キー欠落は hard error、暗黙 default なし）。TDDD を回すための `commit_gate.impl_catalog: "interim"` は committed config（D6）で明示的に与える点が本選択肢と異なる。「strict 固定」ではなく「全セルを明示する config で必要なセルだけ interim」を採る。

### B: 名前空間を作らず Makefile / Rust に直接 `--strict` を散在させる

config も `signal` 名前空間も導入せず、現状の `track` / `verify` 各タスクに chain 別で `--strict` を明示する選択肢。

却下理由: 設定とコマンドが散在したまま、chain × gate × 動詞のマトリクスを一覧できない。重複（`spec-signals`/`spec-states` 等）も残る。テンプレート利用者がポリシーを宣言的に調整する手段も無い。

### C: `signal` 名前空間を作らず既存 `verify` / `track` に per-chain 化だけ施す

`verify spec-states` の分割と `adr-signals` の strict 追加だけ行い、`signal` 名前空間への統一は見送る選択肢。

却下理由: §3-2 / §3-3 の束縛は解消されるが、§3-1 の組織的散在（calc=`track`, check=`verify`, render=散在）と重複は残る。動詞 × chain の直交性・一覧性が得られず、将来 chain や動詞が増えるたびに散らかりが再発する。

### D: config 不在時に commit gate を全 interim にする（fail-open フォールバック）

`signal-gates.json` が無いとき commit gate を全 chain interim にフォールバックする選択肢（導入コスト最小化・既存プロジェクト無影響を狙う）。

却下理由: 設定ファイルの消失・破損という異常時に gate が黙って permissive 側へ倒れる fail-open であり、Yellow が無警告で commit を通過して drift をそのまま再発させる。設定が欠けたときは安全側（block）に倒す（D4: 不在・不正は hard error）。「既存プロジェクト無影響」より「設定欠落が enforcement を弱めない」ことを優先する。

### E: config 不在時に silent な strict フォールバックを採る

`signal-gates.json` が無いとき error で止めず、全セルを `strict` として黙って動作させる選択肢（gate は安全側なので一応「動く」）。

却下理由: strict 自体は安全側だが、「config が無い」事実が表面化しないため、TDDD の commit が `commit_gate.impl_catalog` の緩和を得られないまま Yellow で止まる等、原因の掴めない失敗を生む。config は D6 で事実上必須であり、必須物の不在は silent に黙認せず hard error で修復を促すべき（D4）。fail-open（Alt D）よりは安全だが、actionability で error に劣る。

### F: `render` を `signal` 名前空間に押し込める

`signal render` / `signal render-<chain>` を追加し、calc / check と同じ `signal` 名前空間で描画も扱う選択肢。

却下理由: render は per-chain ではなく per-view / per-format の責務であり、spec.md frontmatter / plan.md / contract-map / 型グラフ / サマリのように複数 chain を集約する出力も多い。`check-*` の `--strict` / `--gate` と render の `--format` / 出力先 / chain filter はパラメータ形状も異なる。`signal` は gate pipeline の `calc` / `check` に限定し、描画は現状経路に残す。将来整理する場合も `signal` ではなく別の render/view 系コマンドファミリとして別 ADR で扱う。

### G: 全 4 chain に同じ persisted trait surface を強制する（3×4 trait scope）

⓪①②③ すべてに同一 trait で `calc` / `check` / freshness 系メソッドを要求し、4 chain × 共通操作の完全な trait surface として扱う選択肢。

却下理由: chain ⓪ は live 計算で永続ファイルを持たないため、永続 `calc` / `check_freshness` は構造的に不要である。単一 trait に押し込むと `unimplemented!()` 等の不正状態を作るか、意味のない no-op freshness を定義することになり、make illegal states unrepresentable に反する。D7 の `LiveSoTChain` / `PersistedSoTChain` 分割により、`calc-adr-user` の no-persist live calc は型化しつつ、永続 freshness を chain ⓪ に要求しない。

### H: per-layer ループ・SHA-256 を CLI コマンドや Makefile に保持する

per-layer 反復（`for LAYER in domain usecase infrastructure; do ... done`）と SHA-256 ハッシュ計算を CLI コマンドファイルまたは `Makefile.toml` タスクに残す選択肢（実装コストを最小化するために adapter 層にロジックを書く）。

却下理由: per-layer 列挙はビジネス知識（レイヤーセットは `architecture-rules.json` が定義する domain invariant）であり、adapter / 顧客カスタマイズ可能なサーフェス（Makefile・CLI コマンドファイル）に置くとヘキサゴナルアーキテクチャの境界（`knowledge/conventions/hexagonal-architecture.md`）に違反する。Makefile はビルド・実行の orchestration ツールであって、domain 知識を encode する場所ではない。同様に、CLI コマンドファイルが 30 LOC を超えてループやパス構築を含むと、adapter が use case を模倣した二重実装となり、D7 が防ごうとする「inline 重複実装による腐敗」を CLI 側で再発させる。adapter から per-layer ロジックを排除し usecase に集約することで、レイヤーセットの変更が 1 か所（domain の定数 / architecture-rules.json の読み取り箇所）で完結し、CLI・Makefile の変更不要で伝播する。

## Consequences

**良い影響**:

- 信号操作のゲートパイプライン（`calc` → `check`）が `signal` 名前空間の一覧可能な直交サーフェスに整理される
- commit gate で chain 別 strictness（① strict・③ interim 等）を独立制御できる
- chain ⓪ が merge gate で strict 評価され、`review_finding_ref` 止まりの ADR decision が `user_decision_ref` へ昇格しないまま merge されることを防げる
- chain × gate の strictness が 1 ファイルで把握・調整可能になる
- config の欠落・破損は hard error で即座に検出され、gate が黙って弱まることも黙って strict で動くこともない（最も actionable な fail-closed）
- テンプレート利用者が自プロジェクトのポリシーを宣言的に管理できる
- `ChainIdentity` / `SoTChain` / `LiveSoTChain` / `PersistedSoTChain` trait（D7）により新 chain 追加時の "うっかり忘れ" と inline 重複実装が構造的に防止される（no-persist live calc と persisted calc/freshness の取り違えはコンパイルエラー）

**悪い影響・トレードオフ**:

- **大規模な CLI サーフェス変更**になる。`Makefile.toml` / hooks / `track-active-gate` / `merge_gate.rs` / docs を一括更新する移行コストが生じ、後方互換を切る破壊的変更となる
- **config が必須化される**。`signal-gates.json` を持たない / 壊れた / 不完全なプロジェクトは gate が hard error で停止するため、推奨デフォルト（全 8 セルを明示した完全な config）の同梱が必須になる（特に TDDD は `commit_gate.impl_catalog: "interim"` が要る）。「既存プロジェクトへの影響ゼロ」ではない
- chain ① ② の commit gate が strict になることで、Yellow を持ったまま commit する従来のワークフローができなくなる
- `merge_gate.rs` に config 読み込み + chain ⓪ の repo-global な ADR ディレクトリ走査が加わり、merge gate の I/O 依存が増える
- chain ⓪ の merge 評価は repo-global のため、あるトラックの merge が「無関係な ADR の未エスカレート Yellow」で block されうる
- config ファイルの schema version を管理する責務が生じる
- 4 chain しか無いうちは `ChainIdentity` / `SoTChain` / `LiveSoTChain` / `PersistedSoTChain` trait（D7）が薄い抽象に見える可能性があるが、chain ② の 2 重 inline 実装と同型の腐敗、および no-persist chain へ永続 freshness を要求する取り違えを再発不能化する保険として割り切る

## Reassess When

- chain の種類が増えた場合（⓪ ① ② ③ 以外の新 chain が追加されたとき）
- `calc` / `check` の 2 動詞で表現できない信号操作が必要になったとき
- 描画（`render`）を `signal` とは別の render/view 系コマンドファミリとして整理する需要が顕在化したとき（現状は `track views sync` 等の別経路に委ねている）
- strictness が現在の 2 値（strict / interim）では表現できないというフィードバックが来たとき
- fail-closed の既定 strict が TDDD 以外のワークフローでも過剰に厳しいというフィードバックが蓄積したとき
- merge gate の strict モード判定を Rust の型システムで保証する（config を読まない設計に戻す）価値が生じたとき
- `knowledge/conventions/hexagonal-architecture.md` が adapter 層でのループ保持を許容するよう改訂されたとき（D8 の根拠が失われる）
- `signal` 名前空間に引数必須（argful）の動詞が追加され、呼び出し側がレイヤー名を渡さざるを得ない設計上の理由が生じたとき（D8-1 の「argless dispatch」前提が崩れる）

## Related ADRs

- [`2026-04-12-1200-strict-spec-signal-gate-v2.md`](../../knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md) — strict / interim 分離パターンの確立、domain 層への純粋シグナル評価関数の配置、`check_spec_doc_signals` の `strict: bool` 引数設計の原典。本 ADR はこのパターンを chain 単位に可変化する。
- [`2026-04-23-0344-catalogue-spec-signal-activation.md`](../../knowledge/adr/2026-04-23-0344-catalogue-spec-signal-activation.md) — SoT Chain ②（catalogue → spec signal 評価）の有効化。chain ② の CI 統合（interim モード）と merge gate 統合（strict モード）の設計を確立した。本 ADR はその strictness を config 駆動にする。
- [`2026-04-27-1234-adr-decision-traceability-lifecycle.md`](../../knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md) — chain ⓪（ADR 来歴信号）の原典。`user_decision_ref` / `review_finding_ref` / `grandfathered` による decision grounding 信号と `verify adr-signals` を定義した。本 ADR はその strictness を config 駆動にし、merge gate へ統合する。
- [`2026-06-16-0042-adr-signal-review-grounding-precedence.md`](../../knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md) — chain ⓪ の review-priority ルール（`review_finding_ref` があれば `user_decision_ref` 併存でも 🟡 に留める）。本 ADR の commit=interim / merge=strict 設計はこの「Yellow は escalate 待ちの一過性状態」という前提に立つ。
