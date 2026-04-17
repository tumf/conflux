## Implementation Tasks

- [x] 1. Add a `cargo audit` step to the CI checks job (verification: see `.github/workflows/ci.yml` lines 67-71; run `make audit` to confirm cargo audit works locally)
- [x] 2. Add a `make audit` target and include it in `make check` (verification: `Makefile:102` defines `audit` target running `cargo audit`; `Makefile:107` adds `audit` to `check` dependencies; run `make -n audit` to confirm)
- [x] 3. Update the development guide with audit commands and policy (verification: `docs/guides/DEVELOPMENT.md:57-83` documents `make audit`, `cargo audit`, install instructions, and clarifies audit is not part of pre-commit hooks)

## Future Work

- Advisory exception handling, if needed later, should be defined in a separate proposal
