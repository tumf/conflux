## 1. Specification Updates
- [x] 1.1 `MergeWait`/`ResolveWait`の意味づけと自動更新の扱いを`tui-architecture`の差分仕様に反映する（検証: `openspec/changes/update-tui-merge-wait-refresh/specs/tui-architecture/spec.md` の要件とシナリオが更新されている）
- [x] 1.2 `merge wait`で`F5`がresolve開始を優先する仕様を追加する（検証: `openspec/changes/update-tui-merge-wait-refresh/specs/tui-architecture/spec.md` にシナリオが追加されている）

## 2. Refresh Event Payload Alignment
- [x] 2.1 `ChangesRefreshed`の`resolve_wait_ids`を`merge_wait_ids`へ置換し、refreshパイプラインの参照を統一する（検証: `src/events.rs`, `src/tui/runner.rs`, `src/tui/state/events/refresh.rs`）
- [x] 2.2 自動更新で`WorkspaceState::Archived`を検出したchangeに`MergeWait`を付与するヘルパーを実装し、`ResolveWait`や進行中ステータスを上書きしない（検証: `src/tui/state/events/helpers.rs`）

## 3. ResolveWait Entry Timing
- [x] 3.1 `M`キーで手動resolveを開始した直後に`ResolveWait`へ遷移させる（検証: `src/tui/state/mod.rs` の`resolve_merge`でステータスが更新される）
- [x] 3.2 `ResolveStarted`/`ResolveFailed`の既存遷移（Resolving/MergeWait）を維持し、単一起動待ちの`ResolveWait`が上書きされることを確認する（検証: `src/tui/state/events/stages.rs`, `src/tui/state/events/completion.rs`）

## 4. F5 Resolve Prioritization
- [x] 4.1 `merge wait`のchangeで`F5`を押した場合、`StartProcessing`ではなく`ResolveMerge`を優先するように更新する（検証: `src/tui/key_handlers.rs` または `src/tui/state/modes.rs` の分岐が追加されている）
- [x] 4.2 `StartProcessing`の対象から`MergeWait`/`ResolveWait`を除外し、待機状態が`Queued`に上書きされないことを保証する（検証: `src/tui/state/modes.rs` のキュー化ロジックがrunnableのみを対象にしている）

## 5. Tests
- [x] 5.1 自動更新で`WorkspaceState::Archived`を検出した際に`MergeWait`が付与されることをテストで保証する（検証: `cargo test resolve_wait` など該当テスト名で実行）
- [x] 5.2 手動resolve開始直後に`ResolveWait`が付与されることをテストで保証する（検証: `cargo test resolve_merge` など該当テスト名で実行）
- [x] 5.3 `merge wait`で`F5`を押した場合に`ResolveMerge`が優先されることをテストで保証する（検証: `cargo test f5` など該当テスト名で実行）

## Acceptance #1 Failure Follow-up
- [x] src/tui/state/mod.rs: toggle_approval allows MergeWait changes to trigger UnapproveAndDequeue, and src/tui/command_handlers.rs: UnapproveAndDequeue always calls dynamic_queue.remove/mark_removed, so pressing @ on MergeWait modifies DynamicQueue in violation of the MergeWait queue operation requirement.
