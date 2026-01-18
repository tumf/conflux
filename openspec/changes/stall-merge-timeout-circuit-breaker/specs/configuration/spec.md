## ADDED Requirements
### Requirement: merge停滞監視の設定

オーケストレーターは merge 停滞監視のために `merge_stall_detection` 設定を提供し、閾値と監視間隔を構成できなければならない（SHALL）。

#### Scenario: 監視設定を指定する
- **GIVEN** `.cflx.jsonc` に以下の設定がある:
  ```jsonc
  {
    "merge_stall_detection": {
      "enabled": true,
      "threshold_minutes": 30,
      "check_interval_seconds": 60
    }
  }
  ```
- **WHEN** オーケストレーターを実行する
- **THEN** merge 停滞監視が有効になる
- **AND** 閾値は 30 分として扱われる
- **AND** 監視間隔は 60 秒として扱われる

#### Scenario: 監視設定が未指定の場合はデフォルトを使用する
- **GIVEN** `merge_stall_detection` が未設定である
- **WHEN** オーケストレーターを実行する
- **THEN** デフォルト値で merge 停滞監視が評価される
