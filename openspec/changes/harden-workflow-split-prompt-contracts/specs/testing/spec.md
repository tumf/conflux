## MODIFIED Requirements

### Requirement: 仕様ベーステスト

全ての仕様シナリオに対応するテストが存在しなければならない（SHALL）。workflow split によって fixed guidance の authoritative source が command template / dedicated skill / Rust runtime enforcement に分割される場合、少なくとも output contract と ownership boundary を検証する drift-detection tests が存在しなければならない（SHALL）。

#### Scenario: Workflow split prompt contracts are regression-tested

- **WHEN** operation-specific prompt surfaces are split across command templates, dedicated skills, and Rust prompt builders
- **THEN** the repository contains targeted tests that detect authoritative-source drift for fixed guidance and machine-readable output contracts
- **AND** acceptance-related tests cover production-style formatting drift cases that would otherwise trigger an unintended `CONTINUE` fallback
- **AND** the tests fail when documented prompt or parser contracts are changed in only one surface without corresponding updates to the others
