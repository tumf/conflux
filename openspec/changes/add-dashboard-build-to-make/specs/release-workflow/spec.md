## MODIFIED Requirements

### Requirement: Release and installation targets include current dashboard assets

The release-oriented Make targets SHALL build the dashboard frontend before invoking Rust build or install commands so the produced `cflx` binary embeds current dashboard assets even when Cargo build script caching would otherwise skip dashboard rebuilding.

#### Scenario: make build refreshes dashboard assets before release build
- **GIVEN** the repository contains the `dashboard` frontend and the `build` Make target is invoked
- **WHEN** `make build` runs
- **THEN** it runs the dashboard build step before `cargo build --release`
- **AND** if the dashboard build step fails, the `build` target exits with failure

#### Scenario: make install refreshes dashboard assets before install
- **GIVEN** the repository contains the `dashboard` frontend and the `install` Make target is invoked
- **WHEN** `make install` runs
- **THEN** it runs the dashboard build step before `cargo install --path .`
- **AND** if the dashboard build step fails, the `install` target exits with failure

#### Scenario: cross-build targets refresh dashboard assets before Rust cross compilation
- **GIVEN** the repository contains the `dashboard` frontend and a Linux cross-build Make target is invoked
- **WHEN** `make build-linux-x86` or `make build-linux-arm` runs
- **THEN** it runs the dashboard build step before the corresponding `cargo zigbuild --release --target ...` command
- **AND** if the dashboard build step fails, the cross-build target exits with failure
