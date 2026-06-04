# Design: TUI rendering の描画責務分割

## 現状

`src/tui/render.rs` は TUI 全体の描画とテストを単一ファイルに集約している。UI 領域ごとの責務境界は関数名では分かるが、ファイル構造としては分離されていないため、変更時の影響範囲を狭めにくい。

## 方針

- `render(frame, app)` は public entry point として維持する。
- 表示内容、layout、key hints は変更しない。
- module 分割前に characterization test を確認し、分割後も同じ buffer assertion を通す。
- `AppState` 構造体や event handling は触らない。

## 分割候補

- main mode rendering。
- changes list rendering。
- status/log rendering。
- worktree rendering。
- popup rendering。
- render test helpers。

## Trade-offs

ファイル数は増えるが、描画領域ごとの所有範囲が明確になり、将来の UI 変更で不要な衝突を減らせる。今回は挙動不変を優先し、見た目の改善や layout 再設計は行わない。
