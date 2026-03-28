# Change: resolve 試行時の dirty base 遷移を理由別に分離する

**Change Type**: implementation

## Problem/Context

parallel 実行では archive 後の merge defer 時に `MergeDeferred(auto_resumable=true)` を使い、先行 merge / resolve 完了後に自動再評価される待機を `resolve pending` として扱っている。
一方で、ユーザーが `MergeWait` の change に対して手動 resolve を実行した際、base branch が dirty だと理由を区別せず `ResolveFailed` として扱われ、常に `merge wait` に戻る。
このため、「他の resolve が進行中なので少し待てば自動で進む」ケースでも、手動修復が必要な待機と同じ表示になり、自動遷移の意図が失われる。

## Proposed Solution

- 手動 resolve 開始時に base dirty で merge を開始できない場合、dirty reason を分類する
- dirty reason が他の merge / resolve 進行中で自動再評価可能な待機なら、`ResolveFailed` ではなく `MergeDeferred(auto_resumable=true)` として扱う
- dirty reason が uncommitted changes などユーザーによる workspace 修復を要する場合は、従来どおり手動介入待ちとして `merge wait` に戻す
- TUI / shared reducer / Web state で、上記分類に応じて `resolve pending` と `merge wait` を一貫表示する

## Acceptance Criteria

- 手動 resolve 実行時に base dirty reason が `MERGE_HEAD exists` 系である場合、change は `resolve pending` と表示される
- 上記 change は先行 merge / resolve 完了後の再評価で自動的に resolve / merge 再試行フローへ戻る
- 手動 resolve 実行時に base dirty reason が uncommitted changes 系である場合、change は `merge wait` と表示され、ユーザー修復まで自動進行しない
- stale refresh や reducer reconciliation により、自動再評価待ちの change が誤って `merge wait` に固定されない

## Out of Scope

- base dirty reason 判定ロジックの大規模な再設計
- merge / resolve 以外の待機状態語彙の変更
