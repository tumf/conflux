# Design: server control API の責務分割

## 現状

`src/server/api/control.rs` は control API の複数責務を単一ファイルに保持している。route handler、registry mutation、WebSocket update、DB/log access、テスト fixture が近接し、局所変更でも広い文脈を読む必要がある。

## 方針

- route path と response schema は変更しない。
- 内部関数の分割だけを行い、機能追加はしない。
- characterization test を先に固定し、分割後の同等性をテストで確認する。
- `AppState` と registry の所有モデルは変更しない。

## 分割候補

- change selection 操作。
- global run/control 操作。
- stop/dequeue 操作。
- stats/logs 読み取り。
- test fixture/helper。

## Trade-offs

小さなサブモジュール化により import は増えるが、handler ごとの変更影響範囲とテスト対象が明確になる。挙動変更を避けるため、今回の change では error message の文言整理や schema 整理は行わない。
