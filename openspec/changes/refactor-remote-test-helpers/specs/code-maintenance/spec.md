## ADDED Requirements
### Requirement: Remote Test Support Helpers
リモートモジュールのテストは、WS/HTTP モックサーバー生成と JSON フィクスチャ生成を共通ヘルパー経由で行わなければならない (MUST)。

#### Scenario: 共通ヘルパーの利用
- **WHEN** リモートテストがモックサーバーを必要とする
- **THEN** 共通ヘルパーが WS/HTTP のモックサーバーを生成する
- **AND** テストは同じ待機/検証条件で実行できる
