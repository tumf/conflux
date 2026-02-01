# 変更提案: worktree デフォルトディレクトリを cflx に統一

## Why
設定ファイルは `cflx` で統一されているのに対し、worktree のデフォルト保存先は `conflux` を使用しており整合性が崩れています。また macOS のフォールバック先が `~/Library/Application Support` 固定で Linux と挙動が異なるため、運用の一貫性が低下します。

## What Changes
- `workspace_base_dir` 未指定時のデフォルトパスを `cflx` に統一する
- macOS のデフォルトフォールバックを Linux と同じ `~/.local/share` 系に変更する
- 互換性フォールバックは行わない（旧 `conflux` パスは参照しない）

## Impact
- Affected specs: configuration
- Affected code: `src/config/defaults.rs`（デフォルトパス生成ロジックとテスト）
