## Implementation Tasks

- [x] 1.1 Add the narrowly scoped `.attention[hidden]` display-none override after the current attention rule, preserving all other CSS and app behavior (verification: unit - web/style.css and tests/web/attention-visibility.spec.js computed display; verification-id: attention-visibility — done only when hidden current-state summary is not rendered).
- [x] 1.2 Add at least four production-console tests with shipped styles covering design inventory A-E, computed display and zero command effects; retain existing operator-console/responsive/contrast selection (verification: integration - tests/web/attention-visibility.spec.js and declared npm gate; verification-id: attention-visibility — old stylesheet fails hidden visibility assertion and changed stylesheet passes the identical assertion).

## Notes

- Before (baseline web/style.css, new spec only): `npm --prefix tests/web test -- attention-visibility.spec.js --reporter=verbose` reported `Tests 7 failed | 2 passed (9)`; every hidden-state case failed on `expected 'flex' to be 'none'` from `getComputedStyle(attention-summary).display`, and the stylesheet contract test failed on the missing `.attention[hidden]` rule. The two passing tests were state B (visible flex summary) and the baseline flex-anatomy check.
- After (web/style.css with the 7-line `.attention[hidden] { display: none; }` block): the declared gate `npm --prefix tests/web test -- attention-visibility.spec.js operator-console.spec.js responsive.spec.js contrast.spec.js --reporter=verbose` reported `Test Files 4 passed (4)`, `Tests 121 passed (121)`, no skipped or todo cases, with identical assertions.
- Full repository-local web suite `npm --prefix tests/web test` reported `Test Files 10 passed (10)`, `Tests 204 passed (204)`.
- The spec covers inventory A, B, C (archived and removed), D, E, plus a zero-command transition sweep; computed display is a jsdom cascade observation, not pixel or real-browser proof.
- `cflx openspec validate fix-web-attention-visibility --strict` passed.

## Final Validation

Archive validation is authoritative. Expected archive gate: `cflx openspec validate fix-web-attention-visibility --archive-gate`.

## Future Work

Parent-owned real-browser observation of corrected visibility in an isolated owner. No production replacement or publication.
