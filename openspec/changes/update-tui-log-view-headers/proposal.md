# Change: Logsビューのログヘッダにchange_idを復帰

## Why
Logsビューのログは複数のchangeが混在するため、change_idが表示されないと識別性が低下します。一方で変更一覧のログプレビューは左カラムにchange_idが既に表示されているため、ヘッダでの重複表示は冗長です。この役割差を明確にし、Logsビューではchange_idを復帰させます。

## What Changes
- Logsビューのログヘッダを`[{change_id}:{operation}:{iteration}]`/`[{change_id}:{operation}]`形式に戻す
- 変更一覧のログプレビューは従来どおり`[operation:iteration]`/`[operation]`の短縮形式を維持する

## Impact
- Affected specs: `specs/tui-architecture/spec.md`
- Affected code: `src/tui/render.rs`, `src/tui/state/logs.rs`
