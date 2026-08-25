<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# シークレット秘匿の正規表現を fail-closed にする

## Summary

GO-01 is delivered by T001's fail-stop redaction construction and T002's security-boundary convention. The track intentionally introduces no public catalogue entries or startup execution surface.

## Tasks (0/2 resolved)

### S1 — Fail-closed static redaction patterns

> Convert the four static redaction-boundary regexes and update their sanitization call sites in libs/usecase/src/pr_review.rs; add construction verification. IN-01; IN-02; OS-01; OS-02; CN-01; CN-02; AC-01; AC-02; AC-04.

- [ ] **T001**: Convert the four static redaction-boundary regexes and update their sanitization call sites in libs/usecase/src/pr_review.rs; add construction verification. IN-01; IN-02; OS-01; OS-02; CN-01; CN-02; AC-01; AC-02; AC-04.

### S2 — Security-boundary convention

> Add redaction, validation, and authorization boundary guidance to knowledge/conventions/security.md. IN-03; AC-03.

- [ ] **T002**: Add redaction, validation, and authorization boundary guidance to knowledge/conventions/security.md. IN-03; AC-03.
