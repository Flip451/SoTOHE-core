---
adr_id: 2026-06-18-1406-review-prompts-relocation-per-layer-briefings
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_01MFJugxYUs63u163aNbtBYt:2026-06-18"
    candidate_selection: "from:[custom-review-prompts, briefings, config-review-prompts, prompts, track-review-prompts] chose:custom-review-prompts"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_01MFJugxYUs63u163aNbtBYt:2026-06-18"
    candidate_selection: "from:[config-move, custom-move, repoint-only] chose:config-move"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session_01MFJugxYUs63u163aNbtBYt:2026-06-18"
    candidate_selection: "from:[code5, code5+harness-policy, libs3] chose:code5+harness-policy"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session_01MFJugxYUs63u163aNbtBYt:2026-06-18"
    candidate_selection: "from:[plan-artifacts-shape, layer-views-only, minimal-stub] chose:plan-artifacts-shape"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session_01MFJugxYUs63u163aNbtBYt:2026-06-18"
    candidate_selection: "from:[clean-move, keep-old-path-fallback] chose:clean-move"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session_01MFJugxYUs63u163aNbtBYt:2026-06-18"
    candidate_selection: "from:[inplace-default, samples-copy] chose:inplace-default"
    status: proposed
---
# レイヤー別 reviewer briefing prompt の導入と review-prompts ディレクトリの再配置

## Context

### §1 現状: briefing の scope 間非対称性

`/track:review` は `track/review-scope.json` の `groups` 定義に従って scope（`domain` / `usecase` / `infrastructure` / `cli` / `cli_composition` / `plan-artifacts` / `harness-policy`）ごとに reviewer を起動する。各 scope の reviewer に「どの観点で findings を選別させるか」は、`groups.<scope>.briefing_file` が指す severity policy md（= review-prompts）で制御できる。

しかし現状この `briefing_file` を持つのは **`plan-artifacts` ただ 1 つ**（`track/review-prompts/plan-artifacts.md`）であり、code 5 層（`domain` / `usecase` / `infrastructure` / `cli` / `cli_composition`）と `harness-policy` の reviewer は scope 固有の severity policy を持たない。結果としてこれらの reviewer は層に依らないグローバル固定の観点でしか findings を選別できず、「domain には型安全 / 不変条件 / typestate を、infrastructure には adapter / I-O 境界を効かせたい」といった層別の review 知見を機械化できていない。この `briefing_file` 機構は `knowledge/adr/2026-04-18-1354-review-scope-prompt-injection.md` で導入されたが、その Rollout Plan Phase 4（他 scope への briefing 展開）は未実施のまま残っている。

### §2 briefing_file の注入機構（実装検証済み）

<!-- illustrative, non-canonical -->
reviewer の prompt は **2 段階の「Read パス参照」注入**で組み立てられる（`apps/cli-composition/src/review_v2/`）:

1. **base prompt**（`helpers.rs::build_base_prompt_from_input`）: `--briefing-file <path>` が渡されると base は `Read {path} and perform the task described there.` という**パス参照**になる（内容を inline しない）。
2. **scope severity policy 注入**（`briefing.rs::append_scope_briefing_reference_str`）: `review-scope.json` の `briefing_file_for_scope(scope)` を引き、安全なパスなら base prompt に `## Scope-specific severity policy` 節を追記し、`- {briefing_path}` を「必ず先に Read せよ」と指示する。

つまり `briefing_file`（= review-prompts）は **reviewer briefing の中に「これを先に Read せよ」というパス参照として注入され、reviewer 自身が Read する severity-policy 断片**である。`briefing_file` の値は domain / loader 側で canonicalize されず、特定ディレクトリ配下に縛る制約もない（注入時に `is_safe_briefing_path` が文字種だけ検査し、loader は symlink / trusted_root 違反を fail-closed で拒否する）。したがって配置の選択は **runtime 挙動に影響せず、執筆時の凝集・概念的な所属・配布時の所有境界だけに効く**。

### §3 現配置と .harness/ の住所体系

review-prompts は現在 `track/review-prompts/`（`plan-artifacts.md` のみ）に暫定配置され、`track/review-scope.json` がそれを指す。一方 `.harness/` は既に役割別の住所体系を持つ:

<!-- illustrative, non-canonical -->
- `.harness/config/` — capability 配線（`agent-profiles.json` の provider / model wiring）と framework の描画/検査 config（`*-style.toml` / `dry-check.json`）。`samples/` に template 提供のデフォルト agent-profiles を同梱。
- `.harness/briefings/` — reviewer が読む framework 運用 briefing（`review-fix-lead-codex.md`）
- `.harness/prompts/` — `agent-profiles.json` の `prompt_template_path` で wiring され、コードが `{{var}}` 置換して単発 LLM 呼び出しに渡すテンプレート（`ref-verifier-chain1.md` / `ref-verifier-chain2.md`）
- `.harness/capabilities/` — named specialist の framework 運用 briefing（`spec-designer.md` ほか）

review-prompts はこの体系のどこに属するべきかが定まっておらず、`track/` という track ライフサイクル成果物の住所に間借りしている。

### §4 テンプレート配布と所有境界

SoTOHE は template / framework として配布される（`knowledge/conventions/responsibility-boundary.md`）。CI で enforce してよいのは framework 自身の方法論・コード整合性に限られ、**利用者がカスタマイズする設定（provider / agent 設定、利用者の domain コード）は「提供 + docs」に留め CI 強制しない**。`.harness/config/samples/`（`agent-profiles.*.json`）はこの「デフォルト同梱」モデルの実例である。

per-layer review-prompts は「どの観点で各層を review させるか」という **利用者の review 方針**であり、利用者の架構・conventions・優先度に依存する利用者所有領域に当たる。実際 `architecture-customizer` は `review-scope.json` も review-prompts も touch しない（層をリネームしても review 設定は追従しない）。したがって review-prompts は framework methodology（`review-fix-lead-codex.md` 等）とは所有クラスが異なり、両者を同一ディレクトリに混ぜると、テンプレート配布時の「framework は更新、利用者カスタムは温存」という分離が効かなくなる。これを §2 で確認した「配置は runtime 非依存」という性質と合わせ、review-prompts を利用者所有ゾーンに分離して再配置し、併せて未整備の各層 briefing を新設するのが本 ADR の主題である。

## Decision

### D1: review-prompts を `.harness/custom/review-prompts/` に再配置する

§4 の通り review-prompts は利用者所有領域であり、framework methodology から物理的に分離するため、新設する利用者所有ゾーン `.harness/custom/`（D6）の配下 `review-prompts/` に置く。`track/review-prompts/*.md` を `.harness/custom/review-prompts/*.md` へ移し（`plan-artifacts.md` を移設し、D3 の各層 briefing をここに新設する）。§2 の通り配置は runtime 非依存なので reviewer 動作は不変。

### D2: review-scope.json を `.harness/config/` に移設し、briefing_file を repoint する

scope 配線 `review-scope.json` は構造化 JSON wiring で loader を持ち、`agent-profiles.json` と同類である。よって利用者所有ゾーン `custom/` ではなく、`agent-profiles.json` と並べて `.harness/config/` に置き、「review / 実行系 capability configure の SSoT」を揃える（デフォルト提供が要れば後で `samples/` 方式を足せる）。各 `briefing_file` 値は `.harness/custom/review-prompts/<scope>.md` に書き換える（workspace-relative path 文字列のまま維持し、loader の解決規則は変更しない）。これに伴い:

<!-- illustrative, non-canonical -->
- `apps/cli-composition/src/review_v2/scope.rs` の `root.join("track/review-scope.json")` ハードコードを `.harness/config/review-scope.json` に更新する。
- `harness-policy` scope の `patterns` にある明示の `track/review-scope.json` エントリは、移設先 `.harness/config/review-scope.json` が既存の `.harness/**` パターンで自動的に harness-policy に分類されるため削除する（冗長解消）。

最終レイアウト:

<!-- illustrative, non-canonical -->
```
.harness/
├── custom/                         # 利用者所有ゾーン (D6; template 更新で never-clobber)
│   └── review-prompts/
│       ├── plan-artifacts.md       (track/review-prompts/ から移設)
│       ├── domain.md
│       ├── usecase.md
│       ├── infrastructure.md
│       ├── cli.md
│       ├── cli_composition.md
│       └── harness-policy.md
├── briefings/                      # framework methodology (template 維持)
│   └── review-fix-lead-codex.md
├── prompts/ / capabilities/        # framework methodology
└── config/
    ├── agent-profiles.json         (利用者所有; samples/ あり)
    ├── review-scope.json           (track/ から移設; briefing_file → .harness/custom/review-prompts/<scope>.md)
    ├── *-style.toml / dry-check.json
    └── samples/
```

### D3: briefing 新設範囲を code 5 層 + harness-policy とする

`domain` / `usecase` / `infrastructure` / `cli` / `cli_composition` の各アーキテクチャ層に severity policy briefing を 1 枚ずつ新設し、`harness-policy` scope にも 1 枚新設する（`plan-artifacts` は既存を移設）。これにより `review-scope.json` の全 named scope が `briefing_file` を持つ。

### D4: 各 briefing は plan-artifacts 同形 + 層固有観点で書く

既存 `plan-artifacts.md` の「What to report / What NOT to report」構成を踏襲し、各層の severity 観点を盛り込む（例: `domain` = 型安全 / 不変条件 / enum-first / typestate / no-panics、`infrastructure` = adapter rules / I-O 境界 / serde codec、`cli` = CLI→usecase 経由強制 / domain 直参照禁止 など）。各層の着眼点のうち review で効かせたい境界を、reviewer が「報告する / 無視する」を判断できる粒度で明文化する。convention 本文との重複は層固有観点に絞って最小化する。

### D5: clean move（旧 path fallback なし）で移行する

旧 path（`track/review-scope.json` / `track/review-prompts/`）への fallback は設けず、移行 commit で全参照（review-scope.json 値 / scope.rs ハードコード path / harness-policy patterns / docs / tests）を同時更新し、空になった `track/review-prompts/` を削除する。これは fail-closed / lenient path 排除の既存方針に沿う。ただし `knowledge/conventions/adr.md` §Lifecycle の post-merge 不変性に従い、過去に merge 済みの ADR 本文中の旧 path 参照は historical record として改変せず、更新対象から除外する。

### D6: `.harness/custom/` を利用者所有ゾーンとして確立する

`.harness/custom/**` を「利用者が所有しカスタマイズする」ゾーンとして新設する。`responsibility-boundary.md` に従い:

- SoTOHE は dogfood 済みのデフォルト（自身の層に対する severity policy）を `.harness/custom/` に**直接同梱**する（利用者は in-place で編集する。reviewer は初期からデフォルト policy を得る）。
- テンプレート配布更新では `custom/**` を**上書きしない**（never-clobber。利用者のカスタムが温存される）。
- `custom/` 配下の内容を CI で hard-fail enforce しない（提供 + docs のみ。利用者所有領域）。`briefing_file` の参照先存在チェックもこの CI 非強制境界に含め、broken path は既存通り reviewer runtime の Read 失敗に委ねる。

## Rejected Alternatives

### A. review-prompts を `.harness/briefings/` に置く

reviewer が Read する briefing 断片という機構一致から `briefings/` に同居させる案（本 ADR 検討初期に採用しかけた）。却下理由: `.harness/briefings/` は framework methodology（`review-fix-lead-codex.md` 等）の住所であり、利用者所有の severity policy を混ぜると、テンプレート配布時の framework / 利用者カスタムの分離（never-clobber 境界）が効かなくなる（§4）。

### B. review-prompts を `.harness/config/review-prompts/` に置く

`review-scope.json` と同居させ「scope 配線 + scope prompt」の凝集を優先する案（前 ADR の Future Migration 原案）。却下理由: review-prompts は利用者所有の自由文 policy であり、config（構造化 wiring）扱いは性質と合わない。利用者所有ゾーン `custom/` に分離する方が配布境界が明確。

### C. review-prompts を `.harness/prompts/` 配下に置く

却下理由: `.harness/prompts/` は `agent-profiles.json` の `prompt_template_path` で wiring され、コードが `{{var}}` を置換して単発 LLM 呼び出しに渡す機械消費テンプレート専用ゾーンである。review-prompts は置換されず、wiring 経路も別（`review-scope.json`）。機構が一致しない。

### D. `track/review-prompts/` を維持する（移動しない）

却下理由: `track/` は track ライフサイクル成果物の住所であり、track 横断的かつ利用者所有の reviewer 設定の住所として不適。利用者所有ゾーンへ分離する意義がある。

### E. review-scope.json も `.harness/custom/` に入れる

利用者所有領域を `custom/**` に一括し never-clobber の境界を単一プレフィックスに揃える案。却下理由: `review-scope.json` は loader を持つ構造化 JSON wiring で `agent-profiles.json` と同類であり、`config/`（+ 必要なら `samples/`）に置く既存 precedent に揃える方が一貫する。また scope patterns は SoTOHE の architecture に結合するため、never-clobber 一辺倒より template 側更新を取り込める余地を残す方が良い。

### F. review-scope.json は移さず briefing_file 値だけ repoint する

却下理由: `.harness/` に寄せるなら配線（`review-scope.json`）も `agent-profiles.json` と並べて config SSoT 化する方が概念的に揃う。配線を `track/` に残す案は中途半端。

### G. デフォルトを `samples/` 方式で配布する

per-layer デフォルトを `samples/` に置き、利用者が `custom/` にコピーして起こす案（agent-profiles 方式）。却下理由: 未コピー時は layer policy なしで reviewer が回る。review-prompts は安定で clobber リスクが低く、dogfood 済みを直接同梱する方が初期体験が良い（D6）。

### H. 各 briefing を最小 stub + convention 参照のみにする

却下理由: `plan-artifacts.md` の実績ある「What to report / What NOT to report」構成が reviewer の severity 選別に直接効く。convention への参照だけでは「何を報告し何を無視するか」の境界が reviewer に伝わりにくい。重複は層固有観点に絞って最小化する。

### I. 旧 path への fallback を残す

却下理由: fail-closed / lenient path 排除方針。新旧二重 path の複雑性と腐敗リスクを抱える。clean move で一括更新する方が健全。

## Consequences

### Positive

- **レイヤー別 severity の機械化**: 各 reviewer がレイヤー固有観点で findings を選別でき、briefing の scope 間非対称性が解消する。
- **所有境界の明確化**: framework methodology（`briefings/` / `prompts/` / `capabilities/`）と利用者所有（`custom/`）がディレクトリで自明になり、テンプレート配布時に framework 更新と利用者の review カスタムが衝突しない（`custom/**` never-clobber）。
- **`.harness/` の役割別整理**: `config/`（配線）/ `custom/`（利用者所有）/ `briefings`・`prompts`・`capabilities`（framework methodology）の三分で住所が整理され、`review-scope.json` は `agent-profiles.json` と並ぶ SSoT に揃う。
- **手動注入の再現性**: feedback memory に積もる各層の review 観点が config 化され、会話セッション越しの手動再入力が不要になる（前 ADR の動機を全層へ展開）。

### Negative / Risk

- **briefing 腐敗リスク増**: briefing が 1 → 7 枚に増え、code が変わっても briefing が古いまま残る腐敗が起きやすい。CI lint を入れないため、broken path は runtime の Read 失敗まで検出されない（Open Questions 参照）。
- **移行の touch 範囲が広い**: review-scope.json 値 / scope.rs ハードコード path / harness-policy patterns / docs 2 件 / tests / 旧 dir 削除を同時更新する。clean move ゆえ移行 commit のミスは即 reviewer 注入を壊す。
- **custom/ デフォルトの陳腐化**: in-place 同梱 + never-clobber ゆえ、SoTOHE が後から改善した層別 severity 観点を利用者が自動では取り込めない（利用者所有の代償）。
- **harness-policy の自己言及**: `harness-policy` scope に briefing を付けると、harness-policy 自身の review 対象（`.harness/**`）に自分の severity policy md が含まれる自己言及構造になる。

### Neutral

- **schema 変更なし**: `review-scope.json` の `version` は据え置き。`briefing_file` は既存の optional field であり、path 値の変更と各 scope への追加のみ。
- **runtime 挙動は不変**: 注入されるのは Read パス参照 1 本で配置非依存。安全なパスである限り reviewer の動作は移行前後で同じ。

## Reassess When

- **broken path の実害発生**: `briefing_file` の broken path で review が実際に壊れたら、見送った CI lint（参照先存在チェック）を本実装に昇格する。
- **`.harness/` 構造方針の変更**: `config/` / `custom/` / `briefings/` の境界再編、`prompts` / `briefings` / `capabilities` の統廃合など `.harness/` の住所方針が変わったら、review-prompts の配置を再評価する。
- **prompt 注入機構の変更**: reviewer の Read パス参照方式をやめて content inline 化する等、注入機構が変わったら「配置非依存」の前提が崩れるので再検討する。
- **briefing 重複の肥大化**: scope 固有 briefing が増えて重複が肥大化したら、共通 preamble の抽出や `briefing_files: [...]` 複数指定への拡張を検討する。
- **architecture-customizer の層リネーム対応**: `architecture-customizer` が層をリネーム / 再構成する際、`custom/review-prompts/<scope>.md` と `review-scope.json` の patterns の追従を組み込むかを検討する（現状は未連動）。

## Open Questions

- **briefing_file 参照先存在の CI lint**: briefing が 7 枚に増えることで broken path リスクが上がるが、本 ADR では lint を decision 化せず、注入時 Read 失敗に委ねる既存方針を維持する（前 ADR Open Question Q3 を引き継ぐ）。Reassess When の「broken path の実害発生」が満たされた時点で昇格を検討する。
- **`custom/**` never-clobber の配布機構**: テンプレート配布更新時に `custom/**` を温存する具体機構（merge tool / docs convention / `.gitattributes` 等）は本 ADR では未確定。`custom/` ゾーンの確立（D6）を先行し、配布機構は別途詰める。

## Related

- `knowledge/adr/2026-04-18-1354-review-scope-prompt-injection.md` — `briefing_file` 機構の起点。本 ADR はその Rollout Phase 4（各 scope への briefing 展開）の実現にあたる。
- `knowledge/conventions/responsibility-boundary.md` — framework enforce 領域と利用者所有領域の分界。`.harness/custom/`（D6）の根拠。
- `knowledge/conventions/coding-principles.md` / `knowledge/conventions/hexagonal-architecture.md` / `knowledge/conventions/prefer-type-safe-abstractions.md` — 各層 briefing が参照する severity 観点の原典。
- `knowledge/adr/README.md` — ADR 索引。
