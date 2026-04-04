# Change: TUI状態管理を責務ごとに分割しやすい形へ整理する

## Why
`src/tui/state.rs` は 3701 行あり、カーソル移動、ワークツリー操作、ログ管理、並列実行状態、マージガード、テストまで一箇所に集約されています。`src/tui/README.md` でも過去に `runner.rs` から責務分割した経緯が示されており、次の保守改善候補として state 周辺の分解余地が大きいです。

## What Changes
- `AppState` / `ChangeState` の公開挙動を維持したまま、内部ロジックを責務別サブモジュールへ移せる構造に整理する
- 選択操作、ログ操作、ワークツリー操作、マージガードなどの独立した責務境界を明文化する
- 既存テストを活用しつつ、主要な状態遷移を固定するキャラクタリゼーションテストを補強する

## Evidence
- `src/tui/state.rs:147` `AppState` が巨大な集約点になっている
- `src/tui/state.rs:329` 以降に多数の `impl AppState` ブロックが続いている
- `src/tui/state.rs:1270` ログ管理責務が同一ファイルに混在している
- `src/tui/state.rs:1415` 更新・マージガード・テストまで同一ファイルに存在する
- `src/tui/README.md:10` 現在も `state.rs` が状態管理の中心とされている
- `src/tui/README.md:18` 直近でも TUI 内で責務分割リファクタが行われている

## Impact
- Affected specs: `code-maintenance`, `tui-state`, `tui-state-management`
- Affected code: `src/tui/state.rs`, `src/tui/mod.rs`, 関連テスト
- API/CLI互換性: 変更なし

## Acceptance Criteria
- TUIのキーバインド、表示状態、キュー操作、ワークツリー操作の公開挙動が回帰しない
- `AppState` の主要状態遷移に対する既存テストが維持されるか強化される
- 責務ごとの分割方針が明確になり、将来の状態ロジック変更が局所化しやすくなる
