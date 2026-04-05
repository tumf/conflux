---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/parallel/acceptance_state.rs
  - src/vcs/git/mod.rs
  - src/vcs/git/commands/basic.rs
  - openspec/specs/cli/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Change: acceptance state の ignore 設定を repo 共有ではなく workspace local exclude に移す

**Change Type**: implementation

## Premise / Context

- 現セッションでは `.cflx/acceptance-state.json` が working tree を dirty にし、acceptance の clean working tree check を落とす懸念が共有された。
- `src/parallel/acceptance_state.rs` は workspace local な durable state として `.cflx/acceptance-state.json` を保存している。
- 現在の repo では `.gitignore` に `.cflx/acceptance-state.json` が入っているが、これは repo tracked な ignore であり、ローカル運用ファイルまで commit 対象のポリシーへ混ぜてしまう。
- canonical spec `openspec/specs/cli/spec.md` は未追跡ファイル判定で `.gitignore` と `.git/info/exclude` の両方を適用することを要求している。
- parallel/worktree 運用では acceptance state は workspace ごとに自己完結して扱う必要がある。

## Problem / Context

`.cflx/acceptance-state.json` は acceptance/archive 再開制御に必要な内部状態ファイルだが、repo 共有の `.gitignore` に依存して除外すると、ローカル運用都合の ignore ルールを commit し続けることになる。

この状態では、workspace の生成方法や ignore 設定の伝播条件によっては acceptance state が dirty worktree 判定へ混入し、acceptance の clean working tree check を不安定にする。内部運用ファイルの除外は repository policy ではなく workspace local な Git exclude で自己完結させるべきである。

## Proposed Solution

Conflux が `.cflx/acceptance-state.json` を使う workspace に対して、repo tracked な `.gitignore` ではなく、その workspace の実効 `info/exclude` に ignore ルールを idempotent に登録する。

具体的には:

1. workspace ごとの実効 Git dir を基準に `info/exclude` を解決するヘルパーを追加する。
2. acceptance state を保存する前、または workspace 作成直後に `.cflx/acceptance-state.json` の exclude 登録を保証する。
3. 同じ exclude 行が既にある場合は重複追加しない。
4. repo root の `.gitignore` から `.cflx/acceptance-state.json` を取り除き、ローカル生成物の除外責務を workspace local exclude へ移す。
5. worktree/通常 repo の両方で `git status --porcelain` に `.cflx/acceptance-state.json` が出ないことを回帰テストで確認する。

## Acceptance Criteria

- `.cflx/acceptance-state.json` の ignore は repository-tracked `.gitignore` ではなく、workspace local な実効 `info/exclude` で管理される。
- Conflux は exclude 登録を idempotent に行い、同一 rule を重複追加しない。
- Git worktree と通常 repo の両方で `.cflx/acceptance-state.json` は `git status --porcelain` に現れない。
- acceptance/archive 用の durable state は従来どおり保存され、resume/archive guard の挙動は変わらない。
- 回帰テストで exclude 設定がない初期 workspace でも dirty worktree 判定へ acceptance state が混入しないことを確認できる。

## Out of Scope

- `.cflx/` 配下の他生成物すべてを一括 ignore する一般化
- acceptance の clean working tree ルール自体の廃止や緩和
- serial モードの Git 状態判定の再設計
