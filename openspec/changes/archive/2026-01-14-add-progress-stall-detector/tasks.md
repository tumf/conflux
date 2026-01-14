# タスク一覧: 進捗停滞検出（空WIPコミット連続）

## 1. 仕様・設定

- [x] `stall_detection.enabled`（default: true）を追加
- [x] `stall_detection.threshold`（default: 3）を追加
- [x] stall 判定の対象を `apply` / `archive` に限定し、`resolve` は out of scope とする

## 2. WIP と stall 検出（共通）

- [x] WIP コミット直後に「空コミット（親との差分なし）」を判定するヘルパを追加
- [x] change ごと・phase ごとに「空コミット連続回数」を保持し、非空コミットでリセットする
- [x] 連続回数が `threshold` に達したら stall と判定する

## 3. apply ループ（serial / parallel）

- [x] apply 反復の WIP 作成は既存仕様を維持しつつ、空コミット連続判定に接続
- [x] stall 発生時は change を停止（failed 扱い）として扱う
- [x] 停止 change に依存する change はこの run ではスキップし、依存しない queued change は継続する

## 4. archive ループ（serial / parallel）

- [x] archive retry 反復ごとに `WIP(archive)`（`--allow-empty`）を作成する
- [x] `WIP(archive)` の空コミット連続判定で stall したら、その change の archive を中断し停止扱いにする
- [x] archive 成功時に `WIP(archive)` を phase 別に squash し、最終 `Archive:` コミットを残す

## 5. 依存スキップ

- [x] parallel: stall を failure として扱い、既存の依存スキップ機構に乗せる
- [x] serial: 停止済み change を依存に含む queued change を、この run の選択対象から除外する

## 6. テスト

- [x] 空コミットが `threshold` 回連続した場合に stall すること
- [x] 非空コミットが挟まると連続カウントがリセットされること
- [x] `enabled=false` の場合は stall しないこと
- [x] 停止 change の依存先がスキップされ、独立 change が継続されること
