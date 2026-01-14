# Change: parallel の最終マージを base dirty 時に延期し、MergeWait と手動解決を提供する

## なぜ
parallel モードの resolve 最終段（base へのマージ）で base ブランチが dirty の場合、マージが失敗して処理が中断しやすく、ユーザが「base を clean にしてから安全に再開する」ための明確な手順と状態表示が不足している。

## 何が変わるか
- parallel 実行中の「archive 完了後の個別マージ」は、base が dirty（未コミット変更、未追跡、またはマージ進行中）であればマージを実行せず、対象 change を `MergeWait` として保持する
- `MergeWait` の change は TUI 上で明示され、ユーザが base cleanup 後に `M` キーで選択中の change のみを resolve（マージ）できる
- `MergeWait` の change に依存しない queued change は実行を継続する
- `MergeWait` の change に依存する queued change はキューに残したまま実行しない（自動再開はしない）

## 影響
- 影響する仕様: `parallel-execution`, `tui-key-hints`, `tui-architecture`
- 影響する主なコード領域（参考）: parallel executor の merge フロー、TUI のキーハンドリング、イベント型（ExecutionEvent/TuiCommand）

## 互換性
- 既存の並列適用/アーカイブ処理の意味は維持する
- ただし base dirty 時の「個別マージは即時に行う」という挙動は変更され、明示的に延期される

## 成功条件
- base が dirty のとき、個別マージは実行されず `MergeWait` に遷移する
- `MergeWait` の存在は TUI で判別でき、`M` により選択中 change のみを解決できる
- base が dirty のままでは `M` によるマージは実行されない
- `MergeWait` に依存しない queued change は実行され、依存する queued change はキューに残る
