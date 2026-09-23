---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/proposal-metadata/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - src/config/defaults.rs
  - src/orchestration/acceptance/verification_evidence.rs
  - src/orchestration/acceptance/execution_manifest.rs
  - src/orchestration/acceptance/gate_execution.rs
  - src/orchestration/acceptance/manifest_builder.rs
  - src/parallel/executor.rs
  - src/orchestration/operator_command.rs
verifications:
  - id: external-acceptance-state-tests
    requirement: Acceptance stores reusable evidence outside every target worktree, evaluates full target cleanliness, and keeps retry authority process-local.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/acceptance_execution_boundary.rs
    evidence: cargo test acceptance_execution_boundary --lib
    rerun: cargo test acceptance_execution_boundary --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Relocate Acceptance evidence outside target worktrees

**Change Type**: implementation

## Problem / Context

Acceptance currently creates `.cflx/verification-evidence/` inside each managed target worktree. `EvidenceStore` writes gate logs, envelopes, a self-ignoring `.gitignore`, and Acceptance manifests there. Cleanliness logic then excludes this directory so Conflux-created files do not make the worktree appear dirty.

That behavior violates `openspec/CONSTITUTION.md`: Conflux-owned runtime artifacts may not be written into target repositories or worktrees, and ignore rules or cleanliness exceptions do not make such writes permissible. It also leaks into repository tooling, as formatters and other whole-tree checks encounter files the target project never requested.

The stored terminal manifest additionally controls unchanged-input retry after process restart. Moving that workflow authority to external state would violate the constitution's repository-derived control rule: deleting hidden Conflux state would change whether work may proceed. Reusable gate output may be external cache, but it cannot own admission, retry, archive, merge, or next-action selection.

## Proposed Solution

Store Acceptance manifests, gate evidence envelopes, and bounded gate-output artifacts under the resolved Conflux state root, never under the target repository or any managed worktree. Add one shared resolver rooted at `<conflux-state-root>/acceptance/<project-slug>/<workspace-slug>/<change-id>/`. `project-slug` and `workspace-slug` use the existing canonical-path slug algorithm; every path component supplied by a change or verification ID remains validated before use. The resolver follows existing `state_base_dir` precedence and uses an external temporary fallback only when no state root can be derived.

Pass the resolved store explicitly through manifest construction, gate planning, execution, terminal recording, and cleanup. Storage creation or write failure yields the existing typed Acceptance runtime hold before any declared gate starts. It must not fall back to the target worktree.

Treat external records as cache and diagnostics only. Exact successful gate evidence may still be reused when every repository, declaration, tool, review-range, artifact, and clean-state binding matches. Missing, malformed, deleted, or stale external evidence reruns the declared gate or produces its ordinary typed hold; it never implies PASS.

Remove durable manifest-based admission. During one live owner process, the reducer's typed non-resumable hold may refuse an unchanged retry and expose `unchanged_acceptance_input`. After owner restart, no external manifest may recreate that refusal; an explicit retry receives one fresh bounded attempt, while exact valid gate evidence may reduce repeated work. This keeps retry control process-local and prevents external cache deletion from changing repository-derived workflow state.

Evaluate target cleanliness without special Conflux path exclusions. Remove the self-ignoring file, `dirty_entries_excluding_evidence`, evidence-path filtering, and all production writes below `.cflx/verification-evidence`. Legacy target-local evidence is never read, migrated, rewritten, or automatically deleted. If an old target-local evidence path is visible as untracked or ignored state, full cleanliness detects it and fails closed with an actionable diagnostic; cleanup remains an explicit repository-owner action.

## Acceptance Criteria

- Running declared Acceptance gates creates no file or directory in the target repository or managed worktree except the intended repository deliverables already produced by Apply or Archive.
- Gate envelopes, bounded output, and manifests resolve below the configured/default Conflux state root and are isolated by project, workspace, and change identity.
- An unavailable or unwritable Acceptance state path yields a typed runtime hold before gate execution and never falls back to `.cflx` or another target path.
- Cleanliness checks evaluate all staged, unstaged, untracked, and ignored target paths relevant to legacy Conflux evidence; no Conflux-owned path is excluded.
- Exact valid external gate evidence remains reusable. Missing, malformed, stale, mismatched, or deleted external evidence reruns the declared gate or returns its ordinary typed hold and never proves PASS.
- External manifests and evidence do not control retry admission, queue intent, scheduler dispatch, archive, merge, or next-action selection.
- An unchanged non-resumable hold may be refused within the same live owner from typed reducer state. After owner restart, explicit retry is admitted as one fresh bounded attempt without requiring a repository mutation.
- Existing bounded Acceptance deadline, process cleanup, closed declared-gate allowlist, semantic verdict, and `client wait` terminal projection remain unchanged.
- Legacy `.cflx/verification-evidence` content is not read, written, hidden, or automatically deleted by the new runtime.

## Explicit Completion Conditions

- `src/config/defaults.rs` provides and tests the external Acceptance state-root resolver, including configured root, platform/XDG root, project/workspace isolation, safe identifiers, and external temporary fallback.
- `EvidenceStore` and `ManifestStore` receive explicit external storage paths; their APIs no longer derive storage by joining a target workspace with `.cflx/verification-evidence`.
- Manifest build, gate reuse planning, gate execution, terminal diagnostics, settlement cleanup, and all operator paths use the same resolved external store.
- Production code contains no writer for `.cflx/verification-evidence`, no self-ignore creation, and no cleanliness function that excludes an evidence path.
- Admission tests prove live reducer refusal without durable-manifest authority and restart admission for one fresh bounded attempt.
- Regression tests inspect both the external store and target tree, test state-root failure before command launch, prove cache deletion causes rerun rather than PASS, and prove legacy ignored evidence makes the target fail full cleanliness.
- `cargo test acceptance_execution_boundary --lib` passes.

## Out of Scope

- Automatically deleting legacy `.cflx/verification-evidence` from user repositories or worktrees.
- Changing repository-local `.cflx.jsonc`, `.gitignore`, `.git/info/exclude`, formatter ignores, or lint exclusions.
- Migrating old target-local evidence into the external cache.
- Making external state authoritative for workflow progress, retry, archive, merge, or completion.
- Changing the declared verification metadata schema or the semantic Acceptance verdict contract.

## Verification Ownership

`src/parallel/tests/acceptance_execution_boundary.rs` and its focused Rust target own the end-to-end boundary: external storage, zero target writes, full cleanliness, cache-loss rerun, live-owner retry refusal, restart admission, deadline behavior, and process cleanup. Repository-wide checks remain owned by tracked commit hooks.

## Final Validation

Archive validation is authoritative: `cflx openspec validate relocate-acceptance-evidence --archive-gate`.
