# Change: Worktree エラー診断とログ可観測性の改善

## Why
worktree 作成失敗時にエラーメッセージが途切れたり原因が曖昧になるため、原因特定と復旧判断に時間がかかっています。ログと TUI 表示の文脈を拡充し、診断の再現性を高めます。

## What Changes
- VCS コマンド失敗時にコマンド・作業ディレクトリ・stderr/stdout を含む診断情報を記録する
- git worktree add 失敗の代表的な原因を分類し、必要に応じて安全な再試行を行う
- TUI のログパネルで長いエラーメッセージを省略せずに閲覧できるようにする

## Impact
- Affected specs: observability, vcs-worktree-operations, cli
- Affected code: src/vcs/commands.rs, src/vcs/git/commands/worktree.rs, src/tui/state/logs.rs, src/tui/render.rs
