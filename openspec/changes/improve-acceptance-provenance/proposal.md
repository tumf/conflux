---
change_type: implementation
priority: high
dependencies:
  - fix-required-verification-fail-closed
  - fix-rerun-leading-env
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/observability/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/tui-key-hints/spec.md
  - openspec/specs/tui-error-handling/spec.md
  - src/parallel/executor.rs
  - src/orchestration/acceptance/gate_execution.rs
  - src/web/remote_control_api/dto.rs
  - src/tui/state.rs
verifications:
  - id: improve-acceptance-provenance-local
    requirement: Every Acceptance settlement produces one typed provenance record from manifest and verdict facts, published additively on the change row and in the TUI details popup, with identities and counts only, never values, and never used as a routing input
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/executor.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests acceptance_provenance -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests acceptance_provenance -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: improve-acceptance-provenance-cli
    requirement: The real cflx client observes the published provenance for a native pre-reviewer stop and a reviewer verdict through the isolated owner, and the OpenAPI contract documents the additive nullable field
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/client_cli_tests.rs
    evidence: cargo test --test client_cli_tests --features web-monitoring,heavy-tests acceptance_provenance -- --nocapture
    rerun: cargo test --test client_cli_tests --features web-monitoring,heavy-tests acceptance_provenance -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Expose acceptance decision provenance and candidate identity

**Change Type**: implementation

## Problem

When Acceptance stops, the operator sees a sanitized `error_detail`, a `blocker`, or a `stalled` row. None of them says *who* decided: the runtime-owned gate phase before any reviewer existed (`GateVerdict::DeclarationInvalid`, `Failed`, `ExternalPrerequisite`, `Hold` in `src/orchestration/acceptance/gate_execution.rs`) or the semantic reviewer's verdict. Nothing says which candidate commit and which `proposal.md` content were judged, so "is the candidate still carrying the old proposal?" is answered by opening the worktree by hand. Logs are bounded and are not — and must not become — full prompt or raw response storage.

## Supported result

### One typed provenance record per Acceptance settlement

At every Acceptance settlement point in `src/parallel/executor.rs` — the four gate-phase verdicts, the boundary-derivation holds, and every reviewer result (`Pass`, `Fail`, `Stalled`, `PermissionStalled`, `CommandFailed`, `RuntimeLimit`, `ExecutionHold`, missing verdict, `Cancelled`) — the executor builds one `AcceptanceProvenance` record (name non-normative) from typed facts it already holds: the frozen manifest, the gate outcomes, the typed verdict. It never parses findings prose, log text, or reviewer output to fill a field.

Fields (all additive; `null` where unknown; no field ever holds a command argument value, an environment value, prompt text, or process output):

| Field | Meaning |
| --- | --- |
| `decision_source` | `native_verification` (settled by the runtime before any reviewer) or `reviewer` |
| `reviewer_started` | `true` only when a reviewer subprocess was actually spawned |
| `reason_code` | one closed token: `declaration_invalid`, `gate_failed`, `tool_unavailable`, `gate_deadline_exhausted`, `state_path_unavailable`, `legacy_target_evidence`, `manifest_malformed`, `runtime_defect`, `reviewer_pass`, `reviewer_fail`, `reviewer_stalled`, `reviewer_permission_stalled`, `reviewer_command_failed`, `reviewer_missing_verdict`, `review_deadline_exhausted`, `review_cleanup_unproven`, `cancelled` |
| `defect_code` | the planner's declaration defect code when `reason_code` is `declaration_invalid`; else `null` |
| `verification_id` | the deciding gate or defective declaration; `null` for reviewer decisions and non-gate holds |
| `declaration_path` | `openspec/changes/<id>/proposal.md` when `verification_id` is set; else `null` |
| `candidate_commit` | full manifest `candidate_commit_oid`; `null` only when no manifest exists (non-repository workspace) |
| `proposal_content_id` | Git blob ID of the candidate `proposal.md` at `candidate_commit`; `null` when unobservable |
| `change_input_digest` | the manifest's digest; `null` without a manifest |
| `program` | resolved executable path of the deciding gate, or its unresolved first token when `tool_unavailable`; `null` otherwise |
| `argument_count` | number of arguments after the program for the deciding gate; `null` otherwise |
| `env_names` | the deciding gate's environment binding names (from `fix-rerun-leading-env`); never values |
| `remediation` | one of `fix_declaration`, `satisfy_environment`, `repair_candidate`, `retry`, `refresh_candidate`, `none` |
| `candidate_divergence` | `{status: identical \| diverged \| unknown, source_ref, source_commit, source_blob, candidate_blob, reason}` |

Remediation is a pure table over `reason_code` and divergence: `declaration_invalid` → `fix_declaration`, or `refresh_candidate` when `candidate_divergence.status` is `diverged`; `tool_unavailable` → `satisfy_environment`; `gate_failed`, `reviewer_fail` → `repair_candidate`; every hold and deadline code, `reviewer_missing_verdict`, `reviewer_command_failed`, `reviewer_permission_stalled`, `reviewer_stalled`, `cancelled` → `retry`; `reviewer_pass` → `none`.

Divergence is computed only from an explicit comparison the owner already has: the manifest's bound `review_base_ref` resolved to `review_base_commit`. When both resolve and the base contains `openspec/changes/<id>/proposal.md`, the two blob IDs are compared; otherwise `status: unknown` with a `reason` (`no_manifest`, `base_unresolvable`, `source_proposal_missing`, `candidate_proposal_missing`). No branch name is guessed and no remote is contacted.

### Where it is published

- **Owner state.** The record is attached to the change's in-memory acceptance evidence for the current owner incarnation. It is process-local, is `null` after restart, and is never written under the state root as workflow input; the acceptance `diagnostics.json` the runtime already writes MAY carry the same record as a human-readable copy.
- **API.** `ChangeResource.acceptance_provenance` (nullable, additive, documented in the generated OpenAPI with every token above); existing fields are unchanged. `cflx client status --json` and `cflx client wait` detail projections that already copy change-row facts include it verbatim.
- **TUI.** The details popup opened with `Enter` renders, after the existing `Change:` and `Error:` lines, a `Decision:` line (`native_verification (reviewer not started)` or `reviewer`), `Reason:` (`reason_code` and `defect_code`), `Verification:` (`verification_id` and `declaration_path`), `Candidate:` (first 12 hex of `candidate_commit` and of `proposal_content_id`), `Command:` (`program`, `argument_count` arguments, `env: A, B`), `Next:` (`remediation`), and `Source proposal:` (`identical`, `diverged from <source_ref>@<12 hex>`, or `unknown (<reason>)`). The popup — and its `Enter: details` hint — becomes available for `error`, `stalled`, and external-`blocked` rows, because the native pre-reviewer stops this change exists to explain settle as `blocked` (`tool_unavailable`) and `stalled` (holds) rather than `error`. The copy text keeps the two existing lines first and appends the same lines.
- **Log.** One structured settlement log line per Acceptance invocation carries the same fields; it is observability only.

### What stays out

No raw prompt, reviewer stdout/stderr, gate artifact contents, argument values, or environment values are stored or rendered; `argument_count` and `env_names` replace argv on purpose because a declared command can carry secrets outside assignments. Routing is untouched: retry eligibility, `permits_acceptance_retry`, admission, archive, merge, and next-action selection do not read the record, and deleting logs, `diagnostics.json`, or the whole state root cannot change the next action. Ordinary retry, candidate content, and every existing API field keep their meaning.

## Change boundary

- `src/parallel/executor.rs` (producer at each settlement), `src/orchestration/acceptance/{execution_manifest,gate_execution}.rs` (typed accessors for the deciding gate and defect), `src/orchestration/state.rs` and `src/events.rs` (carrying the record on the settlement event and change evidence), `src/web/remote_control_api/{dto,projection}.rs` (field and OpenAPI), `src/client/control.rs` (pass-through), `src/tui/{state,render,key_handlers}.rs` (popup lines, copy text, availability), the settlement log line.
- Spec deltas: ADDED in `remote-control-api` and `observability`; MODIFIED `Error Details Key Hint` in `tui-key-hints` carried in full; ADDED in `tui-error-handling`.
- Depends on `fix-required-verification-fail-closed` (the `declaration_invalid` reason, `defect_code`, and the no-reviewer `fail` route) and `fix-rerun-leading-env` (`env_names` and the redacted request representation).

## Acceptance

1. **Native stop vs reviewer verdict.** On a disposable repository through the production boundary and executor with a counting runner double: a defective required declaration settles with `decision_source: native_verification`, `reviewer_started: false`, `reason_code: declaration_invalid`, the planner's `defect_code`, the verification ID and declaration path, `candidate_commit` equal to the manifest's, `proposal_content_id` equal to `git rev-parse HEAD:openspec/changes/<id>/proposal.md`, and `remediation: fix_declaration`; a missing executable settles `tool_unavailable` / `satisfy_environment` with `program` naming the unresolved token; a failing gate settles `gate_failed` / `repair_candidate` with `argument_count` and `env_names`; a reviewer `fail` and a reviewer `pass` settle with `decision_source: reviewer`, `reviewer_started: true`, `verification_id: null`, and `repair_candidate` / `none`. The same record is observed on the `/api/v2` change row through the production router and in the TUI popup lines.
2. **Divergence.** With a bound review base whose `proposal.md` differs from the candidate's, `candidate_divergence.status` is `diverged` with both blob IDs and the base ref/commit, and a `declaration_invalid` settlement recommends `refresh_candidate`; with identical blobs it is `identical`; with no manifest, an unresolvable base, or a missing blob it is `unknown` with the frozen reason and nothing is guessed.
3. **Bounded, redacted, non-authoritative.** With a sentinel in an argument and a sentinel in an assignment value, neither appears in the record, the API row, the popup lines, the copy text, or the settlement log line, while `env_names` and `argument_count` are present. With the record present versus absent (and with the state root deleted), retry eligibility, admission classification, and the next action for the same workspace are identical. A TUI rendering regression test renders the popup for `error`, `stalled`, and external-`blocked` rows and asserts the `Enter: details` hint appears only for those.
4. **Real client.** `tests/client_cli_tests.rs` drives the isolated owner fixture's real reducer with a native-stop settlement and a reviewer settlement and asserts `cflx client status --json` publishes `acceptance_provenance` with the frozen tokens, that a change without a settlement publishes `null`, and that `cflx openapi` documents the field and every token.
5. **Gating.** Every new test name contains `acceptance_provenance`; both declared commands select at least one test (zero selected is a failing gate); tests over one second are `#[cfg_attr(not(feature = "heavy-tests"), ignore)]`; no test writes into a target repository or managed worktree.
6. **Spec and docs coherence.** The MODIFIED `Error Details Key Hint` requirement is carried in full with only the extended availability; the ADDED requirements archive cleanly; `docs/guides/VERIFICATION_EVIDENCE.md` gains a short "Reading a settlement" paragraph naming the field and tokens. Strict validation of this change is not implementation evidence.

## Delivery boundary

Do not push, release, replace a live owner, edit global or project configuration, or touch the original scope-drill incident candidate. Work only in the Conflux-managed candidate worktree of this independent develop-based repository; keep `main` untouched. Do not mark any task complete from prose, from a zero-test gate, or from an authoring-time strict validation pass.
