# Tasks

## 1. ExecutionEvent::MergeCompleted に change_id フィールドを追加

`src/events.rs` の `MergeCompleted` variant に `change_id: String` フィールドを追加する。

**検証**:
- `cargo build` が成功すること
- `cargo clippy` でwarningが出ないこと

## 2. parallel/mod.rs の最初の MergeCompleted イベント送信を更新

`src/parallel/mod.rs:1344` 付近の `MergeCompleted` イベント送信に `change_id` を含める。

**検証**:
- `cargo build` が成功すること
- コンパイルエラーがないこと

## 3. parallel/mod.rs の2番目の MergeCompleted イベント送信を更新

`src/parallel/mod.rs:1369` 付近の `MergeCompleted` イベント送信に `change_id` を含める。

**検証**:
- `cargo build` が成功すること
- コンパイルエラーがないこと

## 4. TUI イベントハンドラに MergeCompleted ケースを追加

`src/tui/state/events.rs` の `handle_orchestrator_event` メソッドに `MergeCompleted` ハンドラを追加し、変更のステータスを `Archived` に設定する。

**実装詳細**:
- `ResolveCompleted` ハンドラと同様のロジックを使用
- マージ完了時に `elapsed_time` を記録

**検証**:
- `cargo build` が成功すること
- コンパイルエラーがないこと

## 5. 全テストを実行して回帰がないことを確認

既存のテストを実行し、変更によって破壊された機能がないことを確認する。

**検証**:
- `cargo test` が全て成功すること
- 特に `src/tui/state/events.rs` のテストが成功すること

## 6. 並列モードでの動作確認

実際に並列モードを実行して、マージ完了後のステータス表示が正しいことを確認する。

**検証手順**:
1. 複数の変更を用意
2. 並列モードで実行 (`cargo run -- run --parallel`)
3. マージが完了した変更が `Archived` (緑の completed) として表示されること
4. ログに "Merge completed" メッセージが表示されること

## 7. ドキュメント更新 (必要に応じて)

並列実行に関するドキュメントや設計ドキュメントを更新する (必要な場合のみ)。

**検証**:
- 関連ドキュメントが最新の実装と一致していること
