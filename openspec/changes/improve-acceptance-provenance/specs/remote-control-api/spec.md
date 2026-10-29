## ADDED Requirements

### Requirement: Acceptance decision provenance is published per change

Each projected change MUST carry an additive nullable `acceptance_provenance` object describing the latest Acceptance settlement observed by this owner incarnation, built from typed manifest and verdict facts rather than from prose. It MUST contain `decision_source` (`native_verification` | `reviewer`), `reviewer_started`, a closed `reason_code`, `defect_code`, `verification_id`, `declaration_path`, `candidate_commit`, `proposal_content_id`, `change_input_digest`, `program`, `argument_count`, `env_names`, a closed `remediation` (`fix_declaration` | `satisfy_environment` | `repair_candidate` | `retry` | `refresh_candidate` | `none`), and `candidate_divergence` (`status` `identical` | `diverged` | `unknown`, with `source_ref`, `source_commit`, `source_blob`, `candidate_blob`, `reason`). It MUST NOT contain command argument values, environment values, prompt text, or process output; `argument_count` and `env_names` stand in for argv. It MUST be `null` for a change with no settlement in this incarnation and after owner restart. Existing fields MUST keep their meaning, and the generated OpenAPI document MUST document the field and every token. The field is observability only: no command admission, retry classification, archive, merge, or next-action decision MAY read it.

#### Scenario: Native pre-reviewer stop is attributable

- **GIVEN** Acceptance settled on a defective required declaration before any reviewer started
- **WHEN** a client reads the change row
- **THEN** `acceptance_provenance.decision_source` is `native_verification`, `reviewer_started` is `false`, `reason_code` is `declaration_invalid`, `defect_code` is the planner's code, and `verification_id` and `declaration_path` name the declaration
- **AND** `candidate_commit` equals the manifest candidate and `proposal_content_id` equals the blob of the candidate `proposal.md` at that commit

#### Scenario: Reviewer verdict is distinctly labeled

- **GIVEN** a reviewer subprocess ran and returned `fail`
- **WHEN** a client reads the change row
- **THEN** `decision_source` is `reviewer`, `reviewer_started` is `true`, `reason_code` is `reviewer_fail`, `verification_id` is `null`, and `remediation` is `repair_candidate`

#### Scenario: Environment and tool absence map to their remediation

- **GIVEN** one settlement on `tool_unavailable` and one on a failed gate with environment bindings
- **WHEN** the rows are read
- **THEN** the first carries `remediation: satisfy_environment` with `program` naming the unresolved token
- **AND** the second carries `repair_candidate`, `argument_count`, and `env_names`, and no argument or environment value appears anywhere in the row

#### Scenario: Divergence is explicit or unknown, never guessed

- **GIVEN** the manifest bound a review base whose `proposal.md` differs from the candidate's
- **WHEN** the row is read
- **THEN** `candidate_divergence.status` is `diverged` with both blob IDs, the base ref, and the base commit
- **AND** a `declaration_invalid` settlement on such a candidate recommends `refresh_candidate`
- **AND** without a manifest, with an unresolvable base, or with a missing blob the status is `unknown` with a stated reason

#### Scenario: Provenance is not authority

- **GIVEN** two owner states that differ only in `acceptance_provenance`, and a state root that has been deleted
- **WHEN** retry eligibility, admission, and the next action are computed
- **THEN** all three are identical
- **AND** restart publishes `null` without changing the next action
