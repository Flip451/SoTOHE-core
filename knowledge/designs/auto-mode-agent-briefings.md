# Auto Mode Agent Briefings Design

> Defines the prompt structure and context injection rules for each of the 6 phases
> in the `/track:auto` cycle.

## Common Context Injection

All phases receive a base context:

```
Track: {track_id}
Task: {task_id} — {task_description}
Spec summary: {spec.md first 3 paragraphs}
Tech stack: {track/tech-stack.md key constraints}
Conventions: {relevant project-docs/conventions/*.md}
```

## Phase 1: Plan (planner capability)

**Purpose**: Create a task-level implementation plan.

**Input context**:
- spec.md (full)
- plan.md task description and section context
- track/tech-stack.md
- project-docs/conventions/ (relevant files)
- DESIGN.md canonical blocks (existing type signatures)
- Previous task artifacts (if sequential dependency)

**Prompt template**:
```
You are planning the implementation of task {task_id}: {task_description}

## Context
{base_context}

## Existing Types
{DESIGN.md canonical blocks}

## Instructions
Create an implementation plan with:
1. Files to create or modify (with paths)
2. Types to define (trait/struct/enum signatures)
3. Test cases to write (test names and intent)
4. Dependencies between steps
5. Risks and mitigation

Output format: structured markdown with numbered steps.
```

**Output**: Implementation plan (markdown)

## Phase 2: Plan Review (reviewer capability)

**Purpose**: Verify the implementation plan is complete and feasible.

**Input context**:
- Implementation plan from Phase 1
- spec.md constraints and acceptance criteria
- Architecture rules (docs/architecture-rules.json)
- Existing codebase structure

**Prompt template**:
```
Review this implementation plan for task {task_id}: {task_description}

## Plan
{plan_from_phase_1}

## Spec Constraints
{spec.md constraints section}

## Architecture Rules
{architecture-rules.json layer dependencies}

## Review Criteria
- Does the plan cover all acceptance criteria?
- Are file paths consistent with the workspace structure?
- Are layer dependencies correct (domain ← usecase ← infrastructure ← cli)?
- Are test cases sufficient (happy path + error cases)?
- Is the scope appropriate (not over-engineered)?

Report findings as JSON:
{"verdict":"zero_findings","findings":[]}
or
{"verdict":"findings_remain","findings":[{"message":"...","severity":"P1|P2|P3","file":null,"line":null}]}

Severity guide:
- P3: Design-level issue (wrong abstraction) → rollback to Plan
- P2: Type-level issue (missing type/trait) → rollback to Plan (pre-TypeDesign)
- P1: Minor issue (missing test case) → fix in place (re-enter Plan to apply fix, then re-review)
```

**Output**: JSON verdict
**Rollback trigger**: P2+ → rollback to Plan. P1 → re-enter Plan (authoring phase) to fix, then re-run PlanReview.

## Phase 3: Type Design (planner capability)

**Purpose**: Define trait/struct/enum signatures in `.rs` files.

**Input context**:
- Approved implementation plan
- DESIGN.md canonical blocks (existing types)
- Existing domain/usecase/infrastructure type definitions
- track/tech-stack.md (async/sync decision, dependencies)

**Prompt template**:
```
Design the type definitions for task {task_id}: {task_description}

## Approved Plan
{approved_plan}

## Existing Types
{relevant existing type signatures from domain/usecase layers}

## Instructions
Define the following in Rust source files:
1. New types (struct/enum) with fields and doc comments
2. New traits with method signatures and doc comments
3. Error types with thiserror derives
4. Type aliases if needed

Rules:
- No method bodies — signatures only (bodies come in Implement phase)
- No todo!() or unimplemented!()
- Follow existing naming conventions (PascalCase types, snake_case methods)
- Add /// doc comments with # Errors sections
- Respect layer boundaries (domain types cannot reference infrastructure)
```

**Output**: Rust source files with type definitions

## Phase 4: Type Review (reviewer capability)

**Purpose**: Review type definitions for API ergonomics and correctness.

**Input context**:
- Type definitions from Phase 3
- Approved plan
- Existing types for consistency check
- Architecture rules

**Prompt template**:
```
Review these type definitions for task {task_id}: {task_description}

## New Type Definitions
{diff of new/modified .rs files}

## Existing Types for Consistency
{related existing type signatures}

## Review Criteria
- Object safety (if traits will be used as dyn)
- Send + Sync bounds (if used across threads/tasks)
- API ergonomics (builder pattern where appropriate, Into<T> for flexibility)
- Naming consistency with existing codebase
- Error type granularity (not too broad, not too narrow)
- Doc comment completeness (# Errors sections)
- No todo!()/unimplemented!() in library code

Report findings as JSON with severity:
- P3: Design issue → rollback to Plan
- P2: Signature change needed → rollback to TypeDesign
- P1: Doc/naming fix → fix in place (re-enter TypeDesign to apply fix, then re-review)
```

**Output**: JSON verdict
**Rollback trigger**: P3 → Plan, P2 → TypeDesign. P1 → re-enter TypeDesign to fix, then re-run TypeReview.

## Phase 5: Implement (implementer capability)

**Purpose**: TDD implementation following Red → Green → Refactor.

**Input context**:
- Approved type definitions
- Approved plan (test cases list)
- spec.md acceptance criteria
- Existing test patterns in the codebase

**Prompt template**:
```
Implement task {task_id}: {task_description}

## Approved Types
{type definitions from Phase 3}

## Test Cases from Plan
{test cases list from approved plan}

## Instructions
Follow TDD:
1. RED: Write failing tests first
2. GREEN: Write minimal code to pass tests
3. REFACTOR: Clean up while keeping tests green

Rules:
- No unwrap()/expect() outside #[cfg(test)]
- Use ? operator for error propagation
- Follow existing test naming: test_{target}_{condition}_{expected_result}
- Run cargo make test-one-exec {test_name} after each test
- Run cargo make ci-rust before signaling completion
```

**Output**: Implemented source files + tests

## Phase 6: Code Review (reviewer capability)

**Purpose**: Final review for correctness, performance, idiomatic Rust.

**Input context**:
- Implementation diff (all changes in this cycle)
- Type definitions
- Approved plan
- spec.md acceptance criteria

**Prompt template**:
```
Review the implementation of task {task_id}: {task_description}

## Changes
{git diff of all changes in this cycle}

## Approved Plan
{plan summary}

## Review Criteria
- Logic errors, edge cases, race conditions
- No panics in library code (no unwrap/expect outside #[cfg(test)])
- Proper error propagation (thiserror, #[source], #[from])
- Architecture layer dependency direction
- Idiomatic Rust (naming, patterns, clippy compliance)
- Test coverage (happy path + error cases)
- Security (input validation, error information leakage)
- Performance (unnecessary clones, allocation patterns)

Report findings as JSON with severity:
- P3: Design issue → rollback to Plan
- P2: Type change needed → rollback to TypeDesign
- P1: Implementation fix → rollback to Implement

DO NOT report findings about test code using unwrap/expect.
DO NOT report findings about unchanged pre-existing code.
```

**Output**: JSON verdict with severity-based rollback target
**Rollback mapping**: P3 → Plan, P2 → TypeDesign, P1 → Implement
