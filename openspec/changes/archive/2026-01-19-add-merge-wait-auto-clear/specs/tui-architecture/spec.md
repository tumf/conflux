## MODIFIED Requirements

### Requirement: Event-Driven State Updates
TUI は 5 秒ごとの自動更新で `MergeWait` を評価し、以下のいずれかを満たす場合は `Queued` に戻さなければならない（MUST）。

- 対応する worktree が存在しない
- 対応する worktree が存在し、worktree ブランチが base に ahead していない

自動解除された change では `MergeWait` ではないため、`M` による merge resolve の操作ヒントや実行を行ってはならない（MUST NOT）。

#### Scenario: worktree がない場合は MergeWait を解除する
- **GIVEN** change が `MergeWait` である
- **AND** 対応する worktree が存在しない
- **WHEN** 5秒ポーリングの自動更新が実行される
- **THEN** change のステータスは `Queued` に戻る

#### Scenario: ahead なしの worktree は MergeWait を解除する
- **GIVEN** change が `MergeWait` である
- **AND** 対応する worktree が存在する
- **AND** worktree ブランチが base に ahead していない
- **WHEN** 5秒ポーリングの自動更新が実行される
- **THEN** change のステータスは `Queued` に戻る

#### Scenario: MergeWait が解除された change では M を使えない
- **GIVEN** change が `MergeWait` から `Queued` に戻っている
- **WHEN** TUI のキー表示が描画される
- **THEN** `M` による merge resolve のヒントは表示されない
