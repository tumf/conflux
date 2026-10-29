---
change_type: implementation
priority: medium
dependencies: []
references: [web/style.css, web/app.js, web/index.html, tests/web/helpers/console.js]
verifications:
  - id: attention-visibility
    requirement: Current attention visibility follows hidden state under shipped CSS
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/web/package.json
    evidence: npm --prefix tests/web test -- attention-visibility.spec.js operator-console.spec.js responsive.spec.js contrast.spec.js --reporter=verbose
    rerun: npm --prefix tests/web test -- attention-visibility.spec.js operator-console.spec.js responsive.spec.js contrast.spec.js --reporter=verbose
    prerequisites: [Node.js and npm, npm ci --prefix tests/web --ignore-scripts --no-audit --no-fund]
    execution_class: repository-local
    completion_role: change-blocking
---
# Respect hidden attention summaries under production styling

**Change Type**: implementation

## Problem / Context
At baseline 148e5612b3a7c062624f65aefe0a75ef5905c2b4, web/app.js renderCurrentStatus correctly sets attention-summary.hidden when grouped.attention becomes empty, but web/style.css .attention sets display:flex without a hidden-state override. Author CSS overrides the user-agent hidden display rule. In a real browser after explicit Retry, archive and merge, the summary still visibly names the old error while Needs attention says Nothing. This is presentation, not stale backend workflow state. An unchanged shipped-CSS jsdom probe returned hidden=true, computed display=flex. Existing operator-console tests assert .hidden only and do not detect this.

## Proposed Solution
Add only a targeted `.attention[hidden] { display: none; }` rule after the existing `.attention` block, preserving `.attention` flex styling when not hidden. Do not add a broad global hidden reset, clear domain/error state, or change application JavaScript. Extend repository tests using the real shipped HTML/CSS/renderer to assert computed display, not merely the boolean hidden property.

## Acceptance Criteria
Hidden attention summary has computed display none on initial no-attention state and after an attention row is removed or becomes non-attention. A current attention row still shows the existing flex summary and correct current names. Reappearance restores the existing presentation. Render observations submit zero commands. One nonempty bounded gate runs the new and existing listed suites with no failing, skipped or todo required cases.

## Boundary / Preserved Contracts
Only web/style.css, new tests/web/attention-visibility.spec.js, and minimal test helper extension if essential. No Rust, API, scheduler, process_error, commands, storage, tokens, DOM anatomy, markup, dependencies, or broad CSS selectors. Existing hidden elements elsewhere are outside scope. Canonical spec promotion follows normal archive.

## Visual Authority
web/style.css, web/index.html, and web/app.js at baseline 148e5612b3a7c062624f65aefe0a75ef5905c2b4 are the authority. Override only hidden-state attention visibility; inherit all nonhidden anatomy, color, font, spacing and flex values without change. design.md fixes exact inventory and bounded evidence limits.

## Explicit Completion Conditions
The new file selects at least four tests, importing the production console/helper with styles enabled. Tests cover all design inventory states, computed display and zero commands. The old shipped stylesheet fails the hidden-state computed-style assertion; the correction passes identical assertions. Preserve existing operator-console, responsive and contrast gates. Do not characterize DOM-only tests as pixel-layout or real-browser proof.

## Future Work / Non-goals
Parent-owned isolated real-browser observation checks summary actually disappears after Retry/archive and that the live attention state remains visible. This operational observation is not Apply checkbox work or deployed acceptance. No publication, release, owner restart or user candidate repair belongs to this proposal.
