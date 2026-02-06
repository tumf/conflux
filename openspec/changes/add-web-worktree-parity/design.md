## Context
Web監視機能は現在、変更一覧や実行状態の監視に強みがありますが、TUI Worktrees Viewが提供するワークツリー一覧・作成・削除・マージ操作のパリティが不足しています。運用者はWeb画面で状態を見ても、実操作のためにTUIへ切り替える必要があります。

## Goals
- Web監視でTUIと同等のWorktree取得・操作を可能にする
- RESTとWebSocketで `worktrees` 状態を一貫させる
- 失敗を隠蔽しないfail-fast設計と構造化ログを徹底する

## Non-Goals
- TUIのキー操作体系自体の変更
- VCS実装方式の刷新
- Worktree以外（changes/historyなど）の新規UI再設計

## Decisions
- `GET /api/worktrees` を追加し、Web UIの初期表示と再同期の基準データにする
- 操作APIとしてcreate/delete/mergeエンドポイントを追加し、TUIと同じ制約でバリデーションする
- WebSocket `state_update` に `worktrees` スナップショットを含め、`/api/state` と同一意味を保証する
- fail-fast方針を採用し、操作失敗時にフォールバックで成功扱いしない

## Risks
- Web経由の操作増加により失敗経路が増え、障害解析が難化する可能性
  - Mitigation: request単位の構造化ログを必須化し、操作名・対象・所要時間・エラー詳細を記録する
- TUIとWebで状態判定がずれる可能性
  - Mitigation: `state_update.worktrees` と `/api/state` のスナップショット整合を仕様化する

## Migration
- 後方互換を保つ追加変更のため、既存クライアント移行は不要
- 新規エンドポイントは段階的に公開し、Web UIは未対応時に操作ボタンを表示しない

## Open Questions
- Worktree一覧更新のトリガーを「定期ポーリング」ではなく「操作後 + WebSocket通知」に限定して十分か

## Fail-Fast Status Mapping

| 条件 | HTTPステータス | 方針 |
|---|---:|---|
| 無効な状態遷移（未マージ削除、衝突ありマージなど） | `409` | 状態不整合として即時失敗し、成功フォールバックしない |
| 対象worktreeが存在しない | `404` | 対象不在として失敗を返し、再解決を促す |
| VCS内部失敗・想定外例外 | `500` | 失敗をそのまま返し、詳細ログを残して原因調査可能にする |
