## Context

本プロジェクトは OpenSpec change の処理（list → dependency analysis → apply → archive）を自動化している。
運用では「開始通知」「apply 前後のテスト/静的解析」「完了通知」「エラー通知」「後片付け」など段階的にフックしたい要件がある。

## Goals / Non-Goals

- Goals
  - オーケストレータの段階イベントに対して、設定ベースで任意コマンドを実行できる
  - 既存の `apply_command` / `archive_command` / `analyze_command` は維持
  - hook 失敗時の挙動を `continue_on_failure` で制御できる
  - hook 実行のタイムアウトをフックごとに設定できる

- Non-Goals
  - 1つのフックで複数コマンド（配列）をサポートしない
  - hook からオーケストレータ内部状態を直接変更する仕組みは提供しない

## Decisions

- 設定形式
  - `hooks.<hook_name>` は以下の2形式を許可する
    - 文字列（短縮形）: コマンドのみ。`continue_on_failure=true`, `timeout=60` を使用
    - オブジェクト: `command`, `continue_on_failure`, `timeout` を明示

- 実行方式
  - OS 依存差を吸収するため、`sh -c`（Windows は `cmd /C`）でコマンドを実行
  - 標準出力/標準エラーはオーケストレータのログ/表示に流れる（inherit）

- エラーハンドリング
  - hook の失敗（非0終了、実行エラー、timeout）を `continue_on_failure` に従って扱う
  - `continue_on_failure=true` の場合は警告ログのみにして継続

- コンテキスト伝搬
  - プレースホルダー（例: `{change_id}`）をコマンド文字列に展開
  - 同等の情報を環境変数（例: `OPENSPEC_CHANGE_ID`）としても注入

## Risks / Trade-offs

- hook は任意コマンド実行のため、ユーザー環境依存が大きい
- hook 失敗をデフォルト継続にすることで、運用用途（通知失敗等）でも処理を止めない

## Open Questions

- `on_queue_change` の「キュー変化」の定義（TUIとrunで差異を許容するか）
