## Implementation Tasks

- [ ] Add initial-observation classification that returns typed `change_not_found` for an absent requested change without submitting commands. Preserve known `not queued` and later-disappearance behavior. (verification: integration - `cargo test --test client_cli_tests wait_refuses_unknown_change`; verification-id: client-wait-unknown-change)

- [ ] Add CLI regression coverage proving an unknown target exits with outcome `change_not_found` and status `9` before a short positive timeout, while a known `not queued` target still reaches `timeout` rather than `change_not_found`, and an absent target whose repository evidence certifies completion still returns `completed`. (verification: integration - `cargo test --test client_cli_tests wait_refuses_unknown_change`; verification-id: client-wait-unknown-change)

## Final Validation

Archive validation is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-client-wait-unknown-change --archive-gate`
