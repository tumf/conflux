## MODIFIED Requirements

### Requirement: Acceptance follow-up persistence failure must not override primary acceptance failure

When acceptance returns a non-pass verdict with findings and retry policy routes the change back to apply, the runtime SHALL preserve that acceptance verdict as the primary outcome even if follow-up persistence into `tasks.md` degrades.

For an apply-retry outcome, the runtime SHALL attempt to persist acceptance follow-up findings to the canonical tasks location for the workspace. It MUST prefer the active change tasks path and MUST fall back to the matching archived tasks path when the active path does not exist. A FAIL routed directly to a resumable stalled hold MAY preserve current findings in its workspace checkpoint and stalled marker without updating `tasks.md`.

Runtime MUST be the sole writer of numbered `## Acceptance #<n> Failure Follow-up` sections. For apply-retry outcomes it MUST retain only the latest runtime-owned section, normalize multiline findings into one checkbox task per finding, rehydrate deleted or altered runtime findings during apply, and remove the runtime-owned section after acceptance PASS. Serial and parallel execution MUST apply the same persistence and cleanup behavior.

Runtime MUST ignore matching headings inside fenced code examples. When a detected runtime-owned section has an unambiguous boundary but contains content outside the supported runtime record forms, runtime MUST preserve the unknown content byte-for-byte outside the runtime-owned section under `## Recovered Acceptance Notes`, enclose it in a dynamically sized fenced literal, emit supplemental recovery diagnostics, and continue replacement or cleanup. The recovered representation MUST identify the payload as untrusted content that is neither instructions nor task state, MUST deduplicate identical payload bytes across retries and restarts, and MUST remain after acceptance PASS cleanup. Preservation and runtime-section replacement or removal MUST occur in one atomic tasks-file update.

Runtime MUST refuse the destructive update and leave `tasks.md` unchanged when the runtime-owned boundary cannot be determined safely, including an unclosed fence or ambiguous layout, or when unknown content cannot be preserved before replacement. Failure to persist or recover follow-up findings MUST NOT by itself convert an acceptance `FAIL` into a terminal execution `Error` unless the primary acceptance outcome itself is indeterminate.

Task progress and OpenSpec task validation MUST ignore checkbox-like content inside valid backtick or tilde fenced blocks so recovered content cannot alter completion or archive decisions.

If persistence degrades, the runtime SHALL record the explored path(s) and expose the persistence issue as supplemental warning/error context rather than replacing the acceptance diagnosis.

#### Scenario: unknown follow-up prose is preserved and normalized

- **GIVEN** a runtime-owned acceptance follow-up has an unambiguous boundary
- **AND** it contains supported runtime findings plus unknown multiline evidence or presentation text
- **WHEN** runtime replaces the follow-up for a retry
- **THEN** runtime preserves the unknown bytes in one fenced recovered-notes block
- **AND** runtime writes the canonical current follow-up from normalized findings
- **AND** execution continues with a supplemental warning rather than a terminal configuration error

#### Scenario: repeated recovery is idempotent

- **GIVEN** unknown follow-up content has already been moved to recovered notes
- **WHEN** apply hydration, retry, or process restart normalizes the same findings again
- **THEN** the same recovered payload is not appended a second time
- **AND** workspace-derived follow-up state remains deterministic

#### Scenario: pass cleanup retains recovered notes

- **GIVEN** a current runtime-owned follow-up and previously recovered notes exist
- **WHEN** acceptance returns PASS and runtime performs cleanup
- **THEN** the runtime-owned follow-up is removed
- **AND** recovered notes remain as non-task repository evidence

#### Scenario: recovered checkbox text is inert

- **GIVEN** recovered content contains headings and `- [ ]` or `- [x]` text inside a valid fenced literal
- **WHEN** Conflux calculates task progress or performs strict and archive-gate task validation
- **THEN** fenced checkbox text does not change task totals or completion totals
- **AND** fenced content does not create implementation-task validation findings

#### Scenario: ambiguous boundary remains a hard error

- **GIVEN** a possible runtime-owned follow-up contains an unclosed fence or another layout that prevents safe boundary determination
- **WHEN** runtime attempts replacement or PASS cleanup
- **THEN** runtime leaves the original tasks file byte-for-byte unchanged
- **AND** reports an actionable hard error identifying the structural ambiguity

#### Scenario: failed preservation does not destroy content

- **GIVEN** an unambiguous runtime-owned follow-up contains unknown content
- **AND** runtime cannot complete the atomic recovered-notes update
- **WHEN** replacement or cleanup is attempted
- **THEN** the original tasks file remains unchanged
- **AND** an acceptance FAIL remains the primary diagnosis while persistence degradation is supplemental

<!-- Expected canonical result after archive: acceptance follow-up updates preserve recoverable unknown content in inert workspace-local notes, remain idempotent and atomic, keep fenced text out of task accounting, and reserve hard errors for unsafe boundaries or failed preservation. -->
