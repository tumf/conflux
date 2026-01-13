# Change: apply の各イテレーションでスナップショットを作成し最終的に squash する

## Why

並列 apply の進捗が JJ の stale や失敗時に履歴へ定着せず、作業内容が失われるリスクがあるため、各イテレーションごとの確実なスナップショットと最終的な squash が必要です。

## What Changes

- apply の各イテレーション終了後に、進捗有無に関わらずスナップショットコミットを作成する
- WIP 形式のコミットメッセージにイテレーション番号を含める
- 最終成功時に全イテレーションのスナップショットを squash し、`Apply:` コミットとして確定する
- Git/JJ 両バックエンドで同一の振る舞いを保証する

## Impact

- Affected specs: `parallel-execution`
- Affected code: `src/parallel/executor.rs`, `src/execution/apply.rs`, `src/vcs/jj/mod.rs`, `src/vcs/git/mod.rs`
