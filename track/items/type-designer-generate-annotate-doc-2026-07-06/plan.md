<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# type-designer capability doc の「生成 + 注釈」workflow への同期 — ADR D2/D3 非整合 fix

## Tasks (3/3 resolved)

### S1 — Schema reference manual

> `knowledge/conventions/catalogue-schema-reference.md`; `knowledge/conventions/README.md`: manual creation and convention-index registration (IN-05/IN-06/CN-02/AC-04/AC-05/AC-06/AC-08).

- [x] **T002**: `knowledge/conventions/catalogue-schema-reference.md`; `knowledge/conventions/README.md`: create the manual via `bin/sotp conventions add`, migrate the IN-04 mandatory sections from `.harness/capabilities/type-designer.md`, add the IN-06 authority note, update the convention index via `bin/sotp conventions update-index`, and run `bin/sotp conventions verify-index` (IN-05/IN-06/CN-02/AC-04/AC-05/AC-06/AC-08). (`3a8824670892aa63d0e656b4dfbed834385eff28`)

### S2 — Generate / annotate / verify workflow in the capability doc

> `.harness/capabilities/type-designer.md`; optional CN-05 manual append; wrapper delegation check (IN-01/IN-02/IN-03/IN-04/IN-05/IN-07/CN-01/CN-03/CN-05/AC-01/AC-02/AC-03/AC-06/AC-07/AC-08).

- [x] **T001**: `.harness/capabilities/type-designer.md`; `knowledge/conventions/catalogue-schema-reference.md` only for CN-05 boundary-section placement; `.claude/agents/type-designer.md`; `.agents/skills/type-designer/SKILL.md`: update the internal pipeline, 12a receipt criteria, schema-reference/cookbook placement, CN-05 boundary-section placement, annotation-phase references to the T002-created manual, and wrapper delegation consistency (IN-01/IN-02/IN-03/IN-04/IN-05/IN-07/CN-01/CN-03/CN-05/AC-01/AC-02/AC-03/AC-06/AC-07/AC-08). (`3a8824670892aa63d0e656b4dfbed834385eff28`)

### S3 — Kind-selection convention reference fix

> `knowledge/conventions/type-designer-kind-selection.md`: `kind` wire-format reference redirect (IN-08/CN-04/AC-08/AC-09).

- [x] **T003**: `knowledge/conventions/type-designer-kind-selection.md`: redirect the three `kind` wire-format references to the T002-created manual without changing R1-R10 rule text (IN-08/CN-04/AC-08/AC-09). (`3a8824670892aa63d0e656b4dfbed834385eff28`)
