## ADDED Requirements

### Requirement: Workflow acceptance respects planned verification ownership

The Conflux workflow guidance MUST evaluate implementation evidence in the context of the verification path planned by the proposal so projects using Conflux can distinguish missing coverage from intentionally non-unit verification.

#### Scenario: Manual verification plan is treated as intentional coverage

**Given**: A proposal plans `manual` verification for a UX-oriented requirement
**When**: the `cflx-workflow` acceptance guidance evaluates the change
**Then**: the guidance does not treat the absence of unit or integration tests alone as a failure
**And**: it instead expects manual verification ownership to be tracked explicitly

#### Scenario: Benchmark verification plan is treated as intentional coverage

**Given**: A proposal plans `benchmark` verification for a performance requirement
**When**: the `cflx-workflow` acceptance guidance evaluates the change
**Then**: the guidance recognizes benchmark evidence as the intended verification path
**And**: it does not require a unit-test substitute merely because the requirement is behavior-changing

### Requirement: Workflow checks verification type and evidence type consistency

The workflow guidance MUST distinguish between the planned verification type and the actual evidence type so mislabeled coverage is surfaced during apply or accept.

#### Scenario: Unit-planned task only has integration-style evidence

**Given**: A proposal task claims unit verification ownership
**And**: the available evidence exercises real filesystem, process, VCS, network, database, or other stateful boundaries
**When**: the `cflx-workflow` guidance evaluates task truthfulness
**Then**: the guidance treats the result as a verification mismatch rather than valid unit-test completion
**And**: follow-up work is required to either extract unit-testable logic or reclassify the coverage

### Requirement: Workflow flags missing verification planning

The workflow guidance MUST allow acceptance to report missing or ambiguous verification planning when behavior-changing work has no clear verification ownership.

#### Scenario: Behavior-changing work has no planned verification path

**Given**: A proposal introduces behavior-changing work
**And**: the proposal or tasks do not make the planned verification ownership clear
**When**: the `cflx-workflow` acceptance guidance reviews the change
**Then**: the guidance permits a finding that verification planning is incomplete
**And**: the finding explains that proposal planning and workflow enforcement must align
