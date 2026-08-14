## MODIFIED Requirements

### Requirement: Persistent scheduler idle is a typed resumable transition

When a persistent scheduler reaches a coherent fully-drained or stable blocked/waiting-only state, it MUST remain alive in its existing event-driven wait and MUST emit one typed persistent-idle transition before parking. When that transition observes frontend Running, it MUST project Ready/`select` and set a process-local `persistent_scheduler_idle` idle-episode fact without claiming completion or changing reducer-owned lifecycle, queue intent, blockers, wait state, worktree evidence, diagnostics, or process-local execution marks. A late transition observed in Select, Stopping, Error, or Stopped MUST leave both mode and the idle-episode fact unchanged.

The transition MUST be edge-triggered once per continuous idle episode. Repeated scheduler evaluation or a generic wake notification that adds no scheduler-visible intent MUST NOT emit another idle transition. An accepted Start outcome MAY close the frontend idle episode and project Running before admitted workspace work begins only after reducer queue or explicit-retry intent commits. The scheduler MUST rearm the idle edge when a coherent reconciliation pass observes those committed queue additions or consumes the accepted explicit-retry edge. Dependency analysis, catalog refresh, and generic notification alone MUST NOT rearm the edge or project Running.

The first typed admitted-work event after idle MUST continue to establish actual workspace/base-lane execution. Application mode and actual execution observation remain separate: accepted Start MAY project Running, while active-work and phase facts MUST still require typed dependency-analysis or lifecycle evidence. If reconciled intent produces no admitted work, the next persistent park MUST emit one fresh idle transition and return the frontends to Ready. Persistent lifetime, explicit wake sources, and non-polling behavior MUST remain unchanged.

#### Scenario: Accepted idle Start rearms the next idle edge

- **GIVEN** TUI and Web are Ready while a persistent scheduler remains alive
- **AND** shared run control accepts Start for one or more marked targets
- **WHEN** reducer queue or explicit-retry intent commits and the scheduler reconciles that intent
- **THEN** the accepted outcome projects Running immediately
- **AND** the persistent-idle latch is rearmed exactly once for the new run episode
- **AND** the existing scheduler is notified without spawning another scheduler

#### Scenario: No admitted work parks back to Ready

- **GIVEN** accepted Start projected Running and rearmed the idle edge
- **WHEN** analysis or classification starts no workspace or base-lane operation
- **AND** the coherent persistent-idle predicate becomes true again
- **THEN** the scheduler emits one fresh persistent-idle transition before parking
- **AND** TUI and Web return to Ready
- **AND** the same scheduler remains alive for later explicit wake

#### Scenario: Generic no-op wake does not flicker mode

- **GIVEN** a persistent scheduler already emitted the idle transition
- **WHEN** a duplicate notification or generic wake adds no reducer queue or explicit-retry intent
- **THEN** the idle latch remains set
- **AND** no Running projection is synthesized
- **AND** no additional persistent-idle transition is emitted

#### Scenario: Actual work evidence remains typed

- **GIVEN** accepted Start has projected Running before dependency analysis begins
- **WHEN** dependency analysis or workspace/base-lane work actually starts
- **THEN** its existing typed event opens the corresponding process activity or lifecycle phase
- **AND** application mode alone is not used as proof of that activity

<!-- Expected canonical result after archive: persistent idle will remain edge-triggered and non-polling while accepted queue/retry intent can open Running immediately and deterministically rearm a no-work return-to-Ready edge. -->
