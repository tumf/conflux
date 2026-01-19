## MODIFIED Requirements
### Requirement: すべての状態でtasks.mdの進捗を反映する
TUIは、archive/resolving中であってもtasks.mdから取得できる進捗を表示し続けなければならない（MUST）。tasks.mdの読み取りが失敗し0/0になる場合、直前の進捗を上書きしてはならない（MUST NOT）。

#### Scenario: Archive/Resolving中に0/0が返る
- **GIVEN** 変更がArchivingまたはResolving状態である
- **AND** 直前のprogressが0/0ではない
- **WHEN** 自動更新でtasks.mdの取得に失敗し0/0が返る
- **THEN** 進捗表示は直前の値を維持する
