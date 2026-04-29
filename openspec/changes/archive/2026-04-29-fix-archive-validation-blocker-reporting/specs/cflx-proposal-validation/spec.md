## MODIFIED Requirements

### Requirement: Native validator owns behavior-centric proposal checks

Any Conflux runtime, skill guidance, or archive-side verification path that invokes native `cflx openspec validate` with evidence checking MUST use only the supported evidence enum values `off`, `warn`, or `error`. Runtime-owned archive validation MUST NOT synthesize unsupported evidence mode names.

#### Scenario: archive runtime does not emit unsupported evidence mode
- **GIVEN** archive execution performs strict proposal validation with evidence checking enabled
- **WHEN** the runtime constructs the native validation command
- **THEN** the command uses `--evidence off`, `--evidence warn`, or `--evidence error`
- **AND** it does not emit `--evidence strict`
