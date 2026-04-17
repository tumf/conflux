## MODIFIED Requirements

### Requirement: cflx-accept MUST preserve acceptance command-template single source

The dedicated `cflx-accept` skill MAY provide operation identity and scoped acceptance guidance, but it MUST NOT become the primary source of fixed acceptance procedure. The fixed acceptance procedure MUST remain defined by `.opencode/commands/cflx-accept.md`, and the acceptance output contract MUST be stated in a machine-readable form that is consistent with runtime verdict parsing and regression tests.

The canonical acceptance verdict is an unwrapped standalone line containing exactly one of `ACCEPTANCE: PASS`, `ACCEPTANCE: FAIL`, `ACCEPTANCE: CONTINUE`, or `ACCEPTANCE: BLOCKED`. Markdown wrappers are explicitly categorized as follows:

- **Forbidden in agent output**: markdown headings (`#`, `##`, etc.), blockquotes (`>`), bullets (`-`, `*`), fenced code blocks (`` ``` ``). The command template and skill guidance MUST instruct agents not to wrap the verdict marker in these forms.
- **Tolerated by parser (defensive)**: bold (`**`), italic (`*`), underline (`_`), heading prefixes (`#`+), blockquote prefixes (`>`), bullet prefixes (`-`). The runtime parser strips these before matching as a defense against common LLM formatting drift, but this tolerance does not make them canonical.
- **Rejected by parser**: verdict markers inside fenced code blocks are ignored (they may be examples, not actual verdicts).

#### Scenario: Acceptance command template defines a standalone machine-readable verdict

- **GIVEN** the orchestrator emits `load skills: cflx-accept`
- **AND** acceptance runs through the standard command template flow
- **WHEN** the final verdict is produced
- **THEN** the canonical output contract is an unwrapped standalone line containing exactly one of `ACCEPTANCE: PASS`, `ACCEPTANCE: FAIL`, `ACCEPTANCE: CONTINUE`, or `ACCEPTANCE: BLOCKED`
- **AND** the contract explicitly states that markdown headings, quotes, bullets, and fenced code blocks are forbidden around the verdict marker
- **AND** the runtime parser defensively tolerates common drift prefixes (heading `#`, quote `>`, bullet `-`, bold/italic) but this tolerance does not alter the canonical contract
- **AND** runtime parsing and regression tests enforce the documented contract instead of relying on implicit formatter behavior

#### Scenario: Acceptance ownership boundary stays explicit after workflow split

- **GIVEN** acceptance prompt construction, command-template instructions, dedicated skill guidance, and runtime parser enforcement are reviewed together
- **WHEN** a future refactor changes one of these surfaces
- **THEN** tests fail if fixed acceptance procedure or final verdict contract drifts out of sync across those surfaces
- **AND** Rust-side prompt builders continue to inject runtime context without replacing the command template as the authoritative acceptance procedure source
