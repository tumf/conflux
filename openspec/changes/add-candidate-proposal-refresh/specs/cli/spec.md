## ADDED Requirements

### Requirement: Client refresh-proposal verb

`cflx client refresh-proposal <change-id> --source-ref REF` MUST be a client verb that delegates to the owner's refresh-proposal command and preview resource. `--source-ref` is required. `--dry-run` selects the preview (operation `refresh_proposal_preview`, outcome `previewed`, exit `0`); `--expected-source COMMIT --expected-head COMMIT` selects the apply (operation `control_refresh_proposal`, outcomes `refreshed` and `unchanged`, exit `0`); exactly one of the two forms MUST be chosen, both expectations MUST be present together, and each MUST be a full-length 40-hex commit ID, otherwise the verb exits `usage_error` before contacting the owner. `--json` prints exactly one versioned envelope on stdout with `change_id` set; diagnostics go to stderr. Lifecycle refusals are `target_ineligible`; command-time refusals are `refresh_refused` (exit `28`) with `detail.reason` and `detail.effects`; owner, transport, revision, and not-found outcomes keep their existing tokens and exit statuses. The verb never runs Git, never takes the orchestration lock, and never submits start, retry, queue, mark, or stop intent.

#### Scenario: Preview then apply

- **GIVEN** a running command-capable owner with an eligible `error` candidate `alpha`
- **WHEN** the operator runs `cflx client refresh-proposal alpha --source-ref develop --dry-run --json` and then the apply form with the previewed `source_commit` and `candidate_head`
- **THEN** the first envelope has operation `refresh_proposal_preview` and outcome `previewed`
- **AND** the second has operation `control_refresh_proposal` and outcome `refreshed` with `detail.candidate_head_after`
- **AND** each invocation printed exactly one JSON object

#### Scenario: Malformed invocation is refused locally

- **WHEN** the verb is invoked without `--source-ref`, with `--dry-run` plus an expectation, with one expectation, or with a short commit ID
- **THEN** it exits `2` with `usage_error`
- **AND** no owner request is made

#### Scenario: Headless owner cannot refresh

- **GIVEN** the owner is a headless `cflx run` with no command executor
- **WHEN** the apply form is invoked
- **THEN** the outcome is `owner_not_command_capable`
- **AND** the preview form still answers from the read resource
