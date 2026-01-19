## 1. 実装
- [ ] 1.1 スケジューラ状態を追加する（JoinSet<WorkspaceResult>, Semaphore, in-flight HashSet, queued Vec<Change>, needs_reanalysis フラグ）。完了条件: `src/parallel/mod.rs` でこれらの状態が保持され、available_slots 算出が in-flight を参照している。
- [ ] 1.2 dispatch の spawn ヘルパーを作成し、workspace 作成/再利用と apply+acceptance+archive の spawn をここに集約する。完了条件: re-analysis ループからこのヘルパーのみが dispatch を行うことを `src/parallel/mod.rs` で確認する。
- [ ] 1.3 `execute_with_order_based_reanalysis` を `tokio::select!` ベースのループに置き換える。完了条件: queue 通知 / debounce タイマー / join_set 完了 / cancel を待機し、dispatch を await しない。
- [ ] 1.4 dynamic queue の取り込みを re-analysis ループ先頭に集約し、analysis → order → dispatch の順序を保証する。完了条件: queue 追加が queued に反映された後に analyzer が呼ばれることを `src/parallel/mod.rs` で確認する。
- [ ] 1.5 in-flight 追跡を更新する（spawn 時に追加、join 完了で削除）。完了条件: available_slots が in-flight 数の変化で増減する。
- [ ] 1.6 join_set 完了時の処理を整理する（workspace status 更新、merge/cleanup、failed tracker、needs_reanalysis 設定）。完了条件: join 完了で次の re-analysis が起動できることを `src/parallel/mod.rs` で確認する。
- [ ] 1.7 re-analysis トリガ種別と slots/in-flight をログ出力する。完了条件: `queue/timer/completion` のいずれかがログに残り、slots/in-flight 数が表示される。

## 2. 検証
- [ ] 2.1 `tokio::time::pause` を使った単体テストで「apply 実行中の queue 追加 → re-analysis → slots 空きで dispatch」を確認する。完了条件: `src/parallel/mod.rs` の新規テストが `cargo test` で成功する。
- [ ] 2.2 既存の dynamic queue / デバウンス関連テストが引き続き通過することを確認する。完了条件: `cargo test` で既存テストが失敗しない。
- [ ] 2.3 apply 中の queue 追加で re-analysis が動くことをログで確認する。完了条件: `queue` → `analysis` → `dispatch` の順でログが出る（`RUST_LOG=debug cargo run -- run --parallel --dry-run` の実行ログなど）。
- [ ] 2.4 キャンセル時に dispatch がブロックせず終了することを確認する。完了条件: cancel 直後に re-analysis ループが停止し、`Stopped` イベントが出ることをログで確認する。
