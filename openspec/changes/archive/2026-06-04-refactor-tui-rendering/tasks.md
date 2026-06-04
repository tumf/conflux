## Implementation Tasks

- [x] 1. TUI 主要画面の現在表示を characterization test で固定する。
  verification: unit - `cargo test tui::render`。
  completion: select/running changes list、status/logs、worktree、popup の代表 assertion が存在し成功する。
- [x] 2. `render(frame, app)` entry point を維持したまま、changes list、status/logs、worktree、popup の描画を責務別 module/function へ分割する。
  verification: unit - `cargo test tui::render`。
  completion: public entry point と既存呼び出し元を変更せず、描画責務が分離されている。
- [x] 3. rendering tests の fixture/helper を整理し、描画領域ごとの意図が分かる構造にする。
  verification: unit - `cargo test tui::render`。
  completion: test setup が共通 helper に寄り、各 test が期待表示の assertion を中心に読める。
- [x] 4. TUI 関連テスト全体の後退がないことを確認する。
  verification: integration - `cargo test tui::`。
  completion: TUI state/handler/render の代表テストが成功する。

## Future Work

- visual redesign、layout 変更、accessibility 改善は別 change で扱う。

## Final Validation

Expected archive gate: `cflx openspec validate refactor-tui-rendering --archive-gate`
