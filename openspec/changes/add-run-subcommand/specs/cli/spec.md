## ADDED Requirements

### Requirement: サブコマンド構造

CLI はサブコマンド構造を持ち、将来的なコマンド拡張に対応できなければならない（SHALL）。

#### Scenario: サブコマンドなしで実行
- **WHEN** ユーザーが引数なしで `openspec-orchestrator` を実行する
- **THEN** 利用可能なサブコマンド一覧を含むヘルプメッセージを表示する

#### Scenario: 不明なサブコマンドで実行
- **WHEN** ユーザーが存在しないサブコマンドで実行する
- **THEN** エラーメッセージと利用可能なサブコマンド一覧を表示する

### Requirement: run サブコマンド

`run` サブコマンドは OpenSpec 変更ワークフローのオーケストレーションループを実行しなければならない（SHALL）。

#### Scenario: run サブコマンドの基本実行
- **WHEN** ユーザーが `openspec-orchestrator run` を実行する
- **THEN** オーケストレーションループが開始される

#### Scenario: 特定の変更を指定して実行
- **WHEN** ユーザーが `openspec-orchestrator run --change <id>` を実行する
- **THEN** 指定された変更のみを処理する

#### Scenario: opencode パスのカスタマイズ
- **WHEN** ユーザーが `openspec-orchestrator run --opencode-path <path>` を実行する
- **THEN** 指定されたパスの opencode バイナリを使用する

#### Scenario: openspec コマンドのカスタマイズ
- **WHEN** ユーザーが `openspec-orchestrator run --openspec-cmd <cmd>` を実行する
- **THEN** 指定されたコマンドで openspec を実行する
