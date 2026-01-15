# Parallel Execution Spec Delta

## MODIFIED Requirements

### Requirement: Workspace Resume Idempotency

The parallel execution mode SHALL guarantee idempotency in workspace resume processing.

When Apply/Archive/Merge processing is interrupted, errors occur, or manual intervention happens, resume SHALL work reliably. Multiple executions in the same state SHALL produce the same result.

#### Scenario: Apply中断後の再開

**Given**:
- Apply処理が実行中でWIPコミットが作成された
- Apply処理が中断された

**When**:
- 並列実行を再開

**Then**:
- 正しいワークスペース状態が検出される
- Apply処理が適切に再開される

---

## ADDED Requirements

### Requirement: WIP Snapshot Parsing

The parallel execution mode SHALL parse iteration information from WIP commit messages.

Format: `WIP: {change_id} (N/M tasks, apply#K)`

#### Scenario: WIPコミットメッセージのパース

**Given**:
- コミットメッセージ: `WIP: add-auth (8/12 tasks, apply#2)`

**When**:
- `get_latest_wip_snapshot` を実行

**Then**:
- 正しいiterationnumber情報が返される
