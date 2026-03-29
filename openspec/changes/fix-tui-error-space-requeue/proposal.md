# Change: Error change の mark clear / re-mark による再キューを共通化する

## Why

現在の proposal は TUI の Space キー挙動に寄りすぎている。しかし実際に必要なのは UI 固有の操作定義ではなく、`selected` マークと `Error` 状態の関係を TUI / API / WebSocket で一貫させる共通仕様である。

Error になった change が `selected = true` のまま残ると、失敗済みなのに「まだ実行対象としてマークされている」ように見え、状態遷移が不自然になる。より自然な挙動は次の通りである。

- change が Error になった時点で execution mark (`selected`) は clear される
- ユーザーがその change を再度 mark した時点で、その change は再キュー対象になる
- この意味は TUI でも API / Dashboard でも同一である

## What Changes

- **共通状態遷移**: change が Error に遷移したときは `selected = false` にし、失敗済み change が実行マーク済みのまま残らないようにする。
- **再マーク時の意味づけ**: Error change を再度 mark した場合、その change は再キュー対象として扱う。TUI では Space / F5 フロー、API では selection toggle と Run フロー、WebSocket では更新済み `selected` 状態の配信で一貫させる。
- **TUI 表示**: Error 行は mark clear 後に再マーク可能であることを示すヒントを表示する。
- **API / Dashboard**: Error change の selection toggle は通常 change と同じ API を使うが、次回 Run で再実行対象になる意味を持つことを仕様化する。

## Impact

- Affected specs: `tui-architecture`, `server-api`, `tui-error-handling`
- Affected code: `src/tui/state.rs`, `src/tui/render.rs`, `src/server/api.rs`, `src/server/registry.rs`, WebSocket snapshot/update generation paths

## Out of Scope

- F5 一括リトライ機能の削除や意味変更
- Error 自体の分類やエラーメッセージ表現の変更
- 新しい retry 専用 API の追加
