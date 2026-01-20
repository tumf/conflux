## Context
TUIとWebの両方で変更状態を保持しており、`OrchestratorState`の定義が2箇所に存在する。状態更新ロジックはWebState/TUI側で個別実装され、重複や不整合の温床になっている。

## Goals / Non-Goals
- Goals:
  - 共有ステートを単一のソースとして定義し、TUI/Webの状態更新を統合する。
  - `OrchestratorState`の名称衝突を解消する。
  - ExecutionEvent駆動の状態反映パスを統一する。
- Non-Goals:
  - 表示仕様の変更やUI挙動の変更。
  - 既存のイベントバリアントの削除。

## Decisions
- Decision: `src/orchestration/state.rs` を共有ステートの唯一のソースとする。
  - 理由: 将来的なCLI/TUI/Web統合の意図があり、既に専用の構造体が存在するため。
- Decision: Web側のDTOは `OrchestratorStateSnapshot` にリネームし、専用のスナップショットとして扱う。
  - 理由: 共有ステートとの名称衝突を解消し、責務を明確化するため。
- Decision: ExecutionEventの適用は共有ステートで行い、TUI/Webは参照やスナップショット取得で利用する。
  - 理由: 変更状態の分岐を減らし、一貫性を確保するため。

## Risks / Trade-offs
- Risk: 共有ステートの導入により更新経路が集約され、移行コストが発生する。
  - Mitigation: まずWeb/TUIのDTO変換を明確化し、段階的に参照モデルへ移行する。

## Migration Plan
1. ✅ 共有ステートの更新関数をExecutionEventベースで実装する。
2. ✅ WebStateのDTO変換を共有ステート由来に切り替える。
3. ✅ TUIのChangeState生成を共有ステートのスナップショットから生成する。

## Implementation Summary (Acceptance Failure Resolution)

### TUI Integration (src/tui/runner.rs)
- **共有ステート初期化**: `run_tui_loop` でchange_idsとmax_iterationsから `OrchestratorState` を初期化し、`Arc<RwLock<>>` でラップ
- **AppState注入**: `AppState::set_shared_state` 経由で共有ステート参照を注入
- **WebState注入**: TUI起動時に `WebState::set_shared_state` を呼び出し、共有ステート参照を注入

### TUI Orchestrator Integration (src/tui/orchestrator.rs)
- **シグネチャ更新**: `run_orchestrator` と `run_orchestrator_parallel` に `shared_state` パラメータ追加
- **ExecutionEvent統合**: 全主要イベントで共有ステートを更新
  - `ProcessingStarted`: 処理開始時に `apply_execution_event` 呼び出し
  - `ApplyStarted`: apply開始時に `apply_execution_event` 呼び出し
  - `ApplyCompleted`: apply成功時に `apply_execution_event` 呼び出し (apply_count自動増加)
  - `ProcessingError`: apply失敗時に `apply_execution_event` 呼び出し (pending削除)
  - `ChangeArchived`: archive成功時に `apply_execution_event` 呼び出し (pending→archivedへ移動)
- **archive_single_change統合**: `shared_state` パラメータ追加し、archive成功時に共有ステート更新
- **archive_all_complete_changes統合**: `shared_state` パラメータを転送
- **並列実行統合**: イベント転送タスクで `ParallelEvent` を共有ステートに適用

### Result
- TUI/Webともに共有ステート由来のデータを参照可能
- ExecutionEvent駆動の統一された状態更新フロー確立
- シリアル/並列両モードで共有ステート統合完了

## Open Questions (Resolved)
- ~~TUIの内部専用フィールド（カーソル、UI状態）は共有ステートに含めずに維持する方針で問題ないか。~~ → **解決**: ハイブリッドモデル採用。共有ステートはコア状態（pending/archived/apply_count）のみ保持し、UI専用状態（カーソル、スクロール位置など）はAppState/WebStateで管理。
