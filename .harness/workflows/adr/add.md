# ADR Add Workflow SSoT

> Provider-agnostic workflow SSoT for authoring and amending Architecture Decision Records.
> Provider-specific adapters reference this file for workflow behavior and retain only their invocation surface, tool constraints, and report format.

## Mission

Author a user-owned ADR under `knowledge/adr/` through an interactive hearing.
The resulting ADR is a pre-track artifact that a later track-planning workflow may use as input.

## Inputs

- An optional topic phrase or ASCII kebab-case slug.
- A hearing mode: Full for a new ADR, Focused to add or replace a selected section of an existing ADR, or Quick for a small single-section amendment.
- The user's decisions, decision-ground references, and any references the ADR records.

## Preconditions

Before starting the hearing, read `knowledge/conventions/adr.md`, `knowledge/conventions/pre-track-adr-authoring.md`, and `knowledge/adr/README.md`.
The conventions take precedence if an older template or index example conflicts with them.
Confirm that a target ADR is eligible for direct amendment under the ADR lifecycle rules before using Focused or Quick mode.

## Sequence

### Resolve the subject

When no argument is supplied, ask for a Japanese topic phrase.
When a topic phrase is supplied, propose an ASCII kebab-case slug and confirm it with the user.
When a valid ASCII kebab-case slug is supplied, use it as the slug.
Resolve the Japanese title and slug in one short exchange without combining unrelated questions.

### Choose the hearing mode

Offer Full, Focused, and Quick modes.
Use Full by default for new authoring.
For Focused or Quick mode, identify the target ADR before editing it.
Focused mode adds or replaces one selected section after eliciting that section.
Quick mode changes only one section through one or two prompts and routes multi-section changes to Focused mode.
Do not amend a post-merge ADR's semantic content in place.
Route a requested post-merge semantic change to a new ADR that supersedes or refines the existing record.

### Conduct the hearing

Ask all questions in Japanese and preserve the user's free-form answers in Japanese.
Ask no more than three related questions in a batch.
Each question offers two or three concrete options plus a free-form or skip option.
Do not fabricate a decision or silently fill a skipped section.

For a Full hearing, elicit the sections in this order:

1. Title.
2. Context, including the problem, relevant observations, and relevant references.
3. Decision items starting at `D1`, including any user-confirmed nested decisions.
4. Rejected Alternatives starting at `A`, including each rejection rationale.
5. Consequences, categorized as positive, negative, or neutral when applicable.
6. Reassess When triggers.
7. Related references.

### Write the ADR

Create a new file only at `knowledge/adr/$(date -u +"%Y-%m-%d-%H%M")-<slug>.md`.
Obtain the UTC timestamp with `date -u +"%Y-%m-%d-%H%M"` rather than manual input.
Use the Nygard-style body format required by the ADR convention, with `Context`, `Decision`, `Rejected Alternatives`, `Consequences`, and `Reassess When` sections when the user did not skip them.
Use a Japanese body, with English limited to code identifiers, the ASCII slug, and repository-standard headings.
Do not add a `Status` section because ADR file existence is the operational approval.
Omit skipped sections rather than creating empty headings.

Start every new ADR with conformant YAML front matter.
Set `adr_id` to the file stem and add one `decisions[]` entry for each decision heading.
Give every new decision a non-empty user decision reference recorded during the hearing.
Do not use `grandfathered` for a new ADR.
Include a `Related` section only when the user supplies references or the selected template requires it.
Do not hard-code a specific ADR filename in the template.
Mark code or schema examples with `<!-- illustrative, non-canonical -->`.

### Validate and finish

Re-read the written ADR and verify that its top-level heading immediately follows the closing YAML front matter delimiter.
Verify that every included section has content and every skipped section is absent.
Verify that the file is under `knowledge/adr/` rather than a track directory.
Verify that the front matter is valid and each decision heading has its required metadata entry.
For an amendment, present the proposed diff and obtain user approval before writing it.
Ask whether the hand-maintained ADR index needs updating.
Do not regenerate the ADR index automatically.
Use the guarded track commit workflow or let the user commit the ADR manually.

## Gates

| Gate | Verdict |
| --- | --- |
| Required ADR conventions read before hearing | pass / fail |
| Target ADR is eligible for an in-place amendment | pass / fail / not applicable |
| User supplied or explicitly skipped every authored section | pass / fail |
| New ADR filename has a UTC timestamp and ASCII kebab-case slug | pass / fail / not applicable |
| ADR front matter and decision metadata are valid | pass / fail |
| ADR has no `Status` section and no empty included section | pass / fail |
| ADR is located under `knowledge/adr/` | pass / fail |
| Amendment diff has user approval before write | pass / fail / not applicable |

## Failure / recovery

- Missing or ambiguous subject input: ask the user for the needed topic, title, slug, or target ADR.
- A requested change spans more than one section in Quick mode: switch to Focused mode.
- A requested post-merge semantic change: author a new ADR instead of changing the existing record in place.
- A required decision or decision-ground reference is unavailable: leave that section skipped and report the omission rather than inventing content.
- A filename collision or invalid slug: resolve a different confirmed slug before writing.
- Validation failure: correct the generated ADR with the user where an edit changes their recorded judgment, then repeat validation.

## Outputs

- A new or amended ADR under `knowledge/adr/`.
- A summary containing the ADR path, included and skipped sections, and whether the user requested an ADR-index update.
- Suggested next actions: start track planning with the ADR or re-invoke ADR add in Focused mode.
