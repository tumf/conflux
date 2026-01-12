# Spec: parallel-execution (Delta)

## ADDED Requirements

### Requirement: Failed Change Tracking

並列実行において、失敗した変更を追跡し、依存する変更の実行判断に使用しなければならない（MUST）。

#### Scenario: Failed change recorded

- Given: 変更 `change-A` のapplyがエラーで終了した
- When: グループの実行が完了する
- Then: `change-A` は失敗した変更として記録される

#### Scenario: Failed change persists across groups

- Given: グループ1で `change-A` が失敗として記録された
- When: グループ2の実行が開始される
- Then: `change-A` は引き続き失敗した変更として追跡される

### Requirement: Dependent Change Skipping

失敗した変更に依存する変更は、自動的にスキップされなければならない（MUST）。

#### Scenario: Dependent change skipped

- Given: `change-A` が失敗として記録されている
- And: `change-B` は `change-A` に依存している
- When: `change-B` の実行が開始されようとする
- Then: `change-B` はスキップされる
- And: `ChangeSkipped` イベントが発行される

#### Scenario: Independent change continues

- Given: `change-A` が失敗として記録されている
- And: `change-C` は `change-A` に依存していない
- When: `change-C` の実行が開始されようとする
- Then: `change-C` は通常通り実行される

#### Scenario: Skip reason logged

- Given: `change-B` が依存先 `change-A` の失敗によりスキップされる
- When: スキップが発生する
- Then: ログに「Skipping change-B because dependency change-A failed」が出力される

### Requirement: ChangeSkipped Event

変更がスキップされた場合、`ChangeSkipped` イベントを発行しなければならない（MUST）。

#### Scenario: ChangeSkipped event emitted

- Given: `change-B` が依存先の失敗によりスキップされる
- When: スキップ処理が実行される
- Then: `ChangeSkipped { change_id: "change-B", reason: "Dependency 'change-A' failed" }` イベントが発行される

#### Scenario: TUI displays skipped change

- Given: TUIモードで実行中
- When: `ChangeSkipped` イベントを受信
- Then: ログペインに「Skipped: change-B (Dependency 'change-A' failed)」が表示される

## MODIFIED Requirements

### Requirement: Group Execution with Skip Check

グループ実行時に、各変更について依存先の失敗をチェックしなければならない（MUST）。

#### Scenario: Group execution with skip check

- Given: グループ2に `change-B`, `change-C` が含まれている
- And: `change-B` は失敗した `change-A` に依存している
- And: `change-C` は独立している
- When: グループ2の実行が開始される
- Then: `change-B` はスキップされる
- And: `change-C` のみが実行される

#### Scenario: All changes in group skipped

- Given: グループ内の全ての変更が失敗した依存先を持つ
- When: グループの実行が開始される
- Then: 全ての変更がスキップされる
- And: グループはエラーなく完了する（スキップはエラーではない）
