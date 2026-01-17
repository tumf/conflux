## MODIFIED Requirements

### Requirement: 並列 apply/archive での stagger 適用

システムは並列実行モードの apply/archive に対しても CommandQueue の stagger/retry を適用しなければならない（SHALL）。

#### Scenario: 並列 apply の stagger が適用される

- **GIVEN** 並列実行モードで複数の change が処理されている
- **AND** 遅延時間が2秒に設定されている
- **WHEN** worktree A で apply コマンドが実行される
- **AND** 0.5秒後に worktree B で apply コマンドが実行されようとする
- **THEN** worktree B の apply は1.5秒待機してから実行される
- **AND** 両方の apply が共通の `last_execution` 状態を参照している

#### Scenario: parallel の apply/archive が CommandQueue 経由で実行される

- **GIVEN** parallel 実行モードで apply/archive が実行される
- **WHEN** apply/archive コマンドが起動される
- **THEN** CommandQueue の stagger と retry が適用される
- **AND** streaming 出力のリトライ通知が既存の出力経路に送信される
