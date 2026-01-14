# Change: parallelモードの事前同期（base → worktree）

## Why
parallelモードでは各changeのworktreeブランチを統合先（元）へ順次マージします。統合先ブランチで直接コンフリクト解消を行うと、元ブランチの作業コピーがコンフリクト状態になりやすく、解消手順が複雑化します。

統合先（base）の最新を各worktreeへ先に取り込み、必要なコンフリクト解消をworktree側で完結させてから統合先へ戻すことで、元ブランチの負荷（衝突状態の発生、解消対象の分散、再実行コスト）を下げます。

## What Changes
- Gitバックエンドの逐次マージ手順に、統合先（base）→対象worktreeブランチの事前同期を追加します
- 事前同期でコンフリクトが発生した場合、`resolve_command` を事前同期フェーズでも適用できるようにします
- 最終的な統合コミットの形式（`Merge change: <change_id>`）は維持します

## Impact
- Affected specs: `parallel-execution`
- Affected code (expected): `src/vcs/git/*`, `src/parallel/*`, `src/parallel_run_service.rs`（実装時に確定）
- User impact: コンフリクト解消が各worktree側に寄るため、元ブランチの状態が安定しやすくなります

## Decisions
- pre-sync（base → worktree）は常に有効（必須）とする
- pre-sync は `merge` によって行い、`rebase` は行わない
- pre-sync のマージコミット subject は `Pre-sync base into <change_id>` に統一する
- pre-sync と最終統合（worktree → base）は同一の `resolve_command` 実行ループ内で完結させる
