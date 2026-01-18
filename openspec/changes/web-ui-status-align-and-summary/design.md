## Context
Web UI のステータスと表示構成が TUI と一致しておらず、進捗認識にズレがある。さらに一覧表示が冗長で、情報の優先順位が不明確な状態になっている。

## Goals / Non-Goals
- Goals: Web UI を TUI の QueueStatus 表記に合わせ、全体進捗を最上位に配置し、一覧をスリム化する
- Goals: 状態アイコンの追加、ループのイテレーション番号表示、操作ボタンの折りたたみを実現する
- Non-Goals: REST API の追加エンドポイントや WebSocket プロトコルの刷新は行わない

## Decisions
- Decision: Web UI の status/queue_status 表記は TUI の `QueueStatus::display()` と同一の語彙に揃える
- Decision: 全体進捗は header 直下に配置し、changes 一覧は必要最小限の情報（ID、状態、進捗、イテレーション）に絞る
- Decision: 操作ボタンは通常は非表示にし、ユーザー操作で展開する

## Risks / Trade-offs
- Web UI の表示変更により既存のスクリーンショット/ドキュメントが古くなる
- ステータス名の変更によりユーザーの慣れた表記が変わる可能性がある

## Open Questions
- イテレーション番号の取得元をどのイベント/データに依存させるか（apply/archive のどちらを表示するか）
