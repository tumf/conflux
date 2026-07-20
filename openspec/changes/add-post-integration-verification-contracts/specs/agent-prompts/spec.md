## ADDED Requirements

### Requirement: Acceptance MUST honor declared verification phases

Acceptance guidance and bundled acceptance skills MUST treat structured verification phase declarations as authoritative. For pre-integration verification, acceptance MUST evaluate current-revision repository evidence before archive. For post-integration verification, acceptance MUST evaluate the tracked automation, trigger, evidence publication contract, rerun action, prerequisites, and fixture/local verification evidence without accessing an undeployed or external target.

Missing, placeholder, or incorrectly wired repository automation MUST produce FAIL because repository edits can resolve it. A correctly wired post-integration verification MUST NOT produce FAIL solely because its operational evidence does not exist before integration. A non-mockable external prerequisite that makes the declared automation unusable MUST produce a stalled-hold compatibility verdict with the prerequisite and next action preserved. Acceptance MUST NOT claim an unobserved post-integration operational outcome succeeded.

#### Scenario: acceptance verifies post-integration automation instead of public target

**Given**: a change declares a post-integration deployment verification
**And**: the deployment has not run because the change is not integrated
**When**: pre-integration acceptance reviews the change
**Then**: acceptance verifies repository automation and local tests
**And**: acceptance does not fetch the undeployed public target
**And**: missing pre-integration operational evidence alone is not a FAIL

#### Scenario: missing workflow wiring remains repository-fixable

**Given**: a post-integration declaration references a workflow whose trigger or evidence publication is not implemented
**When**: acceptance reviews repository evidence
**Then**: acceptance returns FAIL with an actionable repository finding
**And**: apply can repair the workflow or tests

#### Scenario: external prerequisite becomes a truthful stalled hold

**Given**: post-integration automation is repository-complete
**And**: a required non-mockable external environment or approval is unavailable
**When**: acceptance determines repository edits cannot resolve the prerequisite
**Then**: acceptance emits the current stalled-hold compatibility verdict
**And**: the blocker summary identifies the prerequisite, owner, and rerun or unblock action

#### Scenario: operational outcome remains pending after pre-integration acceptance

**Given**: post-integration automation and local verification pass
**And**: its external run has not occurred
**When**: acceptance returns its verdict
**Then**: Conflux may accept the Definition of Implemented
**And**: Conflux does not describe the Operational Outcome as successful
**And**: the declaration remains available after archive for operator inspection
