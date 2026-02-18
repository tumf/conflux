## 背景
サーバ常駐モード導入により、TUI はローカルの change 一覧ではなく、サーバから取得した状態を表示・操作する必要がある。

## 目標 / 非目標
- 目標:
  - `--server <endpoint>` でリモート TUI を起動できる
  - リモートの change 一覧をプロジェクト単位でグルーピングして表示する
  - WebSocket 等で状態更新を購読し、TUI に反映する
  - bearer token 認証に対応する
- 非目標:
  - 既存のローカル TUI の挙動を変更する
  - サーバ API の仕様変更（サーバ側 change で定義済みの範囲外）

## 決定事項
- Decision: ローカル/リモートのデータソースを抽象化し、TUI は同一の表示モデルで扱う
  - Rationale: 表示ロジックの重複を避け、既存の更新規則を維持する
- Decision: HTTP は `reqwest`、WebSocket は `tokio-tungstenite` を使用する
  - Rationale: Tokio 環境と整合し、実績のあるクレートを用いる
- Decision: bearer token は `--server-token` と `--server-token-env` で受け取る
  - Rationale: 明示指定と環境変数の両方に対応する

## リスク / トレードオフ
- リモート API の遅延が UI 更新頻度に影響 → WS 更新を優先し、ポーリングは補助とする
- 依存クレート追加によるビルドサイズ増加 → 依存の最小化と feature 管理で緩和

## 移行計画
1. `--server` とトークン指定を追加
2. リモート API クライアントを追加
3. TUI をリモートデータソースで動作させる

## 未解決事項
- なし
