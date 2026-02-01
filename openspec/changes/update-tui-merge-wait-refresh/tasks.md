## 1. Specification Updates
- [ ] 1.1 `MergeWait`/`ResolveWait`の意味づけと自動更新の扱いを`tui-architecture`の差分仕様に反映する（検証: `openspec/changes/update-tui-merge-wait-refresh/specs/tui-architecture/spec.md` の要件とシナリオが更新されている）

## 2. Refresh Event Payload Alignment
- [ ] 2.1 `ChangesRefreshed`の`resolve_wait_ids`を`merge_wait_ids`へ置換し、refreshパイプラインの参照を統一する（検証: `src/events.rs`, `src/tui/runner.rs`, `src/tui/state/events/refresh.rs`）
- [ ] 2.2 自動更新で`WorkspaceState::Archived`を検出したchangeに`MergeWait`を付与するヘルパーを実装し、`ResolveWait`や進行中ステータスを上書きしない（検証: `src/tui/state/events/helpers.rs`）

## 3. ResolveWait Entry Timing
- [ ] 3.1 `M`キーで手動resolveを開始した直後に`ResolveWait`へ遷移させる（検証: `src/tui/state/mod.rs` の`resolve_merge`でステータスが更新される）
- [ ] 3.2 `ResolveStarted`/`ResolveFailed`の既存遷移（Resolving/MergeWait）を維持し、単一起動待ちの`ResolveWait`が上書きされることを確認する（検証: `src/tui/state/events/stages.rs`, `src/tui/state/events/completion.rs`）

## 4. Tests
- [ ] 4.1 自動更新で`WorkspaceState::Archived`を検出した際に`MergeWait`が付与されることをテストで保証する（検証: `cargo test resolve_wait` など該当テスト名で実行）
- [ ] 4.2 手動resolve開始直後に`ResolveWait`が付与されることをテストで保証する（検証: `cargo test resolve_merge` など該当テスト名で実行）
