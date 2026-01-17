## Context

apply/archive の共通ループを serial/parallel 両方で使うには、作業ディレクトリの解決を明示的に切り替える必要がある。parallel では worktree 実行が MUST であり、serial は repo root 実行が前提となる。

## Goals / Non-Goals

- Goals:
  - 共通ループに `workspace_path`（任意）を渡せる設計にする
  - parallel は必ず worktree で実行し、serial は従来どおり repo root で実行する
  - hook 用の `OPENSPEC_WORKSPACE_PATH` などの文脈を維持する

- Non-Goals:
  - worktree 管理の方式変更
  - hook 文脈やイベント順序の変更
  - 挙動変更（ユーザー体験、ログ文言の変更）

## Decisions

- Decision: `workspace_path` が渡された場合は cwd をそこに固定する
- Decision: parallel 側は必ず `workspace_path` を指定し、未指定はエラーとする
- Decision: serial 側は `workspace_path` を未指定のまま使用する

## Risks / Trade-offs

- 既存 API に `workspace_path` の追加が必要になり、呼び出し側の変更が広がる

## Migration Plan

1. 共通ループに `workspace_path` を追加
2. parallel 呼び出し側で worktree パスを渡す
3. serial 側は未指定で従来挙動を維持

## Open Questions

- hook 実行時の環境変数は共通ループで設定すべきか？
