## Context
parallelモードでは、changeごとにworktree（専用の作業コピー/ブランチ）を作成し、apply + archive 完了後に統合先ブランチへ逐次マージします。

現在の逐次マージは「worktree → base」の統合を主軸にしており、base側でコンフリクトが起きると、統合先ブランチがコンフリクト状態になったまま解消が必要になります。

## Goals / Non-Goals
- Goals:
  - コンフリクト解消の作業場所を、可能な限り対象worktree側へ寄せる
  - 最終統合コミットの形式（`Merge change: <change_id>`）を維持する
  - 既存の `resolve_command` を活かし、収束条件を明確にする
- Non-Goals:
  - リモート運用の最適化（本提案ではローカル前提）
  - VCSバックエンド全般の刷新（Gitバックエンドの逐次マージ手順の改善に限定）
  - 履歴の書き換えを前提とする運用（rebaseを必須にはしない）

## Decisions
- Decision: pre-sync（base → worktree）は常に有効（必須）とする
- Decision: pre-sync は `merge` によって行い、`rebase` は行わない
- Decision: pre-sync のマージコミット subject は `Pre-sync base into <change_id>` に統一する
- Decision: pre-sync と最終統合（worktree → base）は同一の `resolve_command` 実行ループ内で完結させる
- Decision: 最終統合コミットの subject は `Merge change: <change_id>` を維持する

## Proposed Flow (Git backend)
1. 対象changeのworktreeブランチ（作業コピー）を取得する
2. 事前同期フェーズ: 統合先（base）の最新を、対象worktreeブランチへ取り込む（base → worktree）
   - 事前同期のマージコミット subject は `Pre-sync base into <change_id>` とする
   - コンフリクトが発生した場合、解消作業は対象worktreeの作業コピーで行う
   - 解消の収束には、既存の `resolve_command` と同等の「未解決コンフリクトなし」「マージ未完了でない」等を用いる
3. 統合フェーズ: 対象worktreeブランチを統合先ブランチへマージする（worktree → base）
   - 生成するマージコミットの subject は `Merge change: <change_id>` とする
   - 2 と 3 は同一の `resolve_command` 実行ループ内で完結させる

## Risks / Trade-offs
- 事前同期のための追加Git操作が増える（実行時間とログ量が増加）
- baseがグループ内の別change統合で更新され続けるため、「どの時点のbaseを事前同期対象とするか」を明確にする必要がある
