# CLI Capability

## MODIFIED Requirements

### Requirement: 反復スナップショット（phase 別 WIP コミット）

Orchestrator は、反復ループの状態を追跡するために phase 別の WIP コミット（`--allow-empty`）を作成しなければならない（SHALL）。

- apply: 既存の WIP フォーマットを維持する
- archive: apply と混ざらないように `WIP(archive):` のように prefix を分離する

#### Scenario: archive 反復で WIP(archive) を作成する
- **GIVEN** ある change の archive 処理が検証失敗により retry される
- **WHEN** archive の各 attempt が完了する
- **THEN** `WIP(archive): {change_id} (attempt#{n})` 形式のコミットが作成される

#### Scenario: archive 成功時に WIP(archive) を squash する
- **GIVEN** ある change で複数の `WIP(archive)` が存在する
- **WHEN** archive が成功する
- **THEN** `WIP(archive)` は squash され、最終的に `Archive: {change_id}` が残る
