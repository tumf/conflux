## MODIFIED Requirements

### Requirement: Native validator owns behavior-centric proposal checks

The native `cflx openspec validate` implementation MUST enforce deterministic proposal authoring contracts such as structural validity, verification-note presence, supported evidence enum usage, and other repository-verifiable formatting rules. It MUST NOT infer implementation-task adequacy solely from wording heuristics about runtime behavior claims or whether tasks appear implementation-facing.

#### Scenario: validator does not emit runtime-behavior wording heuristic

- **GIVEN** an implementation or hybrid proposal claims runtime behavior changes
- **AND** its task wording may or may not look implementation-facing
- **WHEN** `cflx openspec validate <change-id> --strict --evidence warn` runs
- **THEN** validation does not emit a finding based solely on heuristic inference that runtime behavior lacks implementation-facing tasks
- **AND** any remaining findings come from deterministic authoring-contract checks rather than acceptance-style quality judgment
