## ADDED Requirements

### Requirement: Environment Variable Configuration for OpenSpec Command

ユーザーは環境変数 `OPENSPEC_CMD` を通じて openspec コマンドを設定できなければならない (MUST)。

設定値の優先順位は以下の通りとする:
1. CLI 引数 `--openspec-cmd` (最優先)
2. 環境変数 `OPENSPEC_CMD`
3. デフォルト値 `npx @fission-ai/openspec@latest`

#### Scenario: 環境変数のみ設定

- **WHEN** 環境変数 `OPENSPEC_CMD` に `/usr/local/bin/openspec` が設定されている
- **AND** CLI 引数 `--openspec-cmd` が指定されていない
- **THEN** `/usr/local/bin/openspec` が openspec コマンドとして使用される

#### Scenario: CLI 引数が環境変数より優先

- **WHEN** 環境変数 `OPENSPEC_CMD` に `/usr/local/bin/openspec` が設定されている
- **AND** CLI 引数 `--openspec-cmd ./my-openspec` が指定されている
- **THEN** `./my-openspec` が openspec コマンドとして使用される

#### Scenario: どちらも未設定時はデフォルト値を使用

- **WHEN** 環境変数 `OPENSPEC_CMD` が設定されていない
- **AND** CLI 引数 `--openspec-cmd` が指定されていない
- **THEN** `npx @fission-ai/openspec@latest` が openspec コマンドとして使用される
