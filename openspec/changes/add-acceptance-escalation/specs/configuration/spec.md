## MODIFIED Requirements

### Requirement: エージェントコマンドの設定ファイル

オーケストレーターは JSONC 形式の設定ファイルを通じてエージェントコマンドを設定できなければならない（MUST）。

設定可能なコマンドには、通常の apply 用 `apply_command`、late empty-WIP retry 用の optional `apply_escalation_command`、final empty-WIP stall 診断用の optional `apply_stall_diagnose_command` に加えて、invalid Acceptance result retry 用の optional `acceptance_escalation_command` を含めてもよい（MAY）。

`apply_escalation_command` は通常 apply の代替コマンドとして扱われ、runtime が escalation 条件を満たした retry にのみ使用しなければならない（MUST）。未設定の場合、runtime は escalation phase で静かに通常 `apply_command` の挙動を継続しなければならない（MUST）。

`apply_stall_diagnose_command` は final empty-WIP stall の直前診断にのみ使用されなければならない（MUST）。未設定の場合、runtime は診断 phase を静かにスキップして従来の final stall へ進まなければならない（MUST）。

`acceptance_escalation_command` は通常 Acceptance の代替 reviewer command として扱われ、runtime が Acceptance escalation 条件を満たした retry にのみ使用しなければならない（MUST）。未設定の場合、runtime は escalation eligibility が成立しても既存の通常 `acceptance_command` retry behavior を継続しなければならない（MUST）。

<!-- Expected canonical result after archive: configuration accepts one optional Acceptance escalation reviewer command without making it required, and the existing optional apply escalation/diagnose commands keep their behavior and scenario. -->

#### Scenario: optional escalation and diagnose commands are accepted

- **GIVEN** `.cflx.jsonc` contains top-level `apply_command`
- **AND** optional `apply_escalation_command`
- **AND** optional `apply_stall_diagnose_command`
- **WHEN** configuration is loaded
- **THEN** the merged configuration exposes all three command templates
- **AND** missing optional escalation/diagnose commands do not themselves cause config load failure

#### Scenario: optional Acceptance escalation command is accepted

- **GIVEN** merged configuration contains required `acceptance_command`
- **AND** it contains optional `acceptance_escalation_command`
- **WHEN** configuration is loaded
- **THEN** both command templates are exposed
- **AND** omitting `acceptance_escalation_command` does not cause warning or validation failure

## ADDED Requirements

### Requirement: Acceptance escalation policy is bounded and configurable

The orchestrator MUST accept an optional top-level `acceptance_escalation` object containing positive integer `after_invalid_results` and `max_uses_per_sequence`. Their defaults MUST both be `1`. Configuration layers MUST merge the two policy fields item-wise. Zero MUST be rejected with an actionable configuration error.

#### Scenario: default policy escalates the next invalid-result retry once

- **GIVEN** `acceptance_escalation_command` is configured
- **AND** no Acceptance escalation policy values are configured
- **WHEN** one eligible invalid Acceptance result completes
- **THEN** the next permitted Acceptance-only retry uses the escalation command
- **AND** no later retry in the same invalid-result sequence uses it again

#### Scenario: policy values merge independently

- **GIVEN** a lower-priority layer configures both policy fields
- **AND** a higher-priority layer overrides only `after_invalid_results`
- **WHEN** configuration is merged
- **THEN** the higher-priority threshold is used
- **AND** the lower-priority maximum-use value is retained

#### Scenario: zero policy value is rejected

- **GIVEN** either policy value is zero
- **WHEN** configuration validation runs
- **THEN** validation fails
- **AND** the diagnostic identifies the invalid field and positive-value requirement
