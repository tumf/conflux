## ADDED Requirements

### Requirement: TUI rendering の責務分割安全性

TUI rendering の内部リファクタリングは、主要画面と popup の表示 contract を維持しながら、描画領域ごとに変更影響範囲を分離しなければならない。

#### Scenario: TUI 表示 contract が維持される

- **GIVEN** select mode、running mode、worktree view、warning popup、QR popup の代表的な `AppState` がある
- **WHEN** `render(frame, app)` を実行する
- **THEN** changes list、status、logs、footer、popup の代表テキストと選択状態はリファクタ前と同等である
- **AND** public entry point と操作フローは変更されない
