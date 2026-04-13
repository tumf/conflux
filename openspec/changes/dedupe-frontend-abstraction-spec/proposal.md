---
change_type: spec-only
priority: medium
dependencies: []
references:
  - openspec/specs/frontend-abstraction/spec.md
---

**Change Type**: spec-only

# Change: frontend-abstraction spec の重複要件と重複セクションを統合する

## Why

`openspec/specs/frontend-abstraction/spec.md` 内で以下の問題が発生している：
- `Requirement: EventSink トレイトによるフロントエンド抽象化` が 2 回出現（行 3-16, 72-94）
- `## Requirements` ヘッダーが 2 回出現（行 1, 18）
- `## Purpose` セクションが欠落

最新版（行 72-94）は `ReducerCommand` 経由の Frontend → Core 通信規定と追加 Scenario を含む、より完全な版である。旧版は削除して最新版に統合する必要がある。

## What Changes

- 重複する `EventSink トレイトによるフロントエンド抽象化` の旧版（L3-16）を削除し、`ReducerCommand` 条項と `Frontend は ReducerCommand 経由でのみ状態を変更する` Scenario を含む版 1 つに統合
- 重複する `## Requirements` ヘッダーを 1 つに統合
- `## Purpose` セクションを追加

## Impact

- Affected specs: `frontend-abstraction`
- Affected code: なし（実装は既に EventSink + ReducerCommand の双方向経路で動作済み。spec の表現整理のみ）
