# Change: 停止モードで承認時にキューステータスが変わる不具合を修正

## Why

TUIで処理を停止した後（`AppMode::Stopped`）、`@` キーで change を承認すると、queue_status が `Queued` になってしまう。停止中は処理が行われないため、`NotQueued` のままであるべき。

現状の動作:
- `[ ]` → `@` → `[x]` (approved + queued) ← 問題

期待される動作:
- `[ ]` → `@` → `[@]` (approved only, NOT queued)

## What Changes

- `toggle_approval()` 関数で `AppMode::Stopped` を `AppMode::Running` と同様に扱うよう修正
- 停止中は承認のみ行い、キューには追加しない

## Impact

- Affected specs: なし（内部動作の修正のため spec 変更は不要）
- Affected code: `src/tui/state/mod.rs` の `toggle_approval()` 関数
