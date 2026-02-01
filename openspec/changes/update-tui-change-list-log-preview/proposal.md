# Change: 変更一覧のログプレビューとログヘッダ短縮

## Why
変更一覧の右側余白が活用されておらず、直近の状況確認に手間がかかっています。ログヘッダに change 名が繰り返し表示されて冗長なため、視認性を改善します。

## What Changes
- 変更一覧の各 change 行に、最新ログの単一行プレビュー（時刻付き）を表示する
- ログヘッダから change 名を省略し、`[operation:iteration]` 形式に短縮する

## Impact
- Affected specs: `specs/tui-architecture/spec.md`, `specs/cli/spec.md`
- Affected code: `src/tui/render.rs`, `src/tui/state/logs.rs`
