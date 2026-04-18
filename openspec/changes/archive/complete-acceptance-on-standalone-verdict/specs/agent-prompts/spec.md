## MODIFIED Requirements

### Requirement: cflx-accept MUST preserve acceptance command-template single source

The dedicated `cflx-accept` skill MAY provide operation identity and scoped acceptance guidance, but it MUST NOT become the primary source of fixed acceptance procedure. The fixed acceptance procedure MUST remain defined by `.opencode/commands/cflx-accept.md`, and the acceptance output contract MUST be stated in a machine-readable form that is consistent with runtime verdict parsing and regression tests.

The canonical acceptance verdict is an unwrapped standalone line containing exactly one of `ACCEPTANCE: PASS`, `ACCEPTANCE: FAIL`, `ACCEPTANCE: CONTINUE`, or `ACCEPTANCE: BLOCKED`. Markdown wrappers are explicitly categorized as follows:

- **Forbidden in agent output**: markdown headings (`#`, `##`, etc.), blockquotes (`>`), bullets (`-`, `*`), fenced code blocks (`` ``` ``).
- **Tolerated by parser (defensive)**: bold (`**`), italic (`*`), underline (`_`), heading prefixes (`#`+), blockquote prefixes (`>`), bullet prefixes (`-`) when the verdict line still remains standalone after stripping those wrappers.
- **Rejected by canonical parser**: verdict lines with trailing text concatenated onto the marker itself, including `ACCEPTANCE: PASSAll ...` and `ACCEPTANCE: PASS## ...`.

#### Scenario: canonical verdict is a standalone line only

- **GIVEN** acceptance runs through the standard command template flow
- **WHEN** the final verdict is produced
- **THEN** the canonical output contract is an unwrapped standalone line containing exactly one of `ACCEPTANCE: PASS`, `ACCEPTANCE: FAIL`, `ACCEPTANCE: CONTINUE`, or `ACCEPTANCE: BLOCKED`
- **AND** trailing prose or headings concatenated onto that line are not valid canonical verdicts

#### Scenario: command template remains the source of verdict formatting guidance

- **GIVEN** the acceptance prompt loads `cflx-accept`
- **WHEN** the agent is instructed how to emit the final verdict
- **THEN** the formatting rule is defined by `.opencode/commands/cflx-accept.md`
- **AND** the skill may reinforce operation identity but does not replace the template as the source of canonical verdict formatting
