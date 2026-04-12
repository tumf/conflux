## ADDED Requirements

### Requirement: TUI change editor launch resolves archived changes

TUI の Changes view で `e` キーによる change editor launch を実行する際、システムは active change に加えて `openspec/changes/archive/` 配下の archived change entry も解決しなければならない（MUST）。

archive entry の解決では direct match (`openspec/changes/archive/<change_id>`) と date-prefixed match (`openspec/changes/archive/<date>-<change_id>`) の両方を同一 change として扱わなければならない（MUST）。

解決済み entry に `proposal.md` が存在する場合はそのファイルを editor で開き、存在しない場合は change directory 自体を editor のカレントディレクトリとして開かなければならない（MUST）。

#### Scenario: e key opens archived change proposal from dated archive entry

- **GIVEN** TUI の Changes view で selected change id が `fix-archived-editor` である
- **AND** active path `openspec/changes/fix-archived-editor` は存在しない
- **AND** archive path `openspec/changes/archive/2026-04-11-fix-archived-editor/proposal.md` が存在する
- **WHEN** ユーザーが `e` キーを押す
- **THEN** system は archive path を selected change の実体として解決する
- **AND** `proposal.md` を editor launch 対象として使用する
- **AND** `ChangeNotFound` を返さない

#### Scenario: e key still prefers active change path

- **GIVEN** TUI の Changes view で selected change id が `active-change` である
- **AND** `openspec/changes/active-change/proposal.md` が存在する
- **AND** `openspec/changes/archive/2026-04-11-active-change/` も存在する
- **WHEN** ユーザーが `e` キーを押す
- **THEN** system は active path を優先して解決する
- **AND** active change の `proposal.md` を editor launch 対象として使用する
