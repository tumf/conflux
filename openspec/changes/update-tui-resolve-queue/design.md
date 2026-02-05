## コンテキスト
TUI の `MergeWait` は `M` による resolve を前提としているが、resolve 実行中は `M` が無効となり、ユーザーが次の change を待ち行列に入れられない。並列実行では resolve 自体はグローバルロックで直列化されるため、TUI 側で待ち行列を明示化して順番処理するのが自然。

## 目標 / 非目標
### 目標
- resolve 実行中でも `M` による操作を受け付け、`ResolveWait` へ遷移して順番処理できること
- resolve 完了後に次の resolve を自動的に開始すること
- resolve 失敗時は自動開始せず、ユーザー操作で再開できること

### 非目標
- resolve ロジック（git 操作や AI resolve コマンド）の変更
- `MergeWait` 以外の queue 状態の意味変更

## 設計方針
- TUI 内部に手動 resolve 用の FIFO キューを持つ
  - 実装は `VecDeque<String>` + `HashSet<String>` で重複防止
- `M` 押下時の振る舞い
  - resolve 未実行中: 既存どおり `ResolveMerge` を開始
  - resolve 実行中: change を `ResolveWait` に遷移し、キューに追加
- `ResolveCompleted` を受信したらキュー先頭を取り出して次を開始
- `ResolveFailed` の場合は自動開始せず、キューは保持

## 代替案
- 代替1: resolve 実行中は `M` を拒否し続ける
  - 無反応が続き、ユーザー期待（順番処理）に合わないため不採用

## リスク / トレードオフ
- resolve 失敗時に自動開始しないため、キューが停滞する可能性がある
  - ユーザーが base をクリーンにしてから `M` を再実行すれば復帰可能とする

## 移行
既存データ構造への破壊的変更はなく、TUI の状態更新のみを拡張するため移行作業は不要。
