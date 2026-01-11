# Change: TUIキーナビゲーション表示位置の整理

## Why

現在、TUIの「Changes」パネルのタイトル横に全てのキーナビゲーションヒントが表示されているが、`Esc: stop` や `q: quit` といったアプリ全体の制御キーと、`Space: queue` や `@: approve` といったChanges操作のキーが混在しており、ユーザーにとって分かりにくい状態になっている。

## What Changes

- Changesパネルのタイトルには**Changes操作に関連するキーのみ**を表示する
  - `↑↓/jk: move` - リスト移動
  - `Space: queue/unqueue` - キュー操作
  - `@: approve/unapprove` - 承認操作
  - `e: edit` - 編集
  - `F5: run` - 実行開始
  - `=: parallel/sequential` - 並列モード切替
- Statusパネルの横に**アプリ全体の制御キー**を表示する
  - `Esc: stop` - 実行停止（Running/Stoppingモード時）
  - `q: quit` - 終了
  - `F5: resume` - 再開（Stoppedモード時）

## Impact

- Affected specs: `tui-key-hints`
- Affected code: `src/tui/render.rs`
  - `render_changes_list_select` 関数
  - `render_changes_list_running` 関数
  - `render_status` 関数
  - `render_footer_select` 関数
