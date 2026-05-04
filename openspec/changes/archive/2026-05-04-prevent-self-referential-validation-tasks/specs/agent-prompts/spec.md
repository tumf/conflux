## MODIFIED Requirements

### Requirement: acceptance プロンプトは差分コンテキストを提示する

archive-side guidance MAY reference the native validator during archive readiness checks, but when it does so it MUST use only the supported evidence enum values `off`, `warn`, or `error`. Archive-side guidance and bundled proposal guidance MUST NOT instruct agents to create final OpenSpec validation as a checkbox implementation task.

Final validation guidance, when present, MUST be represented as non-checkbox archive-gate text, such as a `## Final Validation` section.

#### Scenario: archive validation uses native evidence enum

**Given**: the archive path invokes native `cflx openspec validate`
**When**: evidence mode is requested during archive-side validation
**Then**: the command uses only `off`, `warn`, or `error`
**And**: it never emits `--evidence strict`

#### Scenario: prompts avoid final validation checkbox tasks

**Given**: an agent prompt or bundled proposal guidance instructs authors to include final OpenSpec validation guidance
**When**: the guidance is rendered or inspected
**Then**: it does not instruct the author to create a checkbox task for final validation of the same change
**And**: it uses non-checkbox archive-gate guidance instead

#### Scenario: self-referential validation blocker is explained

**Given**: archive-side validation detects a checkbox task that asks for final validation of the same change
**When**: archive reports the failure
**Then**: the prompt/error text identifies the self-referential final validation checkbox pattern
**And**: it tells the user to move final validation to a non-checkbox `Final Validation` section
