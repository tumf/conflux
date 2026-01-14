## 1. 仕様・状態
- [x] `MergeWait` 状態（TUI の `QueueStatus`）を追加し、表示上も識別できるようにする
- [x] `ExecutionEvent::MergeDeferred { change_id, reason }` を追加し、TUI が `MergeWait` へ遷移できるようにする
- [x] `tui-architecture` の公開 API 安定性要件を更新し、イベント/コマンドの追加が許容される範囲を明確化する

## 2. parallel 実行（マージ延期）
- [x] archive 完了後の個別マージ直前に base dirty 判定（未コミット/未追跡/マージ進行中）を行う
- [x] dirty の場合はマージを実行せず `MergeDeferred` を発行し、worktree を保持する（cleanup しない）
- [x] `MergeWait` change に依存する queued change は今回の run では実行しないが、キューからは外さない
- [x] `MergeWait` に依存しない queued change は実行を継続する
- [x] merge 待ちが存在する場合、完了イベント/メッセージは成功完了と誤解されない形にする（AllCompleted ではなく停止扱い）

## 3. TUI 操作（手動解決）
- [x] `TuiCommand::ResolveMerge(change_id)` を追加する
- [x] `M` キーを追加し、選択中 change が `MergeWait` のときのみ `ResolveMerge` を発行する
- [x] `ResolveMerge` 実行時は base が clean であることを必須条件とし、dirty の場合は警告して中断する
- [x] base が clean の場合は選択中 change のみを resolve（マージ）し、成功したら worktree cleanup を行う
- [x] `tui-key-hints` を更新し、`MergeWait` のときのみ `M` 操作ヒントを表示する

## 4. テスト・検証
- [x] unit test: `MergeDeferred` 受信で change が `MergeWait` に遷移する
- [x] unit test: base dirty のとき個別マージを延期し、独立 change が継続し、依存 change が実行されない（キュー維持）
- [x] unit test: base dirty のとき `M` を押してもマージが実行されない
- [x] unit test: base clean のとき `M` により選択中 change のみマージが実行される
- [x] `cargo fmt`, `cargo clippy`, `cargo test` を実行する
