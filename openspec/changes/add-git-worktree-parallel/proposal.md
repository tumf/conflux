# Change: Git Worktree による並列実行モードの追加

## Why

現在の並列実行モードは jj (Jujutsu) に依存しているため、jj がインストールされていない環境では並列実行ができない。Git は広く普及しており、Git Worktree 機能を使えば jj と同様の隔離された並列実行が可能になる。

## What Changes

- **VCS バックエンド自動検出**: jj 優先、jj がなければ Git、両方なければ並列実行不可
- **Git Worktree マネージャ**: `git worktree` コマンドを使用した並列実行サポート
- **未コミット変更チェック**: Git の場合、未コミット/未追跡ファイルがあればエラーで停止
- **逐次マージ**: Git では複数ブランチを1つずつマージ（コンフリクト解決を容易に）
- **CLI オプション**: `--vcs` フラグで VCS バックエンドを明示的に指定可能

**重要な制約**:
- jj および非 parallel モードの挙動は一切変更しない
- Git の場合のみ、未コミット変更があると開始できない（jj は従来通りスナップショット作成）

## Impact

- Affected specs: `parallel-execution`, `cli`, `configuration`
- Affected code:
  - `src/git_workspace.rs` (新規)
  - `src/git_commands.rs` (新規)
  - `src/vcs_backend.rs` (新規 - trait 定義)
  - `src/parallel_executor.rs` (VCS 抽象化対応)
  - `src/error.rs` (Git 用エラー型追加)
  - `src/config.rs` (vcs_backend 設定追加)
  - `src/cli.rs` (--vcs オプション追加)
  - `src/tui/` (Git 用エラーポップアップ)
