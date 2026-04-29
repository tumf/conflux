## MODIFIED Requirements

### Requirement: CLI acceptance verdict parsing preserves canonical gating terminology

acceptance ループは change に対して `acceptance_command` を実行し、出力テキストから pass/fail/continue/gated を判定して処理を分岐しなければならない（SHALL）。

移行期間中は、旧 integration 互換のため legacy `blocked` acceptance verdict input を受理してもよい（MAY）が、canonical output contract と operator-facing wording は `gated` を用いなければならない（MUST）。

#### Scenario: CLI treats gated as canonical acceptance blocker verdict
- **GIVEN** acceptance output が `ACCEPTANCE: GATED` または `{"acceptance":"gated"}` を示す
- **WHEN** CLI acceptance loop が verdict を解釈する
- **THEN** change は acceptance gate outcome として処理される
- **AND** dependency wait の `blocked` terminology とは混同されない

#### Scenario: CLI still accepts legacy blocked verdict during migration
- **GIVEN** acceptance output が旧 `ACCEPTANCE: BLOCKED` を示す
- **WHEN** compatibility-aware CLI runtime が verdict を解釈する
- **THEN** change は acceptance gate outcome として処理される
- **AND** canonical docs と新規 tests は `gated` を期待し続ける
