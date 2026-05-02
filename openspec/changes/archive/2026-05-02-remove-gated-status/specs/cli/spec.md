## MODIFIED Requirements

### Requirement: Orchestration loop runs apply and archive

acceptance ループは change に対して `acceptance_command` を実行し、出力テキストから pass/fail/continue と互換入力としての gated/blocked blocker verdict を判定して処理を分岐しなければならない（SHALL）。

移行期間中は、旧 integration 互換のため legacy `blocked` acceptance verdict input と既存 `gated` acceptance verdict input を受理してもよい（MAY）が、operator-facing lifecycle/status wording は `stalled` を用いなければならない（MUST）。

#### Scenario: CLI treats gated input as stalled acceptance blocker
- **GIVEN** acceptance output が `ACCEPTANCE: GATED` または `{"acceptance":"gated"}` を示す
- **WHEN** CLI acceptance loop が verdict を解釈する
- **THEN** change は acceptance blocker outcome として処理される
- **AND** paused lifecycle/status wording は `stalled` になる
- **AND** dependency wait の `blocked` terminology とは混同されない

#### Scenario: CLI still accepts legacy blocked verdict during migration
- **GIVEN** acceptance output が旧 `ACCEPTANCE: BLOCKED` を示す
- **WHEN** compatibility-aware CLI runtime が verdict を解釈する
- **THEN** change は acceptance blocker outcome として処理される
- **AND** canonical docs と新規 lifecycle/status tests は `stalled` を期待し、`gated` を表示状態として期待しない
