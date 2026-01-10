# cli Specification Delta

## MODIFIED Requirements

### Requirement: run サブコマンド

`run` サブコマンドは OpenSpec 変更ワークフローのオーケストレーションループを実行しなければならない（SHALL）。

#### Scenario: 特定の変更を指定して実行
- **WHEN** ユーザーが `openspec-orchestrator run --change <id>` を実行する
- **THEN** 指定された変更のみを処理する
- **AND** スナップショットログには指定された変更のみが表示される

#### Scenario: 複数の変更をカンマ区切りで指定
- **WHEN** ユーザーが `openspec-orchestrator run --change a,b,c` を実行する
- **THEN** `a`, `b`, `c` の変更のみを処理する
- **AND** スナップショットログには `a`, `b`, `c` のみが表示される

#### Scenario: 存在しない変更を指定した場合
- **WHEN** ユーザーが `openspec-orchestrator run --change nonexistent` を実行する
- **AND** `nonexistent` という変更が存在しない
- **THEN** 警告メッセージ "Specified change 'nonexistent' not found, skipping" が出力される
- **AND** 「No changes found」と表示されて終了する

#### Scenario: 有効な変更と無効な変更を混在して指定
- **WHEN** ユーザーが `openspec-orchestrator run --change a,nonexistent,c` を実行する
- **AND** `a` と `c` は存在するが `nonexistent` は存在しない
- **THEN** 警告メッセージ "Specified change 'nonexistent' not found, skipping" が出力される
- **AND** `a` と `c` のみを処理する
- **AND** スナップショットログには `a` と `c` のみが表示される
