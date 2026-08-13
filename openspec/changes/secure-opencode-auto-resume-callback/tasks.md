## Implementation Tasks

- [x] Resolve and validate the final callback URL as the same origin as a loopback-HTTP base, reject absolute/protocol-relative/backslash origin changes, disable redirect following, and prove a second redirect-target listener receives no connection. (verification-id: opencode-callback-hardening) (verification: integration - `cargo test --test opencode_auto_resume_example`)
- [x] Replace check-then-write dedupe with an atomic in-flight claim, atomic success-marker promotion, and a five-minute stale-claim takeover. (verification-id: opencode-callback-hardening) (verification: integration - `cargo test --test opencode_auto_resume_example`)
- [x] Release the in-flight claim on POST failure and deterministically verify with pre-created fresh/stale claims that fresh in-flight refusal is non-zero, stale takeover retries, success-marker dedupe is zero, and no timing race is required. (verification-id: opencode-callback-hardening) (verification: integration - `cargo test --test opencode_auto_resume_example`)
- [x] Update the example README to state loopback same-origin/redirect, failed-delivery retry, stale-claim, and normal/crash delivery semantics; use `127.0.0.1` in examples and do not promise exactly-once or durable owner-crash delivery. (verification-id: opencode-callback-hardening) (verification: unit - `python3 -c "from pathlib import Path; p=Path('examples/integrations/opencode-auto-resume/README.md').read_text(); assert all(x in p.lower() for x in ('redirect', 'retry', 'stale', 'at-least-once'))"`)

## Final Validation

Expected archive gate: `cflx openspec validate secure-opencode-auto-resume-callback --archive-gate`
