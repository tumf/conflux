# Change: MergeWaitを再起動後も維持し、ResolveWaitを単一起動待ちに限定

## Why
TUIの再起動後に`merge wait`の変更が`resolve wait`へ誤って置換され、実際の待ち状態と表示が一致しないため、ユーザーの誤操作と状態誤認が発生します。

## What Changes
- 自動更新で`WorkspaceState::Archived`を検出した場合は`merge wait`を維持するように更新します
- `resolve wait`は手動のresolve開始直後から`ResolveStarted`までの単一起動待ちに限定します
- 既存の`ResolveWait`ブロック/保持ルールを維持しつつ、上書き優先順位を明確化します

## Impact
- Affected specs: `specs/tui-architecture/spec.md`
- Affected code: `src/tui/runner.rs`, `src/tui/state/events/helpers.rs`, `src/tui/state/events/refresh.rs`, `src/events.rs`, `src/tui/state/mod.rs`
