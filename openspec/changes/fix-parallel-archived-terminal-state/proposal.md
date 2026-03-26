# 変更: パラレルモードで ChangeArchived が早期に terminal state を設定する問題の修正

## 前提 / コンテキスト

- `add-unified-change-state-machine` で `ChangeRuntimeState` に `TerminalState` enum（`Archived`, `Merged` を含む）を導入した
- パラレルモードのワークフロー: apply → accept → archive → ベースブランチへマージ → 完了
- シリアルモードのワークフロー: apply → accept → archive → 完了
- 現状、`ChangeArchived` は実行モードに関係なく無条件に `TerminalState::Archived` を設定している
- terminal 状態に入ると `is_terminal()` が `true` を返し、`MergeCompleted` イベントが無視される（`if !rt.is_terminal()` ガード）
- 結果: パラレルモードの change が "merged" ではなく "archived" で終了する
- 既存スペックのシナリオ（「Runtime state distinguishes merge wait from archived result」）は `terminal archived result` + `merge-wait` の共存を記述しているが、現実装ではこの組み合わせは矛盾している：terminal が MergeCompleted の発火を阻止する

## なぜ

パラレルモードの change が、実際にはベースブランチにマージされているにもかかわらず、最終ステータスが "archived" と表示される。これは、ステートマシンがシリアルモードとパラレルモードを同一視しているため、`ChangeArchived` が常に `TerminalState::Archived`（terminal state）を設定し、後続の `MergeCompleted` イベントが `TerminalState::Merged` に遷移できないことが原因。

## 変更内容

1. `OrchestratorState` に `ExecutionMode` enum（`Serial` | `Parallel`）を追加
2. `ChangeArchived` イベント処理をモード分岐:
   - **Serial**: `TerminalState::Archived`（terminal）— 既存動作を維持
   - **Parallel**: `WaitState::MergeWait`（non-terminal）— `MergeCompleted` の発火を許可
3. パラレル実行パス（`orchestrator.rs::run_parallel`、`tui/orchestrator.rs::run_orchestrator_parallel`）で `OrchestratorState` を `ExecutionMode::Parallel` で初期化
4. `orchestration-state` スペックのシナリオをモード依存セマンティクスに更新

## 受け入れ基準

- シリアルモードでは `ChangeArchived` が `TerminalState::Archived`（terminal）を設定 — 動作変更なし
- パラレルモードでは `ChangeArchived` が `WaitState::MergeWait`（non-terminal）を設定し、`MergeCompleted` が `TerminalState::Merged` を設定可能
- `Merged` 後の遅延イベントが状態を退行させない
- 既存テストが全て修正なしで通過（デフォルト `Serial` モード使用）
- 新規パラレルモードテストがライフサイクル全体を検証: archive → merge wait → merged

## スコープ外

- シリアルモードの `TerminalState::Archived` 表示の変更
- 実行モードの永続化ストレージの追加
- Web API ペイロード形状の変更

## 影響範囲

- 影響スペック: `orchestration-state`
- 影響コード:
  - `src/orchestration/state.rs` — `ExecutionMode` enum、モード分岐 `ChangeArchived` ハンドラ
  - `src/orchestrator.rs` — パラレルモードで `OrchestratorState` を `ExecutionMode::Parallel` で初期化
  - `src/tui/orchestrator.rs` — パラレルモードで `OrchestratorState` を `ExecutionMode::Parallel` で初期化
