## Context
現在のファイルログは `tui --logs` 指定時のみ有効で、デフォルト TUI 起動や CLI(run) では永続ログが残らない。運用時のトラブルシュートを容易にするため、常時ファイル出力と保存ポリシーの標準化が必要。

## Goals / Non-Goals
- Goals:
  - TUI/CLI の両方で常時ファイルログを出力する
  - macOS/Linux の出力先を XDG_STATE_HOME に統一する
  - project_slug と日付で分割し、7日分のみ保持する
- Non-Goals:
  - ログ内容のフォーマット変更
  - ログ圧縮や外部ログ基盤との連携

## Decisions
- Decision: `XDG_STATE_HOME/cflx/logs/<project_slug>/<YYYY-MM-DD>.log` を標準保存先とする
  - Why: OS間の一貫性が高く、リポジトリを汚さない
- Decision: 日次ローテーション + 7日保持
  - Why: 運用上の追跡期間として十分で、実装が単純
- Decision: `tui --logs` を廃止し、常時出力へ移行
  - Why: モード差分による取り漏れを防ぐ

## Risks / Trade-offs
- ログの保存先が変わるため、既存の `--logs` 運用手順は無効になる
  - Mitigation: CLI 仕様に明記し、エラーメッセージで誘導する

## Migration Plan
1. `--logs` オプションを削除
2. 常時ファイル出力の初期化を TUI/CLI 共通化
3. 日次ローテーションと7日保持のクリーンアップを追加

## Open Questions
- なし
