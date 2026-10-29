## Implementation Tasks

- [ ] 1.1 Add the narrowly scoped `.attention[hidden]` display-none override after the current attention rule, preserving all other CSS and app behavior (verification: unit - web/style.css and tests/web/attention-visibility.spec.js computed display; verification-id: attention-visibility — done only when hidden current-state summary is not rendered).
- [ ] 1.2 Add at least four production-console tests with shipped styles covering design inventory A-E, computed display and zero command effects; retain existing operator-console/responsive/contrast selection (verification: integration - tests/web/attention-visibility.spec.js and declared npm gate; verification-id: attention-visibility — old stylesheet fails hidden visibility assertion and changed stylesheet passes the identical assertion).

## Final Validation

Archive validation is authoritative. Expected archive gate: `cflx openspec validate fix-web-attention-visibility --archive-gate`.

## Future Work

Parent-owned real-browser observation of corrected visibility in an isolated owner. No production replacement or publication.
