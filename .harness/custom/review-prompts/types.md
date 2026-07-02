# Type Catalogue Review: Severity Policy

The reviewer's role is **type-design soundness review** of the per-layer type
catalogues `track/items/<track-id>/<layer>-types.json` (Phase 2 SSoT) and the
integrated mermaid view `contract-map.md`. The catalogue is the *interface
contract* of each layer — it declares which types / traits / functions are added
or modified, and how they relate to spec elements (`spec_refs[]`).

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

Report findings ONLY for the following categories. Each finding must cite
either a specific entry's `key` (e.g., `domain::ReviewScopeConfig`) or a
spec_refs/role/action mismatch.

### SoT integrity findings

- **role / kind mismatch**: a struct / enum / trait whose declared `role`
  (DomainEntity / ValueObject / Port / PrimaryAdapter / SecondaryAdapter / etc.)
  does not match the `kind` discriminator or the layer the entry lives in
	  (e.g., a port placed in domain instead of usecase). Cite
	  `knowledge/conventions/hexagonal-architecture.md` §Port Placement Rules.
- **action incoherent with the diff**: an entry declared `action: add` that
  references a type already present in the rustdoc baseline, or `action: modify`
  on a method whose signature is identical to baseline — the catalogue's action
  declaration should match the actual change being introduced.
- **spec_refs missing or off-topic**: an entry whose `spec_refs[]` is empty
  (Chain2 would flag this 🔴) — call it out if it's load-bearing, OR an entry
  whose `spec_refs[].anchor` cites a spec element whose intent is plainly
  unrelated to the type's purpose at the narrative level.

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
	  Cite `knowledge/conventions/hexagonal-architecture.md` §Layer Dependencies.

### CQRS findings

- **command / query method mixing in one type**: a domain service or usecase
  interactor whose declared methods both mutate state and return non-trivial
  read models from the same method, when separation would make the contract
  clearer. Cite `knowledge/conventions/coding-principles.md` if a CQRS rule is
  documented; otherwise flag as a clarity concern.
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
