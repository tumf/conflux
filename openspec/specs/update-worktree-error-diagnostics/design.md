## Context
worktree 作成に失敗した際、TUI のログ表示でメッセージが途中で切れたり、原因の判別が困難なケースがある。VCS コマンド失敗は stderr に原因が出力されることが多いため、ログと TUI に十分な文脈を持たせる必要がある。

## Goals / Non-Goals
- Goals:
  - git worktree add 失敗時の原因をログと TUI で判別しやすくする
  - 代表的な失敗パターン（パス重複、ブランチ重複、無効な参照、権限）を明示できるようにする
  - TUI で長文ログを省略せず確認できるようにする
- Non-Goals:
  - worktree 管理の UX 全体（ビューやキー操作）の再設計
  - 外部ツールの導入や新しい永続ストレージの追加

## Decisions
- Decision: VCS コマンド失敗時のエラー文脈は `program + args + cwd + stderr/stdout` を必須情報として扱う。
  - Why: 失敗原因の多くが stderr に出るため。
- Decision: git worktree add 失敗は代表的な原因を識別し、必要に応じて 1 回だけ安全な再試行を行う。
  - Why: 再試行で解消できるケース（stale な worktree 参照など）を自動で復旧できるため。
- Decision: TUI ログパネルは長文を折り返し表示し、スクロールで全文を確認できるようにする。
  - Why: 既存の縦スクロール機能を活かし、キー操作を増やさずに改善できるため。

## Risks / Trade-offs
- 追加のログ情報により出力が冗長になる → debug/info のレベルを維持し、TUI では折り返し表示に限定する
- 自動再試行で想定外の削除が起こる → 再試行は「安全条件」を満たす場合のみ、1 回限定で行う

## Migration Plan
1. VCS コマンド失敗の文脈拡充
2. worktree add 失敗の分類と安全な再試行
3. TUI ログパネルの折り返し表示対応
4. テスト追加とログ確認

## Open Questions
- worktree add 失敗の「安全条件」（空ディレクトリ判定など）をどこまで厳格にするか
