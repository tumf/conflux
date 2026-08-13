## Implementation Tasks

- [ ] Restrict callback destinations to literal IPv4 or IPv6 loopback and reject `localhost` or any hostname before connection. (verification-id: opencode-local-boundary) (verification: integration - `cargo test --test opencode_auto_resume_example literal_loopback`)
- [ ] Make callback state owner-private and reject symlink, non-directory, foreign-owned where testable, or non-`0700` paths before creating claims or markers. (verification-id: opencode-local-boundary) (verification: integration - `cargo test --test opencode_auto_resume_example private_state`)
- [ ] Make `extractBinding` fail closed on unsupported schema versions, non-admission outcomes, and non-string or empty binding IDs. (verification-id: opencode-local-boundary) (verification: integration - `cargo test --test opencode_auto_resume_example enqueue_envelope`)
- [ ] Preserve completion delivery, owner-restart fallback, redirect refusal, dedupe, retry, and automation-marker behavior. (verification-id: opencode-local-boundary) (verification: integration - `cargo test --test opencode_auto_resume_example`)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate harden-opencode-callback-local-boundary --archive-gate`
