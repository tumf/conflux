## MODIFIED Requirements
### Requirement: Common Apply Iteration Logic

システムは、apply コマンドの反復実行を管理するための共通ロジックを提供しなければならない（SHALL）。このロジックは serial mode と parallel mode の両方で使用される。

SerialRunService の apply 反復処理は、進捗再取得・acceptance 判定・履歴更新を個別のヘルパーに分割してもよい（MAY）。ただし実行順序と結果は既存と同一でなければならない（MUST）。

#### Scenario: 単一 apply の実行

- **GIVEN** change_id = "my-change" と apply コマンドが設定されている
- **WHEN** `execute_apply_iteration()` を呼び出す
- **THEN** apply コマンドが実行される
- **AND** 実行後の進捗情報が返される

#### Scenario: 反復 apply の実行

- **GIVEN** max_iterations = 50 が設定されている
- **WHEN** タスクが 100% 完了するまで反復する
- **THEN** 各反復で進捗をチェックする
- **AND** 完了したら反復を終了する

#### Scenario: 最大反復回数の制限

- **GIVEN** max_iterations = 50 が設定されている
- **WHEN** 50 回の反復後もタスクが完了しない
- **THEN** エラーが返される
