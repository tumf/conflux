## MODIFIED Requirements
### Requirement: 設定ファイルの優先順位

オーケストレーターは以下の優先順位で設定ファイルを読み込まなければならない (MUST):

1. **プロジェクト設定** (優先): `.cflx.jsonc` (プロジェクトルート)
2. **XDG グローバル設定 (環境変数)**: `$XDG_CONFIG_HOME/cflx/config.jsonc`
3. **XDG グローバル設定 (デフォルト)**: `~/.config/cflx/config.jsonc`
4. **プラットフォーム標準のグローバル設定**: `dirs::config_dir()/cflx/config.jsonc`

プロジェクト設定が存在する場合はそちらを使用し、存在しない場合のみグローバル設定を使用する。

#### Scenario: XDG_CONFIG_HOME が設定されている場合は最優先で使用する
- **GIVEN** 環境変数 `XDG_CONFIG_HOME=/custom/config` が設定されている
- **AND** `/custom/config/cflx/config.jsonc` に:
  ```jsonc
  { "apply_command": "xdg-agent apply {change_id}" }
  ```
- **AND** `.cflx.jsonc` が存在しない
- **WHEN** 変更 `fix-bug` を適用する
- **THEN** `xdg-agent apply fix-bug` が実行される

#### Scenario: XDG_CONFIG_HOME が未設定の場合は ~/.config を優先する
- **GIVEN** `XDG_CONFIG_HOME` が未設定である
- **AND** `~/.config/cflx/config.jsonc` に:
  ```jsonc
  { "apply_command": "xdg-default apply {change_id}" }
  ```
- **AND** `.cflx.jsonc` が存在しない
- **WHEN** 変更 `fix-bug` を適用する
- **THEN** `xdg-default apply fix-bug` が実行される

#### Scenario: XDG 設定が存在しない場合は platform 標準のグローバル設定を使用する
- **GIVEN** `XDG_CONFIG_HOME` が未設定である
- **AND** `~/.config/cflx/config.jsonc` が存在しない
- **AND** platform 標準のグローバル設定に:
  ```jsonc
  { "apply_command": "platform-default apply {change_id}" }
  ```
- **AND** `.cflx.jsonc` が存在しない
- **WHEN** 変更 `fix-bug` を適用する
- **THEN** `platform-default apply fix-bug` が実行される
