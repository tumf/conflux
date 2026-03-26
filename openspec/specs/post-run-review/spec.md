## Requirements

### Requirement: Post-run review MUST inspect canonical spec diffs

The documented review procedure after `cflx run` MUST require direct inspection of touched canonical files under `openspec/specs/**` in addition to generic git status and commit review.

#### Scenario: Operator reviews canonical spec diffs after run
- **GIVEN** `cflx run` has completed
- **WHEN** the operator performs the documented post-run review
- **THEN** the checklist includes a step to inspect the canonical `openspec/specs/**` diff
- **AND** the review is not considered complete after commit inspection alone

### Requirement: Post-run review MUST summarize canonical spec changes by change

The documented post-run summary MUST identify which canonical spec files changed for each archived change that landed so spec promotion results can be traced back to the originating change.

#### Scenario: Post-run summary maps archived change to canonical specs
- **GIVEN** multiple changes were archived during `cflx run`
- **WHEN** the operator writes the run summary
- **THEN** the summary names each landed change
- **AND** the summary lists the canonical spec files changed by that change

### Requirement: Post-run review MUST flag empty canonical diff for spec-only changes

The documented post-run checklist MUST treat a spec-only change that lands without a canonical `openspec/specs/**` diff as an anomaly that requires investigation.

#### Scenario: Spec-only change lands without canonical diff
- **GIVEN** a landed change is classified as `spec-only`
- **AND** the post-run review finds no canonical spec diff attributable to that change
- **WHEN** the operator completes the review checklist
- **THEN** the checklist reports the change as anomalous
- **AND** the operator is instructed not to treat the run as fully healthy until the missing spec promotion is explained
