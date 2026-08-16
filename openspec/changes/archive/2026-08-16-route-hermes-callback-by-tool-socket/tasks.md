## Implementation Tasks

- [x] Add one shared `RouteSelector`/connection resolver used by client CLI and MCP: absolute `project_dir`, explicit `unix_socket`, or default; reject two call-scoped selectors before contact, let one call-scoped selector override namespace defaults, and derive both canonical repository root and Git-common-dir socket from the selected project. Add `cflx client --project-dir <ABSOLUTE_PATH>` with clap-level conflict against `--unix-socket`. (verification-id: project-scoped-client-routing) (verification: unit - `cargo test --lib client`)
- [x] Expose `project_dir` on all six MCP client tools and route every operation through the shared selector without adding a stable-envelope outcome, project registry, owner startup, or change-ID inference. (verification-id: project-scoped-client-routing) (verification: integration - `cargo test --features heavy-tests --test client_mcp_integration`)
- [x] Make truthful `cflx_wait` evidence certification use the repository root derived from the selected project. Add a regression where MCP server CWD is project A, project B is selected, both contain the same change ID with different evidence, and only B may certify completion. (verification-id: project-scoped-client-routing) (verification: integration - `cargo test --features heavy-tests --test client_mcp_integration`)
- [x] Update the Hermes post-tool hook to preserve the admitted enqueue call's `project_dir` or low-level `unix_socket`, invoke `cflx client --project-dir ... notify set` or its socket equivalent with complete instance/execution/change binding, use `CFLX_UNIX_SOCKET` only when the host exposes no post-tool arguments object, and keep malformed/conflicting/unresolved routes observational. (verification-id: project-scoped-client-routing) (verification: integration - `cargo test --features heavy-tests --test hermes_auto_resume_example plugin_routes_registration_by_call_scoped_project`)
- [x] Add integration coverage using two independent repositories plus a linked worktree to prove per-call owner affinity, canonicalized directory handling, relative/bare/non-repository refusal, call-versus-namespace precedence, mutual-exclusion refusal with no contact, low-level socket compatibility, default-CWD behavior, wait evidence isolation, and callback registration against the enqueue-selected owner. The unreachable-after-MCP-conflict hook defense may use a synthetic admitted-result fixture. (verification-id: project-scoped-client-routing) (verification: integration - `cargo test --features heavy-tests --test hermes_auto_resume_example && cargo test --features heavy-tests --test client_mcp_integration`)
- [x] Update the Hermes auto-resume README, MCP tool descriptions, embedded Conflux operation skill, and `AGENTS.md` client section to present absolute `project_dir` as the normal public selector, `unix_socket` as a low-level override, truthful wait evidence selection, and one global MCP registration with no fixed project socket. (verification-id: project-scoped-client-routing) (verification: integration - `cargo test --features heavy-tests --test hermes_auto_resume_example documentation_describes_project_scoped_routing`)

## Notes

- Base-branch reconciliation: `main` carried the reviewed supersession of this
  change (project-directory routing) in commits 37cf0d41, 528dfb65, 5cc2a7c3.
  `main` was merged into this branch so the workspace change artifacts are the
  reviewed revision: `proposal.md` now declares the `project-scoped-client-routing`
  verification, `specs/cli/spec.md` is present, and
  `specs/external-lifecycle-integrations/spec.md` states project directory as the
  normal public selector. The task list below is `main`'s reviewed list; the three
  pre-review socket-only tasks it replaced are subsumed by tasks 4-6.
- Verification ownership: the reviewed list marks tasks 4 and 6 `unit`, but their
  named evidence drives `python3` subprocesses, temporary-directory fixtures, and
  fake `cflx`/`hermes` executables. That is integration evidence under this
  project's unit-test boundary policy, so the markers are recorded as `integration`
  and the genuinely unit-scoped route-selector logic is covered separately by
  `cargo test --lib client` in task 1. No planned coverage was dropped.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate route-hermes-callback-by-tool-socket --archive-gate`

Run against the artifacts synced from `main` (merge commit `acc52d53`), with the
worktree-built binary rather than the installed release:

- `cflx openspec validate route-hermes-callback-by-tool-socket --strict`: passed.
- `cflx openspec validate route-hermes-callback-by-tool-socket --archive-gate`: passed.
- `cargo fmt --check`: clean.
- `cargo clippy --all-targets --features heavy-tests -- -D warnings`: clean.
- Reviewed rerun suite for `project-scoped-client-routing`:
  - `cargo test --features heavy-tests --test hermes_auto_resume_example`: 20 passed, 0 failed.
  - `cargo test --features heavy-tests --test client_mcp_integration`: 15 passed, 0 failed.
  - `cargo test --lib client`: 86 passed, 0 failed.
- `cargo test` (default suite): 0 failed across every target.
- `git merge-tree --write-tree main HEAD` reports no conflict.

Promotion-preservation check (MODIFIED deltas vs canonical):

`merge_spec_delta` replaces the whole canonical `### Requirement` block with the
delta block, so every canonical scenario that must survive archive has to appear
in its delta block or be named as a one-for-one supersession. Scenario titles
under each modified requirement were compared canonical-vs-delta:

- `cli` / **Existing-owner client MCP namespace**: canonical 5, delta 13,
  unintended drops 0. The four previously omitted canonical scenarios (`MCP
  stdout remains protocol-only`, `Raw workflow commands are not exposed`, `Tool
  calls require initialization`, `Non-JSON-RPC request is rejected`) are now
  copied forward verbatim; `MCP enqueues into the existing TUI` is carried as the
  reworded selected-repository variant.
- `cli` / **Direct client completion notification management**: canonical 6,
  delta 7, unintended drops 0. `Empty callback command is rejected before owner
  access`, `Operator inspects and clears one callback`, `Expected owner
  incarnation changed`, `TCP cannot mutate callback registration`, and
  `Installed operation skill teaches the direct CLI path` are copied forward
  verbatim. The single intended supersession is `Operator registers one callback
  from the shell` → `Operator registers one callback for an explicit project`,
  one-for-one; the superseding scenario now also carries `--blocked` so the
  canonical blocked-event delivery assertion is not lost. The requirement text
  restores the canonical mandate that the embedded skill documents the direct CLI
  commands as the default shell-facing path.
- `external-lifecycle-integrations` / **Reference Hermes completion callback
  notifies the bound messaging thread safely**: canonical 7, delta 12,
  unintended drops 0. `Unsupported enqueue result fails closed`, `Callback posts
  a responder-compatible Slack bot message`, `Hermes host wrapper yields the
  admitted envelope`, `Concurrent turns retain request-scoped routing`, `Missing
  or non-messaging routing context registers nothing`, and `Scrubbed callback
  environment is reconstructed explicitly` are copied forward verbatim;
  `Admitted enqueue registers the originating messaging thread` is carried as the
  project-routed variant. The requirement text restores the canonical
  separate-responder sentence.

Re-validated against the updated deltas with the worktree-built binary
(`/Volumes/OWCUS4EXP1M2/mini-data/work-cache/rust-target/default/debug/cflx`):

- `cflx openspec validate route-hermes-callback-by-tool-socket --strict`: passed (exit 0).
- `cflx openspec validate route-hermes-callback-by-tool-socket --archive-gate`: passed (exit 0).
