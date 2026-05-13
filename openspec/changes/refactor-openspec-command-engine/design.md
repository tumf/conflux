# 設計: OpenSpec コマンドエンジンの責務分離

## 方針

`cflx openspec` の public entrypoint と CLI contract は維持しつつ、内部実装を責務単位に分離します。最初の目標はコードの読みやすさとテスト対象の明確化であり、新しい validation rule や archive 挙動変更は行いません。

## 推奨境界

- promotion: delta parsing、canonical merge、promotion simulation。
- validation: proposal/tasks/spec delta/evidence/archive risk の検証。
- archive: archive 前 validation、移動、spec update の orchestration。
- rendering: list/show/json/text 出力整形。
- dependency status: active/in-flight/archive/rejected dependency 分類。

## トレードオフ

- 一度に CLI contract を再設計しない。既存関数名を残し、内部委譲で分割する。
- テストの移動は最小限にする。挙動確認を優先し、モジュール分割だけで大規模なテスト再編をしない。
- strict validation のルール追加はしない。リファクタ対象は構造のみとする。

## 検証戦略

promotion、validation、archive、rendering の各 characterization test を先に通し、その後で内部境界を変える。最後に `cflx openspec list --specs` と既定テストで CLI と OpenSpec の基本動作を確認する。
