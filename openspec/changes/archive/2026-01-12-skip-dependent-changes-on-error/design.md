# Design: エラー時の依存チェンジセットスキップ

## Context

並列実行モードでは、変更間の依存関係を分析し、グループ単位で実行する。現在の実装では：

- 各グループは依存するグループの完了後に実行される
- グループ内で失敗した変更があっても、後続グループは実行される
- ただし、依存先の失敗を考慮した自動スキップ機能はない

例えば、以下の依存関係がある場合：
```
Group 1: [change-A, change-B]  (依存なし)
Group 2: [change-C]            (Group 1に依存)
Group 3: [change-D]            (依存なし)
```

change-Aが失敗した場合、change-Cは成功しない可能性が高い（change-Aに依存している場合）。しかし、change-Dは独立しているので実行可能。

## Goals

- 失敗した変更に依存する変更を自動的にスキップ
- 独立した変更は実行を続行
- スキップ理由をユーザーに明示

## Non-Goals

- 失敗した変更の自動リトライ
- 依存関係の動的な再計算（再分析機能で対応済み）

## Decisions

### 1. 失敗追跡の実装

**決定**: `FailedChangeTracker` 構造体で失敗した変更を追跡

```rust
pub struct FailedChangeTracker {
    /// 失敗した変更のID
    failed_changes: HashSet<String>,
    /// 変更間の依存関係 (change_id -> 依存先のchange_id)
    dependencies: HashMap<String, Vec<String>>,
}

impl FailedChangeTracker {
    pub fn mark_failed(&mut self, change_id: &str) {
        self.failed_changes.insert(change_id.to_string());
    }

    pub fn should_skip(&self, change_id: &str) -> Option<String> {
        // 依存先が失敗している場合、その失敗した依存先のIDを返す
        if let Some(deps) = self.dependencies.get(change_id) {
            for dep in deps {
                if self.failed_changes.contains(dep) {
                    return Some(dep.clone());
                }
            }
        }
        None
    }
}
```

### 2. 依存関係の取得

**決定**: LLM分析結果から変更間の依存関係を抽出

現在のグループベースの依存関係を、変更レベルの依存関係に変換：

```rust
fn extract_change_dependencies(groups: &[ParallelGroup]) -> HashMap<String, Vec<String>> {
    let mut deps = HashMap::new();
    let mut group_changes: HashMap<u32, Vec<String>> = HashMap::new();

    // グループIDごとの変更を収集
    for group in groups {
        group_changes.insert(group.id, group.changes.clone());
    }

    // 各グループの変更について、依存グループの変更を依存先として追加
    for group in groups {
        for dep_group_id in &group.depends_on {
            if let Some(dep_changes) = group_changes.get(dep_group_id) {
                for change_id in &group.changes {
                    deps.entry(change_id.clone())
                        .or_insert_with(Vec::new)
                        .extend(dep_changes.iter().cloned());
                }
            }
        }
    }

    deps
}
```

### 3. スキップ処理のタイミング

**決定**: グループ実行前に、各変更の依存先をチェック

```rust
// execute_group内
let mut changes_to_execute = Vec::new();
let mut skipped_changes = Vec::new();

for change_id in &group.changes {
    if let Some(failed_dep) = self.failed_tracker.should_skip(change_id) {
        warn!(
            "Skipping {} because dependency {} failed",
            change_id, failed_dep
        );
        skipped_changes.push((change_id.clone(), failed_dep));
    } else {
        changes_to_execute.push(change_id.clone());
    }
}

// スキップされた変更をイベントで通知
for (change_id, failed_dep) in skipped_changes {
    send_event(&self.event_tx, ParallelEvent::ChangeSkipped {
        change_id,
        reason: format!("Dependency '{}' failed", failed_dep),
    }).await;
}
```

### 4. 処理フロー

```
┌─────────────────────────────────────────────────────────────┐
│                    Group Execution Start                     │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
               ┌──────────────────────────────┐
               │ For each change in group     │
               └──────────────────────────────┘
                              │
                              ▼
               ┌──────────────────────────────┐
               │ Check: should_skip(change)?  │
               └──────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
       ┌─────────────┐                 ┌─────────────┐
       │ Dependency  │                 │ No failed   │
       │ failed      │                 │ dependencies│
       └─────────────┘                 └─────────────┘
              │                               │
              ▼                               ▼
   ┌──────────────────────┐       ┌──────────────────────┐
   │ Emit ChangeSkipped   │       │ Add to execution     │
   │ event                │       │ list                 │
   └──────────────────────┘       └──────────────────────┘
              │                               │
              └───────────────┬───────────────┘
                              │
                              ▼
               ┌──────────────────────────────┐
               │ Execute remaining changes    │
               │ in parallel                  │
               └──────────────────────────────┘
                              │
                              ▼
               ┌──────────────────────────────┐
               │ For each failed result       │
               │ → mark_failed(change_id)     │
               └──────────────────────────────┘
                              │
                              ▼
               ┌──────────────────────────────┐
               │ Continue to next group       │
               └──────────────────────────────┘
```

### 5. 再分析モードでの動作

**決定**: `execute_with_reanalysis` では、失敗した変更を除外して再分析

```rust
// 再分析ループ
while !changes.is_empty() {
    // 失敗した変更に依存する変更を除外
    let executable_changes: Vec<_> = changes
        .iter()
        .filter(|c| self.failed_tracker.should_skip(&c.id).is_none())
        .cloned()
        .collect();

    if executable_changes.is_empty() {
        info!("All remaining changes depend on failed changes, stopping");
        break;
    }

    // 再分析
    let groups = analyzer(&executable_changes).await;
    // ...
}
```

## Risks / Trade-offs

### Risk 1: 過剰なスキップ

- **リスク**: グループレベルの依存関係から推測した変更レベルの依存関係が不正確な場合、本来実行可能な変更がスキップされる可能性
- **対策**: 再分析モードでは、スキップした変更も次の分析サイクルで再評価される機会がある

### Risk 2: 依存関係情報の欠落

- **リスク**: LLM分析がグループ単位のため、同一グループ内の変更間の依存関係は追跡できない
- **対策**: 同一グループ内の変更は独立しているという前提で設計（LLM分析の結果を信頼）

## Dependencies

- `add-workspace-resume`: スキップされた変更は、後で手動または自動で再実行可能
- `preserve-workspace-on-error`: 失敗した変更のworkspaceを保持することで、依存先の修正後に再実行可能
