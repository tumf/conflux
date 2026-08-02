## Implementation Tasks

- [ ] Add the minimum repository-local browser test harness and `make web-test` command, with fixtures or an in-process server for `/api/v2` bootstrap, authentication, state, event, command, worktree, log, and typed-error responses; keep production assets dependency-free and mark any test over one second as heavy. (verification: integration - `make web-test` must execute real DOM/browser assertions rather than only linting files; verification-id: web-ui-checks)

- [ ] Replace the legacy browser API client with a `/api/v2` client that discovers capabilities, obtains a coherent state snapshot, supports loopback no-token mode and an accessible bearer-token form, sends credentials only in the Authorization header, never uses URL credentials or localStorage, and clears tab-scoped credentials on disconnect. (verification: e2e - `make web-test` runs `tests/web/auth.spec.*` coverage for unauthenticated loopback success, 401 authentication, wrong-token recovery, reload behavior, disconnect clearing, and absence of tokens from URLs/log output; verification-id: web-ui-checks)

- [ ] Implement authenticated fetch-streamed SSE using `instance_id` and `event_sequence`, ordered event handling, full snapshot recovery for replay gaps/process restarts/parse failures, bounded reconnect, no-store polling fallback, and explicit fresh/stale/disconnected presentation that disables mutations while state is untrusted. (verification: e2e - `make web-test` runs `tests/web/stream-recovery.spec.*` with ordered events, gaps, incarnation changes, malformed frames, disconnects, and recovery assertions; verification-id: web-ui-checks)

- [ ] Implement one typed `/api/v2/commands` submission path for lifecycle, change, retry, and worktree actions using the latest confirmed revision, per-intent 1–200 character idempotency keys, pending-state duplicate prevention, same-key retry only for unknown transport outcomes, command-record settlement, and stale-revision resynchronization without automatic side-effect replay. (verification: integration - `make web-test` runs `tests/web/commands.spec.*` and `cargo test --features web-monitoring remote_control_api::tests::command_tests` to prove one effect for duplicate/unknown-outcome retry, no effect for stale revisions, and typed error recovery; verification-id: web-ui-checks)

- [ ] Replace the page structure with a task-oriented operator console containing process/connection freshness, current mode, active work, attention-required summary, one primary next action, changes grouped by operational priority, explicit detail disclosures, worktrees with server-provided eligibility reasons, persistent logs, and actionable typed-error notifications with correlation IDs. (verification: e2e - `make web-test` runs `tests/web/operator-console.spec.*` for select/running/stopping/error, blocked, conflict, empty, loading, long-content, and next-action states; verification-id: web-ui-checks)

- [ ] Implement destructive-action protection with native accessible dialogs for force stop, active-change stop-and-dequeue, and worktree delete; name consequences and targets, default focus safely, support Escape/cancel, restore invoking focus, prevent backdrop submission and duplicate confirmation, and use only opaque worktree IDs plus server eligibility for mutations. (verification: e2e - `make web-test` runs `tests/web/destructive-actions.spec.*` to prove cancellation sends no command, confirmation sends one opaque-ID command, and focus returns to the invoker; verification-id: web-ui-checks)

- [ ] Rebuild semantic and keyboard interaction behavior to WCAG 2.2 AA: skip link, one main landmark, hierarchical headings, WAI-ARIA tabs, button disclosures, labeled authentication controls, deliberate live regions, visible focus, status not conveyed by color alone, and no touch-only function. (verification: e2e - `make web-test` runs `tests/web/accessibility.spec.*` with automated serious/critical violation checks and keyboard coverage for skip, tabs, disclosures, forms, dialogs, notifications, and commands; verification-id: web-ui-checks)

- [ ] Replace inconsistent CSS with a defined token system whose normal text contrast is at least 4.5:1 and component/focus contrast at least 3:1; remove undefined properties and `transition: all`, respect reduced motion/high contrast, provide 44px primary targets, and support 320px, landscape, tablet, desktop, 200% zoom, and long content without page-level horizontal scrolling. (verification: e2e - `make web-test` runs `tests/web/responsive.spec.*` and `tests/web/contrast.spec.*` to assert no page overflow/hidden actions and valid tokens/contrast for every status and interaction state; verification-id: web-ui-checks)

- [ ] Remove the legacy single-instance `/api/*` and `/ws` router, permissive CORS layer, compatibility assertions, and browser-only backend modules/DTOs with no retained callers; continue serving `/`, `/style.css`, `/app.js`, `/api/v2`, API docs, and OpenAPI output. (verification: integration - `cargo test --features web-monitoring web::` runs `src/web/remote_control_api/tests/compatibility_tests.rs` replacement coverage proving legacy 404/no-side-effect behavior and retained static/v2 routes; verification-id: web-ui-checks)

- [ ] Update `web-monitoring`, `remote-control-api`, and related canonical contract coverage plus current README/configuration/developer documentation to make v2 the sole browser contract, document authentication/SSE recovery and removed routes, preserve local `--web`, and avoid standalone multi-project server claims; reconcile cleanly with `remove-multi-project-server-mode`. (verification: integration - `make check-openapi` and repository searches prove current docs and production UI contain no supported legacy browser route claims; verification-id: web-ui-checks)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate modernize-web-monitoring-ui --archive-gate`

The implementation must also pass `make web-test`, `cargo test --features web-monitoring`, `make check-openapi`, project lint, and project type checks supplied by the repository.

## Future Work

- Usability sessions with keyboard, VoiceOver, NVDA, and mobile assistive-technology users may refine wording and visual density after the repository-local WCAG gate passes.
- Localization, saved user preferences, offline operation, and a frontend framework remain separate changes if demonstrated demand justifies them.
