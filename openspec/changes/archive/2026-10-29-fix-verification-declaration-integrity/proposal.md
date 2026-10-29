---
change_type: implementation
priority: high
dependencies:
  - fix-required-verification-fail-closed
  - fix-rerun-leading-env
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/proposal-metadata/spec.md
  - src/openspec.rs
  - src/openspec_cmd/validation.rs
  - src/openspec_cmd/verify.rs
  - src/orchestration/acceptance/manifest_builder.rs
  - src/orchestration/acceptance/execution_manifest.rs
  - src/orchestration/acceptance.rs
  - src/acceptance.rs
  - src/parallel/executor.rs
verifications:
  - id: declaration-integrity-local
    requirement: Declaration decoding and normalized duplicates fail closed and native per-defect identities allow distinct repairs without resetting repeated-defect protection
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/orchestration/acceptance/execution_manifest.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests verification_declaration_integrity -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests verification_declaration_integrity -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: declaration-integrity-cli
    requirement: Declaration decoding and normalized duplicates fail closed and native per-defect identities allow distinct repairs without resetting repeated-defect protection
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/run_exit_tests.rs
    evidence: cargo test --test run_exit_tests --features web-monitoring,heavy-tests verification_declaration_integrity -- --nocapture
    rerun: cargo test --test run_exit_tests --features web-monitoring,heavy-tests verification_declaration_integrity -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Preserve verification declaration integrity before and after planning

**Change Type**: implementation

## Problem and observed evidence

The first fail-closed change closes planner selection, but not its upstream reader or downstream repair identity. Independent static findings were verified by the parent against integrated `fd0d271310fe06485a19db179d999880a2f2f751`:

1. A required entry with `rerun: [cargo, test]` fails strict validation, yet the real binary's `verify --plan --json` and `verify --json` both exit 0 with `verifications: []`. `parse_proposal_metadata_from_file` turns whole-frontmatter decoding failure into an empty declaration list; native manifest construction uses that same tolerant reader. Whole-Acceptance false PASS was NOT demonstrated. This is a residual pre-existing parser hole.
2. `valid-gate` and `" valid-gate "` pass both real-binary strict and archive-gate validation while verify exits 1 with duplicate defects. The validator compares raw IDs and then suppresses all planner duplicate diagnostics as already reported.
3. The production planner, legacy normalization and repair ledger were exercised from the pinned compiled library. Repairing a duplicate exposes a different shell-syntax defect, but both normalize to `repository|code|[declaration_invalid]`; the second verdict returns `Stop(repeated_acceptance_finding)` before its first repair.

The prior 19 library and 3 CLI regressions pass but do not cover these boundaries. Reproduction scripts/logs are external investigation artifacts, NOT product deliverables.

## Scope and split decision

This one corrective change completes the same end-to-end declaration integrity contract: source decode -> shared gate plan -> consumer diagnostics -> native finding/repair. The typed source defects must flow through the same per-defect identity as planner defects; shipping only the new detection would immediately expose incorrect repair stopping. Keep these coupled fixes atomic. No general parser redesign, retry-policy change, candidate-refresh feature, or provenance UI redesign.

The dependency outputs actually consumed are the shared planner/defect channel from `fix-required-verification-fail-closed` and the typed argv/environment gate representation from `fix-rerun-leading-env`. Do not add milestone or release-only dependencies. Existing `improve-acceptance-provenance` and `add-candidate-proposal-refresh` may continue; do not edit or stop those managed worktrees. If they integrate first, preserve their resulting code and public behavior when resolving the base.

## Final behavior

### One fallible source-to-plan path

All execution/gating consumers load verification metadata through a fallible reader that preserves source status and diagnostics. The tolerant display/dependency APIs may remain for their existing non-execution callers, but neither verify nor Acceptance may derive an authoritative gate list from the tolerant default.

A readable ordinary proposal without frontmatter, without a `verifications` key, with a null `verifications` value (a bare key, which `Option<Vec<_>>` already decodes as absent and strict validation already accepts), or with `verifications: []` is legitimately empty. Malformed YAML, an opening frontmatter delimiter without a closing delimiter, a non-mapping non-null frontmatter root, a `verifications` value that is neither a sequence nor null, non-mapping declaration entries, wrong-typed declaration fields, and unknown verification keys are source defects, never empty success. Existing unknown top-level keys remain warnings. A structurally valid non-required command is still not inspected for supervisability. Do not infer required-ness from prose.

Source structural defects use stable code `declaration_metadata_invalid` on the existing `DeclarationDefect` type, with the trimmed verification ID or `<position N>` label and the offending field when one declaration is identifiable; otherwise the proposal-scoped label `<proposal>`, `position: 0`, and field `frontmatter` or `verifications`. `DeclarationDefect` gains one typed field, `key: DeclarationKey` (`Id(trimmed)`, `Position(N)`, or `Proposal`), fixed by the reader or planner at the point where the source still knows whether an `id` was authored, and built only through `DeclarationDefect::new`; `verification_id` and `position` become display projections of that key. An authored ID that is unstorable, or literally spelled like a fallback label such as `<position 3>`, is still `Id(...)`. The existing JSON fields are unchanged and an additive `key` member is added to each `declaration_defects` entry. Details disclose only shape, expected/found YAML type, and line/column, never field values, command arguments, environment values, YAML snippets, or the serde error text (which can embed values). Source read failures (missing or unreadable proposal) are a typed source-unavailable value, distinct from defects: `verify` reports it through its existing proposal-resolution error path and exits non-zero before resolving the store; native Acceptance returns a new `ManifestBuildError::ProposalUnreadable` variant that the executor's existing error arm already turns into the `ManifestMalformed` execution hold with an all-zero fingerprint, so the next attempt stays admissible and no gate, reviewer, evidence reuse, verdict, or `tool_unavailable` classification occurs. This reuses the current route for "no manifest could be derived"; it adds no hold category and leaves the implementer no routing choice. See design.md for the per-consumer outcome table.

Valid metadata then enters the existing shared planner through `plan_verification_source`. With any source or planner defect, no gate is runnable/reusable and the planner rules are not evaluated on a partially decoded list. Every execution consumer receives the same defects; no partial execution of the valid subset and no `--verification-id` bypass. Preserve legitimate empty and normal leading-env execution semantics.

### Validation parity

Validation's verification-metadata errors come from the same reader, so unclosed frontmatter and wrong-typed fields fail strict and archive-gate with a non-echoing diagnostic naming the change, the ID label, and the field; the raw serde decode diagnostic is not emitted for a proposal whose reader already reported defects. Validation's own untrimmed duplicate loop is removed and `declaration_duplicate_id` leaves the suppression list, so the planner's per-occurrence trimmed-ID duplicate defect is what strict and archive-gate report, including the `gate` / `" gate "` collision; all-whitespace IDs stay missing-ID errors. The other three suppressed codes remain suppressed because their per-field checks already name the verification and field. Text and JSON verify output retain the per-defect ID/position/field/code/detail; source-defective verify plan/run exit 1 and never read, create, or mutate evidence.

### Native verdict and repair identity

Structural metadata defects settle the native candidate boundary as `AcceptanceResult::Fail` before any gate or reviewer, with manifest defects and the candidate commit recorded, not as infrastructure/tool-unavailable. Keep the human `[DECLARATION_INVALID]` text exactly as the executor builds it today. Carry a typed runtime-native per-defect identity (`NativeDeclarationIdentity` on `AcceptanceFinding`, built by `AcceptanceFinding::declaration_defect` from the production `DeclarationDefect`) separately from prose through `normalize_findings` and the unchanged `FindingRepairLedger`; do not parse a more complicated prose prefix, change `normalize_legacy_texts`, or alter reviewer `RepositoryFinding` identity handling.

Identity consists of change ID, declaration key (trimmed ID, 1-based position, or proposal), field, and defect code, encoded as a `repository|declaration|` prefix plus a JSON array so user IDs cannot collide with fallback keys. The key is copied from the defect's own typed `key` field; it is never re-derived from the `verification_id` label, the `position` value, a prose prefix, or the defect code, so a user ID literally spelled `<position 3>` keeps a named identity at every position and a nameless entry keeps its positional one. Duplicate occurrences with the same key/field/code share one repair opportunity through the existing normalization dedup, while every complete finding stays in the recorded attempt and the Fail payload. A named declaration keeps its identity across list reordering, line-number changes, candidate commits, and wording changes. Distinct fields/codes/keys are distinct defects; fixing a duplicate then revealing shell syntax allows the newly exposed defect its first repair. The same still-open defect after its repair remains a repeated-finding stop. No durable ledger, budget increase/reset, config switch, or weakening of protection. Native declaration findings keep `declares_paths() == false` and no reviewer ID, so they never fabricate reviewer-authored file-coverage requirements; their JSON gains an observability-only `declaration_identity` object.

## Acceptance and completion conditions

All newly added test names contain `verification_declaration_integrity` and both declared commands select nonzero tests. At least:

1. Production reader/planner and all consumers: sequence-valued `rerun`, non-sequence non-null `verifications`, malformed YAML, missing closing delimiter, non-mapping root, non-mapping entry, wrong-typed field, unknown verification key, missing/unreadable proposal; named, positional, and proposal-scoped defects with `position: 0` for proposal scope; plain legacy proposal, absent key, null value, and empty list controls; non-required shell command and valid required leading-env controls. Source details with a sentinel value never echo that value.
2. Real binary in a disposable Git repository with HOME/config/state outside that repository: strict/archive-gate reject `gate` / `" gate "`; verify plan/run reject `rerun: [cargo, test]` and an unclosed frontmatter with exit 1, typed JSON `declaration_defects`, and a nonempty diagnostic, regardless of `--verification-id`. Pre-existing evidence remains byte-identical and neither a valid nor invalid command marker appears. Valid controls prove the fixture can actually run a gate and retain zero-gate semantics.
3. Native production boundary/executor with the counting reviewer runner: Apply-mutated candidate metadata (wrong-typed `rerun`, unclosed frontmatter) produces Fail, zero reviewer calls, no gate marker, no evidence read or write, manifest defects bound to the candidate commit. A valid candidate starts its gate and reviewer normally; a legitimately empty candidate proceeds to the reviewer. A missing proposal settles the `ManifestMalformed` hold with an all-zero fingerprint and a proposal-path evidence line, zero reviewer calls, and no verdict. An earlier base validation is never used as proof about the candidate.
4. Real production finding construction/normalization/repair-ledger transition through `AcceptanceFinding::declaration_defect` and `normalize_findings`: duplicate -> repair dispatched -> different shell-syntax defect yields Repair for the new identity; unchanged same defect yields Stop. Reordered named IDs and modified prose/line/candidate text preserve identity. Two distinct defects in one verdict remain distinct; two occurrences of one duplicate share one identity while both findings stay in the attempt. Through the production planner and reader: an authored `id` literally `<position 3>` (and one literally `<proposal>`) at position 3 with a `rerun` defect is keyed as a named ID, is distinct from a nameless entry at position 3 with the same field and code, and keeps that identity after being moved to position 1; a nameless entry is keyed by position; a proposal-scoped defect is keyed by proposal with `position: 0`; an unstorable authored ID is keyed by that ID; the verify JSON `key` member carries the same variant and the five existing JSON fields are byte-identical to today. Native findings report no coverage paths and no reviewer ID, and a structured reviewer finding in the same process keeps its reviewer-ID identity. Tests call the production construction path, not a test-only copy of the formatting.
5. Keep the 19+3 earlier regression behavior, leading-env command parsing/redaction, and legacy reviewer identities intact. Promote the complete MODIFIED requirement with all eight existing scenarios and the seven added ones. Add concise declaration-source and repair-identity guidance to `docs/guides/VERIFICATION_EVIDENCE.md`.

The tracked `.pre-commit-config.yaml` Rust gates are path-scoped, not unconditional on clean amend. Do not claim hooks alone prove the two focused commands ran. Completion requires implementation paths wired, focused gates with actual selected counts/exit codes, canonical spec coherence and clean tree. No global/full-suite success claim is implied.

## Out of scope and delivery boundary

No push, release, deployed binary replacement, owner restart, global/project config edit, main edit, original repository change, or scope-drill candidate operation. No hidden cache authority, automatic source-to-candidate update, implicit shell, arbitrary feature switch, or copied credential. Runtime artifacts stay outside target repositories/worktrees. Only the dedicated Conflux candidate may implement this proposal after review; proposal-authoring resources contain only the proposal.
