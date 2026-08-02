# Design: API v2 web-monitoring operator console

## Goals

- Minimize the time required to understand current orchestration state and choose the next safe action.
- Make the embedded browser UI a first-class `/api/v2` client rather than a compatibility client.
- Meet WCAG 2.2 Level AA for the complete operator workflow.
- Preserve the zero-build, embedded static asset model for production.
- Keep workflow authority in the workspace and server projection, never in browser persistence.

## Non-Goals

- A new component framework, build pipeline, application database, or user-account system.
- Multi-project navigation or standalone server administration.
- Browser access to `/api/v2/ws`; authenticated browsers use fetch-streamed SSE.
- New backend commands or weaker worktree safety rules.

## Atomic Scope Rationale

API migration, destructive-action safety, and accessibility must ship together. Removing legacy routes before the UI migrates breaks the monitor; migrating reads without commands leaves mutations on the unsafe contract; changing interactions without accessible equivalents leaves users unable to operate the product. These concerns therefore form one independently verifiable operator-console replacement.

## Information Architecture

### Header

The header contains the product name, process version/instance summary, connection freshness, and an explicit reconnect or authenticate action. A skip link moves keyboard focus to the primary content.

### Current status

The first content section answers three questions without exposing internal implementation terms:

1. What is Conflux doing now?
2. Does anything require attention?
3. What is the next valid action?

Only currently valid primary and secondary lifecycle actions are shown. Unavailable actions are omitted unless users benefit from knowing why they are unavailable; those controls remain visible with a textual blocked reason.

### Changes

Changes are grouped or sorted by attention required, active, waiting, then completed. Each row exposes status text, progress, dependency summary, and an explicit disclosure button. Contextual actions are derived from current v2 state and capabilities, not duplicated client state machines.

### Worktrees

Worktree rows expose branch, redacted relative path, dirty/conflict evidence, and operation eligibility. Merge and delete are addressed only by `worktree_id`. Blocked actions display the API-provided reason. Conflict recovery points users to local or TUI recovery.

### Logs and notifications

Logs are a durable page section for the process lifetime and support level filtering. Typed errors render `message`, `error_code`, `correlation_id`, and the next recovery action. Toasts are reserved for concise confirmations; errors that need action persist until dismissed or resolved.

## Browser API State Machine

### Bootstrap

1. Request `/api/v2/health` without credentials.
2. Request `/api/v2/capabilities` and `/api/v2/state` with the current in-memory/session token, if present.
3. On `unauthorized`, show the authentication form and move focus to its heading or token field.
4. On success, retain `instance_id`, `state_revision`, and `event_sequence` in memory and render the coherent snapshot.
5. Start authenticated fetch-streamed SSE from `/api/v2/events?after_sequence=...&instance_id=...`.

### Authentication

The token is accepted through a password-type input with an explicit label and show/hide control. It is sent only in `Authorization: Bearer`. It is never appended to a URL, written to logs, included in correlation IDs, or stored in `localStorage`. A tab-scoped `sessionStorage` copy is allowed only to survive reload; Disconnect removes it and clears all sensitive in-memory state. The UI must work without a token for loopback deployments where authentication is disabled.

### Event recovery

Events are consumed in order. A changed state event triggers a coherent state refresh rather than client-side reconstruction from undocumented payloads. Log events may append their documented observation payload after sequence validation. A replay gap, unexpected sequence, stream parse failure, or changed `instance_id` closes the stream and performs a full snapshot refresh before reconnecting. Polling is a fallback, not a parallel source of authority.

### Commands

A command request contains the typed command fields, latest confirmed `expected_revision`, and a `crypto.randomUUID()` idempotency key. The UI tracks one intent record while its outcome is pending:

- ordinary retry after a typed failure creates a new intent only after a new user action;
- retry after an outcome-unknown transport failure reuses the same request and key;
- `stale_revision` refreshes state and asks the user to decide again;
- a known command record result settles the pending UI and refreshes state.

All mutation controls are disabled while stale, disconnected, authenticating, or already pending for the same target.

## Destructive Interaction Pattern

Force stop, active-change stop-and-dequeue, and worktree deletion use native `<dialog>`. The dialog names the target and consequence, provides Cancel as the least destructive default, does not submit on backdrop click, closes on Escape before submission, moves focus inside on open, and restores focus to the invoking control on close. Confirm changes to a pending state immediately and cannot be double-submitted.

Worktree merge remains conflict-preserving rather than destructive, but its dialog or inline confirmation must explain that conflicts require local/TUI recovery when the API advertises that boundary.

## Accessibility

- Semantic header, navigation, main, sections, headings, lists, forms, status, and buttons.
- One main landmark and a visible-on-focus skip link.
- Tabs implement `tablist`, `tab`, `tabpanel`, `aria-selected`, roving tab index, Arrow keys, Home, and End.
- Disclosures use buttons with `aria-expanded` and `aria-controls`; cards are not pseudo-buttons.
- Connection and routine updates use polite status; failed mutations use assertive alerts without announcing every event.
- Visible `:focus-visible` indicators satisfy 3:1 contrast and are not clipped.
- Status labels include text and optional decorative icons with `aria-hidden`.
- Reduced-motion removes nonessential transitions; loading state remains perceivable without animation.
- The document language is declared, identifiers use `translate="no"`, and dates use `Intl.DateTimeFormat`.

## Responsive Layout

The mobile layout prioritizes current state and one primary action, followed by attention-required changes. Tablet and desktop increase density without changing reading order or DOM order. Touch gestures are optional enhancements only; every result has a visible button and keyboard equivalent.

The viewport supports zoom. Containers avoid page-level horizontal overflow. Code-like values use wrapping or bounded scrolling within the value region, with full text available to assistive technology and pointer/keyboard users.

## Styling

Use one explicit CSS custom-property set. Every reference must resolve. Tokens cover surfaces, text, borders, focus, primary, success, warning, and danger states with measured contrast. Color is supplementary to labels and icons. Avoid `transition: all`; animate only opacity or transform and honor reduced motion.

## Production and Test Tooling

Production remains the existing embedded HTML/CSS/JavaScript and does not require a runtime package manager. Repository-local browser tests may add the minimum dev-only tooling needed for DOM, accessibility, and viewport verification. The Makefile exposes one `web-test` command consumed by local and CI verification. Tests that exceed one second must be optimized or placed in the repository's heavy-test tier.

## Migration Sequence

1. Add browser test harness and fixture coverage for current v2 routes.
2. Build the v2 API client, authentication, snapshot, SSE, and command primitives.
3. Replace UI structure and styling with accessible responsive components.
4. Wire changes, worktrees, logs, notifications, and destructive confirmations.
5. Prove all production requests use v2.
6. Remove legacy `/api/*`, `/ws`, permissive CORS, and dead browser-only backend code.
7. Update canonical specs, OpenAPI evidence, and current documentation.

## Interaction with Server-Mode Removal

`remove-multi-project-server-mode` removes standalone server code but retains local web monitoring and v2. If it lands first, this implementation starts from its reduced router and dependency set. If this change lands first, the later removal must retain the new static console and v2 tests. Neither change may restore the old standalone React dashboard or multi-project API.

## Constitutional Compliance

Browser token/session data, selected tabs, pending command presentation, and logs are ephemeral UI/observability state. They do not decide orchestration routing or completion. Every command is revalidated by the shared server service against workspace-derived state, satisfying workspace-local authority and truthful completion.
