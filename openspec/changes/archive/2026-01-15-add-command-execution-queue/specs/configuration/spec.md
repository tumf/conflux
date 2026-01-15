# configuration Specification (Delta)

## ADDED Requirements

### Requirement: Command Queue Configuration

オーケストレーターは JSONC 設定ファイルを通じてコマンド実行キューの動作を設定できなければならない (MUST)。

設定可能な項目は以下の通りとする：

1. `command_queue_stagger_delay_ms` - コマンド実行間の遅延時間（ミリ秒）、デフォルト: 2000
2. `command_queue_max_retries` - 自動リトライの最大回数、デフォルト: 2
3. `command_queue_retry_delay_ms` - リトライ間の待機時間（ミリ秒）、デフォルト: 5000
4. `command_queue_retry_patterns` - リトライ対象のエラーパターン（正規表現のリスト）
5. `command_queue_retry_if_duration_under_secs` - この秒数未満の実行時間で失敗した場合、リトライ対象とする、デフォルト: 5

デフォルトのリトライパターンは以下を含む：
- `Cannot find module` - モジュール解決エラー
- `ResolveMessage:` - モジュール解決メッセージ
- `EBADF.*lock` - ファイルロックエラー
- `Lock acquisition failed` - ロック取得失敗
- `ENOTFOUND registry\.npmjs\.org` - NPM レジストリ接続エラー
- `ETIMEDOUT.*registry` - レジストリタイムアウト

#### Scenario: デフォルト設定でキューが動作

- **WHEN** 設定ファイルにキュー設定が存在しない
- **THEN** デフォルト値（遅延2秒、最大2回リトライ、リトライ待機5秒）が使用される
- **AND** デフォルトのエラーパターンが適用される

#### Scenario: カスタム遅延時間の設定

- **GIVEN** `.openspec-orchestrator.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_queue_stagger_delay_ms": 5000
  }
  ```
- **WHEN** コマンドが連続実行される
- **THEN** 各コマンド実行間に5秒の遅延が適用される

#### Scenario: カスタムリトライ設定

- **GIVEN** `.openspec-orchestrator.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_queue_max_retries": 5,
    "command_queue_retry_delay_ms": 10000,
    "command_queue_retry_patterns": [
      "ECONNREFUSED",
      "timeout"
    ]
  }
  ```
- **WHEN** コマンド実行が `ECONNREFUSED` エラーで失敗
- **THEN** 最大5回まで自動リトライされる
- **AND** 各リトライ間に10秒の待機が発生する

#### Scenario: 空のリトライパターンリスト

- **GIVEN** `.openspec-orchestrator.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_queue_retry_patterns": []
  }
  ```
- **WHEN** コマンド実行が任意のエラーで失敗
- **THEN** 自動リトライは実行されない（リトライパターンにマッチしないため）

#### Scenario: 遅延時間ゼロの設定

- **GIVEN** `.openspec-orchestrator.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_queue_stagger_delay_ms": 0
  }
  ```
- **WHEN** コマンドが連続実行される
- **THEN** 遅延なしで即座に実行される（時間差起動が無効化）

#### Scenario: 実行時間による自動リトライ

- **GIVEN** `.openspec-orchestrator.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_queue_retry_if_duration_under_secs": 5
  }
  ```
- **WHEN** コマンド実行が2秒で失敗
- **AND** エラーメッセージがリトライパターンにマッチしない
- **THEN** 実行時間が5秒未満のため、自動リトライされる

#### Scenario: 長時間実行後のエラーはリトライしない

- **GIVEN** `.openspec-orchestrator.jsonc` にデフォルト設定が使用される
- **WHEN** コマンド実行が30秒で失敗
- **AND** エラーメッセージがリトライパターンにマッチしない
- **THEN** 実行時間が5秒を超えているため、リトライされない
