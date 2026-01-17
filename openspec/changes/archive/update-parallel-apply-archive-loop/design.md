## Context

serial と parallel で apply/archive の実行ループが分かれており、CommandQueue、履歴注入、キャンセル、hook 実行やイベント通知の挙動が一致していない。既に `orchestration/apply.rs` と `orchestration/archive.rs` に共通ループの基盤があるため、それを正として parallel を統合する。

## Goals / Non-Goals

- Goals:
  - serial を正とした共通ループを parallel 実行でも利用する
  - worktree 実行・ParallelEvent といった parallel 固有の差分は薄い変換レイヤで吸収する
  - apply/archive の retry/cancel/hook の挙動を統一する

- Non-Goals:
  - resolve の統合（今回は対象外）
  - parallel 実行のワークスペース管理/依存関係解析の変更
  - 挙動変更（ユーザー体験、イベント順序、ログ文言、エラー処理など）の追加

## Decisions

- Decision: `orchestration/*` の共通ループを正とし、parallel 側はそれを呼び出す構造にする
- Decision: worktree の cwd 指定は共通ループにパラメータとして渡し、実行場所の差分を吸収する
- Decision: 出力は OutputHandler → ParallelEvent へ変換するブリッジを用意する

## Risks / Trade-offs

- 共通ループに worktree 実行の引数が追加されることで API が複雑化する
- OutputHandler 変換の責務が増えるため、TUI のログ出力に影響が出る可能性がある

## Migration Plan

1. apply の共通ループが parallel から呼べるよう引数と出力レイヤを整理する
2. archive の共通ループも同様に置き換える
3. parallel 側の独自ループを削除/縮小する
4. serial/parallel の挙動差分（retry/cancel/hook）を比較検証する

## Open Questions

- 共通ループに worktree 情報を渡す方法は `Path` 引数の追加で十分か？
- 既存の ParallelEvent の粒度を維持するために OutputHandler の拡張が必要か？
