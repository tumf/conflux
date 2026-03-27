## MODIFIED Requirements

### Requirement: MergeDeferred の待ち状態判定

TUI は `MergeDeferred` を受信したとき、resolve 実行中であり対象 change が現在 resolve 中の change ではない場合、対象 change を `ResolveWait` として扱い、resolve 待ち行列に追加しなければならない（SHALL）。

resolve が実行中でない場合でも、`MergeDeferred` の理由が先行 merge / resolve 完了後に自動再評価すべき待機であると判定されている場合、TUI は対象 change を手動専用の `MergeWait` として固定表示してはならない（MUST NOT）。
その場合、先行 merge / resolve 完了後の再評価で対象 change が `ResolveWait` / `Resolving` / merge 再試行のいずれかへ自動遷移したことを表示に反映しなければならない（MUST）。

resolve 非実行時の `MergeDeferred` は、手動介入が必要と分類された場合のみ `MergeWait` のまま保持されなければならない（SHALL）。

#### Scenario: 先行 merge 完了で自動昇格した change は MergeWait に戻らない
- **GIVEN** change B が先行 merge 完了待ちの `MergeDeferred` として表示されている
- **WHEN** 先行 change の merge または resolve が完了して change B が自動再評価される
- **THEN** change B の表示は `MergeWait` のまま放置されない
- **AND** `ResolveWait` `Resolving` または再試行中の状態へ更新される

#### Scenario: 手動介入が必要な MergeDeferred は MergeWait を維持する
- **GIVEN** change B が `MergeDeferred` を受信している
- **AND** その理由分類が手動介入必要である
- **WHEN** TUI が状態を更新する
- **THEN** change B のステータスは `MergeWait` のまま維持される
- **AND** `M: resolve` ヒントが表示される
