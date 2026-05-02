## MODIFIED Requirements

### Requirement: エージェントコマンドの設定ファイル

オーケストレーターは JSONC 形式の設定ファイルを通じてエージェントコマンドを設定できなければならない（MUST）。

設定可能なコマンドには、通常の apply 用 `apply_command` に加えて、late empty-WIP retry 用の optional `apply_escalation_command` と、final empty-WIP stall 診断用の optional `apply_stall_diagnose_command` を含めてもよい（MAY）。

`apply_escalation_command` は通常 apply の代替コマンドとして扱われ、runtime が escalation 条件を満たした retry にのみ使用しなければならない（MUST）。未設定の場合、runtime は escalation phase で静かに通常 `apply_command` の挙動を継続しなければならない（MUST）。

`apply_stall_diagnose_command` は final empty-WIP stall の直前診断にのみ使用されなければならない（MUST）。未設定の場合、runtime は診断 phase を静かにスキップして従来の final stall へ進まなければならない（MUST）。

#### Scenario: optional escalation and diagnose commands are accepted

- **GIVEN** `.cflx.jsonc` contains top-level `apply_command`
- **AND** optional `apply_escalation_command`
- **AND** optional `apply_stall_diagnose_command`
- **WHEN** configuration is loaded
- **THEN** the merged configuration exposes all three command templates
- **AND** missing optional escalation/diagnose commands do not themselves cause config load failure

### Requirement: stall_detection 設定

オーケストレーターは進捗停滞検出の挙動を設定ファイルで制御できなければならない（MUST）。

`stall_detection` は既存の `enabled` / `threshold` に加えて、empty-WIP escalation policy として以下を受け入れなければならない（MUST）。

- `apply_escalation_after_empty_wip`: 何回連続 empty WIP のあとで escalation retry を開始するか
- `apply_escalation_max_uses_per_stall`: 1 回の stall sequence で escalation command を最大何回使うか

`apply_escalation_after_empty_wip` が設定される場合、それは `threshold` より小さくなければならない（MUST）。

`apply_escalation_max_uses_per_stall` が設定される場合、それは 1 以上でなければならない（MUST）。

#### Scenario: escalation policy values are valid only before the final stall threshold

- **GIVEN** `stall_detection.threshold = 5`
- **AND** `stall_detection.apply_escalation_after_empty_wip = 3`
- **AND** `stall_detection.apply_escalation_max_uses_per_stall = 2`
- **WHEN** configuration is loaded
- **THEN** configuration validation succeeds

#### Scenario: invalid escalation boundary is rejected

- **GIVEN** `stall_detection.threshold = 5`
- **AND** `stall_detection.apply_escalation_after_empty_wip = 5`
- **WHEN** configuration is loaded
- **THEN** configuration validation fails
- **AND** the error explains that escalation must begin before the final stall threshold

#### Scenario: missing optional commands do not trigger warnings or validation errors

- **GIVEN** `stall_detection.apply_escalation_after_empty_wip = 3`
- **AND** `stall_detection.apply_escalation_max_uses_per_stall = 2`
- **AND** neither `apply_escalation_command` nor `apply_stall_diagnose_command` is configured
- **WHEN** configuration is loaded
- **THEN** configuration validation succeeds
- **AND** no warning is emitted solely because the optional commands are absent
