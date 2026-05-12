## MODIFIED Requirements

### Requirement: acceptance プロンプトは差分コンテキストを提示する

Archive-side guidance MAY reference the native validator during archive readiness checks, but when it does so it MUST use only the supported evidence enum values `off`, `warn`, or `error`. Archive-side guidance and bundled proposal guidance MUST NOT instruct agents to create final OpenSpec validation as a checkbox implementation task.

Bundled proposal guidance MUST instruct authors to inspect canonical spec requirement headings before creating `MODIFIED Requirements` or `REMOVED Requirements` deltas. If the target canonical heading does not exist, authors MUST use `ADDED Requirements` for a new requirement identity or correct the target name before validation.

Final validation guidance, when present, MUST be represented as non-checkbox archive-gate text, such as a `## Final Validation` section.

#### Scenario: proposal guidance requires canonical heading lookup

**Given**: bundled `cflx-proposal` guidance is used to author a spec delta
**When**: the requested delta modifies or removes an existing requirement
**Then**: the guidance tells the author to inspect `openspec/specs/<capability>/spec.md` for the canonical `### Requirement:` heading
**And**: it tells the author to use the canonical target heading for `MODIFIED` or `REMOVED`
**And**: it tells the author to use `ADDED` when no canonical target exists

#### Scenario: proposal guidance validates target selection before handoff

**Given**: a proposal with spec deltas has been authored
**When**: proposal authoring is complete
**Then**: bundled guidance requires running `cflx openspec validate <id> --strict`
**And**: missing `MODIFIED` or `REMOVED` targets are treated as proposal authoring errors rather than deferred archive blockers
