# 設計: タスク進捗解決ロジックの共通化

## 方針

この変更は挙動変更を目的にせず、進捗読み取りの探索順序を明示的な内部モデルへ寄せます。`tasks.md` の content parsing、path resolution、acceptance follow-up 書き込みを分離し、将来の archive/worktree 追加時に fallback 順序が分岐しないようにします。

## トレードオフ

- 非推奨 API は直ちに削除しない。互換性維持を優先し、内部委譲で重複を減らす。
- エラー文言の完全一致を必要以上に固定しないが、エラー種別と change id / 探索対象が分かる情報は維持する。
- 新しい公開型は追加しない。内部 helper に留めることで呼び出し元の変更を最小化する。

## 検証戦略

先に characterization test を追加し、探索順序と follow-up 更新挙動を固定してから内部構造を変更する。外部プロセスやネットワークは不要で、tempdir ベースの unit test で完結させる。
