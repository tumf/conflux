## MODIFIED Requirements
### Requirement: CLIフラグ一覧

CLIは以下のフラグを提供しなければならない（SHALL）。

- 共通: `--version`, `-V`, `--help`, `-h`
- `run` サブコマンド: `--change`, `--config`, `--max-iterations`, `--parallel`, `--max-concurrent`, `--dry-run`, `--vcs`, `--no-resume`, `--web`, `--web-port`, `--web-bind`
- `tui` サブコマンド: `--config`, `--logs`, `--web`, `--web-port`, `--web-bind`
- `init` サブコマンド: `--template`, `--force`
- `approve` サブコマンド: `set`, `unset`, `status`

`--opencode-path` と `--openspec-cmd` はCLIに存在してはならない（MUST NOT）。

#### Scenario: CLIフラグ一覧にOpenSpec/Opencode関連フラグが含まれない
- **WHEN** ユーザーが `cflx --help` を確認する
- **THEN** `--opencode-path` と `--openspec-cmd` は表示されない
- **AND** `run` と `tui` の主要フラグが一覧に含まれる

### Requirement: 拡張ヘルプ出力

CLIは `cflx --help` に全サブコマンドと主要オプション一覧を明示的に表示しなければならない（SHALL）。

#### Scenario: web監視フラグがヘルプに表示される
- **WHEN** ユーザーが `cflx --help` を実行する
- **THEN** `--web` / `--web-port` / `--web-bind` が `run` と `tui` のオプション一覧に含まれる
