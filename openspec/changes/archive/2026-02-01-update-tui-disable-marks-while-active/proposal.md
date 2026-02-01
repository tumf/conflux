# Change: 実行中(is_active)は@/x操作を無効化する

## Why
実行中のchangeで@/x操作が一部許可されており、操作可否のルールが分散しています。`is_active`に統一して入力拒否を明確化し、誤操作を防ぎます。

## What Changes
- `queue_status.is_active()` が true の change では、Space(@/x) と @ の操作を受け付けない
- 操作拒否時は警告メッセージを表示する（既存の警告メカニズムを利用）
- 既存の待機状態（MergeWait/ResolveWait）の挙動は維持する

## Impact
- Affected specs: `tui-architecture`
- Affected code: `src/tui/state/mod.rs`, `src/tui/state/guards.rs`
