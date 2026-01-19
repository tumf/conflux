## Context
TUI Worktree View の衝突チェックが `git merge-tree` の引数形式違いで失敗し、競合判定が取得できない。現在は stderr の文字列に依存して判定しており、競合時の標準出力の扱いが不十分。

## Goals / Non-Goals
- Goals: `git merge-tree` を正しい引数で実行し、競合を正しく検出できるようにする。
- Goals: 競合時はエラー扱いではなく「競合あり」として判定する。
- Non-Goals: 競合解決フローや UI 表示仕様の変更。

## Decisions
- Decision: `git merge-tree --write-tree --merge-base <base> <branch1> <branch2>` を使用する。
- Decision: 競合判定は stdout を主に解析し、exit code 1 を競合として扱う。
- Decision: 失敗時の診断ログに stdout/stderr/exit code を含める。

## Risks / Trade-offs
- stdout 解析に依存するため、Git バージョン差異の影響を受ける可能性がある。

## Migration Plan
- 既存の競合チェック処理を置換するだけで移行は不要。

## Open Questions
- 競合情報の取得を `--name-only` で最適化する必要があるか。
