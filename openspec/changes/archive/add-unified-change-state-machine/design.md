## Context

現状の状態バグは、単に `QueueStatus` の実装が散らばっているからではなく、**異なる意味の状態を同じ場所で直接書き換えている**ことから生じている。

- TUI は `queue_status` を局所状態として保持し、イベント受信・キー入力・refresh により直接更新する
- orchestrator / parallel 実行側は `pending_changes`, `archived_changes`, `current_change_id`, `in_flight`, `merge_deferred_changes` などの集合や補助値で別管理する
- 5秒 refresh は workspace の観測結果から TUI の表示状態を外側から補正する

この構造では、同じ change について以下が同時に起きる。

- 「ユーザーは queued にしたい」
- 「依存関係で blocked している」
- 「archive は終わったが merge 待ち」
- 「別の resolve が実行中なので自分は resolve wait」
- 「worktree は archived だが base には未統合」

これらは別軸の事実であるにもかかわらず、現状では 1 つの flat status を複数経路が上書きしている。そのため、局所修正では別経路から再度壊れる。

## Goals / Non-Goals

**Goals**
- change ごとの runtime state を `OrchestratorState` に集約し、shared reducer を単一の mutation 境界にする
- queue 操作、execution event、workspace refresh を同じ state model に入力する
- TUI / Web は reducer から導出された display status を読むだけにする
- duplicate / stale / late input に対する precedence と idempotency を先に定義する
- `MergeWait` と `ResolveWait` の意味を明確に分離し、workspace 観測で復元できるものを限定する

**Non-Goals**
- 外部 API の語彙変更
- web dashboard の全面刷新
- reducer state の永続化
- current `pending_changes` / `archived_changes` などの既存集計フィールドをこの change で全面削除すること

## Design Summary

本変更の要点は以下の 3 つ。

1. **状態モデルを分離する**
   - queue intent
   - active activity
   - wait reason
   - terminal result
   - workspace observation
2. **状態変更の入口を reducer に限定する**
   - user command
   - execution event
   - workspace observation
3. **表示状態は導出値にする**
   - TUI / Web は `display_status(change_id)` を使う
   - `queue_status` は canonical state ではなく adapter に下げる

## Decision 1: flat enum ではなく layered runtime model を使う

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunLifecycle {
    Idle,
    Running,
    Stopping,
    Stopped,
    Completed,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueIntent {
    NotQueued,
    Queued,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityState {
    Idle,
    Applying,
    Accepting,
    Archiving,
    Resolving,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitState {
    None,
    Blocked { dependency_ids: Vec<String> },
    MergeWait { reason: String },
    ResolveWait,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalState {
    None,
    Archived,
    Merged,
    Error { stage: ActivityState, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceObservation {
    pub has_worktree: bool,
    pub is_ahead_of_base: Option<bool>,
    pub workspace_state: Option<WorkspaceState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRuntimeState {
    pub queue_intent: QueueIntent,
    pub activity: ActivityState,
    pub wait_state: WaitState,
    pub terminal: TerminalState,
    pub observation: WorkspaceObservation,
}
```

`QueueStatus` は残してよいが、**display adapter** としてのみ使う。type alias 化はしない。

### Rationale

- `Blocked` は queue intent を失わない wait 状態であり、`NotQueued` とは別概念
- `Archived` と `MergeWait` は同時に成立し得る
- `ResolveWait` は workspace から観測できる durable state ではなく、reducer-owned queue の状態
- `Merged` は terminal であり、late event で退行してはならない

## Decision 2: reducer API を唯一の mutation 境界にする

```rust
impl OrchestratorState {
    pub fn apply_command(&mut self, cmd: ReducerCommand) -> ReduceOutcome;
    pub fn apply_execution_event(&mut self, event: &ExecutionEvent) -> ReduceOutcome;
    pub fn apply_observation(
        &mut self,
        change_id: &str,
        observation: WorkspaceObservation,
    ) -> ReduceOutcome;

    pub fn display_status(&self, change_id: &str) -> QueueStatus;
    pub fn is_active_status(&self, change_id: &str) -> bool;
}
```

```rust
pub struct ReduceOutcome {
    pub state_changed: bool,
    pub warnings: Vec<String>,
    pub effects: Vec<ReducerEffect>,
}

pub enum ReducerEffect {
    EnqueueResolve { change_id: String },
    DequeueResolve { change_id: String },
}
```

### Ownership rule

- **writer**: reducer を呼ぶ orchestrator / command handler / refresh handler のみ
- **reader**: TUI / Web / logs / stats 集計
- **禁止**: TUI が局所 `queue_status` を source of truth として直接更新すること

## Decision 3: input source ごとに責務を固定する

### 3.1 Commands own user intent

`ReducerCommand` は少なくとも以下を扱う。

```rust
pub enum ReducerCommand {
    AddToQueue { change_id: String },
    RemoveFromQueue { change_id: String },
    ResolveMerge { change_id: String, resolve_running: bool },
    StopChange { change_id: String },
    RetryChange { change_id: String },
}
```

役割:
- `AddToQueue` / `RemoveFromQueue` は queue intent を更新する
- `ResolveMerge` は resolve 実行中なら `ResolveWait` に積み、そうでなければ resolve 実行可能状態へ進める
- `StopChange` は queue intent / active state / wait state を停止ルールに従って整える
- `RetryChange` は `TerminalState::Error` を clear できる唯一の経路

### 3.2 Execution events own runtime progression

最低限の event-to-state mapping を固定する。

| Event | Reducer responsibility |
|---|---|
| `ApplyStarted` | `activity = Applying`, `wait_state = None` |
| `AcceptanceStarted` | `activity = Accepting` |
| `ArchiveStarted` | `activity = Archiving` |
| `ChangeArchived` | `terminal = Archived`, `activity = Idle` |
| `MergeDeferred` | `wait_state = MergeWait { reason }` or keep `ResolveWait` if command queue says so |
| `ResolveStarted` | `activity = Resolving`, `wait_state = None` |
| `ResolveCompleted` | clear one resolve wait item, possibly emit effect for next resolve |
| `MergeCompleted` | `terminal = Merged`, `activity = Idle`, `wait_state = None` |
| `DependencyBlocked` | `wait_state = Blocked { dependency_ids }` while preserving queue intent |
| `DependencyResolved` | clear `Blocked` wait state |
| failure events | `terminal = Error { stage, message }`, `activity = Idle` |

### 3.3 Observations own reconciliation only

refresh は state を直接上書きしない。`WorkspaceObservation` を reducer に渡し、以下だけを補正する。

- `MergeWait -> queued` release
- durable wait の再構築
- active state 完了後に observation を見て次の表示状態を補正

**Observation がやってはいけないこと**
- active `Applying` / `Accepting` / `Archiving` / `Resolving` を直接上書きする
- `ResolveWait` を workspace の archived 観測だけから生成する
- `Merged` や `Error` を退行させる

## Decision 4: display status は導出値として定義する

表示優先順位を実装前に固定する。

1. active activity (`applying`, `accepting`, `archiving`, `resolving`)
2. wait state (`blocked`, `merge wait`, `resolve pending`)
3. terminal (`error`, `merged`, `archived`)
4. queue intent (`queued`, `not queued`)

### Examples

| queue_intent | activity | wait_state | terminal | display |
|---|---|---|---|---|
| Queued | Idle | None | None | `queued` |
| Queued | Idle | Blocked | None | `blocked` |
| Queued | Idle | MergeWait | Archived | `merge wait` |
| Queued | Resolving | ResolveWait | Archived | `resolving` |
| Queued | Idle | ResolveWait | Archived | `resolve pending` |
| NotQueued | Idle | None | Archived | `archived` |
| Queued | Idle | None | Merged | `merged` |

## Decision 5: `ResolveWait` は reducer-owned queue でのみ表現する

`ResolveWait` は「別の resolve が実行中なので、次の resolve 候補として順番待ちしている」ことを示す一時状態である。

### Consequences

- shared state は resolve wait queue を持つ
- `ResolveCompleted` 時に queue を dequeue して次の resolve を起動できる
- `ResolveFailed` 時は queue を保持するが、自動再開しない
- restart / reconnect 後に workspace 観測だけで `ResolveWait` を再現しない

### Correction of prior misunderstanding

`WorkspaceState::Archived` + ahead-of-base から分かるのは「archive 済みでまだ未統合」までであり、ここから復元するべき表示は `MergeWait` である。`ResolveWait` ではない。

## Decision 6: precedence / idempotency rules を明文化する

### 6.1 Terminal precedence

- `Merged` は terminal。late `ApplyStarted`, `ResolveFailed`, `DependencyBlocked` を無視する
- `Error` は retry command でのみ clear できる

### 6.2 Active precedence over observation

- active activity 中の observation は internal fact として保持するだけ
- display は active activity を優先する

### 6.3 Wait precedence over queue intent

- queued であっても blocked / merge wait / resolve wait なら display は wait を優先する

### 6.4 Duplicate inputs are no-op

- 同一 command / event / observation の再適用で状態を壊さない

### 6.5 Stop precedence

- stop 後に late event が届いても stopped/terminal を退行させない

## Decision 7: 実装境界をファイル単位で固定する

### `src/orchestration/state.rs`
- runtime model 定義
- reducer API
- display adapter
- invariant / precedence / idempotency test

### `src/tui/command_handlers.rs`
- user command を reducer command に変換
- `ReduceOutcome.effects` を見て DynamicQueue / resolve 実行へ橋渡し

### `src/tui/state.rs`
- UI local state のみ保持
- 表示・キー判定では shared reducer state を読む
- `queue_status = ...` の直接代入を削除

### `src/tui/runner.rs`
- worktree refresh から `WorkspaceObservation` を構築
- refresh reconcile の入口を提供

### `src/tui/orchestrator.rs` / `src/parallel/`
- execution event を reducer に流す
- reducer とは別に display state を持たない

### `src/web/state.rs`
- API shape は維持
- `queue_status: Option<String>` は reducer の `display_status()` 由来へ寄せる

## Decision 8: migration plan を tasks と同じ順序で進める

### Step 1: runtime model only

- reducer model を追加
- まだ TUI / Web の source of truth は切り替えない
- unit tests で invariant を固定する

### Step 2: reducer API only

- `apply_command`, `apply_execution_event`, `apply_observation` を追加
- 既存 `apply_execution_event` の legacy 集計は維持する

### Step 3: TUI command path migration

- Space / `M` / stop は reducer command を先に適用
- DynamicQueue は reducer outcome の effect を実行するだけに近づける

### Step 4: event path migration

- serial / parallel の event 経路を reducer に通す
- late/stale event regression test を追加する

### Step 5: refresh reconciliation migration

- `apply_merge_wait_status()` などの直接上書きをやめる
- refresh は observation 入力へ変更する

### Step 6: consumer migration

- TUI と Web が reducer display adapter を読む
- payload shape は変えない

### Step 7: cleanup and verification

- 直接代入の完全除去
- reducer runtime と legacy aggregate の整合性確認

## Testing Strategy

### Unit tests in `src/orchestration/state.rs`
- invariant tests
- display derivation tests
- reducer command tests
- reducer event tests
- reducer observation tests
- precedence / idempotency tests

### TUI regression tests
- queued change が別 change の merge wait で上書きされない
- `MergeWait` / `ResolveWait` に対する Space / `M` が spec 通り
- refresh 後も display が reducer と一致する

### Integration tests
- dependency blocked → resolved
- archived + merge deferred → merge wait
- resolving + queued next resolve → resolve wait queue
- stop 後の late event no-op

## Risks / Trade-offs

### Risk 1: reducer 導入で初期変更量が増える

**Mitigation**: 外部語彙と event 型は維持し、内部の ownership だけ整理する。

### Risk 2: state facets の組み合わせが増える

**Mitigation**: 許可しない組み合わせを invariant test で固定し、display も reducer 1 箇所から導出する。

### Risk 3: Web が一時的に transitional adapter になる

**Mitigation**: payload shape はそのままにし、値の算出元だけ reducer に寄せる。

## Open Questions

- なし。実装時の判断が必要な点は tasks に分割済み。
