# Tasks

## ✅ 1. ExecutionEvent::MergeCompleted に change_id フィールドを追加

`src/events.rs` の `MergeCompleted` variant に `change_id: String` フィールドを追加する。

**検証**:
- ✅ `cargo build` が成功すること
- ✅ `cargo clippy` でwarningが出ないこと

## ✅ 2. parallel/mod.rs の最初の MergeCompleted イベント送信を更新

`src/parallel/mod.rs:1344` 付近の `MergeCompleted` イベント送信に `change_id` を含める。

**検証**:
- ✅ `cargo build` が成功すること
- ✅ コンパイルエラーがないこと

## ✅ 3. parallel/mod.rs の2番目の MergeCompleted イベント送信を更新

`src/parallel/mod.rs:1369` 付近の `MergeCompleted` イベント送信に `change_id` を含める。

**検証**:
- ✅ `cargo build` が成功すること
- ✅ コンパイルエラーがないこと

## ✅ 4. TUI イベントハンドラに MergeCompleted ケースを追加

`src/tui/state/events.rs` の `handle_orchestrator_event` メソッドに `MergeCompleted` ハンドラを追加し、変更のステータスを `Archived` に設定する。

**実装詳細**:
- ✅ `ResolveCompleted` ハンドラと同様のロジックを使用
- ✅ マージ完了時に `elapsed_time` を記録

**検証**:
- ✅ `cargo build` が成功すること
- ✅ コンパイルエラーがないこと

## ✅ 5. 全テストを実行して回帰がないことを確認

既存のテストを実行し、変更によって破壊された機能がないことを確認する。

**検証**:
- ✅ `cargo test` が全て成功すること (671 tests passed)
- ✅ 特に `src/tui/state/events.rs` のテストが成功すること

## ✅ 6. 並列モードでの動作確認

自動テストを追加して、MergeCompleted イベント処理を検証した。

**実装内容**:
- ✅ `src/tui/state/events.rs` に `test_merge_completed_sets_archived_status` テストを追加
- ✅ MergeCompleted イベント受信時に change ステータスが `Archived` に更新されることを検証
- ✅ elapsed_time が正しく記録されることを検証
- ✅ "Merge completed" ログメッセージが追加されることを検証

**検証**:
- ✅ `cargo test test_merge_completed_sets_archived_status` が成功すること
- ✅ 全テスト (675 tests) が成功すること

## ✅ 7. ドキュメント更新

並列実行に関するドキュメントを更新した。

**更新内容**:
- ✅ `openspec/specs/parallel-execution/spec.md` の「マージ完了イベント」シナリオを更新
- ✅ イベントのフィールド名を `change_ids` (plural) から `change_id` (singular) に修正
- ✅ TUI がイベントを受け取ってステータスを更新する動作を追記

**検証**:
- ✅ ドキュメントが実装と一致していること
