## MODIFIED Requirements
### Requirement: Common Archive Command Execution

`archive_change()` および `archive_change_streaming()` は、archive コマンドの実行結果を記録し、再試行時には前回の履歴をプロンプトに含めなければならない（MUST）。

archive ループの実装は、フック実行・コマンド実行・検証・履歴記録をヘルパー関数に分割してもよい（MAY）。ただし、履歴の記録と再試行時の伝播は必ず維持しなければならない（MUST）。

#### Scenario: Archive 実行後の履歴記録

- **GIVEN** システムが change の archive を実行する
- **WHEN** archive コマンドが完了する（成功または失敗）
- **THEN** システムは試行結果を記録する
- **AND** 記録には試行回数、成功/失敗ステータス、所要時間、検証結果が含まれる

#### Scenario: Archive 再試行時の履歴伝播

- **GIVEN** 1回目の archive が検証失敗した
- **WHEN** システムが同じ change の archive を再試行する
- **THEN** `AgentRunner::run_archive_streaming()` に渡されるプロンプトに前回の履歴が含まれる
- **AND** 履歴には検証失敗の理由（"Change still exists at...") が含まれる

#### Scenario: Archive 成功時の履歴クリア

- **GIVEN** change の archive が成功した
- **WHEN** change が完全に処理される
- **THEN** その change の archive 履歴はクリアされる
