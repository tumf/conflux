# Change: Resolve待ちステータスの追加

## Why
並列実行でresolveがシリアライズされている間、archive済みchangeが`not queued`に戻ってしまい、待機中であることが見えません。結果としてSpaceキーでキュー状態を変更できてしまい、誤操作や状態の不整合が発生します。

## What Changes
- resolveが実行中で待機しているchangeを`ResolveWait`として表示し、`not queued`への自動リセットを防止する
- `ResolveWait`はキュー操作の対象外とし、Space/@操作で状態を変更できないようにする
- TUIのステータス表示と共有状態更新を`ResolveWait`に対応させる

## Impact
- Affected specs: `openspec/specs/tui-architecture/spec.md`
- Affected code: `src/tui/types.rs`, `src/tui/state/events/helpers.rs`, `src/tui/state/mod.rs`, `src/tui/render.rs`, `src/parallel/mod.rs` (resolve待ちイベント)
