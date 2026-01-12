# 設計: Archive 完了時に個別にマージする

## アーキテクチャ変更

### 現在のアーキテクチャ

```
ParallelExecutor::execute_group()
  ├─ execute_apply_and_archive_parallel()  # 並列実行
  │   ├─ execute_apply_in_workspace()      # apply
  │   └─ execute_archive_in_workspace()    # archive
  │
  └─ merge_and_resolve(&[rev1, rev2, ...]) # グループ単位で一括マージ
```

**問題**: グループ内の全変更の `final_revision` が揃わないとマージが実行されない

### 変更後のアーキテクチャ

```
ParallelExecutor::execute_group()
  └─ execute_apply_and_archive_parallel()  # 並列実行
      ├─ execute_apply_in_workspace()      # apply
      ├─ execute_archive_in_workspace()    # archive
      └─ merge_and_resolve(&[revision])    # 🆕 archive 完了直後に個別マージ
```

**改善**: 各変更が archive 完了した時点で即座にマージ

## データフロー

### Before (グループ単位マージ)

```
Change A: apply → archive → final_revision (rev_a)
Change B: apply → archive → final_revision (rev_b)  ┐
Change C: apply → 詰まる                              ├─ 待機
                                                     ┘
                                                     
↓ (Change C が完了しないと次に進まない)

merge_and_resolve(&[rev_a, rev_b, rev_c])  # 実行されない
```

### After (個別マージ)

```
Change A: apply → archive → merge_and_resolve(&[rev_a]) ✅
Change B: apply → archive → merge_and_resolve(&[rev_b]) ✅
Change C: apply → 詰まる (他に影響なし)
```

## 実装詳細

### 1. 個別マージの追加 (`src/parallel/mod.rs`)

```rust
// execute_apply_and_archive_parallel 内
async fn execute_apply_and_archive_parallel(...) -> Result<Vec<ChangeResult>> {
    // ... (既存の並列実行ロジック)
    
    // 各タスクの結果を処理
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(change_result)) => {
                // 🆕 archive 完了直後に個別マージ
                if let Some(ref final_revision) = change_result.final_revision {
                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeStarted {
                            change_id: change_result.change_id.clone(),
                            revision: final_revision.clone(),
                        },
                    ).await;
                    
                    match self.merge_and_resolve(&[final_revision.clone()]).await {
                        Ok(merged_rev) => {
                            info!("Merged {} (revision: {})", change_result.change_id, merged_rev);
                            send_event(
                                &self.event_tx,
                                ParallelEvent::MergeCompleted {
                                    change_id: change_result.change_id.clone(),
                                    merged_revision: merged_rev,
                                },
                            ).await;
                        }
                        Err(e) => {
                            error!("Failed to merge {}: {}", change_result.change_id, e);
                            // マージ失敗は致命的なのでエラーとして扱う
                            return Err(e);
                        }
                    }
                }
                
                results.push(change_result);
            }
            // ... (エラーハンドリング)
        }
    }
    
    Ok(results)
}
```

### 2. グループ単位マージの削除 (`src/parallel/mod.rs`)

```rust
// execute_group 内
async fn execute_group(...) -> Result<()> {
    // ... (既存のロジック)
    
    // ❌ 削除: グループ単位のマージ
    // let revisions: Vec<String> = successful
    //     .iter()
    //     .filter_map(|r| r.final_revision.clone())
    //     .collect();
    // 
    // if !revisions.is_empty() {
    //     self.merge_and_resolve(&revisions).await?;
    // }
    
    // Cleanup は通常通り実行
    // ...
}
```

### 3. イベント追加 (`src/events.rs`)

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParallelEvent {
    // ... (既存のイベント)
    
    /// マージ開始
    MergeStarted {
        change_id: String,
        revision: String,
    },
    
    /// マージ完了
    MergeCompleted {
        change_id: String,
        merged_revision: String,
    },
    
    /// マージ失敗
    MergeFailed {
        change_id: String,
        error: String,
    },
}
```

### 4. TUI イベントマッピング (`src/tui/parallel_event_bridge.rs`)

```rust
pub fn parallel_event_to_orchestrator_events(event: &ParallelEvent) -> Vec<OrchestratorEvent> {
    match event {
        // ... (既存のマッピング)
        
        ParallelEvent::MergeStarted { change_id, revision } => vec![
            OrchestratorEvent::Log(
                LogEntry::info(&format!("Merging revision {}", revision))
                    .with_change_id(change_id)
            ),
        ],
        
        ParallelEvent::MergeCompleted { change_id, merged_revision } => vec![
            OrchestratorEvent::Log(
                LogEntry::success(&format!("Merged as {}", merged_revision))
                    .with_change_id(change_id)
            ),
        ],
        
        ParallelEvent::MergeFailed { change_id, error } => vec![
            OrchestratorEvent::Log(
                LogEntry::error(&format!("Merge failed: {}", error))
                    .with_change_id(change_id)
            ),
        ],
    }
}
```

## エラーハンドリング

### マージ失敗時の動作

1. **conflict 検出**
   - `merge_and_resolve` が `VcsError::Conflict` を返す
   - 既存の conflict resolution ロジックが実行される

2. **ワークスペース保持**
   - マージ失敗時もワークスペースは保持される
   - 手動での確認・修正が可能

3. **他の変更への影響**
   - マージ失敗した変更以外は正常に処理される
   - 依存関係がある場合のみスキップされる

## パフォーマンス考慮

### マージのオーバーヘッド

- **グループ単位**: 1回の `merge_and_resolve` 呼び出し（複数 revision を一括マージ）
- **個別マージ**: N回の `merge_and_resolve` 呼び出し（N = 変更数）

**トレードオフ**:
- オーバーヘッド増加: 許容範囲（数秒程度）
- 詰まり耐性: 大幅に向上（致命的な問題の解決）

### jj vs Git の違い

- **jj**: `jj new` は非常に高速（スナップショットベース）
- **Git**: `git merge` は多少時間がかかる可能性

**結論**: 個別マージによるオーバーヘッドは実用上問題ない

## 後方互換性

### 設定変更

不要。既存の設定ファイルはそのまま使用可能。

### CLI/TUI インターフェース

変更なし。ユーザーが意識する必要はない。

### Hook の実行タイミング

変更なし。`pre_apply`, `post_archive` などのタイミングは従来通り。

## 代替案

### 案1: タイムアウトの導入

- 詰まった変更を自動的に kill する
- **却下理由**: タイムアウト設定が難しい、apply の進捗状況が不明確

### 案2: グループサイズの制限

- 1グループあたりの変更数を制限する
- **却下理由**: 根本的な解決にならない、依存関係を無視できない

### 案3: 部分的なグループマージ

- 完了した変更のみをマージし、残りは次回に回す
- **却下理由**: 実装が複雑、個別マージで十分

## まとめ

**個別マージのメリット**:
- ✅ 詰まり耐性の大幅な向上
- ✅ 進捗の可視化
- ✅ リカバリの簡易化
- ✅ 実装がシンプル

**デメリット**:
- ⚠️ マージ回数の増加による多少のオーバーヘッド（許容範囲）

**結論**: 個別マージが最適な解決策
