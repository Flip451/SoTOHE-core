# Type Catalogue Review: Severity Policy

The reviewer's role is **type-design soundness review** of the per-layer type
catalogues `track/items/<track-id>/<layer>-types.json` and the track-scoped
feature declaration `track/items/<track-id>/tddd-features.json` (Phase 2
SoT). A catalogue is the *interface contract* of each layer — it declares
which types / traits / functions are added or modified, and how they relate to
spec elements (`spec_refs[]`). The feature declaration is the extraction
contract: it declares the Cargo features through which each TDDD-enabled
layer's catalogue surface is observed.

Generated rendered views such as `contract-map.md` and `*-types.md` are
read-only human reference outputs. They are not review targets and not fix
targets for this scope; report catalogue JSON issues, not view-only or renderer
issues.

This briefing layers **two reading lenses**:

1. **SoT integrity** — does each catalogue entry trace to a spec element via
   `spec_refs[]`, and is `kind` / `role` / `action` internally consistent?
   (ref-verify Chain2 covers semantic spec ↔ catalogue alignment; the reviewer
   handles the structural reading lens.)
2. **General coding principles applied to the type contract** — SOLID, CQRS,
   DRY. The catalogue is not "just JSON metadata"; it is the *type-level design*
   of the system, so the same principles that guide good Rust API design apply
   to its declarations.

**Mechanical checks** (schema validation, signal computation, layer dependency)
are handled by `bin/sotp signal calc-catalog-spec` / `check-catalog-spec` /
`cargo make check-layers` / `verify-*`, not the reviewer.

## What to report

Report findings ONLY for the following categories. Each catalogue finding must
cite either a specific entry's `key` (e.g., `domain::ReviewScopeConfig`) or a
spec_refs/role/action mismatch. Each feature-declaration finding must cite the
affected layer key.

### Track-scoped feature declaration findings

- **incomplete or ambiguous feature declaration**: `tddd-features.json` does
  not declare every TDDD-enabled layer from `architecture-rules.json` exactly
  once, omits an explicit empty list for a featureless layer, or declares a
  layer that is not TDDD-enabled. The declaration is a committed Phase 2 SoT,
  not a generated view.
- **invalid Cargo feature selection**: a feature named for a layer is absent
  from that layer's target crate `Cargo.toml`, or a non-empty selection is not
  grounded in the feature-gated type surface the track catalogue declares.
  Cite the layer and the mismatching Cargo feature or catalogue entry.
- **extraction-contract drift**: the declaration and the catalogue together
  leave a feature-gated type in the track's declared surface unobservable, or
  select an unrelated feature without a type-contract purpose. Judge this
  against the target crate's feature gates and catalogue entries; do not infer
  a new product feature or require `--all-features`.

### SoT integrity findings

- **role / kind mismatch**: a struct / enum / trait whose declared `role`
  (`Entity` / `ValueObject` / `SpecificationPort` / `SecondaryPort` /
  `PrimaryAdapter` / `SecondaryAdapter` / etc.)
  does not match the `kind` discriminator or the layer the entry lives in
	  (e.g., a review-execution `SecondaryPort` placed in domain when the
	  capability it names is an orchestration concern rather than one described
	  in aggregate vocabulary). A placement the shipped catalogue-lint
  `KindLayerConstraint` already rejects is a mechanical failure, not a review
  finding — report the placements the lint permits but the entry's own
  declared semantics contradict.
- **action incoherent with the diff**: an entry declared `action: add` that
  references a type already present in the rustdoc baseline, or `action: modify`
  on a method whose signature is identical to baseline — the catalogue's action
  declaration should match the actual change being introduced.
  Use the rustdoc baseline / impl-catalog signal semantics for this judgement,
  not the previous committed catalogue JSON or a prior review diff. If
  `bin/sotp signal calc-impl-catalog` accepts `action: add` because the item is
  absent from the rustdoc baseline, do not report it merely because the entry
  existed in an earlier catalogue revision.
- **spec_refs missing or off-topic**: an entry whose `spec_refs[]` is empty
  (Chain2 would flag this 🔴) — call it out if it's load-bearing, OR an entry
  whose `spec_refs[].anchor` cites a spec element whose intent is plainly
  unrelated to the type's purpose at the narrative level.
- **upstream restatement**: an entry whose `docs` / `intent` field restates
  an upstream ADR's or spec.json's design rationale or behaviour contract in
  prose. Flag the restatement itself regardless of whether an anchor cite
  (`AC-NN` / `IN-NN` / `CN-NN` / spec element id) accompanies it — the field
  must reference upstream behaviour by anchor cite, not reproduce it. Cite
  `.harness/policies/no-upstream-restatement.md`.
- **unsupported semantic placement**: a `ValueObject` placed in domain, usecase,
  or infrastructure whose `docs` or reviewable track record does not establish
  the semantic grounds for that placement: ubiquitous language, invariant
  ownership, stable meaning across application operations, and independence
  from persistence / CLI / workflow concerns as applicable. A reviewer must
  not treat absent same-track domain-internal inbound references as sufficient
  rejection; they are supporting evidence only. Cite the entry key and name
  which of those grounds the entry leaves unestablished.

### Cross-layer contract findings

- **unconstructible error variant (dead vocabulary)**: an ErrorType variant
  whose payload no declared layer can actually construct — e.g. a variant
  carrying data (a resolved value, an "actual" comparand) that neither the
  declaring layer nor any caller in the declared call chain possesses at the
  failure point. Cite the variant and walk the missing data's origin.
- **mirror chain divergence**: enums or field lists documented as conversion
  mirrors across layers (CLI value-enum → driver select-enum → core enum, or
  parallel command/input field sets) whose variant-name sets or field
  names/ordering diverge without a documented language-level constraint
  (reserved word at the flag surface). Cite both entry keys and the diverging
  members.
- **enforcement-owner contradiction**: a fail-closed rule whose enforcement is
  claimed by different layers in different entries' docs ("rejected upstream"
  in one, "validated here" in another), or claimed by no layer at all. The
  catalogue must name exactly one owner and the owner must be able to construct
  its rejection data.
- **lossy boundary downgrade**: an upper-layer report or error that reduces
  structured lower-layer detail (a list of typed values) to a count or joined
  string without a documented justification, when the sibling read path carries
  the full values.
- **missing companion modify**: the track's ADR / spec commits to changing an
  existing baseline public type, but no `action: modify` entry declares the
  post-change shape — the change would land as an undeclared API mutation
  caught only at implementation time.

### API shape findings

- **per-method error imprecision**: a method whose declared error type forces
  callers to handle variants that operation can structurally never produce
  (e.g. an `init`-style method returning a shared error enum containing
  parse-failure variants when it parses nothing). A shared error enum is
  acceptable; flag when the gap is large and the entry docs neither list the
  producible subset nor justify the breadth. Cite the method and the
  unreachable variants.
- **ownership signature smell**: a read-only operation taking `String` /
  `Vec<T>` / `PathBuf` by value where a borrow suffices; an owned return that
  clones what a borrow could expose; `&mut self` on a method whose docs
  describe a read. Judge from the declared receiver / params / returns.
- **state-shape smell**: `Option<Option<T>>`; `Result<Option<T>, E>` where
  `None` and `Err` overlap in meaning; a bool + `Option<T>` pair encoding a
  tri-state that a dedicated enum would make unrepresentable-wrong. Cite the
  field or signature and name the illegal state the current shape admits.
- **sync/async color mismatch**: a port whose method set implies external I/O
  (filesystem, network, database naming or failure vocabulary) declared fully
  synchronous without the entry docs acknowledging the choice — or the
  reverse, `is_async` on pure computation. Early color errors force rework at
  the adapter.
- **public enum evolution ambiguity**: a public enum that downstream projects
  are expected to `match` on, where the entry does not indicate whether the
  variant set is intentionally closed (exhaustive matching desired) or
  extension-tolerant (`#[non_exhaustive]` intent). Flag only for enums on a
  template-consumer-facing surface.

### SOLID findings

- **Single Responsibility violation**: a single struct / interactor / port
  bundling unrelated concerns that change for different reasons. Distinguish
  from "the type happens to be large" — flag when separate concerns are
  *encoded into the same type's fields / methods*, not when one cohesive
  concern naturally requires many fields.
- **Open/Closed violation in catalogue shape**: an enum / trait that the
  catalogue must amend every time a new variant or method is added at the same
  layer, when an extension point (separate trait, separate enum, plugin
  pattern) would isolate the change. Flag only when the next plausible
  extension would clearly require touching the same closed entry.
- **Liskov violation in trait design**: a trait method whose default
  implementation or documented invariant cannot be honoured by a plausible
  implementor (e.g., a port method whose contract assumes synchronous behaviour
  but a real adapter will be async without a way to express that). Cite the
  catalogue entry's method declaration.
- **Interface Segregation violation**: a port / trait whose methods clearly
  split into two disjoint usage groups (no real caller needs both halves),
  forcing implementors to stub methods they do not use.
- **Dependency Inversion violation in catalogue placement**: a usecase or
  domain entry whose declared dependencies (via `params[]` / return types /
	  associated types) point at a concrete infrastructure type instead of a port.
	  Cite `architecture-rules.json`.

### CQRS findings

- **unsupported CQRS split**: a Command / Query separation without a
  reviewable record of an operation-specific asymmetry in side effects,
  collaborators, errors, consistency boundaries, or read/write models. A
  read/write label alone is not enough. Do not require a split merely because
  an asymmetry exists: such evidence is necessary for a split, not sufficient
  to mandate one. Cite both entry keys and the asymmetry the record fails to
  establish.
- **port whose name suggests one side but signature does the other**: e.g.,
  a port named `<Thing>Reader` whose declared methods mutate, or a `<Thing>Writer`
  that primarily reads. The catalogue is the contract; misleading names lock
  in misleading expectations.

### DRY findings (at the type-contract level)

- **duplicated structural declaration**: two entries in the same or neighbouring
  layers that declare functionally identical shapes (same fields, same methods,
  same invariants) without one being declared a `reference` to the other. Cite
  both entry keys.
- **duplicated method signature across sibling types**: the same method
  signature declared verbatim on multiple types that could share a trait. Flag
  only when the duplication is across types in the *same* layer and the
  catalogue declares all of them as `add` / `modify` (not when one is a
  pre-existing reference).
- **duplicate adapter-of-port without shared trait declaration**: two
  `SecondaryAdapter` entries implementing the same port concept without the
  catalogue declaring the shared port; the port should be the SSoT and the
  adapters reference it.

## What NOT to report

- Field name nits / Rust naming convention preferences when the existing name
  already passes `cargo make clippy` / project rustfmt config
- Doc string wording suggestions
- Adding derives that the catalogue intentionally omits (the omission is
  almost always deliberate; verify with `<layer>-types.json` first before
  questioning)
- Performance micro-optimization that does not cross a correctness boundary
- Backward-looking observations about how many entries were added or how
  many revisions the catalogue went through
- Suggested behavioural extensions — those expand spec, not types; redirect
  to the spec reviewer's domain
- Layer-split suggestions when the type-design ADR explicitly chose layer-unified
  organisation (refer to the track's ADR before flagging)
- Test-side / `#[cfg(test)]` declarations — the catalogue declares production
  surface only

## Action-classification baseline

Catalogue `action` values (add / modify / reference) are judged against the track's FROZEN
baseline artifacts (`track/items/<id>/<layer>-types-baseline.json` and the frozen rustdoc
baseline captured at track start), per the catalogue action ADRs. A type that first appears
within the current track — even if already committed by an earlier task of the same track —
is `add`, not `modify`; only identities present in the frozen baseline take `modify`. Do
not classify actions against the current rustdoc or committed HEAD.
