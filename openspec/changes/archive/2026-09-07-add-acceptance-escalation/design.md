# Design: bounded Acceptance escalation

## Decision

Acceptance escalation is an alternate reviewer command, not a new lifecycle phase. Shared orchestration classifies invalid completed results and chooses the command mode for the next Acceptance-only invocation. `AgentRunner` only resolves the selected template and builds the existing prompt contract.

## State

Keep this active-run state per change:

- consecutive invalid-result count;
- escalation uses in the current sequence;
- pending command mode for the next retry.

Do not persist it. Any completed non-invalid result clears the sequence. A process restart starts ordinary Acceptance from workspace and Git evidence.

## Eligibility

Eligible results:

- `MissingVerdict`;
- `BareBlocker`;
- `MalformedFinding`;
- FAIL whose parsed finding list is empty and would otherwise become only `Investigate acceptance failure and apply the required fix`.

Excluded results:

- PASS;
- FAIL with at least one actionable finding;
- CONTINUE;
- validated external blocker;
- permission-stalled hold;
- command failure;
- runtime limit;
- cancellation.

Command failure keeps its existing command-recovery budget because it describes transport or process failure, not reviewer output quality. Actionable FAIL keeps existing FAIL-to-Apply routing.

## Selection

After `after_invalid_results` consecutive eligible results, the next permitted Acceptance protocol retry uses `acceptance_escalation_command` while `max_uses_per_sequence` remains. The command receives the same generated prompt and placeholders as the normal command. Existing corrective context identifies the invalid contract; add only trusted framing that an alternate reviewer must evaluate current evidence and emit one fresh canonical verdict.

If no escalation command is configured or its per-sequence cap is spent, use the existing normal retry/terminal routing. Escalation does not add retries beyond existing protocol budgets. Empty-FAIL has no pre-existing Acceptance-only retry: its alternate-review retry is bounded solely by the escalation use cap. Below threshold, after cap exhaustion, or without an escalation command, empty-FAIL follows the existing generic FAIL-to-Apply fallback while retaining its invalid-result sequence across the intervening Apply round on the same revision. A completed non-invalid Acceptance result, revision change, or terminal outcome resets that sequence.

## Boundaries

- Configuration owns templates and positive numeric validation.
- Shared Acceptance orchestration owns classification, counters, reset, eligibility, and diagnostics.
- AgentRunner owns template expansion and command execution only.
- Serial and parallel frontends consume shared decisions; neither reimplements policy.

## Rejected alternatives

- Escalate every semantic FAIL: rejected because disagreement about a real defect is not malformed reviewer output and must remain repair work.
- Use `apply_escalation_command`: rejected because it is an Apply worker selected by empty-WIP state, not a read-only reviewer.
- Persist escalation state: rejected by the workspace-local workflow constitution and unnecessary because restart safely re-runs Acceptance.
- Add an unbounded independent retry loop: rejected because escalation must consume existing protocol retry opportunities.
