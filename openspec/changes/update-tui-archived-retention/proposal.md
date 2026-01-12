# Change: Archived changes remain visible in TUI until exit

## Why
TUI で archived 状態になった change が即座に Changes 一覧から消えるため、完了状況の確認がしづらい。アプリ終了まで一覧に残すことで、実行結果を視覚的に確認できるようにする。

## What Changes
- TUI 上で archived 状態になった change をアプリ終了まで Changes 一覧に残す
- archived change の表示状態は既存の archived 表示ルールを維持する

## Impact
- Affected specs: `cli`
- Affected code: `tui` 状態管理, `tui` レンダリング, archived 反映ロジック
