## ADDED Requirements

### Requirement: Optional purpose-scoped judge command configuration

The orchestrator SHALL accept an optional top-level `judge_commands` object whose entries are named by judgment purpose rather than vendor or model. The first supported purpose SHALL be `parallel_dependency`.

A configured `parallel_dependency` entry SHALL define a non-empty argv command and non-empty pinned concrete request model; aliases ending in `-latest` SHALL be rejected. It MAY define bounded timeout, input-byte limit, output-byte limit, and Noul yes-threshold values. Omission SHALL preserve the existing required agent commands and behavior without warnings or validation errors.

A higher-priority configuration layer that supplies `parallel_dependency` SHALL atomically replace the complete lower-priority entry. It SHALL NOT merge its executable, model, limits, or threshold field-by-field with the replaced entry.

<!-- Expected canonical result after archive: configuration supports one optional product-neutral dependency judge with atomic precedence and no effect when omitted. -->

#### Scenario: Existing configuration remains valid

**Given**: all existing required agent commands are present
**And**: `judge_commands` is absent
**When**: merged configuration is validated
**Then**: validation succeeds
**And**: no judge-related warning is emitted

#### Scenario: Configure a System One compatible command

**Given**: `judge_commands.parallel_dependency.command` is `["jev", "run", "-"]`
**And**: its model and bounded settings are valid
**When**: merged configuration is validated
**Then**: Conflux exposes the complete purpose-scoped configuration to `ParallelizationAnalyzer`
**And**: the command remains an argv array without shell interpolation

#### Scenario: Project override does not form a hybrid command

**Given**: global configuration defines a complete `parallel_dependency` entry
**And**: project configuration defines another complete `parallel_dependency` entry
**When**: configuration precedence is applied
**Then**: the project entry wins as one complete value
**And**: no omitted project subfield is inherited from the global entry

#### Scenario: Invalid judge settings are rejected

**Given**: `parallel_dependency` contains an empty command element, empty model, invalid timeout or byte limit, or a non-finite or out-of-range threshold
**When**: configuration validation runs
**Then**: loading fails before orchestration starts
**And**: the diagnostic names the invalid configuration field
