# Design: TUI rejected change visibility

## Goal

`REJECTED.md` を durable marker として持つ change を、execution candidate list からは除外したまま、TUI change 一覧では `rejected` の read-only row として見えるようにする。

## Non-Goals

- rejected reason の新規表示 UI
- dashboard / WebSocket payload 契約の再設計
- rejected change の再実行フロー追加

## Data Flow

### Current

1. `list_changes_native()` が `REJECTED.md` を持つ change を除外する
2. TUI refresh はこの active list をそのまま一覧更新の入力として扱う
3. そのため rejected change は TUI 上で row 自体が消える

### Proposed

1. execution candidate discovery は従来どおり `list_changes_native()` を使う
2. TUI refresh では別途 `proposal.md` + `REJECTED.md` を持つ change を scan し、display-only rows を組み立てる
3. active rows と rejected rows を統合した表示用 snapshot を `update_changes()` 相当の経路に渡す
4. rejected rows には queue/selection 操作を適用しない

## State Semantics

- rejected row は terminal row であり `display_status_cache = "rejected"`
- rejected row は `selected = false` を維持する
- rejected row は `queued` / `not queued` の queue toggle 対象にしない
- marker removal 後に active listing へ戻った時だけ通常 row として再生成する

## Why split display list from execution list

`REJECTED.md` exclusion は scheduler / queue / run safety のための契約であり、運用可視化のための UI row 表示とは責務が異なる。TUI が execution list をそのまま画面リストに使い続けると、rejected change の durable outcome が hidden state になる。

表示用 snapshot を分けることで、次を同時に満たせる。

- run safety を崩さない
- rejected outcome を一覧で見えるようにする
- x マークや queue intent の誤操作を防ぐ

## Verification Plan

- TUI refresh が rejected marker-bearing change を row として追加する回帰テスト
- rejected row が Space / `@` / `F5` で selected/queued にならないテスト
- `REJECTED.md` removal 後に `not queued` / unselected へ戻る再活性化テスト
