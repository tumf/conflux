---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/changes/remove-multi-project-server-mode
  - openspec/specs/web-monitoring/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/remote-worktree-operations/spec.md
  - web/index.html
  - web/style.css
  - web/app.js
  - src/web/mod.rs
  - src/web/remote_control_api
verifications:
  - id: web-ui-checks
    requirement: The API v2 operator console, legacy-route removal, command safety, responsive behavior, and WCAG 2.2 AA-critical interactions are verified before integration
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Repository-local Rust and browser test output plus generated OpenAPI comparison
    rerun: make web-test && cargo test --features web-monitoring && make check-openapi
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Modernize the web-monitoring operator UI

**Change Type**: implementation

## Problem / Context

The retained `web-monitoring` UI is still a client of the legacy `/api/*` and `/ws` surface even though the supported single-instance contract is `/api/v2`. The legacy browser surface uses permissive CORS and bypasses v2 bearer authentication, optimistic revision control, idempotency, typed errors, replay cursors, and opaque worktree identities.

The current interface also makes destructive actions too easy to trigger, hides recovery information in transient toasts, exposes inaccessible card, tab, and dialog interactions, references undefined CSS tokens, and diverges from its responsive specification. Users cannot reliably determine the current state, the next safe action, or how to recover from an error.

## Proposed Solution

Replace the embedded three-file monitor with a dependency-free production operator console backed exclusively by `/api/v2`. Preserve the static delivery model, but redesign the information architecture around current status, attention-required changes, contextual actions, worktrees, and logs.

The browser client will discover capabilities, authenticate when required, fetch a coherent snapshot, consume authenticated SSE through `fetch()` response streaming, and submit every mutation as a typed command with the current revision and an idempotency key. It will fail closed when state is stale or disconnected and require accessible confirmation for destructive actions.

Remove the legacy single-instance `/api/*` and `/ws` routes after the new console is wired and tested. Keep `/api/v2/ws` as a non-browser API contract and keep `/api/v2` behavior otherwise unchanged.

Implement WCAG 2.2 Level AA semantics and keyboard behavior, responsive layouts from 320 CSS pixels through desktop, explicit focus and status communication, valid contrast tokens, reduced-motion support, and gesture-independent controls. Add repository-local browser tests with test-only tooling; production assets remain static HTML, CSS, and JavaScript with no frontend runtime framework.

## Scope Completeness

- User outcome: one secure, discoverable operator console shows current mode, progress, changes requiring attention, worktrees, logs, connection freshness, and the next valid actions.
- API integration: all reads, streams, errors, and mutations use `/api/v2`; legacy browser routes are removed.
- Error prevention: revision, idempotency, command-pending state, operation eligibility, stale-state refusal, and destructive confirmations are visible UI behavior.
- Accessibility: semantic landmarks, keyboard-complete tabs and disclosures, accessible dialogs, live regions, focus management, contrast, zoom, reduced motion, and minimum target sizes are required.
- Responsive behavior: mobile, tablet, desktop, orientation changes, long identifiers, and 200% zoom retain all information and actions without page-level horizontal scrolling.
- Verification: Rust route/API tests, browser interaction tests, automated accessibility checks, responsive viewport checks, and OpenAPI consistency are change-blocking.
- Migration: remove legacy routes only after the v2 client passes equivalent success and failure tests; update current documentation and canonical specs.

## Acceptance Criteria

1. Loading `/` presents a usable operator console that obtains capabilities and a coherent state from `/api/v2`, and no production UI request uses legacy `/api/*` or `/ws`.
2. When bearer authentication is required, the UI presents an accessible token form after an unauthorized response, sends the token only in the `Authorization` header, never places it in a URL or log, never stores it in `localStorage`, and clears any tab-scoped copy when the user disconnects.
3. The browser consumes `/api/v2/events` with authenticated fetch streaming, resumes with `instance_id` and `event_sequence`, performs a full state refresh on replay gap or process-incarnation change, and falls back to no-store polling while clearly marking stale or disconnected state.
4. Every mutation uses `/api/v2/commands`, the latest confirmed `state_revision`, and a 1–200 character per-intent idempotency key. The same key is reused only when retrying an outcome-unknown network request. Stale revisions trigger resynchronization and a new user decision rather than automatic side-effect replay.
5. Duplicate submission is prevented while a command is pending. Force stop, active-change stop, and worktree deletion require an accessible confirmation dialog before any request. Worktree controls use opaque `worktree_id` and server-provided operation eligibility and blocked reasons.
6. The initial viewport communicates process identity, connection freshness, current mode, active work, attention-required conditions, and the next valid primary action without requiring users to interpret disabled controls.
7. Changes are organized by attention required, active, waiting, and completed states; details are exposed through explicit buttons rather than hidden gestures; logs and typed API errors persist long enough to support recovery and include correlation IDs and actionable guidance.
8. All functionality is keyboard operable. Tabs, disclosures, forms, dialogs, notifications, focus restoration, and dynamic status announcements use native semantics or established WAI-ARIA patterns and pass the defined automated WCAG 2.2 AA checks.
9. Text contrast is at least 4.5:1 for normal text, component and focus-indicator contrast is at least 3:1, state is never communicated by color alone, motion preferences are respected, and every primary touch action is at least 44 by 44 CSS pixels.
10. At 320 CSS pixels, mobile landscape, tablet, desktop, and 200% zoom, content and controls remain available without page-level horizontal scrolling. Long change IDs, branches, paths, log messages, and errors wrap or truncate with an accessible full-value affordance.
11. Requests to removed legacy single-instance `/api/*` and `/ws` routes return not found and cannot mutate state. `/api/v2`, `/api/v2/docs`, and the retained static assets continue to work.
12. Current README/configuration documentation describes the v2 console, browser authentication, SSE recovery, removed legacy routes, and local-only web-monitoring lifecycle without reviving standalone multi-project server claims.

## Explicit Completion Conditions

- `web/index.html`, `web/style.css`, and `web/app.js` implement the v2 console and contain no production reference to `/api/state`, `/api/control`, `/api/worktrees`, or legacy `/ws`.
- `src/web/mod.rs` serves static assets and merges only the v2 API router; legacy API and browser WebSocket modules and dead DTOs are removed when they have no retained callers.
- Browser tests execute the authenticated and unauthenticated initial load, SSE replay/recovery, command success, stale revision, unknown outcome retry, destructive confirmation, keyboard navigation, focus restoration, responsive viewports, and automated accessibility scan against a local fixture or in-process server.
- Rust integration tests prove legacy routes are absent, v2 routes remain available, static content types are correct, and rejected commands produce no side effects.
- CSS contains no undefined custom-property references, no `transition: all`, and no focus suppression without a visible replacement.
- `make web-test`, `cargo test --features web-monitoring`, and `make check-openapi` pass.
- Relevant canonical specs and current user/developer documentation no longer describe the legacy browser contract as supported.

## Out of Scope

- Reintroducing or redesigning the standalone multi-project server/dashboard removed by `remove-multi-project-server-mode`.
- Adding new `/api/v2` command types, arbitrary remote shell execution, remote conflict resolution, or unsafe worktree controls.
- Adding a production frontend framework, persistent user accounts, durable UI workflow state, offline/PWA support, or localization.
- Treating client-side state, session storage, logs, or command history as authoritative workflow-control input.

## Relationship to Active Server Removal

This change has no hard implementation dependency on `remove-multi-project-server-mode` because the integrated `/api/v2` contract already provides every consumed resource. The changes overlap in `src/web/mod.rs`, web-monitoring specs, build configuration, and documentation, so an implementation must reconcile against whichever change lands first while preserving local `--web`, `/api/v2`, OpenAPI generation, and the removal of standalone server behavior.
