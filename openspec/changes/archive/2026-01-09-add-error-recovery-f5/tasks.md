## 1. AppModeにError状態を追加

- [x] 1.1 `AppMode` enumに `Error` バリアントを追加
- [x] 1.2 `ProcessingError` イベント受信時に `AppMode::Error` へ遷移するロジックを実装
- [x] 1.3 Error状態のヘッダー表示色（赤）を設定

## 2. ステータスパネルのError状態表示

- [x] 2.1 `render_status` 関数でError状態時の表示メッセージを実装
- [x] 2.2 エラーメッセージの内容（どのChangeでエラーが発生したか）を表示
- [x] 2.3 リトライ案内「Press F5 to retry」を表示

## 3. F5キーでのリトライ機能

- [x] 3.1 Error状態でのF5キー押下を検知するロジックを追加
- [x] 3.2 Error状態のChangeを `QueueStatus::Queued` にリセットする機能を実装
- [x] 3.3 リトライ時に新しいorchestratorタスクを起動する処理を実装
- [x] 3.4 リトライ開始時のログメッセージを追加

## 4. テスト

- [x] 4.1 `AppMode::Error` への遷移テストを追加
- [x] 4.2 Error状態でのF5リトライテストを追加
- [x] 4.3 リトライ後の状態リセット確認テストを追加
