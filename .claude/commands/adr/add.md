---
description: Author a new Architecture Decision Record (ADR) via interactive hearing.
---

Create a new ADR under `knowledge/adr/` by eliciting each section (Context, Decision, Rejected Alternatives, Consequences, Reassess When) through AskUserQuestion batches. The resulting ADR becomes the pre-track-stage artifact that `/track:plan` reads as a precondition.

**Language convention**: the ADR **body must be written in Japanese** (ADRs are project-specific user-facing decision records). English is reserved for code identifiers, the filename slug, and section headers where the repository convention already uses English (e.g., `## Context`, `## Decision`). When eliciting through AskUserQuestion, prompt the user in Japanese and transcribe the answers in Japanese. Only the slug is ASCII kebab-case for filesystem compatibility. ADRs do NOT include a `## Status` section — file existence is the operational approval (see `knowledge/conventions/pre-track-adr-authoring.md`).

Arguments:

- `$ARGUMENTS` may be empty, a topic phrase, or an ASCII kebab-case slug.
  - Empty: ask for a topic phrase in Japanese first, then derive a slug and confirm with the user.
  - Topic phrase (free-form Japanese): suggest an ASCII kebab-case slug (romaji or concise English summary) and confirm with the user before using it as the filename slug.
  - Slug (already ASCII kebab-case): use as-is.

Execution:

- Before anything else, read:
  - `knowledge/conventions/adr.md` — ADR writing rules (Nygard + Rejected Alternatives + Reassess When)
  - `knowledge/conventions/pre-track-adr-authoring.md` — ADR lifecycle and placement rules (**authoritative**: ADRs carry no `## Status` section; file existence is the operational approval)
  - `knowledge/adr/README.md` — ADR index and section format reference (note: the `## Status` block in the README template is superseded by `knowledge/conventions/pre-track-adr-authoring.md`; do NOT include `## Status` in new ADRs)
- Resolve the slug and title in one short exchange. Prefer deriving the slug from the topic phrase; do not ask multiple orthogonal questions in one turn.
- Offer three hearing modes via AskUserQuestion:
  - **Full** — elicit every section of a brand-new ADR (default for new authoring)
  - **Focused** — add or replace specific sections in an existing ADR (specify the target ADR path separately)
  - **Quick** — small amendment to an existing ADR (elicit one or two sections only; leave the rest untouched)
- Per-section elicitation (Full mode). Use AskUserQuestion batches of at most 3 related prompts; always provide 2–3 concrete options plus a free-form / skip option.

### Section order (Full mode)

All AskUserQuestion prompts are asked in Japanese and the user's free-form answers are stored verbatim in Japanese.

1. **Title** — short Japanese title for the heading. Elicit in one prompt; the title is Japanese only (no status value — ADRs carry no `Status` field per `knowledge/conventions/pre-track-adr-authoring.md`).
2. **Context (背景)** — what the problem is, which observations call for a decision, and which existing ADRs / conventions / discussions are relevant. Elicit in Japanese.
   - Options: "自由記述" / "既存 ADR / convention を cite (path 指定)" / "skip (後で埋める)"
3. **Decision D1..Dn (決定)** — elicit each decision item with a numeric prefix. Start at D1 and stop when the user selects "次の section へ進む". Section title stored in Japanese; the `D1:` prefix is English.
   - Options: "D{n} を自由記述で入力" / "次の section (Rejected Alt) へ進む"
   - Nested sub-decisions (`D1.1`, `D1.2`, ...) are allowed. Confirm in one question whether sub-decisions exist.
4. **Rejected Alternatives A..Z (却下した代替案)** — elicit alternatives starting from A. Each alternative carries a Japanese title plus the rejection rationale.
   - Options: "A を自由記述で入力" / "次の section (Consequences) へ進む"
5. **Consequences (影響)** — record the adoption consequences (positive / negative / neutral) in Japanese.
   - Options: "Positive / Negative / Neutral を自由記述" / "Positive のみ (Negative は skip)" / "section を skip"
6. **Reassess When (再評価条件)** — list triggers (conditions) under which the decision should be reconsidered.
   - Options: "自由記述で列挙" / "skip (general default を挿入: 採用プロジェクトのフィードバック / 新しい技術要件の出現 など)"
7. **Related (任意)** — references to related ADRs / conventions. Directory references are fine; the template must not hard-code specific ADR filenames.
   - Options: "path を自由記述で列挙" / "skip"

### Focused / Quick modes

- Focused: confirm the target ADR path in one question → ask which section to add/replace in one question → run Full-mode elicitation only for that section → insert / append it → present the diff to the user and write after approval.
- Quick: a lightweight Focused variant. Reduce AskUserQuestion to 1–2 prompts and apply a small edit that stays within a single section. Suggest switching to Focused when the change would span multiple sections.

### ADR file generation

- Destination: `knowledge/adr/$(date -u +"%Y-%m-%d-%H%M")-<slug>.md`
  - The timestamp is UTC (`date -u +"%Y-%m-%d-%H%M"`). Manual input is forbidden.
- Template skeleton (body in Japanese; skipped sections are omitted):

  ```markdown
  # <日本語タイトル>

  ## Context

  <elicit した背景 (日本語)>

  ## Decision

  ### D1: <D1 タイトル>

  <D1 本文 (日本語)>

  ### D2: <D2 タイトル>

  <D2 本文 (日本語)>

  ...

  ## Rejected Alternatives

  ### A. <A タイトル>

  <A 本文 + 却下理由 (日本語)>

  ### B. <B タイトル>

  ...

  ## Consequences

  ### Positive

  - ...

  ### Negative

  - ...

  ## Reassess When

  - <trigger 1>
  - <trigger 2>

  ## Related

  - `knowledge/adr/` — ADR 索引
  - `knowledge/conventions/<related>.md`
  ```

- Embed specific other-ADR filenames only when the user explicitly cites them; the template itself must not hard-code them.
- When the body shows code / schema examples, add `<!-- illustrative, non-canonical -->` markers so the canonical-block suspicion detector does not flag them.

### Post-creation

- Immediately re-read the generated ADR and verify:
  - The top heading (`# <Title>`) is on line 1
  - No empty sections remain (skipped sections should be omitted)
  - The file lives under `knowledge/adr/`, not under a track directory (`track/items/<id>/`)
- If further edits are needed, propose the change to the user and edit only after confirmation.
- The ADR index is not regenerated by tooling (the current `knowledge/adr/README.md` is hand-maintained). Ask the user in one question whether the index needs updating.
- After authoring, the ADR becomes visible to `/track:plan`'s pre-check.

Behavior:

- Do **not** run `git add` / `git commit` directly. Commit the ADR via `/track:commit` or let the user commit it manually.
- Do **not** fabricate decisions — an ADR records the user's judgement; the AI must not invent content. Every section must either be elicited or explicitly skipped.
- After creation, present a summary:
  1. The generated ADR file path
  2. The list of sections included (skipped sections are listed explicitly)
  3. Suggested next commands: `/track:plan <feature>` (start a track that references the ADR) or re-invoke `/adr:add` in Focused mode to update this ADR.
