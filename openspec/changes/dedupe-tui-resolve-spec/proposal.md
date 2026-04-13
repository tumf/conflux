---
change_type: spec-only
priority: medium
dependencies: []
references:
  - openspec/specs/tui-resolve/spec.md
---

**Change Type**: spec-only

# Change: tui-resolve spec の重複要件を最新版に統合する

## Why

`openspec/specs/tui-resolve/spec.md` 内で以下の重複が発生している：
- `Requirement: auto-resumable-merge-deferred-triggers-resolve` が 3 回出現（行 3-17, 37-57, 60-82）
- `Requirement: resolve-merge-exclusive-execution` が 2 回出現（行 20-34, 84-118）

最新版（Project スコープの `is_resolving` と apply/accept/archive 非ブロック条項を含む版）のみを残し、他を除去する必要がある。現状では validator が複数の同名 Requirement を通してしまい、どれが canonical か曖昧になっている。また `## Purpose` セクションが欠落している。

## What Changes

- 重複する `auto-resumable-merge-deferred-triggers-resolve` の古い 2 版を削除し、Project スコープ版 1 つに統合する
- 重複する `resolve-merge-exclusive-execution` の古い版を削除し、Project スコープフラグ条項を含む版 1 つに統合する
- `## Purpose` セクションを追加する

## Impact

- Affected specs: `tui-resolve`
- Affected code: なし（実装は既に Project スコープの `is_resolving` を採用済み。spec 側の表現が実装に追いついていないだけ）
