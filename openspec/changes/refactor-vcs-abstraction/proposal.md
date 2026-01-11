# Change: VCS バックエンド抽象化の改善

## Why

現在 `jj_workspace.rs` (757行) と `git_workspace.rs` (512行) に多くの重複コードが存在する。
同様に `jj_commands.rs` と `git_commands.rs` にも共通パターンが散見される。
このまま放置すると、新しい VCS バックエンド追加時や既存機能変更時にバグが発生しやすくなる。

## What Changes

- VCS コマンド実行の共通パターンを抽出し、トレイトベースの設計に統一
- `vcs/` サブモジュールを導入し、責務を明確に分離
  - `vcs/mod.rs` - 公開 API とトレイト定義
  - `vcs/commands.rs` - 共通コマンド実行ヘルパー
  - `vcs/jj/` - Jujutsu 固有実装
  - `vcs/git/` - Git 固有実装
- エラー型を統一し、VCS 固有のエラーを共通の `VcsError` にラップ

## Impact

- 対象 specs: `code-maintenance`
- 対象コード:
  - `src/jj_workspace.rs` → `src/vcs/jj/workspace.rs`
  - `src/git_workspace.rs` → `src/vcs/git/workspace.rs`
  - `src/jj_commands.rs` → `src/vcs/jj/commands.rs`
  - `src/git_commands.rs` → `src/vcs/git/commands.rs`
  - `src/vcs_backend.rs` → `src/vcs/mod.rs`
  - `src/error.rs` - VcsError の追加
