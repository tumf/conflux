# Design

## Producer

`AcceptanceProvenance` is built in `src/parallel/executor.rs` by a single function called at every settlement with the typed inputs the executor already holds:

```text
build_provenance(manifest: Option<&AcceptanceExecutionManifest>, gates: Option<&GateResults>, verdict: &SettlementKind, reviewer_started: bool, divergence: CandidateDivergence) -> AcceptanceProvenance
```

`SettlementKind` is a closed enum mirroring the `reason_code` table; mapping from `GateVerdict`/`AcceptanceResult` to it is exhaustive (`match` with no wildcard), so a new result variant fails to compile rather than silently publishing `null`. The deciding gate is `gates.first_failure()` / the first `Refused(tool_unavailable)` / the first defect, read from the typed outcomes, never from summary strings.

`proposal_content_id` comes from `git rev-parse <candidate_commit>:openspec/changes/<id>/proposal.md` through the existing `RepositoryFacts` trait (one new method with a Git-backed and an in-memory implementation). Divergence resolves `review_base_ref` with the same facts.

## Carrier

The record travels on the existing acceptance settlement events into `OrchestratorState`'s per-change acceptance evidence (process-local, cleared on restart) and is projected to `ChangeResource.acceptance_provenance`. It is deliberately not part of the retry classifier's inputs; the non-authoritative test constructs two states that differ only in this field and asserts identical classification.

## Redaction by construction

The record type has no `argv` and no `env` map; it holds `program`, `argument_count`, and `env_names`. There is nothing to redact at render time because the value never enters the type. The sentinel tests exist to keep it that way.

## TUI

`open_error_details_popup` accepts `error`, `stalled`, and `blocked`-external rows; `ErrorDetailsPopup` gains `provenance: Option<AcceptanceProvenance>`; `error_details_popup_lines` appends the fixed line set; the copy text appends the same lines after the two existing ones so the clipboard shape stays stable for existing consumers. The `Enter: details` hint follows the same availability.

## Dependencies

`declaration_invalid`/`defect_code` and the no-reviewer `fail` route exist only after `fix-required-verification-fail-closed`; `env_names` and the redacted request exist only after `fix-rerun-leading-env`. Both are repository outputs this producer reads.

## Verification strategy

- `--lib` (`acceptance_provenance`): boundary/executor tests with the counting runner double and an in-memory `RepositoryFacts` for blob and base resolution; router projection tests; TUI render/key tests; non-authoritative routing tests; sentinel-absence tests over record, row, popup, copy text, and log line.
- `tests/client_cli_tests.rs` (`acceptance_provenance`): isolated owner fixture driven by real reducer events; `cflx client status --json`; `cflx openapi`.
- Heavy tests are `#[cfg_attr(not(feature = "heavy-tests"), ignore)]`.
