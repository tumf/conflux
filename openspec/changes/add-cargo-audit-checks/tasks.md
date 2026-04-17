## Implementation Tasks

- [x] 1. Add a `cargo audit` step to the CI checks job (verification: integration - `.github/workflows/ci.yml`; confirm the workflow contains the audit step and `cflx openspec validate add-cargo-audit-checks --strict` succeeds)
- [x] 2. Add a `make audit` target and include it in `make check` (verification: integration - `Makefile`; confirm `audit` is defined and `check` depends on it)
- [x] 3. Update the development guide with audit commands and policy (verification: manual - `docs/guides/DEVELOPMENT.md`; confirm it documents `make audit`, `cargo audit`, and that pre-commit or prek does not run audit automatically)

## Future Work

- Advisory exception handling, if needed later, should be defined in a separate proposal
