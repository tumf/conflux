# Change: TUIの強制停止後にaccepting表示が残る不具合の修正

## Why
TUIでEsc二度押しによる強制停止を行っても、accepting状態のまま表示が残るため、停止後のキュー状態がユーザーに誤って伝わる。

## What Changes
- 強制停止時にaccepting状態のchangeをNotQueuedへリセットする
- 停止時のキュー状態リセット範囲を「実行中ステータス」として統一する

## Impact
- Affected specs: `specs/cli/spec.md`
- Affected code: `src/tui/state/events/processing.rs`
