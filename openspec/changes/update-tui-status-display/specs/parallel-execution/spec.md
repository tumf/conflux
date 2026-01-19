## MODIFIED Requirements
### Requirement: Loop termination reason must be tracked and distinguished

The system SHALL track the reason for loop termination (cancellation, graceful stop, normal completion, or merge_wait) using local state flags.

The system SHALL use this information to conditionally send completion events and messages.

加えて、`merge_wait` が残っている場合でも実行可能な change の処理が完了したときは `OrchestratorEvent::AllCompleted` を送信し、オーケストレーションは完了状態に遷移しなければならない（MUST）。

ただし、成功完了メッセージは `merge_wait` の有無を誤解させないように設計しなければならない（SHALL）。

#### Scenario: マージ待ちが残る場合でも完了イベントを送信する
- **GIVEN** 並列実行で少なくとも 1 件の change が `MergeWait` で残っている
- **AND** 実行可能な queued change の処理がすべて完了している
- **WHEN** 並列実行ループが終了処理に入る
- **THEN** システムは `OrchestratorEvent::AllCompleted` を送信する
- **AND** オーケストレーションは完了状態に遷移する
