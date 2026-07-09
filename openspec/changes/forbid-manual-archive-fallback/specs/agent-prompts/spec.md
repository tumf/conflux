## MODIFIED Requirements

### Requirement: acceptance プロンプトは差分コンテキストを提示する

Archive-side guidance MAY reference the native validator during archive readiness checks, but when it does so it MUST use only the supported evidence enum values `off`, `warn`, or `error`. Archive-side guidance and bundled proposal guidance MUST NOT instruct agents to create final OpenSpec validation as a checkbox implementation task.

Bundled proposal guidance MUST instruct authors to inspect canonical spec requirement headings before creating `MODIFIED Requirements` or `REMOVED Requirements` deltas. If the target canonical heading does not exist, authors MUST use `ADDED Requirements` for a new requirement identity or correct the target name before validation.

Final validation guidance, when present, MUST be represented as non-checkbox archive-gate text, such as a `## Final Validation` section.

Archive-side guidance MUST instruct agents that the only supported archive mutation command is `cflx openspec archive <change_id> --yes`, or `cflx openspec archive <change_id> --yes --skip-specs` for tooling-only changes. Archive-side guidance MUST explicitly prohibit direct archive layout mutation with `mkdir`, `mv`, `git mv`, scripts, or equivalent filesystem operations under `openspec/changes/archive/`. If the CLI archive command fails, archive-side guidance MUST require terminal failure rather than manual archive repair.

#### Scenario: proposal guidance requires canonical heading lookup

**Given**: bundled `cflx-proposal` guidance is used to author a spec delta
**When**: the requested delta modifies or removes an existing requirement
**Then**: the guidance tells the author to inspect `openspec/specs/<capability>/spec.md` for the canonical `### Requirement:` heading
**And**: it tells the author to use the canonical target heading for `MODIFIED` or `REMOVED`
**And**: it tells the author to use `ADDED` when no canonical target exists

#### Scenario: final validation is not represented as checkbox task

**Given**: proposal or archive guidance includes final OpenSpec validation
**When**: tasks are written
**Then**: final validation is represented outside checkbox task lists
**And**: archive-gate validation remains the authoritative final gate

#### Scenario: archive guidance prohibits manual archive fallback

- **GIVEN** archive-side guidance is used after acceptance passes
- **WHEN** `cflx openspec archive <change_id> --yes` fails
- **THEN** the guidance instructs the agent to stop with terminal archive failure
- **AND** the guidance does not permit `mkdir`, `mv`, `git mv`, scripts, or equivalent filesystem operations to create or move `openspec/changes/archive/` entries
- **AND** the guidance does not permit a success-style archive commit after the failed CLI archive command
