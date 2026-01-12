# Design: Workspace レジューム機能

## Context

並列実行モードでは、各changeに対してjj workspaceまたはGit worktreeを作成して独立した作業空間を提供する。プロセスが中断した場合、これらのworkspaceがディスク上に残り、途中の作業結果も保持されている可能性がある。

現在の実装では既存workspaceを検出した場合に削除して新規作成しているが、これはユーザーの作業進捗を無駄にする。

## Goals

- 中断した作業を効率的に再開できるようにする
- 既存workspaceの自動検出と再利用オプションの提供
- jj/Git両バックエンドでの一貫した動作

## Non-Goals

- 複数プロセスからの同時workspace利用の保護（排他制御）
- 古いworkspaceの自動クリーンアップ（別機能として検討）

## Decisions

### 1. Workspace検出方式

**決定**: change_idベースのパターンマッチングで検出する

```rust
// workspace名のフォーマット: {sanitized_change_id}_{unique_suffix}
// 例: add-feature-x_1234abcd
fn find_existing_workspace(&self, change_id: &str) -> Option<WorkspaceInfo>;
```

**理由**: 
- 既存のworkspace命名規則を活用できる
- change_idから直接検索可能

### 2. 再利用判断基準

**決定**: workspaceディレクトリが存在すれば再利用可能と判断

tasks.mdの進捗状況やVCSの変更状態は判断基準に含めない。workspaceが存在すること自体が再利用の条件となる。

```rust
pub struct WorkspaceInfo {
    pub path: PathBuf,
    pub change_id: String,
    pub workspace_name: String,
    pub last_modified: SystemTime,
}
```

**理由**:
- シンプルな判断基準により実装が簡潔になる
- workspaceが存在する = 何らかの作業が行われた可能性がある
- 進捗確認は再利用後のapply loopで自動的に行われる

### 3. 自動レジューム

**決定**: 既存workspaceが検出された場合、確認なしで自動的に再利用する

```
[INFO] Resuming existing workspace for 'add-feature-x'
       Last modified: 2 hours ago
```

**理由**:
- ユーザーの操作を最小化し、効率的なワークフローを実現
- 中断からの復帰をシームレスに行える
- `--no-resume` フラグで明示的に新規作成を指定可能

### 4. 複数workspace存在時の処理

**決定**: 同一change_idに対して複数のworkspaceが存在する場合、最新のものを選択し、残りは削除する

```rust
fn find_existing_workspace(&self, change_id: &str) -> Option<WorkspaceInfo> {
    let candidates = self.find_all_workspaces_for_change(change_id);
    if candidates.is_empty() {
        return None;
    }
    
    // 最新（last_modified が最も新しい）を選択
    let (newest, others) = select_newest_workspace(candidates);
    
    // 古いworkspaceを削除
    for old_ws in others {
        self.cleanup_workspace(&old_ws.workspace_name);
    }
    
    Some(newest)
}
```

**理由**:
- 最新のworkspaceが最も有用な作業状態を保持している可能性が高い
- 古いworkspaceを残すとディスクを無駄に消費する
- 自動クリーンアップにより管理の手間を削減

### 5. 処理フロー

```
┌─────────────────────────────────────────────────────────────┐
│                    Parallel Execution Start                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                   ┌──────────────────────┐
                   │ For each change_id   │
                   └──────────────────────┘
                              │
                              ▼
               ┌────────────────────────────┐
               │ find_existing_workspace()  │
               └────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
       ┌─────────────┐                 ┌─────────────┐
       │  Not Found  │                 │   Found     │
       └─────────────┘                 └─────────────┘
              │                               │
              ▼                               ▼
    ┌─────────────────┐            ┌─────────────────────┐
    │ Create new      │            │ Check --no-resume   │
    │ workspace       │            └─────────────────────┘
    └─────────────────┘                      │
              │                 ┌────────────┴────────────┐
              │                 ▼                         ▼
              │       ┌─────────────────┐       ┌─────────────────┐
              │       │ Default:        │       │ --no-resume     │
              │       │ Auto resume     │       │ specified       │
              │       └─────────────────┘       └─────────────────┘
              │                 │                         │
              │                 ▼                         ▼
              │       ┌─────────────────┐       ┌─────────────────┐
              │       │ Reuse existing  │       │ Delete & create │
              │       │ workspace       │       │ new workspace   │
              │       │ (log progress)  │       └─────────────────┘
              │       └─────────────────┘                 │
              │                 │                         │
              └─────────────────┴─────────────────────────┘
                              │
                              ▼
                   ┌──────────────────────┐
                   │ Execute apply loop   │
                   └──────────────────────┘
```

## Risks / Trade-offs

### Risk 1: 破損したworkspaceの再利用

- **リスク**: 中断によりworkspaceが不整合な状態になっている可能性
- **対策**: 再利用前にVCS statusを確認し、コンフリクトがある場合は警告を表示

### Risk 2: 古いworkspaceの誤再利用

- **リスク**: 長期間放置されたworkspaceが誤って再利用される
- **対策**: last_modifiedを表示し、ユーザーが判断できるようにする

## Migration Plan

1. Phase 1: WorkspaceManager trait拡張とjj実装
2. Phase 2: Git worktree対応
3. Phase 3: CLI/TUI統合

## Open Questions

- [ ] workspaceの最大保持期間を設定で指定できるようにするか？
