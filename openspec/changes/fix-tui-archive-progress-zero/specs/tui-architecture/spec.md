## MODIFIED Requirements
### Requirement: すべての状態でtasks.mdの進捗を反映する
TUIは、archive/resolving中であってもtasks.mdから取得できる進捗を表示し続けなければならない（MUST）。tasks.mdの読み取りが失敗し0/0になる場合、直前の進捗を上書きしてはならない（MUST NOT）。
自動更新処理において、active locationから0/0が返った場合はアーカイブ先を試し、それでも0/0なら既存値を保持しなければならない（MUST）。

#### Scenario: Archive/Resolving中に0/0が返る
- **GIVEN** 変更がArchivingまたはResolving状態である
- **AND** 直前のprogressが0/0ではない
- **WHEN** 自動更新でtasks.mdの取得に失敗し0/0が返る
- **THEN** 進捗表示は直前の値を維持する

#### Scenario: アーカイブ移動直後の自動更新で進捗を保持する
- **GIVEN** 変更がArchiving状態であり、worktree上でtasks.mdがアーカイブ先へ移動されている
- **AND** 直前のprogressが0/0ではない
- **WHEN** 自動更新で `parse_change_with_worktree_fallback` が0/0を返す
- **THEN** `parse_archived_change_with_worktree_fallback` を試みる
- **AND** それでも0/0なら既存の進捗値を保持する
