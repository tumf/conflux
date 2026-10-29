## Implementation Tasks

- [ ] In `web/app.js:renderChangeRow`, append the conditional `Error detail` term after Dependencies via `appendDetail` and `textContent`; implement the exact nonblank-string/fallback and non-error omission rules in design.md, without changing the API or other metadata. Done when the positive real-renderer assertion, all absent/malformed cases and hostile Unicode/HTML cases pass. (verification: integration - tests/web/error-detail.spec.js through tests/web/helpers/console.js; verification-id: web-error-detail)
- [ ] Preserve the existing ID-keyed disclosure state across coherent refresh/replay-gap replacement; update/remove current error text without resetting open/closed choices, automatic opening, command dispatch or new storage. Keep native disclosure semantics and all existing actions unchanged. Done when open/closed refresh, disappearance/reappearance, multiple-row isolation and error-to-non-error tests pass and action eligibility remains invariant. (verification: integration - tests/web/error-detail.spec.js refresh and commands assertions; verification-id: web-error-detail)
- [ ] Add `tests/web/error-detail.spec.js` using the shipped console/stylesheet and fixture server; execute every required case in ui-matrix.json, assert fail-closed matrix coverage, at least 12 actual tests and no skipped/todo cases, plus inherited token/rule assertions and existing responsive/contrast regression checks by explicit imports of their specs. Do not treat jsdom as a layout engine. Done when the declared gate selects the new file, fails against the old omission, and passes the entire contract with fresh execution output. (verification: integration - tests/web/error-detail.spec.js with tests/web/responsive.spec.js and tests/web/contrast.spec.js; verification-id: web-error-detail)

## Future Work

Real-browser layout, actual Tab/Enter/Space behavior and screen-reader observation remain parent/operator-owned follow-up; they do not block local implementation completion. Queue/mark/Start reconciliation is independent follow-up and consumes no output from this change.

## Final Validation

Archive validation is the authoritative final OpenSpec validation gate: `cflx openspec validate project-web-error-detail --archive-gate`. Structural proposal validation does not mean the implementation test exists or has passed.
