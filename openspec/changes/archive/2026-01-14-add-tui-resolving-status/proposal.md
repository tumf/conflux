# Change: TUI のマージ解決中ステータスを resolving にする

## Why
現在の TUI では、`MergeWait` の change に対して `M: resolve` を実行すると解決処理が同期的に走り、UI が固まります。その結果、解決中であることがステータスとして表示されず、ユーザーが進行状況を把握できません。

本変更では、解決処理の実行中に change のステータスを `resolving` として表示し、TUI の描画ループを止めないことで、ユーザーが「いま解決中である」ことを確実に認識できるようにします。

## What Changes
- `MergeWait` の change に対する `M: resolve` 実行中、UI 上の状態を `resolving` として表示する。
- resolve 実行は TUI のメインループをブロックしない形で行い、解決中でも UI の操作・描画を継続できるようにする。
- resolve が成功した場合は対象 change を完了状態（現行の挙動に合わせて `Archived` 相当）に遷移させる。
- resolve が失敗した場合は対象 change を `MergeWait` に戻し、失敗理由をログ/警告ポップアップで提示する。

## Impact
- Affected specs: `tui-key-hints`
- Affected code (implementation work items): `src/tui/types.rs`, `src/tui/runner.rs`, `src/tui/render.rs`, `src/events.rs`, `src/tui/state/events.rs`
- Risk: resolve 結果のイベント反映・状態遷移の競合（resolve 中に同 change が別経路で更新されない前提の確認が必要）
