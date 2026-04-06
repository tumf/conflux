---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/parallel/executor.rs
  - src/agent/prompt.rs
  - skills/cflx-workflow/SKILL.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/agent-prompts/spec.md
---

# Change: managed worktree apply の post-apply cleanup review を追加する

**Change Type**: implementation

## Problem / Context

parallel mode では apply が分離された git worktree で実行されるため、dirty の責任は base ではなくその worktree 上の apply 成果に限定される。

一方で現行フローでは、apply 完了後に worktree が dirty のままでも acceptance に進めてしまい、acceptance あるいは archive/merge 段で dirty を理由に差し戻しや defer が発生しやすい。これは `apply -> accept` 遷移時の非効率を生み、acceptance が本来扱うべき実装検証と handoff hygiene が混在する原因になっている。

単純な自動 `git add -A` による cleanup は、本来コミットすべきでない一時ファイルや秘密情報まで巻き込む危険がある。そのため、Conflux-managed worktree に限定した post-apply cleanup review を明示的に導入し、安全に acceptance handoff できる差分だけを整理する必要がある。

## Proposed Solution

parallel mode の apply ループがタスク完了で終了したあと、`Apply:` 最終コミット作成前に post-apply cleanup review を追加する。

cleanup review は通常の apply 再実行ではなく、`cflx-workflow` skill に追加される専用 operation として agent command を起動する。cleanup review は dirty worktree を確認し、acceptance handoff のために安全に整理できる差分のみを扱う。無差別 `git add -A` は行わず、判断不能・危険・スコープ外の差分が残る場合は acceptance に進めない。

外部の coarse-grained 状態遷移は増やさず、cleanup review は apply の内部サブステップとして扱う。成功時のみ clean handoff-ready な `Apply:` 完了状態を確定し、失敗時は apply 側の失敗として現在の run で先に進めない。

## Acceptance Criteria

- parallel mode の managed worktree で apply がタスク完了した後、dirty worktree が残る場合は acceptance 開始前に cleanup review が起動される。
- cleanup review は `cflx-workflow` の専用 operation としてプロンプト構築・起動・verdict 解析ができる。
- cleanup review は無差別 `git add -A` を前提にせず、安全に handoff 可能な差分だけを整理対象とする。
- cleanup review が成功した場合のみ `Apply:` 完了状態が確定し、その後 acceptance に進む。
- cleanup review が blocked/fail の場合、change は acceptance や archive に進まず apply 側の失敗として停止する。
- acceptance は dirty worktree を検出したときの安全網を維持するが、managed worktree apply が残した dirty を主に後段で拾う構造を減らす。

## Out of Scope

- serial mode の apply semantics 変更
- archive 後 dirty を resolve で畳む既存フローの再設計
- ユーザが手動で実行する apply の責務変更
- 新しい外部表示ステータスの追加
