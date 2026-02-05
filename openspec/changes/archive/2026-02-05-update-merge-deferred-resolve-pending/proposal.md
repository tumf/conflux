# Change: resolve 中の MergeDeferred を resolve pending として扱う

## Why
resolve 実行中に別 change の archive が完了して `MergeDeferred` が発生すると、TUI 表示が `merge wait` になり、実際の動作（先行 resolve 完了後に自動で resolve に移る）と一致しない。
待ち状態の意味を UI 上で明確に区別し、手動マージ待ち（merge wait）と自動解決待ち（resolve pending）を正しく表示できるようにする。

## What Changes
- resolve 実行中の `MergeDeferred` 受信時、TUI は `ResolveWait`（表示: resolve pending）へ遷移し、resolve 待ち行列に追加する
- resolve 実行中ではない場合、従来どおり `MergeWait` を保持する
- Web ダッシュボードの語彙に `resolve pending` を追加し、TUI と同じ待ち状態を表示できるようにする
- `MergeDeferred` のイベント報告は維持しつつ、待ち状態の表示は TUI 仕様に従うことを明文化する

## Impact
- Affected specs: `openspec/specs/tui-architecture/spec.md`, `openspec/specs/parallel-execution/spec.md`, `openspec/specs/web-monitoring/spec.md`
- Affected code: `src/tui/state.rs`, `src/tui/render.rs`, `src/web/state.rs`
