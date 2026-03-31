---
change_type: spec-only
priority: high
dependencies: []
references:
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/tui-state-management/spec.md
  - openspec/specs/tui-mode-management/spec.md
  - openspec/specs/parallel-merge/spec.md
  - openspec/project.md
---

# Change: Orchestration / Project / Change 三層構造の仕様明確化

**Change Type**: spec-only

## Why

コードの実態は `Orchestration 1--* Project 1--* Change` の三層構造だが、仕様にこの階層が定義されておらず、以下の問題を繰り返し引き起こしている:

1. `project.md` の Domain Context に **Project** 概念が未定義。`OrchestratorState` が Project 相当の役割を担っているが仕様上は「system」としか書かれていない
2. `is_resolving` が「TUI のフラグ」と表現されており、本来 **Project レベルの resolve 直列化ガード** であることが曖昧。結果として実装者が「TUI 全体のグローバルロック」と誤解し、apply/accept/archive パイプラインまでブロックするコードが何度も書かれる
3. `parallel-merge/spec.md` だけが「プロジェクトレベル」と明示しているが、他の仕様と整合していない
4. `tui-resolve/spec.md` の `resolve-merge-exclusive-execution` が直列化スコープを「M キー操作のみ」と明記しておらず、全操作ブロックの根拠として誤読される

## What Changes

### `project.md` — Domain Context に Project 概念を追加
- Orchestration / Project / Change の三層構造を定義

### `orchestration-state/spec.md` — OrchestratorState = Project スコープを明記
- 「system」→「Project」に統一
- `is_resolving_active()` は Project スコープであることを明記

### `tui-resolve/spec.md` — is_resolving のスコープと影響範囲を限定
- 「TUI の `is_resolving`」→「Project の resolve 直列化フラグ」に修正
- resolve 直列化が影響するのは **resolve 操作同士のみ** であり、apply/accept/archive パイプラインをブロックしないことを明記

### `parallel-merge/spec.md` — 既存の「プロジェクトレベル」表現との整合
- 三層構造定義と整合する表現に統一

## Impact

- Affected specs: `orchestration-state`, `tui-resolve`, `tui-state-management`, `tui-mode-management`, `parallel-merge`
- Affected code: なし（spec-only）
- この仕様修正は `fix-resolving-blocks-other-changes` の実装根拠となる

## Acceptance Criteria

- 三層構造（Orchestration / Project / Change）が `project.md` に定義されている
- `is_resolving` のスコープが「Project レベルの resolve 直列化」と仕様上明記されている
- `is_resolving` が apply/accept/archive をブロックしないことが仕様上明記されている
- 既存の「プロジェクトレベル」表現が三層構造定義と整合している

## Out of Scope

- `is_resolving` フラグの実装修正（`fix-resolving-blocks-other-changes` で対応）
- `is_resolving` の変数名リネーム（Future Work）
