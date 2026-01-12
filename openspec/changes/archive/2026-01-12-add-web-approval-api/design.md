## Context

現在のオーケストレーターは`approval.rs`モジュールで変更の承認管理を行っている。TUIでは`@`キーで承認/解除をトグルできる。Web UIからも同様の操作を可能にする必要がある。

### 既存の承認関数
- `approve_change(change_id: &str) -> Result<()>`
- `unapprove_change(change_id: &str) -> Result<()>`
- `check_approval(change_id: &str) -> Result<bool>`

## Goals / Non-Goals

### Goals
- Web UIから変更の承認/解除ができる
- 承認状態の変更がWebSocket経由でリアルタイムに反映される
- モバイルデバイスでも操作しやすいUI

### Non-Goals
- 認証・認可機能（現時点ではローカルネットワーク内の信頼された環境を前提）
- バッチ承認（一括で複数変更を承認）

## Decisions

### APIエンドポイント設計
POSTメソッドを使用し、RESTfulな設計に従う：
- `POST /api/changes/{id}/approve` - 承認
- `POST /api/changes/{id}/unapprove` - 承認解除

理由: 状態を変更する操作のため、GETではなくPOSTを使用。approveとunapproveを別エンドポイントにすることで意図が明確になる。

### UIデザイン
変更カード内に承認ボタンを配置：
- 承認済み: 緑色の「Approved」バッジ → クリックで解除
- 未承認: オレンジ色の「Pending Approval」バッジ → クリックで承認

ボタンはトグル動作として実装し、現在の状態を反転させる。

### エラーハンドリング
- 存在しない変更IDの場合: 404 Not Found
- 承認操作が失敗した場合: 500 Internal Server Error
- レスポンスにはエラーメッセージを含める

## Risks / Trade-offs

### リスク: 同時操作の競合
TUIとWeb UIで同時に承認操作が行われた場合の競合。
→ 軽減策: 最後の操作が優先される（last-write-wins）。ファイルベースの承認状態なので、原子的な操作が保証される。

### リスク: 意図しない承認解除
誤操作による承認解除。
→ 軽減策: 確認ダイアログなしで即座に反映するが、再度承認することで簡単に復旧可能。将来的に確認ダイアログを追加可能。

## Open Questions
- 承認/解除のフック（`on_approve`/`on_unapprove`）をWeb API経由の操作でも発火させるか？
  → 現時点では発火させない。TUIの操作と同様にファイルベースの操作のみ行う。
