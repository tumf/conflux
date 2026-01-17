# Change: 共通ループでの作業ディレクトリ解決を統一する

## Why

serial と parallel の統合により apply/archive の共通ループを使用する前提になったが、作業ディレクトリ（repo root vs worktree）の指定方法が分岐したままだと、parallel 側の MUST（worktree 実行）を満たせない。共通ループに作業ディレクトリの明示的な入力を追加し、serial/parallel で一貫した cwd 解決を行う。

## What Changes

- 共通ループに `workspace_path`（任意）を渡せるようにして実行ディレクトリを統一する
- parallel 実行では worktree パスを必ず渡し、repo root での実行を禁止する
- serial 実行では `workspace_path` を省略し、従来どおり repo root で実行する

## Impact

- Affected specs: `parallel-execution`, `hooks`
- Affected code:
  - `src/orchestration/apply.rs`
  - `src/orchestration/archive.rs`
  - `src/parallel/executor.rs`
  - `src/tui/orchestrator.rs`
