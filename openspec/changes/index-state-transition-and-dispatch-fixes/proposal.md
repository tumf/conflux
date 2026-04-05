---
change_type: spec-only
priority: high
dependencies: []
references:
  - openspec/changes/align-reducer-derived-status/proposal.md
  - openspec/changes/target-workspace-status-events/proposal.md
  - openspec/changes/define-rejecting-resume-state/proposal.md
  - openspec/changes/derive-reanalysis-from-scheduler-state/proposal.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-events/spec.md
  - openspec/specs/server-api/spec.md
---

# 状態遷移・dispatch 整合性の一括整理

**Change Type**: spec-only

## Problem / Context

状態遷移・イベント・dispatch 周りに複数の構造的不整合が蓄積している。
個別に起票済みの 4 proposal を統合管理し、実行順序と依存関係を明確にする。

### 発見された問題一覧

| # | 問題 | 影響 | 対応 proposal |
|---|------|------|---------------|
| 1 | WebSocket API が Reducer 正典を無視し WorkspaceState から独自に表示ステータスを導出 | TUI/Web で表示不一致 | `align-reducer-derived-status` |
| 2 | `WorkspaceStatusUpdated` が `current_change_id` に依存し、並列モードで誤った change に状態適用 | 並列実行中のステータス誤表示 | `target-workspace-status-events` |
| 3 | Rejecting → resume_apply / Rejecting → Rejected の Reducer 遷移が未定義 | rejection review 後に activity が Rejecting のまま残る | `define-rejecting-resume-state` |
| 4 | `needs_reanalysis` フラグがイベント駆動で管理され、立て忘れ/落とし過ぎで queued → applying が起きない | resolving 中に slot 空きがあっても新規 change が dispatch されない | `derive-reanalysis-from-scheduler-state` |

## Proposed Solution

上記 4 proposal を以下の順序で実行する。

## 実行順序と依存関係

```
Phase 1 (並列実行可能):
  ├── derive-reanalysis-from-scheduler-state  ← 最優先: queued→applying が動かない実害
  └── define-rejecting-resume-state           ← Reducer 遷移の穴埋め

Phase 2 (Phase 1 完了後):
  └── target-workspace-status-events          ← Reducer への書き込み経路を整理

Phase 3 (Phase 2 完了後):
  └── align-reducer-derived-status            ← 表示層を Reducer 正典に統一
```

### 理由

- Phase 1 の 2 件は独立しており並列実行可能
- `target-workspace-status-events` は Reducer イベント構造を変えるため、先に Reducer 遷移定義 (`define-rejecting-resume-state`) が完了している必要がある
- `align-reducer-derived-status` は表示層の切り替えであり、Reducer 側の正典が安定した Phase 3 で行うのが安全

## Acceptance Criteria

- 4 proposal がすべて実装・archive されている
- TUI / WebSocket API / Dashboard で同一のステータス語彙が表示される
- resolving 中に空きスロットがあれば queued change が debounce 経過後に applying に遷移する
- Rejecting 状態が resume/confirm/error のいずれかで必ず解消される
- 並列モードで WorkspaceStatus 更新が誤った change に適用されない

## Out of Scope

- Serial モードの Rejecting/Resolving サポート
- `WorkspaceStatus` enum 自体の廃止
- debounce 時間やポリシーの変更
- ダッシュボード UI コンポーネントの色/ラベル対応
