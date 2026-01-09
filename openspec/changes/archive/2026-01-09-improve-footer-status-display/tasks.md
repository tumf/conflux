# Tasks: improve-footer-status-display

## Implementation Tasks

- [x] 1. `render_footer_select` 関数を修正して状態別メッセージを表示
  - changes が空の場合: "Add new proposals to get started"
  - 選択数が 0 の場合: "Select changes with Space to process"
  - 選択済みの場合: "Press F5 to start processing"

- [x] 2. 実行中のフッターに進捗バーを追加
  - キュー内の全タスク数を計算（選択された changes の total_tasks 合計）
  - 完了タスク数を計算（completed_tasks 合計）
  - 進捗バーとパーセンテージを表示

- [x] 3. 単体テストを追加
  - 各状態でのフッターメッセージ表示を検証

- [x] 4. 手動テストで動作確認
  - changes なし → proposals 追加を促すメッセージ
  - 全て未選択 → 選択を促すメッセージ
  - 選択済み → F5 メッセージ
  - 実行中 → 進捗バー表示
