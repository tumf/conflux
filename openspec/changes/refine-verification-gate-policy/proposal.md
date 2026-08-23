---
change_type: implementation
priority: high
dependencies:
  - prevent-bundled-verification-gates
references:
  - src/openspec_cmd/validation.rs
  - openspec/specs/cflx-proposal-validation/spec.md
  - skills/cflx-proposal/SKILL.md
verifications:
  - id: verification-policy-tests
    requirement: Native validation warns deterministically about declared heavyweight change-blocking commands without inferring task semantics or rejecting bounded Docker builds
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/openspec_cmd/validation.rs
    evidence: cargo test openspec_cmd --lib
    rerun: cargo test openspec_cmd --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Refine verification gate policy

**Change Type**: implementation

## Problem / Context

Fable reviewed `prevent-bundled-verification-gates` after its proposal had already been merged and implementation started. The review found that exact task-note cohesion rewards repeated labels rather than detecting unrelated work, duplicates frontmatter command authority in task prose, and can reject legitimate narrow task filters. It also found that a broad Docker denylist conflicts with existing support for bounded `docker build` evidence.

The useful prevention mechanism is smaller: warn when a structured change-blocking declaration explicitly authorizes known unbounded execution. Runtime Acceptance behavior remains outside proposal validation.

## Proposed Solution

Refine the landed policy before hard-error adoption:

- Remove native cohesion enforcement based on task-note ownership markers or commands. `verifications[].evidence` and `verifications[].rerun` remain the only command authority.
- Inspect only structured frontmatter commands. Never scan task prose for heaviness.
- Match exact command tokens and explicit flag pairs, not substrings. Initially flag container orchestration (`docker compose`, `docker-compose`, `docker run`, `docker swarm`, `podman`, `kubectl`), architecture emulation (`qemu-system-*`, `cross`), benchmarks (`cargo bench`), broad selectors (`--workspace`, `--all-features`, `--ignored`, `--include-ignored`, `--features heavy`, `--exhaustive`), and structural repetition (`seq`, `xargs`).
- Explicitly permit bounded `docker build` and test names containing words such as `full`, `heavy`, `benchmark`, or `exhaustive` inside longer tokens.
- Emit actionable strict-validation warnings during migration. Archive-gate validation does not fail on these findings. A later proposal may promote observed warnings to errors after an active-proposal survey.
- Preserve existing guidance that broad suites belong to repository automation, Acceptance, benchmark, manual, or operational-observation ownership.

## Acceptance Criteria

- Native validation emits no cohesion finding from task-note marker or command differences.
- Only frontmatter `evidence` and `rerun` commands control heavyweight warnings.
- Exact-token matching avoids substring false positives and permits `docker build`.
- Strict mode emits actionable warnings naming verification ID and matched token; archive gate remains non-blocking during migration.
- Legacy declarations without structured execution/completion roles remain valid.
- Documentation states that validation constrains declared authorization, not commands independently chosen by an AI session.

## Explicit Completion Conditions

- Tests cover warning-only migration, bounded `docker build`, literal substring false positives, explicit heavyweight forms, legacy declarations, and ignored task prose.
- An active-proposal survey is recorded in `design.md` before any future hard-error promotion.
- `cargo test openspec_cmd --lib` passes and selects at least one relevant test.

## Retired Scenarios

- cflx-proposal-validation: Proposal verification plans bound Apply-owned work / Heavy repository gate is not an Apply checkbox

Superseded by `Heavy repository gate produces a migration warning`: the same declared forms are still discouraged, but during migration they are reported as strict-validation warnings rather than as a rejected Apply checkbox.

## Out of Scope

- Semantic task-cohesion inference.
- Runtime command interception or Acceptance task selection.
- Acceptance wall-clock limits and verification result reuse.
- Promoting warnings to errors in this change.

## Verification Ownership

Focused validator tests own this policy. Repository-wide checks remain hook-owned.

## Fable Review

Fable verdict on the predecessor proposal: `adopt-with-changes`. This correction adopts the lower-risk option Fable identified: drop cohesion and ship only deterministic heavyweight-command warnings with migration evidence.
