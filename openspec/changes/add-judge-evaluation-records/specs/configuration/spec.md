## MODIFIED Requirements

### Requirement: Optional purpose-scoped judge command configuration

The orchestrator SHALL accept an optional top-level `judge_commands` object whose entries are named by judgment purpose rather than vendor or model. The first supported purpose SHALL be `parallel_dependency`.

A configured `parallel_dependency` entry SHALL define a non-empty argv command and non-empty pinned concrete request model; aliases ending in `-latest` SHALL be rejected. It MAY define bounded timeout, input-byte limit, output-byte limit, and Noul yes-threshold values. Its optional `mode` SHALL accept only `shadow`; omission SHALL normalize to `shadow` for backward compatibility. Omission of the complete judge entry SHALL preserve the existing required agent commands and behavior without warnings or validation errors.

The entry MAY define `evaluation`. Omitted `evaluation`, or `evaluation.enabled: false`, SHALL disable durable evaluation recording without creating evaluation directories, salt, or records. When `evaluation.enabled` is true, `retention_days` and `max_total_bytes` SHALL both be present and positive; `retention_days` SHALL be at most 3650 and `max_total_bytes` SHALL be at most 10 GiB. Optional bounds supplied while disabled SHALL still be validated. Invalid mode or evaluation settings SHALL fail configuration loading before orchestration starts.

A higher-priority configuration layer that supplies `parallel_dependency` SHALL atomically replace the complete lower-priority entry. It SHALL NOT merge its executable, model, mode, limits, threshold, or evaluation settings field-by-field with the replaced entry.

<!-- Expected canonical result after archive: configuration supports one optional product-neutral shadow dependency judge with separately opt-in bounded evaluation persistence, atomic precedence, and no effect when omitted. -->

#### Scenario: Existing configuration remains valid

**Given**: all existing required agent commands are present
**And**: `judge_commands` is absent
**When**: merged configuration is validated
**Then**: validation succeeds
**And**: no judge-related warning is emitted
**And**: no evaluation storage is created

#### Scenario: Configure a System One compatible command

**Given**: `judge_commands.parallel_dependency.command` is `["jev", "run", "-"]`
**And**: its model and bounded settings are valid
**When**: merged configuration is validated
**Then**: Conflux exposes the complete purpose-scoped configuration to `ParallelizationAnalyzer`
**And**: the command remains an argv array without shell interpolation
**And**: omitted mode behaves as `shadow`

#### Scenario: Enable isolated bounded evaluation

**Given**: `parallel_dependency.evaluation.enabled` is true
**And**: positive `retention_days` and `max_total_bytes` are within their hard maximums
**When**: merged configuration is validated
**Then**: Conflux exposes the evaluation policy to the shadow recorder
**And**: the policy does not enable any non-shadow judge mode

#### Scenario: Disabled evaluation remains side-effect free

**Given**: `evaluation` is omitted or its `enabled` value is false
**When**: configuration loads and dependency analysis runs
**Then**: no evaluation directory, salt, or record is created
**And**: existing aggregate diagnostics remain permitted

#### Scenario: Project override does not form a hybrid command

**Given**: global configuration defines a complete `parallel_dependency` entry including evaluation settings
**And**: project configuration defines another complete `parallel_dependency` entry
**When**: configuration precedence is applied
**Then**: the project entry wins as one complete value
**And**: no omitted project command, model, mode, limit, threshold, or evaluation subfield is inherited from the global entry

#### Scenario: Invalid judge settings are rejected

**Given**: `parallel_dependency` contains an empty command element, empty model, unsupported mode, invalid timeout or byte limit, non-finite or out-of-range threshold, enabled evaluation with missing or zero bounds, or evaluation bounds above their maximums
**When**: configuration validation runs
**Then**: loading fails before orchestration starts
**And**: the diagnostic names the invalid configuration field
