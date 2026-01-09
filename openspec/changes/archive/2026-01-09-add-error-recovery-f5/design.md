## Context

OpenSpec Orchestrator TUIで、opencode実行がLLMエラーや料金不足等で失敗した場合の挙動改善が必要。現状はChangeのステータスが `[error]` になるが、Orchestratorのステータスパネルは `Waiting...` のままとなり、ユーザーがリカバリー操作を行えない。

関係するコンポーネント:
- `AppMode` enum: TUIの状態管理
- `QueueStatus` enum: 個別Changeのキュー状態
- `OrchestratorEvent`: orchestratorからTUIへのイベント通知
- `render_status`: ステータスパネルの描画

## Goals / Non-Goals

Goals:
- エラー発生時にOrchestratorステータスを明示的にError表示
- F5キーでエラー状態からリトライ可能にする
- ユーザーがエラー状態を認識し、適切にリカバリーできるUXを提供

Non-Goals:
- 自動リトライ機能（ユーザー操作を必須とする）
- エラー原因の詳細分析・分類
- 複数Change同時エラー時の個別リトライ（全エラーChange一括リトライ）

## Decisions

### Decision 1: AppMode::Error の追加

```rust
pub enum AppMode {
    Select,
    Running,
    Completed,
    Error,  // 新規追加
}
```

Rationale: 既存の `Completed` とは異なる状態として明示的に分離。エラー状態専用のUI表示とキー操作を可能にする。

Alternatives:
- `Running` のまま維持し、内部フラグでエラー管理 → UIロジックが複雑化
- `Completed` として扱いログのみでエラー通知 → ユーザーが見落としやすい

### Decision 2: エラー発生後の処理停止

`ProcessingError` イベント受信時点で `AllCompleted` を待たずに `AppMode::Error` へ遷移。残りのキュー内Changeは処理せず停止。

Rationale: LLMエラーや料金不足は一時的な問題の可能性が高く、継続処理より停止してユーザー確認を優先すべき。

### Decision 3: F5リトライの挙動

Error状態でF5を押すと:
1. `QueueStatus::Error` のChangeを `QueueStatus::Queued` にリセット
2. `AppMode::Running` に遷移
3. 新しいorchestratorタスクを起動（エラーChangeのみ再処理）

Rationale: エラーでないChangeは既にCompleted/Archivedなので再処理不要。

## Risks / Trade-offs

- Risk: 連続エラー発生時にユーザーが何度もF5を押す必要がある
  - Mitigation: 将来的に自動リトライ（回数制限付き）を検討可能だが、今回は手動のみ

- Trade-off: エラー発生時に残りのキュー処理を中断する
  - 複数Changeがキューにある場合、1つのエラーで全体停止となる
  - 独立したChangeは続行できる選択肢もあるが、UXの複雑化を避けて一律停止を採用

## Migration Plan

1. コード変更は後方互換性あり（既存のSelect/Running/Completedの挙動は維持）
2. 設定ファイルの変更なし
3. テスト追加で動作確認

## Open Questions

- なし（スコープは明確）
