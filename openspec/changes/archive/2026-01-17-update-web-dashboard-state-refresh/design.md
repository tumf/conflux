## Context
Webダッシュボードは現在WebStateのメモリ内容のみを参照しており、TUIの自動更新や作業ツリーの進捗がWeb側に伝わらない場合があります。手動リロードやポーリングでも最新状態を取得できず、ダッシュボードが停滞します。

## Goals / Non-Goals
- Goals:
  - /api/state などが常に最新状態を返す
  - WebSocket初期送信が最新スナップショットになる
  - 過剰なディスクアクセスを防ぐ
- Non-Goals:
  - ダッシュボードUIのデザイン変更
  - Web API仕様の追加

## Decisions
- Decision: WebStateに「ディスク/作業ツリー由来の最新スナップショット」を読み込む更新関数を追加し、REST APIやWebSocket初期送信の直前に呼ぶ。
- Decision: サーバ起動時に一定間隔でリフレッシュするタスクを追加し、TUIが動いていない状況でも状態が更新されるようにする。
- Alternatives considered:
  - TUI側イベントのみで更新する: 手動リロードで最新状態が取れないため不十分。

## Risks / Trade-offs
- 追加のファイルI/Oが発生するため、最小間隔を設けて負荷を抑える。

## Migration Plan
- Web監視機能内の更新フローのみを変更するため移行作業は不要。

## Open Questions
- なし
