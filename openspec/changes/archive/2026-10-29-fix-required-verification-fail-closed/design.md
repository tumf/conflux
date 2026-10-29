# Design

## Where the leak is

`eligible_gates` (`src/orchestration/acceptance/execution_manifest.rs`) is `filter(is_eligible_declaration) → filter_map(eligible_request(..).ok()) → sort → dedup_by(id)`. Every `Err` from `eligible_request` disappears at the first filter, and every duplicate ID disappears at the last step. Both losses are invisible to `build_manifest`, which then freezes an empty or shortened allowlist and hands `GateVerdict::Proceed` to the reviewer launch in `src/parallel/executor.rs`.

## Shape of the fix

Replace selection with a planner that separates *classification* from *eligibility*:

```text
plan_declared_gates(&[VerificationDeclaration]) -> DeclarationPlan { gates: Vec<VerificationRequest>, defects: Vec<DeclarationDefect> }
DeclarationDefect { verification_id: String /* or "<position N>" */, position: usize, field: &'static str, code: &'static str, detail: String }
```

Rules in order: duplicate IDs across the whole list → required-ness from `completion_role` → role consistency (`phase`, `execution_class`) → identity → command tokenization → automation path. Codes reuse the existing `IneligibleReason` strings (`declaration_missing_id`, `declaration_unstorable_id`, `declaration_missing_command`, `declaration_shell_syntax`, `declaration_missing_automation`) and add `declaration_duplicate_id` and `declaration_role_conflict`. `IneligibleReason::ShellSyntax` stops carrying the whole command and carries the offending construct instead, so the detail never echoes a value that the later leading-environment change will need to redact.

`eligible_request` keeps its signature for the reuse-decision path (`decide_reuse`, `plan_reuse`), which still needs a per-declaration answer; it is no longer used for gate *selection*.

## Consumers

- **Manifest.** `AcceptanceExecutionManifest` gains `declaration_defects: Vec<DeclarationDefect>` (serialized, part of the fingerprint through the canonical JSON). `build_manifest` always builds; it never maps a defect to `ManifestBuildError`, because that path becomes `acceptance_manifest_malformed`, which is a runtime hold and the wrong classification.
- **Gate phase.** `execute_declared_gates` is not entered when `manifest.declaration_defects` is non-empty. `GateVerdict` gains `DeclarationInvalid { defects }`, classified *before* `tool_unavailable` because a defective declaration was never resolved against PATH. The executor routes it exactly like `GateVerdict::Failed`: record an acceptance attempt with `commit_hash`, findings with leading code `[DECLARATION_INVALID]`, emit the existing failure log/completed events, return `AcceptanceResult::Fail`. No hold is recorded (`record_acceptance_hold` is not called), so the in-memory admission guard never refuses a corrected retry, and the normal rejection/repair cycle can fix `proposal.md`.
- **`cflx openspec verify`.** `cmd_verify` runs the planner first. Defects → report (`declaration_defects` in JSON, one line each in text), execute nothing, exit `1`. Otherwise unchanged.
- **`cflx openspec validate`.** `validate_verification_declarations` runs the planner after the existing per-field checks and appends one error per defect not already reported by the existing checks (dedupe on `(id, field, code)` is allowed; the requirement is "reported at least once").

## Why `fail` rather than a hold

A hold means "the runtime could not complete the boundary"; a defect means "the candidate's own declaration is wrong". The candidate is what the rejection cycle repairs, `proposal.md` is in the candidate, and a verdict leaves the admission guard untouched. A hold would also refuse an unchanged retry, which is correct for a deadline but hostile for a one-character YAML fix. Routing as `Fail` keeps every existing retry, history, and display contract intact.

## Verification strategy

- Planner unit tests on `src/orchestration/acceptance/execution_manifest/tests.rs` use real `VerificationDeclaration` values, not doubles of the planner.
- Validation and verify tests call the production `validate` and `cmd_verify` paths on temp repositories; the real-binary tests in `tests/run_exit_tests.rs` set the state root (`XDG_STATE_HOME` or `state_base_dir`) to a temp directory outside the repository and use a marker-writing fixture command to prove nothing ran.
- Boundary tests extend `src/parallel/tests/acceptance_execution_boundary.rs` with the existing counting runner double so "no reviewer launched" is a count, not a log grep.
- Test names contain `required_verification_fail_closed`; anything over one second is `#[cfg_attr(not(feature = "heavy-tests"), ignore)]`.
