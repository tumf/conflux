## 1. 実装
- [ ] 1.1 in-flight 管理を導入し、apply 実行タスクの開始/完了を追跡する。完了条件: in-flight 数から available_slots を算出できることを `src/parallel/mod.rs` で確認する。
- [ ] 1.2 ディスパッチ処理を非同期化し、re-analysis ループが apply 完了を待たずに進行できるようにする。完了条件: `execute_with_order_based_reanalysis` が dispatch を await せずループ継続することをコードで確認する。
- [ ] 1.3 queue 通知/タイマー/完了イベントをトリガに re-analysis を起動する。完了条件: `tokio::select!` 等で通知を待機し、apply 中でも re-analysis が走ることをログで確認する。
- [ ] 1.4 queued → apply 遷移が apply 中の追加でも成立する。完了条件: apply 中にキュー追加した変更が空きスロットで即ディスパッチされることを実行ログで確認する。

## 2. 検証
- [ ] 2.1 再現手順（apply 実行中に変更追加）で、analysis が走り queued が apply へ遷移することを確認する。完了条件: 実行ログに re-analysis と dispatch の順序が記録される。
- [ ] 2.2 `cargo test` を実行し、既存並列実行テストが通過することを確認する。
