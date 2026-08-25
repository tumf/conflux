## Implementation Tasks

- [ ] Add the stable non-success client outcome for a final/manual-action change status, with an observed-status detail and existing envelope/exit-code guarantees. (verification-id: client-wait-terminal-release)
- [ ] Classify coherent wait observations into continue, repository-certify, existing typed failure, or immediate final/manual-action release without submitting commands. (verification-id: client-wait-terminal-release)
- [ ] Add compiled-CLI tests for initial and transitioned `error`, `merge wait`, `stopped`, `rejected`, and `merged` observations, plus one automatically progressing HOLD case. (verification-id: client-wait-terminal-release)
- [ ] Update CLI documentation for HOLD versus immediate release behavior and the new outcome. (verification-id: client-wait-terminal-release)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate release-client-wait-terminal-status --archive-gate`.
