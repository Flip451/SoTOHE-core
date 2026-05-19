<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->
<!-- IN-24 / OS-07 DEFERRED: detailed v3 contract-map rendering requires ADR-level design decisions (node shapes, edges, role clustering). This placeholder lists entry names per layer only. -->
```mermaid
flowchart LR
    %% contract-map renderer: IN-24 minimal placeholder (detailed v3 rendering deferred to follow-up ADR/track per OS-07).
    %% Each layer block lists entry names for observability. No node shapes or edges are emitted.
    subgraph domain [domain]
        %% type: BoundOp
        %% type: CatalogueDocument
        %% type: InherentImplDeclV2
        %% type: NewTypeGraphCodecError
        %% type: TraitEntry
        %% type: TraitImplDeclV2
        %% type: WherePredicateDecl
    end
    subgraph usecase [usecase]
        %% type: ReviewCheckApprovedInteractor
        %% type: TaskOperationInteractor
        %% trait: ReviewCheckApprovedService
        %% trait: TaskOperationService
    end
    subgraph infrastructure [infrastructure]
        %% type: CatalogueToExtendedCrateCodec
        %% type: CatalogueToExtendedCrateCodecError
        %% type: SignalEvaluatorV2
    end
```
