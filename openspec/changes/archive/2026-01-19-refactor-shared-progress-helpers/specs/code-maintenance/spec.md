## ADDED Requirements
### Requirement: 進捗取得とAPIエラー応答の共通化
オーケストレーターは change の進捗取得と Web API の Not Found 応答生成を共通ヘルパーに集約し、既存挙動を維持するために SHALL 共通ヘルパーを使用しなければならない。

#### Scenario: 進捗取得のフォールバック順序を維持する
- **WHEN** TUI または Web が change の進捗を取得する
- **THEN** 共通ヘルパーが worktree → archive → base の順でフォールバックする

#### Scenario: Not Found 応答の形式を維持する
- **WHEN** Web API が change を見つけられない
- **THEN** 共通ヘルパーが既存と同等の StatusCode とエラーメッセージを返す
