## ADDED Requirements

### Requirement: Acceptance runtime configuration is validated separately

Configuration MUST expose `acceptance_max_runtime_secs` with a default of 1,800 seconds and a valid inclusive range of 300 through 10,800 seconds. Zero and out-of-range values MUST fail normal configuration validation with an actionable diagnostic. The key MUST follow existing global, project, custom, and CLI construction precedence and appear in generated configuration examples. The range constrains the dedicated key; a shorter positive `command_max_runtime_secs` MAY still produce an effective Acceptance limit below 300 seconds.

#### Scenario: Dedicated limit defaults safely

**Given**: no configuration layer sets `acceptance_max_runtime_secs`
**When**: configuration is loaded
**Then**: the dedicated Acceptance limit is 1,800 seconds

#### Scenario: Dedicated range is validated

**Given**: configuration sets the key to 0, 299, or 10,801
**When**: normal configuration validation runs
**Then**: loading fails with an actionable `300..=10800` diagnostic

#### Scenario: Valid boundaries load

**Given**: configuration sets the key to 300 or 10,800
**When**: normal configuration validation runs
**Then**: loading succeeds and preserves the selected value

#### Scenario: Standard precedence applies

**Given**: multiple existing configuration layers set the key
**When**: effective configuration is constructed
**Then**: the same precedence used by other command runtime settings determines the value
