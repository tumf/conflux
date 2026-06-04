---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/server/api/git_sync.rs
  - openspec/specs/git-sync-api-errors/spec.md
  - openspec/specs/code-maintenance/spec.md
---

# git sync API の計画・実行・テスト責務分割

**Change Type**: implementation

## Problem / Context

`src/server/api/git_sync.rs` は 1,800 行超の大型ファイルで、resolve command 実行、pull/push 計画、route handler、実 git repository を使う integration tests が同居している。`build_resolve_command_argv`、`run_resolve_command`、`plan_sync` 系、git pull/push route tests が同じ compilation unit に集まっており、git 同期の小さな修正でも広い範囲の副作用を把握しづらい。

証拠:

- `src/server/api/git_sync.rs:57` に resolve command 実行ロジックがある。
- `src/server/api/git_sync.rs:693` 以降に plan sync の単体テストがある。
- `src/server/api/git_sync.rs:911` 以降に実 git repository を作る pull/push integration tests が連続している。
- `src/server/api/git_sync.rs:1666` 以降に route 登録と sync response のテストがある。

## Proposed Solution

挙動を変えずに、git sync API を「resolve command」「sync planning」「pull/push route orchestration」「test fixtures」へ分割する。先に plan と route response の characterization test を固定し、その後に内部モジュール構成を整理する。

## Acceptance Criteria

- `git pull`、`git push`、`git sync` API の route、status code、response body は変更されない。
- resolve command の quoted/unquoted `{prompt}` 展開と login shell 実行挙動は維持される。
- non-fast-forward、resolve command 未設定、already up-to-date のエラー/skip 判定は維持される。
- `cargo test server::api::git_sync` が成功する。
- Git 操作の実行順序と安全側の失敗扱いは変更されない。

## Explicit Completion Conditions

- git sync の planning と command execution が個別に読める内部構造へ分割されている。
- 既存の route tests と integration tests がリファクタ前後の同等性を示す。
- `cflx openspec validate refactor-git-sync-api --strict` と Rust テストが成功する。

## Out of Scope

- 新しい git sync 機能の追加。
- resolve command template の仕様変更。
- Git backend や VCS abstraction の置き換え。
- WebUI の表示変更。
