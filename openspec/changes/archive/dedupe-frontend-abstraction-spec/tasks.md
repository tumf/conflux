## Specification Tasks

- [x] 1. 旧 `EventSink トレイトによるフロントエンド抽象化` (L3-16) を削除
    verification: manual — spec diff で重複が 1 件になっていること
- [x] 2. 重複する `## Requirements` ヘッダーを 1 つに統合
    verification: integration — `openspec validate frontend-abstraction --strict` が通過
- [x] 3. `## Purpose` セクションを追加
    verification: integration — `openspec validate frontend-abstraction --strict` が通過

## Future Work

- なし
