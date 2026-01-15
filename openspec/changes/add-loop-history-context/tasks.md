# タスクリスト

## Phase 1: Archive 履歴機能の実装

- [ ] `src/history.rs` に `ArchiveAttempt` 構造体を追加
  - 試行回数、成功/失敗、所要時間、検証結果、エラーメッセージを含む
- [ ] `src/history.rs` に `ArchiveHistory` 構造体を追加
  - `ApplyHistory` と同様の API を提供
  - `format_context()` メソッドで履歴を XML 形式で出力
- [ ] `src/agent.rs` の `AgentRunner` に `archive_history` フィールドを追加
- [ ] `src/agent.rs` に `record_archive_attempt()` メソッドを追加
  - archive 実行後に結果を記録
- [ ] `src/agent.rs` に `clear_archive_history()` メソッドを追加
  - change の完全な完了時に履歴をクリア
- [ ] `src/agent.rs` の `run_archive_streaming()` を更新
  - archive 履歴コンテキストをプロンプトに追加
- [ ] `src/agent.rs` に `build_archive_prompt()` 関数を追加
  - `build_apply_prompt()` と同様の構造
- [ ] `src/orchestration/archive.rs` の `archive_change()` を更新
  - archive 完了後に `record_archive_attempt()` を呼び出す
- [ ] `src/orchestration/archive.rs` の `archive_change_streaming()` を更新
  - streaming 版でも履歴を記録
- [ ] `src/orchestrator.rs` の archive 成功時に `clear_archive_history()` を呼び出す

## Phase 2: Resolve コンテキスト機能の実装

- [ ] `src/history.rs` に `ResolveAttempt` 構造体を追加
  - 試行回数、成功/失敗、所要時間、検証失敗理由を含む
- [ ] `src/history.rs` に `ResolveContext` 構造体を追加
  - 現在の resolve セッション内での試行履歴を保持
  - `format_continuation_context()` メソッドで継続理由を出力
- [ ] `src/parallel/conflict.rs` の `resolve_conflicts_with_retry()` を更新
  - `ResolveContext` を作成してループ内で更新
  - 検証失敗時に失敗理由を記録
  - 次回プロンプトに前回の試行結果を追加
- [ ] `src/parallel/conflict.rs` の `resolve_merges_with_retry()` を更新
  - `ResolveContext` を使用
  - マージ検証の失敗理由を記録
  - 次回プロンプトに継続理由を追加
- [ ] `src/execution/archive.rs` の `ensure_archive_commit()` を更新
  - archive commit 作成の resolve でもコンテキストを使用

## Phase 3: テストの追加

- [ ] `src/history.rs` に `ArchiveHistory` のユニットテストを追加
  - 記録、取得、クリア、フォーマット機能をテスト
- [ ] `src/history.rs` に `ResolveContext` のユニットテストを追加
  - 継続理由の記録とフォーマットをテスト
- [ ] `src/agent.rs` に archive プロンプト構築のテストを追加
  - 初回実行時（履歴なし）
  - 2回目以降（履歴あり）
- [ ] 統合テストで archive 再試行時の履歴伝播を検証
- [ ] 統合テストで resolve 再試行時のコンテキスト伝播を検証

## Phase 4: ドキュメント更新

- [ ] `AGENTS.md` に archive と resolve の履歴機能について記載
- [ ] 設定ファイルのテンプレートにコメントを追加
  - archive と resolve でも履歴が使用されることを明記
