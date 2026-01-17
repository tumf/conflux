## Context

apply/archive の実行ループが parallel と serial に分岐しており、履歴注入・リトライ・フックイベント・キャンセル処理・WIP/スタール検知などで差分がある。これにより挙動が不一致となり、TUI と CLI の体験差や保守負荷が発生している。

## Goals / Non-Goals

- Goals:
  - apply/archive の実行ループを共通化し、挙動差分を解消する
  - parallel 側にも履歴注入を追加し、apply/archive の履歴利用を統一する
  - hook 実行順序とイベント通知を統一する
  - キャンセル時の子プロセス終了・停止通知を統一する
  - WIP/スタール検知は Git バックエンド時のみ有効とし、非 Git ではスキップする

- Non-Goals:
  - resolve の serial 実行導入（parallel 専用のまま）
  - ワークスペース管理方式（worktree/jj）の変更
  - 既存の UI 表示や表示文言の刷新

## Decisions

- apply/archive の共通ループを `orchestration` 配下に集約し、serial/parallel は引数で差分（作業ディレクトリ・イベント送信先・キャンセル方法）を渡す構造にする。
- parallel における worktree 実行は維持し、作業ディレクトリは共通ループに明示的に渡す。
- apply/archive の履歴注入は共通ループで実施し、serial/parallel で同一のフォーマットを使用する。
- キャンセルは `ManagedChild` を使用して統一し、タイムアウト時の扱いとイベント通知を揃える。
- WIP スナップショットとスタール検知は共通ループに移し、Git バックエンド時のみ有効とする。

## Risks / Trade-offs

- 既存の parallel ループから責務を移すため、移行時にイベント順序やログ出力が変わるリスクがある。
- 共通ループの汎用化により引数が増え、呼び出し側の設定ミスが起こりやすくなる。

## Migration Plan

1. 現行の apply/archive 実装を共通ループ向けに切り出す
2. serial/parallel の呼び出し側を新ループへ切り替える
3. 既存のイベント・ログ順序に差分がないか確認する
4. テストと実行ログで挙動を確認する

## Open Questions

- hook イベント通知を serial でも導入する場合、どのイベント型を流用するか（既存 TUI/CLI の扱い確認）
- Git バックエンド以外で WIP/スタール検知が無効な場合のログ/通知要件
