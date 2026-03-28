## Implementation Tasks

- [ ] 1. `src/server/api.rs`: `run_resolve_command()` を `$SHELL -l -c` 経由で実行するように変更 (verification: `resolve_command` が launchd 環境から正しくコマンドを見つけて実行できること)
- [ ] 2. `src/hooks.rs`: `execute_hook_command()` を `/bin/sh -c` から `$SHELL -l -c` に変更 (verification: hooks で PATH 依存のコマンドが実行できること)
- [ ] 3. `src/web/api.rs`: worktree command 実行を `$SHELL -l -c` 経由に変更 (verification: web API 経由の worktree command が正しく動作すること)
- [ ] 4. 共通ヘルパー関数の抽出: `agent/runner.rs` の `build_command()` と同等のロジックを再利用可能な形で提供するか、既存関数を pub にして呼び出す (verification: 3箇所すべてが同一のシェル起動ロジックを使用していること)
- [ ] 5. Windows パスの維持確認: Windows では `cmd /C` の既存動作が変わらないことを確認 (verification: `cfg!(target_os = "windows")` 分岐が保持されていること)
- [ ] 6. テスト追加: `run_resolve_command` がログインシェル経由で実行されることを検証するユニットテスト (verification: `cargo test` パス)

## Future Work

- launchd plist に `EnvironmentVariables` を設定する方法のドキュメント化（フォールバック手段として）
