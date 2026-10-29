---
change_type: implementation
priority: high
dependencies: []
references:
  - web/app.js
  - web/style.css
  - src/web/remote_control_api/dto.rs
  - tests/web/helpers/console.js
verifications:
  - id: web-error-detail
    requirement: Safe current change-local error details remain accessible without changing command authority
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/web/package.json
    evidence: npm --prefix tests/web test -- error-detail.spec.js responsive.spec.js contrast.spec.js --reporter=verbose
    rerun: npm --prefix tests/web test -- error-detail.spec.js responsive.spec.js contrast.spec.js --reporter=verbose
    prerequisites: [Node.js and npm, npm ci --prefix tests/web --ignore-scripts --no-audit --no-fund]
    execution_class: repository-local
    completion_role: change-blocking
---

# Project change-local error details into Web recovery

**Change Type**: implementation

## Problem / Context

At source revision `0444fd4c4f32be306f072619a1d2017393d457cb`, the v2 DTO already provides nullable sanitized `ChangeResource.error_detail`. `src/web/state.rs` derives it from reducer error state; `src/web/remote_control_api/projection.rs` preserves it. `web/app.js:renderChangeRow` constructs Details with Status, Task status, optional Iteration, and Dependencies only. It never reads `error_detail`. Thus a recoverable error is visible but its actual reason/location/remedy is not. This is a browser projection omission, not a missing backend diagnostic.

Parent's read-only runtime evidence from source build `1dd04e39` reported `fixture-invalid-metadata` in error with this detail: `Change 'fixture-invalid-metadata' cannot start: invalid proposal metadata at openspec/changes/fixture-invalid-metadata/proposal.md. Correct and commit that proposal, then explicitly Retry the change.` This is corroboration, not a runtime fixture or a requirement to control that owner. Repository tests must recreate the public data shape locally, not read the parent's private path.

Independent Queue/Start finding: `changeActions` sends `set_queue_intent` for Add to queue, not `set_execution_mark`. `operator_coordinator::apply_queue_intent` changes queue state; `run_control::classify_start_targets` reads authoritative marks and accepts ordinary status `not queued`, not already `queued`. Consequently queueing alone is not Start selection, and even a marked already-queued ordinary row is excluded. Removing queue intent does not clear the separate mark, explaining why Start then succeeds. This needs a separate selection/admission proposal; changing a label alone does not reconcile the contract. It is neither a dependency nor part of this diagnostic projection fix.

## Proposed Solution

Append exactly one `Error detail` term/value inside the existing Details `dl` when the current coherent row has `display_status === 'error'`. A nonblank string `error_detail` is rendered verbatim through existing `appendDetail` / `textContent`. Absent, null, empty, whitespace-only, or nonstring values on an error row render exactly `Error details unavailable`. No log, latest activity, blocker, command failure, or process-level error substitutes for it. Non-error rows never render this term, even if an obsolete detail happens to remain in input.

Use the existing `expanded` Set, keyed by change ID, to preserve open/closed state across snapshot refresh and event-gap recovery in the same console instance. No settings, storage persistence, automatic opening, new modal, copy control, retry shortcut, or visual redesign.

## Visual authority and precedence

Existing production `web/app.js`, `web/index.html`, and `web/style.css` at `0444fd4c4f32be306f072619a1d2017393d457cb` govern the existing card, disclosure, definition-list geometry, tokens, themes and controls. This proposal overrides only the additional detail term/value and its presence rules. `design.md` fixes owners, inventory, exact inheritance and verification limitations. That revision is review authority, not a gate freezing unrelated future production file hashes.

## Acceptance Criteria

1. Expanding an errored row reveals the API-supplied location and correction/commit/explicit-Retry text in full, after Dependencies, using literal text only. A backend-bounded 8,192-byte Unicode detail is not sliced, parsed, auto-linked, ANSI-rendered, or interpreted as HTML by the browser.
2. Error rows with unavailable/malformed detail show the explicit neutral fallback, never a made-up failure or raw unrelated log. Non-error rows have no Error detail term. Refresh updates/replaces/removes old detail in the same row while preserving that row's disclosure choice.
3. Existing button semantics, aria-expanded/aria-controls association, keyboard activation, styling, responsive wrapping, themes, and other metadata remain intact. Diagnostics submit zero commands; Retry stays exclusively controlled by `actions.retry_change.allowed === true` and current mutation availability.
4. A bounded nonempty repository-local suite imports the shipped renderer and proves every required matrix row, safe text behavior, refresh transitions, action invariance and exact inherited token contract. Missing test selection, missing/duplicate/stale/unexecuted matrix coverage or failed assertions cannot pass.

## Change boundary and preserved contracts

Implementation may edit `web/app.js` and add `tests/web/error-detail.spec.js`, reusing `tests/web/helpers/*`; minimal helper extension is permitted only for exercising the new tests. No CSS, HTML shell, package/dependency/lockfile, Rust/API/schema, scheduler, mark, queue, retry classifier, TUI, config, or deployment changes. If the approved local addition cannot satisfy the existing style contract, stop rather than redesign it. Canonical spec promotion is the ordinary archive operation.

Preserve status grouping, progress, unrelated Details fields, lifecycle and per-row commands, authentication, stale/offline behavior, same-console expanded-state ownership, coherent v2 observation/recovery and server diagnostic sanitation. Domain state never derives from display text.

## Explicit Completion Conditions

The shipped `renderChangeRow` path consumes current `error_detail` with `appendDetail` only for error rows; the new test file runs the real console through `tests/web/helpers/console.js` and fixture server, not a duplicate renderer. Gate output must name `error-detail.spec.js`, report at least 15 executed tests from the new file (one per each of the 14 required rows plus the inventory assertion), the explicitly selected existing responsive/contrast checks, and zero skips/todos, and show all required matrix rows evaluated. The implementation must add a test-side fail-closed inventory check and individual behavioral assertions; reading a prefilled PASS report is not execution. The current old renderer must fail the positive-content assertion. All implementation checkboxes remain open until this exists and the declared single gate succeeds.

## Verification plan / limitations

`tests/web/package.json` has only `test: vitest run`, with jsdom, Vitest and axe-core; no real-layout/screenshot runner is installed. `vitest.config.js` includes root `*.spec.js`; the proposed file is selected by its unique filename and Vitest's default no-tests refusal must remain enabled. `npm ci --ignore-scripts --no-audit --no-fund` installed the existing lockfile dependencies successfully in the isolated proposal worktree. Existing operator-console + responsive tests actually ran: 39 passed in 2 files. That is infrastructure evidence, not evidence of this unimplemented fix.

Repository-local proof covers DOM behavior and inherited token/rule contract, NOT pixel layout, native keyboard event synthesis, focus across whole-tree refresh, or screen-reader conformance. Future read-only browser observation at widths in design.md is intentionally non-blocking operational follow-up, not a hidden Apply checkbox or a claim of pre-integration visual conformance. No new command or release automation is introduced. The tracked `.pre-commit-config.yaml` Rust hooks are path-scoped, not an unconditional substitute for this focused Web gate.

## Out of Scope / Future Work

- Queue/mark selection controls and Start semantics, including existing Add to queue eligibility presentation: separate proposal; no cosmetic relabeling or scheduler widening here.
- Structured blocker/provenance or process-error projection, editing proposals in Web, automatic recovery, clipboard, new settings, durable UI preferences, full-dashboard redesign, external owner control.
- Real-browser responsive/keyboard and screen-reader observations after implementation, owned by the parent/operator on an isolated fixture. Record actual source/build identity, fixture, width/theme/zoom, disclosure action and settled result; do not claim them from jsdom.
- This authoring branch contains proposal artifacts only. No implementation, archive, merge, mark, restart, publish or deployment is authorized by its validation.
