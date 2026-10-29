## ADDED Requirements

### Requirement: Web change-local error detail disclosure

The embedded console SHALL expose current v2 change-local `error_detail` inside the existing change Details definition list after Dependencies, under the exact term `Error detail`, only for a row whose display status is `error`. Nonblank strings SHALL be displayed verbatim through textContent without HTML/ANSI interpretation, auto-linking, unescaping or additional truncation; an absent, null, blank or nonstring error detail SHALL display exactly `Error details unavailable`. No logs, activity, command failures, blockers or process errors SHALL substitute for the current row detail. Non-error rows SHALL omit this term even when input retains stale detail. Diagnostic presentation SHALL have no workflow side effects or command-authority role.

#### Scenario: Real reason and remedy remain literal and complete

- **GIVEN** an error row with a producer-bounded Unicode error_detail containing a repository-relative proposal path and correction/commit/explicit Retry remedy
- **WHEN** the operator expands Details
- **THEN** the additional term and exact supplied value are readable in the existing definition-list anatomy after Dependencies
- **AND** the value contains no interpreted HTML, ANSI spans or recovery links and is not truncated by the browser
- **AND** no command is submitted

#### Scenario: Unavailable input does not invent a cause

- **GIVEN** an error row with absent, null, empty, whitespace-only or nonstring error_detail
- **WHEN** its Details are opened
- **THEN** exactly one Error detail value reads `Error details unavailable`
- **AND** an unrelated log, latest activity or process failure is not presented as the cause

#### Scenario: Non-error control has no fake diagnostic

- **GIVEN** a non-error row with absent or stale populated error_detail
- **WHEN** Details are expanded
- **THEN** there is no Error detail term
- **AND** all existing metadata and actions remain unchanged

### Requirement: Refresh-stable accessible Web error details

The console SHALL preserve existing same-instance ID-keyed disclosure choices across coherent snapshot replacement and replay-gap recovery. Open rows SHALL show the current value or fallback, and remove the term upon becoming non-error; closed rows SHALL stay closed. Multiple rows SHALL remain isolated and a disappeared/reappearing ID SHALL retain its existing same-console Set membership. A new console SHALL retain existing default collapsed behavior, without durable preferences. Details SHALL preserve native button semantics, aria-expanded/aria-controls, current themes, wrapping, tokens and responsive styling; diagnostic contents SHALL NOT authorize Retry or any other command.

#### Scenario: Current snapshot replaces an open diagnostic

- **GIVEN** an open error disclosure and a different closed row
- **WHEN** a new coherent snapshot or replay-gap resnapshot changes or removes the first row detail
- **THEN** the first row remains open and reflects only current detail, fallback or non-error omission
- **AND** the other row remains closed and unchanged by that disclosure choice

#### Scenario: Row removal and reappearance preserves ephemeral choice

- **GIVEN** an ID was expanded in this console instance
- **WHEN** it disappears and later reappears
- **THEN** its existing ID-keyed expanded membership is retained
- **AND** a new console instance has no durable record of that choice

#### Scenario: Disclosure preserves existing visual and keyboard contract

- **GIVEN** a current error row at a supported viewport class in either shipped theme
- **WHEN** the native Details button is activated
- **THEN** its associated details container toggles with matching aria-expanded
- **AND** the additional term/value inherits the existing definition-list typography, color and wrapping rules without a new visual control or inline styling
- **AND** local tests distinguish DOM/token proof from real-browser layout and keyboard observations

#### Scenario: Retry authority is not inferred from detail text

- **GIVEN** error rows with the same detail but different authoritative retry eligibility or mutation availability
- **WHEN** the console renders or toggles Details
- **THEN** existing Retry presence/availability follows only the unchanged action and mutation contracts
- **AND** toggling Details submits zero commands
