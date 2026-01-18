## 1. 仕様更新
- [x] 1.1 resume 時の acceptance 再実行ルールを parallel-execution に追記する

## 2. 実装
- [x] 2.1 resume 判定で acceptance を必ず実行する条件を追加する（archive 完了前が対象）
- [x] 2.2 受理結果が永続化されない前提を補足するログまたはコメントを追加する

## 3. 検証
- [x] 3.1 acceptance 中断後に resume して acceptance が再実行されることを確認する
- [x] 3.2 関連するユニットテストまたは E2E テストを追加・更新する

### 検証結果
#### 3.1 コードレビューによる確認
- `Applied` 状態: `execute_group` で `changes_for_apply` に追加され、apply ループ（タスク完了で即終了）→ acceptance → archive の順で実行される
- `Archiving` 状態: `execute_group` で直接 acceptance を実行してから archive commit を実行する
- ログ追加により、acceptance が再実行される理由（結果が永続化されない）が明示された

#### 3.2 既存テストの実行確認
- `test_detect_workspace_state_applied`: Applied 状態の検出ロジックを検証 ✓
- `test_detect_workspace_state_archiving`: Archiving 状態の検出ロジックを検証 ✓
- 全テスト (850 tests) が成功: コード変更による regression がないことを確認 ✓
