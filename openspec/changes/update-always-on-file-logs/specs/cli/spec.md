## ADDED Requirements
### Requirement: TUIログファイルの常時出力

TUI のログファイル出力は常時有効でなければならず（MUST）、`tui --logs` オプションは提供してはならない（MUST NOT）。

#### Scenario: `tui --logs` は無効
- **WHEN** ユーザーが `cflx tui --logs /tmp/debug.log` を実行する
- **THEN** CLI は不明なオプションとしてエラーを表示する
- **AND** 終了コードは非0である
