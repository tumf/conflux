## Implementation Tasks

- [ ] Add `change_requires_action` with stable exit status `27`, observed/error detail, zero-command detail, and envelope contract assertions. (verification-id: client-wait-terminal-release)
- [ ] Classify coherent wait observations into continue, repository-certify, existing typed failure, or immediate final/manual-action release without submitting commands. (verification-id: client-wait-terminal-release)
- [ ] Add compiled-CLI tests for initial and transitioned `error`, `merge wait`, `stopped`, `stalled`, `rejected`, and `merged` observations; HOLD coverage for `not queued`, `blocked`, and active status; and the one-retry merged evidence race. (verification-id: client-wait-terminal-release)
- [ ] Update CLI documentation for HOLD versus immediate release behavior and the new outcome. (verification-id: client-wait-terminal-release)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate release-client-wait-terminal-status --archive-gate`.
