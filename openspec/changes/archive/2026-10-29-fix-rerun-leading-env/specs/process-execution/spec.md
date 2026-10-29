## ADDED Requirements

### Requirement: Declared verification environment bindings apply to the supervised child only

When Conflux supervises a declared verification command that carries environment bindings parsed from leading `NAME=value` assignments, it MUST start the executable directly with those bindings applied to the child's environment on top of the environment it would otherwise inherit. Conflux MUST NOT mutate its own process environment, MUST NOT interpose `sh -c`, `env`, or any other intermediary process, and MUST keep the child in the owned session/process group the existing gate supervision requires so termination and cleanup evidence are unchanged. The executable is resolved from the first non-assignment token only.

#### Scenario: Child observes the binding and the owner does not

- **GIVEN** a supervised request with bindings `A=1` and executable argv `printenv A`
- **WHEN** the direct supervisor runs it
- **THEN** the child exits `0` with output `1`
- **AND** the owner process has no `A` variable afterwards
- **AND** the same request with no bindings exits non-zero

#### Scenario: Supervision evidence is unchanged by bindings

- **GIVEN** a supervised request with bindings whose child exceeds the gate budget
- **WHEN** the deadline expires
- **THEN** the existing process-group termination, reaping, and cleanup-evidence path applies exactly as for a request without bindings
