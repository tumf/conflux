# configuration Specification

## ADDED Requirements

### Requirement: stall_detection 設定

Orchestrator は進捗停滞検出の挙動を設定ファイルで制御できなければならない（MUST）。

- `stall_detection.enabled`: 停滞検出を有効化する（default: `true`）
- `stall_detection.threshold`: 空WIPコミット連続回数のしきい値（default: `3`）

#### Scenario: デフォルト値が適用される
- **GIVEN** 設定ファイルに `stall_detection` が存在しない
- **WHEN** orchestrator を実行する
- **THEN** `stall_detection.enabled` は `true` として扱われる
- **AND** `stall_detection.threshold` は `3` として扱われる

#### Scenario: enabled=false で停滞検出が無効化される
- **GIVEN** config 内で `stall_detection.enabled = false` が設定されている
- **WHEN** 空WIPコミットが連続して発生する
- **THEN** stall 判定は行われない

#### Scenario: threshold を変更できる
- **GIVEN** config 内で `stall_detection.threshold = 5` が設定されている
- **WHEN** 空WIPコミットが5回連続で発生する
- **THEN** stall と判定される
