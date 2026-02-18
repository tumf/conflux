## Context
現在の `POST /api/v1/projects` は project 登録のみで、ローカルに clone/checkout を作成しません。追加直後に `openspec/changes` を読めないため、運用上の手戻りが発生します。

## Goals / Non-Goals
- Goals:
  - project 追加時点でローカル clone と作業ツリーが用意されている
  - clone 失敗時に registry へ不整合が残らない
  - 既存の `git/pull` 実装と整合した Git 処理を再利用する
- Non-Goals:
  - リモートからの自動同期スケジューラ追加
  - 既存 API の breaking 変更

## Decisions
- Decision: `POST /api/v1/projects` を同期処理にし、clone/worktree の準備完了後に 201 を返す
  - 理由: 追加後すぐに changes を読み取れる状態を保証するため
- Decision: bare clone は `data_dir/<project_id>` に保持し、作業ツリーは `data_dir/worktrees/<project_id>/<branch>` に作成する
  - 理由: 既存の server data_dir 配下で管理し、プロジェクトとブランチ単位で整理するため
- Decision: 追加処理でも global semaphore と project lock を適用する
  - 理由: `git/pull` などの Git 操作と競合しないよう直列化するため

## Risks / Trade-offs
- リモート接続や git 実行失敗により、追加 API のレスポンス時間が伸びる
  - Mitigation: 既存の git/pull と同等のエラーハンドリングを流用し、エラーを明確に返す

## Migration Plan
1. add_project 内に clone/worktree 準備のステップを追加
2. 失敗時のロールバック（registry から削除、必要ならローカルディレクトリ削除）を導入
3. テストを追加して成功/失敗ケースを検証

## Open Questions
- worktree 既存時の再作成ポリシー（再利用/作り直し）は最小実装で再利用とする
