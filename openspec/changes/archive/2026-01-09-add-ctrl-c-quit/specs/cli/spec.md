# cli Spec Delta

## MODIFIED Requirements

### Requirement: 変更選択モード

TUI起動時、変更選択モードを表示し、ユーザーが処理する変更を選択できなければならない（SHALL）。

#### Scenario: 終了
- **WHEN** ユーザーが `q` キーまたは `Ctrl+C` を押す
- **THEN** TUIが終了し、ターミナルが元の状態に復元される
