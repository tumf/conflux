## Context
TUI は `openspec/changes` を定期的に読み取り、Changes リストを更新します。
一方で、apply/archiving の進行により `tasks.md` や change ディレクトリが移動/消失する可能性があり、更新のたびに表示が揺れる問題が発生します。

## Goals
- 未開始で実体が消えた change を Changes リストから除外し、一覧のノイズを減らす
- 一度でも apply を開始した change は、セッション中の追跡対象として保持する

## Non-Goals
- TUI の永続的な履歴（再起動後も保持）を提供する
- Auto-refresh 間隔や UI レイアウトの変更

## Decision
- Auto-refresh の fetched 一覧に存在しない change は、原則として表示から除外する
- ただし「TUI セッション中に apply を開始した」change は例外として保持する

### Rationale
- apply 開始済みの change は、進捗/結果の追跡のために UI 上で参照できる必要がある
- 未開始で実体が消えた change は、ユーザーの操作対象でも追跡対象でもないため、一覧から外してよい

## Alternatives
- すべての change を保持する（一覧が増え続ける/古いノイズが残る）
- すべて削除する（完了直後に消えて追跡できない）

## Risks / Trade-offs
- 「apply 開始」の判定がイベント依存になるため、イベントの取りこぼしがあると意図せず除外される可能性がある
  - 対策: apply 開始に相当するイベントを網羅して状態にフラグを立てる、テストで担保する
