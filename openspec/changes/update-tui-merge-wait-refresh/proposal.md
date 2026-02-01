# Change: MergeWaitをリポジトリ状態から復元し、F5でresolve開始を優先

## Why
TUIの再起動後に`merge wait`の変更が`resolve wait`へ誤って置換され、実際の待ち状態と表示が一致しないため、ユーザーの誤操作と状態誤認が発生します。さらに、待機中の変更で`F5`を押すとacceptanceに戻ってしまい、期待するresolve開始と一致しません。

## What Changes
- 自動更新で`WorkspaceState::Archived`を検出した場合は、リポジトリ状態から`merge wait`を冪等に復元します
- `resolve wait`は手動resolve開始直後から`ResolveStarted`までの単一起動待ちに限定し、再起動時の復元には使いません
- `F5`は`merge wait`の変更に対してresolve開始を優先し、acceptanceに戻さないようにします
- 既存の`ResolveWait`ブロック/保持ルールを維持しつつ、上書き優先順位を明確化します

## Impact
- Affected specs: `specs/tui-architecture/spec.md`
- Affected code: `src/tui/runner.rs`, `src/tui/state/events/helpers.rs`, `src/tui/state/events/refresh.rs`, `src/events.rs`, `src/tui/state/mod.rs`, `src/tui/key_handlers.rs`, `src/tui/state/modes.rs`
