## ADDED Requirements

### Requirement: Native validator owns behavior-centric proposal checks

The native `cflx openspec validate` implementation MUST be the sole executable contract for behavior-centric proposal validation. It MUST detect behavior-changing proposal tasks that appear complete through artifact existence alone rather than through provable runtime behavior delivery.

#### Scenario: behavior-changing task without verification ownership is warned

- **GIVEN** an implementation or hybrid proposal contains a behavior-changing checkbox task with a `(verification: ...)` note
- **AND** the verification note does not declare one of `unit`, `integration`, `e2e`, `manual`, `benchmark`, or `not-testable`
- **WHEN** `cflx openspec validate <change-id> --strict --evidence warn` runs
- **THEN** validation succeeds
- **AND** it emits a warning that verification ownership is missing for that behavior-changing task

#### Scenario: artifact-oriented tasks dominate a runtime proposal

- **GIVEN** an implementation or hybrid proposal claims runtime behavior changes
- **AND** its active checkbox tasks are dominated by `define` / `document` / `describe` style tasks instead of implementation-facing wiring or behavior-delivery tasks
- **WHEN** `cflx openspec validate <change-id> --strict --evidence warn` runs
- **THEN** validation succeeds
- **AND** it emits a warning that artifact-oriented tasks dominate or match behavior-changing tasks

#### Scenario: executable surface lacks runnable verification

- **GIVEN** an implementation or hybrid proposal mentions a CLI, API, workflow, job, worker, or background process as part of the requested behavior
- **AND** its task verification notes do not include a runnable verification path that exercises the executable surface
- **WHEN** `cflx openspec validate <change-id> --strict --evidence warn` runs
- **THEN** validation succeeds
- **AND** it emits a warning that executable-surface behavior lacks runnable verification coverage

#### Scenario: runtime behavior claim lacks implementation-facing tasks

- **GIVEN** an implementation or hybrid proposal claims runtime behavior changes such as handlers, webhook delivery, persistence, notifications, commands, or jobs
- **AND** its checkbox tasks do not include implementation-facing behavior tasks
- **WHEN** `cflx openspec validate <change-id> --strict --evidence warn` runs
- **THEN** validation succeeds
- **AND** it emits a warning that runtime behavior is claimed without implementation-facing tasks

## MODIFIED Requirements

### Requirement: Bundled skills use native OpenSpec CLI commands

Conflux skill sources, active canonical specs, and repository-facing validator guidance MUST instruct agents and users to call native `cflx openspec` subcommands for list/show/validate/archive operations. The repository MUST NOT depend on `skills/cflx-proposal/scripts/cflx.py` as an executable validator contract once native CLI parity is complete.

#### Scenario: repository no longer retains cflx.py validator helper

- **GIVEN** the native Rust validator already covers the proposal validation contract used by active Conflux workflows
- **WHEN** repository validation and skill-distribution checks are executed
- **THEN** the repository does not contain `skills/cflx-proposal/scripts/cflx.py`
- **AND** proposal validation continues to work through `cflx openspec validate ...`
- **AND** no active canonical spec or skill-facing documentation instructs the user to execute `cflx.py`
