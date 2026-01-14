## Context

Resolve は `resolve_command`（AI エージェント等）にマージ/コンフリクト解消/コミット完了までを委譲します。
しかし、`resolve_command` が「コンフリクトを解消した」時点で終了し、Git が `still merging` 状態のまま残ることがあります。

Apply / Archive には「完了条件（目標）を満たすまでリトライして収束させる」発想があり、Resolve にも同様の構造が必要です。

## Goals / Non-Goals

- Goals
  - Resolve の完了条件（目標）を仕様で明確にする
  - 目標未達時に `resolve_command` を再実行して収束させる
  - 既存の最大リトライ回数ポリシーと整合させる

- Non-Goals
  - LLM のプロンプトエンジニアリングを高度化する（まずは最小限の要件追加）
  - Git 以外の VCS を新規に追加する

## Decisions

- 成功判定は「コンフリクトファイルが空」だけでは不十分とし、VCS 状態（例: `MERGE_HEAD`）を含む目標達成で判定する
- 目標を満たさない場合は `resolve_command` を同じリトライ枠の中で再実行する

## Risks / Trade-offs

- `MERGE_HEAD` の判定は Git 特有のため、VCS backend ごとに条件分岐が必要になる
- 目標判定を厳密にしすぎると、想定外の状態でリトライが増える可能性がある

## Open Questions

- `resolve_command` の再実行時に、前回の失敗理由（マージ未完了／マージコミット不足／ディレクトリ残存）をどの程度プロンプトに反映するべきか

## Notes

- Resolve の目標には「各 change_id を含むマージコミット（`Merge change: <change_id>`）の存在」を含める。
- archive 後に `openspec/changes/{change_id}` が `approved` だけ残存する場合は、ディレクトリごと削除して完了とする。
