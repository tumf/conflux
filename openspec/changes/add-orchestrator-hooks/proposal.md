# Change: オーケストレータ段階フック（hook）による任意コマンド実行

## Why

オーケストレータ実行中の各段階（開始/適用/アーカイブ/終了など）で、外部コマンド（通知、テスト、ログ収集、環境準備/後片付け等）を柔軟に実行したい。
現状は `apply_command` / `archive_command` のみで、段階的な拡張ポイントがないため運用の自動化が難しい。

## What Changes

- 設定ファイルに `hooks` セクションを追加し、段階ごとに任意コマンドを定義できるようにする。
- フックはすべてオプションとし、未設定の場合は何もしない。
- フックごとに `continue_on_failure`（デフォルト `true`）と `timeout`（デフォルト 60 秒、フックごとに変更可）を設定できる。
- フックコマンドにはプレースホルダー（例: `{change_id}`）を提供し、実行時に展開する。
- フックコマンドには環境変数（例: `OPENSPEC_CHANGE_ID`）を提供する。

## Hook Points

- `on_start`: オーケストレータ開始
- `on_first_apply`: 最初の apply 開始前（1回のみ）
- `on_iteration_start`: 各イテレーション開始
- `pre_apply`: 各 apply 前
- `post_apply`: 各 apply 後（成功時）
- `on_change_complete`: change のタスクが 100% になったとき
- `pre_archive`: archive 前
- `post_archive`: archive 後（成功時）
- `on_iteration_end`: 各イテレーション終了
- `on_queue_change`: キュー状態（残件数）が変化したとき
- `on_finish`: オーケストレータ終了（完了 or 上限到達）
- `on_error`: apply/archive の失敗時

## Impact

- Affected specs:
  - `openspec/specs/configuration/spec.md`
- Affected code:
  - `src/config.rs`, `src/orchestrator.rs`, `src/tui.rs`, `src/error.rs`（ほか hook 実行の追加）
- Backward compatibility:
  - 既存の設定（`apply_command` / `archive_command` / `analyze_command`）は変更不要で、そのまま動作する
