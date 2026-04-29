## MODIFIED Requirements

### Requirement: acceptance-detected archive blocker survives later archive verification noise

archive-side guidance MAY reference the native validator during archive readiness checks, but when it does so it MUST use only the supported evidence enum values `off`, `warn`, or `error`. This change does not redefine the root-cause-preserving archive failure contract already covered by archived archive-readiness work.

#### Scenario: archive validation uses native evidence enum
- **GIVEN** the archive path invokes native `cflx openspec validate`
- **WHEN** evidence mode is requested during archive-side validation
- **THEN** the command uses only `off`, `warn`, or `error`
- **AND** it never emits `--evidence strict`
