## ADDED Requirements

### Requirement: Candidate proposal refresh API and clients

The v2 command registry MUST accept `refresh_proposal` with exactly one non-empty `change_id`, a non-empty `source_ref`, full-length `expected_source` and `expected_head` commit IDs, ordinary expected-revision fencing, idempotency identity, and `deny_unknown_fields`. The preview MUST be the read resource `GET /api/v2/proposals/{change_id}/refresh-preview?source_ref=<ref>`, served on both listeners, creating no command record and advancing no revision, and returning `instance_id`, the observed `state_revision`, `eligible`, a `refusal` reason when any precondition fails, `source_ref`, `source_commit`, `candidate_head`, `path`, `source_blob`, `candidate_blob`, `no_op`, and a unified `patch`. Each projected change MUST publish `actions.refresh_proposal` eligibility with the lifecycle reasons defined by `operator-command-execution`. A settled result of kind `refresh_proposal` MUST carry `outcome` (`refreshed` or `unchanged`), `source_commit`, `source_blob`, `candidate_head_before`, `candidate_head_after`, `candidate_blob_before`, `path`, `effects`, and a bounded hook output tail; a refused command MUST carry the stable reason and the same `effects` object. A lifecycle refusal of the command MUST settle with error code `target_ineligible`; a command-time refusal MUST settle with the distinct error code `refresh_refused`, and its record MUST carry the `refresh_proposal` result so the effects remain readable. Capabilities MUST advertise the command and the preview resource, and the generated OpenAPI document MUST include the command variant, the preview route, the result kind, and the action field.

The CLI MUST expose `cflx client refresh-proposal <change-id> --source-ref REF` with `--dry-run` (operation `refresh_proposal_preview`, success outcome `previewed`) or `--expected-source COMMIT --expected-head COMMIT` (operation `control_refresh_proposal`, success outcomes `refreshed` and `unchanged`), each with `--json` printing one envelope. Lifecycle refusals MUST map to `target_ineligible`; command-time refusals MUST map to the outcome `refresh_refused` with its own exit status `28` and `detail.reason` plus `detail.effects`. The client MUST delegate through the ordinary v2 command and the preview resource, MUST NOT run Git itself, and MUST NOT expose the command over `cflx client mcp` in this change.

#### Scenario: Preview is a read

- **GIVEN** an eligible candidate
- **WHEN** a client reads `/api/v2/proposals/alpha/refresh-preview?source_ref=develop`
- **THEN** the response carries the preview facts and `eligible: true`
- **AND** no command record exists and the state revision is unchanged

#### Scenario: Apply is an ordinary fenced command

- **GIVEN** a preview reported `source_commit` S and `candidate_head` H
- **WHEN** `refresh_proposal` is submitted at the current revision with `expected_source: S` and `expected_head: H`
- **THEN** one command record is created and settles with kind `refresh_proposal` and `outcome: refreshed`
- **AND** a stale `expected_revision` is refused with the ordinary revision conflict and no Git effect

#### Scenario: Per-change eligibility is authoritative

- **GIVEN** `alpha` is an idle `error` candidate with a worktree and `beta` is `applying`
- **WHEN** clients inspect state
- **THEN** `alpha.actions.refresh_proposal` is allowed
- **AND** `beta.actions.refresh_proposal` carries `target_active`

#### Scenario: CLI validates the invocation before contacting the owner

- **WHEN** `cflx client refresh-proposal alpha` is invoked without `--source-ref`, with `--dry-run` and an expectation together, with only one expectation, or with a non-40-hex expectation
- **THEN** it returns `usage_error` before any owner request
- **AND** no command record is created

#### Scenario: CLI reports typed refusals and effects

- **GIVEN** an apply whose candidate `pre-commit` hook rejects the commit
- **WHEN** `cflx client refresh-proposal alpha --source-ref develop --expected-source S --expected-head H --json` runs
- **THEN** the envelope outcome is `refresh_refused` with exit status `28`
- **AND** `detail.reason` is `hook_rejected` and `detail.effects` reports `file_written: true, staged: true, committed: false`

#### Scenario: Contract completeness

- **GIVEN** a client captures `cflx openapi`
- **WHEN** it inspects the document
- **THEN** it contains the `refresh_proposal` command variant, the refresh-preview route, the `refresh_proposal` result kind, and `actions.refresh_proposal`
