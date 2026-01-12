# hooks spec delta

## MODIFIED Requirements

### Requirement: Hook Execution Modes

システムは、serial mode と parallel mode の両方で hooks を実行しなければならない（SHALL）。

#### Scenario: Serial mode での hook 実行

- **GIVEN** serial mode で orchestrator が実行されている
- **WHEN** apply コマンドが実行される前
- **THEN** `pre_apply` hook が実行される

#### Scenario: Parallel mode での hook 実行

- **GIVEN** parallel mode で orchestrator が実行されている
- **WHEN** apply コマンドが実行される前
- **THEN** `pre_apply` hook が実行される
- **AND** hook は対応する workspace で実行される

#### Scenario: Parallel mode での複数 change の hook 実行

- **GIVEN** parallel mode で 3 つの change が同時に処理されている
- **WHEN** 各 change で apply が実行される
- **THEN** 各 change に対して独立して `pre_apply` hook が実行される
- **AND** hook 実行は並行して行われる可能性がある

## ADDED Requirements

### Requirement: Parallel Mode Hook Context

parallel mode での hook 実行時、`HookContext` には workspace 固有の情報が含まれなければならない（SHALL）。

#### Scenario: Workspace path の提供

- **GIVEN** parallel mode で hook が実行される
- **WHEN** `HookContext` が構築される
- **THEN** 環境変数 `OPENSPEC_WORKSPACE_PATH` に workspace のパスが設定される

#### Scenario: Group 情報の提供

- **GIVEN** parallel mode で複数の change がグループとして処理されている
- **WHEN** hook が実行される
- **THEN** 環境変数 `OPENSPEC_GROUP_INDEX` に現在のグループインデックスが設定される
