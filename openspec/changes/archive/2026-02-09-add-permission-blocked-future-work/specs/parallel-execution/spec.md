## ADDED Requirements
### Requirement: Permission Auto-Reject Handling
apply実行中にエージェント出力から権限要求のauto-rejectが検出された場合、システムは当該changeを実行不能として扱わなければならない（MUST）。

システムは以下を満たさなければならない（MUST）。
- applyの再試行を停止する
- stalled/blockedとして記録する
- 理由に拒否されたパスと権限設定の案内を含める
- 空WIPコミットによるstall検出を当該changeについては実行しない
- 依存スキップの判定に反映する

#### Scenario: Permission auto-reject is detected during apply
- **GIVEN** apply出力に`permission requested`と`auto-rejecting`が含まれる
- **WHEN** applyループが出力を評価する
- **THEN** changeはstalled/blockedとして記録される
- **AND** applyの再試行は行われない
- **AND** stall検出（空WIPコミット）は実行されない
- **AND** 理由に拒否パスと権限設定の案内が含まれる

#### Scenario: Non-permission errors do not trigger permission handling
- **GIVEN** apply出力にpermission auto-rejectが含まれない
- **WHEN** applyループが出力を評価する
- **THEN** 通常の失敗処理が適用される

## MODIFIED Requirements
### Requirement: Failed Change Tracking
並列実行において、失敗した変更を追跡し、依存する変更の実行判断に使用しなければならない（MUST）。

権限auto-rejectなど人手介入が必要なapply失敗は、失敗した変更として記録しなければならない（MUST）。

#### Scenario: Failed change recorded
- Given: 変更`change-A`のapplyがエラーで終了した
- When: グループの実行が完了する
- Then: `change-A`は失敗した変更として記録される

#### Scenario: Failed change persists across groups
- Given: グループ1で`change-A`が失敗として記録された
- When: グループ2の実行が開始される
- Then: `change-A`は引き続き失敗した変更として追跡される

#### Scenario: Permission auto-reject is recorded as failed
- **GIVEN** apply出力に`permission requested`と`auto-rejecting`が含まれる
- **WHEN** changeがstalled/blockedとして扱われる
- **THEN** changeは失敗した変更として記録される
- **AND** 依存するchangeはスキップ判定の対象となる
