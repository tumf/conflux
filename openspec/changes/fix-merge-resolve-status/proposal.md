# Change: Fix merge resolve status to show merged

## Why
マージ待機中の change を `M` で resolve した後に TUI が `archived` を表示しており、実際に base へマージ済みであることが伝わらないため、ユーザーが状態を誤認する。

## What Changes
- `MergeWait` の change を resolve 完了した際、TUI の最終状態を `merged` として扱う
- resolve 完了時のイベントフローを明確化し、`Merged` 表示と整合させる

## Impact
- Affected specs: parallel-execution
- Affected code: TUI event handling, resolve merge command flow
