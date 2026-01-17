# Change: 並列/シリアル実行ループを統合して履歴・フック・キャンセル挙動を揃える

## Why

現在の実行ループは parallel と serial で別実装になっており、履歴注入・リトライ・フックイベント・キャンセル処理・WIP/スタール検知などの挙動が一致していません。この差異は運用時の期待値ズレや保守負荷の増大につながります。

今回の変更は、apply/archive の実行ループを共通化し、実装差分を最小化することで一貫した挙動を提供することを目的とします。resolve は parallel 専用のままとします。

## What Changes

- apply/archive の実行ループを共通化し、serial/parallel で同一の実行パスを利用する
- parallel 側にも apply/archive 履歴注入を追加し、履歴コンテキストの扱いを統一する
- hook 実行と hook イベント通知の挙動を serial/parallel で揃える
- キャンセル時の子プロセス終了・イベント通知を統一する
- WIP スナップショット作成は Git バックエンド時のみ有効とし、非 Git ではスキップする（スタール検知も同様に無効化）
- parallel の作業場所（worktree）と serial の作業場所（repo root）は引き続き分離し、共通ループ内で引数として扱う

## Impact

- Affected specs: `cli`, `parallel-execution`, `hooks`
- Affected code:
  - `src/orchestration/apply.rs`（共通 apply ループの中心化）
  - `src/orchestration/archive.rs`（共通 archive ループの中心化）
  - `src/parallel/executor.rs`（共通ループ呼び出しに置換）
  - `src/agent.rs`（履歴注入とキャンセル処理の統一ポイント）
  - `src/tui/orchestrator.rs`（イベント整合の影響範囲）
