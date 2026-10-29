# Design: one local failure contract before classification can return

## Ordering

Only ordinary admitted candidates enter this check. Retain authoritative active, terminal, held and revoked-intent exclusions. Strict metadata read errors must be reconciled to the existing ProcessingError projection before blocked-only/no-capacity exits can suppress diagnostics. Recheck the same condition at the existing late dispatch guard to cover a changed file. Current repository state is authoritative; no file/cache marker controls admission. Event deduplication suppresses noise only, never projection.

## User-visible state inventory (existing design retained)

| ID | Source state and transition | Existing owner/authority at f618a3e7 | Proof |
| --- | --- | --- | --- |
| invalid-only | queued -> error before analysis, actionable details | queue_state/reducer -> src/tui existing Changes error row / details | scheduler integration + TestBackend 120x30, 80x24 |
| mixed | invalid row error, valid sibling still eligible | same typed row projection, no global Error | mixed-queue assertion + existing row rendering |
| repeat | identical invalid error remains stable | current state, dedup only affects diagnostic events | event count + projection reconstruction |
| fixed-await-retry | corrected committed proposal remains error until explicit Retry | existing retry owner / existing key hints | reducer/operator test |
| recovered | authorized Retry rechecks valid data then dispatches once | normal scheduler admission | exact execution counters |
| still-invalid | Retry rechecks and returns to error, zero execution | same error/detail UI, no invented provenance | failure control |
| dependency-control | genuine dependency wait/recovery unchanged | existing dependency projection | existing dependency test + focused control |

No geometry, token, key, modal focus, action-priority, localization or accessibility behavior changes. Existing error popup wrapping, scrolling and Esc handling remain authoritative; its error text is bounded and contains relative location and correction/Retry instruction. A normal/narrow TestBackend assertion must inspect rendered text, not merely error field existence. API error_detail supplies the same safe diagnostic; frontend parsing of text is forbidden.

## Failure behavior

Unreadable, malformed or wrong-typed proposal metadata never silently becomes no dependencies. Suppress only the affected candidate. Keep raw YAML and values out of published detail. Use change-local ProcessingError for both early and late checks; process fatal errors remain global and untouched. On Retry, new repository read decides eligibility; an in-memory error from a past process is not a durable workflow input.
