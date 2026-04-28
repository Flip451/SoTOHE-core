---
adr_id: 2026-04-15-1012-catalogue-active-guard-fix
decisions:
  - id: 2026-04-15-1012-catalogue-active-guard-fix_grandfathered
    status: accepted
    grandfathered: true
---
# Catalogue rendering pipeline: active-track guard + source-file-name fix + multi-layer sync

## Status

Accepted

## Context

catalogue rendering pipeline に以下 3 つの構造的バグが同時に存在することが観測された。再現コマンド:

```
bin/sotp track type-signals <done-track-id> --layer infrastructure
```

このコマンドは既に `status=done` 状態の過去トラックの `<layer>-types.json` と `<layer>-types.md` を意図せず書き換え、ヘッダが `<!-- Generated from infrastructure-types.json -->` → `<!-- Generated from domain-types.json -->` に drift した。

### バグ 1: `execute_type_signals` に active-track guard が存在しない

`apps/cli/src/commands/track/tddd/signals.rs::execute_type_signals` は `TrackId::try_new` のパス走査チェックのみで、`metadata.json.status` が `Done` / `Archived` でも catalogue の write を許してしまう。merged track のデータ不変性が侵害される。

### バグ 2: `render_type_catalogue` のヘッダが hardcode

`libs/infrastructure/src/type_catalogue_render.rs` が `"Generated from domain-types.json"` で固定されており、`doc` に layer 情報が含まれていないため、infrastructure / usecase layer の rendered view でも常に `"domain-types.json"` と表示される。

### バグ 3: `sync_rendered_views` が domain-only

`libs/infrastructure/src/track/render.rs::sync_rendered_views` は `track/items/<id>/domain-types.json` → `domain-types.md` のみを対象に生成する設計 (tddd-01 時代の実装のまま)。TDDD 多層化 (tddd-02 で usecase 層、domain-serde-ripout で infrastructure 層が opt-in) に update が追従しておらず、`cargo make track-sync-views` を実行しても `usecase-types.md` / `infrastructure-types.md` は更新されない。`sotp track type-signals` の write 経路でのみ refresh される。

### 非対称性

バグ 1 の non-trivial な点は、同じ catalogue を write する別経路である `sync_rendered_views` (`libs/infrastructure/src/track/render.rs:573`) は既に `is_done_or_archived` guard (文字列ベース実装) を持つことである。同じ write path の 2 経路のうち 1 つしか保護されていない。`execute_type_signals` は「on-demand 評価ツール」として独立設計された結果、lifecycle guard を持たずに merged track に到達する。

### SKILL.md SSoT drift (D4 の動機)

本件調査中に `.claude/skills/track-plan/SKILL.md` line 165-167 + line 283-285 で feedback signal を Blue に分類する古い記述が発見された。`knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` で feedback は Blue → Yellow に降格され、`knowledge/conventions/source-attribution.md` (SSoT) は更新済だが SKILL.md の更新が漏れており、source tag の意味論が 2 箇所で矛盾している。`/track:plan` skill を使う度に同種の誤認識が再発する構造的欠陥であり、catalogue rendering pipeline のバグと同じ pipeline (spec/plan 作成時の誤情報源) として 1 つの ADR で併記する。

## Decision

### D1: `execute_type_signals` に status-based fail-closed guard を追加する

`apps/cli/src/commands/track/tddd/signals.rs::execute_type_signals` の先頭 (`TrackId::try_new` 検証直後、`resolve_layers` 呼び出し前) に以下の guard を追加する:

```rust
let (metadata, _) = infrastructure::track::fs_store::read_track_metadata(&items_dir, &track_id)
    .map_err(|e| CliError::Message(format!("cannot load metadata for '{track_id}': {e}")))?;

use domain::track::TrackStatus;
match metadata.status() {
    TrackStatus::Done | TrackStatus::Archived => {
        return Err(CliError::Message(format!(
            "cannot run type-signals on '{track_id}' (status={}). \
             Completed tracks are frozen — run on an active track instead.",
            metadata.status()
        )));
    }
    TrackStatus::Planned
    | TrackStatus::InProgress
    | TrackStatus::Blocked
    | TrackStatus::Cancelled => {
        // active / pending — proceed with type-signals evaluation
    }
}
```

判断根拠:

1. **`sync_rendered_views` との対称性**: 同じ catalogue file + rendered view を触る別経路 (`render.rs:573`) は既に `is_done_or_archived` guard を持つ。signals.rs 側にも同じ guard を置くことで、merged track のデータ不変性を複数経路から保護する。
2. **fail-closed**: guard 発動時は明示的な `CliError::Message` で reject し、silent skip/warning には留めない。merged track への write は明確に誤った操作であり、silent fail は bug を隠蔽する。
3. **layer 依存方向**: guard ロジックは `apps/cli` 層に置き、`infrastructure::track` の metadata loader を呼ぶ。domain は touch しない (domain → infrastructure の依存は禁止のまま)。
4. **exhaustive match で型安全な compile-time check**: `matches!` マクロは内部的に `_ => false` を暗黙展開するため非網羅的で、新 variant を silent に通過させる (fail-open)。exhaustive `match` 文は `_` を持たず `TrackStatus` の 6 variants (`Planned` / `InProgress` / `Done` / `Blocked` / `Cancelled` / `Archived`) 全てを明示列挙するため、新 variant 追加時に compile error が発生し、開発者は「frozen 扱い (reject arm)」と「active 扱い (proceed arm)」のどちらに分類するか明示的に選択せざるを得ない。これは fail-closed を維持する structural guarantee。

### D2: `render_type_catalogue` の signature を `(doc, source_file_name: &str)` に変更する

`libs/infrastructure/src/type_catalogue_render.rs::render_type_catalogue` を以下に変更する:

```rust
pub fn render_type_catalogue(
    doc: &TypeCatalogueDocument,
    source_file_name: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<!-- Generated from {source_file_name} — DO NOT EDIT DIRECTLY -->\n"
    ));
    // ... 以降の section rendering は変更なし
}
```

呼び出し側を更新する (D3 の multi-layer loop と連動):

- `apps/cli/src/commands/track/tddd/signals.rs::validate_and_write_catalogue`: catalogue JSON ファイル名を `domain_types_path.file_name()` から derive して渡す (既存の `evaluate_and_write_signals` と同じ pattern)。`binding` はこの scope に存在せず、`rendered_file_stem` は rendered `.md` path なので source_file_name には使わない
- `libs/infrastructure/src/track/render.rs::sync_rendered_views` 内 (D3 の multi-layer loop 内): `render_type_catalogue(doc, binding.catalogue_file())`

判断根拠:

1. **Single Source of Truth**: 複数 layer で共通に使われる renderer のヘッダは caller が source file 名を知っているべき (layer-aware 情報は呼び出し側)。`"domain-types.json"` hardcode は layer 中立でなく SSoT 原則に反する。
2. **Backward compat 不要**: workspace 内の呼び出し側は 2 箇所のみで一括修正可能 (`rg 'render_type_catalogue\('` で確認済)。breaking change のコストは limited。
3. **D3 との密結合**: signature 引数化と sync_rendered_views の multi-layer 化は同じ PR で完結させる必要がある (B4 参照)。

既存 test 2 件 (`type_catalogue_render.rs:211`, `render.rs:1965`) を signature 変更に追従させ、新規 test (non-domain source file 名) を追加する。

### D3: `sync_rendered_views` を multi-layer 対応に拡張する

`libs/infrastructure/src/track/render.rs::sync_rendered_views` の domain-types.md 専用ブロック (line 568-606) を `architecture-rules.json` の `tddd.enabled=true` 全 layer を iterate するループに置き換える。既存 public resolver `libs/infrastructure/src/verify/tddd_layers.rs::parse_tddd_layers` を直接 reuse する — 新 helper は不要:

```rust
use crate::verify::tddd_layers::parse_tddd_layers;

let arch_rules_path = root.join("architecture-rules.json");
let bindings = match std::fs::read_to_string(&arch_rules_path) {
    Ok(json) => parse_tddd_layers(&json).map_err(|e| {
        RenderError::Io(std::io::Error::other(format!("architecture-rules.json: {e}")))
    })?,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
        // Legacy fallback: synthetic domain binding
        parse_tddd_layers(
            r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#,
        )
        .map_err(|e| RenderError::Io(std::io::Error::other(format!("builtin fallback: {e}"))))?
    }
    Err(e) => return Err(RenderError::Io(e)),
};

for binding in &bindings {
    let catalogue_file = binding.catalogue_file();
    let catalogue_path = track_dir.join(catalogue_file);
    if is_done_or_archived || !catalogue_path.is_file() {
        continue;
    }
    let content = std::fs::read_to_string(&catalogue_path)?;
    match catalogue_codec::decode(&content) {
        Ok(doc) => {
            let rendered = type_catalogue_render::render_type_catalogue(&doc, catalogue_file);
            let rendered_md_path = track_dir.join(binding.rendered_file());
            let old_md = match std::fs::read_to_string(&rendered_md_path) {
                Ok(c) => Some(c),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(RenderError::Io(e)),
            };
            if old_md.as_deref().is_none_or(|existing| !rendered_matches(existing, &rendered)) {
                atomic_write_file(&rendered_md_path, rendered.as_bytes())?;
                changed.push(rendered_md_path);
            }
        }
        Err(catalogue_codec::TypeCatalogueCodecError::Json(_)) => {
            eprintln!(
                "warning: skipping {} render for {} (malformed JSON)",
                binding.rendered_file(),
                track_dir.display()
            );
        }
        Err(e) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "{} error at {}: {e}",
                catalogue_file,
                track_dir.display()
            ))));
        }
    }
}
```

判断根拠:

1. **`track-sync-views` の完全性**: `track-sync-views` は「track state を rendered view に反映する single 統合 sync point」として設計されたが、domain-types.md だけを扱っていたため multi-layer track で partial sync になっていた。本修正で full sync point として機能する。
2. **既存 3 pattern の保持**: `is_done_or_archived` guard、`rendered_matches` drift check、`TypeCatalogueCodecError::Json` warn-and-continue の 3 pattern を loop 内の各 layer に個別適用する。既存 domain-types.md 経路の挙動は完全互換。
3. **per-layer opt-out の尊重**: `catalogue_path.is_file()` check で layer catalogue が不在なら skip。`knowledge/adr/2026-04-14-1531-domain-serde-ripout.md` §D8 で確立された「catalogue file を作らない opt-out」pattern を壊さない。
4. **既存 `parse_tddd_layers` の reuse**: `libs/infrastructure/src/verify/tddd_layers.rs` に tddd-01 Phase 1 Task 7 で導入された `pub struct TdddLayerBinding` (line 29) + `pub fn parse_tddd_layers` (line 139) が既存。`apps/cli/src/commands/track/tddd/signals.rs:15` は既に `use infrastructure::verify::tddd_layers::{TdddLayerBinding, parse_tddd_layers};` で import しており、`resolve_layers` (line 28) はこの resolver を呼ぶ薄い wrapper。`apps/cli → infrastructure` は正しい依存方向であり、signals.rs の既存 reuse がそれを示す。render.rs の拡張は `use crate::verify::tddd_layers::parse_tddd_layers;` 1 行 + 数行の呼び出しで完結する。
5. **test coverage**: 既存 `sync_rendered_views_generates_domain_types_md_from_domain_types_json` を維持 (backward compat)。`sync_rendered_views_generates_usecase_types_md_from_usecase_types_json` と `sync_rendered_views_generates_infrastructure_types_md_from_infrastructure_types_json`、および「複数 catalogue file を持つ multi-layer case」test を追加し、loop の独立性を保証する。
6. **atomic な D2 + D3**: D2 で `render_type_catalogue` signature を引数化した時点で、sync_rendered_views 内の呼び出しが引数を渡す必要がありどのみち render.rs を touch する。同じタイミングで loop 化まで進めるのが自然。

### D4: `.claude/skills/track-plan/SKILL.md` の 2 箇所を source-attribution SSoT に整合させる

> Note: 「ADR Accepted 前に実装作業を始めない」という運用ルール (ADR-first gate) は process constraint であり architectural decision ではない。このような運用制約は spec 側 (`spec.json::constraints`) で記録し、ADR には含めない (architectural decision と process constraint の分離原則)。

- **line 165-167** (classification table): feedback を Blue 行から削除し Yellow 行に移動
  - 修正前: `| 🔵 確定済み | 最高信頼度の source が document / feedback / convention → Blue signal | 不要（スキップ） |`
  - 修正後: `| 🔵 確定済み | 最高信頼度の source が document / convention → Blue signal | 不要（スキップ） |`
  - Yellow 行追加: `| 🟡 要確認 | 最高信頼度の source が feedback / inference / discussion → Yellow signal | 確認を推奨 |`
- **line 283-285** (diff hearing update guidance): feedback → Blue 昇格 の記述を修正
  - 修正前: 「`feedback — {内容}` を追加（→ Blue に昇格）」
  - 修正後: 「`feedback — {内容}` を追加（Yellow のまま保持。Blue 昇格には ADR/convention への永続化が必要 — source-attribution.md §Upgrading Yellow to Blue 参照）」

判断根拠:

1. **SSoT との整合**: `knowledge/conventions/source-attribution.md` が source tag signal の SSoT (§Source Tag Types Table で feedback=Yellow 明記)。SKILL.md は 2026-04-12 `strict-signal-gate-v2` ADR 実施時の update 漏れ。
2. **Structural fix**: この修正を行わない限り、将来 `/track:plan` skill を呼ぶ度に同種の誤認識が起きうる。catalogue rendering pipeline のバグ (D1-D3) と同じ pipeline (spec/plan 作成時の誤情報源) として 1 つの ADR にまとめる。
3. **歴史 spec は触らない**: 過去の merged/done track 配下にも古記述が残っているが、当時の判断の歴史記録として残す (git blame の尊重)。

## Rejected Alternatives

### B1: `--read-only` / `--dry-run` フラグを `sotp track type-signals` に追加する (Fix B)

過去 track の signal を再計算したい use case に対応するため、`--read-only` flag で signals 計算のみ行い catalogue write を skip する案。

**却下理由**:

1. **YAGNI**: merged track の signals は strict merge gate (`verify-spec-states`) により必然的に全 blue で main に入る。再計算しても情報獲得にならない。
2. **既に rendered view に永続化**: 計算結果は `<layer>-types.md` の Signal 列に既に埋め込まれ git history に残っている。view を直接読めば現状の signals が分かるため、再実行の必要性自体が存在しない。

### B2: current git branch と `track.branch` の一致を強制する guard (Fix C)

active track の定義として「現在の git branch が `track.branch` と一致する」という条件を D1 の status guard に追加する案。

**却下理由**:

1. **D1 で core protection が完結**: root cause は「done/archived track への write が許される」ことで、D1 の status guard がこれを完全に拒否する。branch match は defense-in-depth に過ぎず、新しい protection value は active track 同士の cross-track 誤操作のみ。
2. **他 track 状態確認は artifact 直読で代替可能**: 過去 track の signals / catalogue を見たい場合、`cat track/items/<id>/<layer>-types.md` 等で rendered artifact を直接読めば十分 (strict merge gate で merged track は全 blue 確定、signals は rendered view に永続化済)。branch match guard を導入しても "他 track 状態確認" workflow は阻害されない — 判断は純粋な cost/benefit に帰着する。
3. **Over-engineering の回避**: active track 同士の cross-track 誤操作は現実的には rare で、`cargo make track-branch-switch` の明示運用で回避可能。rare case の defense-in-depth のための metadata load + branch field 読取 + 比較の複雑度は割に合わない。

### B3: hardcoded header を `#[cfg(test)]` assertion で detect する (D2 代替)

test 側で hardcoded header を検出する assertion を追加する案。

**却下理由**:

1. **対症療法**: hardcoded が何故生まれたか (API design の欠陥) を根本修正しない。
2. **signature 変更の方が cheaper**: 2 箇所の呼び出し側を更新するだけで終わる小さい変更。
3. **再発しない保証**: signature 変更後はコンパイル時に call site が強制的に引数を渡す必要があり、regression が structurally 不可能。

### B4: `sync_rendered_views` の multi-layer 対応を別 ADR に分離する (D3 代替)

D1 + D2 のみを scope とし、`sync_rendered_views` の multi-layer 化は別 ADR で扱う案。

**却下理由**:

1. **D2 との密結合**: `render_type_catalogue` signature を引数化した時点で、呼び出し側 (sync_rendered_views) の `"domain-types.json"` hardcoded 回避が自然に必要になる。別 ADR にすると signature 変更後に一時的な `render_type_catalogue(doc, "domain-types.json")` hardcode が残る中途半端な状態が生じる。
2. **domain-only 挙動自体がバグ**: 「他 layer を render しない」のは設計ミス/追従漏れで、D1 / D2 と同じく catalogue rendering pipeline の構造的欠陥。1 つの theme として扱うべき。
3. **render pipeline の atomicity**: D1-D3 は catalogue rendering pipeline の 3 つの構造的 gap であり、別々に fix すると pipeline の挙動が中途半端に変わる。

### B5: SKILL.md 修正 (D4) を別 ADR に分離する

D4 の SKILL.md structural fix を別 ADR で処理する案。

**却下理由**:

1. **因果関係の明確化**: SKILL.md 古記述は catalogue rendering pipeline のバグ調査過程で発見された同じ pipeline (spec/plan 作成時の誤情報源) の SSoT drift で、同一 ADR に含めることで修正の必然性が reader に明示される。
2. **small diff**: 修正は 1 箇所あたり数行の cosmetic diff であり、別 ADR のオーバーヘッドに見合わない。

## Consequences

### Good

- **データ不変性の回復**: `status=done/archived` track の catalogue ファイルが `sotp track type-signals` 経由で書き換えられる経路が塞がれる。`sync_rendered_views` と `execute_type_signals` の protection level が揃う。
- **rendering の帰属情報整合**: rendered view のヘッダが実際の source file 名を反映するようになり、読み手の混乱が解消される。
- **`track-sync-views` の完全性**: multi-layer rendered view の sync が `cargo make track-sync-views` で一括処理される。`usecase-types.md` / `infrastructure-types.md` の drift 経路が解消される。
- **SSoT との整合**: `.claude/skills/track-plan/SKILL.md` が `source-attribution.md` と一致し、`/track:plan` 時の feedback=Blue 誤認識が再発しない。

### Bad

- **breaking change**: `render_type_catalogue` の signature 変更で既存呼び出し側 (2 箇所) の一括修正が必要。workspace 内なので影響範囲は限定的だが、将来このレンダラーを外部から呼ぶ場合は同じ breaking change が再度必要になる。
- **sync_rendered_views の処理コスト増**: 各 layer の catalogue file を decode + render する iteration が必要。現 workspace (3 layer) では追加コスト数 ms 程度で実用上の impact は無視できる。layer 数が 10+ に増えた場合は parallel iter 化を検討する (Reassess When §2)。

### Neutral

- 既存 `libs/infrastructure/src/track/render.rs:505` の `is_done_or_archived = matches!(parsed.status.as_str(), "done" | "archived")` は文字列ベース非網羅的実装。D1 では signals.rs 側の guard のみ exhaustive `match` on `TrackStatus` enum にアップグレードし、render.rs 側の同種 upgrade は deferred (別 sub-ADR で対応)。render.rs 側の文字列 `matches!` は metadata codec の raw string field に依存しており upgrade には parser の touch が必要で scope creep を招く。両経路の guard は意味的に等価なので一方が強化されれば merged track 保護は確保される。
- layer binding resolver の共通化状態: D3 は既存 `libs/infrastructure/src/verify/tddd_layers.rs::parse_tddd_layers` を reuse する。`apps/cli::resolve_layers` と D3 の sync_rendered_views loop の両方が同じ `parse_tddd_layers` を呼ぶ形で既に共通化されており、重複実装は発生しない。

## Reassess When

1. **`architecture-rules.json` の `tddd.enabled` 判定ロジックが変わる場合**: 例えば per-track override / feature flag が導入された場合。D3 の拡張は既存 `parse_tddd_layers` を reuse しているため、判定ロジック変更時は `parse_tddd_layers` 本体を update すれば全 caller に自動的に反映される。
2. **layer 数が 10+ に増えた場合**: `sync_rendered_views` の iteration cost がボトルネックになった場合、parallel iter (`rayon::par_iter`) 化を検討する。現 workspace (3 layer) では impact 無視可能。
3. **catalogue write 経路が他にも追加される場合**: `baseline-capture` / `design` / 新 subcommand が catalogue を write するようになったら、それぞれに active-track guard を追加する。
4. **`TrackStatus` enum に新 variant が追加される場合**: D1 の guard は exhaustive `match` で 6 variants 全てを明示列挙しているため、新 variant 追加時に compile error が発生する。開発者は compile error に従って新 variant を frozen 側 (`Done | Archived` と同じ reject arm) か active 側 (`Planned | InProgress | Blocked | Cancelled` と同じ proceed arm) に分類する必要がある。既存 `render.rs:505` の文字列ベース `matches!` は別 sub-track での refactor 時に同じ exhaustive match pattern を適用する。
5. **feedback 再昇格の要求**: `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §Reassess When にあるように、feedback を Blue に再昇格する設計判断が将来なされた場合、SKILL.md / source-attribution.md の両方を同時に更新する必要がある。D4 修正はその時点で obsolete になる。
6. **`parse_tddd_layers` resolver の evolution**: `libs/infrastructure/src/verify/tddd_layers.rs::parse_tddd_layers` は現状 `apps/cli::resolve_layers` と D3 の sync_rendered_views loop の 2 caller から reuse されている。将来 3 caller 目以降が追加される、または `TdddLayerBinding` に新 field / accessor が必要になる場合は `verify/tddd_layers.rs` 本体を update する (共通化済なので再配置は不要)。
7. **B2 (branch match guard) の採用再評価**: active track 同士の cross-track 誤操作が実際に観測された場合、D1 の上に branch match guard を defense-in-depth として追加する。

## References

- `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` — TDDD 多層化 parent ADR
- `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §問題 2 / §Reassess When — feedback=Yellow 降格の根拠
- `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md` §D8 — per-layer opt-out pattern
- `knowledge/conventions/source-attribution.md` §Source Tag Types Table / §Upgrading Yellow to Blue — source tag SSoT
- `knowledge/conventions/hexagonal-architecture.md` — layer 依存方向制約
- `apps/cli/src/commands/track/tddd/signals.rs::execute_type_signals` (line 96-128) — D1 の修正対象
- `apps/cli/src/commands/track/tddd/signals.rs::resolve_layers` (line 15, 28) — `parse_tddd_layers` の既存 caller
- `libs/infrastructure/src/track/render.rs::sync_rendered_views` (line 568-606) — D3 の修正対象 (domain-only loop)
- `libs/infrastructure/src/track/render.rs:573` `is_done_or_archived` guard — D1 reference pattern
- `libs/infrastructure/src/type_catalogue_render.rs:64` — D2 の hardcoded header 修正対象
- `libs/infrastructure/src/verify/tddd_layers.rs` (`pub struct TdddLayerBinding` line 29, `pub fn parse_tddd_layers` line 139) — tddd-01 Phase 1 Task 7 で導入された既存 layer binding resolver
- `.claude/skills/track-plan/SKILL.md:165-167, :283-285` — D4 の修正対象
