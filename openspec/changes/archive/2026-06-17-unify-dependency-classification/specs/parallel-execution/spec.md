## ADDED Requirements

### Requirement: Dependency classification logic is centralized

The scheduler SHALL maintain a single `DependencyContext` implementation that encapsulates the construction of queued, in-flight, active, archived, rejected, and terminal-error lookup sets, as well as the `classify_dependency_target` and `effective_dependency_base` decision logic. `classify_queued_work`, `select_changes_for_dispatch`, and any future callers SHALL delegate to this shared context rather than duplicating HashSet construction or classification loops.

#### Scenario: Archived dependency uses effective base after branch switch

- **GIVEN** a change is archived on the `integration` branch but not on `main`
- **AND** the executor `repo_root` is on the `integration` branch
- **WHEN** the scheduler evaluates a dependent change that declares the archived change as a dependency
- **THEN** the dependency is treated as satisfied on the effective base (`integration`)
- **AND** the dependent change becomes eligible for dispatch

#### Scenario: Dependency classification is consistent between analysis and dispatch

- **GIVEN** a change is queued and blocked by a terminal-error dependency
- **WHEN** `classify_queued_work` and `select_changes_for_dispatch` are both called during the same scheduler iteration
- **THEN** both functions classify the dependency target identically
- **AND** the change is excluded from analysis and dispatch without duplication of classification logic
