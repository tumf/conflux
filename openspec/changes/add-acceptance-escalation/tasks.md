## Implementation Tasks

- [ ] Add optional `acceptance_escalation_command` and validated positive `acceptance_escalation.after_invalid_results` / `max_uses_per_sequence` configuration with item-wise precedence and defaults (verification: unit - `cargo test acceptance_escalation --lib`; verification-id: acceptance-escalation-tests)
- [ ] Refactor `AgentRunner` Acceptance launch to select the normal or escalation template through one prompt-construction and execution path, retaining `{change_id}`, `{prompt}`, portable skill guidance, bounded history, protocol retry, and command-recovery context (verification: unit - `cargo test acceptance_escalation --lib`; verification-id: acceptance-escalation-tests)
- [ ] Implement one shared active-run invalid-result sequence policy for `MissingVerdict`, `BareBlocker`, `MalformedFinding`, and exact generic empty-FAIL fallback, including threshold, use cap, reset, missing-command fallback, and diagnostics (verification: unit - `cargo test acceptance_escalation --lib`; verification-id: acceptance-escalation-tests)
- [ ] Wire serial and parallel managed-worktree Acceptance loops to consume the shared command-selection decision without rerunning Apply or cleanup-review (verification: unit - `cargo test acceptance_escalation --lib`; verification-id: acceptance-escalation-tests)
- [ ] Add focused regression tests proving semantic actionable FAIL exclusion, all eligible invalid classes, command selection, reset boundaries, empty-FAIL routing below threshold and across its Apply round, cap exhaustion, config merge/validation, and non-persistence across fresh runners (verification: unit - `cargo test acceptance_escalation --lib`; verification-id: acceptance-escalation-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-acceptance-escalation --archive-gate`
