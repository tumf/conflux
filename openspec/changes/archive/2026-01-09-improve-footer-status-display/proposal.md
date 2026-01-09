# Proposal: improve-footer-status-display

## Summary

フッターの "Press F5 to start processing" 表示を、アプリケーションの状態に応じて動的に切り替える。

## Problem

現状、フッターのメッセージは `warning_message` がない場合に常に "Press F5 to start processing" を表示している。これは以下の状況で不適切：

1. **changes がない場合** - 実行するものがないのに F5 を押すよう促す
2. **選択されていない場合** - 選択なしで F5 を押しても意味がない
3. **実行中の場合** - 処理中なのに「開始せよ」と表示される

## Solution

フッターメッセージを以下の状態に応じて動的に表示：

| 状態 | 表示内容 |
|------|----------|
| changes がない | "Add new proposals to get started" |
| changes はあるが未選択 | "Select changes with Space to process" |
| 選択済み & 非実行中 | "Press F5 to start processing" |
| 実行中 | 進捗バー（全タスク数ベース） |

## Scope

- **Modified**: `src/tui.rs` - `render_footer_select` 関数の条件分岐追加
- **Modified**: CLI spec - フッター表示要件の更新

## Dependencies

なし

## Risks

- Low: UI のみの変更、既存ロジックへの影響なし
