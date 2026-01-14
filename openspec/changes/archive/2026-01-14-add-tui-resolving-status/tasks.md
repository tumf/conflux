# Tasks

- [x] `QueueStatus` に `Resolving` を追加し、表示文字列を `resolving` にする
- [x] `M: resolve` 実行開始時に対象 change を `Resolving` に遷移させる
- [x] resolve 実行を非同期化し、TUI のメインループをブロックしないようにする
- [x] resolve 成功時は対象 change を `Archived` に遷移させ、必要に応じて worktree 状態を再取得する
- [x] resolve 失敗時は対象 change を `MergeWait` に戻し、エラー内容を警告ポップアップとログに出す
- [x] `Resolving` 中は changes list の status 表示に spinner を付けて視認性を確保する
- [x] イベント型に resolve 結果を表現するバリアントを追加し、TUI 側の `handle_orchestrator_event` で状態更新できるようにする
- [x] 既存の unit test を更新・追加して `Resolving` の表示/色/遷移が保証されるようにする
- [x] `cargo test` を実行して回帰がないことを確認する
