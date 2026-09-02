# UI implementation proposal contract

Read this reference whenever a proposal changes a user-visible screen, shell, component, interaction, responsive layout, or visual state. A UI proposal is incomplete when it specifies only copy, hierarchy, controls, routes, or accessibility semantics. Those properties can all pass while the rendered product is visibly wrong.

Apply the full contract to surfaces whose visual properties, structure, or interaction design are introduced or changed. For changes that preserve the existing visual language, the current production revision may be the authority and an existing deterministic render/snapshot suite may be reused when it covers every affected state. Do not create a parallel authority or redundant whole-product inventory for a local copy-only or token-only change.

## 1. Fix the visual authority before drafting

Name the exact authoritative source for each in-scope surface:

- repository path and revision or SHA-256 for an accepted HTML prototype, design file export, screenshot set, or existing production screen;
- the screens and states governed by that authority;
- precedence when several references disagree;
- aspects that are intentionally native or platform-specific.

Do not write “follow the prototype” or “match the design” without this binding. Do not transform an accepted prototype into another artifact and then use the transformed copy as equivalent authority. Preserve accepted repository artifacts byte-for-byte when the repository permits it.

If the authority is missing, contradictory, or not available to the implementation agent, stop proposal drafting and resolve it first. Visual design discovery is not Conflux implementation work.

## 2. Build a screen-state inventory

Enumerate every in-scope screen or screen family and every state that changes rendering or behavior. Include, where applicable:

- initial, loading, empty, populated, validation-error, dependency-error, offline, locked, and permission-denied states;
- closed/open, selected/unselected, enabled/disabled, focused, pressed, expanded/collapsed, and transient states;
- viewport classes, safe areas, keyboard-visible layout, orientation, theme, locale, text scaling, and reduced motion;
- navigation entry, back/fallback destination, focus restoration, dismissal, and recovery paths.

Give rows stable IDs. For each row record its visual authority, production owner, expected transition, and verification class. Aggregate conformance is fail-closed: a missing, duplicate, stale, failed, or `NOT_EVALUATED` required row makes the UI outcome non-PASS.

## 3. Convert visual intent into an implementation contract

For every screen family, specify measurable or directly comparable properties rather than impression words:

- background and semantic color roles;
- content alignment, max width, edge insets, safe-area handling, and vertical rhythm;
- top bar, title, step/progress, status, and screen-specific actions;
- component anatomy, icon placement, chevrons/arrows, borders, radii, elevation, and separators;
- typography role, weight, size, line height, wrapping, truncation, and text scaling;
- primary/secondary/destructive action priority and placement;
- minimum target size, focus order, announcements, keyboard access, gesture alternatives, and reduce-motion behavior.

Use exact values from the authority when they exist. When production tokens intentionally replace literal values, name the token and prove its resolved value. “Same hierarchy and copy,” “use existing components,” “visually similar,” and “polish the UI” are not acceptance criteria.

## 4. Bind authority to production owners

Map every inventory row to the concrete renderer, route, component, style/token owner, typed presentation input, and accepted command. Explicitly list preserved runtime/domain authority and out-of-scope plumbing.

A generic renderer or existing design-system component is allowed only when it preserves the required anatomy and resolved visual properties. If it cannot, require a typed dedicated renderer or an explicitly scoped extension. Never let the implementation agent decide whether a visible difference is acceptable.

UI code consumes typed presentation data and dispatches accepted commands. It must not infer domain meaning from display labels or parse rendered text to select behavior.

## 5. Require evidence that can detect the wrong UI

Each major visual requirement needs a check that would fail if the implementation kept the old generic UI while merely copying labels and transitions.

Use two layers:

1. **Change-blocking repository-local proof.** Prefer a bounded deterministic render/snapshot test, browser screenshot comparison, token-resolution assertion, or mechanically checked screen-state matrix. Bind it to a complete `pre-integration`, `repository-local`, `change-blocking` verification declaration and reference its ID from active implementation tasks.
2. **Runtime observation where needed.** Native Simulator/device screenshots, screen-reader behavior, physical-device layout, credentialed data, and deployed-service rendering are `operational-observation` evidence. Declare them as post-integration verification or place them in a separate release-observation change; do not disguise them as repository-local blockers.

A source scan, typecheck, semantic component catalog, accessibility tree, route test, or successful build does not prove visual conformance. One representative screenshot does not cover a screen family. A screenshot proves only its exact revision, build/app identity, runtime/device, viewport, state/fixture, locale/theme/text scale, action, settled postcondition, and artifact hash.

If no repository-local check can detect the principal visual regression, say so explicitly. Do not claim pre-integration visual conformance. Either add a deterministic render harness within the proposal's agreed scope or separate the runtime observation and make the limitation visible in acceptance and completion language.

## 6. Required proposal artifacts

For UI implementation or hybrid changes, include:

- `proposal.md`: exact visual authority, user-visible final state, acceptance criteria, preserved behavior, and explicit non-goals;
- `design.md`: required unless the change is one trivial component/state; include authority precedence, screen-state inventory, production-owner mapping, navigation topology, component decisions, and deliberate native exceptions;
- `tasks.md`: separate renderer/style implementation, command wiring, accessibility behavior, regression tests, and evidence generation; every checkbox has a bounded completion condition and a change-blocking verification ID;
- spec deltas: scenarios for every behavior-bearing state and transition, including failure, dismissal, recovery, and accessibility behavior;
- tracked matrix or fixture whenever the proposal claims fail-closed aggregate conformance; its check must reject missing, duplicate, stale, failed, and `NOT_EVALUATED` required rows.

Do not hide unresolved screen coverage, visual decisions, or runtime-only evidence in task prose. If an omitted row could leave a visibly wrong or unreachable state while all tasks pass, the proposal is not implementation-ready.

## 7. Fable review blockers

Ask the independent proposal reviewer to return BLOCKER when any of these is true:

- visual authority is unnamed, unavailable, mutable without revision binding, or contradictory;
- screen/state inventory is incomplete or aggregate evaluation is not fail-closed;
- impression words replace measurable anatomy or comparison criteria;
- generic component substitution can erase the accepted visual language;
- navigation reachability or fallback is left for implementation;
- mechanical tests are presented as visual evidence;
- screenshot/runtime evidence is unbound to exact revision, build, state, and environment;
- a simulator/device/deployed check is mislabeled as a repository-local completion blocker;
- the implementation agent must make a product, visual, interaction, or exception-policy decision.

After fixing blockers, run a second review. UI proposal defects often hide in adjacent states or canonical requirements that the first correction exposes.

## Compact readiness checklist

Before strict validation, answer all with repository evidence:

- [ ] Exact visual authority and precedence are fixed.
- [ ] Every required screen, state, transition, and viewport variant has a stable row.
- [ ] Every row maps to a production owner and evidence class.
- [ ] Visual anatomy and action priority are objective enough to implement without interpretation.
- [ ] Generic component reuse has explicit preservation conditions.
- [ ] At least one bounded gate would fail for the old/generic-but-semantic UI.
- [ ] Runtime-only evidence is separated from change-blocking evidence.
- [ ] Accessibility, localization, text scaling, safe area, keyboard, focus, and reduced motion are covered where relevant.
- [ ] Aggregate PASS fails closed on missing or unevaluated rows.
- [ ] Fable has reviewed the complete UI contract and no design decision remains for Conflux.
